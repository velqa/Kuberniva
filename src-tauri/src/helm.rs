//! Helm releases read straight from Helm's storage: Secrets of type `helm.sh/release.v1`
//! labelled `owner=helm`, whose `release` field is base64 of gzip-compressed JSON.

use super::*;
use std::collections::BTreeMap;
use std::io::Read;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct HelmRequest {
    kubeconfig_path: Option<String>,
    context: Option<String>,
    /// A namespace, or "all namespaces".
    namespace: Option<String>,
    name: Option<String>,
    revision: Option<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(super) struct HelmRelease {
    name: String,
    namespace: String,
    revision: i64,
    status: String,
    chart: String,
    chart_version: String,
    app_version: String,
    updated: String,
    description: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct HelmReleaseDetail {
    release: HelmRelease,
    notes: String,
    /// User-supplied values (what `helm get values` shows), as YAML.
    values: String,
    manifest: String,
    chart_description: String,
    history: Vec<HelmRelease>,
}

/// Decodes Helm's `release` payload: base64 → (gzip) → JSON.
pub(super) fn decode_release(payload: &[u8]) -> Result<Value, String> {
    let text =
        std::str::from_utf8(payload).map_err(|_| "Helm release data is not text".to_string())?;
    let compressed = STANDARD
        .decode(text.trim())
        .map_err(|error| format!("Helm release data is not base64: {error}"))?;
    let json = if compressed.starts_with(&[0x1f, 0x8b]) {
        let mut decoder = flate2::read::GzDecoder::new(compressed.as_slice());
        let mut buffer = Vec::new();
        decoder
            .read_to_end(&mut buffer)
            .map_err(|error| format!("Helm release data could not be decompressed: {error}"))?;
        buffer
    } else {
        compressed
    };
    serde_json::from_slice(&json).map_err(|error| format!("Helm release data is not JSON: {error}"))
}

pub(super) fn release_summary(release: &Value) -> HelmRelease {
    let text = |value: &Value| value.as_str().unwrap_or_default().to_string();
    let metadata = &release["chart"]["metadata"];
    HelmRelease {
        name: text(&release["name"]),
        namespace: text(&release["namespace"]),
        revision: release["version"].as_i64().unwrap_or_default(),
        status: text(&release["info"]["status"]),
        chart: text(&metadata["name"]),
        chart_version: text(&metadata["version"]),
        app_version: text(&metadata["appVersion"]),
        updated: text(&release["info"]["last_deployed"]),
        description: text(&release["info"]["description"]),
    }
}

fn secrets_api(client: Client, namespace: Option<&str>) -> Api<k8s_openapi::api::core::v1::Secret> {
    match namespace.filter(|scope| !scope.is_empty() && *scope != "all namespaces") {
        Some(namespace) => Api::namespaced(client, namespace),
        None => Api::all(client),
    }
}

async fn release_secrets(
    client: Client,
    namespace: Option<&str>,
    name: Option<&str>,
) -> Result<Vec<Value>, String> {
    let selector = match name {
        Some(name) => format!("owner=helm,name={name}"),
        None => "owner=helm".to_string(),
    };
    let secrets = secrets_api(client, namespace)
        .list(&ListParams::default().labels(&selector))
        .await
        .map_err(|error| {
            let message = error.to_string();
            if message.contains("403") || message.to_ascii_lowercase().contains("forbidden") {
                "Helm keeps releases in Secrets; your access does not allow listing Secrets here"
                    .to_string()
            } else {
                message
            }
        })?;
    Ok(secrets
        .items
        .into_iter()
        .filter(|secret| secret.type_.as_deref() == Some("helm.sh/release.v1"))
        .filter_map(|secret| {
            secret
                .data?
                .get("release")
                .and_then(|payload| decode_release(&payload.0).ok())
        })
        .collect())
}

/// The newest revision of every release.
pub(super) fn latest_releases(releases: &[Value]) -> Vec<HelmRelease> {
    let mut latest: BTreeMap<(String, String), HelmRelease> = BTreeMap::new();
    for release in releases.iter().map(release_summary) {
        let key = (release.namespace.clone(), release.name.clone());
        if latest
            .get(&key)
            .is_none_or(|current| release.revision > current.revision)
        {
            latest.insert(key, release);
        }
    }
    latest.into_values().collect()
}

pub(super) async fn list_releases(request: HelmRequest) -> Result<Vec<HelmRelease>, String> {
    let client = client_for(request.kubeconfig_path, request.context).await?;
    let releases = release_secrets(client, request.namespace.as_deref(), None).await?;
    Ok(latest_releases(&releases))
}

pub(super) async fn release_detail(request: HelmRequest) -> Result<HelmReleaseDetail, String> {
    let name = request
        .name
        .clone()
        .ok_or_else(|| "Choose a Helm release".to_string())?;
    let client = client_for(request.kubeconfig_path, request.context).await?;
    let releases = release_secrets(client, request.namespace.as_deref(), Some(&name)).await?;
    let mut history: Vec<HelmRelease> = releases.iter().map(release_summary).collect();
    history.sort_by(|left, right| right.revision.cmp(&left.revision));
    let wanted = request.revision.unwrap_or_else(|| {
        history
            .first()
            .map(|release| release.revision)
            .unwrap_or_default()
    });
    let release = releases
        .iter()
        .find(|release| release["version"].as_i64() == Some(wanted))
        .ok_or_else(|| format!("Revision {wanted} of {name} was not found"))?;
    let config = &release["config"];
    let values = if config.is_null() || config.as_object().is_some_and(|map| map.is_empty()) {
        String::new()
    } else {
        serde_yaml::to_string(config).unwrap_or_default()
    };
    Ok(HelmReleaseDetail {
        release: release_summary(release),
        notes: release["info"]["notes"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        values,
        manifest: release["manifest"].as_str().unwrap_or_default().to_string(),
        chart_description: release["chart"]["metadata"]["description"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        history,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn encode(release: &Value) -> Vec<u8> {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(release.to_string().as_bytes()).unwrap();
        STANDARD.encode(encoder.finish().unwrap()).into_bytes()
    }

    #[test]
    fn helm_payloads_decode_and_keep_the_newest_revision() {
        let release = |revision: i64, status: &str| {
            serde_json::json!({
                "name": "shop", "namespace": "web", "version": revision,
                "info": { "status": status, "last_deployed": "2026-10-01T10:00:00Z", "notes": "Visit http://shop", "description": "Upgrade complete" },
                "chart": { "metadata": { "name": "shop", "version": format!("1.{revision}.0"), "appVersion": "3.2.0", "description": "Storefront" } },
                "config": { "replicaCount": 3 },
                "manifest": "---\nkind: Deployment"
            })
        };
        let decoded = decode_release(&encode(&release(2, "deployed"))).unwrap();
        assert_eq!(decoded["chart"]["metadata"]["version"], "1.2.0");
        let summary = release_summary(&decoded);
        assert_eq!(
            (
                summary.revision,
                summary.status.as_str(),
                summary.chart_version.as_str()
            ),
            (2, "deployed", "1.2.0")
        );
        let latest = latest_releases(&[
            release(1, "superseded"),
            release(3, "failed"),
            release(2, "superseded"),
        ]);
        assert_eq!(latest.len(), 1);
        assert_eq!(
            (latest[0].revision, latest[0].status.as_str()),
            (3, "failed")
        );
        // Uncompressed payloads (older Helm 3 betas) still decode.
        let plain = STANDARD
            .encode(release(1, "deployed").to_string())
            .into_bytes();
        assert_eq!(decode_release(&plain).unwrap()["version"], 1);
        assert!(decode_release(b"not base64!").is_err());
    }
}
