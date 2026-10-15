//! Live pod logs, like `kubectl logs -f`: one follow stream per open log view. Lines are
//! batched every 100 ms and sent as `kuberniva://log-stream` events. A dropped stream
//! reconnects from the last line's timestamp and skips lines it already sent, so the
//! view never repeats or loses lines.

use super::*;
use crate::live_resources::resume_signal;
use futures_util::{AsyncBufReadExt, StreamExt};
use k8s_openapi::jiff::Timestamp;
use std::time::Duration;

static LOG_STREAM_REGISTRY: OnceLock<Mutex<HashMap<String, oneshot::Sender<()>>>> = OnceLock::new();
const BATCH_INTERVAL: Duration = Duration::from_millis(100);
const BATCH_LINES: usize = 500;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LogStreamRequest {
    stream_id: String,
    kubeconfig_path: Option<String>,
    context: Option<String>,
    namespace: String,
    pod: String,
    container: Option<String>,
    /// RFC 3339 timestamp of the last line already shown; the stream continues after it.
    since_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LogStreamEvent {
    stream_id: String,
    lines: Vec<String>,
    /// "live", "reconnecting", or "ended" (the container stopped and will not restart).
    status: &'static str,
    message: Option<String>,
}

fn registry() -> &'static Mutex<HashMap<String, oneshot::Sender<()>>> {
    LOG_STREAM_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The timestamp prefix Kubernetes adds with `timestamps=true`.
fn line_timestamp(line: &str) -> Option<&str> {
    let stamp = line.split_once(' ').map_or(line, |(stamp, _)| stamp);
    (stamp.len() >= 20 && stamp.as_bytes().get(4) == Some(&b'-') && stamp.contains('T'))
        .then_some(stamp)
}

/// Drops lines at or before the resume point. Kubernetes' `sinceTime` has one-second
/// precision, so a reconnect replays the last second; lines in that second that were already
/// sent are recognised by their exact text.
#[derive(Default)]
pub(super) struct Dedupe {
    last: Option<String>,
    at_last: HashSet<String>,
}

impl Dedupe {
    pub(super) fn starting_after(timestamp: Option<String>) -> Self {
        Self {
            last: timestamp,
            at_last: HashSet::new(),
        }
    }

    pub(super) fn accept(&mut self, line: &str) -> bool {
        let Some(stamp) = line_timestamp(line) else {
            return true;
        };
        if let Some(last) = &self.last {
            // RFC 3339 timestamps from one kubelet compare correctly as text.
            match stamp.cmp(last.as_str()) {
                std::cmp::Ordering::Less => return false,
                std::cmp::Ordering::Equal => {
                    // With no text recorded yet, this is the resume point the view already has.
                    let first = self.at_last.is_empty();
                    return self.at_last.insert(line.to_string()) && !first;
                }
                std::cmp::Ordering::Greater => {}
            }
        }
        self.last = Some(stamp.to_string());
        self.at_last.clear();
        self.at_last.insert(line.to_string());
        true
    }

    fn resume_from(&self) -> Option<Timestamp> {
        self.last.as_deref()?.parse().ok()
    }
}

/// True when the container has stopped for good (completed Pod, or deleted).
async fn container_finished(pods: &Api<Pod>, pod: &str, container: Option<&str>) -> bool {
    match pods.get_opt(pod).await {
        Ok(None) => true,
        Ok(Some(pod)) => {
            let status = pod.status.unwrap_or_default();
            if matches!(status.phase.as_deref(), Some("Succeeded" | "Failed")) {
                return true;
            }
            let policy = pod
                .spec
                .and_then(|spec| spec.restart_policy)
                .unwrap_or_default();
            let terminated = status
                .container_statuses
                .unwrap_or_default()
                .into_iter()
                .find(|item| container.is_none_or(|name| item.name == name))
                .and_then(|item| item.state?.terminated)
                .is_some();
            terminated && policy == "Never"
        }
        Err(_) => false,
    }
}

async fn follow<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    request: LogStreamRequest,
    mut stop: oneshot::Receiver<()>,
) {
    let emit = |lines: Vec<String>, status: &'static str, message: Option<String>| {
        let _ = app.emit(
            "kuberniva://log-stream",
            LogStreamEvent {
                stream_id: request.stream_id.clone(),
                lines,
                status,
                message,
            },
        );
    };
    let mut dedupe = Dedupe::starting_after(request.since_time.clone());
    let mut delay = 1_u64;
    loop {
        let attempt = async {
            let client =
                client_for(request.kubeconfig_path.clone(), request.context.clone()).await?;
            let pods = Api::<Pod>::namespaced(client, &request.namespace);
            let since_time = dedupe.resume_from();
            let params = LogParams {
                container: request.container.clone(),
                follow: true,
                timestamps: true,
                since_time,
                // A brand-new stream with no history starts at the end, like `kubectl logs -f --tail=0`.
                tail_lines: since_time.is_none().then_some(0),
                ..Default::default()
            };
            let reader = tokio::time::timeout(
                Duration::from_secs(35),
                pods.log_stream(&request.pod, &params),
            )
            .await
            .map_err(|_| "Opening the log stream timed out".to_string())?
            .map_err(|error| error.to_string())?;
            emit(Vec::new(), "live", None);
            let mut lines = Box::pin(reader.lines());
            let mut pending: Vec<String> = Vec::new();
            let mut flush = tokio::time::interval(BATCH_INTERVAL);
            flush.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    // After sleep or a long time in the background the socket may be dead without an error.
                    _ = resume_signal().notified() => break,
                    _ = flush.tick() => {
                        if !pending.is_empty() { emit(std::mem::take(&mut pending), "live", None); }
                    }
                    line = lines.next() => match line {
                        Some(Ok(line)) => {
                            delay = 1;
                            if dedupe.accept(&line) { pending.push(line); }
                            if pending.len() >= BATCH_LINES { emit(std::mem::take(&mut pending), "live", None); }
                        }
                        Some(Err(error)) => {
                            if !pending.is_empty() { emit(std::mem::take(&mut pending), "live", None); }
                            return Err(error.to_string());
                        }
                        None => break,
                    }
                }
            }
            if !pending.is_empty() {
                emit(pending, "live", None);
            }
            Ok::<_, String>(pods)
        };
        let result = tokio::select! {
            _ = &mut stop => return,
            result = attempt => result,
        };
        match result {
            Ok(pods) => {
                if container_finished(&pods, &request.pod, request.container.as_deref()).await {
                    emit(
                        Vec::new(),
                        "ended",
                        Some("The container has stopped; no new lines will arrive".into()),
                    );
                    return;
                }
                // The server closes follow streams now and then; reopen quietly from the last line.
                tokio::select! {
                    _ = &mut stop => return,
                    _ = tokio::time::sleep(Duration::from_millis(500)) => {}
                }
            }
            Err(error) => {
                emit(Vec::new(), "reconnecting", Some(error));
                let _ = invalidate_cluster_client(
                    request.kubeconfig_path.clone(),
                    request.context.clone(),
                );
                tokio::select! {
                    _ = &mut stop => return,
                    _ = tokio::time::sleep(Duration::from_secs(delay)) => {}
                }
                delay = (delay * 2).min(30);
            }
        }
    }
}

// Async so Tauri runs it on its runtime, not the main thread; the task itself is started
// with Tauri's runtime handle, which works from any thread.
#[tauri::command]
pub(super) async fn start_log_stream<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    request: LogStreamRequest,
) -> Result<(), String> {
    if request.stream_id.is_empty() || request.stream_id.len() > 128 {
        return Err("Invalid log stream identifier".into());
    }
    let (stop_tx, stop_rx) = oneshot::channel();
    {
        let mut streams = registry()
            .lock()
            .map_err(|_| "The log stream registry is unavailable".to_string())?;
        if let Some(previous) = streams.remove(&request.stream_id) {
            let _ = previous.send(());
        }
        if streams.len() >= 8 {
            return Err("Too many open log streams".into());
        }
        streams.insert(request.stream_id.clone(), stop_tx);
    }
    let id = request.stream_id.clone();
    tauri::async_runtime::spawn(async move {
        follow(app, request, stop_rx).await;
        if let Ok(mut streams) = registry().lock() {
            if streams.get(&id).is_some_and(|sender| sender.is_closed()) {
                streams.remove(&id);
            }
        }
    });
    Ok(())
}

#[tauri::command]
pub(super) fn stop_log_stream(stream_id: String) -> Result<(), String> {
    if let Some(stop) = registry()
        .lock()
        .map_err(|_| "The log stream registry is unavailable".to_string())?
        .remove(&stream_id)
    {
        let _ = stop.send(());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Opening logs must never take the app down: the stream starts on Tauri's runtime and an
    /// unreachable cluster is reported as a reconnecting stream, not a crash.
    #[test]
    fn starting_a_stream_outside_a_runtime_reports_errors_instead_of_crashing() {
        use tauri::Listener;
        let app = tauri::test::mock_app();
        let handle = app.handle().clone();
        let (sender, receiver) = std::sync::mpsc::channel::<String>();
        handle.listen_any("kuberniva://log-stream", move |event| {
            let _ = sender.send(event.payload().to_string());
        });
        let request = LogStreamRequest {
            stream_id: "test-stream".into(),
            kubeconfig_path: Some("/nonexistent/kubeconfig".into()),
            context: Some("missing".into()),
            namespace: "default".into(),
            pod: "api".into(),
            container: None,
            since_time: None,
        };
        tauri::async_runtime::block_on(start_log_stream(handle.clone(), request)).unwrap();
        let event = receiver
            .recv_timeout(Duration::from_secs(20))
            .expect("the stream reports its state");
        assert!(event.contains("\"status\":\"reconnecting\""), "{event}");
        stop_log_stream("test-stream".into()).unwrap();
    }

    #[test]
    fn reconnects_skip_lines_already_shown_but_keep_new_ones_in_the_same_second() {
        let mut dedupe = Dedupe::starting_after(Some("2026-10-15T10:00:01.000000000Z".into()));
        // Replayed history before and at the resume point is dropped.
        assert!(!dedupe.accept("2026-10-15T10:00:00.900000000Z old"));
        assert!(!dedupe.accept("2026-10-15T10:00:01.000000000Z last shown"));
        // A later line arrives once.
        assert!(dedupe.accept("2026-10-15T10:00:01.500000000Z new"));
        assert!(!dedupe.accept("2026-10-15T10:00:01.500000000Z new"));
        assert!(dedupe.accept("2026-10-15T10:00:01.500000000Z another at the same instant"));
        assert!(dedupe.accept("2026-10-15T10:00:02.000000000Z next"));
        assert!(dedupe.accept("continuation line without a timestamp"));
        assert_eq!(
            dedupe.resume_from().unwrap().to_string(),
            "2026-10-15T10:00:02Z"
        );
    }
}
