use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::Utc;
use futures_util::StreamExt;
use k8s_openapi::api::authorization::v1::{
    ResourceAttributes, SelfSubjectAccessReview, SelfSubjectAccessReviewSpec,
};
use k8s_openapi::api::core::v1::{Event, Namespace, Node, Pod};
use kube::{
    api::{
        Api, AttachParams, DeleteParams, DynamicObject, ListParams, LogParams, PostParams,
        Preconditions, WatchEvent, WatchParams,
    },
    config::{KubeConfigOptions, Kubeconfig},
    core::ApiResource,
    discovery::{verbs, Discovery, Scope},
    Client, Config,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::{
    env, fs,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, OnceLock,
    },
};
use tauri::{Emitter, Manager};
use tokio::{
    io::{copy_bidirectional, AsyncBufReadExt, AsyncReadExt, BufReader},
    net::TcpListener,
    process::Command as TokioCommand,
    sync::{mpsc, oneshot},
};
use x509_parser::{parse_x509_certificate, pem::parse_x509_pem};

static KUBE_CLIENT_CACHE: OnceLock<Mutex<HashMap<String, Client>>> = OnceLock::new();
static PORT_FORWARD_REGISTRY: OnceLock<Mutex<HashMap<String, PortForwardRuntime>>> =
    OnceLock::new();
static RESOURCE_WATCH_REGISTRY: OnceLock<Mutex<HashMap<String, oneshot::Sender<()>>>> =
    OnceLock::new();
static KUBECONFIG_IMPORT_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
static NEXT_PORT_FORWARD_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_RESOURCE_WATCH_ID: AtomicU64 = AtomicU64::new(1);
static CLI_RUN_REGISTRY: OnceLock<Mutex<HashMap<String, oneshot::Sender<()>>>> = OnceLock::new();
const MAX_PASTED_KUBECONFIG_BYTES: usize = 5 * 1024 * 1024;
const MAX_LOG_EXPORT_BYTES: usize = 20 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct KubeContext {
    name: String,
    cluster: String,
    namespace: String,
    auth_method: String,
    current: bool,
    source_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct KubeconfigSummary {
    contexts: Vec<KubeContext>,
    current_context: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourceDescriptor {
    group: String,
    version: String,
    api_version: String,
    kind: String,
    plural: String,
    namespaced: bool,
    category: String,
    custom: bool,
    crd: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClusterCatalog {
    context: String,
    namespaces: Vec<String>,
    resources: Vec<ResourceDescriptor>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResourceRequest {
    kubeconfig_path: Option<String>,
    context: Option<String>,
    group: String,
    version: String,
    kind: String,
    plural: String,
    namespaced: bool,
    namespace: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccessReviewCheck {
    key: String,
    group: String,
    version: String,
    resource: String,
    verb: String,
    namespace: Option<String>,
    name: Option<String>,
    subresource: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccessReviewRequest {
    kubeconfig_path: Option<String>,
    context: Option<String>,
    checks: Vec<AccessReviewCheck>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AccessReviewDecision {
    key: String,
    allowed: bool,
    denied: bool,
    reason: Option<String>,
    evaluation_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourceWatchSignal {
    watch_id: String,
    action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourceObject {
    name: String,
    namespace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    uid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resource_version: Option<String>,
    created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ready_containers: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    total_containers: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    restarts: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cpu_usage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    memory_usage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    node_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResourceObjectRequest {
    kubeconfig_path: Option<String>,
    context: Option<String>,
    group: String,
    version: String,
    kind: String,
    plural: String,
    namespaced: bool,
    namespace: Option<String>,
    name: String,
    uid: Option<String>,
    resource_version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveResourceRequest {
    kubeconfig_path: Option<String>,
    context: Option<String>,
    group: String,
    version: String,
    kind: String,
    plural: String,
    namespaced: bool,
    namespace: Option<String>,
    name: String,
    manifest: Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveResourceYamlRequest {
    kubeconfig_path: Option<String>,
    context: Option<String>,
    group: String,
    version: String,
    kind: String,
    plural: String,
    namespaced: bool,
    namespace: Option<String>,
    name: String,
    yaml: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CertificateInfo {
    expires_at: String,
    days_remaining: i64,
    expired: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourceDetail {
    manifest: Value,
    yaml: String,
    certificate: Option<CertificateInfo>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkloadPodRequest {
    kubeconfig_path: Option<String>,
    context: Option<String>,
    group: String,
    version: String,
    kind: String,
    plural: String,
    namespace: String,
    name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PodLogRequest {
    kubeconfig_path: Option<String>,
    context: Option<String>,
    namespace: String,
    pod: String,
    container: Option<String>,
    since_time: Option<String>,
    tail_lines: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PodLogResponse {
    lines: Vec<String>,
    containers: Vec<String>,
    selected_container: Option<String>,
    ports: Vec<PodPort>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PodPort {
    container: String,
    name: Option<String>,
    port: i32,
    protocol: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PodRuntimeRequest {
    kubeconfig_path: Option<String>,
    context: Option<String>,
    namespace: String,
    pod: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PodRuntimeInfo {
    containers: Vec<String>,
    ports: Vec<PodPort>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PodExecRequest {
    kubeconfig_path: Option<String>,
    context: Option<String>,
    namespace: String,
    pod: String,
    container: Option<String>,
    command: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PodExecResponse {
    stdout: String,
    stderr: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CliChunk {
    stream: &'static str,
    text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CliOutputEvent {
    run_id: String,
    chunks: Vec<CliChunk>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CliExitEvent {
    run_id: String,
    exit_code: Option<i32>,
    success: bool,
    cancelled: bool,
    error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct KubeCliRequest {
    kubeconfig_path: Option<String>,
    context: String,
    namespace: Option<String>,
    command: String,
    #[serde(default)]
    shell: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct KubeCliResponse {
    stdout: String,
    stderr: String,
    exit_code: Option<i32>,
    success: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveLogFileRequest {
    path: String,
    content: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartPortForwardRequest {
    kubeconfig_path: Option<String>,
    cluster_id: Option<String>,
    context: Option<String>,
    namespace: String,
    pod: String,
    remote_port: u16,
    local_port: Option<u16>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StopPortForwardRequest {
    id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PortForwardInfo {
    id: String,
    cluster_id: Option<String>,
    context: Option<String>,
    local_address: String,
    local_port: u16,
    remote_port: u16,
    namespace: String,
    pod: String,
}

struct PortForwardRuntime {
    info: PortForwardInfo,
    shutdown: oneshot::Sender<()>,
    stopped: oneshot::Receiver<()>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct NodeOverview {
    name: String,
    ready: bool,
    roles: Vec<String>,
    labels: Vec<NodeProperty>,
    annotations: Vec<NodeProperty>,
    addresses: Vec<NodeAddressOverview>,
    conditions: Vec<NodeConditionOverview>,
    taints: Vec<NodeTaintOverview>,
    architecture: Option<String>,
    operating_system: Option<String>,
    os_image: Option<String>,
    kernel_version: Option<String>,
    kubelet_version: Option<String>,
    container_runtime_version: Option<String>,
    pod_cidrs: Vec<String>,
    provider_id: Option<String>,
    unschedulable: bool,
    uid: Option<String>,
    creation_timestamp: Option<String>,
    capacity: Vec<NodeProperty>,
    allocatable: Vec<NodeProperty>,
    cpu_capacity: Option<String>,
    memory_capacity: Option<String>,
    cpu_usage: Option<String>,
    memory_usage: Option<String>,
    cpu_usage_percent: Option<f64>,
    memory_usage_percent: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClusterTotals {
    cpu_capacity: Option<String>,
    memory_capacity: Option<String>,
    storage_capacity: Option<String>,
    cpu_usage: Option<String>,
    memory_usage: Option<String>,
    cpu_usage_percent: Option<f64>,
    memory_usage_percent: Option<f64>,
    metric_nodes: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct NodeProperty {
    key: String,
    value: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct NodeAddressOverview {
    type_: String,
    address: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct NodeConditionOverview {
    type_: String,
    status: String,
    reason: Option<String>,
    message: Option<String>,
    last_heartbeat_time: Option<String>,
    last_transition_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct NodeTaintOverview {
    key: String,
    value: Option<String>,
    effect: String,
    time_added: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClusterOverview {
    nodes: Vec<NodeOverview>,
    totals: ClusterTotals,
    metrics_available: bool,
    observed_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClusterEvent {
    name: String,
    namespace: Option<String>,
    event_type: String,
    reason: Option<String>,
    message: Option<String>,
    involved_kind: Option<String>,
    involved_name: Option<String>,
    action: Option<String>,
    count: Option<i32>,
    source: Option<String>,
    first_observed: Option<String>,
    last_observed: Option<String>,
}

fn node_time_string(
    time: Option<&k8s_openapi::apimachinery::pkg::apis::meta::v1::Time>,
) -> Option<String> {
    time.map(|value| value.0.to_string())
}

fn days_remaining_from_seconds(seconds: i64) -> i64 {
    if seconds >= 0 {
        (seconds.saturating_add(86_399)) / 86_400
    } else {
        -((-seconds).saturating_add(86_399) / 86_400)
    }
}

fn resolve_local_path(input: &str) -> PathBuf {
    let path = input.trim();
    let expanded = if path == "~" {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(path))
    } else if let Some(relative_path) = path.strip_prefix("~/") {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default()
            .join(relative_path)
    } else {
        PathBuf::from(path)
    };
    if expanded.is_relative() {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
            .join(expanded)
    } else {
        expanded
    }
}

fn load_kubeconfig(path: Option<&str>) -> Result<Kubeconfig, String> {
    match path {
        Some(path) if !path.trim().is_empty() => {
            let resolved_path = resolve_local_path(path);
            Kubeconfig::read_from(&resolved_path)
                .map_err(|error| format!("Could not read {}: {error}", resolved_path.display()))
        }
        _ => Kubeconfig::read().map_err(|error| error.to_string()),
    }
}

fn auth_method_for(kubeconfig: &Kubeconfig, user: Option<&str>) -> String {
    let auth_info = user.and_then(|user| {
        kubeconfig
            .auth_infos
            .iter()
            .find(|candidate| candidate.name == user)
            .and_then(|candidate| candidate.auth_info.as_ref())
    });
    match auth_info {
        Some(auth) if auth.exec.is_some() => "OIDC / exec".to_string(),
        Some(auth) if auth.auth_provider.is_some() => "OIDC provider".to_string(),
        Some(auth) if auth.token.is_some() || auth.token_file.is_some() => {
            "Bearer token".to_string()
        }
        Some(auth)
            if auth.client_certificate.is_some() || auth.client_certificate_data.is_some() =>
        {
            "Client certificate".to_string()
        }
        _ => "Credentials unavailable".to_string(),
    }
}

fn contexts_from_kubeconfig(
    kubeconfig: &Kubeconfig,
    source_path: Option<String>,
) -> Vec<KubeContext> {
    let current_context = kubeconfig.current_context.as_deref();
    kubeconfig
        .contexts
        .iter()
        .map(|named| {
            let context = named.context.clone().unwrap_or_default();
            KubeContext {
                current: current_context == Some(named.name.as_str()),
                name: named.name.clone(),
                cluster: context.cluster,
                namespace: context.namespace.unwrap_or_else(|| "default".to_string()),
                auth_method: auth_method_for(kubeconfig, context.user.as_deref()),
                source_path: source_path.clone(),
            }
        })
        .collect()
}

fn validate_relocatable_file_reference(
    value: Option<&str>,
    overridden_by_inline_data: bool,
    owner: &str,
    field: &str,
    inline_field: &str,
) -> Result<(), String> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(());
    };
    if overridden_by_inline_data || Path::new(value).is_absolute() {
        return Ok(());
    }
    Err(format!(
        "{owner} uses a relative `{field}` path. Pasted kubeconfigs are stored in Kuberniva's app data, so use an absolute path or embed the value with `{inline_field}`"
    ))
}

fn validate_relocatable_kubeconfig(kubeconfig: &Kubeconfig) -> Result<(), String> {
    for named_cluster in &kubeconfig.clusters {
        let Some(cluster) = named_cluster.cluster.as_ref() else {
            continue;
        };
        validate_relocatable_file_reference(
            cluster.certificate_authority.as_deref(),
            cluster
                .certificate_authority_data
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty()),
            &format!("Cluster `{}`", named_cluster.name),
            "certificate-authority",
            "certificate-authority-data",
        )?;
    }

    for named_auth_info in &kubeconfig.auth_infos {
        let Some(auth_info) = named_auth_info.auth_info.as_ref() else {
            continue;
        };
        let owner = format!("User `{}`", named_auth_info.name);
        validate_relocatable_file_reference(
            auth_info.client_certificate.as_deref(),
            auth_info
                .client_certificate_data
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty()),
            &owner,
            "client-certificate",
            "client-certificate-data",
        )?;
        validate_relocatable_file_reference(
            auth_info.client_key.as_deref(),
            auth_info.client_key_data.is_some(),
            &owner,
            "client-key",
            "client-key-data",
        )?;
        validate_relocatable_file_reference(
            auth_info.token_file.as_deref(),
            auth_info.token.is_some(),
            &owner,
            "tokenFile",
            "token",
        )?;

        if let Some(exec) = auth_info.exec.as_ref() {
            if let Some(command) = exec
                .command
                .as_deref()
                .map(str::trim)
                .filter(|command| !command.is_empty())
            {
                let contains_separator = command.contains('/') || command.contains('\\');
                if contains_separator && !Path::new(command).is_absolute() {
                    return Err(format!(
                        "{owner} uses a relative exec command. Pasted kubeconfigs are stored in Kuberniva's app data, so use a bare command available on PATH or an absolute path"
                    ));
                }
            }
        }

        if let Some(auth_provider) = auth_info.auth_provider.as_ref() {
            let idp_ca = auth_provider
                .config
                .get("idp-certificate-authority")
                .map(String::as_str);
            let has_inline_idp_ca = auth_provider
                .config
                .get("idp-certificate-authority-data")
                .is_some_and(|value| !value.trim().is_empty());
            validate_relocatable_file_reference(
                idp_ca,
                has_inline_idp_ca,
                &owner,
                "idp-certificate-authority",
                "idp-certificate-authority-data",
            )?;
        }
    }

    Ok(())
}

fn validate_imported_kubeconfig(kubeconfig: &Kubeconfig) -> Result<(), String> {
    if let Some(kind) = kubeconfig.kind.as_deref() {
        if kind != "Config" {
            return Err(format!(
                "The pasted YAML is a Kubernetes `{kind}` object, not a kubeconfig"
            ));
        }
    }
    if let Some(api_version) = kubeconfig.api_version.as_deref() {
        if api_version != "v1" {
            return Err(format!(
                "Unsupported kubeconfig API version `{api_version}`; expected `v1`"
            ));
        }
    }
    if kubeconfig.contexts.is_empty() {
        return Err("The pasted kubeconfig does not contain any contexts".to_string());
    }
    if kubeconfig.clusters.is_empty() {
        return Err("The pasted kubeconfig does not contain any clusters".to_string());
    }

    let mut cluster_names = HashSet::new();
    for cluster in &kubeconfig.clusters {
        if cluster.name.trim().is_empty() {
            return Err("The pasted kubeconfig contains a cluster without a name".to_string());
        }
        if !cluster_names.insert(cluster.name.as_str()) {
            return Err(format!(
                "The pasted kubeconfig contains duplicate cluster `{}` entries",
                cluster.name
            ));
        }
    }

    let mut auth_info_names = HashSet::new();
    for auth_info in &kubeconfig.auth_infos {
        if auth_info.name.trim().is_empty() {
            return Err("The pasted kubeconfig contains a user without a name".to_string());
        }
        if !auth_info_names.insert(auth_info.name.as_str()) {
            return Err(format!(
                "The pasted kubeconfig contains duplicate user `{}` entries",
                auth_info.name
            ));
        }
    }

    let mut context_names = HashSet::new();
    for named_context in &kubeconfig.contexts {
        if named_context.name.trim().is_empty() {
            return Err("The pasted kubeconfig contains a context without a name".to_string());
        }
        if !context_names.insert(named_context.name.as_str()) {
            return Err(format!(
                "The pasted kubeconfig contains duplicate context `{}` entries",
                named_context.name
            ));
        }
        let context = named_context.context.as_ref().ok_or_else(|| {
            format!(
                "Context `{}` does not contain connection details",
                named_context.name
            )
        })?;
        if context.cluster.trim().is_empty() || !cluster_names.contains(context.cluster.as_str()) {
            return Err(format!(
                "Context `{}` references missing cluster `{}`",
                named_context.name, context.cluster
            ));
        }
        if let Some(user) = context.user.as_deref() {
            if !user.trim().is_empty() && !auth_info_names.contains(user) {
                return Err(format!(
                    "Context `{}` references missing user `{user}`",
                    named_context.name
                ));
            }
        }
    }

    for cluster_name in kubeconfig
        .contexts
        .iter()
        .filter_map(|named| named.context.as_ref())
        .map(|context| context.cluster.as_str())
        .collect::<HashSet<_>>()
    {
        let cluster = kubeconfig
            .clusters
            .iter()
            .find(|candidate| candidate.name == cluster_name)
            .and_then(|candidate| candidate.cluster.as_ref())
            .ok_or_else(|| {
                format!("Cluster `{cluster_name}` does not contain connection details")
            })?;
        if cluster
            .server
            .as_deref()
            .map_or(true, |server| server.trim().is_empty())
        {
            return Err(format!(
                "Cluster `{cluster_name}` does not declare a Kubernetes API server"
            ));
        }
    }

    if let Some(current_context) = kubeconfig
        .current_context
        .as_deref()
        .filter(|context| !context.trim().is_empty())
    {
        if !context_names.contains(current_context) {
            return Err(format!(
                "Current context `{current_context}` is not present in the pasted kubeconfig"
            ));
        }
    }

    validate_relocatable_kubeconfig(kubeconfig)?;

    Ok(())
}

fn parse_pasted_kubeconfig(yaml: &str) -> Result<(Kubeconfig, String, String), String> {
    if yaml.trim().is_empty() {
        return Err("Paste a kubeconfig before importing it".to_string());
    }
    if yaml.len() > MAX_PASTED_KUBECONFIG_BYTES {
        return Err(format!(
            "The pasted kubeconfig is larger than the {} MiB import limit",
            MAX_PASTED_KUBECONFIG_BYTES / 1024 / 1024
        ));
    }

    let normalized = yaml
        .trim_start_matches('\u{feff}')
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let persisted = format!("{}\n", normalized.trim());
    let kubeconfig = Kubeconfig::from_yaml(&persisted)
        .map_err(|error| format!("Could not parse the pasted kubeconfig: {error}"))?;
    validate_imported_kubeconfig(&kubeconfig)?;
    let canonical = serde_yaml::to_string(&kubeconfig)
        .map_err(|error| format!("Could not normalize the pasted kubeconfig: {error}"))?;
    Ok((kubeconfig, persisted, canonical))
}

fn ensure_managed_kubeconfig_directory(directory: &Path) -> Result<(), String> {
    match fs::symlink_metadata(directory) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err("Kuberniva's managed kubeconfig path is not a directory".to_string());
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir_all(directory)
                .map_err(|error| format!("Could not create managed kubeconfig storage: {error}"))?;
        }
        Err(error) => {
            return Err(format!(
                "Could not inspect managed kubeconfig storage: {error}"
            ));
        }
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("Could not secure managed kubeconfig storage: {error}"))?;
    }
    Ok(())
}

fn secure_kubeconfig_file(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|error| format!("Could not secure imported kubeconfig: {error}"))?;
    }
    Ok(())
}

fn kubeconfig_summary_with_source(kubeconfig: &Kubeconfig, source: &Path) -> KubeconfigSummary {
    let source_path = source
        .canonicalize()
        .unwrap_or_else(|_| source.to_path_buf())
        .to_string_lossy()
        .into_owned();
    KubeconfigSummary {
        contexts: contexts_from_kubeconfig(kubeconfig, Some(source_path)),
        current_context: kubeconfig.current_context.clone(),
    }
}

fn managed_kubeconfig_path(managed_directory: &Path, canonical: &str) -> PathBuf {
    let digest = format!("{:x}", Sha256::digest(canonical.as_bytes()));
    managed_directory.join(format!("kubeconfig-{digest}.yaml"))
}

fn atomically_replace_managed_kubeconfig(path: &Path, contents: &str) -> Result<(), String> {
    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("kubeconfig.yaml");
    let mut staged = None;
    for suffix in 0..1_000_u16 {
        let temporary_path = path.with_file_name(format!(
            ".{filename}.kuberniva-import-{}-{suffix}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&temporary_path) {
            Ok(file) => {
                staged = Some((temporary_path, file));
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!("Could not stage imported kubeconfig: {error}"));
            }
        }
    }
    let (temporary_path, mut temporary_file) =
        staged.ok_or_else(|| "Could not allocate a temporary kubeconfig filename".to_string())?;
    if let Err(error) = temporary_file
        .write_all(contents.as_bytes())
        .and_then(|_| temporary_file.sync_all())
    {
        drop(temporary_file);
        let _ = fs::remove_file(&temporary_path);
        return Err(format!("Could not save imported kubeconfig: {error}"));
    }
    drop(temporary_file);
    if let Err(error) = secure_kubeconfig_file(&temporary_path) {
        let _ = fs::remove_file(&temporary_path);
        return Err(error);
    }
    // std::fs::rename replaces an existing destination on Unix, but Windows
    // requires the destination to be removed first. The import lock keeps this
    // short replacement window private to Kuberniva's managed store.
    #[cfg(windows)]
    if let Err(error) = fs::remove_file(path) {
        if error.kind() != std::io::ErrorKind::NotFound {
            let _ = fs::remove_file(&temporary_path);
            return Err(format!(
                "Could not replace the previous imported kubeconfig: {error}"
            ));
        }
    }
    if let Err(error) = fs::rename(&temporary_path, path) {
        let _ = fs::remove_file(&temporary_path);
        return Err(format!("Could not install imported kubeconfig: {error}"));
    }
    Ok(())
}

fn import_pasted_kubeconfig_into(
    kubeconfig_yaml: &str,
    managed_directory: &Path,
) -> Result<KubeconfigSummary, String> {
    let (kubeconfig, persisted, canonical) = parse_pasted_kubeconfig(kubeconfig_yaml)?;
    let _import_guard = KUBECONFIG_IMPORT_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| "Kuberniva's kubeconfig importer is unavailable".to_string())?;
    ensure_managed_kubeconfig_directory(managed_directory)?;

    let managed_path = managed_kubeconfig_path(managed_directory, &canonical);
    match fs::symlink_metadata(&managed_path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(
                    "Kuberniva's digest-owned kubeconfig path is not a regular file".to_string(),
                );
            }
            let existing_matches = fs::read_to_string(&managed_path)
                .ok()
                .and_then(|yaml| Kubeconfig::from_yaml(&yaml).ok())
                .and_then(|existing| serde_yaml::to_string(&existing).ok())
                .is_some_and(|existing| existing == canonical);
            if !existing_matches {
                atomically_replace_managed_kubeconfig(&managed_path, &persisted)?;
            } else {
                secure_kubeconfig_file(&managed_path)?;
            }
            return Ok(kubeconfig_summary_with_source(&kubeconfig, &managed_path));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "Could not inspect the managed kubeconfig destination: {error}"
            ));
        }
    }

    let mut existing_files = fs::read_dir(managed_directory)
        .map_err(|error| format!("Could not read managed kubeconfig storage: {error}"))?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let filename = entry.file_name();
            let filename = filename.to_str()?;
            entry.file_type().ok().filter(|kind| kind.is_file())?;
            (filename.starts_with("kubeconfig-") && filename.ends_with(".yaml"))
                .then(|| entry.path())
        })
        .collect::<Vec<_>>();
    existing_files.sort();
    for path in existing_files {
        let Ok(existing_yaml) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(existing) = Kubeconfig::from_yaml(&existing_yaml) else {
            continue;
        };
        let Ok(existing_canonical) = serde_yaml::to_string(&existing) else {
            continue;
        };
        if existing_canonical == canonical {
            secure_kubeconfig_file(&path)?;
            return Ok(kubeconfig_summary_with_source(&existing, &path));
        }
    }
    atomically_replace_managed_kubeconfig(&managed_path, &persisted)?;
    Ok(kubeconfig_summary_with_source(&kubeconfig, &managed_path))
}

fn category_for(group: &str, plural: &str) -> (String, bool, bool) {
    let plural = plural.to_ascii_lowercase();
    let builtin_groups = [
        "",
        "apps",
        "batch",
        "autoscaling",
        "policy",
        "networking.k8s.io",
        "discovery.k8s.io",
        "storage.k8s.io",
        "rbac.authorization.k8s.io",
        "authentication.k8s.io",
        "authorization.k8s.io",
        "admissionregistration.k8s.io",
        "apiextensions.k8s.io",
        "apiregistration.k8s.io",
        "scheduling.k8s.io",
        "coordination.k8s.io",
        "events.k8s.io",
        "flowcontrol.apiserver.k8s.io",
        "node.k8s.io",
        "certificates.k8s.io",
    ];
    let custom = !builtin_groups.contains(&group);
    let crd = custom || plural == "customresourcedefinitions";
    // Gateway API resources are CRD-backed in most clusters, but they form a
    // first-class operational surface. Keep them out of the generic custom
    // resource bucket so Gateways and Routes are immediately discoverable.
    let category = if group == "gateway.networking.k8s.io" {
        "Gateway APIs"
    } else if crd {
        "Custom Resources"
    } else if matches!(
        plural.as_str(),
        "pods"
            | "deployments"
            | "daemonsets"
            | "statefulsets"
            | "replicasets"
            | "replicationcontrollers"
            | "jobs"
            | "cronjobs"
            | "horizontalpodautoscalers"
    ) {
        "Workloads"
    } else if matches!(
        plural.as_str(),
        "configmaps"
            | "secrets"
            | "resourcequotas"
            | "limitranges"
            | "priorityclasses"
            | "runtimeclasses"
    ) {
        "Configuration"
    } else if matches!(
        plural.as_str(),
        "serviceaccounts"
            | "roles"
            | "rolebindings"
            | "clusterroles"
            | "clusterrolebindings"
            | "leases"
    ) {
        "Access Control"
    } else if matches!(
        plural.as_str(),
        "services"
            | "ingresses"
            | "networkpolicies"
            | "endpoints"
            | "endpointslices"
            | "gateways"
            | "httproutes"
            | "grpcroutes"
            | "tcproutes"
            | "udproutes"
            | "referencegrants"
    ) {
        "Network"
    } else if matches!(
        plural.as_str(),
        "persistentvolumes"
            | "persistentvolumeclaims"
            | "storageclasses"
            | "volumeattachments"
            | "csidrivers"
            | "csinodes"
            | "csistoragecapacities"
    ) {
        "Storage"
    } else {
        "Cluster"
    };
    (category.to_string(), custom, crd)
}

fn api_resource(group: String, version: String, kind: String, plural: String) -> ApiResource {
    let api_version = if group.is_empty() {
        version.clone()
    } else {
        format!("{group}/{version}")
    };
    ApiResource {
        group,
        version,
        api_version,
        kind,
        plural,
    }
}

fn normalized_access_value(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_string();
        (!value.is_empty()).then_some(value)
    })
}

async fn review_resource_access(client: Client, check: AccessReviewCheck) -> AccessReviewDecision {
    let review = SelfSubjectAccessReview {
        metadata: Default::default(),
        spec: SelfSubjectAccessReviewSpec {
            non_resource_attributes: None,
            resource_attributes: Some(ResourceAttributes {
                group: Some(check.group.clone()),
                version: Some(check.version.clone()),
                resource: Some(check.resource.clone()),
                verb: Some(check.verb.clone()),
                namespace: normalized_access_value(check.namespace.clone()),
                name: normalized_access_value(check.name.clone()),
                subresource: normalized_access_value(check.subresource.clone()),
                ..Default::default()
            }),
        },
        status: None,
    };
    match Api::<SelfSubjectAccessReview>::all(client)
        .create(&PostParams::default(), &review)
        .await
    {
        Ok(response) => match response.status {
            Some(status) => AccessReviewDecision {
                key: check.key,
                allowed: status.allowed,
                denied: status.denied.unwrap_or(false),
                reason: status.reason,
                evaluation_error: status.evaluation_error,
            },
            None => AccessReviewDecision {
                key: check.key,
                allowed: false,
                denied: false,
                reason: None,
                evaluation_error: Some(
                    "The Kubernetes API returned no authorization decision".to_string(),
                ),
            },
        },
        Err(error) => AccessReviewDecision {
            key: check.key,
            allowed: false,
            denied: false,
            reason: None,
            evaluation_error: Some(error.to_string()),
        },
    }
}

fn access_denied_message(check: &AccessReviewCheck, decision: &AccessReviewDecision) -> String {
    let target = check
        .subresource
        .as_ref()
        .filter(|value| !value.is_empty())
        .map(|subresource| format!("{}/{}", check.resource, subresource))
        .unwrap_or_else(|| check.resource.clone());
    let scope = check
        .namespace
        .as_ref()
        .filter(|value| !value.is_empty())
        .map(|namespace| format!(" in namespace {namespace}"))
        .unwrap_or_default();
    let detail = decision
        .evaluation_error
        .as_ref()
        .or(decision.reason.as_ref())
        .map(|detail| format!(": {detail}"))
        .unwrap_or_default();
    format!(
        "Kubernetes authorization did not allow {} on {}{}{}",
        check.verb, target, scope, detail
    )
}

async fn ensure_resource_access(client: Client, check: AccessReviewCheck) -> Result<(), String> {
    let decision = review_resource_access(client, check.clone()).await;
    decision
        .allowed
        .then_some(())
        .ok_or_else(|| access_denied_message(&check, &decision))
}

#[tauri::command]
async fn check_resource_permissions(
    request: AccessReviewRequest,
) -> Result<Vec<AccessReviewDecision>, String> {
    if request.checks.len() > 256 {
        return Err("Permission reviews are limited to 256 actions per request".to_string());
    }
    if request.checks.iter().any(|check| {
        check.key.trim().is_empty()
            || check.resource.trim().is_empty()
            || check.verb.trim().is_empty()
    }) {
        return Err("Every permission review needs a key, resource, and verb".to_string());
    }
    let client = client_for(request.kubeconfig_path, request.context).await?;
    let decisions = futures_util::stream::iter(request.checks.into_iter().map(|check| {
        let client = client.clone();
        async move { review_resource_access(client, check).await }
    }))
    .buffer_unordered(16)
    .collect::<Vec<_>>()
    .await;
    Ok(decisions)
}

fn workload_label_selector(manifest: &Value) -> Result<String, String> {
    let selector = manifest
        .pointer("/spec/selector")
        .and_then(Value::as_object)
        .ok_or_else(|| "This resource does not expose a pod label selector".to_string())?;
    let mut parts = Vec::new();
    if let Some(labels) = selector.get("matchLabels").and_then(Value::as_object) {
        for (key, value) in labels {
            let value = value.as_str().ok_or_else(|| {
                "The workload pod selector is not a set of string labels".to_string()
            })?;
            parts.push(format!("{key}={value}"));
        }
    }
    if let Some(expressions) = selector.get("matchExpressions").and_then(Value::as_array) {
        for expression in expressions {
            let key = expression
                .get("key")
                .and_then(Value::as_str)
                .filter(|key| !key.is_empty())
                .ok_or_else(|| {
                    "A workload label selector expression is missing its key".to_string()
                })?;
            let operator = expression
                .get("operator")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    "A workload label selector expression is missing its operator".to_string()
                })?;
            let values = match expression.get("values").and_then(Value::as_array) {
                Some(values) => values
                    .iter()
                    .map(|value| value.as_str().map(str::to_string))
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(|| {
                        "A workload label selector expression contains a non-string value"
                            .to_string()
                    })?,
                None => Vec::new(),
            };
            match operator {
                "In" if !values.is_empty() => {
                    parts.push(format!("{key} in ({})", values.join(",")))
                }
                "NotIn" if !values.is_empty() => {
                    parts.push(format!("{key} notin ({})", values.join(",")))
                }
                "Exists" => parts.push(key.to_string()),
                "DoesNotExist" => parts.push(format!("!{key}")),
                _ => {
                    return Err(format!(
                        "Unsupported or empty label selector expression for {key}"
                    ))
                }
            }
        }
    }
    if parts.is_empty() {
        return Err("This workload has an empty pod label selector".to_string());
    }
    Ok(parts.join(","))
}

fn certificate_info(manifest: &Value) -> Option<CertificateInfo> {
    if let Some(expires_at) = manifest.pointer("/status/notAfter").and_then(Value::as_str) {
        let expiry = chrono::DateTime::parse_from_rfc3339(expires_at)
            .ok()?
            .with_timezone(&Utc);
        let days_remaining =
            days_remaining_from_seconds(expiry.timestamp() - Utc::now().timestamp());
        return Some(CertificateInfo {
            expires_at: expiry.to_rfc3339(),
            days_remaining,
            expired: days_remaining < 0,
        });
    }

    let certificate = manifest.pointer("/data/tls.crt").and_then(Value::as_str)?;
    let bytes = STANDARD.decode(certificate).ok()?;
    let expiry_timestamp = if let Ok((_, pem)) = parse_x509_pem(&bytes) {
        pem.parse_x509().ok()?.validity().not_after.timestamp()
    } else {
        parse_x509_certificate(&bytes)
            .ok()?
            .1
            .validity()
            .not_after
            .timestamp()
    };
    let days_remaining = days_remaining_from_seconds(expiry_timestamp - Utc::now().timestamp());
    let expires_at = chrono::DateTime::from_timestamp(expiry_timestamp, 0)?.to_rfc3339();
    Some(CertificateInfo {
        expires_at,
        days_remaining,
        expired: days_remaining < 0,
    })
}

fn quantity_as_number(quantity: &str, cpu: bool) -> Option<f64> {
    let quantity = quantity.trim();
    if quantity.is_empty() {
        return None;
    }
    // Plain and exponent values are valid Kubernetes quantities, and are the
    // common representation for CPU cores and byte counts.
    if let Ok(number) = quantity.parse::<f64>() {
        return Some(number);
    }
    let units: &[(&str, f64)] = if cpu {
        &[
            ("n", 1e-9),
            ("u", 1e-6),
            ("m", 1e-3),
            ("k", 1e3),
            ("M", 1e6),
            ("G", 1e9),
            ("T", 1e12),
            ("P", 1e15),
            ("E", 1e18),
        ]
    } else {
        &[
            ("Ki", 1024_f64.powi(1)),
            ("Mi", 1024_f64.powi(2)),
            ("Gi", 1024_f64.powi(3)),
            ("Ti", 1024_f64.powi(4)),
            ("Pi", 1024_f64.powi(5)),
            ("Ei", 1024_f64.powi(6)),
            ("k", 1e3),
            ("M", 1e6),
            ("G", 1e9),
            ("T", 1e12),
            ("P", 1e15),
            ("E", 1e18),
            ("m", 1e-3),
        ]
    };
    units.iter().find_map(|(suffix, multiplier)| {
        quantity
            .strip_suffix(suffix)
            .and_then(|number| number.parse::<f64>().ok())
            .map(|number| number * multiplier)
    })
}

fn usage_percent(usage: Option<&str>, capacity: Option<&str>, cpu: bool) -> Option<f64> {
    let usage = quantity_as_number(usage?, cpu)?;
    let capacity = quantity_as_number(capacity?, cpu)?;
    (capacity > 0.0).then(|| (usage / capacity * 100.0).clamp(0.0, 100.0))
}

fn format_cpu_usage(cores: f64) -> String {
    if cores >= 1.0 {
        format!("{cores:.2} cores")
    } else {
        // Keep sub-core values in the same unit as the rest of the UI.  The
        // Kubernetes API commonly returns values such as 1m/2m, but those
        // labels are difficult to compare with node and cluster totals.
        let rounded = (cores * 1_000.0).round() / 1_000.0;
        let display = if cores > 0.0 && rounded == 0.0 {
            "<0.001".to_string()
        } else {
            format!("{rounded:.3}")
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_string()
        };
        format!("{display} cores")
    }
}

fn format_memory_usage(bytes: f64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    if bytes >= GIB {
        format!("{:.1}Gi", bytes / GIB)
    } else if bytes >= MIB {
        format!("{:.0}Mi", bytes / MIB)
    } else if bytes >= KIB {
        format!("{:.0}Ki", bytes / KIB)
    } else {
        format!("{}B", bytes.max(0.0).round() as i64)
    }
}

fn sum_quantities(values: Vec<Option<&str>>, cpu: bool) -> Option<f64> {
    let values = values
        .into_iter()
        .flatten()
        .filter_map(|value| quantity_as_number(value, cpu))
        .collect::<Vec<_>>();
    (!values.is_empty()).then(|| values.into_iter().sum())
}

fn node_property_value<'a>(properties: &'a [NodeProperty], key: &str) -> Option<&'a str> {
    properties
        .iter()
        .find(|property| property.key == key)
        .map(|property| property.value.as_str())
}

async fn pod_usage_map(
    client: Client,
    namespace: Option<&str>,
) -> HashMap<(String, String), (String, String)> {
    let resource = ApiResource {
        group: "metrics.k8s.io".to_string(),
        version: "v1beta1".to_string(),
        api_version: "metrics.k8s.io/v1beta1".to_string(),
        kind: "PodMetrics".to_string(),
        plural: "pods".to_string(),
    };
    let api = match namespace.filter(|value| *value != "all namespaces") {
        Some(namespace) => Api::<DynamicObject>::namespaced_with(client, namespace, &resource),
        None => Api::<DynamicObject>::all_with(client, &resource),
    };
    let response = match api.list(&ListParams::default()).await {
        Ok(response) => response,
        Err(_) => return HashMap::new(),
    };
    response
        .items
        .into_iter()
        .filter_map(|metric| {
            let name = metric.metadata.name?;
            let namespace = metric.metadata.namespace.unwrap_or_default();
            let containers = metric.data.get("containers")?.as_array()?;
            let mut cpu_values = Vec::new();
            let mut memory_values = Vec::new();
            for container in containers {
                let usage = container.get("usage").and_then(Value::as_object);
                if let Some(cpu) = usage
                    .and_then(|usage| usage.get("cpu"))
                    .and_then(Value::as_str)
                    .and_then(|value| quantity_as_number(value, true))
                {
                    cpu_values.push(cpu);
                }
                if let Some(memory) = usage
                    .and_then(|usage| usage.get("memory"))
                    .and_then(Value::as_str)
                    .and_then(|value| quantity_as_number(value, false))
                {
                    memory_values.push(memory);
                }
            }
            if cpu_values.is_empty() && memory_values.is_empty() {
                return None;
            }
            let cpu = if cpu_values.is_empty() {
                String::new()
            } else {
                format_cpu_usage(cpu_values.into_iter().sum())
            };
            let memory = if memory_values.is_empty() {
                String::new()
            } else {
                format_memory_usage(memory_values.into_iter().sum())
            };
            Some(((namespace, name), (cpu, memory)))
        })
        .collect()
}

fn pod_resource_object(pod: Pod, usage: Option<&(String, String)>) -> ResourceObject {
    let name = pod.metadata.name.unwrap_or_default();
    let namespace = pod.metadata.namespace;
    let uid = pod.metadata.uid;
    let resource_version = pod.metadata.resource_version;
    let created_at = pod
        .metadata
        .creation_timestamp
        .map(|timestamp| timestamp.0.to_string());
    let total_containers = pod.spec.as_ref().map(|spec| spec.containers.len() as u32);
    let node_name = pod.spec.as_ref().and_then(|spec| spec.node_name.clone());
    let (ready_containers, restarts) = pod
        .status
        .as_ref()
        .and_then(|status| status.container_statuses.as_ref())
        .map(|statuses| {
            (
                statuses.iter().filter(|status| status.ready).count() as u32,
                statuses
                    .iter()
                    .map(|status| status.restart_count.max(0) as u32)
                    .sum::<u32>(),
            )
        })
        .unwrap_or((0, 0));
    ResourceObject {
        name,
        namespace,
        uid,
        resource_version,
        created_at,
        status: pod.status.as_ref().and_then(|status| status.phase.clone()),
        ready_containers: total_containers.map(|_| ready_containers),
        total_containers,
        restarts: total_containers.map(|_| restarts),
        cpu_usage: usage
            .map(|usage| usage.0.clone())
            .filter(|value| !value.is_empty()),
        memory_usage: usage
            .map(|usage| usage.1.clone())
            .filter(|value| !value.is_empty()),
        node_name,
    }
}

fn configure_desktop_exec_path() {
    static PATH_CONFIGURED: OnceLock<()> = OnceLock::new();
    PATH_CONFIGURED.get_or_init(|| {
        let mut paths = env::var_os("PATH")
            .map(|path| env::split_paths(&path).collect::<Vec<_>>())
            .unwrap_or_default();
        let mut fallback_paths = vec![
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/opt/homebrew/sbin"),
            PathBuf::from("/usr/local/bin"),
            PathBuf::from("/usr/local/sbin"),
        ];
        if let Some(home) = env::var_os("HOME").map(PathBuf::from) {
            fallback_paths.extend([
                home.join(".local/bin"),
                home.join(".krew/bin"),
                home.join(".asdf/shims"),
                home.join("go/bin"),
                home.join("bin"),
            ]);
        }
        for fallback_path in fallback_paths {
            if fallback_path.is_dir() && !paths.contains(&fallback_path) {
                paths.push(fallback_path);
            }
        }
        if let Ok(path) = env::join_paths(paths) {
            env::set_var("PATH", path);
        }
    });
}

fn exec_command_for(
    kubeconfig: &Kubeconfig,
    requested_context: Option<&str>,
) -> Option<(String, Option<String>)> {
    let context_name = requested_context.or(kubeconfig.current_context.as_deref())?;
    let context = kubeconfig
        .contexts
        .iter()
        .find(|candidate| candidate.name == context_name)
        .and_then(|candidate| candidate.context.as_ref())?;
    let auth_info = context.user.as_deref().and_then(|user| {
        kubeconfig
            .auth_infos
            .iter()
            .find(|candidate| candidate.name == user)
            .and_then(|candidate| candidate.auth_info.as_ref())
    })?;
    let exec = auth_info.exec.as_ref()?;
    Some((exec.command.clone()?, exec.install_hint.clone()))
}

fn command_is_available(command: &str) -> bool {
    let command_path = Path::new(command);
    if command_path.components().count() > 1 {
        return command_path.is_file();
    }
    env::var_os("PATH")
        .map(|path| env::split_paths(&path).any(|directory| directory.join(command).is_file()))
        .unwrap_or(false)
}

fn validate_exec_command(
    kubeconfig: &Kubeconfig,
    requested_context: Option<&str>,
) -> Result<(), String> {
    configure_desktop_exec_path();
    let Some((command, install_hint)) = exec_command_for(kubeconfig, requested_context) else {
        return Ok(());
    };
    if command_is_available(&command) {
        return Ok(());
    }
    let hint = install_hint
        .map(|hint| format!(" {hint}"))
        .unwrap_or_else(|| " Install the credential helper, or set user.exec.command in this kubeconfig to its absolute path.".to_string());
    Err(format!(
        "OIDC credential helper `{command}` was not found. Kuberniva checked the app PATH plus Homebrew, Krew, ~/.local/bin, ~/.asdf/shims, ~/go/bin, and ~/bin.{hint}"
    ))
}

fn client_cache_key(path: Option<&str>, context: Option<&str>) -> String {
    let source = path
        .filter(|path| !path.trim().is_empty())
        .map(resolve_local_path)
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|| "default-kubeconfig".to_string());
    format!("{source}\u{0}{}", context.unwrap_or("current-context"))
}

async fn client_for(path: Option<String>, context: Option<String>) -> Result<Client, String> {
    let cache_key = client_cache_key(path.as_deref(), context.as_deref());
    let client_cache = KUBE_CLIENT_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(client) = client_cache
        .lock()
        .map_err(|_| "Kuberniva's Kubernetes client cache is unavailable".to_string())?
        .get(&cache_key)
        .cloned()
    {
        return Ok(client);
    }

    let options = KubeConfigOptions {
        context,
        ..Default::default()
    };
    let kubeconfig = load_kubeconfig(path.as_deref())?;
    validate_exec_command(&kubeconfig, options.context.as_deref())?;
    let config = Config::from_custom_kubeconfig(kubeconfig, &options)
        .await
        .map_err(|error| error.to_string())?;
    let client = Client::try_from(config).map_err(|error| error.to_string())?;
    let mut cache = client_cache
        .lock()
        .map_err(|_| "Kuberniva's Kubernetes client cache is unavailable".to_string())?;
    if cache.len() >= 64 {
        if let Some(evicted_key) = cache.keys().next().cloned() {
            cache.remove(&evicted_key);
        }
    }
    cache.insert(cache_key, client.clone());
    Ok(client)
}

#[tauri::command]
fn invalidate_cluster_client(
    kubeconfig_path: Option<String>,
    context: Option<String>,
) -> Result<(), String> {
    let cache_key = client_cache_key(kubeconfig_path.as_deref(), context.as_deref());
    let cache = KUBE_CLIENT_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    cache
        .lock()
        .map_err(|_| "Kuberniva's Kubernetes client cache is unavailable".to_string())?
        .remove(&cache_key);
    Ok(())
}

#[tauri::command]
async fn import_pasted_kubeconfig(
    app: tauri::AppHandle,
    content: String,
) -> Result<KubeconfigSummary, String> {
    let managed_directory = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Could not locate Kuberniva's app data directory: {error}"))?
        .join("kubeconfigs");
    tokio::task::spawn_blocking(move || import_pasted_kubeconfig_into(&content, &managed_directory))
        .await
        .map_err(|error| format!("Kubeconfig import was interrupted: {error}"))?
}

#[tauri::command]
fn read_kubeconfig_contexts(kubeconfig_path: Option<String>) -> Result<KubeconfigSummary, String> {
    if let Some(path) = kubeconfig_path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
    {
        let resolved_path = resolve_local_path(path);
        if resolved_path.is_dir() {
            let mut contexts = Vec::new();
            let mut files = fs::read_dir(&resolved_path)
                .map_err(|error| format!("Could not read kubeconfig directory: {error}"))?
                .filter_map(Result::ok)
                .filter_map(|entry| {
                    entry
                        .file_type()
                        .ok()
                        .filter(|file_type| file_type.is_file())
                        .map(|_| entry.path())
                })
                .collect::<Vec<_>>();
            files.sort();

            for file in files {
                let source_path = file
                    .canonicalize()
                    .unwrap_or(file)
                    .to_string_lossy()
                    .into_owned();
                if let Ok(kubeconfig) = Kubeconfig::read_from(&source_path) {
                    contexts.extend(contexts_from_kubeconfig(&kubeconfig, Some(source_path)));
                }
            }

            if contexts.is_empty() {
                return Err("No valid kubeconfig contexts found in this directory".to_string());
            }
            let current_context = contexts
                .iter()
                .find(|context| context.current)
                .map(|context| context.name.clone());
            return Ok(KubeconfigSummary {
                contexts,
                current_context,
            });
        }
    }

    let kubeconfig = load_kubeconfig(kubeconfig_path.as_deref())?;
    let current_context = kubeconfig.current_context.clone();
    let source_path = kubeconfig_path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
        .map(resolve_local_path)
        .map(|path| path.to_string_lossy().into_owned());
    let contexts = contexts_from_kubeconfig(&kubeconfig, source_path);
    Ok(KubeconfigSummary {
        contexts,
        current_context,
    })
}

#[tauri::command]
async fn discover_cluster_catalog(
    kubeconfig_path: Option<String>,
    context: Option<String>,
) -> Result<ClusterCatalog, String> {
    let selected_context = context
        .clone()
        .unwrap_or_else(|| "current context".to_string());
    let client = client_for(kubeconfig_path, context).await?;

    // Aggregated discovery is two API calls on modern clusters. Older servers retain the
    // conventional discovery fallback, so the catalog remains complete across versions.
    // Namespaces are listed concurrently so connecting costs one round trip, not two.
    let discover = async {
        match Discovery::new(client.clone()).run_aggregated().await {
            Ok(discovery) => Ok(discovery),
            Err(_) => Discovery::new(client.clone()).run().await,
        }
    };
    let namespace_api = Api::<Namespace>::all(client.clone());
    let namespace_params = ListParams::default();
    let (discovery, namespace_list) =
        tokio::join!(discover, namespace_api.list(&namespace_params));
    let discovery = discovery.map_err(|error| error.to_string())?;

    let mut resources = discovery
        .groups_alphabetical()
        .into_iter()
        .flat_map(|group| group.resources_by_stability())
        .filter(|(_, capabilities)| capabilities.supports_operation(verbs::LIST))
        .map(|(resource, capabilities)| {
            let (category, custom, crd) = category_for(&resource.group, &resource.plural);
            ResourceDescriptor {
                group: resource.group,
                version: resource.version,
                api_version: resource.api_version,
                kind: resource.kind,
                plural: resource.plural,
                namespaced: capabilities.scope == Scope::Namespaced,
                category,
                custom,
                crd,
            }
        })
        .collect::<Vec<_>>();
    resources.sort_by(|left, right| {
        left.category
            .cmp(&right.category)
            .then(left.kind.cmp(&right.kind))
    });

    let namespaces: BTreeSet<String> = match namespace_list {
        Ok(response) => response
            .items
            .into_iter()
            .filter_map(|namespace| namespace.metadata.name)
            .collect(),
        Err(error) => {
            log::warn!("Could not list namespaces while discovering the API catalog: {error}");
            BTreeSet::new()
        }
    };

    Ok(ClusterCatalog {
        context: selected_context,
        namespaces: namespaces.into_iter().collect(),
        resources,
    })
}

#[tauri::command]
async fn read_cluster_overview(
    kubeconfig_path: Option<String>,
    context: Option<String>,
) -> Result<ClusterOverview, String> {
    let client = client_for(kubeconfig_path, context).await?;
    let node_metrics_resource = ApiResource {
        group: "metrics.k8s.io".to_string(),
        version: "v1beta1".to_string(),
        api_version: "metrics.k8s.io/v1beta1".to_string(),
        kind: "NodeMetrics".to_string(),
        plural: "nodes".to_string(),
    };
    // Node metrics are optional (the metrics-server may not be installed). Fetch them in
    // parallel with the authoritative Node list so an unavailable Metrics API never delays
    // the cluster overview itself.
    let metrics_api = Api::<DynamicObject>::all_with(client.clone(), &node_metrics_resource);
    let nodes_api = Api::<Node>::all(client);
    let metrics_params = ListParams::default();
    let nodes_params = ListParams::default();
    let metrics_request = metrics_api.list(&metrics_params);
    let nodes_request = nodes_api.list(&nodes_params);
    let (metrics_response, nodes_response) = tokio::join!(metrics_request, nodes_request);
    let metrics = metrics_response.ok().map(|response| {
        response
            .items
            .into_iter()
            .filter_map(|metric| {
                let name = metric.metadata.name?;
                Some((
                    name,
                    (
                        metric
                            .data
                            .pointer("/usage/cpu")
                            .and_then(Value::as_str)
                            .map(str::to_string),
                        metric
                            .data
                            .pointer("/usage/memory")
                            .and_then(Value::as_str)
                            .map(str::to_string),
                    ),
                ))
            })
            .collect::<HashMap<_, _>>()
    });
    let metrics_available = metrics.is_some();
    let metrics = metrics.unwrap_or_default();
    let mut nodes = nodes_response
        .map_err(|error| error.to_string())?
        .items
        .into_iter()
        .filter_map(|node| {
            let name = node.metadata.name.clone()?;
            let label_map = node.metadata.labels.clone().unwrap_or_default();
            let roles = {
                let mut roles = label_map
                    .iter()
                    .filter_map(|(key, value)| {
                        if key == "kubernetes.io/role" {
                            Some(value.clone())
                        } else {
                            key.strip_prefix("node-role.kubernetes.io/").map(|role| {
                                if role.is_empty() {
                                    "worker".to_string()
                                } else {
                                    role.to_string()
                                }
                            })
                        }
                    })
                    .collect::<Vec<_>>();
                roles.sort();
                roles.dedup();
                roles
            };
            let labels = label_map
                .iter()
                .map(|(key, value)| NodeProperty {
                    key: key.clone(),
                    value: value.clone(),
                })
                .collect::<Vec<_>>();
            let annotations = node
                .metadata
                .annotations
                .as_ref()
                .map(|values| {
                    values
                        .iter()
                        .map(|(key, value)| NodeProperty {
                            key: key.clone(),
                            value: value.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            let ready = node
                .status
                .as_ref()
                .and_then(|status| status.conditions.as_ref())
                .map(|conditions| {
                    conditions
                        .iter()
                        .any(|condition| condition.type_ == "Ready" && condition.status == "True")
                })
                .unwrap_or(false);
            let capacity = node
                .status
                .as_ref()
                .and_then(|status| status.capacity.as_ref());
            let allocatable = node
                .status
                .as_ref()
                .and_then(|status| status.allocatable.as_ref());
            let (cpu_usage, memory_usage) = metrics.get(&name).cloned().unwrap_or((None, None));
            let cpu_capacity = capacity
                .and_then(|capacity| capacity.get("cpu"))
                .map(|quantity| quantity.0.clone());
            let memory_capacity = capacity
                .and_then(|capacity| capacity.get("memory"))
                .map(|quantity| quantity.0.clone());
            let capacity = capacity
                .map(|values| {
                    values
                        .iter()
                        .map(|(key, value)| NodeProperty {
                            key: key.clone(),
                            value: value.0.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            let allocatable = allocatable
                .map(|values| {
                    values
                        .iter()
                        .map(|(key, value)| NodeProperty {
                            key: key.clone(),
                            value: value.0.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            let status = node.status.as_ref();
            let node_info = status.and_then(|status| status.node_info.as_ref());
            let conditions = status
                .and_then(|status| status.conditions.as_ref())
                .map(|values| {
                    values
                        .iter()
                        .map(|condition| NodeConditionOverview {
                            type_: condition.type_.clone(),
                            status: condition.status.clone(),
                            reason: condition.reason.clone(),
                            message: condition.message.clone(),
                            last_heartbeat_time: node_time_string(
                                condition.last_heartbeat_time.as_ref(),
                            ),
                            last_transition_time: node_time_string(
                                condition.last_transition_time.as_ref(),
                            ),
                        })
                        .collect()
                })
                .unwrap_or_default();
            let addresses = status
                .and_then(|status| status.addresses.as_ref())
                .map(|values| {
                    values
                        .iter()
                        .map(|address| NodeAddressOverview {
                            type_: address.type_.clone(),
                            address: address.address.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default();
            let taints = node
                .spec
                .as_ref()
                .and_then(|spec| spec.taints.as_ref())
                .map(|values| {
                    values
                        .iter()
                        .map(|taint| NodeTaintOverview {
                            key: taint.key.clone(),
                            value: taint.value.clone(),
                            effect: taint.effect.clone(),
                            time_added: node_time_string(taint.time_added.as_ref()),
                        })
                        .collect()
                })
                .unwrap_or_default();
            let pod_cidrs = node
                .spec
                .as_ref()
                .map(|spec| {
                    spec.pod_cidrs
                        .clone()
                        .unwrap_or_else(|| spec.pod_cidr.clone().into_iter().collect())
                })
                .unwrap_or_default();
            Some(NodeOverview {
                name,
                ready,
                roles,
                labels,
                annotations,
                addresses,
                conditions,
                taints,
                architecture: node_info.map(|info| info.architecture.clone()),
                operating_system: node_info.map(|info| info.operating_system.clone()),
                os_image: node_info.map(|info| info.os_image.clone()),
                kernel_version: node_info.map(|info| info.kernel_version.clone()),
                kubelet_version: node_info.map(|info| info.kubelet_version.clone()),
                container_runtime_version: node_info
                    .map(|info| info.container_runtime_version.clone()),
                pod_cidrs,
                provider_id: node.spec.as_ref().and_then(|spec| spec.provider_id.clone()),
                unschedulable: node
                    .spec
                    .as_ref()
                    .and_then(|spec| spec.unschedulable)
                    .unwrap_or(false),
                uid: node.metadata.uid.clone(),
                creation_timestamp: node_time_string(node.metadata.creation_timestamp.as_ref()),
                capacity,
                allocatable,
                cpu_usage_percent: usage_percent(
                    cpu_usage.as_deref(),
                    cpu_capacity.as_deref(),
                    true,
                ),
                memory_usage_percent: usage_percent(
                    memory_usage.as_deref(),
                    memory_capacity.as_deref(),
                    false,
                ),
                cpu_capacity,
                memory_capacity,
                cpu_usage,
                memory_usage,
            })
        })
        .collect::<Vec<_>>();
    nodes.sort_by(|left, right| left.name.cmp(&right.name));
    let cpu_capacity = sum_quantities(
        nodes
            .iter()
            .map(|node| node.cpu_capacity.as_deref())
            .collect(),
        true,
    );
    let memory_capacity = sum_quantities(
        nodes
            .iter()
            .map(|node| node.memory_capacity.as_deref())
            .collect(),
        false,
    );
    let storage_capacity = sum_quantities(
        nodes
            .iter()
            .map(|node| node_property_value(&node.capacity, "ephemeral-storage"))
            .collect(),
        false,
    );
    let cpu_usage = sum_quantities(
        nodes.iter().map(|node| node.cpu_usage.as_deref()).collect(),
        true,
    );
    let memory_usage = sum_quantities(
        nodes
            .iter()
            .map(|node| node.memory_usage.as_deref())
            .collect(),
        false,
    );
    let totals = ClusterTotals {
        cpu_capacity: cpu_capacity.map(format_cpu_usage),
        memory_capacity: memory_capacity.map(format_memory_usage),
        storage_capacity: storage_capacity.map(format_memory_usage),
        cpu_usage_percent: cpu_usage.zip(cpu_capacity).and_then(|(usage, capacity)| {
            (capacity > 0.0).then(|| (usage / capacity * 100.0).clamp(0.0, 100.0))
        }),
        memory_usage_percent: memory_usage
            .zip(memory_capacity)
            .and_then(|(usage, capacity)| {
                (capacity > 0.0).then(|| (usage / capacity * 100.0).clamp(0.0, 100.0))
            }),
        cpu_usage: cpu_usage.map(format_cpu_usage),
        memory_usage: memory_usage.map(format_memory_usage),
        metric_nodes: nodes
            .iter()
            .filter(|node| node.cpu_usage.is_some() || node.memory_usage.is_some())
            .count(),
    };
    Ok(ClusterOverview {
        nodes,
        totals,
        metrics_available,
        observed_at: Utc::now().to_rfc3339(),
    })
}

#[tauri::command]
async fn read_cluster_events(
    kubeconfig_path: Option<String>,
    context: Option<String>,
) -> Result<Vec<ClusterEvent>, String> {
    let client = client_for(kubeconfig_path, context).await?;
    let params = ListParams {
        limit: Some(250),
        ..Default::default()
    };
    let mut events = Api::<Event>::all(client)
        .list(&params)
        .await
        .map_err(|error| error.to_string())?
        .items
        .into_iter()
        .filter_map(|event| {
            let name = event.metadata.name.clone()?;
            let last_observed = event
                .event_time
                .as_ref()
                .map(|time| time.0.to_string())
                .or_else(|| node_time_string(event.last_timestamp.as_ref()))
                .or_else(|| node_time_string(event.first_timestamp.as_ref()))
                .or_else(|| node_time_string(event.metadata.creation_timestamp.as_ref()));
            let first_observed = event
                .first_timestamp
                .as_ref()
                .and_then(|time| node_time_string(Some(time)))
                .or_else(|| event.event_time.as_ref().map(|time| time.0.to_string()));
            let source = event.reporting_component.clone().or_else(|| {
                event
                    .source
                    .as_ref()
                    .and_then(|source| source.component.clone())
            });
            Some(ClusterEvent {
                name,
                namespace: event
                    .metadata
                    .namespace
                    .clone()
                    .or(event.involved_object.namespace.clone()),
                event_type: event.type_.unwrap_or_else(|| "Normal".to_string()),
                reason: event.reason,
                message: event.message,
                involved_kind: event.involved_object.kind,
                involved_name: event.involved_object.name,
                action: event.action,
                count: event
                    .count
                    .or_else(|| event.series.as_ref().and_then(|series| series.count)),
                source,
                first_observed,
                last_observed,
            })
        })
        .collect::<Vec<_>>();
    events.sort_by(|left, right| right.last_observed.cmp(&left.last_observed));
    Ok(events)
}

fn dynamic_api_for_request(client: Client, request: &ResourceRequest) -> Api<DynamicObject> {
    let resource = api_resource(
        request.group.clone(),
        request.version.clone(),
        request.kind.clone(),
        request.plural.clone(),
    );
    if request.namespaced {
        match request.namespace.as_deref() {
            Some(namespace) if namespace != "all namespaces" => {
                Api::namespaced_with(client, namespace, &resource)
            }
            _ => Api::all_with(client, &resource),
        }
    } else {
        Api::all_with(client, &resource)
    }
}

fn resource_watch_signal(
    app: &tauri::AppHandle,
    watch_id: &str,
    action: &str,
    error: Option<String>,
) {
    let _ = app.emit(
        "kuberniva://resource-watch",
        ResourceWatchSignal {
            watch_id: watch_id.to_string(),
            action: action.to_string(),
            error,
        },
    );
}

async fn run_resource_watch(
    app: tauri::AppHandle,
    watch_id: String,
    request: ResourceRequest,
    mut stop_rx: oneshot::Receiver<()>,
) {
    let mut retry_delay = std::time::Duration::from_secs(1);
    loop {
        let client =
            match client_for(request.kubeconfig_path.clone(), request.context.clone()).await {
                Ok(client) => client,
                Err(error) => {
                    resource_watch_signal(&app, &watch_id, "error", Some(error));
                    tokio::select! {
                        _ = &mut stop_rx => return,
                        _ = tokio::time::sleep(retry_delay) => {}
                    }
                    retry_delay = (retry_delay * 2).min(std::time::Duration::from_secs(30));
                    continue;
                }
            };
        let api = dynamic_api_for_request(client, &request);
        let watch_params = WatchParams::default().timeout(290);
        let stream = match api.watch(&watch_params, "0").await {
            Ok(stream) => {
                retry_delay = std::time::Duration::from_secs(1);
                resource_watch_signal(&app, &watch_id, "connected", None);
                stream
            }
            Err(error) => {
                resource_watch_signal(&app, &watch_id, "error", Some(error.to_string()));
                let _ = invalidate_cluster_client(
                    request.kubeconfig_path.clone(),
                    request.context.clone(),
                );
                tokio::select! {
                    _ = &mut stop_rx => return,
                    _ = tokio::time::sleep(retry_delay) => {}
                }
                retry_delay = (retry_delay * 2).min(std::time::Duration::from_secs(30));
                continue;
            }
        };
        futures_util::pin_mut!(stream);

        loop {
            tokio::select! {
                _ = &mut stop_rx => return,
                event = stream.next() => {
                    match event {
                        Some(Ok(WatchEvent::Added(_))) => resource_watch_signal(&app, &watch_id, "added", None),
                        Some(Ok(WatchEvent::Modified(_))) => resource_watch_signal(&app, &watch_id, "modified", None),
                        Some(Ok(WatchEvent::Deleted(_))) => resource_watch_signal(&app, &watch_id, "deleted", None),
                        Some(Ok(WatchEvent::Bookmark(_))) => {}
                        Some(Ok(WatchEvent::Error(status))) => {
                            resource_watch_signal(&app, &watch_id, "error", Some(status.to_string()));
                            break;
                        }
                        Some(Err(error)) => {
                            resource_watch_signal(&app, &watch_id, "error", Some(error.to_string()));
                            break;
                        }
                        None => break,
                    }
                }
            }
        }

        // A watch can survive a laptop sleep long enough to return a stale
        // transport. Rebuild the client before reconnecting instead of
        // retrying the dead HTTP connection forever.
        let _ = invalidate_cluster_client(request.kubeconfig_path.clone(), request.context.clone());
        tokio::select! {
            _ = &mut stop_rx => return,
            _ = tokio::time::sleep(retry_delay) => {}
        }
        retry_delay = (retry_delay * 2).min(std::time::Duration::from_secs(30));
    }
}

#[tauri::command]
async fn start_resource_watch(
    app: tauri::AppHandle,
    request: ResourceRequest,
) -> Result<String, String> {
    let watch_id = format!(
        "resource-watch-{}",
        NEXT_RESOURCE_WATCH_ID.fetch_add(1, Ordering::Relaxed)
    );
    let (stop_tx, stop_rx) = oneshot::channel();
    RESOURCE_WATCH_REGISTRY
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| "Kuberniva's resource watch registry is unavailable".to_string())?
        .insert(watch_id.clone(), stop_tx);
    let task_watch_id = watch_id.clone();
    tokio::spawn(async move {
        run_resource_watch(app, task_watch_id.clone(), request, stop_rx).await;
        if let Some(registry) = RESOURCE_WATCH_REGISTRY.get() {
            if let Ok(mut registry) = registry.lock() {
                registry.remove(&task_watch_id);
            }
        }
    });
    Ok(watch_id)
}

#[tauri::command]
fn stop_resource_watch(watch_id: String) -> Result<(), String> {
    let sender = RESOURCE_WATCH_REGISTRY
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| "Kuberniva's resource watch registry is unavailable".to_string())?
        .remove(&watch_id);
    if let Some(sender) = sender {
        let _ = sender.send(());
    }
    Ok(())
}

#[tauri::command]
async fn list_resource_objects(request: ResourceRequest) -> Result<Vec<ResourceObject>, String> {
    let client = client_for(request.kubeconfig_path, request.context).await?;
    if request.kind == "Pod" {
        let namespace = request
            .namespace
            .as_deref()
            .filter(|value| *value != "all namespaces");
        let pods_api = match namespace {
            Some(namespace) => Api::<Pod>::namespaced(client.clone(), namespace),
            None => Api::<Pod>::all(client.clone()),
        };
        let list_params = ListParams::default();
        let (usage, pods_result) = tokio::join!(
            pod_usage_map(client, namespace),
            pods_api.list(&list_params),
        );
        let mut pods = pods_result
            .map_err(|error| error.to_string())?
            .items
            .into_iter()
            .map(|pod| {
                let key = (
                    pod.metadata.namespace.clone().unwrap_or_default(),
                    pod.metadata.name.clone().unwrap_or_default(),
                );
                pod_resource_object(pod, usage.get(&key))
            })
            .collect::<Vec<_>>();
        pods.sort_by(|left, right| {
            left.namespace
                .cmp(&right.namespace)
                .then(left.name.cmp(&right.name))
        });
        return Ok(pods);
    }
    let api_version = if request.group.is_empty() {
        request.version.clone()
    } else {
        format!("{}/{}", request.group, request.version)
    };
    let resource = ApiResource {
        group: request.group,
        version: request.version,
        api_version,
        kind: request.kind,
        plural: request.plural,
    };
    let api: Api<DynamicObject> = if request.namespaced {
        match request.namespace.as_deref() {
            Some(namespace) if namespace != "all namespaces" => {
                Api::namespaced_with(client, namespace, &resource)
            }
            _ => Api::all_with(client, &resource),
        }
    } else {
        Api::all_with(client, &resource)
    };
    let mut objects = api
        .list(&ListParams::default())
        .await
        .map_err(|error| error.to_string())?
        .items
        .into_iter()
        .map(|object| ResourceObject {
            name: object.metadata.name.unwrap_or_default(),
            namespace: object.metadata.namespace,
            uid: object.metadata.uid,
            resource_version: object.metadata.resource_version,
            created_at: object
                .metadata
                .creation_timestamp
                .map(|timestamp| timestamp.0.to_string()),
            status: None,
            ready_containers: None,
            total_containers: None,
            restarts: None,
            cpu_usage: None,
            memory_usage: None,
            node_name: None,
        })
        .collect::<Vec<_>>();
    objects.sort_by(|left, right| {
        left.namespace
            .cmp(&right.namespace)
            .then(left.name.cmp(&right.name))
    });
    Ok(objects)
}

#[tauri::command]
async fn get_resource_detail(request: ResourceObjectRequest) -> Result<ResourceDetail, String> {
    let client = client_for(request.kubeconfig_path, request.context).await?;
    let resource = api_resource(request.group, request.version, request.kind, request.plural);
    let object: DynamicObject = if request.namespaced {
        let namespace = request
            .namespace
            .ok_or_else(|| "A namespace is required for this resource".to_string())?;
        Api::<DynamicObject>::namespaced_with(client, &namespace, &resource)
            .get(&request.name)
            .await
            .map_err(|error| error.to_string())?
    } else {
        Api::<DynamicObject>::all_with(client, &resource)
            .get(&request.name)
            .await
            .map_err(|error| error.to_string())?
    };
    let manifest = serde_json::to_value(object).map_err(|error| error.to_string())?;
    let certificate = certificate_info(&manifest);
    let yaml = serde_yaml::to_string(&manifest).map_err(|error| error.to_string())?;
    Ok(ResourceDetail {
        manifest,
        yaml,
        certificate,
    })
}

#[tauri::command]
async fn delete_resource_object(request: ResourceObjectRequest) -> Result<(), String> {
    let client = client_for(request.kubeconfig_path, request.context).await?;
    ensure_resource_access(
        client.clone(),
        AccessReviewCheck {
            key: "delete".to_string(),
            group: request.group.clone(),
            version: request.version.clone(),
            resource: request.plural.clone(),
            verb: "delete".to_string(),
            namespace: request.namespace.clone(),
            name: Some(request.name.clone()),
            subresource: None,
        },
    )
    .await?;
    let resource = api_resource(request.group, request.version, request.kind, request.plural);
    let delete_params = if request.uid.is_some() || request.resource_version.is_some() {
        DeleteParams::default().preconditions(Preconditions {
            uid: request.uid,
            resource_version: request.resource_version,
        })
    } else {
        DeleteParams::default()
    };
    if request.namespaced {
        let namespace = request
            .namespace
            .ok_or_else(|| "A namespace is required for this resource".to_string())?;
        Api::<DynamicObject>::namespaced_with(client, &namespace, &resource)
            .delete(&request.name, &delete_params)
            .await
            .map_err(|error| error.to_string())?;
    } else {
        Api::<DynamicObject>::all_with(client, &resource)
            .delete(&request.name, &delete_params)
            .await
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn save_resource_detail(request: SaveResourceRequest) -> Result<(), String> {
    let client = client_for(request.kubeconfig_path, request.context).await?;
    ensure_resource_access(
        client.clone(),
        AccessReviewCheck {
            key: "update".to_string(),
            group: request.group.clone(),
            version: request.version.clone(),
            resource: request.plural.clone(),
            verb: "update".to_string(),
            namespace: request.namespace.clone(),
            name: Some(request.name.clone()),
            subresource: None,
        },
    )
    .await?;
    let resource = api_resource(request.group, request.version, request.kind, request.plural);
    let object: DynamicObject = serde_json::from_value(request.manifest)
        .map_err(|error| format!("Invalid resource data: {error}"))?;
    if request.namespaced {
        let namespace = request
            .namespace
            .ok_or_else(|| "A namespace is required for this resource".to_string())?;
        Api::namespaced_with(client, &namespace, &resource)
            .replace(&request.name, &kube::api::PostParams::default(), &object)
            .await
            .map_err(|error| error.to_string())?;
    } else {
        Api::all_with(client, &resource)
            .replace(&request.name, &kube::api::PostParams::default(), &object)
            .await
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn save_resource_yaml(request: SaveResourceYamlRequest) -> Result<(), String> {
    let manifest: Value =
        serde_yaml::from_str(&request.yaml).map_err(|error| format!("Invalid YAML: {error}"))?;
    save_resource_detail(SaveResourceRequest {
        kubeconfig_path: request.kubeconfig_path,
        context: request.context,
        group: request.group,
        version: request.version,
        kind: request.kind,
        plural: request.plural,
        namespaced: request.namespaced,
        namespace: request.namespace,
        name: request.name,
        manifest,
    })
    .await
}

#[tauri::command]
async fn list_workload_pods(request: WorkloadPodRequest) -> Result<Vec<ResourceObject>, String> {
    let client = client_for(request.kubeconfig_path, request.context).await?;
    let api_version = if request.group.is_empty() {
        request.version.clone()
    } else {
        format!("{}/{}", request.group, request.version)
    };
    let resource = ApiResource {
        group: request.group,
        version: request.version,
        api_version,
        kind: request.kind,
        plural: request.plural,
    };
    let workload: DynamicObject =
        Api::namespaced_with(client.clone(), &request.namespace, &resource)
            .get(&request.name)
            .await
            .map_err(|error| error.to_string())?;
    let selector = workload_label_selector(&workload.data)?;

    let pods_api = Api::<Pod>::namespaced(client.clone(), &request.namespace);
    let list_params = ListParams::default().labels(&selector);
    let (usage, pods_result) = tokio::join!(
        pod_usage_map(client, Some(&request.namespace)),
        pods_api.list(&list_params),
    );
    let mut pods = pods_result
        .map_err(|error| error.to_string())?
        .items
        .into_iter()
        .filter_map(|pod| {
            pod.metadata.name.as_ref()?;
            let key = (
                pod.metadata.namespace.clone().unwrap_or_default(),
                pod.metadata.name.clone().unwrap_or_default(),
            );
            Some(pod_resource_object(pod, usage.get(&key)))
        })
        .collect::<Vec<_>>();
    pods.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(pods)
}

#[tauri::command]
async fn read_pod_logs(request: PodLogRequest) -> Result<PodLogResponse, String> {
    let client = client_for(request.kubeconfig_path, request.context).await?;
    ensure_resource_access(
        client.clone(),
        AccessReviewCheck {
            key: "pod-logs".to_string(),
            group: String::new(),
            version: "v1".to_string(),
            resource: "pods".to_string(),
            verb: "get".to_string(),
            namespace: Some(request.namespace.clone()),
            name: Some(request.pod.clone()),
            subresource: Some("log".to_string()),
        },
    )
    .await?;
    let pods = Api::<Pod>::namespaced(client, &request.namespace);
    let pod = pods
        .get(&request.pod)
        .await
        .map_err(|error| error.to_string())?;
    let containers: Vec<String> = pod
        .spec
        .as_ref()
        .map(|spec| {
            spec.containers
                .iter()
                .map(|container| container.name.clone())
                .collect()
        })
        .unwrap_or_default();
    let ports = pod
        .spec
        .as_ref()
        .map(|spec| {
            spec.containers
                .iter()
                .flat_map(|container| {
                    container
                        .ports
                        .as_ref()
                        .into_iter()
                        .flatten()
                        .map(move |port| PodPort {
                            container: container.name.clone(),
                            name: port.name.clone(),
                            port: port.container_port,
                            protocol: port
                                .protocol
                                .as_ref()
                                .map(ToString::to_string)
                                .unwrap_or_else(|| "TCP".to_string()),
                        })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let since_seconds = request
        .since_time
        .as_deref()
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .map(|timestamp| {
            Utc::now()
                .signed_duration_since(timestamp.with_timezone(&Utc))
                .num_seconds()
                .max(1)
        });
    let tail_lines = since_seconds
        .is_none()
        .then(|| request.tail_lines.unwrap_or(500).clamp(1, 5_000));
    let selected_container = request.container.or_else(|| containers.first().cloned());
    let output = pods
        .logs(
            &request.pod,
            &LogParams {
                container: selected_container.clone(),
                since_seconds,
                tail_lines,
                timestamps: true,
                ..Default::default()
            },
        )
        .await
        .map_err(|error| error.to_string())?;
    Ok(PodLogResponse {
        lines: output.lines().map(str::to_string).collect(),
        containers,
        selected_container,
        ports,
    })
}

fn write_log_snapshot(path: &Path, content: &str) -> Result<(), String> {
    if content.len() > MAX_LOG_EXPORT_BYTES {
        return Err("Log exports are limited to 20 MiB".to_string());
    }
    if path.as_os_str().is_empty() || path.file_name().is_none() {
        return Err("Choose a valid file name for the log export".to_string());
    }
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .map_err(|error| format!("Could not create {}: {error}", path.display()))?;
    file.write_all(content.as_bytes())
        .map_err(|error| format!("Could not write {}: {error}", path.display()))?;
    file.sync_all()
        .map_err(|error| format!("Could not finish writing {}: {error}", path.display()))
}

#[tauri::command]
async fn save_log_file(request: SaveLogFileRequest) -> Result<(), String> {
    let path = PathBuf::from(request.path);
    let content = request.content;
    tokio::task::spawn_blocking(move || write_log_snapshot(&path, &content))
        .await
        .map_err(|error| format!("Could not finish the log export: {error}"))?
}

fn pod_runtime_info(pod: &Pod) -> PodRuntimeInfo {
    let containers = pod
        .spec
        .as_ref()
        .map(|spec| {
            spec.containers
                .iter()
                .map(|container| container.name.clone())
                .collect()
        })
        .unwrap_or_default();
    let ports = pod
        .spec
        .as_ref()
        .map(|spec| {
            spec.containers
                .iter()
                .flat_map(|container| {
                    container
                        .ports
                        .as_ref()
                        .into_iter()
                        .flatten()
                        .map(move |port| PodPort {
                            container: container.name.clone(),
                            name: port.name.clone(),
                            port: port.container_port,
                            protocol: port
                                .protocol
                                .as_ref()
                                .map(ToString::to_string)
                                .unwrap_or_else(|| "TCP".to_string()),
                        })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    PodRuntimeInfo { containers, ports }
}

#[tauri::command]
async fn get_pod_runtime(request: PodRuntimeRequest) -> Result<PodRuntimeInfo, String> {
    let client = client_for(request.kubeconfig_path, request.context).await?;
    let pod = Api::<Pod>::namespaced(client, &request.namespace)
        .get(&request.pod)
        .await
        .map_err(|error| error.to_string())?;
    Ok(pod_runtime_info(&pod))
}

#[tauri::command]
async fn exec_pod_command(request: PodExecRequest) -> Result<PodExecResponse, String> {
    let command = request.command.trim();
    if command.is_empty() {
        return Err("Enter a command to run in the selected container".to_string());
    }
    let client = client_for(request.kubeconfig_path, request.context).await?;
    ensure_resource_access(
        client.clone(),
        AccessReviewCheck {
            key: "pod-exec".to_string(),
            group: String::new(),
            version: "v1".to_string(),
            resource: "pods".to_string(),
            verb: "create".to_string(),
            namespace: Some(request.namespace.clone()),
            name: Some(request.pod.clone()),
            subresource: Some("exec".to_string()),
        },
    )
    .await?;
    let pods = Api::<Pod>::namespaced(client, &request.namespace);
    let selected_container = request
        .container
        .filter(|container| !container.trim().is_empty());
    let attach = selected_container
        .clone()
        .map(|container| AttachParams::default().container(container))
        .unwrap_or_default()
        .stdout(true)
        .stderr(true);
    let shell_command = command.to_string();
    let response = tokio::time::timeout(std::time::Duration::from_secs(30), async move {
        let mut attached = pods
            .exec(
                &request.pod,
                vec!["/bin/sh", "-lc", shell_command.as_str()],
                &attach,
            )
            .await
            .map_err(|error| error.to_string())?;
        let mut stdout_reader = attached.stdout().ok_or_else(|| {
            "The Kubernetes API did not provide stdout for this exec session".to_string()
        })?;
        let mut stderr_reader = attached.stderr().ok_or_else(|| {
            "The Kubernetes API did not provide stderr for this exec session".to_string()
        })?;
        let stdout_task = async {
            let mut buffer = Vec::new();
            stdout_reader
                .read_to_end(&mut buffer)
                .await
                .map_err(|error| error.to_string())?;
            Ok::<Vec<u8>, String>(buffer)
        };
        let stderr_task = async {
            let mut buffer = Vec::new();
            stderr_reader
                .read_to_end(&mut buffer)
                .await
                .map_err(|error| error.to_string())?;
            Ok::<Vec<u8>, String>(buffer)
        };
        let (stdout, stderr) = tokio::join!(stdout_task, stderr_task);
        attached.join().await.map_err(|error| error.to_string())?;
        Ok::<PodExecResponse, String>(PodExecResponse {
            stdout: String::from_utf8_lossy(&stdout?).into_owned(),
            stderr: String::from_utf8_lossy(&stderr?).into_owned(),
        })
    })
    .await
    .map_err(|_| "The command did not finish within 30 seconds".to_string())??;
    Ok(response)
}

fn command_basename(command: &str) -> &str {
    Path::new(command)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(command)
}

fn command_is(command: &str, expected: &str) -> bool {
    command_basename(command).eq_ignore_ascii_case(expected)
}

fn is_kubectl_shorthand(command: &str) -> bool {
    matches!(
        command,
        "alpha"
            | "annotate"
            | "api-resources"
            | "api-versions"
            | "apply"
            | "attach"
            | "auth"
            | "autoscale"
            | "certificate"
            | "cluster-info"
            | "completion"
            | "config"
            | "cordon"
            | "cp"
            | "create"
            | "debug"
            | "delete"
            | "describe"
            | "diff"
            | "drain"
            | "edit"
            | "events"
            | "exec"
            | "explain"
            | "expose"
            | "get"
            | "kustomize"
            | "label"
            | "logs"
            | "patch"
            | "plugin"
            | "port-forward"
            | "proxy"
            | "replace"
            | "rollout"
            | "run"
            | "scale"
            | "set"
            | "taint"
            | "top"
            | "uncordon"
            | "version"
            | "wait"
    )
}

fn command_has_context_flag(tokens: &[String]) -> bool {
    tokens.iter().any(|token| {
        matches!(
            token.as_str(),
            "--context" | "--kube-context" | "--kubeconfig"
        ) || token.starts_with("--context=")
            || token.starts_with("--kube-context=")
            || token.starts_with("--kubeconfig=")
    })
}

fn cli_tokens(command: &str) -> Result<Vec<String>, String> {
    shell_words::split(command).map_err(|error| format!("Could not parse the command: {error}"))
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn cluster_shell_command(command: &str, request: &KubeCliRequest) -> String {
    let mut kubectl_flags = Vec::new();
    let mut helm_flags = Vec::new();
    if let Some(path) = request
        .kubeconfig_path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
    {
        let path = shell_quote(path.trim());
        kubectl_flags.extend(["--kubeconfig".to_string(), path.clone()]);
        helm_flags.extend(["--kubeconfig".to_string(), path]);
    }
    let context = shell_quote(request.context.trim());
    kubectl_flags.extend(["--context".to_string(), context.clone()]);
    helm_flags.extend(["--kube-context".to_string(), context]);
    if let Some(namespace) = request
        .namespace
        .as_deref()
        .filter(|namespace| !namespace.trim().is_empty())
    {
        let namespace = shell_quote(namespace.trim());
        kubectl_flags.extend(["--namespace".to_string(), namespace.clone()]);
        helm_flags.extend(["--namespace".to_string(), namespace]);
    }
    format!(
        "kubectl() {{ command kubectl {} \"$@\"; }}\nhelm() {{ command helm {} \"$@\"; }}\n{}",
        kubectl_flags.join(" "),
        helm_flags.join(" "),
        command
    )
}

/// Builds the process for a CLI command, scoped to the active kubeconfig, context, and
/// namespace. Returns the process and the tool name used in error messages.
fn prepare_cluster_process(request: &KubeCliRequest) -> Result<(TokioCommand, String), String> {
    let command = request.command.trim();
    if command.is_empty() {
        return Err("Enter a command, for example: kubectl get pods or helm list".to_string());
    }
    if request.context.trim().is_empty() {
        return Err("Select a cluster context before running the CLI".to_string());
    }
    if command.len() > 4_096 {
        return Err("CLI commands are limited to 4,096 characters".to_string());
    }
    let mut tokens = cli_tokens(command)?;
    if tokens.is_empty() {
        return Err("Enter a command, for example: kubectl get pods or helm list".to_string());
    }
    if tokens.len() > 128 {
        return Err("CLI commands are limited to 128 arguments".to_string());
    }

    // Keep the old shorthand useful: entering `get pods` still means
    // `kubectl get pods`, while a complete executable such as `helm` or
    // `kustomize` is passed through unchanged.
    let first_token_is_tool = command_is(&tokens[0], "kubectl")
        || command_is(&tokens[0], "helm")
        || tokens[0].contains('/')
        || tokens[0].contains('\\');
    if !first_token_is_tool && is_kubectl_shorthand(&tokens[0]) {
        tokens.insert(0, "kubectl".to_string());
    }

    configure_desktop_exec_path();

    let tool = tokens[0].clone();
    let tool_is_kubectl = command_is(&tool, "kubectl");
    let tool_is_helm = command_is(&tool, "helm");
    if request.shell && (tool_is_kubectl || tool_is_helm) && command_has_context_flag(&tokens[1..])
    {
        return Err("Kuberniva supplies the active kubeconfig and context automatically, including shell mode. Remove --context/--kube-context/--kubeconfig from the command; -n and -A still work.".to_string());
    }
    let mut process;
    if request.shell {
        // Shell mode runs the text as typed, so apply the `get pods` shorthand there too.
        let shell_input = if !first_token_is_tool && is_kubectl_shorthand(&cli_tokens(command)?[0]) {
            format!("kubectl {command}")
        } else {
            command.to_string()
        };
        let shell_command = cluster_shell_command(&shell_input, request);
        let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        process = TokioCommand::new(shell);
        process.args(["-lc", &shell_command]);
    } else {
        let mut args = tokens[1..].to_vec();
        if tool_is_kubectl || tool_is_helm {
            if command_has_context_flag(&args) {
                return Err("Kuberniva supplies the active kubeconfig and context automatically. Remove --context/--kube-context/--kubeconfig from the command; -n and -A still work.".to_string());
            }
            let mut injected = Vec::with_capacity(args.len() + 7);
            if let Some(path) = request
                .kubeconfig_path
                .as_deref()
                .filter(|path| !path.trim().is_empty())
            {
                injected.extend(["--kubeconfig".to_string(), path.to_string()]);
            }
            if tool_is_kubectl {
                injected.extend(["--context".to_string(), request.context.trim().to_string()]);
            } else {
                injected.extend([
                    "--kube-context".to_string(),
                    request.context.trim().to_string(),
                ]);
            }
            if let Some(namespace) = request
                .namespace
                .as_deref()
                .filter(|namespace| !namespace.trim().is_empty())
            {
                injected.extend(["--namespace".to_string(), namespace.trim().to_string()]);
            }
            injected.append(&mut args);
            args = injected;
        }
        process = TokioCommand::new(&tool);
        process.args(args);
    }
    if let Some(path) = request
        .kubeconfig_path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
    {
        process.env("KUBECONFIG", path);
        process.env("KUBERNIVA_KUBECONFIG", path);
    } else {
        process.env_remove("KUBECONFIG");
        process.env_remove("KUBERNIVA_KUBECONFIG");
    }
    process.env("KUBERNIVA_CONTEXT", request.context.trim());
    process.env(
        "KUBERNIVA_NAMESPACE",
        request.namespace.as_deref().unwrap_or(""),
    );

    Ok((process, tool))
}

fn cli_run_registry() -> &'static Mutex<HashMap<String, oneshot::Sender<()>>> {
    CLI_RUN_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Forwards each line of a CLI pipe to the batching channel.
fn forward_cli_stream<R>(reader: R, stream: &'static str, sender: mpsc::UnboundedSender<CliChunk>)
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tauri::async_runtime::spawn(async move {
        let mut reader = BufReader::new(reader);
        let mut line = Vec::new();
        loop {
            line.clear();
            match reader.read_until(b'\n', &mut line).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let text = String::from_utf8_lossy(&line).into_owned();
                    if sender.send(CliChunk { stream, text }).is_err() {
                        break;
                    }
                }
            }
        }
    });
}

/// Starts a CLI command and streams its output as `kuberniva://cli-output` events, so
/// long-running commands such as `logs -f` or `get -w` show output immediately and can
/// be cancelled. Completion is reported once as `kuberniva://cli-exit`.
#[tauri::command]
async fn start_cluster_command<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    request: KubeCliRequest,
    run_id: String,
) -> Result<String, String> {
    // The frontend chooses the run id so it can match output that arrives before this
    // call returns.
    let run_id = run_id.trim().to_string();
    if run_id.is_empty() || run_id.len() > 128 {
        return Err("A valid CLI run id is required".to_string());
    }
    let (mut process, tool) = prepare_cluster_process(&request)?;
    process
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    // A dedicated process group lets Cancel stop the shell and everything it started.
    #[cfg(unix)]
    process.process_group(0);
    let mut child = process.spawn().map_err(|error| {
        format!("Could not start `{tool}`: {error}. Make sure the command is installed and available on PATH.")
    })?;
    let (cancel_sender, cancel_receiver) = oneshot::channel();
    cli_run_registry()
        .lock()
        .map_err(|_| "The CLI registry is unavailable".to_string())?
        .insert(run_id.clone(), cancel_sender);

    let (chunk_sender, mut chunk_receiver) = mpsc::unbounded_channel::<CliChunk>();
    if let Some(stdout) = child.stdout.take() {
        forward_cli_stream(stdout, "stdout", chunk_sender.clone());
    }
    if let Some(stderr) = child.stderr.take() {
        forward_cli_stream(stderr, "stderr", chunk_sender.clone());
    }
    drop(chunk_sender);

    // Batch lines every 50 ms so fast output does not flood the webview with events.
    let pump_app = app.clone();
    let pump_run_id = run_id.clone();
    let pump = tauri::async_runtime::spawn(async move {
        let mut batch = Vec::new();
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(50));
        loop {
            tokio::select! {
                chunk = chunk_receiver.recv() => match chunk {
                    Some(chunk) => {
                        batch.push(chunk);
                        if batch.len() >= 500 {
                            let _ = pump_app.emit("kuberniva://cli-output", CliOutputEvent { run_id: pump_run_id.clone(), chunks: std::mem::take(&mut batch) });
                        }
                    }
                    None => break,
                },
                _ = interval.tick() => {
                    if !batch.is_empty() {
                        let _ = pump_app.emit("kuberniva://cli-output", CliOutputEvent { run_id: pump_run_id.clone(), chunks: std::mem::take(&mut batch) });
                    }
                }
            }
        }
        if !batch.is_empty() {
            let _ = pump_app.emit("kuberniva://cli-output", CliOutputEvent { run_id: pump_run_id, chunks: batch });
        }
    });

    let exit_run_id = run_id.clone();
    tauri::async_runtime::spawn(async move {
        let pid = child.id();
        let (status, cancelled) = tokio::select! {
            status = child.wait() => (Some(status), false),
            _ = cancel_receiver => {
                #[cfg(unix)]
                if let Some(pid) = pid {
                    let _ = std::process::Command::new("kill").args(["-TERM", &format!("-{pid}")]).status();
                }
                let status = match tokio::time::timeout(std::time::Duration::from_secs(2), child.wait()).await {
                    Ok(status) => status,
                    Err(_) => {
                        let _ = child.kill().await;
                        child.wait().await
                    }
                };
                (Some(status), true)
            }
        };
        // Output from the final moments is flushed before the exit event.
        let _ = tokio::time::timeout(std::time::Duration::from_secs(2), pump).await;
        if let Ok(mut registry) = cli_run_registry().lock() {
            registry.remove(&exit_run_id);
        }
        let (exit_code, success, error) = match status {
            Some(Ok(status)) => (status.code(), status.success() && !cancelled, None),
            Some(Err(error)) => (None, false, Some(error.to_string())),
            None => (None, false, None),
        };
        let _ = app.emit(
            "kuberniva://cli-exit",
            CliExitEvent { run_id: exit_run_id, exit_code, success, cancelled, error },
        );
    });
    Ok(run_id)
}

#[tauri::command]
fn cancel_cluster_command(run_id: String) -> Result<(), String> {
    let sender = cli_run_registry()
        .lock()
        .map_err(|_| "The CLI registry is unavailable".to_string())?
        .remove(&run_id);
    if let Some(sender) = sender {
        let _ = sender.send(());
    }
    Ok(())
}

#[tauri::command]
async fn run_cluster_command(request: KubeCliRequest) -> Result<KubeCliResponse, String> {
    let (mut process, tool) = prepare_cluster_process(&request)?;
    let output = tokio::time::timeout(std::time::Duration::from_secs(120), process.output())
    .await
    .map_err(|_| "The command did not finish within 120 seconds".to_string())?
    .map_err(|error| format!("Could not start `{tool}`: {error}. Make sure the command is installed and available on PATH."))?;
    Ok(KubeCliResponse {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        exit_code: output.status.code(),
        success: output.status.success(),
    })
}

#[tauri::command]
async fn run_kubectl_command(request: KubeCliRequest) -> Result<KubeCliResponse, String> {
    run_cluster_command(request).await
}

#[tauri::command]
async fn start_port_forward(request: StartPortForwardRequest) -> Result<PortForwardInfo, String> {
    if request.remote_port == 0 {
        return Err("Choose a remote port between 1 and 65535".to_string());
    }
    let requested_local_port = request.local_port.unwrap_or(request.remote_port);
    if requested_local_port == 0 {
        return Err("Choose a local port between 1 and 65535".to_string());
    }

    // Resolve credentials before binding a local port so an OIDC/configuration error does not
    // leave a misleading listener behind.
    let context = request.context.clone();
    let client = client_for(request.kubeconfig_path, request.context).await?;
    ensure_resource_access(
        client.clone(),
        AccessReviewCheck {
            key: "pod-portforward".to_string(),
            group: String::new(),
            version: "v1".to_string(),
            resource: "pods".to_string(),
            verb: "create".to_string(),
            namespace: Some(request.namespace.clone()),
            name: Some(request.pod.clone()),
            subresource: Some("portforward".to_string()),
        },
    )
    .await?;
    let listener = TcpListener::bind(("127.0.0.1", requested_local_port))
        .await
        .map_err(|error| {
            format!(
                "Could not listen on 127.0.0.1:{requested_local_port}: {error}. Choose another local port."
            )
        })?;
    let local_port = listener
        .local_addr()
        .map_err(|error| format!("Could not read the local listener address: {error}"))?
        .port();
    let id = format!(
        "pf-{}",
        NEXT_PORT_FORWARD_ID.fetch_add(1, Ordering::Relaxed)
    );
    let info = PortForwardInfo {
        id: id.clone(),
        cluster_id: request.cluster_id.clone(),
        context,
        local_address: format!("127.0.0.1:{local_port}"),
        local_port,
        remote_port: request.remote_port,
        namespace: request.namespace.clone(),
        pod: request.pod.clone(),
    };
    let (shutdown_sender, mut shutdown_receiver) = oneshot::channel();
    let (stopped_sender, stopped_receiver) = oneshot::channel();
    PORT_FORWARD_REGISTRY
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| "The port-forward registry is unavailable".to_string())?
        .insert(
            id.clone(),
            PortForwardRuntime {
                info: info.clone(),
                shutdown: shutdown_sender,
                stopped: stopped_receiver,
            },
        );

    let task_id = id.clone();
    let namespace = request.namespace.clone();
    let pod = request.pod.clone();
    let remote_port = request.remote_port;
    tauri::async_runtime::spawn(async move {
        let mut connections = Vec::new();
        loop {
            tokio::select! {
                _ = &mut shutdown_receiver => break,
                accepted = listener.accept() => match accepted {
                    Ok((mut local_stream, _)) => {
                        let pod_client = client.clone();
                        let pod_namespace = namespace.clone();
                        let pod_name = pod.clone();
                        connections.retain(|connection: &tokio::task::JoinHandle<()>| !connection.is_finished());
                        connections.push(tokio::spawn(async move {
                            let pods = Api::<Pod>::namespaced(pod_client, &pod_namespace);
                            match pods.portforward(&pod_name, &[remote_port]).await {
                                Ok(mut forwarder) => {
                                    if let Some(mut remote_stream) = forwarder.take_stream(remote_port) {
                                        let _ = copy_bidirectional(&mut local_stream, &mut remote_stream).await;
                                    }
                                    forwarder.abort();
                                }
                                Err(error) => log::warn!("Kuberniva port forward to {pod_namespace}/{pod_name}:{remote_port} failed: {error}"),
                            }
                        }));
                    }
                    Err(error) => {
                        log::warn!("Kuberniva port-forward listener stopped: {error}");
                        break;
                    }
                }
            }
        }
        for connection in connections {
            connection.abort();
            let _ = connection.await;
        }
        if let Some(registry) = PORT_FORWARD_REGISTRY.get() {
            if let Ok(mut forwards) = registry.lock() {
                forwards.remove(&task_id);
            }
        }
        let _ = stopped_sender.send(());
    });

    Ok(info)
}

#[tauri::command]
fn list_port_forwards() -> Result<Vec<PortForwardInfo>, String> {
    let mut forwards = PORT_FORWARD_REGISTRY
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| "The port-forward registry is unavailable".to_string())?
        .values()
        .map(|runtime| runtime.info.clone())
        .collect::<Vec<_>>();
    forwards.sort_by(|left, right| {
        left.context
            .cmp(&right.context)
            .then(left.namespace.cmp(&right.namespace))
            .then(left.pod.cmp(&right.pod))
            .then(left.local_port.cmp(&right.local_port))
    });
    Ok(forwards)
}

#[tauri::command]
async fn stop_port_forward(request: StopPortForwardRequest) -> Result<PortForwardInfo, String> {
    let runtime = PORT_FORWARD_REGISTRY
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| "The port-forward registry is unavailable".to_string())?
        .remove(&request.id)
        .ok_or_else(|| "This port forward is no longer active".to_string())?;
    let PortForwardRuntime {
        info,
        shutdown,
        stopped,
    } = runtime;
    let _ = shutdown.send(());
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), stopped).await;
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn cli_request(kubeconfig_path: &Path, namespace: &str, command: &str) -> KubeCliRequest {
        KubeCliRequest {
            kubeconfig_path: Some(kubeconfig_path.to_string_lossy().into_owned()),
            context: "cli".to_string(),
            namespace: Some(namespace.to_string()),
            command: command.to_string(),
            shell: true,
        }
    }

    fn cli_kubeconfig_file() -> Option<PathBuf> {
        configure_desktop_exec_path();
        if std::process::Command::new("kubectl").arg("version").arg("--client").output().is_err() {
            return None; // kubectl is not installed on this machine.
        }
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = env::temp_dir().join(format!("kuberniva-cli-test-{nanos}.yaml"));
        fs::write(&path, kubeconfig("cli")).unwrap();
        Some(path)
    }

    #[tokio::test]
    async fn cli_shorthand_runs_kubectl_in_the_selected_namespace() {
        let Some(path) = cli_kubeconfig_file() else { return };
        let response = run_cluster_command(cli_request(
            &path,
            "team-a",
            "create configmap probe --dry-run=client -o jsonpath={.metadata.namespace}",
        ))
        .await
        .unwrap();
        let _ = fs::remove_file(&path);
        assert!(response.success, "{}", response.stderr);
        assert_eq!(response.stdout.trim(), "team-a");
    }

    #[tokio::test]
    async fn cli_namespace_flag_overrides_the_selected_namespace() {
        let Some(path) = cli_kubeconfig_file() else { return };
        let response = run_cluster_command(cli_request(
            &path,
            "team-a",
            "kubectl create configmap probe -n team-b --dry-run=client -o jsonpath={.metadata.namespace}",
        ))
        .await
        .unwrap();
        let _ = fs::remove_file(&path);
        assert!(response.success, "{}", response.stderr);
        assert_eq!(response.stdout.trim(), "team-b");
    }

    #[tokio::test]
    async fn cli_streams_output_before_exit_and_can_be_cancelled() {
        use tauri::Listener;
        let app = tauri::test::mock_app();
        let handle = app.handle().clone();
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<(bool, String)>();
        let output_sender = sender.clone();
        handle.listen_any("kuberniva://cli-output", move |event| {
            let _ = output_sender.send((false, event.payload().to_string()));
        });
        handle.listen_any("kuberniva://cli-exit", move |event| {
            let _ = sender.send((true, event.payload().to_string()));
        });
        let request = KubeCliRequest {
            kubeconfig_path: None,
            context: "cli".to_string(),
            namespace: None,
            command: "echo first-line; sleep 30; echo never-printed".to_string(),
            shell: true,
        };
        let started = std::time::Instant::now();
        start_cluster_command(handle.clone(), request, "run-stream".to_string())
            .await
            .unwrap();

        let wait = std::time::Duration::from_secs(10);
        let (is_exit, first) = tokio::time::timeout(wait, receiver.recv()).await.unwrap().unwrap();
        assert!(!is_exit, "output should stream before the command exits: {first}");
        assert!(first.contains("first-line"), "{first}");

        cancel_cluster_command("run-stream".to_string()).unwrap();
        let exit = tokio::time::timeout(wait, async {
            loop {
                let (is_exit, payload) = receiver.recv().await.unwrap();
                assert!(!payload.contains("never-printed"));
                if is_exit {
                    return payload;
                }
            }
        })
        .await
        .unwrap();
        assert!(exit.contains("\"cancelled\":true"), "{exit}");
        assert!(started.elapsed() < std::time::Duration::from_secs(15));
    }

    #[test]
    fn cli_rejects_switching_the_context() {
        let request = KubeCliRequest {
            kubeconfig_path: None,
            context: "cli".to_string(),
            namespace: None,
            command: "kubectl get pods --context other".to_string(),
            shell: true,
        };
        assert!(prepare_cluster_process(&request).is_err());
    }

    fn kubeconfig(name: &str) -> String {
        format!(
            "apiVersion: v1\nkind: Config\nclusters:\n  - name: {name}\n    cluster:\n      server: https://{name}.example.invalid\nusers:\n  - name: {name}-user\n    user:\n      token: test-token\ncontexts:\n  - name: {name}\n    context:\n      cluster: {name}\n      user: {name}-user\ncurrent-context: {name}\n"
        )
    }

    fn exec_kubeconfig(command: &str) -> String {
        format!(
            "apiVersion: v1\nkind: Config\nclusters:\n  - name: oidc\n    cluster:\n      server: https://oidc.example.invalid\nusers:\n  - name: oidc-user\n    user:\n      exec:\n        apiVersion: client.authentication.k8s.io/v1beta1\n        command: {command}\n        installHint: Install the OIDC helper.\ncontexts:\n  - name: oidc\n    context:\n      cluster: oidc\n      user: oidc-user\ncurrent-context: oidc\n"
        )
    }

    fn multi_context_kubeconfig() -> &'static str {
        "apiVersion: v1\nkind: Config\nclusters:\n  - name: first\n    cluster:\n      server: https://first.example.invalid\n  - name: second\n    cluster:\n      server: https://second.example.invalid\nusers:\n  - name: first-user\n    user:\n      token: first-token\n  - name: second-user\n    user:\n      token: second-token\ncontexts:\n  - name: first\n    context:\n      cluster: first\n      user: first-user\n  - name: second\n    context:\n      cluster: second\n      user: second-user\ncurrent-context: first\n"
    }

    fn kubeconfig_with_references(cluster_fields: &str, user_fields: &str) -> String {
        format!(
            "apiVersion: v1\nkind: Config\nclusters:\n  - name: referenced\n    cluster:\n      server: https://referenced.example.invalid\n{cluster_fields}users:\n  - name: referenced-user\n    user:\n{user_fields}contexts:\n  - name: referenced\n    context:\n      cluster: referenced\n      user: referenced-user\ncurrent-context: referenced\n"
        )
    }

    fn temporary_directory(prefix: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock is after the epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("kuberniva-{prefix}-{suffix}"))
    }

    #[test]
    fn trims_optional_access_review_attributes() {
        assert_eq!(
            normalized_access_value(Some(" platform ".to_string())),
            Some("platform".to_string())
        );
        assert_eq!(normalized_access_value(Some("   ".to_string())), None);
        assert_eq!(normalized_access_value(None), None);
    }

    #[test]
    fn formats_actionable_authorization_denials() {
        let check = AccessReviewCheck {
            key: "logs".to_string(),
            group: String::new(),
            version: "v1".to_string(),
            resource: "pods".to_string(),
            verb: "get".to_string(),
            namespace: Some("platform".to_string()),
            name: Some("api-0".to_string()),
            subresource: Some("log".to_string()),
        };
        let decision = AccessReviewDecision {
            key: "logs".to_string(),
            allowed: false,
            denied: true,
            reason: Some("RBAC denied the request".to_string()),
            evaluation_error: None,
        };

        assert_eq!(
            access_denied_message(&check, &decision),
            "Kubernetes authorization did not allow get on pods/log in namespace platform: RBAC denied the request"
        );
    }

    #[test]
    fn rejects_empty_invalid_and_non_kubeconfig_pasted_yaml() {
        let directory = temporary_directory("paste-invalid");

        let empty = import_pasted_kubeconfig_into("  \n", &directory)
            .expect_err("empty content must be rejected");
        assert!(empty.contains("Paste a kubeconfig"));

        let invalid = import_pasted_kubeconfig_into("contexts: [", &directory)
            .expect_err("invalid YAML must be rejected");
        assert!(invalid.contains("Could not parse"));

        let wrong_document = import_pasted_kubeconfig_into(
            "apiVersion: v1\nkind: Pod\nmetadata:\n  name: not-a-config\n",
            &directory,
        )
        .expect_err("a Kubernetes resource must not be treated as a kubeconfig");
        assert!(wrong_document.contains("not a kubeconfig"));
        assert!(!directory.exists());
    }

    #[test]
    fn rejects_relative_file_references_in_pasted_kubeconfigs() {
        let cases = [
            (
                kubeconfig_with_references(
                    "      certificate-authority: ./certs/ca.crt\n",
                    "      token: test-token\n",
                ),
                "certificate-authority",
            ),
            (
                kubeconfig_with_references(
                    "      certificate-authority: ./certs/ca.crt\n      certificate-authority-data: \"\"\n",
                    "      token: test-token\n",
                ),
                "certificate-authority",
            ),
            (
                kubeconfig_with_references(
                    "",
                    "      client-certificate: certs/client.crt\n      token: test-token\n",
                ),
                "client-certificate",
            ),
            (
                kubeconfig_with_references(
                    "",
                    "      client-certificate: certs/client.crt\n      client-certificate-data: \"\"\n      token: test-token\n",
                ),
                "client-certificate",
            ),
            (
                kubeconfig_with_references(
                    "",
                    "      client-key: ../keys/client.key\n      token: test-token\n",
                ),
                "client-key",
            ),
            (
                kubeconfig_with_references("", "      tokenFile: ./tokens/cluster.token\n"),
                "tokenFile",
            ),
            (
                kubeconfig_with_references(
                    "",
                    "      exec:\n        apiVersion: client.authentication.k8s.io/v1beta1\n        command: ./bin/kubelogin\n",
                ),
                "relative exec command",
            ),
            (
                kubeconfig_with_references(
                    "",
                    "      auth-provider:\n        name: oidc\n        config:\n          idp-certificate-authority: ./certs/idp-ca.crt\n",
                ),
                "idp-certificate-authority",
            ),
        ];

        for (yaml, expected) in cases {
            let error = parse_pasted_kubeconfig(&yaml)
                .expect_err("relative file reference must be rejected");
            assert!(
                error.contains(expected),
                "expected `{expected}` in `{error}`"
            );
        }
    }

    #[test]
    fn allows_inline_overrides_and_bare_path_exec_commands() {
        let yaml = kubeconfig_with_references(
            "      certificate-authority: ./certs/ca.crt\n      certificate-authority-data: Y2E=\n",
            "      tokenFile: ./tokens/cluster.token\n      token: test-token\n      client-certificate: ./certs/client.crt\n      client-certificate-data: Y2VydA==\n      client-key: ./keys/client.key\n      client-key-data: a2V5\n      exec:\n        apiVersion: client.authentication.k8s.io/v1beta1\n        command: kubelogin\n      auth-provider:\n        name: oidc\n        config:\n          idp-certificate-authority: ./certs/idp-ca.crt\n          idp-certificate-authority-data: aWRwLWNh\n",
        );

        parse_pasted_kubeconfig(&yaml)
            .expect("inline values and a bare PATH command must remain portable");
    }

    #[cfg(unix)]
    #[test]
    fn allows_absolute_file_and_exec_references() {
        let yaml = kubeconfig_with_references(
            "      certificate-authority: /absolute/certs/ca.crt\n",
            "      tokenFile: /absolute/tokens/cluster.token\n      client-certificate: /absolute/certs/client.crt\n      client-key: /absolute/keys/client.key\n      exec:\n        apiVersion: client.authentication.k8s.io/v1beta1\n        command: /absolute/bin/kubelogin\n      auth-provider:\n        name: oidc\n        config:\n          idp-certificate-authority: /absolute/certs/idp-ca.crt\n",
        );

        parse_pasted_kubeconfig(&yaml).expect("absolute references must remain valid after import");
    }

    #[test]
    fn reuses_identical_pasted_kubeconfig_content() {
        let directory = temporary_directory("paste-reuse");
        let first = import_pasted_kubeconfig_into(&kubeconfig("cluster-a"), &directory)
            .expect("import kubeconfig");
        let second = import_pasted_kubeconfig_into(&kubeconfig("cluster-a"), &directory)
            .expect("reuse imported kubeconfig");
        let first_source = first.contexts[0]
            .source_path
            .as_deref()
            .expect("first source path");
        let second_source = second.contexts[0]
            .source_path
            .as_deref()
            .expect("second source path");

        assert_eq!(first.current_context.as_deref(), Some("cluster-a"));
        assert_eq!(first_source, second_source);
        assert!(Path::new(first_source).is_file());
        assert_eq!(
            fs::read_dir(&directory)
                .expect("read managed directory")
                .count(),
            1
        );
        fs::remove_dir_all(directory).expect("remove temporary directory");
    }

    #[test]
    fn ignores_hidden_import_staging_files_during_deduplication() {
        let directory = temporary_directory("paste-hidden-stage");
        ensure_managed_kubeconfig_directory(&directory).expect("create managed directory");
        let staged_path = directory.join(".kubeconfig-crash.yaml.kuberniva-import-123-0");
        fs::write(&staged_path, kubeconfig("cluster-staged")).expect("write staged kubeconfig");

        let summary = import_pasted_kubeconfig_into(&kubeconfig("cluster-staged"), &directory)
            .expect("import kubeconfig despite hidden staging file");
        let source_path = Path::new(
            summary.contexts[0]
                .source_path
                .as_deref()
                .expect("managed source path"),
        );

        assert_ne!(source_path, staged_path);
        assert!(source_path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("kubeconfig-") && name.ends_with(".yaml")));
        assert_eq!(
            fs::read_dir(&directory)
                .expect("read managed directory")
                .count(),
            2
        );
        fs::remove_dir_all(directory).expect("remove temporary directory");
    }

    #[cfg(unix)]
    #[test]
    fn secures_managed_pasted_kubeconfig_storage_on_unix() {
        use std::os::unix::fs::PermissionsExt;

        let directory = temporary_directory("paste-permissions");
        let summary = import_pasted_kubeconfig_into(&kubeconfig("cluster-secure"), &directory)
            .expect("import kubeconfig");
        let source_path = summary.contexts[0]
            .source_path
            .as_deref()
            .expect("source path");
        let directory_mode = fs::metadata(&directory)
            .expect("managed directory metadata")
            .permissions()
            .mode()
            & 0o777;
        let file_mode = fs::metadata(source_path)
            .expect("managed kubeconfig metadata")
            .permissions()
            .mode()
            & 0o777;

        assert_eq!(directory_mode, 0o700);
        assert_eq!(file_mode, 0o600);
        fs::remove_dir_all(directory).expect("remove temporary directory");
    }

    #[test]
    fn reimporting_identical_digest_owned_kubeconfig_reuses_it_in_place() {
        let directory = temporary_directory("paste-restore");
        let first = import_pasted_kubeconfig_into(multi_context_kubeconfig(), &directory)
            .expect("import multi-context kubeconfig");
        let original_source = first.contexts[0]
            .source_path
            .clone()
            .expect("managed source path");
        let restored = import_pasted_kubeconfig_into(multi_context_kubeconfig(), &directory)
            .expect("reimport original kubeconfig");
        let restored_source = restored.contexts[0]
            .source_path
            .as_deref()
            .expect("restored source path");
        let restored_names = restored
            .contexts
            .iter()
            .map(|context| context.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(restored_source, original_source);
        assert_eq!(restored_names, vec!["first", "second"]);
        assert_eq!(restored.current_context.as_deref(), Some("first"));
        assert_eq!(
            fs::read_dir(&directory)
                .expect("read managed directory")
                .count(),
            1
        );
        fs::remove_dir_all(directory).expect("remove temporary directory");
    }

    #[test]
    fn rejects_a_non_regular_digest_owned_destination() {
        let directory = temporary_directory("paste-non-regular");
        let (_, _, canonical) =
            parse_pasted_kubeconfig(&kubeconfig("cluster-owned")).expect("parse kubeconfig");
        ensure_managed_kubeconfig_directory(&directory).expect("create managed directory");
        let managed_path = managed_kubeconfig_path(&directory, &canonical);
        fs::create_dir(&managed_path).expect("create conflicting directory");

        let error = import_pasted_kubeconfig_into(&kubeconfig("cluster-owned"), &directory)
            .expect_err("non-regular destination must be rejected");
        assert!(error.contains("not a regular file"));
        fs::remove_dir_all(directory).expect("remove temporary directory");
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_symlinked_digest_owned_destination() {
        use std::os::unix::fs::symlink;

        let directory = temporary_directory("paste-symlink");
        let (_, _, canonical) =
            parse_pasted_kubeconfig(&kubeconfig("cluster-owned")).expect("parse kubeconfig");
        ensure_managed_kubeconfig_directory(&directory).expect("create managed directory");
        let managed_path = managed_kubeconfig_path(&directory, &canonical);
        let symlink_target = directory.join("unrelated.yaml");
        fs::write(&symlink_target, kubeconfig("unrelated")).expect("write symlink target");
        symlink(&symlink_target, &managed_path).expect("create conflicting symlink");

        let error = import_pasted_kubeconfig_into(&kubeconfig("cluster-owned"), &directory)
            .expect_err("symlinked destination must be rejected");
        assert!(error.contains("not a regular file"));
        assert_eq!(
            fs::read_to_string(&symlink_target).expect("read untouched target"),
            kubeconfig("unrelated")
        );
        fs::remove_dir_all(directory).expect("remove temporary directory");
    }

    #[test]
    fn reads_contexts_from_each_kubeconfig_in_a_directory() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock is after the epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("kuberniva-kubeconfigs-{suffix}"));
        fs::create_dir_all(&directory).expect("create temporary kubeconfig directory");
        fs::write(directory.join("cluster-a.yaml"), kubeconfig("cluster-a"))
            .expect("write first kubeconfig");
        fs::write(directory.join("cluster-b.yaml"), kubeconfig("cluster-b"))
            .expect("write second kubeconfig");
        fs::write(directory.join("not-a-kubeconfig"), "not valid yaml")
            .expect("write ignored file");

        let summary = read_kubeconfig_contexts(Some(directory.to_string_lossy().into_owned()))
            .expect("read kubeconfig directory");
        let names = summary
            .contexts
            .iter()
            .map(|context| context.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(names, vec!["cluster-a", "cluster-b"]);
        assert!(summary
            .contexts
            .iter()
            .all(|context| context.source_path.is_some()));
        fs::remove_dir_all(directory).expect("remove temporary kubeconfig directory");
    }

    #[test]
    fn expands_home_shorthand_and_home_relative_paths() {
        let home = std::env::var_os("HOME").expect("home directory is set");
        assert_eq!(
            resolve_local_path("~/clusters/production"),
            PathBuf::from(&home).join("clusters/production")
        );
        assert_eq!(
            resolve_local_path("clusters/production"),
            PathBuf::from(home).join("clusters/production")
        );
    }

    #[test]
    fn labels_crd_definitions_and_crd_backed_resources_in_the_catalog() {
        let (definitions_category, definitions_custom, definitions_crd) =
            category_for("apiextensions.k8s.io", "customresourcedefinitions");
        assert_eq!(definitions_category, "Custom Resources");
        assert!(!definitions_custom);
        assert!(definitions_crd);

        let (resource_category, resource_custom, resource_crd) =
            category_for("example.platform.io", "widgets");
        assert_eq!(resource_category, "Custom Resources");
        assert!(resource_custom);
        assert!(resource_crd);
    }

    #[test]
    fn gives_gateway_api_resources_their_own_catalog_category() {
        let (category, custom, crd) = category_for("gateway.networking.k8s.io", "httproutes");
        assert_eq!(category, "Gateway APIs");
        assert!(custom);
        assert!(crd);

        let (gateway_category, _, _) = category_for("gateway.networking.k8s.io", "gateways");
        assert_eq!(gateway_category, "Gateway APIs");
    }

    #[test]
    fn builds_label_selectors_from_labels_and_match_expressions() {
        let manifest = serde_json::json!({
            "spec": {
                "selector": {
                    "matchLabels": { "app": "api" },
                    "matchExpressions": [
                        { "key": "track", "operator": "In", "values": ["stable", "canary"] },
                        { "key": "debug", "operator": "DoesNotExist" }
                    ]
                }
            }
        });
        assert_eq!(
            workload_label_selector(&manifest).expect("valid selector"),
            "app=api,track in (stable,canary),!debug"
        );
    }

    #[test]
    fn calculates_cpu_and_memory_utilization_from_kubernetes_quantities() {
        let cpu = usage_percent(Some("250m"), Some("4"), true).expect("CPU percentage");
        let memory = usage_percent(Some("512Mi"), Some("2Gi"), false).expect("memory percentage");

        assert!((cpu - 6.25).abs() < f64::EPSILON);
        assert!((memory - 25.0).abs() < f64::EPSILON);
    }

    #[test]
    fn sums_cluster_capacity_and_usage_quantities() {
        let cpu = sum_quantities(vec![Some("4"), Some("2500m"), None], true)
            .expect("combined CPU capacity");
        let memory = sum_quantities(vec![Some("8Gi"), Some("512Mi")], false)
            .expect("combined memory capacity");

        assert!((cpu - 6.5).abs() < f64::EPSILON);
        assert_eq!(format_memory_usage(memory), "8.5Gi");
    }

    #[test]
    fn formats_subcore_cpu_as_decimal_cores() {
        assert_eq!(format_cpu_usage(0.001), "0.001 cores");
        assert_eq!(format_cpu_usage(0.002), "0.002 cores");
        assert_eq!(format_cpu_usage(0.1), "0.1 cores");
        assert_eq!(format_cpu_usage(1.25), "1.25 cores");
    }

    #[test]
    fn rounds_certificate_days_away_from_zero_for_partial_days() {
        assert_eq!(days_remaining_from_seconds(0), 0);
        assert_eq!(days_remaining_from_seconds(43_200), 1);
        assert_eq!(days_remaining_from_seconds(86_400), 1);
        assert_eq!(days_remaining_from_seconds(-43_200), -1);
        assert_eq!(days_remaining_from_seconds(-86_400), -1);
    }

    #[test]
    fn recognizes_workspace_flags_for_shell_commands() {
        assert!(command_has_context_flag(&[
            "--context=other".to_string(),
            "get".to_string(),
        ]));
        // Namespace flags are allowed: they override the selected namespace.
        assert!(!command_has_context_flag(&[
            "-n".to_string(),
            "other".to_string()
        ]));
        assert!(command_has_context_flag(&[
            "--kube-context=other".to_string()
        ]));
        assert!(!command_has_context_flag(&[
            "get".to_string(),
            "pods".to_string()
        ]));
    }

    #[test]
    fn shell_mode_wraps_kubectl_and_helm_with_the_active_workspace() {
        let request = KubeCliRequest {
            kubeconfig_path: Some("/Users/example/cluster config.yaml".to_string()),
            context: "production-context".to_string(),
            namespace: Some("payments".to_string()),
            command: "kubectl get pods | grep Running && helm list".to_string(),
            shell: true,
        };

        let script = cluster_shell_command(&request.command, &request);

        assert!(script.contains("kubectl() { command kubectl --kubeconfig '/Users/example/cluster config.yaml' --context 'production-context' --namespace 'payments' \"$@\"; }"));
        assert!(script.contains("helm() { command helm --kubeconfig '/Users/example/cluster config.yaml' --kube-context 'production-context' --namespace 'payments' \"$@\"; }"));
        assert!(script.ends_with("kubectl get pods | grep Running && helm list"));
    }

    #[test]
    fn shell_mode_omits_namespace_when_all_namespaces_is_selected() {
        let request = KubeCliRequest {
            kubeconfig_path: None,
            context: "development".to_string(),
            namespace: None,
            command: "kubectl get pods --all-namespaces".to_string(),
            shell: true,
        };

        let script = cluster_shell_command(&request.command, &request);

        assert!(!script.contains("--namespace"));
        assert!(script.contains("--context 'development'"));
        assert!(script.contains("--kube-context 'development'"));
    }

    #[test]
    fn writes_the_visible_log_snapshot_exactly() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock is after the epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("kuberniva-log-{suffix}.log"));
        let content = "2026-08-12T20:30:00Z first line\n2026-08-12T20:30:01Z second line\n";

        write_log_snapshot(&path, content).expect("write log snapshot");

        assert_eq!(
            fs::read_to_string(&path).expect("read log snapshot"),
            content
        );
        fs::remove_file(path).expect("remove log snapshot");
    }

    #[test]
    fn reports_a_missing_oidc_exec_helper_with_an_actionable_hint() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock is after the epoch")
            .as_nanos();
        let kubeconfig_path = std::env::temp_dir().join(format!("kuberniva-oidc-{suffix}.yaml"));
        fs::write(
            &kubeconfig_path,
            exec_kubeconfig("kuberniva-helper-that-does-not-exist"),
        )
        .expect("write exec kubeconfig");
        let kubeconfig = Kubeconfig::read_from(&kubeconfig_path).expect("read exec kubeconfig");

        let error = validate_exec_command(&kubeconfig, Some("oidc"))
            .expect_err("missing helper should be reported");

        assert!(error.contains("kuberniva-helper-that-does-not-exist"));
        assert!(error.contains("Install the OIDC helper."));
        fs::remove_file(kubeconfig_path).expect("remove temporary kubeconfig");
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        // The window starts hidden so it never shows an unstyled or unzoomed frame. The
        // frontend reveals it after its first paint; this is only a safety net.
        .on_page_load(|webview, payload| {
            if payload.event() == tauri::webview::PageLoadEvent::Finished {
                let window = webview.window();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
                    let _ = window.show();
                });
            }
        })
        .invoke_handler(tauri::generate_handler![
            import_pasted_kubeconfig,
            read_kubeconfig_contexts,
            invalidate_cluster_client,
            discover_cluster_catalog,
            read_cluster_overview,
            read_cluster_events,
            start_resource_watch,
            stop_resource_watch,
            check_resource_permissions,
            list_resource_objects,
            get_resource_detail,
            delete_resource_object,
            save_resource_detail,
            save_resource_yaml,
            list_workload_pods,
            read_pod_logs,
            save_log_file,
            get_pod_runtime,
            exec_pod_command,
            run_cluster_command,
            run_kubectl_command,
            start_cluster_command,
            cancel_cluster_command,
            start_port_forward,
            list_port_forwards,
            stop_port_forward
        ])
        .run(tauri::generate_context!())
        .expect("error while running Kuberniva");
}
