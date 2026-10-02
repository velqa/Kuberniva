/** Structured views of ValidatingAdmissionPolicy / MutatingAdmissionPolicy and their bindings. */

type Manifest = Record<string, unknown>;
const record = (value: unknown): Manifest => (value && typeof value === 'object' && !Array.isArray(value) ? value as Manifest : {});
const list = (value: unknown): Manifest[] => (Array.isArray(value) ? value.map(record) : []);
const text = (value: unknown) => (typeof value === 'string' || typeof value === 'number' || typeof value === 'boolean' ? String(value) : '');
const strings = (value: unknown) => (Array.isArray(value) ? value.map(text).filter(Boolean) : []);

export type PolicyFact = { label: string; value: string };
export type CelEntry = { title: string; expression: string; note: string };
export type PolicyView = {
  properties: PolicyFact[];
  resourceRules: string[];
  matchConditions: CelEntry[];
  variables: CelEntry[];
  validations: CelEntry[];
  mutations: CelEntry[];
  auditAnnotations: CelEntry[];
  warnings: string[];
};

function selectorSummary(selector: unknown) {
  const labels = Object.entries(record(record(selector).matchLabels)).map(([key, value]) => `${key}=${text(value)}`);
  const expressions = list(record(selector).matchExpressions).map((expression) => `${text(expression.key)} ${text(expression.operator)} ${strings(expression.values).join(',')}`.trim());
  return [...labels, ...expressions].join(', ');
}

export function isAdmissionPolicyKind(kind: string) {
  return /^(Validating|Mutating)AdmissionPolicy(Binding)?$/.test(kind);
}

export function admissionPolicyView(manifest: Manifest | null): PolicyView {
  const spec = record(manifest?.spec);
  const kind = text(manifest?.kind);
  const binding = kind.endsWith('Binding');
  const properties: PolicyFact[] = [];
  const add = (label: string, value: string) => { if (value) properties.push({ label, value }); };
  if (binding) {
    add('Policy', text(spec.policyName));
    add('Actions', strings(spec.validationActions).join(', '));
    const paramRef = record(spec.paramRef);
    add('Parameters', [text(paramRef.namespace), text(paramRef.name)].filter(Boolean).join('/') || selectorSummary(paramRef.selector));
    add('Missing parameters', text(paramRef.parameterNotFoundAction));
  } else {
    add('Failure policy', text(spec.failurePolicy) || 'Fail');
    const paramKind = record(spec.paramKind);
    add('Parameter kind', [text(paramKind.kind), text(paramKind.apiVersion)].filter(Boolean).join(' · '));
    add('Reinvocation', text(spec.reinvocationPolicy));
  }
  const match = record(binding ? spec.matchResources : spec.matchConstraints);
  add('Match policy', text(match.matchPolicy));
  add('Namespaces', selectorSummary(match.namespaceSelector));
  add('Objects', selectorSummary(match.objectSelector));
  const resourceRules = list(match.resourceRules).map((rule) => {
    const groups = (Array.isArray(rule.apiGroups) ? rule.apiGroups.map(text) : []).map((group) => group || 'core');
    return `${strings(rule.operations).join(', ') || '*'} on ${strings(rule.resources).join(', ') || '*'}${groups.length ? ` (${groups.join(', ')})` : ''}`;
  });
  const warnings = list(record(record(manifest?.status).typeChecking).expressionWarnings)
    .map((warning) => `${text(warning.fieldRef)}: ${text(warning.warning)}`);
  return {
    properties,
    resourceRules,
    matchConditions: list(spec.matchConditions).map((condition) => ({ title: text(condition.name), expression: text(condition.expression), note: '' })),
    variables: list(spec.variables).map((variable) => ({ title: `variables.${text(variable.name)}`, expression: text(variable.expression), note: '' })),
    validations: list(spec.validations).map((validation, index) => ({
      title: text(validation.reason) || `Rule ${index + 1}`,
      expression: text(validation.expression),
      note: text(validation.message) || (text(validation.messageExpression) ? `message: ${text(validation.messageExpression)}` : ''),
    })),
    mutations: list(spec.mutations).map((mutation, index) => ({
      title: text(mutation.patchType) || `Mutation ${index + 1}`,
      expression: text(record(mutation.applyConfiguration).expression) || text(record(mutation.jsonPatch).expression),
      note: '',
    })),
    auditAnnotations: list(spec.auditAnnotations).map((annotation) => ({ title: text(annotation.key), expression: text(annotation.valueExpression), note: '' })),
    warnings,
  };
}

export type CelToken = { text: string; kind: 'keyword' | 'variable' | 'string' | 'number' | 'function' | 'operator' | 'plain' };
const CEL_KEYWORDS = new Set(['true', 'false', 'null', 'in', 'has', 'all', 'exists', 'exists_one', 'map', 'filter']);
const CEL_VARIABLES = new Set(['self', 'object', 'oldObject', 'params', 'request', 'variables', 'namespaceObject', 'authorizer']);
const CEL_PATTERN = /("(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*')|(\b\d+(?:\.\d+)?\b)|([A-Za-z_][\w]*)|(&&|\|\||==|!=|<=|>=|[!<>?:+\-*/%])|(\s+|.)/gs;

/** Splits a CEL expression into tokens for theme-aware highlighting. */
export function tokenizeCel(expression: string): CelToken[] {
  const tokens: CelToken[] = [];
  for (const match of expression.matchAll(CEL_PATTERN)) {
    const [value, quoted, number, identifier, operator] = match;
    const next = expression.slice((match.index || 0) + value.length).trimStart();
    const kind: CelToken['kind'] = quoted ? 'string'
      : number ? 'number'
        : identifier ? (CEL_VARIABLES.has(identifier) ? 'variable' : CEL_KEYWORDS.has(identifier) ? 'keyword' : next.startsWith('(') ? 'function' : 'plain')
          : operator ? 'operator' : 'plain';
    const previous = tokens[tokens.length - 1];
    if (kind === 'plain' && previous?.kind === 'plain') previous.text += value;
    else tokens.push({ text: value, kind });
  }
  return tokens;
}
