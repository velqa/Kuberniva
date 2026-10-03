use super::*;
use futures_util::stream::BoxStream;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{ListMeta, ObjectMeta};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const PAGE_SIZE: u32 = 500;

static RESUME_SIGNAL: std::sync::OnceLock<tokio::sync::Notify> = std::sync::OnceLock::new();

pub(super) fn resume_signal() -> &'static tokio::sync::Notify {
    RESUME_SIGNAL.get_or_init(tokio::sync::Notify::new)
}

/// Drops pooled connections and makes every live watch reopen from its cursor.
pub(super) fn resume_live_connections() {
    if let Some(cache) = KUBE_CLIENT_CACHE.get() {
        if let Ok(mut cache) = cache.lock() {
            cache.clear();
        }
    }
    resume_signal().notify_waiters();
}

/// How long the machine slept between two observations: wall-clock time advances during
/// sleep while the monotonic clock (CLOCK_UPTIME_RAW / CLOCK_MONOTONIC) does not.
pub(super) fn slept_between(
    wall_elapsed: Duration,
    monotonic_elapsed: Duration,
) -> Option<Duration> {
    wall_elapsed
        .checked_sub(monotonic_elapsed)
        .filter(|gap| *gap >= Duration::from_secs(15))
}

/// Watches for system sleep and resumes live connections on wake, so a minimized or
/// hidden window keeps current data without the UI having to reconnect.
pub(super) fn spawn_sleep_monitor(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut wall = SystemTime::now();
        let mut monotonic = std::time::Instant::now();
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            let (now_wall, now_monotonic) = (SystemTime::now(), std::time::Instant::now());
            let wall_elapsed = now_wall.duration_since(wall).unwrap_or_default();
            if let Some(slept) = slept_between(wall_elapsed, now_monotonic - monotonic) {
                resume_live_connections();
                let _ = app.emit("kuberniva://system-resumed", slept.as_millis() as u64);
            }
            wall = now_wall;
            monotonic = now_monotonic;
        }
    });
}
const BATCH_SIZE: usize = 256;
const BATCH_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ResourceSnapshot {
    items: Vec<ResourceObject>,
    resource_version: String,
    columns: Vec<TableColumn>,
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    columns: Vec<TableColumn>,
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
        cells: None,
    }
}

/// One server-printed column, as `kubectl get` shows it.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(super) struct TableColumn {
    name: String,
    #[serde(rename = "type")]
    kind: String,
    priority: i64,
}

// Ask for the server's Table rendering (the columns `kubectl get` prints, including CRD
// printer columns), falling back to plain JSON where an API does not offer tables.
const TABLE_ACCEPT: &str = "application/json;as=Table;v=v1;g=meta.k8s.io, application/json";

fn as_table(mut request: http::Request<Vec<u8>>) -> Result<http::Request<Vec<u8>>, String> {
    let uri = request.uri().to_string();
    let separator = if uri.contains('?') { '&' } else { '?' };
    *request.uri_mut() = format!("{uri}{separator}includeObject=Metadata")
        .parse()
        .map_err(|error| format!("Could not build the table request: {error}"))?;
    request.headers_mut().insert(
        http::header::ACCEPT,
        http::HeaderValue::from_static(TABLE_ACCEPT),
    );
    Ok(request)
}

fn cell_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        Value::Array(items) => items.iter().map(cell_text).collect::<Vec<_>>().join(","),
        other => other.to_string(),
    }
}

fn table_columns(table: &Value) -> Vec<TableColumn> {
    table["columnDefinitions"]
        .as_array()
        .map(|columns| {
            columns
                .iter()
                .map(|column| TableColumn {
                    name: column["name"].as_str().unwrap_or_default().to_string(),
                    kind: column["type"].as_str().unwrap_or("string").to_string(),
                    priority: column["priority"].as_i64().unwrap_or(0),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn table_rows(table: &Value) -> Vec<ResourceObject> {
    table["rows"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .map(|row| {
                    let metadata =
                        serde_json::from_value::<ObjectMeta>(row["object"]["metadata"].clone())
                            .unwrap_or_default();
                    let mut object = metadata_row(metadata);
                    object.cells = Some(
                        row["cells"]
                            .as_array()
                            .map(|cells| cells.iter().map(cell_text).collect())
                            .unwrap_or_default(),
                    );
                    object
                })
                .filter(|object| !object.name.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn unsupported_table(error: &kube::Error) -> bool {
    matches!(error, kube::Error::Api(response) if response.code == 406 || response.code == 415)
}

async fn table_page(
    client: &Client,
    request: &ResourceRequest,
    params: &ListParams,
) -> Result<Option<(Vec<ResourceObject>, ListMeta, Vec<TableColumn>)>, String> {
    let api = dynamic_api_for_request(client.clone(), request);
    let http_request = as_table(
        kube::core::Request::new(api.resource_url())
            .list(params)
            .map_err(|error| error.to_string())?,
    )?;
    match client.request::<Value>(http_request).await {
        Ok(table) if table["kind"] == "Table" => {
            let metadata =
                serde_json::from_value::<ListMeta>(table["metadata"].clone()).unwrap_or_default();
            Ok(Some((table_rows(&table), metadata, table_columns(&table))))
        }
        Ok(_) => Ok(None),
        Err(error) if unsupported_table(&error) => Ok(None),
        Err(error) => Err(error.to_string()),
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
    let mut columns = Vec::new();
    let mut use_table = !(request.kind == "Pod" && request.group.is_empty());
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
        } else if let Some((rows, metadata, page_columns)) = if use_table {
            table_page(&client, request, &params).await?
        } else {
            None
        } {
            if columns.is_empty() {
                columns = page_columns;
            }
            (rows, metadata)
        } else {
            use_table = false;
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
        columns,
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

fn project_table_event(event: WatchEvent<Value>) -> RowEvent {
    let first_row = |table: Value| table_rows(&table).into_iter().next();
    match event {
        WatchEvent::Added(table) | WatchEvent::Modified(table) => first_row(table)
            .map(RowEvent::Upsert)
            .unwrap_or(RowEvent::Bookmark(String::new())),
        WatchEvent::Deleted(table) => first_row(table)
            .map(RowEvent::Delete)
            .unwrap_or(RowEvent::Bookmark(String::new())),
        other => project_event(other, |_| metadata_row(ObjectMeta::default())),
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
        let url = dynamic_api_for_request(client.clone(), request)
            .resource_url()
            .to_string();
        let table_request = kube::core::Request::new(url)
            .watch(&params, version)
            .map_err(kube::Error::BuildRequest)
            .and_then(|request| {
                as_table(request)
                    .map_err(|error| kube::Error::Service(std::io::Error::other(error).into()))
            })?;
        match client.request_events::<Value>(table_request).await {
            Ok(stream) => {
                return Ok(stream
                    // A row the client cannot parse is skipped, never fatal to the watch.
                    .filter_map(|event| async move {
                        match event {
                            Err(kube::Error::SerdeError(_)) => None,
                            other => Some(other),
                        }
                    })
                    .map(|event| event.map(project_table_event))
                    .boxed());
            }
            Err(error) if unsupported_table(&error) => {}
            Err(error) => return Err(error),
        }
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
    columns: Vec<TableColumn>,
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
                columns: if action == "resetEnd" {
                    std::mem::take(&mut self.columns)
                } else {
                    Vec::new()
                },
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
        columns: Vec::new(),
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
                publisher.columns = snapshot.columns;
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
                    // After sleep or a network change the socket may be dead without an error;
                    // reopen now, resuming from the exact cursor (no snapshot, no UI reset).
                    _ = resume_signal().notified() => {
                        publisher.flush(&version);
                        return Ok(());
                    }
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
    fn sleep_is_the_gap_between_wall_and_monotonic_time() {
        let secs = Duration::from_secs;
        assert_eq!(slept_between(secs(3605), secs(5)), Some(secs(3600)));
        assert_eq!(
            slept_between(secs(6), secs(5)),
            None,
            "scheduling jitter is not sleep"
        );
        assert_eq!(
            slept_between(secs(5), secs(30)),
            None,
            "a wall-clock step back is not sleep"
        );
    }

    #[test]
    fn server_tables_become_rows_with_cells_and_identity() {
        let table = serde_json::json!({
            "kind": "Table",
            "metadata": { "resourceVersion": "42" },
            "columnDefinitions": [
                { "name": "Name", "type": "string", "priority": 0 },
                { "name": "Type", "type": "string", "priority": 0 },
                { "name": "Cluster-IP", "type": "string", "priority": 0 },
                { "name": "Port(s)", "type": "string", "priority": 0 },
                { "name": "Selector", "type": "string", "priority": 1 }
            ],
            "rows": [
                { "cells": ["api", "ClusterIP", "10.0.0.12", "80/TCP,443/TCP", null], "object": { "metadata": { "name": "api", "namespace": "shop", "uid": "u1", "resourceVersion": "41" } } },
                { "cells": ["bad"], "object": { "metadata": {} } }
            ]
        });
        let columns = table_columns(&table);
        assert_eq!(columns.len(), 5);
        assert_eq!(columns[4].priority, 1);
        let rows = table_rows(&table);
        assert_eq!(rows.len(), 1, "rows without a name are dropped");
        assert_eq!(rows[0].uid.as_deref(), Some("u1"));
        assert_eq!(
            rows[0].cells.as_deref(),
            Some(
                &[
                    "api".to_string(),
                    "ClusterIP".into(),
                    "10.0.0.12".into(),
                    "80/TCP,443/TCP".into(),
                    String::new()
                ][..]
            )
        );
        assert_eq!(cell_text(&serde_json::json!(["a", 2])), "a,2");
    }

    #[test]
    fn table_requests_ask_for_metadata_and_the_table_format() {
        let request = as_table(
            kube::core::Request::new("/api/v1/namespaces/shop/services")
                .list(&ListParams::default().limit(500))
                .unwrap(),
        )
        .unwrap();
        assert!(request
            .uri()
            .to_string()
            .ends_with("&includeObject=Metadata"));
        assert!(request.headers()[http::header::ACCEPT]
            .to_str()
            .unwrap()
            .contains("as=Table"));
        match project_table_event(WatchEvent::Deleted(serde_json::json!({ "rows": [] }))) {
            RowEvent::Bookmark(cursor) => assert!(cursor.is_empty()),
            _ => panic!("an empty table event must not delete anything"),
        }
    }

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
