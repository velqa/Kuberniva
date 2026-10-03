/** Open Cluster Management hub objects, read from their manifests. */
import { cpuLabel, memoryLabel } from './quantity.ts';

type Manifest = Record<string, unknown>;
const record = (value: unknown): Manifest => (value && typeof value === 'object' && !Array.isArray(value) ? value as Manifest : {});
const list = (value: unknown): Manifest[] => (Array.isArray(value) ? value.map(record) : []);
const text = (value: unknown) => (typeof value === 'string' || typeof value === 'number' || typeof value === 'boolean' ? String(value) : '');

export type OcmCondition = { type: string; status: string; reason: string; message: string };
export type OcmCluster = {
  name: string; health: 'Healthy' | 'Degraded' | 'Unknown' | 'Pending'; available: string; joined: boolean; accepted: boolean;
  kubernetes: string; clusterSet: string; url: string; cpu: string; memory: string; claims: { name: string; value: string }[];
  labels: [string, string][]; taints: string[]; conditions: OcmCondition[];
};
export type OcmClusterSet = { name: string; selector: string; members: number };
export type OcmPlacement = { name: string; namespace: string; clusterSets: string[]; requested: string; selected: number; satisfied: string; decisions: string[] };
export type OcmManifestWork = { name: string; cluster: string; resources: number; applied: string; available: string; degraded: string; health: 'Healthy' | 'Degraded' | 'Progressing' | 'Unknown'; failing: string[] };
export type OcmPolicy = { name: string; namespace: string; remediation: string; disabled: boolean; compliant: string; clusters: { cluster: string; compliant: string }[] };

export const CLUSTER_SET_LABEL = 'cluster.open-cluster-management.io/clusterset';

export function conditions(value: unknown): OcmCondition[] {
  return list(value).map((condition) => ({ type: text(condition.type), status: text(condition.status) || 'Unknown', reason: text(condition.reason), message: text(condition.message) })).filter((condition) => condition.type);
}

const conditionStatus = (all: OcmCondition[], type: string) => all.find((condition) => condition.type === type)?.status || 'Unknown';

export function managedCluster(manifest: Manifest): OcmCluster {
  const metadata = record(manifest.metadata);
  const spec = record(manifest.spec);
  const status = record(manifest.status);
  const all = conditions(status.conditions);
  const labels = Object.entries(record(metadata.labels)).map(([key, value]) => [key, text(value)] as [string, string]);
  const available = conditionStatus(all, 'ManagedClusterConditionAvailable');
  const joined = conditionStatus(all, 'ManagedClusterJoined') === 'True';
  const accepted = Boolean(spec.hubAcceptsClient) && conditionStatus(all, 'HubAcceptedManagedCluster') !== 'False';
  const capacity = record(status.allocatable).cpu ? record(status.allocatable) : record(status.capacity);
  return {
    name: text(metadata.name),
    health: !accepted || !joined ? 'Pending' : available === 'True' ? 'Healthy' : available === 'False' ? 'Degraded' : 'Unknown',
    available,
    joined,
    accepted,
    kubernetes: text(record(status.version).kubernetes),
    clusterSet: labels.find(([key]) => key === CLUSTER_SET_LABEL)?.[1] || '',
    url: text(list(spec.managedClusterClientConfigs)[0]?.url),
    cpu: capacity.cpu ? cpuLabel(text(capacity.cpu)) : '',
    memory: capacity.memory ? memoryLabel(text(capacity.memory)) : '',
    claims: list(status.clusterClaims).map((claim) => ({ name: text(claim.name), value: text(claim.value) })),
    labels,
    taints: list(spec.taints).map((taint) => `${text(taint.key)}${taint.value ? `=${text(taint.value)}` : ''}:${text(taint.effect)}`),
    conditions: all,
  };
}

export function clusterSet(manifest: Manifest, clusters: OcmCluster[]): OcmClusterSet {
  const name = text(record(manifest.metadata).name);
  const selector = record(record(manifest.spec).clusterSelector);
  const type = text(selector.selectorType) || 'ExclusiveClusterSetLabel';
  return {
    name,
    selector: type === 'LabelSelector' ? 'Label selector' : type === 'ExclusiveClusterSetLabel' ? 'Cluster set label' : type,
    members: type === 'ExclusiveClusterSetLabel' ? clusters.filter((cluster) => cluster.clusterSet === name).length : name === 'global' ? clusters.length : 0,
  };
}

export function placement(manifest: Manifest, decisions: Manifest[]): OcmPlacement {
  const metadata = record(manifest.metadata);
  const spec = record(manifest.spec);
  const status = record(manifest.status);
  const name = text(metadata.name);
  const namespace = text(metadata.namespace);
  const chosen = decisions
    .filter((decision) => text(record(decision.metadata).namespace) === namespace && text(record(record(decision.metadata).labels)['cluster.open-cluster-management.io/placement']) === name)
    .flatMap((decision) => list(record(decision.status).decisions).map((entry) => text(entry.clusterName)))
    .filter(Boolean);
  return {
    name,
    namespace,
    clusterSets: Array.isArray(spec.clusterSets) ? spec.clusterSets.map(text) : [],
    requested: spec.numberOfClusters === undefined ? 'All' : text(spec.numberOfClusters),
    selected: typeof status.numberOfSelectedClusters === 'number' ? status.numberOfSelectedClusters : chosen.length,
    satisfied: conditionStatus(conditions(status.conditions), 'PlacementSatisfied'),
    decisions: [...new Set(chosen)].sort(),
  };
}

export function manifestWork(manifest: Manifest): OcmManifestWork {
  const metadata = record(manifest.metadata);
  const status = record(manifest.status);
  const all = conditions(status.conditions);
  const applied = conditionStatus(all, 'Applied');
  const available = conditionStatus(all, 'Available');
  const degraded = conditionStatus(all, 'Degraded');
  const failing = list(record(status.resourceStatus).manifests)
    .filter((entry) => conditions(entry.conditions).some((condition) => ['Applied', 'Available'].includes(condition.type) && condition.status === 'False'))
    .map((entry) => { const meta = record(entry.resourceMeta); return `${text(meta.kind)} ${text(meta.namespace) ? `${text(meta.namespace)}/` : ''}${text(meta.name)}`; });
  return {
    name: text(metadata.name),
    cluster: text(metadata.namespace),
    resources: list(record(record(manifest.spec).workload).manifests).length,
    applied,
    available,
    degraded,
    health: degraded === 'True' || applied === 'False' || failing.length ? 'Degraded' : applied === 'True' && available === 'True' ? 'Healthy' : applied === 'True' ? 'Progressing' : 'Unknown',
    failing,
  };
}

export function policy(manifest: Manifest): OcmPolicy {
  const metadata = record(manifest.metadata);
  const spec = record(manifest.spec);
  const status = record(manifest.status);
  return {
    name: text(metadata.name),
    namespace: text(metadata.namespace),
    remediation: text(spec.remediationAction) || 'inform',
    disabled: Boolean(spec.disabled),
    compliant: text(status.compliant) || 'Pending',
    clusters: list(status.status).map((entry) => ({ cluster: text(entry.clustername) || text(entry.clusterName), compliant: text(entry.compliant) || 'Pending' })),
  };
}

export function ocmTone(value: string): 'ok' | 'warn' | 'bad' | 'none' {
  if (['Healthy', 'True', 'Compliant'].includes(value)) return 'ok';
  if (['Progressing', 'Pending', 'Unknown'].includes(value)) return value === 'Unknown' ? 'none' : 'warn';
  if (['Degraded', 'False', 'NonCompliant'].includes(value)) return 'bad';
  return 'none';
}
