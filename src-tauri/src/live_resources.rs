use super::*;
use futures_util::stream::BoxStream;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ListMeta, ObjectMeta};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const PAGE_SIZE: u32 = 500;
const BATCH_SIZE: usize = 256;
const BATCH_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ResourceSnapshot {
    items: Vec<ResourceObject>,
    resource_version: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ResourceWatchRequest {
    #[serde(flatten)]
    pub resource: ResourceRequest,
    pub resource_version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Change {
    action: &'static str,
    object: ResourceObject,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Signal {
    watch_id: String,
    sequence: u64,
    action: &'static str,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    changes: Vec<Change>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    items: Vec<ResourceObject>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resource_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

fn metadata_row(metadata: ObjectMeta) -> ResourceObject {
    ResourceObject {
        name: metadata.name.unwrap_or_default(),
        namespace: metadata.namespace,
        uid: metadata.uid,
        resource_version: metadata.resource_version,
        created_at: metadata.creation_timestamp.map(|time| time.0.to_string()),
        status: None,
        ready_containers: None,
        total_containers: None,
        restarts: None,
        cpu_usage: None,
        memory_usage: None,
        node_name: None,
    }
}

fn pod_api(client: Client, request: &ResourceRequest) -> Api<Pod> {
    match request
        .namespace
        .as_deref()
        .filter(|scope| !scope.is_empty() && *scope != "all namespaces")
    {
        Some(namespace) => Api::namespaced(client, namespace),
        None => Api::all(client),
    }
}

fn snapshot_version(current: Option<&str>, metadata: &ListMeta) -> Result<String, String> {
    let next = metadata
        .resource_version
        .as_deref()
        .filter(|version| !version.is_empty())
        .ok_or_else(|| "The API returned a snapshot without a resourceVersion".to_string())?;
    if current.is_some_and(|version| version != next) {
        return Err("The API changed resourceVersion between snapshot pages".to_string());
    }
    Ok(next.to_string())
}

async fn snapshot_with_client(
    client: Client,
    request: &ResourceRequest,
) -> Result<ResourceSnapshot, String> {
    let mut items = Vec::new();
    let mut continuation = String::new();
    let mut version = None;
    loop {
        let params = ListParams::default()
            .limit(PAGE_SIZE)
            .continue_token(&continuation);
        let (page, metadata) = if request.kind == "Pod" && request.group.is_empty() {
            let response = pod_api(client.clone(), request)
                .list(&params)
                .await
                .map_err(|error| error.to_string())?;
            (
                response
                    .items
                    .into_iter()
                    .map(|pod| pod_resource_object(pod, None))
                    .collect::<Vec<_>>(),
                response.metadata,
            )
        } else {
            let response = dynamic_api_for_request(client.clone(), request)
                .list_metadata(&params)
                .await
                .map_err(|error| error.to_string())?;
            (
                response
                    .items
                    .into_iter()
                    .map(|object| metadata_row(object.metadata))
                    .collect::<Vec<_>>(),
                response.metadata,
            )
        };
        version = Some(snapshot_version(version.as_deref(), &metadata)?);
        items.extend(page);
        let next = metadata.continue_.unwrap_or_default();
        if next.is_empty() {
            break;
        }
        if next == continuation {
            return Err("The API repeated a pagination token".to_string());
        }
        continuation = next;
    }
    items.sort_by(|left, right| {
        left.namespace
            .cmp(&right.namespace)
            .then(left.name.cmp(&right.name))
    });
    Ok(ResourceSnapshot {
        items,
        resource_version: version.unwrap_or_default(),
    })
}

pub(super) async fn snapshot(request: ResourceRequest) -> Result<ResourceSnapshot, String> {
    let client = client_for(request.kubeconfig_path.clone(), request.context.clone()).await?;
    // Metrics have a different update cadence and must not hold up the object snapshot.
    let mut snapshot = snapshot_with_client(client.clone(), &request).await?;
    if request.kind == "Pod" && request.group.is_empty() {
        if let Ok(metrics) = tokio::time::timeout(
            Duration::from_secs(4),
            pod_usage_map(client, request.namespace.as_deref()),
        )
        .await
        {
            for row in &mut snapshot.items {
                if let Some((cpu, memory)) =
                    metrics.get(&(row.namespace.clone().unwrap_or_default(), row.name.clone()))
                {
                    row.cpu_usage = (!cpu.is_empty()).then(|| cpu.clone());
                    row.memory_usage = (!memory.is_empty()).then(|| memory.clone());
                }
            }
        }
    }
    Ok(snapshot)
}

#[derive(Debug)]
enum RowEvent {
    Upsert(ResourceObject),
    Delete(ResourceObject),
    Bookmark(String),
    Expired,
    Forbidden(String),
    Failed(String),
}

fn project_event<T>(event: WatchEvent<T>, project: impl FnOnce(T) -> ResourceObject) -> RowEvent {
    match event {
        WatchEvent::Added(object) | WatchEvent::Modified(object) => {
            RowEvent::Upsert(project(object))
        }
        WatchEvent::Deleted(object) => RowEvent::Delete(project(object)),
        WatchEvent::Bookmark(bookmark) => RowEvent::Bookmark(bookmark.metadata.resource_version),
        WatchEvent::Error(status) if status.code == 410 => RowEvent::Expired,
        WatchEvent::Error(status) if status.code == 403 => RowEvent::Forbidden(status.to_string()),
        WatchEvent::Error(status) => RowEvent::Failed(status.to_string()),
    }
}

async fn watch_stream(
    client: Client,
    request: &ResourceRequest,
    version: &str,
) -> Result<BoxStream<'static, Result<RowEvent, kube::Error>>, kube::Error> {
    let params = WatchParams::default().timeout(290);
    if request.kind == "Pod" && request.group.is_empty() {
        Ok(pod_api(client, request)
            .watch(&params, version)
            .await?
            .map(|event| {
                event.map(|event| project_event(event, |pod| pod_resource_object(pod, None)))
            })
            .boxed())
    } else {
        Ok(dynamic_api_for_request(client, request)
            .watch_metadata(&params, version)
            .await?
            .map(|event| {
                event.map(|event| project_event(event, |object| metadata_row(object.metadata)))
            })
            .boxed())
    }
}

struct Publisher {
    app: tauri::AppHandle,
    id: String,
    sequence: u64,
    pending: Vec<Change>,
}

impl Publisher {
    fn emit(
        &mut self,
        action: &'static str,
        items: Vec<ResourceObject>,
        version: Option<String>,
        error: Option<String>,
    ) {
        self.sequence += 1;
        let changes = if action == "delta" {
            std::mem::take(&mut self.pending)
        } else {
            Vec::new()
        };
        let _ = self.app.emit(
            "kuberniva://resource-watch",
            Signal {
                watch_id: self.id.clone(),
                sequence: self.sequence,
                action,
                changes,
                items,
                resource_version: version,
                error,
            },
        );
    }
    fn flush(&mut self, version: &Option<String>) {
        if !self.pending.is_empty() {
            self.emit("delta", Vec::new(), version.clone(), None);
        }
    }
}

fn expired(error: &kube::Error) -> bool {
    matches!(error, kube::Error::Api(response) if response.code == 410)
}
fn forbidden(error: &kube::Error) -> bool {
    matches!(error, kube::Error::Api(response) if response.code == 403)
}

pub(super) async fn run_resource_watch(
    app: tauri::AppHandle,
    watch_id: String,
    watch: ResourceWatchRequest,
    mut stop: oneshot::Receiver<()>,
) {
    let request = watch.resource;
    let mut version = watch
        .resource_version
        .filter(|version| !version.is_empty() && version != "0");
    let mut publisher = Publisher {
        app,
        id: watch_id,
        sequence: 0,
        pending: Vec::with_capacity(BATCH_SIZE),
    };
    let mut delay = 1_u64;
    loop {
        let client = tokio::select! {
            _ = &mut stop => return,
            result = tokio::time::timeout(Duration::from_secs(35), client_for(request.kubeconfig_path.clone(), request.context.clone())) => result,
        };
        let connection = async {
            let client = client.map_err(|_| "Watch connection timed out".to_string())??;
            if version.is_none() {
                publisher.emit("resetBegin", Vec::new(), None, None);
                let snapshot = tokio::time::timeout(
                    Duration::from_secs(60),
                    snapshot_with_client(client.clone(), &request),
                )
                .await
                .map_err(|_| "Watch snapshot timed out".to_string())??;
                for chunk in snapshot.items.chunks(BATCH_SIZE) {
                    publisher.emit("resetChunk", chunk.to_vec(), None, None);
                    tokio::task::yield_now().await;
                }
                version = Some(snapshot.resource_version);
                publisher.emit("resetEnd", Vec::new(), version.clone(), None);
            }
            let opening = tokio::time::timeout(
                Duration::from_secs(35),
                watch_stream(client, &request, version.as_deref().unwrap_or_default()),
            )
            .await
            .map_err(|_| "Watch connection timed out".to_string())?;
            let mut stream = match opening {
                Ok(stream) => stream,
                Err(error) if expired(&error) => {
                    version = None;
                    return Ok(());
                }
                Err(error) if forbidden(&error) => {
                    publisher.emit(
                        "forbidden",
                        Vec::new(),
                        version.clone(),
                        Some(error.to_string()),
                    );
                    return Err("watch-forbidden".to_string());
                }
                Err(error) => return Err(error.to_string()),
            };
            publisher.emit("connected", Vec::new(), version.clone(), None);
            let opened_at = std::time::Instant::now();
            let mut flush = tokio::time::interval(BATCH_INTERVAL);
            flush.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            // The server's watch timeout may be lost on a broken transport. Rotate
            // that transport after a bounded interval, retaining the exact cursor.
            let transport_deadline = tokio::time::sleep(Duration::from_secs(320));
            tokio::pin!(transport_deadline);
            loop {
                tokio::select! {
                    _ = &mut transport_deadline => return Err("Watch transport timed out".to_string()),
                    _ = flush.tick() => publisher.flush(&version),
                    event = stream.next() => match event {
                        Some(Ok(RowEvent::Upsert(row))) => {
                            delay = 1;
                            if row.resource_version.is_some() { version = row.resource_version.clone(); }
                            publisher.pending.push(Change { action: "upsert", object: row });
                        }
                        Some(Ok(RowEvent::Delete(row))) => {
                            delay = 1;
                            if row.resource_version.is_some() { version = row.resource_version.clone(); }
                            publisher.pending.push(Change { action: "delete", object: row });
                        }
                        Some(Ok(RowEvent::Bookmark(cursor))) => {
                            delay = 1;
                            if !cursor.is_empty() { version = Some(cursor); }
                            publisher.flush(&version);
                        }
                        Some(Ok(RowEvent::Expired)) => { publisher.flush(&version); version = None; return Ok(()); }
                        Some(Ok(RowEvent::Forbidden(error))) => {
                            publisher.flush(&version);
                            publisher.emit("forbidden", Vec::new(), version.clone(), Some(error));
                            return Err("watch-forbidden".to_string());
                        }
                        Some(Ok(RowEvent::Failed(error))) => return Err(error),
                        Some(Err(error)) if expired(&error) => { publisher.flush(&version); version = None; return Ok(()); }
                        Some(Err(error)) => return Err(error.to_string()),
                        None => {
                            publisher.flush(&version);
                            if opened_at.elapsed() < Duration::from_secs(1) { return Err("Watch connection closed immediately".to_string()); }
                            delay = 1;
                            return Ok(());
                        }
                    }
                }
                if publisher.pending.len() >= BATCH_SIZE {
                    publisher.flush(&version);
                }
            }
        };
        let result: Result<(), String> = tokio::select! {
            _ = &mut stop => return,
            result = connection => result,
        };
        publisher.flush(&version);
        if let Err(error) = result {
            if error == "watch-forbidden" {
                return;
            }
            publisher.emit("reconnecting", Vec::new(), version.clone(), Some(error));
            let _ =
                invalidate_cluster_client(request.kubeconfig_path.clone(), request.context.clone());
            let jitter = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_millis() as u64
                % 500;
            tokio::select! {
                _ = &mut stop => return,
                _ = tokio::time::sleep(Duration::from_millis(delay * 1000 + jitter)) => {}
            }
            delay = (delay * 2).min(30);
        }
        // Cancellation remains responsive during LIST, authentication, and streaming.
        if stop.try_recv().is_ok() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_pages_must_share_the_collection_cursor() {
        let metadata = ListMeta {
            resource_version: Some("opaque-v2".into()),
            ..Default::default()
        };
        assert_eq!(snapshot_version(None, &metadata).unwrap(), "opaque-v2");
        assert!(snapshot_version(Some("opaque-v1"), &metadata).is_err());
        assert!(snapshot_version(None, &ListMeta::default()).is_err());
    }
    #[test]
    fn deleted_pod_keeps_identity_for_safe_tombstones() {
        let pod = Pod {
            metadata: ObjectMeta {
                name: Some("api".into()),
                namespace: Some("team".into()),
                uid: Some("old-uid".into()),
                resource_version: Some("123".into()),
                ..Default::default()
            },
            ..Default::default()
        };
        match project_event(WatchEvent::Deleted(pod), |pod| {
            pod_resource_object(pod, None)
        }) {
            RowEvent::Delete(row) => {
                assert_eq!(row.uid.as_deref(), Some("old-uid"));
                assert_eq!(row.resource_version.as_deref(), Some("123"));
            }
            _ => panic!("expected deletion"),
        }
    }
}
