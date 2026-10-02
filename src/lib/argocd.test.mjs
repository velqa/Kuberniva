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

import { argoApplicationSet, argoProject, isForbidden } from './argocd.ts';

test('application sets summarize generators, template, and generation errors', () => {
  const owned = argoApplication({ metadata: { name: 'guestbook-prod', namespace: 'argocd', ownerReferences: [{ kind: 'ApplicationSet', name: 'guestbook' }] }, spec: { project: 'web' } });
  assert.equal(owned.owner, 'guestbook');
  const set = argoApplicationSet({
    metadata: { name: 'guestbook', namespace: 'argocd' },
    spec: { generators: [{ clusters: {} }, { git: {} }], template: { metadata: { name: '{{name}}-guestbook' }, spec: { project: 'web' } }, syncPolicy: { applicationsSync: 'create-only' } },
    status: { conditions: [{ type: 'ErrorOccurred', status: 'True', message: 'repo not found' }, { type: 'ResourcesUpToDate', status: 'False' }] },
  }, [owned]);
  assert.deepEqual(set.generators, ['clusters', 'git']);
  assert.equal(set.template, '{{name}}-guestbook');
  assert.equal(set.policy, 'create-only');
  assert.equal(set.generated, 1);
  assert.deepEqual(set.attention, ['repo not found']);
  assert.equal(set.conditions[0].ok, false);
});

test('projects list sources, destinations, and their applications', () => {
  const project = argoProject({
    metadata: { name: 'web', namespace: 'argocd' },
    spec: { description: 'Web team', sourceRepos: ['https://github.com/acme/*'], destinations: [{ server: 'https://kubernetes.default.svc', namespace: 'web-*' }], roles: [{ name: 'ci' }] },
  }, [argoApplication({ metadata: { name: 'a' }, spec: { project: 'web' } }), argoApplication({ metadata: { name: 'b' }, spec: { project: 'other' } })]);
  assert.deepEqual(project.destinations, ['kubernetes.default.svc / web-*']);
  assert.equal(project.apps, 1);
  assert.equal(project.roles, 1);
  assert.equal(isForbidden('applications.argoproj.io is forbidden: User "dev" cannot list resource'), true);
  assert.equal(isForbidden('connection refused'), false);
});

import { resourcesByKind } from './argocd.ts';

test('applications expose sources, history, last sync results, and ordered resources', () => {
  const app = argoApplication({
    metadata: { name: 'shop', namespace: 'argocd', creationTimestamp: '2026-09-01T00:00:00Z' },
    spec: {
      project: 'web',
      source: { repoURL: 'https://github.com/acme/charts.git', chart: 'shop', targetRevision: '1.4.0', helm: { releaseName: 'shop', valueFiles: ['values-prod.yaml'], parameters: [{ name: 'replicas', value: '3' }] } },
      destination: { server: 'https://kubernetes.default.svc', namespace: 'shop' },
      syncPolicy: { automated: { selfHeal: true }, syncOptions: ['CreateNamespace=true'] },
    },
    status: {
      summary: { images: ['ghcr.io/acme/shop:1.4.0'], externalURLs: ['https://shop.example.com'] },
      resources: [
        { kind: 'Deployment', group: 'apps', version: 'v1', name: 'shop', namespace: 'shop', status: 'Synced', health: { status: 'Degraded', message: 'Deployment exceeded its progress deadline' } },
        { kind: 'ConfigMap', version: 'v1', name: 'shop-config', namespace: 'shop', status: 'OutOfSync', requiresPruning: true },
        { kind: 'Service', version: 'v1', name: 'shop', namespace: 'shop', status: 'Synced', health: { status: 'Healthy' } },
      ],
      history: [{ id: 1, revision: '1.3.0', deployedAt: '2026-09-02T00:00:00Z', initiatedBy: { automated: true } }, { id: 2, revision: '1.4.0', deployedAt: '2026-09-03T00:00:00Z', initiatedBy: { username: 'vijay' } }],
      operationState: { phase: 'Failed', message: 'one or more objects failed to apply', startedAt: '2026-09-03T00:00:00Z', finishedAt: '2026-09-03T00:00:20Z', operation: { initiatedBy: { username: 'vijay' }, sync: { revision: '1.4.0', prune: true } }, syncResult: { revision: '1.4.0', resources: [{ kind: 'Deployment', name: 'shop', namespace: 'shop', status: 'SyncFailed', message: 'admission webhook denied the request', syncPhase: 'Sync' }] } },
    },
  });
  assert.deepEqual(app.sources[0], { repoURL: 'https://github.com/acme/charts.git', repo: 'github.com/acme/charts', path: '', chart: 'shop', targetRevision: '1.4.0', type: 'Helm', parameters: [{ name: 'releaseName', value: 'shop' }, { name: 'replicas', value: '3' }], valueFiles: ['values-prod.yaml'] });
  assert.equal(app.inCluster, true);
  assert.deepEqual(app.automated, { prune: false, selfHeal: true });
  assert.deepEqual(app.syncOptions, ['CreateNamespace=true']);
  assert.deepEqual(app.history.map((entry) => [entry.id, entry.initiatedBy]), [[2, 'vijay'], [1, 'Auto-sync']]);
  assert.equal(app.lastOperation.results[0].status, 'SyncFailed');
  assert.equal(app.lastOperation.prune, true);
  assert.deepEqual(app.resources.map((resource) => resource.kind), ['ConfigMap', 'Service', 'Deployment']);
  assert.equal(app.resources[0].requiresPruning, true);
  assert.equal(app.resources[2].healthMessage, 'Deployment exceeded its progress deadline');
  assert.deepEqual(resourcesByKind(app.resources).map(([kind, items]) => [kind, items.length]), [['ConfigMap', 1], ['Service', 1], ['Deployment', 1]]);
  assert.equal(argoApplication({ spec: { destination: { name: 'prod' } } }).inCluster, false);
});
