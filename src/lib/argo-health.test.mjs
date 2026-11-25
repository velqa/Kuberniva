import assert from 'node:assert/strict';
import test from 'node:test';
import { analyzeGraph, matchesFilter, podFacts } from './argo-health.ts';

const nodes = [
  { id: 'app', parent: null, kind: 'Application', name: 'billing', health: 'Degraded', sync: 'OutOfSync' },
  { id: 'deploy', parent: 'app', kind: 'Deployment', name: 'billing-api', health: 'Healthy', sync: 'Synced', managed: true },
  { id: 'cm', parent: 'app', kind: 'ConfigMap', name: 'settings', health: '', sync: 'OutOfSync', managed: true },
  { id: 'svc', parent: 'app', kind: 'Service', name: 'billing-api', health: 'Missing', sync: 'OutOfSync', managed: true },
  { id: 'rs', parent: 'deploy', kind: 'ReplicaSet', name: 'billing-api-6f9', health: 'Progressing', info: 'rev 16 · 2/3 ready' },
  { id: 'pod-ok', parent: 'rs', kind: 'Pod', name: 'billing-api-6f9-a', health: '', info: 'Running · 2/2 ready' },
  { id: 'pod-bad', parent: 'rs', kind: 'Pod', name: 'billing-api-6f9-b', health: '', info: 'CrashLoopBackOff · 0/2 ready · 12 restarts' },
];

test('a crashing Pod is degraded and shows on every ancestor, even a Deployment Argo calls healthy', () => {
  const { nodes: health } = analyzeGraph(nodes);
  assert.equal(health.get('pod-bad').health, 'Degraded');
  assert.equal(health.get('pod-bad').reason, 'CrashLoopBackOff');
  assert.deepEqual(health.get('pod-bad').pod, { state: 'CrashLoopBackOff', ready: 0, total: 2, restarts: 12 });
  assert.equal(health.get('pod-ok').health, 'Healthy');
  assert.equal(health.get('deploy').health, 'Healthy');
  assert.equal(health.get('deploy').rollup, 'Degraded');
  assert.equal(health.get('deploy').problemsBelow, 2);
  assert.equal(health.get('app').rollup, 'Degraded');
});

test('the summary counts health and sync, and lists failures worst first with the Pod before its owners', () => {
  const { summary } = analyzeGraph(nodes);
  assert.equal(summary.outOfSync, 2);
  assert.equal(summary.health.Degraded, 1);
  assert.deepEqual(summary.failures.map((failure) => failure.id), ['pod-bad', 'svc', 'rs']);
});

test('filters keep matching nodes and, for problems, the path that leads to them', () => {
  const { nodes: health } = analyzeGraph(nodes);
  assert.ok(matchesFilter('problems', nodes[1], health.get('deploy')));
  assert.ok(!matchesFilter('problems', nodes[2], health.get('cm')));
  assert.ok(matchesFilter('OutOfSync', nodes[2], health.get('cm')));
  assert.equal(podFacts('Succeeded · 0/1 ready').state, 'Succeeded');
});
