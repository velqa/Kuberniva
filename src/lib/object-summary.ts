/**
 * A short, kind-aware summary of a Kubernetes object for the topology details popup:
 * a few facts, its containers, and data keys. Secret values are never included, only
 * key names.
 */
type Manifest = Record<string, unknown>;
export type SummaryFact = { label: string; value: string };
export type SummaryContainer = { name: string; image: string; state: string; ready: boolean | null; restarts: number | null; init: boolean };
export type ObjectSummary = { facts: SummaryFact[]; containers: SummaryContainer[]; keys: string[]; labels: SummaryFact[] };

const record = (value: unknown): Manifest => (value && typeof value === 'object' && !Array.isArray(value) ? (value as Manifest) : {});
const list = (value: unknown): Manifest[] => (Array.isArray(value) ? value.map(record) : []);
const text = (value: unknown) => (value === undefined || value === null ? '' : String(value));
const pairs = (value: unknown) => Object.entries(record(value)).map(([key, item]) => `${key}=${text(item)}`).join(', ');

export function age(timestamp: string, now = Date.now()) {
  const time = Date.parse(timestamp);
  if (!Number.isFinite(time)) return '';
  const seconds = Math.max(0, Math.round((now - time) / 1000));
  if (seconds < 120) return `${seconds}s`;
  if (seconds < 7200) return `${Math.round(seconds / 60)}m`;
  if (seconds < 172800) return `${Math.round(seconds / 3600)}h`;
  return `${Math.round(seconds / 86400)}d`;
}

function containerState(status: Manifest) {
  const state = record(status.state);
  if (state.running) return 'Running';
  const waiting = record(state.waiting);
  if (Object.keys(waiting).length) return text(waiting.reason) || 'Waiting';
  const terminated = record(state.terminated);
  if (Object.keys(terminated).length) return text(terminated.reason) || 'Terminated';
  return '';
}

function templateContainers(spec: Manifest, statuses: Manifest[] = [], initStatuses: Manifest[] = []): SummaryContainer[] {
  const statusOf = (name: string, from: Manifest[]) => from.find((status) => status.name === name);
  const build = (container: Manifest, init: boolean): SummaryContainer => {
    const status = statusOf(text(container.name), init ? initStatuses : statuses);
    return {
      name: text(container.name),
      image: text(container.image),
      state: status ? containerState(status) : '',
      ready: status ? Boolean(status.ready) : null,
      restarts: status ? Number(status.restartCount || 0) : null,
      init,
    };
  };
  return [...list(spec.initContainers).map((container) => build(container, true)), ...list(spec.containers).map((container) => build(container, false))];
}

export function summarizeObject(kind: string, manifest: Manifest, now = Date.now()): ObjectSummary {
  const metadata = record(manifest.metadata);
  const spec = record(manifest.spec);
  const status = record(manifest.status);
  const facts: SummaryFact[] = [];
  const fact = (label: string, value: unknown) => {
    const shown = text(value);
    if (shown) facts.push({ label, value: shown });
  };
  let containers: SummaryContainer[] = [];
  let keys: string[] = [];
  const created = text(metadata.creationTimestamp);
  switch (kind) {
    case 'Pod': {
      fact('Phase', status.phase);
      fact('Node', spec.nodeName);
      fact('Pod IP', status.podIP);
      fact('QoS', status.qosClass);
      fact('Service account', spec.serviceAccountName);
      containers = templateContainers(spec, list(status.containerStatuses), list(status.initContainerStatuses));
      break;
    }
    case 'Deployment':
    case 'StatefulSet':
    case 'ReplicaSet': {
      fact('Replicas', `${Number(status.readyReplicas || 0)} ready / ${Number(spec.replicas ?? 1)} desired`);
      if (kind === 'Deployment') fact('Strategy', record(spec.strategy).type);
      if (kind === 'StatefulSet') fact('Service', spec.serviceName);
      fact('Selector', pairs(record(spec.selector).matchLabels));
      containers = templateContainers(record(record(spec.template).spec));
      break;
    }
    case 'DaemonSet': {
      fact('Pods', `${Number(status.numberReady || 0)} ready / ${Number(status.desiredNumberScheduled || 0)} scheduled`);
      fact('Selector', pairs(record(spec.selector).matchLabels));
      containers = templateContainers(record(record(spec.template).spec));
      break;
    }
    case 'Job': {
      fact('Completions', `${Number(status.succeeded || 0)} / ${Number(spec.completions ?? 1)}`);
      if (status.failed) fact('Failed', status.failed);
      fact('Started', status.startTime ? `${age(text(status.startTime), now)} ago` : '');
      containers = templateContainers(record(record(spec.template).spec));
      break;
    }
    case 'CronJob': {
      fact('Schedule', spec.schedule);
      fact('Suspended', spec.suspend ? 'Yes' : 'No');
      fact('Last run', status.lastScheduleTime ? `${age(text(status.lastScheduleTime), now)} ago` : 'Never');
      fact('Active jobs', list(status.active).length || '');
      containers = templateContainers(record(record(record(record(spec.jobTemplate).spec).template).spec));
      break;
    }
    case 'Service': {
      fact('Type', spec.type || 'ClusterIP');
      fact('Cluster IP', spec.clusterIP);
      const ingress = list(record(status.loadBalancer).ingress).map((item) => text(item.ip || item.hostname)).filter(Boolean);
      fact('External', [...ingress, ...(Array.isArray(spec.externalIPs) ? spec.externalIPs.map(text) : [])].join(', ') || spec.externalName);
      fact('Ports', list(spec.ports).map((port) => `${port.name ? `${text(port.name)} ` : ''}${text(port.port)}→${text(port.targetPort ?? port.port)}/${text(port.protocol || 'TCP')}${port.nodePort ? ` (node ${text(port.nodePort)})` : ''}`).join(', '));
      fact('Selector', pairs(spec.selector) || 'None');
      break;
    }
    case 'Ingress': {
      fact('Class', spec.ingressClassName);
      fact('Address', list(record(status.loadBalancer).ingress).map((item) => text(item.ip || item.hostname)).join(', '));
      for (const rule of list(spec.rules)) {
        for (const path of list(record(rule.http).paths)) {
          const service = record(record(path.backend).service);
          fact(`${text(rule.host) || '*'}${text(path.path) || '/'}`, `${text(service.name)}:${text(record(service.port).number || record(service.port).name)}`);
        }
      }
      fact('TLS', list(spec.tls).map((item) => text(item.secretName)).filter(Boolean).join(', '));
      break;
    }
    case 'PersistentVolumeClaim': {
      fact('Status', status.phase);
      fact('Capacity', record(status.capacity).storage || record(record(spec.resources).requests).storage);
      fact('Storage class', spec.storageClassName);
      fact('Access modes', Array.isArray(spec.accessModes) ? spec.accessModes.join(', ') : '');
      fact('Volume', spec.volumeName);
      break;
    }
    case 'ConfigMap':
    case 'Secret': {
      if (kind === 'Secret') fact('Type', manifest.type);
      keys = [...Object.keys(record(manifest.data)), ...Object.keys(record(manifest.binaryData)), ...Object.keys(record(manifest.stringData))].sort();
      fact('Keys', keys.length);
      break;
    }
    case 'Gateway': {
      fact('Class', spec.gatewayClassName);
      fact('Listeners', list(spec.listeners).map((listener) => `${text(listener.name)} ${text(listener.protocol)}:${text(listener.port)}${listener.hostname ? ` ${text(listener.hostname)}` : ''}`).join(', '));
      fact('Addresses', list(status.addresses).map((address) => text(address.value)).join(', '));
      break;
    }
    case 'HTTPRoute':
    case 'GRPCRoute': {
      fact('Hostnames', Array.isArray(spec.hostnames) ? spec.hostnames.join(', ') : '');
      fact('Parents', list(spec.parentRefs).map((parent) => text(parent.name)).join(', '));
      fact('Backends', list(spec.rules).flatMap((rule) => list(rule.backendRefs)).map((backend) => `${text(backend.name)}${backend.port ? `:${text(backend.port)}` : ''}${backend.weight !== undefined ? ` (weight ${text(backend.weight)})` : ''}`).join(', '));
      break;
    }
  }
  if (created) fact('Age', age(created, now));
  const owner = list(metadata.ownerReferences)[0];
  if (owner) fact('Owner', `${text(owner.kind)} ${text(owner.name)}`);
  const labels = Object.entries(record(metadata.labels)).map(([label, value]) => ({ label, value: text(value) }));
  return { facts, containers, keys, labels };
}

/** The manifest as shown in the popup: Secret values are replaced so they never render. */
export function redactSecret(kind: string, manifest: Manifest): Manifest {
  if (kind !== 'Secret') return manifest;
  const hide = (value: unknown) => Object.fromEntries(Object.keys(record(value)).map((key) => [key, '••••••']));
  const copy: Manifest = { ...manifest };
  for (const field of ['data', 'stringData', 'binaryData']) if (copy[field]) copy[field] = hide(copy[field]);
  const metadata = record(copy.metadata);
  const annotations = { ...record(metadata.annotations) };
  delete annotations['kubectl.kubernetes.io/last-applied-configuration'];
  copy.metadata = { ...metadata, annotations };
  return copy;
}
