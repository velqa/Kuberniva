//! Cluster topology: what routes to what, what selects what, what owns what, and what each
//! workload mounts. Gateways and Ingresses → routes → Services → workloads → ReplicaSets/Jobs
//! → Pods, plus the ConfigMaps, Secrets, and PersistentVolumeClaims workloads use.
//! Secrets are read as metadata only, so their values never leave the cluster.

use super::*;
use k8s_openapi::api::apps::v1::{DaemonSet, Deployment, ReplicaSet, StatefulSet};
use k8s_openapi::api::batch::v1::{CronJob, Job};
use k8s_openapi::api::core::v1::{ConfigMap, PersistentVolumeClaim, PodSpec, Secret, Service};
use k8s_openapi::api::networking::v1::Ingress;
use kube::api::DynamicObject;
use serde::de::DeserializeOwned;
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TopologyRequest {
    pub(super) kubeconfig_path: Option<String>,
    pub(super) context: Option<String>,
    /// A namespace, or "all namespaces".
    namespace: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(super) struct TopologyNode {
    id: String,
    kind: String,
    group: String,
    name: String,
    namespace: String,
    health: String,
    info: String,
    /// The controlling owner (Deployment for a ReplicaSet, ReplicaSet for a Pod).
    owner: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(super) struct TopologyEdge {
    from: String,
    to: String,
    /// "routes", "selects", "owns", or "uses".
    relation: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub(super) struct Topology {
    nodes: Vec<TopologyNode>,
    edges: Vec<TopologyEdge>,
    /// Object types that could not be read, such as Secrets without list permission.
    warnings: Vec<String>,
}

fn scoped<K>(client: &Client, namespace: Option<&str>) -> Api<K>
where
    K: kube::Resource<Scope = k8s_openapi::NamespaceResourceScope>,
    <K as kube::Resource>::DynamicType: Default,
{
    match namespace {
        Some(namespace) => Api::namespaced(client.clone(), namespace),
        None => Api::all(client.clone()),
    }
}

fn dynamic_api(
    client: &Client,
    namespace: Option<&str>,
    group: &str,
    version: &str,
    kind: &str,
    plural: &str,
) -> Api<DynamicObject> {
    let resource = api_resource(group.into(), version.into(), kind.into(), plural.into());
    match namespace {
        Some(namespace) => Api::namespaced_with(client.clone(), namespace, &resource),
        None => Api::all_with(client.clone(), &resource),
    }
}

fn reason(error: &kube::Error) -> String {
    match error {
        kube::Error::Api(response) if response.code == 403 => "no permission to list them".into(),
        other => other.to_string(),
    }
}

async fn list<K>(api: Api<K>, label: &str, warnings: &mut Vec<String>) -> Option<Vec<K>>
where
    K: Clone + DeserializeOwned + std::fmt::Debug,
{
    match api.list(&ListParams::default()).await {
        Ok(list) => Some(list.items),
        Err(error) => {
            warnings.push(format!("{label}: {}", reason(&error)));
            None
        }
    }
}

/// Optional APIs (Gateway API) are skipped quietly when the cluster does not serve them.
async fn list_optional(api: Api<DynamicObject>) -> Vec<DynamicObject> {
    api.list(&ListParams::default())
        .await
        .map(|list| list.items)
        .unwrap_or_default()
}

/// Names only: Secret values are never read.
async fn list_names<K>(
    api: Api<K>,
    label: &str,
    warnings: &mut Vec<String>,
) -> Option<HashSet<String>>
where
    K: Clone + DeserializeOwned + std::fmt::Debug + kube::Resource,
{
    match api.list_metadata(&ListParams::default()).await {
        Ok(list) => Some(
            list.items
                .into_iter()
                .filter_map(|item| {
                    Some(format!(
                        "{}/{}",
                        item.metadata.namespace?, item.metadata.name?
                    ))
                })
                .collect(),
        ),
        Err(error) => {
            warnings.push(format!("{label}: {}", reason(&error)));
            None
        }
    }
}

fn meta_ns(meta: &k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta) -> String {
    meta.namespace.clone().unwrap_or_default()
}

fn meta_name(meta: &k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta) -> String {
    meta.name.clone().unwrap_or_default()
}

/// ConfigMaps, Secrets, and claims a pod template references, from volumes and env.
pub(super) fn template_refs(spec: &PodSpec) -> BTreeSet<(&'static str, String)> {
    let mut refs = BTreeSet::new();
    for volume in spec.volumes.iter().flatten() {
        if let Some(name) = volume.config_map.as_ref().map(|source| source.name.clone()) {
            refs.insert(("ConfigMap", name));
        }
        if let Some(name) = volume
            .secret
            .as_ref()
            .and_then(|source| source.secret_name.clone())
        {
            refs.insert(("Secret", name));
        }
        if let Some(claim) = &volume.persistent_volume_claim {
            refs.insert(("PersistentVolumeClaim", claim.claim_name.clone()));
        }
        for source in volume
            .projected
            .iter()
            .flat_map(|projected| projected.sources.iter().flatten())
        {
            if let Some(name) = source.config_map.as_ref().map(|source| source.name.clone()) {
                refs.insert(("ConfigMap", name));
            }
            if let Some(name) = source.secret.as_ref().map(|source| source.name.clone()) {
                refs.insert(("Secret", name));
            }
        }
    }
    for container in spec
        .containers
        .iter()
        .chain(spec.init_containers.iter().flatten())
    {
        for source in container.env_from.iter().flatten() {
            if let Some(name) = source
                .config_map_ref
                .as_ref()
                .map(|source| source.name.clone())
            {
                refs.insert(("ConfigMap", name));
            }
            if let Some(name) = source.secret_ref.as_ref().map(|source| source.name.clone()) {
                refs.insert(("Secret", name));
            }
        }
        for value in container
            .env
            .iter()
            .flatten()
            .filter_map(|env| env.value_from.as_ref())
        {
            if let Some(name) = value
                .config_map_key_ref
                .as_ref()
                .map(|source| source.name.clone())
            {
                refs.insert(("ConfigMap", name));
            }
            if let Some(name) = value
                .secret_key_ref
                .as_ref()
                .map(|source| source.name.clone())
            {
                refs.insert(("Secret", name));
            }
        }
    }
    for secret in spec.image_pull_secrets.iter().flatten() {
        refs.insert(("Secret", secret.name.clone()));
    }
    refs.retain(|(_, name)| !name.is_empty());
    refs
}

fn replicas_health(desired: i32, ready: i32) -> &'static str {
    if desired == 0 || ready >= desired {
        "Healthy"
    } else if ready == 0 {
        "Degraded"
    } else {
        "Progressing"
    }
}

#[derive(Default)]
struct Builder {
    nodes: BTreeMap<String, TopologyNode>,
    edges: BTreeSet<(String, String, &'static str)>,
}

impl Builder {
    fn node(
        &mut self,
        kind: &str,
        group: &str,
        namespace: &str,
        name: &str,
        health: &str,
        info: String,
        owner: Option<String>,
    ) -> String {
        let id = tree_id(kind, namespace, name);
        self.nodes.insert(
            id.clone(),
            TopologyNode {
                id: id.clone(),
                kind: kind.into(),
                group: group.into(),
                name: name.into(),
                namespace: namespace.into(),
                health: health.into(),
                info,
                owner,
            },
        );
        id
    }

    fn edge(&mut self, from: &str, to: &str, relation: &'static str) {
        if from != to {
            self.edges
                .insert((from.to_string(), to.to_string(), relation));
        }
    }

    /// A referenced object. If it was not found among listed objects, it is shown as missing.
    fn reference(
        &mut self,
        kind: &str,
        group: &str,
        namespace: &str,
        name: &str,
        known: Option<&HashSet<String>>,
    ) -> String {
        let id = tree_id(kind, namespace, name);
        if !self.nodes.contains_key(&id) {
            let (health, info) = match known {
                Some(known) if !known.contains(&format!("{namespace}/{name}")) => {
                    ("Missing", "Referenced but not found".to_string())
                }
                Some(_) => ("Healthy", String::new()),
                None => ("Unknown", String::new()),
            };
            self.node(kind, group, namespace, name, health, info, None);
        }
        id
    }

    /// Follows owner links up to the top-level workload (Pod → ReplicaSet → Deployment).
    fn top_owner(&self, id: &str) -> String {
        let mut current = id.to_string();
        for _ in 0..4 {
            match self.nodes.get(&current).and_then(|node| node.owner.clone()) {
                Some(owner) if self.nodes.contains_key(&owner) => current = owner,
                _ => break,
            }
        }
        current
    }

    fn finish(self, warnings: Vec<String>) -> Topology {
        let mut edges: Vec<TopologyEdge> = self
            .edges
            .into_iter()
            .filter(|(from, to, _)| self.nodes.contains_key(from) && self.nodes.contains_key(to))
            .map(|(from, to, relation)| TopologyEdge {
                from,
                to,
                relation: relation.into(),
            })
            .collect();
        edges.sort_by(|left, right| left.from.cmp(&right.from).then(left.to.cmp(&right.to)));
        Topology {
            nodes: self.nodes.into_values().collect(),
            edges,
            warnings,
        }
    }
}

fn selector_matches(
    selector: &BTreeMap<String, String>,
    labels: Option<&BTreeMap<String, String>>,
) -> bool {
    !selector.is_empty()
        && labels.is_some_and(|labels| {
            selector
                .iter()
                .all(|(key, value)| labels.get(key) == Some(value))
        })
}

fn condition_true(object: &DynamicObject, condition: &str) -> Option<bool> {
    let conditions = object.data["status"]["conditions"].as_array()?;
    conditions
        .iter()
        .find(|item| item["type"] == condition)
        .map(|item| item["status"] == "True")
}

pub(super) async fn read(request: TopologyRequest) -> Result<Topology, String> {
    let client = client_for(request.kubeconfig_path.clone(), request.context.clone()).await?;
    let namespace = request
        .namespace
        .as_deref()
        .filter(|scope| !scope.is_empty() && *scope != "all namespaces");
    let mut warnings = Vec::new();
    let (
        mut w1,
        mut w2,
        mut w3,
        mut w4,
        mut w5,
        mut w6,
        mut w7,
        mut w8,
        mut w9,
        mut w10,
        mut w11,
        mut w12,
    ) = (
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let (
        pods,
        replica_sets,
        deployments,
        stateful_sets,
        daemon_sets,
        jobs,
        cron_jobs,
        services,
        ingresses,
        claims,
        config_maps,
        secrets,
        gateways,
        http_routes,
        grpc_routes,
    ) = tokio::join!(
        list(scoped::<Pod>(&client, namespace), "Pods", &mut w1),
        list(
            scoped::<ReplicaSet>(&client, namespace),
            "ReplicaSets",
            &mut w2
        ),
        list(
            scoped::<Deployment>(&client, namespace),
            "Deployments",
            &mut w3
        ),
        list(
            scoped::<StatefulSet>(&client, namespace),
            "StatefulSets",
            &mut w4
        ),
        list(
            scoped::<DaemonSet>(&client, namespace),
            "DaemonSets",
            &mut w5
        ),
        list(scoped::<Job>(&client, namespace), "Jobs", &mut w6),
        list(scoped::<CronJob>(&client, namespace), "CronJobs", &mut w7),
        list(scoped::<Service>(&client, namespace), "Services", &mut w8),
        list(scoped::<Ingress>(&client, namespace), "Ingresses", &mut w9),
        list(
            scoped::<PersistentVolumeClaim>(&client, namespace),
            "PersistentVolumeClaims",
            &mut w10
        ),
        list_names(
            scoped::<ConfigMap>(&client, namespace),
            "ConfigMaps",
            &mut w11
        ),
        list_names(scoped::<Secret>(&client, namespace), "Secrets", &mut w12),
        list_optional(dynamic_api(
            &client,
            namespace,
            "gateway.networking.k8s.io",
            "v1",
            "Gateway",
            "gateways"
        )),
        list_optional(dynamic_api(
            &client,
            namespace,
            "gateway.networking.k8s.io",
            "v1",
            "HTTPRoute",
            "httproutes"
        )),
        list_optional(dynamic_api(
            &client,
            namespace,
            "gateway.networking.k8s.io",
            "v1",
            "GRPCRoute",
            "grpcroutes"
        )),
    );
    for list in [w1, w2, w3, w4, w5, w6, w7, w8, w9, w10, w11, w12] {
        warnings.extend(list);
    }
    if pods.is_none() && deployments.is_none() && services.is_none() {
        return Err(warnings
            .first()
            .cloned()
            .unwrap_or_else(|| "The cluster returned no objects".into()));
    }
    let claim_names: Option<HashSet<String>> = claims.as_ref().map(|claims| {
        claims
            .iter()
            .map(|claim| {
                format!(
                    "{}/{}",
                    meta_ns(&claim.metadata),
                    meta_name(&claim.metadata)
                )
            })
            .collect()
    });
    let mut graph = Builder::default();
    let mut uses: Vec<(String, String, PodSpec)> = Vec::new();

    for claim in claims.unwrap_or_default() {
        let phase = claim
            .status
            .as_ref()
            .and_then(|status| status.phase.clone())
            .unwrap_or_default();
        let size = claim
            .status
            .as_ref()
            .and_then(|status| {
                status
                    .capacity
                    .as_ref()?
                    .get("storage")
                    .map(|quantity| quantity.0.clone())
            })
            .unwrap_or_default();
        let class = claim
            .spec
            .as_ref()
            .and_then(|spec| spec.storage_class_name.clone())
            .unwrap_or_default();
        let health = match phase.as_str() {
            "Bound" => "Healthy",
            "Lost" => "Degraded",
            _ => "Progressing",
        };
        let info = [phase, size, class]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" · ");
        graph.node(
            "PersistentVolumeClaim",
            "",
            &meta_ns(&claim.metadata),
            &meta_name(&claim.metadata),
            health,
            info,
            None,
        );
    }
    for deployment in deployments.unwrap_or_default() {
        let (ns, name) = (
            meta_ns(&deployment.metadata),
            meta_name(&deployment.metadata),
        );
        let desired = deployment
            .spec
            .as_ref()
            .and_then(|spec| spec.replicas)
            .unwrap_or(1);
        let ready = deployment
            .status
            .as_ref()
            .and_then(|status| status.ready_replicas)
            .unwrap_or(0);
        let id = graph.node(
            "Deployment",
            "apps",
            &ns,
            &name,
            replicas_health(desired, ready),
            format!("{ready}/{desired} ready"),
            None,
        );
        if let Some(spec) = deployment.spec.and_then(|spec| spec.template.spec) {
            uses.push((id, ns, spec));
        }
    }
    for set in stateful_sets.unwrap_or_default() {
        let (ns, name) = (meta_ns(&set.metadata), meta_name(&set.metadata));
        let desired = set
            .spec
            .as_ref()
            .and_then(|spec| spec.replicas)
            .unwrap_or(1);
        let ready = set
            .status
            .as_ref()
            .and_then(|status| status.ready_replicas)
            .unwrap_or(0);
        let id = graph.node(
            "StatefulSet",
            "apps",
            &ns,
            &name,
            replicas_health(desired, ready),
            format!("{ready}/{desired} ready"),
            None,
        );
        if let Some(spec) = set.spec.and_then(|spec| spec.template.spec) {
            uses.push((id, ns, spec));
        }
    }
    for set in daemon_sets.unwrap_or_default() {
        let (ns, name) = (meta_ns(&set.metadata), meta_name(&set.metadata));
        let status = set.status.clone().unwrap_or_default();
        let id = graph.node(
            "DaemonSet",
            "apps",
            &ns,
            &name,
            replicas_health(status.desired_number_scheduled, status.number_ready),
            format!(
                "{}/{} ready",
                status.number_ready, status.desired_number_scheduled
            ),
            None,
        );
        if let Some(spec) = set.spec.and_then(|spec| spec.template.spec) {
            uses.push((id, ns, spec));
        }
    }
    for cron in cron_jobs.unwrap_or_default() {
        let (ns, name) = (meta_ns(&cron.metadata), meta_name(&cron.metadata));
        let schedule = cron
            .spec
            .as_ref()
            .map(|spec| spec.schedule.clone())
            .unwrap_or_default();
        let suspended = cron
            .spec
            .as_ref()
            .and_then(|spec| spec.suspend)
            .unwrap_or(false);
        let id = graph.node(
            "CronJob",
            "batch",
            &ns,
            &name,
            "Healthy",
            if suspended {
                format!("{schedule} · suspended")
            } else {
                schedule
            },
            None,
        );
        if let Some(spec) = cron
            .spec
            .and_then(|spec| spec.job_template.spec)
            .and_then(|spec| spec.template.spec)
        {
            uses.push((id, ns, spec));
        }
    }
    for job in jobs.unwrap_or_default() {
        let (ns, name) = (meta_ns(&job.metadata), meta_name(&job.metadata));
        let owner = controller_owner(&job.metadata, &ns);
        let status = job.status.clone().unwrap_or_default();
        let failed = status.failed.unwrap_or(0);
        let succeeded = status.succeeded.unwrap_or(0);
        let completions = job
            .spec
            .as_ref()
            .and_then(|spec| spec.completions)
            .unwrap_or(1);
        let health = if status
            .conditions
            .iter()
            .flatten()
            .any(|condition| condition.type_ == "Failed" && condition.status == "True")
        {
            "Degraded"
        } else if succeeded >= completions {
            "Healthy"
        } else {
            "Progressing"
        };
        let info = format!(
            "{succeeded}/{completions} complete{}",
            if failed > 0 {
                format!(" · {failed} failed")
            } else {
                String::new()
            }
        );
        let id = graph.node("Job", "batch", &ns, &name, health, info, owner.clone());
        if let Some(owner) = owner {
            graph.edge(&owner, &id, "owns");
        } else if let Some(spec) = job
            .spec
            .map(|spec| spec.template)
            .and_then(|template| template.spec)
        {
            uses.push((id, ns, spec));
        }
    }
    let mut live_replica_sets = HashSet::new();
    for pod in pods.iter().flatten() {
        if let Some(owner) = controller_owner(&pod.metadata, &meta_ns(&pod.metadata)) {
            live_replica_sets.insert(owner);
        }
    }
    for replica_set in replica_sets.unwrap_or_default() {
        let (ns, name) = (
            meta_ns(&replica_set.metadata),
            meta_name(&replica_set.metadata),
        );
        let id = tree_id("ReplicaSet", &ns, &name);
        let desired = replica_set
            .spec
            .as_ref()
            .and_then(|spec| spec.replicas)
            .unwrap_or(0);
        // Old revisions scaled to zero only add noise, as in Argo CD's default view.
        if desired == 0 && !live_replica_sets.contains(&id) {
            continue;
        }
        let ready = replica_set
            .status
            .as_ref()
            .and_then(|status| status.ready_replicas)
            .unwrap_or(0);
        let owner = controller_owner(&replica_set.metadata, &ns);
        let revision = replica_set
            .metadata
            .annotations
            .as_ref()
            .and_then(|annotations| {
                annotations
                    .get("deployment.kubernetes.io/revision")
                    .cloned()
            })
            .map(|revision| format!("rev {revision} · "))
            .unwrap_or_default();
        graph.node(
            "ReplicaSet",
            "apps",
            &ns,
            &name,
            replicas_health(desired, ready),
            format!("{revision}{ready}/{desired} ready"),
            owner.clone(),
        );
        if let Some(owner) = owner {
            graph.edge(&owner, &id, "owns");
        } else if let Some(spec) = replica_set
            .spec
            .and_then(|spec| spec.template)
            .and_then(|template| template.spec)
        {
            uses.push((id, ns, spec));
        }
    }
    let pods = pods.unwrap_or_default();
    for pod in &pods {
        let (ns, name) = (meta_ns(&pod.metadata), meta_name(&pod.metadata));
        let owner = controller_owner(&pod.metadata, &ns);
        let (health, info) = pod_health(pod);
        let id = graph.node("Pod", "", &ns, &name, &health, info, owner.clone());
        match owner {
            Some(owner) if graph.nodes.contains_key(&owner) => graph.edge(&owner, &id, "owns"),
            // Bare Pods (and Pods of controllers we do not list) show their own references.
            _ => {
                if let Some(spec) = pod.spec.clone() {
                    uses.push((id, ns, spec));
                }
            }
        }
    }
    for (from, ns, spec) in uses {
        for (kind, name) in template_refs(&spec) {
            let known = match kind {
                "ConfigMap" => config_maps.as_ref(),
                "Secret" => secrets.as_ref(),
                _ => claim_names.as_ref(),
            };
            let id = graph.reference(kind, "", &ns, &name, known);
            graph.edge(&from, &id, "uses");
        }
    }
    let service_ids: HashSet<String> = services
        .iter()
        .flatten()
        .map(|service| {
            tree_id(
                "Service",
                &meta_ns(&service.metadata),
                &meta_name(&service.metadata),
            )
        })
        .collect();
    let service_names: HashSet<String> = services
        .iter()
        .flatten()
        .map(|service| {
            format!(
                "{}/{}",
                meta_ns(&service.metadata),
                meta_name(&service.metadata)
            )
        })
        .collect();
    let mut pods_by_namespace: HashMap<String, Vec<&Pod>> = HashMap::new();
    for pod in &pods {
        pods_by_namespace
            .entry(meta_ns(&pod.metadata))
            .or_default()
            .push(pod);
    }
    for service in services.unwrap_or_default() {
        let (ns, name) = (meta_ns(&service.metadata), meta_name(&service.metadata));
        let spec = service.spec.unwrap_or_default();
        let kind = spec.type_.clone().unwrap_or_else(|| "ClusterIP".into());
        let ports = spec
            .ports
            .iter()
            .flatten()
            .map(|port| port.port.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let selector = spec.selector.clone().unwrap_or_default();
        let selected: Vec<String> = pods_by_namespace
            .get(&ns)
            .into_iter()
            .flatten()
            .filter(|pod| selector_matches(&selector, pod.metadata.labels.as_ref()))
            .map(|pod| graph.top_owner(&tree_id("Pod", &ns, &meta_name(&pod.metadata))))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let (health, note) = if kind == "ExternalName" {
            ("Healthy", spec.external_name.clone().unwrap_or_default())
        } else if selector.is_empty() {
            ("Unknown", "no selector".to_string())
        } else if selected.is_empty() {
            ("Degraded", "no matching pods".to_string())
        } else {
            ("Healthy", String::new())
        };
        let info = [
            kind,
            if ports.is_empty() {
                String::new()
            } else {
                format!(":{ports}")
            },
            note,
        ]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
        .replacen(" · :", " :", 1);
        let id = graph.node("Service", "", &ns, &name, health, info, None);
        for target in selected {
            graph.edge(&id, &target, "selects");
        }
    }
    for ingress in ingresses.unwrap_or_default() {
        let (ns, name) = (meta_ns(&ingress.metadata), meta_name(&ingress.metadata));
        let spec = ingress.spec.unwrap_or_default();
        let hosts: BTreeSet<String> = spec
            .rules
            .iter()
            .flatten()
            .filter_map(|rule| rule.host.clone())
            .collect();
        let mut backends: BTreeSet<String> = spec
            .rules
            .iter()
            .flatten()
            .flat_map(|rule| rule.http.iter().flat_map(|http| http.paths.iter()))
            .filter_map(|path| {
                path.backend
                    .service
                    .as_ref()
                    .map(|service| service.name.clone())
            })
            .collect();
        if let Some(service) = spec.default_backend.and_then(|backend| backend.service) {
            backends.insert(service.name);
        }
        let class = spec.ingress_class_name.unwrap_or_default();
        let info = [hosts.into_iter().collect::<Vec<_>>().join(", "), class]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" · ");
        let id = graph.node(
            "Ingress",
            "networking.k8s.io",
            &ns,
            &name,
            "Healthy",
            info,
            None,
        );
        for backend in backends {
            let target = graph.reference("Service", "", &ns, &backend, Some(&service_names));
            graph.edge(&id, &target, "routes");
        }
    }
    for gateway in &gateways {
        let ns = gateway.metadata.namespace.clone().unwrap_or_default();
        let name = gateway.metadata.name.clone().unwrap_or_default();
        let class = gateway.data["spec"]["gatewayClassName"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let health = match condition_true(gateway, "Programmed") {
            Some(true) => "Healthy",
            Some(false) => "Degraded",
            None => "Progressing",
        };
        graph.node(
            "Gateway",
            "gateway.networking.k8s.io",
            &ns,
            &name,
            health,
            class,
            None,
        );
    }
    for (kind, routes) in [("HTTPRoute", &http_routes), ("GRPCRoute", &grpc_routes)] {
        for route in routes {
            let ns = route.metadata.namespace.clone().unwrap_or_default();
            let name = route.metadata.name.clone().unwrap_or_default();
            let hosts = route.data["spec"]["hostnames"]
                .as_array()
                .map(|hosts| {
                    hosts
                        .iter()
                        .filter_map(|host| host.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            let accepted = route.data["status"]["parents"]
                .as_array()
                .map(|parents| {
                    parents.iter().all(|parent| {
                        parent["conditions"].as_array().is_none_or(|conditions| {
                            !conditions
                                .iter()
                                .any(|item| item["type"] == "Accepted" && item["status"] == "False")
                        })
                    })
                })
                .unwrap_or(true);
            let id = graph.node(
                kind,
                "gateway.networking.k8s.io",
                &ns,
                &name,
                if accepted { "Healthy" } else { "Degraded" },
                hosts,
                None,
            );
            for parent in route.data["spec"]["parentRefs"]
                .as_array()
                .into_iter()
                .flatten()
            {
                if parent["kind"].as_str().unwrap_or("Gateway") != "Gateway" {
                    continue;
                }
                let parent_ns = parent["namespace"].as_str().unwrap_or(&ns).to_string();
                let parent_name = parent["name"].as_str().unwrap_or_default();
                let gateway = tree_id("Gateway", &parent_ns, parent_name);
                if graph.nodes.contains_key(&gateway) {
                    graph.edge(&gateway, &id, "routes");
                }
            }
            let rules = route.data["spec"]["rules"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            for backend in rules
                .iter()
                .flat_map(|rule| rule["backendRefs"].as_array().cloned().unwrap_or_default())
            {
                if backend["kind"].as_str().unwrap_or("Service") != "Service"
                    || backend["group"]
                        .as_str()
                        .is_some_and(|group| !group.is_empty())
                {
                    continue;
                }
                let backend_ns = backend["namespace"].as_str().unwrap_or(&ns).to_string();
                let Some(backend_name) = backend["name"].as_str() else {
                    continue;
                };
                // Only resolve Services in the namespaces we read.
                if namespace.is_some_and(|scope| scope != backend_ns)
                    && !service_ids.contains(&tree_id("Service", &backend_ns, backend_name))
                {
                    continue;
                }
                let target = graph.reference(
                    "Service",
                    "",
                    &backend_ns,
                    backend_name,
                    Some(&service_names),
                );
                graph.edge(&id, &target, "routes");
            }
        }
    }
    Ok(graph.finish(warnings))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_reference_config_secrets_and_claims_once() {
        let spec: PodSpec = serde_json::from_value(serde_json::json!({
            "containers": [{
                "name": "app",
                "envFrom": [{ "configMapRef": { "name": "settings" } }, { "secretRef": { "name": "db" } }],
                "env": [{ "name": "TOKEN", "valueFrom": { "secretKeyRef": { "name": "db", "key": "token" } } }]
            }],
            "volumes": [
                { "name": "config", "configMap": { "name": "settings" } },
                { "name": "data", "persistentVolumeClaim": { "claimName": "data" } },
                { "name": "bundle", "projected": { "sources": [{ "secret": { "name": "tls" } }] } }
            ],
            "imagePullSecrets": [{ "name": "registry" }]
        }))
        .unwrap();
        let refs: Vec<_> = template_refs(&spec).into_iter().collect();
        assert_eq!(
            refs,
            vec![
                ("ConfigMap", "settings".to_string()),
                ("PersistentVolumeClaim", "data".to_string()),
                ("Secret", "db".to_string()),
                ("Secret", "registry".to_string()),
                ("Secret", "tls".to_string()),
            ]
        );
    }

    #[test]
    fn services_resolve_to_the_top_level_workload_and_missing_references_are_flagged() {
        let mut graph = Builder::default();
        let deployment = graph.node(
            "Deployment",
            "apps",
            "web",
            "shop",
            "Healthy",
            String::new(),
            None,
        );
        let replica_set = graph.node(
            "ReplicaSet",
            "apps",
            "web",
            "shop-7d9",
            "Healthy",
            String::new(),
            Some(deployment.clone()),
        );
        let pod = graph.node(
            "Pod",
            "",
            "web",
            "shop-7d9-x",
            "Healthy",
            String::new(),
            Some(replica_set),
        );
        assert_eq!(graph.top_owner(&pod), deployment);
        let known: HashSet<String> = ["web/present".to_string()].into();
        let missing = graph.reference("ConfigMap", "", "web", "absent", Some(&known));
        let present = graph.reference("ConfigMap", "", "web", "present", Some(&known));
        assert_eq!(graph.nodes[&missing].health, "Missing");
        assert_eq!(graph.nodes[&present].health, "Healthy");
        assert!(selector_matches(
            &[("app".to_string(), "shop".to_string())].into(),
            Some(
                &[
                    ("app".to_string(), "shop".to_string()),
                    ("tier".to_string(), "web".to_string())
                ]
                .into()
            )
        ));
        assert!(!selector_matches(&BTreeMap::new(), None));
    }
}
