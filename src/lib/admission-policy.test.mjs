import assert from 'node:assert/strict';
import { test } from 'node:test';
import { admissionPolicyView, isAdmissionPolicyKind, tokenizeCel } from './admission-policy.ts';

test('validating policies expose properties, rules, and every CEL section', () => {
  const view = admissionPolicyView({
    kind: 'ValidatingAdmissionPolicy',
    spec: {
      paramKind: { apiVersion: 'v1', kind: 'ConfigMap' },
      matchConstraints: { resourceRules: [{ apiGroups: ['apps', ''], apiVersions: ['v1'], operations: ['CREATE', 'UPDATE'], resources: ['deployments'] }] },
      matchConditions: [{ name: 'not-system', expression: "!object.metadata.namespace.startsWith('kube-')" }],
      variables: [{ name: 'replicas', expression: 'object.spec.replicas' }],
      validations: [{ expression: 'variables.replicas <= params.data.max', message: 'Too many replicas', reason: 'Invalid' }],
      auditAnnotations: [{ key: 'high-replicas', valueExpression: "'replicas: ' + string(variables.replicas)" }],
    },
    status: { typeChecking: { expressionWarnings: [{ fieldRef: 'spec.validations[0].expression', warning: 'no such key: max' }] } },
  });
  assert.deepEqual(view.properties, [{ label: 'Failure policy', value: 'Fail' }, { label: 'Parameter kind', value: 'ConfigMap · v1' }]);
  assert.deepEqual(view.resourceRules, ['CREATE, UPDATE on deployments (apps, core)']);
  assert.equal(view.matchConditions[0].title, 'not-system');
  assert.equal(view.variables[0].title, 'variables.replicas');
  assert.deepEqual(view.validations[0], { title: 'Invalid', expression: 'variables.replicas <= params.data.max', note: 'Too many replicas' });
  assert.equal(view.auditAnnotations[0].title, 'high-replicas');
  assert.deepEqual(view.warnings, ['spec.validations[0].expression: no such key: max']);
});

test('bindings show the policy, actions, parameters, and namespace selector', () => {
  const view = admissionPolicyView({
    kind: 'ValidatingAdmissionPolicyBinding',
    spec: { policyName: 'replica-limit', validationActions: ['Deny', 'Audit'], paramRef: { name: 'limits', namespace: 'policy', parameterNotFoundAction: 'Deny' }, matchResources: { namespaceSelector: { matchLabels: { env: 'prod' } } } },
  });
  assert.deepEqual(view.properties.map((fact) => [fact.label, fact.value]), [
    ['Policy', 'replica-limit'], ['Actions', 'Deny, Audit'], ['Parameters', 'policy/limits'], ['Missing parameters', 'Deny'], ['Namespaces', 'env=prod'],
  ]);
  assert.equal(isAdmissionPolicyKind('MutatingAdmissionPolicyBinding'), true);
  assert.equal(isAdmissionPolicyKind('ValidatingWebhookConfiguration'), false);
});

test('CEL tokens keep the full expression and classify its parts', () => {
  const expression = "has(object.spec) && self.size() <= 3 || 'x' in params";
  const tokens = tokenizeCel(expression);
  assert.equal(tokens.map((token) => token.text).join(''), expression);
  const kinds = Object.fromEntries(tokens.filter((token) => token.kind !== 'plain').map((token) => [token.text, token.kind]));
  assert.equal(kinds.has, 'keyword');
  assert.equal(kinds.object, 'variable');
  assert.equal(kinds.size, 'function');
  assert.equal(kinds["'x'"], 'string');
  assert.equal(kinds['3'], 'number');
  assert.equal(kinds['&&'], 'operator');
});
