//! Live feeds for views that are not plain object lists: the Overview (Nodes + metrics),
//! cluster Events, and full manifests (Argo CD Applications). Same model as the resource
//! watch: one snapshot, a watch resumed from its cursor, and changes batched every 50 ms.

use super::live_resources::resume_signal;
use super::*;
use futures_util::stream::BoxStream;
use std::collections::BTreeMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const BATCH_INTERVAL: Duration = Duration::from_millis(50);
// Metrics cannot be watched; poll often and publish only when metrics-server has a new sample.
const METRICS_INTERVAL: Duration = Duration::from_secs(3);
const EVENT_SNAPSHOT_LIMIT: u32 = 500;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LiveFeedRequest {
    kubeconfig_path: Option<String>,
    context: Option<String>,
    /// "overview", "events", or "manifests"
    kind: String,
    /// The resource type for "manifests".
    resource: Option<ResourceRequest>,
}

struct FeedPublisher {
    app: tauri::AppHandle,
    id: String,
    sequence: u64,
}

impl FeedPublisher {
    fn emit(&mut self, action: &str, payload: Value) {
        self.sequence += 1;
        let mut message =
            serde_json::json!({ "feedId": self.id, "sequence": self.sequence, "action": action });
        if let (Some(target), Value::Object(fields)) = (message.as_object_mut(), payload) {
            target.extend(fields);
        }
        let _ = self.app.emit("kuberniva://live-feed", message);
    }
}

enum FeedEvent {
    Upsert(String, Value),
    Delete(String),
    Bookmark(String),
    Expired,
    Forbidden(String),
    Failed(String),
}

fn status_event(status: &kube::core::Status) -> FeedEvent {
    match status.code {
        410 => FeedEvent::Expired,
        403 => FeedEvent::Forbidden(status.to_string()),
        _ => FeedEvent::Failed(status.to_string()),
    }
}

fn object_key(metadata: &k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta) -> String {
    metadata.uid.clone().unwrap_or_else(|| {
        format!(
            "{}/{}",
            metadata.namespace.clone().unwrap_or_default(),
            metadata.name.clone().unwrap_or_default()
        )
    })
}

fn event_value(event: Event) -> Option<(String, Value)> {
    let key = object_key(&event.metadata);
    let row = cluster_event_from(event)?;
    Some((key, serde_json::to_value(row).ok()?))
}

fn manifest_value(mut object: DynamicObject) -> Option<(String, Value)> {
    object.metadata.managed_fields = None;
    let key = object_key(&object.metadata);
    Some((key, serde_json::to_value(object).ok()?))
}

fn is_expired(error: &kube::Error) -> bool {
    matches!(error, kube::Error::Api(response) if response.code == 410)
}

fn is_forbidden(error: &kube::Error) -> bool {
    matches!(error, kube::Error::Api(response) if response.code == 403)
}

enum KeyedKind {
    Events,
    Manifests(ResourceRequest),
}

async fn list_keyed(
    client: Client,
    kind: &KeyedKind,
) -> Result<(Vec<(String, Value)>, String), kube::Error> {
    match kind {
        KeyedKind::Events => {
            let list = Api::<Event>::all(client)
                .list(&ListParams::default().limit(EVENT_SNAPSHOT_LIMIT))
                .await?;
            let version = list.metadata.resource_version.unwrap_or_default();
            Ok((
                list.items.into_iter().filter_map(event_value).collect(),
                version,
            ))
        }
        KeyedKind::Manifests(request) => {
            let api = dynamic_api_for_request(client, request);
            let mut items = Vec::new();
            let mut continuation = String::new();
            loop {
                let list = api
                    .list(
                        &ListParams::default()
                            .limit(500)
                            .continue_token(&continuation),
                    )
                    .await?;
                let version = list.metadata.resource_version.clone().unwrap_or_default();
                items.extend(list.items.into_iter().filter_map(manifest_value));
                let next = list.metadata.continue_.unwrap_or_default();
                if next.is_empty() || next == continuation {
                    return Ok((items, version));
                }
                continuation = next;
            }
        }
    }
}

async fn watch_keyed(
    client: Client,
    kind: &KeyedKind,
    version: &str,
) -> Result<BoxStream<'static, Result<FeedEvent, kube::Error>>, kube::Error> {
    let params = WatchParams::default().timeout(290);
    match kind {
        KeyedKind::Events => Ok(Api::<Event>::all(client)
            .watch(&params, version)
            .await?
            .map(|event| {
                event.map(|event| match event {
                    WatchEvent::Added(item) | WatchEvent::Modified(item) => event_value(item)
                        .map(|(key, value)| FeedEvent::Upsert(key, value))
                        .unwrap_or(FeedEvent::Bookmark(String::new())),
                    WatchEvent::Deleted(item) => FeedEvent::Delete(object_key(&item.metadata)),
                    WatchEvent::Bookmark(bookmark) => {
                        FeedEvent::Bookmark(bookmark.metadata.resource_version)
                    }
                    WatchEvent::Error(status) => status_event(&status),
                })
            })
            .boxed()),
        KeyedKind::Manifests(request) => Ok(dynamic_api_for_request(client, request)
            .watch(&params, version)
            .await?
            .map(|event| {
                event.map(|event| match event {
                    WatchEvent::Added(item) | WatchEvent::Modified(item) => manifest_value(item)
                        .map(|(key, value)| FeedEvent::Upsert(key, value))
                        .unwrap_or(FeedEvent::Bookmark(String::new())),
                    WatchEvent::Deleted(item) => FeedEvent::Delete(object_key(&item.metadata)),
                    WatchEvent::Bookmark(bookmark) => {
                        FeedEvent::Bookmark(bookmark.metadata.resource_version)
                    }
                    WatchEvent::Error(status) => status_event(&status),
                })
            })
            .boxed()),
    }
}

fn backoff_with_jitter(delay: u64) -> Duration {
    let jitter = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_millis() as u64
        % 500;
    Duration::from_millis(delay * 1000 + jitter)
}

enum Ending {
    Resume,
    Resnapshot,
    Forbidden(String),
}

async fn run_keyed_feed(
    publisher: &mut FeedPublisher,
    request: &LiveFeedRequest,
    kind: KeyedKind,
    stop: &mut oneshot::Receiver<()>,
) {
    let mut version: Option<String> = None;
    let mut delay = 1_u64;
    loop {
        let session = async {
            let client = tokio::time::timeout(
                Duration::from_secs(35),
                client_for(request.kubeconfig_path.clone(), request.context.clone()),
            )
            .await
            .map_err(|_| "Feed connection timed out".to_string())??;
            if version.is_none() {
                let (items, cursor) = tokio::time::timeout(
                    Duration::from_secs(60),
                    list_keyed(client.clone(), &kind),
                )
                .await
                .map_err(|_| "Feed snapshot timed out".to_string())?
                .map_err(|error| {
                    if is_forbidden(&error) {
                        format!("forbidden:{error}")
                    } else {
                        error.to_string()
                    }
                })?;
                let items = items
                    .into_iter()
                    .map(|(key, item)| serde_json::json!({ "key": key, "item": item }))
                    .collect::<Vec<_>>();
                publisher.emit("reset", serde_json::json!({ "items": items }));
                version = Some(cursor);
            }
            let mut stream =
                match watch_keyed(client, &kind, version.as_deref().unwrap_or_default()).await {
                    Ok(stream) => stream,
                    Err(error) if is_expired(&error) => return Ok(Ending::Resnapshot),
                    Err(error) if is_forbidden(&error) => {
                        return Ok(Ending::Forbidden(error.to_string()))
                    }
                    Err(error) => return Err(error.to_string()),
                };
            publisher.emit("connected", serde_json::json!({}));
            let mut pending: Vec<Value> = Vec::new();
            let mut flush = tokio::time::interval(BATCH_INTERVAL);
            flush.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            let deadline = tokio::time::sleep(Duration::from_secs(320));
            tokio::pin!(deadline);
            loop {
                tokio::select! {
                    _ = &mut deadline => return Err("Feed transport timed out".to_string()),
                    _ = resume_signal().notified() => {
                        if !pending.is_empty() { publisher.emit("delta", serde_json::json!({ "changes": std::mem::take(&mut pending) })); }
                        return Ok(Ending::Resume);
                    }
                    _ = flush.tick() => {
                        if !pending.is_empty() { publisher.emit("delta", serde_json::json!({ "changes": std::mem::take(&mut pending) })); }
                    }
                    event = stream.next() => match event {
                        Some(Ok(FeedEvent::Upsert(key, item))) => { delay = 1; pending.push(serde_json::json!({ "action": "upsert", "key": key, "item": item })); }
                        Some(Ok(FeedEvent::Delete(key))) => { delay = 1; pending.push(serde_json::json!({ "action": "delete", "key": key })); }
                        Some(Ok(FeedEvent::Bookmark(cursor))) => { if !cursor.is_empty() { version = Some(cursor); } }
                        Some(Ok(FeedEvent::Expired)) => return Ok(Ending::Resnapshot),
                        Some(Ok(FeedEvent::Forbidden(error))) => return Ok(Ending::Forbidden(error)),
                        Some(Ok(FeedEvent::Failed(error))) => return Err(error),
                        Some(Err(error)) if is_expired(&error) => return Ok(Ending::Resnapshot),
                        Some(Err(error)) => return Err(error.to_string()),
                        None => {
                            if !pending.is_empty() { publisher.emit("delta", serde_json::json!({ "changes": std::mem::take(&mut pending) })); }
                            return Ok(Ending::Resume);
                        }
                    }
                }
            }
        };
        let ending = tokio::select! {
            _ = &mut *stop => return,
            ending = session => ending,
        };
        match ending {
            // Events and manifests carry their own resourceVersion; a fresh snapshot keeps
            // the cursor exact after any interruption.
            Ok(Ending::Resume) => {
                delay = 1;
                version = None;
            }
            Ok(Ending::Resnapshot) => version = None,
            Ok(Ending::Forbidden(error)) => {
                publisher.emit("forbidden", serde_json::json!({ "error": error }));
                return;
            }
            Err(error) => {
                if let Some(error) = error.strip_prefix("forbidden:") {
                    publisher.emit("forbidden", serde_json::json!({ "error": error }));
                    return;
                }
                publisher.emit("reconnecting", serde_json::json!({ "error": error }));
                let _ = invalidate_cluster_client(
                    request.kubeconfig_path.clone(),
                    request.context.clone(),
                );
                tokio::select! {
                    _ = &mut *stop => return,
                    _ = tokio::time::sleep(backoff_with_jitter(delay)) => {}
                }
                delay = (delay * 2).min(30);
                version = None;
            }
        }
    }
}

/// The Overview without its timestamp, for change detection.
fn overview_fingerprint(overview: &ClusterOverview) -> String {
    let mut value = serde_json::to_value(overview).unwrap_or_default();
    value["observedAt"] = Value::Null;
    value.to_string()
}

async fn run_overview_feed(
    publisher: &mut FeedPublisher,
    request: &LiveFeedRequest,
    stop: &mut oneshot::Receiver<()>,
) {
    let mut delay = 1_u64;
    loop {
        let session = async {
            let client = tokio::time::timeout(
                Duration::from_secs(35),
                client_for(request.kubeconfig_path.clone(), request.context.clone()),
            )
            .await
            .map_err(|_| "Feed connection timed out".to_string())??;
            let nodes_api = Api::<Node>::all(client.clone());
            let list_params = ListParams::default();
            let (metrics, listed) = tokio::join!(
                read_node_metric_samples(client.clone()),
                nodes_api.list(&list_params)
            );
            let listed = listed.map_err(|error| {
                if is_forbidden(&error) {
                    format!("forbidden:{error}")
                } else {
                    error.to_string()
                }
            })?;
            let version = listed.metadata.resource_version.clone().unwrap_or_default();
            let mut nodes: BTreeMap<String, Node> = listed
                .items
                .into_iter()
                .map(|node| (node.metadata.name.clone().unwrap_or_default(), node))
                .collect();
            let mut metrics = metrics;
            let mut last = String::new();
            let mut publish = |nodes: &BTreeMap<String, Node>,
                               metrics: &Option<NodeMetricSamples>,
                               publisher: &mut FeedPublisher| {
                let overview =
                    overview_with_samples(nodes.values().cloned().collect(), metrics.as_ref());
                let fingerprint = overview_fingerprint(&overview);
                if fingerprint != last {
                    last = fingerprint;
                    publisher.emit("overview", serde_json::json!({ "overview": overview }));
                }
            };
            publish(&nodes, &metrics, publisher);
            let mut stream = nodes_api
                .watch(&WatchParams::default().timeout(290), &version)
                .await
                .map_err(|error| error.to_string())?
                .boxed();
            publisher.emit("connected", serde_json::json!({}));
            let mut dirty = false;
            let mut flush = tokio::time::interval(BATCH_INTERVAL);
            flush.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            let mut metrics_tick = tokio::time::interval(METRICS_INTERVAL);
            metrics_tick.tick().await;
            let deadline = tokio::time::sleep(Duration::from_secs(320));
            tokio::pin!(deadline);
            loop {
                tokio::select! {
                    _ = &mut deadline => return Ok::<(), String>(()),
                    _ = resume_signal().notified() => return Ok(()),
                    _ = flush.tick() => if dirty { dirty = false; publish(&nodes, &metrics, publisher); },
                    _ = metrics_tick.tick() => {
                        // Only a new sample from metrics-server changes anything; repeated reads are free.
                        if let Ok(Some(next)) = tokio::time::timeout(Duration::from_secs(10), read_node_metric_samples(client.clone())).await {
                            if metrics.as_ref().map(|current| current.fingerprint.as_str()) != Some(next.fingerprint.as_str()) {
                                metrics = Some(next);
                                dirty = true;
                            }
                        }
                    }
                    event = stream.next() => match event {
                        Some(Ok(WatchEvent::Added(node) | WatchEvent::Modified(node))) => {
                            delay = 1;
                            nodes.insert(node.metadata.name.clone().unwrap_or_default(), node);
                            dirty = true;
                        }
                        Some(Ok(WatchEvent::Deleted(node))) => {
                            nodes.remove(node.metadata.name.as_deref().unwrap_or_default());
                            dirty = true;
                        }
                        Some(Ok(WatchEvent::Bookmark(_))) => {}
                        Some(Ok(WatchEvent::Error(status))) if status.code == 403 => return Err(format!("forbidden:{status}")),
                        Some(Ok(WatchEvent::Error(status))) if status.code == 410 => return Ok(()),
                        Some(Ok(WatchEvent::Error(status))) => return Err(status.to_string()),
                        Some(Err(error)) if is_expired(&error) => return Ok(()),
                        Some(Err(error)) => return Err(error.to_string()),
                        None => return Ok(()),
                    }
                }
            }
        };
        let result = tokio::select! {
            _ = &mut *stop => return,
            result = session => result,
        };
        if let Err(error) = result {
            if let Some(error) = error.strip_prefix("forbidden:") {
                publisher.emit("forbidden", serde_json::json!({ "error": error }));
                return;
            }
            publisher.emit("reconnecting", serde_json::json!({ "error": error }));
            let _ =
                invalidate_cluster_client(request.kubeconfig_path.clone(), request.context.clone());
            tokio::select! {
                _ = &mut *stop => return,
                _ = tokio::time::sleep(backoff_with_jitter(delay)) => {}
            }
            delay = (delay * 2).min(30);
        }
    }
}

pub(super) async fn run_live_feed(
    app: tauri::AppHandle,
    feed_id: String,
    request: LiveFeedRequest,
    mut stop: oneshot::Receiver<()>,
) {
    let mut publisher = FeedPublisher {
        app,
        id: feed_id,
        sequence: 0,
    };
    match request.kind.as_str() {
        "overview" => run_overview_feed(&mut publisher, &request, &mut stop).await,
        "events" => run_keyed_feed(&mut publisher, &request, KeyedKind::Events, &mut stop).await,
        "manifests" => match request.resource.clone() {
            Some(resource) if !(resource.group.is_empty() && resource.plural == "secrets") => {
                run_keyed_feed(
                    &mut publisher,
                    &request,
                    KeyedKind::Manifests(resource),
                    &mut stop,
                )
                .await
            }
            _ => publisher.emit(
                "forbidden",
                serde_json::json!({ "error": "This resource cannot be streamed" }),
            ),
        },
        other => publisher.emit(
            "forbidden",
            serde_json::json!({ "error": format!("Unknown live feed: {other}") }),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overview_changes_ignore_the_timestamp() {
        let node: Node = serde_json::from_value(serde_json::json!({
            "metadata": { "name": "worker-1" },
            "status": { "conditions": [{ "type": "Ready", "status": "True" }], "capacity": { "cpu": "4", "memory": "16Gi" } }
        }))
        .unwrap();
        let first = build_cluster_overview(vec![node.clone()], None);
        let mut second = build_cluster_overview(vec![node.clone()], None);
        second.observed_at = "later".into();
        assert_eq!(overview_fingerprint(&first), overview_fingerprint(&second));
        let mut metrics = NodeMetricMap::new();
        metrics.insert("worker-1".into(), (Some("500m".into()), Some("2Gi".into())));
        let with_metrics = build_cluster_overview(vec![node], Some(metrics));
        assert_ne!(
            overview_fingerprint(&first),
            overview_fingerprint(&with_metrics)
        );
    }

    #[test]
    fn metric_samples_change_only_when_metrics_server_resamples() {
        let sample = |timestamp: &str, cpu: &str| -> DynamicObject {
            serde_json::from_value(serde_json::json!({
                "apiVersion": "metrics.k8s.io/v1beta1", "kind": "NodeMetrics",
                "metadata": { "name": "worker-1" },
                "timestamp": timestamp, "window": "15.012s",
                "usage": { "cpu": cpu, "memory": "2Gi" }
            }))
            .unwrap()
        };
        let first = node_metric_samples(vec![sample("2026-10-02T10:00:00Z", "500m")]);
        let repeat = node_metric_samples(vec![sample("2026-10-02T10:00:00Z", "500m")]);
        let next = node_metric_samples(vec![sample("2026-10-02T10:00:15Z", "650m")]);
        assert_eq!(first.fingerprint, repeat.fingerprint);
        assert_ne!(first.fingerprint, next.fingerprint);
        assert_eq!(next.sampled_at.as_deref(), Some("2026-10-02T10:00:15Z"));
        assert_eq!(first.window_seconds, Some(15.012));
        assert_eq!(next.usage["worker-1"].0.as_deref(), Some("650m"));
    }

    #[test]
    fn feed_rows_are_keyed_by_uid_and_drop_managed_fields() {
        let event: Event = serde_json::from_value(serde_json::json!({
            "metadata": { "name": "api.17f", "namespace": "shop", "uid": "e-1" },
            "involvedObject": { "kind": "Pod", "name": "api" },
            "reason": "BackOff", "type": "Warning"
        }))
        .unwrap();
        let (key, value) = event_value(event).unwrap();
        assert_eq!(key, "e-1");
        assert_eq!(value["reason"], "BackOff");

        let app: DynamicObject = serde_json::from_value(serde_json::json!({
            "apiVersion": "argoproj.io/v1alpha1", "kind": "Application",
            "metadata": { "name": "shop", "namespace": "argocd", "managedFields": [{ "manager": "argocd" }] },
            "status": { "sync": { "status": "Synced" } }
        }))
        .unwrap();
        let (key, value) = manifest_value(app).unwrap();
        assert_eq!(
            key, "argocd/shop",
            "falls back to namespace/name without a uid"
        );
        assert!(value["metadata"].get("managedFields").is_none());
        assert_eq!(value["status"]["sync"]["status"], "Synced");
    }
}
