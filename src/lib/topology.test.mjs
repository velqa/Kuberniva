import assert from 'node:assert/strict';
import test from 'node:test';
import { collapsePods, connectedGroups, layoutTopology } from './topology.ts';

const node = (kind, name, namespace = 'web', health = 'Healthy') => ({ id: `${kind}/${namespace}/${name}`, kind, group: '', name, namespace, health, info: '' });
const edge = (from, to, relation) => ({ from: from.id, to: to.id, relation });

const ingress = node('Ingress', 'shop');
const service = node('Service', 'shop');
const deployment = node('Deployment', 'shop');
const replicaSet = node('ReplicaSet', 'shop-7d9');
const config = node('ConfigMap', 'settings');
const pods = Array.from({ length: 6 }, (_, index) => node('Pod', `shop-7d9-${index}`, 'web', index === 5 ? 'Degraded' : 'Healthy'));
const lonely = node('Service', 'legacy', 'tools', 'Degraded');
const topology = {
  nodes: [ingress, service, deployment, replicaSet, config, ...pods, lonely],
  edges: [edge(ingress, service, 'routes'), edge(service, deployment, 'selects'), edge(deployment, replicaSet, 'owns'), edge(deployment, config, 'uses'), ...pods.map((pod) => edge(replicaSet, pod, 'owns'))],
};

test('objects are layered left to right along routes, selectors, and owners', () => {
  const layout = layoutTopology(topology);
  const layer = Object.fromEntries(layout.nodes.map((item) => [item.id, item.layer]));
  assert.deepEqual([ingress, service, deployment, replicaSet, config, pods[5]].map((item) => layer[item.id]), [0, 1, 2, 3, 3, 4]);
  assert.equal(layout.groups, 2);
  assert.deepEqual(layout.bands.map((band) => band.namespace), ['tools', 'web']);
  // Every edge points right, so the graph reads from traffic entry to Pods.
  const byId = new Map(layout.nodes.map((item) => [item.id, item]));
  for (const item of layout.edges) assert.ok(byId.get(item.from).x < byId.get(item.to).x);
});

test('long pod lists collapse but always keep failing pods visible', () => {
  const collapsed = collapsePods(topology, new Set(), 4);
  const names = collapsed.nodes.filter((item) => item.kind === 'Pod').map((item) => item.name);
  assert.ok(names.includes('shop-7d9-5'));
  assert.ok(names.includes('+3 pods'));
  assert.equal(collapsePods(topology, new Set([replicaSet.id]), 4).nodes.length, topology.nodes.length);
});

test('search and problems-only keep whole groups so context is not lost', () => {
  assert.equal(connectedGroups(topology.nodes, topology.edges).length, 2);
  const problems = layoutTopology(topology, { problemsOnly: true, expanded: new Set([replicaSet.id]) });
  assert.equal(problems.groups, 2);
  const search = layoutTopology(topology, { search: 'settings' });
  assert.equal(search.groups, 1);
  assert.ok(search.nodes.some((item) => item.kind === 'Ingress'));
});

test('a ConfigMap shared by two apps is drawn in both instead of merging them', () => {
  const other = node('Deployment', 'billing');
  const shared = { nodes: [...topology.nodes, other], edges: [...topology.edges, edge(other, config, 'uses')] };
  const layout = layoutTopology(shared, { expanded: new Set([replicaSet.id]) });
  assert.equal(layout.groups, 3);
  const copies = layout.nodes.filter((item) => item.name === 'settings');
  assert.equal(copies.length, 2);
  assert.ok(copies.every((item) => item.sharedBy === 2));
  assert.equal(new Set(layout.edges.map((item) => `${item.from}>${item.to}`)).size, layout.edges.length);
});
