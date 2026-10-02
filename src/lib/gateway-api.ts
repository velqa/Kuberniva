/** Structured views of Gateway API objects, read from their manifests. */

type Manifest = Record<string, unknown>;
const GATEWAY_GROUP = 'gateway.networking.k8s.io';

const record = (value: unknown): Manifest => (value && typeof value === 'object' && !Array.isArray(value) ? value as Manifest : {});
const list = (value: unknown): Manifest[] => (Array.isArray(value) ? value.map(record) : []);
const text = (value: unknown) => (typeof value === 'string' || typeof value === 'number' ? String(value) : '');

export type ConditionView = { type: string; ok: boolean; reason: string; message: string };
export type ListenerView = { name: string; protocol: string; port: string; hostname: string; tls: string; allowedFrom: string; attachedRoutes: number | null; ok: boolean | null };
export type BackendView = { label: string; weight: string };
export type RuleView = { matches: string[]; filters: string[]; backends: BackendView[] };
export type ParentView = { label: string; section: string; conditions: ConditionView[] };
export type RouteSummary = { kind: string; name: string; namespace: string; hostnames: string[]; parents: string[] };

export function conditions(value: unknown): ConditionView[] {
  return list(value).map((condition) => ({
    type: text(condition.type),
    ok: text(condition.status) === 'True',
    reason: text(condition.reason),
    message: text(condition.message),
  })).filter((condition) => condition.type);
}

export function gatewayListeners(manifest: Manifest | null): ListenerView[] {
  const spec = record(manifest?.spec);
  const statuses = new Map(list(record(manifest?.status).listeners).map((status) => [text(status.name), status]));
  return list(spec.listeners).map((listener) => {
    const status = statuses.get(text(listener.name));
    const listenerConditions = conditions(status?.conditions);
    const programmed = listenerConditions.find((condition) => condition.type === 'Programmed' || condition.type === 'Ready');
    return {
      name: text(listener.name),
      protocol: text(listener.protocol),
      port: text(listener.port),
      hostname: text(listener.hostname) || '*',
      tls: text(record(listener.tls).mode),
      allowedFrom: text(record(record(listener.allowedRoutes).namespaces).from) || 'Same',
      attachedRoutes: typeof status?.attachedRoutes === 'number' ? status.attachedRoutes : null,
      ok: programmed ? programmed.ok : null,
    };
  });
}

export function gatewayAddresses(manifest: Manifest | null) {
  return list(record(manifest?.status).addresses).map((address) => text(address.value)).filter(Boolean);
}

function parentLabel(parent: Manifest, routeNamespace: string) {
  const kind = text(parent.kind) || 'Gateway';
  const namespace = text(parent.namespace) || routeNamespace;
  return `${kind === 'Gateway' ? '' : `${kind} `}${namespace ? `${namespace}/` : ''}${text(parent.name)}`;
}

export function routeHostnames(manifest: Manifest | null) {
  const hostnames = record(manifest?.spec).hostnames;
  return Array.isArray(hostnames) ? hostnames.map(text).filter(Boolean) : [];
}

export function routeParents(manifest: Manifest | null): ParentView[] {
  const namespace = text(record(manifest?.metadata).namespace);
  const statuses = list(record(manifest?.status).parents);
  return list(record(manifest?.spec).parentRefs).map((parent) => {
    const status = statuses.find((candidate) => {
      const ref = record(candidate.parentRef);
      return text(ref.name) === text(parent.name)
        && (text(ref.namespace) || namespace) === (text(parent.namespace) || namespace)
        && text(ref.sectionName) === text(parent.sectionName);
    });
    return { label: parentLabel(parent, namespace), section: text(parent.sectionName), conditions: conditions(status?.conditions) };
  });
}

function describeMatch(match: Manifest) {
  const parts: string[] = [];
  const path = record(match.path);
  if (text(path.value)) parts.push(`${text(path.type) || 'PathPrefix'} ${text(path.value)}`);
  if (text(match.method) && typeof match.method === 'string') parts.push(text(match.method));
  const grpc = record(match.method);
  if (text(grpc.service) || text(grpc.method)) parts.push(`${text(grpc.service) || '*'}/${text(grpc.method) || '*'}`);
  for (const header of list(match.headers)) parts.push(`header ${text(header.name)}${text(header.type) === 'RegularExpression' ? ' ~ ' : '='}${text(header.value)}`);
  for (const query of list(match.queryParams)) parts.push(`query ${text(query.name)}=${text(query.value)}`);
  return parts.join(' · ') || 'All requests';
}

export function routeRules(manifest: Manifest | null): RuleView[] {
  const namespace = text(record(manifest?.metadata).namespace);
  return list(record(manifest?.spec).rules).map((rule) => {
    const matches = list(rule.matches).map(describeMatch);
    return {
      matches: matches.length ? matches : ['All requests'],
      filters: list(rule.filters).map((filter) => text(filter.type)).filter(Boolean),
      backends: list(rule.backendRefs).map((backend) => {
        const kind = text(backend.kind) || 'Service';
        const backendNamespace = text(backend.namespace);
        const port = text(backend.port);
        return {
          label: `${kind === 'Service' ? '' : `${kind} `}${backendNamespace && backendNamespace !== namespace ? `${backendNamespace}/` : ''}${text(backend.name)}${port ? `:${port}` : ''}`,
          weight: text(backend.weight),
        };
      }),
    };
  });
}

function summarize(route: Manifest): RouteSummary {
  const metadata = record(route.metadata);
  const namespace = text(metadata.namespace);
  return {
    kind: text(route.kind),
    name: text(metadata.name),
    namespace,
    hostnames: routeHostnames(route),
    parents: list(record(route.spec).parentRefs).map((parent) => parentLabel(parent, namespace)),
  };
}

/** Routes whose parentRefs attach to this Gateway (group, kind, and namespace defaults applied). */
export function routesAttachedToGateway(routes: Manifest[], gatewayName: string, gatewayNamespace: string): RouteSummary[] {
  return routes.filter((route) => {
    const namespace = text(record(route.metadata).namespace);
    return list(record(route.spec).parentRefs).some((parent) =>
      (text(parent.group) || GATEWAY_GROUP) === GATEWAY_GROUP
      && (text(parent.kind) || 'Gateway') === 'Gateway'
      && text(parent.name) === gatewayName
      && (text(parent.namespace) || namespace) === gatewayNamespace);
  }).map(summarize);
}

/** Routes that send traffic to this Service through any rule's backendRefs. */
export function routesTargetingService(routes: Manifest[], serviceName: string, serviceNamespace: string): RouteSummary[] {
  return routes.filter((route) => {
    const namespace = text(record(route.metadata).namespace);
    return list(record(route.spec).rules).some((rule) => list(rule.backendRefs).some((backend) =>
      (text(backend.group) || '') === ''
      && (text(backend.kind) || 'Service') === 'Service'
      && text(backend.name) === serviceName
      && (text(backend.namespace) || namespace) === serviceNamespace));
  }).map(summarize);
}
