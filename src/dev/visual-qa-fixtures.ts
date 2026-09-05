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

export const visualQaOverview = {
  nodes: [
    { ...nodeBase, name: 'worker-1', uid: 'visual-node-1', addresses: [{ type: 'InternalIP', address: '10.0.12.21' }], providerId: 'qa://worker-1', creationTimestamp: new Date(Date.now() - 14 * 86_400_000).toISOString(), cpuCapacity: '8 cores', memoryCapacity: '32Gi', cpuUsage: '3.12 cores', memoryUsage: '14.6Gi', cpuUsagePercent: 39, memoryUsagePercent: 46 },
    { ...nodeBase, name: 'worker-2', uid: 'visual-node-2', addresses: [{ type: 'InternalIP', address: '10.0.12.22' }], providerId: 'qa://worker-2', creationTimestamp: new Date(Date.now() - 14 * 86_400_000).toISOString(), cpuCapacity: '8 cores', memoryCapacity: '32Gi', cpuUsage: '2.47 cores', memoryUsage: '11.2Gi', cpuUsagePercent: 31, memoryUsagePercent: 35 },
  ],
  totals: { cpuCapacity: '16 cores', memoryCapacity: '64Gi', storageCapacity: '480Gi', cpuUsage: '5.59 cores', memoryUsage: '25.8Gi', cpuUsagePercent: 35, memoryUsagePercent: 40, metricNodes: 2 },
  metricsAvailable: true,
  observedAt: new Date().toISOString(),
};
