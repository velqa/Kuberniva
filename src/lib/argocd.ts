/** Argo CD Application status, read from argoproj.io/v1alpha1 manifests. */

type Manifest = Record<string, unknown>;
const record = (value: unknown): Manifest => (value && typeof value === 'object' && !Array.isArray(value) ? value as Manifest : {});
const list = (value: unknown): Manifest[] => (Array.isArray(value) ? value.map(record) : []);
const text = (value: unknown) => (typeof value === 'string' || typeof value === 'number' ? String(value) : '');

export type ArgoApp = {
  name: string;
  namespace: string;
  project: string;
  sync: string;
  health: string;
  operation: string;
  operationMessage: string;
  source: string;
  revision: string;
  destination: string;
  autoSync: string;
  reconciledAt: string;
  conditions: string[];
  attention: string[];
  /** The ApplicationSet that generated this app, if any. */
  owner: string;
  resources: ArgoResource[];
  sources: ArgoSourceDetail[];
  destinationNamespace: string;
  /** True when Argo CD deploys into the cluster it runs in, so Kuberniva can show live objects. */
  inCluster: boolean;
  automated: { prune: boolean; selfHeal: boolean } | null;
  syncOptions: string[];
  images: string[];
  externalURLs: string[];
  history: ArgoHistoryEntry[];
  lastOperation: ArgoOperation | null;
  createdAt: string;
};

export type ArgoResource = { group: string; version: string; kind: string; name: string; namespace: string; sync: string; health: string; healthMessage: string; requiresPruning: boolean; hook: boolean };
export type ArgoSourceDetail = { repoURL: string; repo: string; path: string; chart: string; targetRevision: string; type: 'Helm' | 'Kustomize' | 'Directory' | 'Plugin'; parameters: { name: string; value: string }[]; valueFiles: string[] };
export type ArgoHistoryEntry = { id: number; revision: string; deployedAt: string; startedAt: string; initiatedBy: string; source: string };
export type ArgoSyncResult = { group: string; kind: string; name: string; namespace: string; status: string; message: string; hookPhase: string; syncPhase: string };
export type ArgoOperation = { phase: string; message: string; startedAt: string; finishedAt: string; initiatedBy: string; revision: string; dryRun: boolean; prune: boolean; retryCount: number; results: ArgoSyncResult[] };

const KIND_ORDER = ['Namespace', 'ServiceAccount', 'ClusterRole', 'ClusterRoleBinding', 'Role', 'RoleBinding', 'CustomResourceDefinition', 'ConfigMap', 'Secret', 'PersistentVolumeClaim', 'Service', 'Deployment', 'StatefulSet', 'DaemonSet', 'Job', 'CronJob', 'HorizontalPodAutoscaler', 'Ingress', 'Gateway', 'HTTPRoute', 'NetworkPolicy'];
const kindRank = (kind: string) => {
  const index = KIND_ORDER.indexOf(kind);
  return index === -1 ? KIND_ORDER.length : index;
};

function sourceDetail(source: Manifest): ArgoSourceDetail {
  const helm = record(source.helm);
  const kustomize = record(source.kustomize);
  const plugin = record(source.plugin);
  const type: ArgoSourceDetail['type'] = text(source.chart) || Object.keys(helm).length ? 'Helm' : Object.keys(kustomize).length ? 'Kustomize' : Object.keys(plugin).length ? 'Plugin' : 'Directory';
  const parameters = [
    ...list(helm.parameters).map((parameter) => ({ name: text(parameter.name), value: text(parameter.value) })),
    ...(Array.isArray(kustomize.images) ? kustomize.images.map((image) => ({ name: 'image', value: text(image) })) : []),
    ...(text(kustomize.namePrefix) ? [{ name: 'namePrefix', value: text(kustomize.namePrefix) }] : []),
    ...list(plugin.env).map((env) => ({ name: text(env.name), value: text(env.value) })),
  ];
  if (text(helm.releaseName)) parameters.unshift({ name: 'releaseName', value: text(helm.releaseName) });
  return {
    repoURL: text(source.repoURL),
    repo: text(source.repoURL).replace(/^https?:\/\//, '').replace(/\.git$/, ''),
    path: text(source.path),
    chart: text(source.chart),
    targetRevision: text(source.targetRevision) || 'HEAD',
    type,
    parameters,
    valueFiles: Array.isArray(helm.valueFiles) ? helm.valueFiles.map(text).filter(Boolean) : [],
  };
}

function initiator(operation: Manifest) {
  const initiatedBy = record(operation.initiatedBy);
  return initiatedBy.automated ? 'Auto-sync' : text(initiatedBy.username) || '—';
}

function operationState(state: Manifest): ArgoOperation | null {
  if (!Object.keys(state).length) return null;
  const operation = record(state.operation);
  const sync = record(operation.sync);
  const result = record(state.syncResult);
  return {
    phase: text(state.phase),
    message: text(state.message),
    startedAt: text(state.startedAt),
    finishedAt: text(state.finishedAt),
    initiatedBy: initiator(operation),
    revision: (text(result.revision) || text(sync.revision) || (Array.isArray(result.revisions) ? result.revisions.map(text).join(', ') : '')).slice(0, 12),
    dryRun: Boolean(sync.dryRun),
    prune: Boolean(sync.prune),
    retryCount: typeof state.retryCount === 'number' ? state.retryCount : 0,
    results: list(result.resources).map((item) => ({
      group: text(item.group),
      kind: text(item.kind),
      name: text(item.name),
      namespace: text(item.namespace),
      status: text(item.status),
      message: text(item.message),
      hookPhase: text(item.hookPhase),
      syncPhase: text(item.syncPhase),
    })),
  };
}

/** Managed resources grouped by kind, in the order Argo CD applies them. */
export function resourcesByKind(resources: ArgoResource[]) {
  const groups = new Map<string, ArgoResource[]>();
  for (const resource of resources) groups.set(resource.kind, [...(groups.get(resource.kind) || []), resource]);
  return [...groups.entries()].sort(([left], [right]) => kindRank(left) - kindRank(right) || left.localeCompare(right));
}

function describeSource(source: Manifest) {
  const repo = text(source.repoURL).replace(/^https?:\/\//, '').replace(/\.git$/, '');
  const target = text(source.chart) ? `${text(source.chart)}@${text(source.targetRevision) || 'latest'}` : `${text(source.path) || '.'}@${text(source.targetRevision) || 'HEAD'}`;
  return repo ? `${repo} · ${target}` : target;
}

function syncRevision(sync: Manifest) {
  const revisions = Array.isArray(sync.revisions) ? sync.revisions.map(text) : [text(sync.revision)];
  return revisions.filter(Boolean).map((revision) => revision.slice(0, 7)).join(', ');
}

export function argoApplication(manifest: Manifest): ArgoApp {
  const metadata = record(manifest.metadata);
  const spec = record(manifest.spec);
  const status = record(manifest.status);
  const operationState_ = record(status.operationState);
  const sources = spec.source ? [record(spec.source)] : list(spec.sources);
  const destination = record(spec.destination);
  const automated = record(spec.syncPolicy).automated;
  const sync = text(record(status.sync).status) || 'Unknown';
  const health = text(record(status.health).status) || 'Unknown';
  const operation = text(operationState_.phase);
  const conditions = list(status.conditions).map((condition) => `${text(condition.type)}: ${text(condition.message)}`);
  const attention: string[] = [];
  if (sync === 'OutOfSync') attention.push('Out of sync');
  if (['Degraded', 'Missing'].includes(health)) attention.push(health);
  if (['Failed', 'Error'].includes(operation)) attention.push(`Sync ${operation.toLowerCase()}`);
  if (list(status.conditions).some((condition) => /Error|Warning/.test(text(condition.type)))) attention.push('Has conditions');
  const autoSettings = automated && typeof automated === 'object'
    ? ['Automated', record(automated).prune ? 'prune' : '', record(automated).selfHeal ? 'self-heal' : ''].filter(Boolean).join(' · ')
    : 'Manual';
  return {
    name: text(metadata.name),
    namespace: text(metadata.namespace),
    project: text(spec.project) || 'default',
    sync,
    health,
    operation,
    operationMessage: text(operationState_.message),
    source: sources.map(describeSource).join(' + ') || '—',
    revision: syncRevision(record(status.sync)),
    destination: `${text(destination.name) || text(destination.server).replace(/^https?:\/\//, '') || 'in-cluster'} / ${text(destination.namespace) || '—'}`,
    autoSync: autoSettings,
    reconciledAt: text(status.reconciledAt),
    conditions,
    attention,
    owner: list(metadata.ownerReferences).find((reference) => text(reference.kind) === 'ApplicationSet')?.name as string || '',
    sources: sources.map(sourceDetail),
    destinationNamespace: text(destination.namespace),
    inCluster: (!text(destination.server) && (!text(destination.name) || text(destination.name) === 'in-cluster')) || /^https:\/\/kubernetes\.default\.svc(:443)?\/?$/.test(text(destination.server)),
    automated: automated && typeof automated === 'object' ? { prune: Boolean(record(automated).prune), selfHeal: Boolean(record(automated).selfHeal) } : null,
    syncOptions: Array.isArray(record(spec.syncPolicy).syncOptions) ? (record(spec.syncPolicy).syncOptions as unknown[]).map(text).filter(Boolean) : [],
    images: Array.isArray(record(status.summary).images) ? (record(status.summary).images as unknown[]).map(text).filter(Boolean) : [],
    externalURLs: Array.isArray(record(status.summary).externalURLs) ? (record(status.summary).externalURLs as unknown[]).map(text).filter(Boolean) : [],
    history: list(status.history).map((entry) => ({
      id: typeof entry.id === 'number' ? entry.id : Number(entry.id) || 0,
      revision: (text(entry.revision) || (Array.isArray(entry.revisions) ? entry.revisions.map(text).join(', ') : '')).slice(0, 12),
      deployedAt: text(entry.deployedAt),
      startedAt: text(entry.deployStartedAt),
      initiatedBy: record(entry.initiatedBy).automated ? 'Auto-sync' : text(record(entry.initiatedBy).username) || '—',
      source: entry.source ? describeSource(record(entry.source)) : list(entry.sources).map(describeSource).join(' + '),
    })).sort((left, right) => right.id - left.id),
    lastOperation: operationState(operationState_),
    createdAt: text(metadata.creationTimestamp),
    resources: list(status.resources).map((resource) => ({
      group: text(resource.group),
      version: text(resource.version),
      kind: text(resource.kind),
      name: text(resource.name),
      namespace: text(resource.namespace),
      sync: text(resource.status),
      health: text(record(resource.health).status),
      healthMessage: text(record(resource.health).message),
      requiresPruning: Boolean(resource.requiresPruning),
      hook: Boolean(resource.hook),
    })).sort((left, right) => kindRank(left.kind) - kindRank(right.kind) || left.name.localeCompare(right.name)),
  };
}

export type ArgoSummary = { total: number; synced: number; outOfSync: number; healthy: number; degraded: number; progressing: number; attention: ArgoApp[] };

export function summarizeArgo(apps: ArgoApp[]): ArgoSummary {
  return {
    total: apps.length,
    synced: apps.filter((app) => app.sync === 'Synced').length,
    outOfSync: apps.filter((app) => app.sync === 'OutOfSync').length,
    healthy: apps.filter((app) => app.health === 'Healthy').length,
    degraded: apps.filter((app) => app.health === 'Degraded' || app.health === 'Missing').length,
    progressing: apps.filter((app) => app.health === 'Progressing' || app.operation === 'Running').length,
    attention: apps.filter((app) => app.attention.length),
  };
}

export function argoTone(value: string): 'ok' | 'warn' | 'bad' | 'none' {
  if (['Synced', 'Healthy', 'Succeeded'].includes(value)) return 'ok';
  if (['OutOfSync', 'Progressing', 'Suspended', 'Running'].includes(value)) return 'warn';
  if (['Degraded', 'Missing', 'Failed', 'Error'].includes(value)) return 'bad';
  return 'none';
}

export type ArgoAppSet = { name: string; namespace: string; generators: string[]; template: string; project: string; policy: string; conditions: { type: string; ok: boolean; message: string }[]; generated: number; attention: string[] };

export function argoApplicationSet(manifest: Manifest, apps: ArgoApp[] = []): ArgoAppSet {
  const metadata = record(manifest.metadata);
  const spec = record(manifest.spec);
  const template = record(spec.template);
  const name = text(metadata.name);
  const conditions = list(record(manifest.status).conditions).map((condition) => ({
    type: text(condition.type),
    ok: (text(condition.type) === 'ErrorOccurred') !== (text(condition.status) === 'True'),
    message: text(condition.message),
  }));
  const generated = list(record(manifest.status).resources).length || apps.filter((app) => app.owner === name).length;
  const syncPolicy = record(spec.syncPolicy);
  const policy = [text(syncPolicy.applicationsSync) || 'create, update, delete', syncPolicy.preserveResourcesOnDeletion ? 'preserves resources' : ''].filter(Boolean).join(' · ');
  return {
    name,
    namespace: text(metadata.namespace),
    generators: list(spec.generators).flatMap((generator) => Object.keys(generator)),
    template: text(record(template.metadata).name) || '—',
    project: text(record(template.spec).project) || 'default',
    policy,
    conditions,
    generated,
    attention: conditions.filter((condition) => !condition.ok && condition.type === 'ErrorOccurred').map((condition) => condition.message || 'Generation error'),
  };
}

export type ArgoProject = { name: string; namespace: string; description: string; sources: string[]; destinations: string[]; clusterResources: number; roles: number; syncWindows: number; apps: number };

export function argoProject(manifest: Manifest, apps: ArgoApp[] = []): ArgoProject {
  const metadata = record(manifest.metadata);
  const spec = record(manifest.spec);
  const name = text(metadata.name);
  return {
    name,
    namespace: text(metadata.namespace),
    description: text(spec.description),
    sources: Array.isArray(spec.sourceRepos) ? spec.sourceRepos.map(text).filter(Boolean) : [],
    destinations: list(spec.destinations).map((destination) => `${text(destination.name) || text(destination.server).replace(/^https?:\/\//, '') || '*'} / ${text(destination.namespace) || '*'}`),
    clusterResources: list(spec.clusterResourceWhitelist).length,
    roles: list(spec.roles).length,
    syncWindows: list(spec.syncWindows).length,
    apps: apps.filter((app) => app.project === name).length,
  };
}

/** True for errors that mean "not allowed at this scope", where a narrower namespace may still work. */
export function isForbidden(error: unknown) {
  return /forbidden|403|cannot list/i.test(String(error));
}
