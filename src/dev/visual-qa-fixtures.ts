export const visualQaCluster = {
  id: 'visual-qa-cluster',
  name: 'qa-production-west',
  provider: 'Kubernetes',
  status: 'Connected',
  tone: 'green',
  authMethod: 'OIDC',
  namespace: 'platform',
  kubeconfigPath: '/tmp/kuberniva-visual-qa.yaml',
};

/** Read-only transport for exercising sleep recovery without any real cluster. */
export async function readVisualQaRequest(command: string, args: Record<string, unknown>, stall = false): Promise<unknown> {
  if (stall) return new Promise(() => {});
  await new Promise((resolve) => setTimeout(resolve, 550));
  if (command === 'discover_cluster_catalog') return { context: visualQaCluster.name, namespaces: ['platform', 'payments'], resources: visualQaResources };
  if (command === 'check_resource_permissions') {
    const checks = (args.request as { checks: { key: string }[] }).checks;
    return checks.map((check) => ({ key: check.key, allowed: true, denied: false }));
  }
  if (command === 'list_resource_objects') {
    const request = args.request as { kind: string };
    return request.kind === 'Pod' ? visualQaPods : request.kind === 'Deployment' ? visualQaDeployments : visualQaConfigMaps;
  }
  if (command === 'read_cluster_overview') return visualQaOverview;
  if (command === 'read_cluster_events') return visualQaPodEvents;
  throw new Error(`Unsupported visual QA read: ${command}`);
}

export function installRecoveryControl(container: HTMLElement, onResume: () => void) {
  const button = document.createElement('button');
  button.type = 'button';
  button.textContent = 'Simulate one hour away';
  button.addEventListener('click', onResume);
  container.append(button);
  return () => {
    button.removeEventListener('click', onResume);
    button.remove();
  };
}

export const visualQaFavoriteClusters = [
  visualQaCluster,
  { ...visualQaCluster, id: 'visual-qa-staging', name: 'payments-staging', status: 'Not connected', tone: 'gray', namespace: 'payments' },
  { ...visualQaCluster, id: 'visual-qa-observability', name: 'observability-prod', status: 'Not connected', tone: 'gray' },
  { ...visualQaCluster, id: 'visual-qa-sandbox', name: 'developer-sandbox', status: 'Not connected', tone: 'gray' },
];

export const visualQaResources = [
  { group: 'apps', version: 'v1', apiVersion: 'apps/v1', kind: 'Deployment', plural: 'deployments', namespaced: true, category: 'Workloads', custom: false, crd: false },
  { group: '', version: 'v1', apiVersion: 'v1', kind: 'Pod', plural: 'pods', namespaced: true, category: 'Workloads', custom: false, crd: false },
  { group: '', version: 'v1', apiVersion: 'v1', kind: 'ConfigMap', plural: 'configmaps', namespaced: true, category: 'Configuration', custom: false, crd: false },
  { group: '', version: 'v1', apiVersion: 'v1', kind: 'Secret', plural: 'secrets', namespaced: true, category: 'Configuration', custom: false, crd: false },
  { group: '', version: 'v1', apiVersion: 'v1', kind: 'Service', plural: 'services', namespaced: true, category: 'Network', custom: false, crd: false },
  { group: 'gateway.networking.k8s.io', version: 'v1', apiVersion: 'gateway.networking.k8s.io/v1', kind: 'HTTPRoute', plural: 'httproutes', namespaced: true, category: 'Gateway APIs', custom: true, crd: true },
  { group: '', version: 'v1', apiVersion: 'v1', kind: 'Namespace', plural: 'namespaces', namespaced: false, category: 'Cluster', custom: false, crd: false },
  { group: 'platform.example.io', version: 'v1beta1', apiVersion: 'platform.example.io/v1beta1', kind: 'TenantPolicy', plural: 'tenantpolicies', namespaced: true, category: 'Custom Resources', custom: true, crd: true },
];

// A realistic production-sized catalog for designing the resource directory.
export const visualQaLargeCatalog = [
  { group: '', version: 'v1', apiVersion: 'v1', kind: 'LimitRange', plural: 'limitranges', namespaced: true, category: 'Configuration', custom: false, crd: false },
  { group: '', version: 'v1', apiVersion: 'v1', kind: 'ResourceQuota', plural: 'resourcequotas', namespaced: true, category: 'Configuration', custom: false, crd: false },
  { group: 'scheduling.k8s.io', version: 'v1', apiVersion: 'scheduling.k8s.io/v1', kind: 'PriorityClass', plural: 'priorityclasses', namespaced: false, category: 'Configuration', custom: false, crd: false },
  { group: 'node.k8s.io', version: 'v1', apiVersion: 'node.k8s.io/v1', kind: 'RuntimeClass', plural: 'runtimeclasses', namespaced: false, category: 'Configuration', custom: false, crd: false },
  { group: 'policy', version: 'v1', apiVersion: 'policy/v1', kind: 'PodDisruptionBudget', plural: 'poddisruptionbudgets', namespaced: true, category: 'Configuration', custom: false, crd: false },
  { group: '', version: 'v1', apiVersion: 'v1', kind: 'ServiceAccount', plural: 'serviceaccounts', namespaced: true, category: 'Access Control', custom: false, crd: false },
  { group: 'rbac.authorization.k8s.io', version: 'v1', apiVersion: 'rbac.authorization.k8s.io/v1', kind: 'Role', plural: 'roles', namespaced: true, category: 'Access Control', custom: false, crd: false },
  { group: 'rbac.authorization.k8s.io', version: 'v1', apiVersion: 'rbac.authorization.k8s.io/v1', kind: 'RoleBinding', plural: 'rolebindings', namespaced: true, category: 'Access Control', custom: false, crd: false },
  { group: 'rbac.authorization.k8s.io', version: 'v1', apiVersion: 'rbac.authorization.k8s.io/v1', kind: 'ClusterRole', plural: 'clusterroles', namespaced: false, category: 'Access Control', custom: false, crd: false },
  { group: 'rbac.authorization.k8s.io', version: 'v1', apiVersion: 'rbac.authorization.k8s.io/v1', kind: 'ClusterRoleBinding', plural: 'clusterrolebindings', namespaced: false, category: 'Access Control', custom: false, crd: false },
  { group: 'coordination.k8s.io', version: 'v1', apiVersion: 'coordination.k8s.io/v1', kind: 'Lease', plural: 'leases', namespaced: true, category: 'Access Control', custom: false, crd: false },
  { group: 'networking.k8s.io', version: 'v1', apiVersion: 'networking.k8s.io/v1', kind: 'Ingress', plural: 'ingresses', namespaced: true, category: 'Network', custom: false, crd: false },
  { group: 'networking.k8s.io', version: 'v1', apiVersion: 'networking.k8s.io/v1', kind: 'IngressClass', plural: 'ingressclasses', namespaced: false, category: 'Network', custom: false, crd: false },
  { group: 'networking.k8s.io', version: 'v1', apiVersion: 'networking.k8s.io/v1', kind: 'NetworkPolicy', plural: 'networkpolicies', namespaced: true, category: 'Network', custom: false, crd: false },
  { group: '', version: 'v1', apiVersion: 'v1', kind: 'Endpoints', plural: 'endpoints', namespaced: true, category: 'Network', custom: false, crd: false },
  { group: 'discovery.k8s.io', version: 'v1', apiVersion: 'discovery.k8s.io/v1', kind: 'EndpointSlice', plural: 'endpointslices', namespaced: true, category: 'Network', custom: false, crd: false },
  { group: 'gateway.networking.k8s.io', version: 'v1', apiVersion: 'gateway.networking.k8s.io/v1', kind: 'Gateway', plural: 'gateways', namespaced: true, category: 'Gateway APIs', custom: true, crd: true },
  { group: 'gateway.networking.k8s.io', version: 'v1', apiVersion: 'gateway.networking.k8s.io/v1', kind: 'GatewayClass', plural: 'gatewayclasses', namespaced: false, category: 'Gateway APIs', custom: true, crd: true },
  { group: 'gateway.networking.k8s.io', version: 'v1', apiVersion: 'gateway.networking.k8s.io/v1', kind: 'GRPCRoute', plural: 'grpcroutes', namespaced: true, category: 'Gateway APIs', custom: true, crd: true },
  { group: 'gateway.networking.k8s.io', version: 'v1beta1', apiVersion: 'gateway.networking.k8s.io/v1beta1', kind: 'ReferenceGrant', plural: 'referencegrants', namespaced: true, category: 'Gateway APIs', custom: true, crd: true },
  { group: '', version: 'v1', apiVersion: 'v1', kind: 'PersistentVolumeClaim', plural: 'persistentvolumeclaims', namespaced: true, category: 'Storage', custom: false, crd: false },
  { group: '', version: 'v1', apiVersion: 'v1', kind: 'PersistentVolume', plural: 'persistentvolumes', namespaced: false, category: 'Storage', custom: false, crd: false },
  { group: 'storage.k8s.io', version: 'v1', apiVersion: 'storage.k8s.io/v1', kind: 'StorageClass', plural: 'storageclasses', namespaced: false, category: 'Storage', custom: false, crd: false },
  { group: 'storage.k8s.io', version: 'v1', apiVersion: 'storage.k8s.io/v1', kind: 'VolumeAttachment', plural: 'volumeattachments', namespaced: false, category: 'Storage', custom: false, crd: false },
  { group: 'storage.k8s.io', version: 'v1', apiVersion: 'storage.k8s.io/v1', kind: 'CSIDriver', plural: 'csidrivers', namespaced: false, category: 'Storage', custom: false, crd: false },
  { group: 'storage.k8s.io', version: 'v1', apiVersion: 'storage.k8s.io/v1', kind: 'CSINode', plural: 'csinodes', namespaced: false, category: 'Storage', custom: false, crd: false },
  { group: 'snapshot.storage.k8s.io', version: 'v1', apiVersion: 'snapshot.storage.k8s.io/v1', kind: 'VolumeSnapshot', plural: 'volumesnapshots', namespaced: true, category: 'Storage', custom: false, crd: false },
  { group: '', version: 'v1', apiVersion: 'v1', kind: 'Node', plural: 'nodes', namespaced: false, category: 'Cluster', custom: false, crd: false },
  { group: '', version: 'v1', apiVersion: 'v1', kind: 'Event', plural: 'events', namespaced: true, category: 'Cluster', custom: false, crd: false },
  { group: 'apiextensions.k8s.io', version: 'v1', apiVersion: 'apiextensions.k8s.io/v1', kind: 'CustomResourceDefinition', plural: 'customresourcedefinitions', namespaced: false, category: 'Cluster', custom: false, crd: false },
  { group: 'admissionregistration.k8s.io', version: 'v1', apiVersion: 'admissionregistration.k8s.io/v1', kind: 'MutatingWebhookConfiguration', plural: 'mutatingwebhookconfigurations', namespaced: false, category: 'Cluster', custom: false, crd: false },
  { group: 'admissionregistration.k8s.io', version: 'v1', apiVersion: 'admissionregistration.k8s.io/v1', kind: 'ValidatingWebhookConfiguration', plural: 'validatingwebhookconfigurations', namespaced: false, category: 'Cluster', custom: false, crd: false },
  { group: 'apiregistration.k8s.io', version: 'v1', apiVersion: 'apiregistration.k8s.io/v1', kind: 'APIService', plural: 'apiservices', namespaced: false, category: 'Cluster', custom: false, crd: false },
  { group: 'certificates.k8s.io', version: 'v1', apiVersion: 'certificates.k8s.io/v1', kind: 'CertificateSigningRequest', plural: 'certificatesigningrequests', namespaced: false, category: 'Cluster', custom: false, crd: false },
  { group: 'flowcontrol.apiserver.k8s.io', version: 'v1', apiVersion: 'flowcontrol.apiserver.k8s.io/v1', kind: 'FlowSchema', plural: 'flowschemas', namespaced: false, category: 'Cluster', custom: false, crd: false },
  { group: 'flowcontrol.apiserver.k8s.io', version: 'v1', apiVersion: 'flowcontrol.apiserver.k8s.io/v1', kind: 'PriorityLevelConfiguration', plural: 'prioritylevelconfigurations', namespaced: false, category: 'Cluster', custom: false, crd: false },
  { group: 'cert-manager.io', version: 'v1', apiVersion: 'cert-manager.io/v1', kind: 'Certificate', plural: 'certificates', namespaced: true, category: 'Custom Resources', custom: true, crd: true },
  { group: 'cert-manager.io', version: 'v1', apiVersion: 'cert-manager.io/v1', kind: 'Issuer', plural: 'issuers', namespaced: true, category: 'Custom Resources', custom: true, crd: true },
  { group: 'cert-manager.io', version: 'v1', apiVersion: 'cert-manager.io/v1', kind: 'ClusterIssuer', plural: 'clusterissuers', namespaced: false, category: 'Custom Resources', custom: true, crd: true },
  { group: 'monitoring.coreos.com', version: 'v1', apiVersion: 'monitoring.coreos.com/v1', kind: 'ServiceMonitor', plural: 'servicemonitors', namespaced: true, category: 'Custom Resources', custom: true, crd: true },
  { group: 'monitoring.coreos.com', version: 'v1', apiVersion: 'monitoring.coreos.com/v1', kind: 'PrometheusRule', plural: 'prometheusrules', namespaced: true, category: 'Custom Resources', custom: true, crd: true },
  { group: 'monitoring.coreos.com', version: 'v1', apiVersion: 'monitoring.coreos.com/v1', kind: 'PodMonitor', plural: 'podmonitors', namespaced: true, category: 'Custom Resources', custom: true, crd: true },
  { group: 'argoproj.io', version: 'v1alpha1', apiVersion: 'argoproj.io/v1alpha1', kind: 'Application', plural: 'applications', namespaced: true, category: 'Custom Resources', custom: true, crd: true },
  { group: 'argoproj.io', version: 'v1alpha1', apiVersion: 'argoproj.io/v1alpha1', kind: 'AppProject', plural: 'appprojects', namespaced: true, category: 'Custom Resources', custom: true, crd: true },
  { group: 'external-secrets.io', version: 'v1beta1', apiVersion: 'external-secrets.io/v1beta1', kind: 'ExternalSecret', plural: 'externalsecrets', namespaced: true, category: 'Custom Resources', custom: true, crd: true },
  { group: 'external-secrets.io', version: 'v1beta1', apiVersion: 'external-secrets.io/v1beta1', kind: 'ClusterSecretStore', plural: 'clustersecretstores', namespaced: false, category: 'Custom Resources', custom: true, crd: true },
  { group: 'keda.sh', version: 'v1alpha1', apiVersion: 'keda.sh/v1alpha1', kind: 'ScaledObject', plural: 'scaledobjects', namespaced: true, category: 'Custom Resources', custom: true, crd: true },
  { group: 'velero.io', version: 'v1', apiVersion: 'velero.io/v1', kind: 'Backup', plural: 'backups', namespaced: true, category: 'Custom Resources', custom: true, crd: true },
];

export const visualQaPods = [
  ['api-7d8496f6d9-2wkd8', 'Running', '2/2', '0.042 cores', '184Mi', '18m'],
  ['api-7d8496f6d9-jg6h4', 'Running', '2/2', '0.038 cores', '176Mi', '18m'],
  ['worker-6ff5d79c75-8zq7t', 'Running', '1/1', '0.116 cores', '412Mi', '2h'],
  ['worker-6ff5d79c75-wf5kp', 'Running', '1/1', '0.093 cores', '396Mi', '2h'],
  ['scheduler-5fc7d47868-p6c9m', 'Running', '1/1', '0.018 cores', '128Mi', '1d'],
  ['metrics-7d6c6bb7fc-f4kz2', 'Pending', '0/1', '0 cores', '0Mi', '42s'],
].map(([name, status, ready, cpuUsage, memoryUsage, age], index) => {
  const [readyContainers, totalContainers] = ready.split('/').map(Number);
  const ageMinutes = age.endsWith('m') ? Number(age.slice(0, -1)) : age.endsWith('h') ? Number(age.slice(0, -1)) * 60 : age.endsWith('d') ? Number(age.slice(0, -1)) * 1440 : 1;
  return {
    name,
    namespace: 'platform',
    uid: `visual-pod-${index + 1}`,
    resourceVersion: `${4100 + index}`,
    createdAt: new Date(Date.now() - ageMinutes * 60_000).toISOString(),
    status,
    readyContainers,
    totalContainers,
    restarts: index === 2 ? 1 : 0,
    cpuUsage,
    memoryUsage,
    nodeName: `worker-${(index % 2) + 1}`,
  };
});

export const visualQaDeployments = [
  { name: 'api', status: 'Available', readyContainers: 3, totalContainers: 3, ageMinutes: 180 },
  { name: 'worker', status: 'Available', readyContainers: 4, totalContainers: 4, ageMinutes: 420 },
  { name: 'scheduler', status: 'Progressing', readyContainers: 1, totalContainers: 2, ageMinutes: 55 },
].map((deployment, index) => ({
  name: deployment.name,
  namespace: 'platform',
  uid: `visual-deployment-${index + 1}`,
  resourceVersion: `${4600 + index}`,
  createdAt: new Date(Date.now() - deployment.ageMinutes * 60_000).toISOString(),
  status: deployment.status,
  readyContainers: deployment.readyContainers,
  totalContainers: deployment.totalContainers,
}));

export const visualQaWorkloadManifest = {
  apiVersion: 'apps/v1',
  kind: 'Deployment',
  metadata: { name: 'api', namespace: 'platform', labels: { app: 'api', tier: 'backend' } },
  spec: {
    replicas: 3,
    template: {
      metadata: { labels: { app: 'api', tier: 'backend' } },
      spec: {
        imagePullSecrets: [{ name: 'registry-credentials' }],
        containers: [{
          name: 'api',
          image: 'example.invalid/platform/api:3.8.2',
          envFrom: [{ configMapRef: { name: 'api-settings' } }, { secretRef: { name: 'api-credentials' } }],
          volumeMounts: [
            { name: 'configuration', mountPath: '/etc/platform', readOnly: true },
            { name: 'credentials', mountPath: '/var/run/secrets/platform', readOnly: true },
            { name: 'cache', mountPath: '/var/cache/platform' },
          ],
        }],
        volumes: [
          { name: 'configuration', configMap: { name: 'api-settings' } },
          { name: 'credentials', secret: { secretName: 'api-credentials' } },
          { name: 'cache', emptyDir: {} },
          { name: 'uploads', persistentVolumeClaim: { claimName: 'api-uploads' } },
        ],
      },
    },
  },
  status: { readyReplicas: 3, availableReplicas: 3 },
};

export const visualQaWorkloadYaml = `apiVersion: apps/v1
kind: Deployment
metadata:
  name: api
  namespace: platform
spec:
  replicas: 3
  template:
    spec:
      containers:
        - name: api
          image: example.invalid/platform/api:3.8.2
          volumeMounts:
            - name: configuration
              mountPath: /etc/platform
      volumes:
        - name: configuration
          configMap:
            name: api-settings
        - name: uploads
          persistentVolumeClaim:
            claimName: api-uploads
`;

export const visualQaPodManifest = {
  apiVersion: 'v1',
  kind: 'Pod',
  metadata: { name: 'api-7d8496f6d9-2wkd8', namespace: 'platform', labels: { app: 'api' } },
  spec: {
    containers: [{ name: 'api', image: 'example.invalid/platform/api:3.8.2' }],
  },
  status: {
    phase: 'Pending',
    conditions: [
      { type: 'Initialized', status: 'True', reason: 'PodCompleted' },
      { type: 'PodScheduled', status: 'True' },
      { type: 'ContainersReady', status: 'False', reason: 'ContainersNotReady', message: 'containers with unready status: [api]' },
      { type: 'Ready', status: 'False', reason: 'ContainersNotReady' },
    ],
    containerStatuses: [{
      name: 'api',
      image: 'example.invalid/platform/api:3.8.2',
      ready: false,
      restartCount: 4,
      state: { waiting: { reason: 'ImagePullBackOff', message: 'Back-off pulling image example.invalid/platform/api:3.8.2' } },
    }],
  },
};

// A realistic mix of cluster events for designing the Events page.
const eventAt = (secondsAgo: number) => new Date(Date.now() - secondsAgo * 1000).toISOString();
export const visualQaClusterEvents = [
  { name: 'e1', namespace: 'platform', eventType: 'Warning', reason: 'BackOff', message: 'Back-off pulling image "example.invalid/platform/api:3.8.2"', involvedKind: 'Pod', involvedName: 'api-7d8496f6d9-2wkd8', count: 14, source: 'kubelet', lastObserved: eventAt(20) },
  { name: 'e2', namespace: 'platform', eventType: 'Warning', reason: 'Failed', message: 'Failed to pull image "example.invalid/platform/api:3.8.2": manifest unknown', involvedKind: 'Pod', involvedName: 'api-7d8496f6d9-2wkd8', count: 14, source: 'kubelet', lastObserved: eventAt(25) },
  { name: 'e3', namespace: 'platform', eventType: 'Normal', reason: 'Pulling', message: 'Pulling image "example.invalid/platform/api:3.8.2"', involvedKind: 'Pod', involvedName: 'api-7d8496f6d9-2wkd8', count: 15, source: 'kubelet', lastObserved: eventAt(30) },
  { name: 'e4', namespace: 'payments', eventType: 'Warning', reason: 'Unhealthy', message: 'Readiness probe failed: HTTP probe failed with statuscode: 503', involvedKind: 'Pod', involvedName: 'payments-worker-6c9d7-xk2lp', count: 6, source: 'kubelet', lastObserved: eventAt(70) },
  { name: 'e5', namespace: 'platform', eventType: 'Normal', reason: 'ScalingReplicaSet', message: 'Scaled up replica set api-7d8496f6d9 to 3', involvedKind: 'Deployment', involvedName: 'api', count: 1, source: 'deployment-controller', lastObserved: eventAt(140) },
  { name: 'e6', namespace: 'platform', eventType: 'Normal', reason: 'SuccessfulCreate', message: 'Created pod: api-7d8496f6d9-jg6h4', involvedKind: 'ReplicaSet', involvedName: 'api-7d8496f6d9', count: 1, source: 'replicaset-controller', lastObserved: eventAt(145) },
  { name: 'e7', namespace: '', eventType: 'Warning', reason: 'NodeNotReady', message: 'Node worker-b-2 status is now: NodeNotReady', involvedKind: 'Node', involvedName: 'worker-b-2', count: 1, source: 'node-controller', lastObserved: eventAt(320) },
  { name: 'e8', namespace: 'platform', eventType: 'Normal', reason: 'Scheduled', message: 'Successfully assigned platform/api-7d8496f6d9-jg6h4 to worker-2', involvedKind: 'Pod', involvedName: 'api-7d8496f6d9-jg6h4', count: 1, source: 'default-scheduler', lastObserved: eventAt(150) },
  { name: 'e9', namespace: 'platform', eventType: 'Normal', reason: 'Started', message: 'Started container api', involvedKind: 'Pod', involvedName: 'api-7d8496f6d9-jg6h4', count: 1, source: 'kubelet', lastObserved: eventAt(130) },
  { name: 'e10', namespace: 'payments', eventType: 'Warning', reason: 'FailedScheduling', message: '0/8 nodes are available: 1 node(s) were unschedulable, 7 Insufficient memory. preemption: 0/8 nodes are available.', involvedKind: 'Pod', involvedName: 'payments-batch-29381-qz8v4', count: 3, source: 'default-scheduler', lastObserved: eventAt(410) },
  { name: 'e11', namespace: 'platform', eventType: 'Normal', reason: 'Killing', message: 'Stopping container worker', involvedKind: 'Pod', involvedName: 'worker-6ff5d79c75-8zq7t', count: 1, source: 'kubelet', lastObserved: eventAt(900) },
  { name: 'e12', namespace: 'platform', eventType: 'Normal', reason: 'Pulled', message: 'Successfully pulled image "example.invalid/platform/worker:1.4.0" in 1.204s', involvedKind: 'Pod', involvedName: 'worker-6ff5d79c75-wf5kp', count: 1, source: 'kubelet', lastObserved: eventAt(1100) },
  { name: 'e13', namespace: 'observability', eventType: 'Normal', reason: 'SuccessfulRescale', message: 'New size: 4; reason: cpu resource utilization (percentage of request) above target', involvedKind: 'HorizontalPodAutoscaler', involvedName: 'collector', count: 2, source: 'horizontal-pod-autoscaler', lastObserved: eventAt(1500) },
  { name: 'e14', namespace: 'platform', eventType: 'Normal', reason: 'Sync', message: 'Scheduled for sync', involvedKind: 'Ingress', involvedName: 'api-public', count: 3, source: 'nginx-ingress-controller', lastObserved: eventAt(2400) },
];

export const visualQaPodEvents = [
  { name: 'api-pull', namespace: 'platform', eventType: 'Normal', reason: 'Pulling', message: 'Pulling image "example.invalid/platform/api:3.8.2"', involvedKind: 'Pod', involvedName: 'api-7d8496f6d9-2wkd8', count: 1, lastObserved: new Date(Date.now() - 90_000).toISOString() },
  { name: 'api-failed', namespace: 'platform', eventType: 'Warning', reason: 'Failed', message: 'Failed to pull image: manifest unknown', involvedKind: 'Pod', involvedName: 'api-7d8496f6d9-2wkd8', count: 4, lastObserved: new Date(Date.now() - 45_000).toISOString() },
  { name: 'api-backoff', namespace: 'platform', eventType: 'Warning', reason: 'BackOff', message: 'Back-off pulling image "example.invalid/platform/api:3.8.2"', involvedKind: 'Pod', involvedName: 'api-7d8496f6d9-2wkd8', count: 4, lastObserved: new Date().toISOString() },
];

export const visualQaLogLines = [
  '2026-08-19T05:10:21.105Z INFO server listening on :8080',
  '2026-08-19T05:10:24.410Z INFO request completed method=GET path=/health status=200 duration=3ms',
  '2026-08-19T05:10:29.028Z WARN cache miss key=tenant-settings',
  '2026-08-19T05:10:29.041Z INFO database query completed duration=12ms',
  '2026-08-19T05:10:34.991Z ERROR upstream timeout service=payments attempt=1',
  '2026-08-19T05:10:35.112Z INFO retry succeeded service=payments attempt=2',
  '2026-08-19T05:10:39.440Z INFO request completed method=POST path=/v1/jobs status=202 duration=46ms',
];

export const visualQaConfigMaps = [
  'api-settings',
  'feature-flags',
  'gateway-routing',
  'logging-config',
  'worker-environment',
].map((name, index) => ({
  name,
  namespace: 'platform',
  uid: `visual-config-${index + 1}`,
  resourceVersion: `${5100 + index}`,
  createdAt: new Date(Date.now() - (index + 1) * 3_600_000).toISOString(),
}));

export const visualQaCustomObjects = ['payments-policy', 'platform-default', 'sandbox-policy'].map((name, index) => ({
  name,
  namespace: 'platform',
  uid: `visual-policy-${index + 1}`,
  resourceVersion: `${6100 + index}`,
  createdAt: new Date(Date.now() - (index + 2) * 3_600_000).toISOString(),
}));

export const visualQaConfigValues = {
  'APP_MODE': 'production',
  'FEATURE_FLAGS': 'newNavigation=true\nbulkActions=true\ncompactOverview=true',
  'pod-template.yaml': 'apiVersion: v1\nkind: Pod\nmetadata:\n  labels:\n    app: worker\nspec:\n  serviceAccountName: platform-worker\n  containers:\n    - name: worker\n      image: example.invalid/worker:2.4.1',
  'retention.json': '{\n  "logs": "14d",\n  "events": "7d",\n  "snapshots": "30d"\n}',
};

export const visualQaLargeConfigValues = {
  AIRFLOW_HOME: '/opt/airflow',
  CCLOUD_S3_BUCKET_NAME: 'airflow-platform-archive',
  CCLOUD_S3_ENDPOINT_URL: 'https://rgw.example.internal',
  CCLOUD_S3_REGION: 'eu-de-2',
  DEPLOYMENT: 'platform-production',
  DOMAIN: 'project.example',
  GITHUB_OAUTH_ACCESS_URL: 'https://github.example/login/oauth/access_token',
  GITHUB_OAUTH_AUTHORIZE_URL: 'https://github.example/login/oauth/authorize',
  LOG_LEVEL: 'INFO',
  MAX_ACTIVE_RUNS: '24',
  METRICS_ENABLED: 'true',
  OTEL_EXPORTER_OTLP_ENDPOINT: 'http://telemetry-collector:4317',
  PARALLELISM: '64',
  POSTGRES_DATABASE: 'airflow',
  POSTGRES_HOST: 'postgres-rw.platform.svc',
  POSTGRES_PORT: '5432',
  REDIS_HOST: 'redis-master.platform.svc',
  REDIS_PORT: '6379',
  SCHEDULER_HEARTBEAT_SEC: '5',
  SMTP_HOST: 'mailrelay.platform.svc',
  WEBSERVER_BASE_URL: 'https://airflow.example',
  WORKER_CONCURRENCY: '16',
};

export const visualQaSecretValues = {
  username: 'cGxhdGZvcm0tYWRtaW4=',
  password: 'dmlzdWFsLXFhLW9ubHk=',
  'tls.crt': 'LS0tLS1CRUdJTiBDRVJUSUZJQ0FURS0tLS0tXG5WSVNVQUwtUUEtQ0VSVElGSUNBVEVcbi0tLS0tRU5EIENFUlRJRklDQVRFLS0tLS0=',
};

const nodeBase = {
  ready: true,
  roles: ['worker'],
  labels: [{ key: 'kubernetes.io/arch', value: 'arm64' }],
  annotations: [],
  conditions: [{ type: 'Ready', status: 'True', reason: 'KubeletReady', message: 'kubelet is posting ready status' }],
  taints: [],
  architecture: 'arm64',
  operatingSystem: 'linux',
  osImage: 'Ubuntu 24.04 LTS',
  kernelVersion: '6.8.0',
  kubeletVersion: 'v1.32.4',
  containerRuntimeVersion: 'containerd://2.0.3',
  podCidrs: ['10.42.0.0/24'],
  unschedulable: false,
  capacity: [{ key: 'cpu', value: '8 cores' }, { key: 'memory', value: '32Gi' }, { key: 'pods', value: '110' }],
  allocatable: [{ key: 'cpu', value: '7.8 cores' }, { key: 'memory', value: '30Gi' }, { key: 'pods', value: '110' }],
};

// Eight nodes with mixed health and load, for designing overview layouts at a realistic scale.
const largeNodeSpecs: [string, string[], number, number, Record<string, unknown>][] = [
  ['control-plane-1', ['control-plane'], 22, 41, {}],
  ['control-plane-2', ['control-plane'], 18, 38, {}],
  ['worker-a-1', ['worker'], 64, 71, {}],
  ['worker-a-2', ['worker'], 81, 88, { conditions: [{ type: 'Ready', status: 'True', reason: 'KubeletReady', message: 'kubelet is posting ready status' }, { type: 'MemoryPressure', status: 'True', reason: 'KubeletHasInsufficientMemory', message: 'kubelet has insufficient memory available' }] }],
  ['worker-b-1', ['worker'], 37, 52, {}],
  ['worker-b-2', ['worker'], 0, 0, { ready: false, conditions: [{ type: 'Ready', status: 'Unknown', reason: 'NodeStatusUnknown', message: 'Kubelet stopped posting node status.' }] }],
  ['worker-gpu-1', ['worker', 'gpu'], 46, 63, {}],
  ['worker-spot-1', ['worker', 'spot'], 12, 24, { unschedulable: true, taints: [{ key: 'node.kubernetes.io/unschedulable', effect: 'NoSchedule' }] }],
];
export const visualQaLargeOverview = {
  nodes: largeNodeSpecs.map(([name, roles, cpu, memory, extra], index) => ({
    ...nodeBase,
    ...extra,
    name,
    roles,
    uid: `visual-large-node-${index}`,
    addresses: [{ type: 'InternalIP', address: `10.0.20.${11 + index}` }],
    providerId: `qa://${name}`,
    creationTimestamp: new Date(Date.now() - (30 - index * 3) * 86_400_000).toISOString(),
    cpuCapacity: '8 cores',
    memoryCapacity: '32Gi',
    cpuUsage: cpu ? `${(cpu * 0.08).toFixed(2)} cores` : undefined,
    memoryUsage: memory ? `${(memory * 0.32).toFixed(1)}Gi` : undefined,
    cpuUsagePercent: cpu || undefined,
    memoryUsagePercent: memory || undefined,
  })),
  totals: { cpuCapacity: '64 cores', memoryCapacity: '256Gi', storageCapacity: '1.9Ti', cpuUsage: '28.0 cores', memoryUsage: '121.6Gi', cpuUsagePercent: 44, memoryUsagePercent: 48, metricNodes: 7 },
  metricsAvailable: true,
  observedAt: new Date().toISOString(),
};

export const visualQaOverview = {
  nodes: [
    { ...nodeBase, name: 'worker-1', uid: 'visual-node-1', addresses: [{ type: 'InternalIP', address: '10.0.12.21' }], providerId: 'qa://worker-1', creationTimestamp: new Date(Date.now() - 14 * 86_400_000).toISOString(), cpuCapacity: '8', memoryCapacity: '32863332Ki', cpuUsage: '3123456789n', memoryUsage: '15309824Ki', cpuUsagePercent: 39.0432098625, memoryUsagePercent: 46.5864537 },
    { ...nodeBase, name: 'worker-2', uid: 'visual-node-2', addresses: [{ type: 'InternalIP', address: '10.0.12.22' }], providerId: 'qa://worker-2', creationTimestamp: new Date(Date.now() - 14 * 86_400_000).toISOString(), cpuCapacity: '8', memoryCapacity: '32863332Ki', cpuUsage: '178901234n', memoryUsage: '11744052Ki', cpuUsagePercent: 2.2362654250, memoryUsagePercent: 35.7360112 },
  ],
  totals: { cpuCapacity: '16 cores', memoryCapacity: '64Gi', storageCapacity: '480Gi', cpuUsage: '3.30 cores', memoryUsage: '25.8Gi', cpuUsagePercent: 20.6397376438, memoryUsagePercent: 41.1612324, metricNodes: 2 },
  metricsAvailable: true,
  observedAt: new Date().toISOString(),
};

/** 3,000 Pods across 30 namespaces, for measuring large-cluster rendering. */
export const visualQaLargePods = Array.from({ length: 3_000 }, (_, index) => {
  const statuses = ['Running', 'Running', 'Running', 'Running', 'Running', 'Running', 'Running', 'Pending', 'CrashLoopBackOff', 'Completed'];
  const status = statuses[index % statuses.length];
  const totalContainers = (index % 3) + 1;
  return {
    name: `service-${String(Math.floor(index / 4)).padStart(4, '0')}-7d8496f6d9-${(index * 7919).toString(36).slice(-5)}`,
    namespace: `team-${String(index % 30).padStart(2, '0')}`,
    uid: `visual-large-pod-${index}`,
    resourceVersion: `${90_000 + index}`,
    createdAt: new Date(Date.now() - ((index % 600) + 1) * 60_000).toISOString(),
    status,
    readyContainers: status === 'Running' ? totalContainers : 0,
    totalContainers,
    restarts: status === 'CrashLoopBackOff' ? 12 : index % 17 === 0 ? 1 : 0,
    cpuUsage: `${((index % 50) / 100).toFixed(3)} cores`,
    memoryUsage: `${64 + (index % 40) * 16}Mi`,
    nodeName: `worker-${(index % 24) + 1}`,
  };
});

export const visualQaExtraResources = [
  { group: 'gateway.networking.k8s.io', version: 'v1', apiVersion: 'gateway.networking.k8s.io/v1', kind: 'Gateway', plural: 'gateways', namespaced: true, category: 'Gateway APIs', custom: true, crd: true },
  { group: 'admissionregistration.k8s.io', version: 'v1', apiVersion: 'admissionregistration.k8s.io/v1', kind: 'ValidatingAdmissionPolicy', plural: 'validatingadmissionpolicies', namespaced: false, category: 'Admission Policies', custom: false, crd: false },
  { group: 'admissionregistration.k8s.io', version: 'v1', apiVersion: 'admissionregistration.k8s.io/v1', kind: 'ValidatingAdmissionPolicyBinding', plural: 'validatingadmissionpolicybindings', namespaced: false, category: 'Admission Policies', custom: false, crd: false },
  { group: 'argoproj.io', version: 'v1alpha1', apiVersion: 'argoproj.io/v1alpha1', kind: 'Application', plural: 'applications', namespaced: true, category: 'Custom Resources', custom: true, crd: true },
  { group: 'argoproj.io', version: 'v1alpha1', apiVersion: 'argoproj.io/v1alpha1', kind: 'ApplicationSet', plural: 'applicationsets', namespaced: true, category: 'Custom Resources', custom: true, crd: true },
  { group: 'argoproj.io', version: 'v1alpha1', apiVersion: 'argoproj.io/v1alpha1', kind: 'AppProject', plural: 'appprojects', namespaced: true, category: 'Custom Resources', custom: true, crd: true },
];

export const visualQaGateway = {
  apiVersion: 'gateway.networking.k8s.io/v1', kind: 'Gateway',
  metadata: { name: 'public-edge', namespace: 'platform' },
  spec: { gatewayClassName: 'istio', listeners: [
    { name: 'https', protocol: 'HTTPS', port: 443, hostname: '*.shop.example.com', tls: { mode: 'Terminate' }, allowedRoutes: { namespaces: { from: 'All' } } },
    { name: 'http-redirect', protocol: 'HTTP', port: 80, allowedRoutes: { namespaces: { from: 'Same' } } },
    { name: 'grpc', protocol: 'HTTPS', port: 8443, hostname: 'api.shop.example.com', tls: { mode: 'Terminate' } },
  ] },
  status: {
    addresses: [{ type: 'IPAddress', value: '34.120.18.44' }],
    conditions: [{ type: 'Accepted', status: 'True' }, { type: 'Programmed', status: 'True' }],
    listeners: [
      { name: 'https', attachedRoutes: 3, conditions: [{ type: 'Programmed', status: 'True' }] },
      { name: 'http-redirect', attachedRoutes: 1, conditions: [{ type: 'Programmed', status: 'True' }] },
      { name: 'grpc', attachedRoutes: 0, conditions: [{ type: 'Programmed', status: 'False', reason: 'InvalidCertificateRef' }] },
    ],
  },
};

export const visualQaGatewayRoutes = [
  { kind: 'HTTPRoute', name: 'storefront', namespace: 'payments', hostnames: ['www.shop.example.com'], parents: ['platform/public-edge'] },
  { kind: 'HTTPRoute', name: 'checkout-api', namespace: 'payments', hostnames: ['api.shop.example.com'], parents: ['platform/public-edge'] },
  { kind: 'HTTPRoute', name: 'status-page', namespace: 'platform', hostnames: [], parents: ['platform/public-edge'] },
];

export const visualQaHttpRoute = {
  apiVersion: 'gateway.networking.k8s.io/v1', kind: 'HTTPRoute',
  metadata: { name: 'checkout-api', namespace: 'platform' },
  spec: {
    parentRefs: [{ name: 'public-edge', namespace: 'platform', sectionName: 'https' }],
    hostnames: ['api.shop.example.com', 'checkout.shop.example.com'],
    rules: [
      { matches: [{ path: { type: 'PathPrefix', value: '/v2/checkout' }, method: 'POST' }], backendRefs: [{ name: 'checkout', port: 8080, weight: 90 }, { name: 'checkout-canary', port: 8080, weight: 10 }] },
      { matches: [{ path: { type: 'PathPrefix', value: '/v2' }, headers: [{ name: 'x-tenant', value: 'beta' }] }], filters: [{ type: 'RequestHeaderModifier' }], backendRefs: [{ name: 'api-beta', namespace: 'beta', port: 8080 }] },
      { backendRefs: [{ name: 'api', port: 8080 }] },
    ],
  },
  status: { parents: [{ parentRef: { name: 'public-edge', namespace: 'platform', sectionName: 'https' }, conditions: [{ type: 'Accepted', status: 'True' }, { type: 'ResolvedRefs', status: 'False', reason: 'RefNotPermitted', message: 'ReferenceGrant missing for beta/api-beta' }] }] },
};

export const visualQaAdmissionPolicy = {
  apiVersion: 'admissionregistration.k8s.io/v1', kind: 'ValidatingAdmissionPolicy',
  metadata: { name: 'replica-limits' },
  spec: {
    failurePolicy: 'Fail',
    paramKind: { apiVersion: 'v1', kind: 'ConfigMap' },
    matchConstraints: { resourceRules: [{ apiGroups: ['apps'], apiVersions: ['v1'], operations: ['CREATE', 'UPDATE'], resources: ['deployments', 'statefulsets'] }] },
    matchConditions: [{ name: 'exclude-system', expression: "!object.metadata.namespace.startsWith('kube-')" }],
    variables: [{ name: 'maxReplicas', expression: "int(params.data['maxReplicas'])" }],
    validations: [
      { expression: 'object.spec.replicas <= variables.maxReplicas', messageExpression: "'replicas must be at most ' + string(variables.maxReplicas)", reason: 'Invalid' },
      { expression: "has(object.metadata.labels) && 'team' in object.metadata.labels", message: 'Every workload needs a team label', reason: 'Forbidden' },
    ],
    auditAnnotations: [{ key: 'high-replica-count', valueExpression: "object.spec.replicas > 10 ? 'replicas: ' + string(object.spec.replicas) : null" }],
  },
  status: { typeChecking: { expressionWarnings: [{ fieldRef: 'spec.validations[0].expression', warning: 'params may be absent when the binding omits paramRef' }] } },
};

const argoApp = (name: string, project: string, sync: string, health: string, phase: string, extra: Record<string, unknown> = {}) => {
  const minutesAgo = (minutes: number) => new Date(Date.now() - minutes * 60_000).toISOString();
  const failed = phase === 'Failed';
  return {
    metadata: { name, namespace: 'argocd', creationTimestamp: minutesAgo(60 * 24 * 40) },
    spec: {
      project,
      source: name.startsWith('billing')
        ? { repoURL: 'https://charts.acme.io', chart: 'billing', targetRevision: '2.8.1', helm: { releaseName: name, valueFiles: ['values-prod.yaml'], parameters: [{ name: 'replicaCount', value: '3' }, { name: 'image.tag', value: '2.8.1' }] } }
        : { repoURL: 'https://github.com/acme/deploy.git', path: `apps/${name}/overlays/prod`, targetRevision: 'main', kustomize: { images: [`ghcr.io/acme/${name}:4f2a91c`] } },
      destination: { server: 'https://kubernetes.default.svc', namespace: name },
      syncPolicy: name.startsWith('billing') ? { syncOptions: ['CreateNamespace=true', 'ServerSideApply=true'] } : { automated: { prune: true, selfHeal: true }, syncOptions: ['CreateNamespace=true'] },
    },
    status: {
      sync: { status: sync, revision: '4f2a91c0d1e2' },
      health: { status: health },
      reconciledAt: minutesAgo(4),
      summary: { images: [`ghcr.io/acme/${name}:4f2a91c`, 'ghcr.io/acme/otel-sidecar:0.9.2'], externalURLs: name === 'storefront' || name.startsWith('billing') ? [`https://${name}.acme.example.com`] : [] },
      operationState: {
        phase,
        message: failed ? 'one or more synchronization tasks completed unsuccessfully' : phase === 'Running' ? 'waiting for healthy state of apps/Deployment/storefront' : 'successfully synced (all tasks run)',
        startedAt: minutesAgo(phase === 'Running' ? 1 : 22),
        finishedAt: phase === 'Running' ? undefined : minutesAgo(21),
        operation: { initiatedBy: failed ? { username: 'vijay' } : { automated: true }, sync: { revision: '4f2a91c0d1e2', prune: !failed } },
        syncResult: { revision: '4f2a91c0d1e2', resources: [
          { kind: 'ConfigMap', name: `${name}-config`, namespace: name, status: 'Synced', message: 'configmap/' + name + '-config configured', syncPhase: 'Sync' },
          { kind: 'Service', name, namespace: name, status: 'Synced', message: 'service/' + name + ' unchanged', syncPhase: 'Sync' },
          { group: 'apps', kind: 'Deployment', name, namespace: name, status: failed ? 'SyncFailed' : 'Synced', message: failed ? 'admission webhook "policy.acme.io" denied the request: image must come from registry.acme.io' : 'deployment.apps/' + name + ' configured', syncPhase: 'Sync' },
          { group: 'batch', kind: 'Job', name: `${name}-migrate`, namespace: name, status: 'Synced', message: 'job completed', hookPhase: 'Succeeded', syncPhase: 'PreSync' },
        ] },
      },
      history: [
        { id: 14, revision: '9c1d7e2a4b11', deployedAt: minutesAgo(60 * 50), deployStartedAt: minutesAgo(60 * 50 + 2), initiatedBy: { automated: true }, source: { repoURL: 'https://github.com/acme/deploy.git', path: `apps/${name}`, targetRevision: 'main' } },
        { id: 15, revision: '1b7e40c9d2f3', deployedAt: minutesAgo(60 * 26), deployStartedAt: minutesAgo(60 * 26 + 1), initiatedBy: { username: 'oncall' }, source: { repoURL: 'https://github.com/acme/deploy.git', path: `apps/${name}`, targetRevision: 'main' } },
        { id: 16, revision: '4f2a91c0d1e2', deployedAt: minutesAgo(21), deployStartedAt: minutesAgo(22), initiatedBy: { automated: true }, source: { repoURL: 'https://github.com/acme/deploy.git', path: `apps/${name}`, targetRevision: 'main' } },
      ],
      resources: [
        { version: 'v1', kind: 'Namespace', name, status: 'Synced', health: { status: 'Healthy' } },
        { version: 'v1', kind: 'ServiceAccount', name, namespace: name, status: 'Synced' },
        { version: 'v1', kind: 'ConfigMap', name: `${name}-config`, namespace: name, status: sync },
        { version: 'v1', kind: 'ConfigMap', name: `${name}-feature-flags`, namespace: name, status: 'OutOfSync', requiresPruning: sync === 'OutOfSync' },
        { version: 'v1', kind: 'Secret', name: `${name}-tls`, namespace: name, status: 'Synced' },
        { version: 'v1', kind: 'Service', name, namespace: name, status: 'Synced', health: { status: 'Healthy' } },
        { group: 'apps', version: 'v1', kind: 'Deployment', name, namespace: name, status: failed ? 'OutOfSync' : 'Synced', health: { status: health, message: health === 'Degraded' ? 'Deployment "' + name + '" exceeded its progress deadline' : health === 'Progressing' ? 'Waiting for rollout to finish: 1 of 3 updated replicas are available...' : undefined } },
        { group: 'autoscaling', version: 'v2', kind: 'HorizontalPodAutoscaler', name, namespace: name, status: 'Synced', health: { status: 'Healthy' } },
        { group: 'networking.k8s.io', version: 'v1', kind: 'Ingress', name, namespace: name, status: 'Synced', health: { status: 'Healthy' } },
      ],
      ...extra,
    },
  };
};

export const visualQaArgoApps = [
  argoApp('billing-api', 'payments', 'OutOfSync', 'Degraded', 'Failed', { conditions: [{ type: 'SyncError', message: 'Deployment billing-api: container image pull back-off' }] }),
  argoApp('checkout', 'payments', 'Synced', 'Healthy', 'Succeeded'),
  argoApp('storefront', 'web', 'Synced', 'Progressing', 'Running'),
  argoApp('search', 'web', 'OutOfSync', 'Healthy', 'Succeeded'),
  argoApp('ingress-nginx', 'platform', 'Synced', 'Healthy', 'Succeeded'),
  argoApp('cert-manager', 'platform', 'Synced', 'Healthy', 'Succeeded'),
  argoApp('observability', 'platform', 'Synced', 'Healthy', 'Succeeded'),
];

export const visualQaArgoSets = [
  { metadata: { name: 'platform-addons', namespace: 'argocd' }, spec: { generators: [{ clusters: {} }], template: { metadata: { name: '{{name}}-addons' }, spec: { project: 'platform' } } }, status: { conditions: [{ type: 'ResourcesUpToDate', status: 'True', message: 'All applications have been generated successfully' }], resources: [{}, {}, {}] } },
  { metadata: { name: 'team-previews', namespace: 'argocd' }, spec: { generators: [{ pullRequest: {} }], template: { metadata: { name: 'preview-{{number}}' }, spec: { project: 'web' } }, syncPolicy: { applicationsSync: 'create-update' } }, status: { conditions: [{ type: 'ErrorOccurred', status: 'True', message: 'failed to list pull requests: 401 Unauthorized' }] } },
];

export const visualQaArgoProjects = [
  { metadata: { name: 'payments', namespace: 'argocd' }, spec: { description: 'Payments team services', sourceRepos: ['https://github.com/acme/deploy.git'], destinations: [{ server: 'https://kubernetes.default.svc', namespace: 'billing-*' }, { server: 'https://kubernetes.default.svc', namespace: 'checkout' }], roles: [{ name: 'ci' }, { name: 'oncall' }], syncWindows: [{ kind: 'deny', schedule: '0 22 * * *' }] } },
  { metadata: { name: 'platform', namespace: 'argocd' }, spec: { description: 'Cluster add-ons', sourceRepos: ['*'], destinations: [{ server: '*', namespace: '*' }], clusterResourceWhitelist: [{ group: '*', kind: '*' }] } },
  { metadata: { name: 'web', namespace: 'argocd' }, spec: { sourceRepos: ['https://github.com/acme/web-*'], destinations: [{ name: 'prod', namespace: 'web' }] } },
];
