import assert from 'node:assert/strict';
import test from 'node:test';
import { redactSecret, summarizeObject } from './object-summary.ts';

const now = Date.parse('2026-10-08T12:00:00Z');

test('a Pod summary shows where it runs and why its containers are not ready', () => {
  const summary = summarizeObject('Pod', {
    metadata: { creationTimestamp: '2026-10-08T10:00:00Z', labels: { app: 'shop' }, ownerReferences: [{ kind: 'ReplicaSet', name: 'shop-7d9' }] },
    spec: { nodeName: 'node-a', containers: [{ name: 'app', image: 'shop:1.4' }] },
    status: { phase: 'Running', podIP: '10.0.0.7', containerStatuses: [{ name: 'app', ready: false, restartCount: 14, state: { waiting: { reason: 'CrashLoopBackOff' } } }] },
  }, now);
  const facts = Object.fromEntries(summary.facts.map((item) => [item.label, item.value]));
  assert.equal(facts.Node, 'node-a');
  assert.equal(facts.Age, '2h');
  assert.equal(facts.Owner, 'ReplicaSet shop-7d9');
  assert.deepEqual(summary.containers, [{ name: 'app', image: 'shop:1.4', state: 'CrashLoopBackOff', ready: false, restarts: 14, init: false }]);
  assert.deepEqual(summary.labels, [{ label: 'app', value: 'shop' }]);
});

test('Services list their ports and selector', () => {
  const summary = summarizeObject('Service', { spec: { clusterIP: '10.96.0.10', selector: { app: 'shop' }, ports: [{ name: 'http', port: 80, targetPort: 8080 }] } }, now);
  const facts = Object.fromEntries(summary.facts.map((item) => [item.label, item.value]));
  assert.equal(facts.Ports, 'http 80→8080/TCP');
  assert.equal(facts.Selector, 'app=shop');
});

test('Secrets show key names only, and their manifest is redacted', () => {
  const secret = { type: 'Opaque', metadata: { name: 'db', annotations: { 'kubectl.kubernetes.io/last-applied-configuration': '{"data":{"password":"c2VjcmV0"}}' } }, data: { password: 'c2VjcmV0', user: 'YWRtaW4=' } };
  assert.deepEqual(summarizeObject('Secret', secret, now).keys, ['password', 'user']);
  const shown = JSON.stringify(redactSecret('Secret', secret));
  assert.ok(!shown.includes('c2VjcmV0'));
  assert.ok(!shown.includes('YWRtaW4='));
  assert.ok(shown.includes('password'));
});
