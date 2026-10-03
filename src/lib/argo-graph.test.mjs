import assert from 'node:assert/strict';
import { test } from 'node:test';
import { DEFAULT_GRAPH_OPTIONS, layoutGraph } from './argo-graph.ts';

const app = { id: 'app', parent: null, kind: 'Application', name: 'shop' };
const nodes = [
  app,
  { id: 'svc', parent: 'app', kind: 'Service', name: 'shop' },
  { id: 'dep', parent: 'app', kind: 'Deployment', name: 'shop' },
  { id: 'rs', parent: 'dep', kind: 'ReplicaSet', name: 'shop-7d' },
  { id: 'p1', parent: 'rs', kind: 'Pod', name: 'shop-7d-a' },
  { id: 'p2', parent: 'rs', kind: 'Pod', name: 'shop-7d-b' },
];

test('columns follow depth and parents centre on their children', () => {
  const { nodes: placed, edges, width } = layoutGraph(nodes);
  const at = Object.fromEntries(placed.map((node) => [node.id, node]));
  const step = DEFAULT_GRAPH_OPTIONS.columnWidth + DEFAULT_GRAPH_OPTIONS.columnGap;
  assert.deepEqual([at.app.x, at.dep.x, at.rs.x, at.p1.x], [0, step, step * 2, step * 3]);
  assert.equal(at.rs.y, (at.p1.y + at.p2.y) / 2);
  assert.ok(at.svc.y < at.dep.y, 'siblings keep their order');
  assert.equal(edges.length, 5);
  assert.equal(width, 4 * DEFAULT_GRAPH_OPTIONS.columnWidth + 3 * DEFAULT_GRAPH_OPTIONS.columnGap);
});

test('long sibling lists collapse into a "+N more" node until expanded', () => {
  const many = [app, { id: 'rs', parent: 'app', kind: 'ReplicaSet', name: 'rs' }, ...Array.from({ length: 20 }, (_, index) => ({ id: `p${index}`, parent: 'rs', kind: 'Pod', name: `pod-${index}` }))];
  const collapsed = layoutGraph(many).nodes.filter((node) => node.parent === 'rs');
  assert.equal(collapsed.length, DEFAULT_GRAPH_OPTIONS.maxChildren);
  assert.equal(collapsed.at(-1).more, 20 - (DEFAULT_GRAPH_OPTIONS.maxChildren - 1));
  assert.equal(layoutGraph(many, new Set(['rs'])).nodes.filter((node) => node.parent === 'rs').length, 20);
});

test('the application root never collapses its managed resources', () => {
  const managed = [app, ...Array.from({ length: 12 }, (_, index) => ({ id: `r${index}`, parent: 'app', kind: index % 2 ? 'Service' : 'ConfigMap', name: `r${index}` }))];
  assert.equal(layoutGraph(managed).nodes.filter((node) => node.parent === 'app').length, 12);
});
