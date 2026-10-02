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
  resources: { kind: string; name: string; namespace: string; sync: string; health: string }[];
};

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
  const operationState = record(status.operationState);
  const sources = spec.source ? [record(spec.source)] : list(spec.sources);
  const destination = record(spec.destination);
  const automated = record(spec.syncPolicy).automated;
  const sync = text(record(status.sync).status) || 'Unknown';
  const health = text(record(status.health).status) || 'Unknown';
  const operation = text(operationState.phase);
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
    operationMessage: text(operationState.message),
    source: sources.map(describeSource).join(' + ') || '—',
    revision: syncRevision(record(status.sync)),
    destination: `${text(destination.name) || text(destination.server).replace(/^https?:\/\//, '') || 'in-cluster'} / ${text(destination.namespace) || '—'}`,
    autoSync: autoSettings,
    reconciledAt: text(status.reconciledAt),
    conditions,
    attention,
    resources: list(status.resources).map((resource) => ({
      kind: text(resource.kind),
      name: text(resource.name),
      namespace: text(resource.namespace),
      sync: text(resource.status),
      health: text(record(resource.health).status),
    })),
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
