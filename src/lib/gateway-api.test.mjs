import assert from 'node:assert/strict';
import { test } from 'node:test';
import { gatewayListeners, routeParents, routeRules, routesAttachedToGateway, routesTargetingService } from './gateway-api.ts';

const gateway = {
  metadata: { name: 'public', namespace: 'infra' },
  spec: { gatewayClassName: 'istio', listeners: [
    { name: 'https', protocol: 'HTTPS', port: 443, hostname: '*.example.com', tls: { mode: 'Terminate' }, allowedRoutes: { namespaces: { from: 'All' } } },
    { name: 'http', protocol: 'HTTP', port: 80 },
  ] },
  status: { listeners: [{ name: 'https', attachedRoutes: 2, conditions: [{ type: 'Programmed', status: 'True' }] }] },
};

const route = {
  kind: 'HTTPRoute',
  metadata: { name: 'shop', namespace: 'apps' },
  spec: {
    parentRefs: [{ name: 'public', namespace: 'infra', sectionName: 'https' }],
    hostnames: ['shop.example.com'],
    rules: [
      { matches: [{ path: { type: 'PathPrefix', value: '/api' }, method: 'GET', headers: [{ name: 'x-canary', value: '1' }] }], backendRefs: [{ name: 'api', port: 8080, weight: 90 }, { name: 'api-canary', namespace: 'canary', port: 8080, weight: 10 }] },
      { backendRefs: [{ name: 'web', port: 80 }], filters: [{ type: 'RequestHeaderModifier' }] },
    ],
  },
  status: { parents: [{ parentRef: { name: 'public', namespace: 'infra', sectionName: 'https' }, conditions: [{ type: 'Accepted', status: 'True' }, { type: 'ResolvedRefs', status: 'False', reason: 'BackendNotFound' }] }] },
};

test('gateway listeners merge spec with status and keep unreported listeners unknown', () => {
  const [https, http] = gatewayListeners(gateway);
  assert.deepEqual(https, { name: 'https', protocol: 'HTTPS', port: '443', hostname: '*.example.com', tls: 'Terminate', allowedFrom: 'All', attachedRoutes: 2, ok: true });
  assert.equal(http.hostname, '*');
  assert.equal(http.allowedFrom, 'Same');
  assert.equal(http.attachedRoutes, null);
  assert.equal(http.ok, null);
});

test('route rules describe matches, filters, weights, and cross-namespace backends', () => {
  const [first, second] = routeRules(route);
  assert.deepEqual(first.matches, ['PathPrefix /api · GET · header x-canary=1']);
  assert.deepEqual(first.backends, [{ label: 'api:8080', weight: '90' }, { label: 'canary/api-canary:8080', weight: '10' }]);
  assert.deepEqual(second.matches, ['All requests']);
  assert.deepEqual(second.filters, ['RequestHeaderModifier']);
});

test('route parents carry their own status conditions', () => {
  const [parent] = routeParents(route);
  assert.equal(parent.label, 'infra/public');
  assert.equal(parent.section, 'https');
  assert.deepEqual(parent.conditions.map((condition) => [condition.type, condition.ok]), [['Accepted', true], ['ResolvedRefs', false]]);
});

test('attached routes and service routes apply Gateway API namespace defaults', () => {
  const sameNamespace = { kind: 'HTTPRoute', metadata: { name: 'internal', namespace: 'infra' }, spec: { parentRefs: [{ name: 'public' }], rules: [] } };
  const otherGateway = { kind: 'HTTPRoute', metadata: { name: 'other', namespace: 'apps' }, spec: { parentRefs: [{ name: 'public' }] } };
  assert.deepEqual(routesAttachedToGateway([route, sameNamespace, otherGateway], 'public', 'infra').map((r) => r.name), ['shop', 'internal']);
  assert.deepEqual(routesTargetingService([route], 'api', 'apps').map((r) => r.name), ['shop']);
  assert.deepEqual(routesTargetingService([route], 'api-canary', 'canary').map((r) => r.name), ['shop']);
  assert.deepEqual(routesTargetingService([route], 'api', 'canary'), []);
});
