import assert from 'node:assert/strict';
import { test } from 'node:test';
import { argoApplication, argoTone, summarizeArgo } from './argocd.ts';

const healthy = argoApplication({
  metadata: { name: 'shop', namespace: 'argocd' },
  spec: { project: 'payments', source: { repoURL: 'https://github.com/acme/deploy.git', path: 'shop', targetRevision: 'main' }, destination: { server: 'https://kubernetes.default.svc', namespace: 'shop' }, syncPolicy: { automated: { prune: true, selfHeal: true } } },
  status: { sync: { status: 'Synced', revision: '0123456789abcdef' }, health: { status: 'Healthy' }, operationState: { phase: 'Succeeded' }, resources: [{ kind: 'Deployment', name: 'shop', namespace: 'shop', status: 'Synced', health: { status: 'Healthy' } }] },
});

const broken = argoApplication({
  metadata: { name: 'billing', namespace: 'argocd' },
  spec: { sources: [{ repoURL: 'https://charts.acme.io', chart: 'billing', targetRevision: '1.2.0' }], destination: { name: 'prod', namespace: 'billing' } },
  status: { sync: { status: 'OutOfSync', revisions: ['abcdef0123'] }, health: { status: 'Degraded' }, operationState: { phase: 'Failed', message: 'hook failed' }, conditions: [{ type: 'SyncError', message: 'boom' }] },
});

test('applications read source, destination, revision, and sync policy', () => {
  assert.equal(healthy.source, 'github.com/acme/deploy · shop@main');
  assert.equal(healthy.destination, 'kubernetes.default.svc / shop');
  assert.equal(healthy.revision, '0123456');
  assert.equal(healthy.autoSync, 'Automated · prune · self-heal');
  assert.deepEqual(healthy.attention, []);
  assert.equal(broken.source, 'charts.acme.io · billing@1.2.0');
  assert.equal(broken.revision, 'abcdef0');
  assert.equal(broken.autoSync, 'Manual');
  assert.equal(broken.project, 'default');
});

test('needs-attention collects every failing signal and the summary counts them', () => {
  assert.deepEqual(broken.attention, ['Out of sync', 'Degraded', 'Sync failed', 'Has conditions']);
  const summary = summarizeArgo([healthy, broken]);
  assert.deepEqual({ ...summary, attention: summary.attention.map((app) => app.name) }, { total: 2, synced: 1, outOfSync: 1, healthy: 1, degraded: 1, progressing: 0, attention: ['billing'] });
  assert.equal(argoTone('Degraded'), 'bad');
  assert.equal(argoTone('Unknown'), 'none');
});
