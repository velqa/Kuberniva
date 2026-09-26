<script lang="ts">
  import { onDestroy, onMount, tick } from 'svelte';
  import { Bell, Boxes, Check, ChevronDown, ChevronRight, Command, Container, Copy, Database, Download, LayoutDashboard, Menu, Moon, RefreshCw, Search, ScrollText, Settings2, Star, Sun, Terminal, Workflow } from '@lucide/svelte';

  type View = 'Clusters' | 'Favorites' | 'Overview' | 'Events' | 'Resources' | 'Workloads' | 'Explore' | 'Logs' | 'Settings';
  type ThemeMode = 'light' | 'dark';
  type NodeDetailTab = 'Overview' | 'Allocation' | 'Network' | 'Health' | 'Metadata';
  type ResourceCategory = 'Workloads' | 'Configuration' | 'Access Control' | 'Network' | 'Gateway APIs' | 'Storage' | 'Cluster' | 'Custom Resources';
  type ResourceDescriptor = { group: string; version: string; apiVersion: string; kind: string; plural: string; namespaced: boolean; category: ResourceCategory; custom: boolean; crd: boolean };
  type AccessReviewCheck = { key: string; group: string; version: string; resource: string; verb: string; namespace?: string; name?: string; subresource?: string };
  type AccessReviewDecision = { key: string; allowed: boolean; denied: boolean; reason?: string; evaluationError?: string };
  type ResourcePermissionSet = { resolved: boolean; canList: boolean; canWatch: boolean; canGet: boolean; canCreate: boolean; canUpdate: boolean; canPatch: boolean; canDelete: boolean; canViewLogs: boolean; canExec: boolean };
  type ClusterCatalog = { context: string; namespaces: string[]; resources: ResourceDescriptor[] };
  type ResourceObject = {
    name: string;
    namespace?: string;
    uid?: string;
    resourceVersion?: string;
    createdAt?: string;
    status?: string;
    readyContainers?: number;
    totalContainers?: number;
    restarts?: number;
    cpuUsage?: string;
    memoryUsage?: string;
    nodeName?: string;
  };
  type PodPort = { container: string; name?: string; port: number; protocol: string };
  type PodLogResponse = { lines: string[]; containers: string[]; selectedContainer?: string; ports: PodPort[] };
  type PodRuntime = { containers: string[]; ports: PodPort[] };
  type PodExecResponse = { stdout: string; stderr: string };
  type OpeningLogsTarget = { key: string; label: string };
  type LogTarget = { pod: string; namespace: string };
  type CertificateInfo = { expiresAt: string; daysRemaining: number; expired: boolean };
  type ResourceDetail = { manifest: Record<string, unknown>; yaml: string; certificate?: CertificateInfo };
  type EditorEntry = { key: string; value: string };
  type NodeProperty = { key: string; value: string };
  type NodeAddress = { type: string; address: string };
  type NodeCondition = { type: string; status: string; reason?: string; message?: string; lastHeartbeatTime?: string; lastTransitionTime?: string };
  type NodeTaint = { key: string; value?: string; effect: string; timeAdded?: string };
  type NodeOverview = { name: string; ready: boolean; roles: string[]; labels: NodeProperty[]; annotations: NodeProperty[]; addresses: NodeAddress[]; conditions: NodeCondition[]; taints: NodeTaint[]; architecture?: string; operatingSystem?: string; osImage?: string; kernelVersion?: string; kubeletVersion?: string; containerRuntimeVersion?: string; podCidrs: string[]; providerId?: string; unschedulable: boolean; uid?: string; creationTimestamp?: string; capacity: NodeProperty[]; allocatable: NodeProperty[]; cpuCapacity?: string; memoryCapacity?: string; cpuUsage?: string; memoryUsage?: string; cpuUsagePercent?: number; memoryUsagePercent?: number };
  type ClusterTotals = { cpuCapacity?: string; memoryCapacity?: string; storageCapacity?: string; cpuUsage?: string; memoryUsage?: string; cpuUsagePercent?: number; memoryUsagePercent?: number; metricNodes: number };
  type NetworkFact = { label: string; value: string; tone: 'neutral' | 'primary' | 'external' };
  type ClusterOverview = { nodes: NodeOverview[]; totals: ClusterTotals; metricsAvailable: boolean; observedAt: string };
  type ClusterEvent = { name: string; namespace?: string; eventType: string; reason?: string; message?: string; involvedKind?: string; involvedName?: string; action?: string; count?: number; source?: string; firstObserved?: string; lastObserved?: string };
  type KubeContext = { name: string; cluster: string; namespace: string; authMethod: string; current: boolean; sourcePath?: string };
  type KubeconfigSummary = { contexts: KubeContext[]; currentContext?: string };
  type KubeconfigInputMode = 'file' | 'folder' | 'paste';
  type Cluster = { id: string; name: string; provider: string; status: string; tone: string; authMethod?: string; namespace?: string; kubeconfigPath?: string; sourceId?: string };
  type GlobalSearchResult = { type: 'resource' | 'object'; resource: ResourceDescriptor; object?: ResourceObject; title: string; detail: string };
  type UpdateState = 'idle' | 'checking' | 'current' | 'available' | 'downloading' | 'ready' | 'error';
  type PendingUpdate = { version: string; body?: string; date?: string; downloadAndInstall: (onEvent?: (event: { event: string; data?: { contentLength?: number; chunkLength?: number } }) => void) => Promise<void> };
  type CliLine = { stream: 'stdout' | 'stderr' | 'prompt' | 'meta'; text: string };
  type CliSession = { lines: CliLine[]; runId: string | null; running: boolean; startedAt: number; draft: string; open: boolean; expanded: boolean };
  type CliOutputEvent = { runId: string; chunks: { stream: 'stdout' | 'stderr'; text: string }[] };
  type CliExitEvent = { runId: string; exitCode?: number | null; success: boolean; cancelled: boolean; error?: string | null };
  type ResourceWatchSignal = { watchId: string; action: string; error?: string };
  type ResourceWatchStatus = 'idle' | 'connecting' | 'connected' | 'reconnecting' | 'error';
  type LiveDataStatus = 'loading' | 'live' | 'loaded' | 'stale' | 'paused' | 'unavailable';
  type LiveRefreshContext = { clusterId: string; view: View; dataKey: string };
  type ClusterSession = { namespace: string; selectedCategory: ResourceCategory | 'All resources'; resourceSearch: string; workloadResource: ResourceDescriptor | null; workloadObjects: ResourceObject[]; workloadSearch: string; clusterOverview: ClusterOverview | null };
  type PersistedWorkspace = { version: 7; sourceConfigured: boolean; kubeconfigPath: string; kubeconfigPaths: string[]; clusters: Cluster[]; sidebarWidth?: number; sidebarHidden?: boolean; clusterNamespaces?: Record<string, string>; favoriteClusterIds?: string[]; favoriteClusterNames?: Record<string, string>; theme?: ThemeMode };
  type DeletionTarget =
    | { type: 'resource'; resource: ResourceDescriptor; object: ResourceObject; clusterId: string; cluster: string; kubeconfigPath?: string; namespaceScope: string; view: 'Resources' | 'Workloads' }
    | { type: 'bulk-resource'; resource: ResourceDescriptor; objects: ResourceObject[]; namespaceScope: string; clusterId: string; cluster: string; kubeconfigPath?: string; view: 'Resources' | 'Workloads' }
    | { type: 'cluster'; cluster: Cluster };

  let activeView: View = 'Overview';
  let activeCluster = 'No cluster connected';
  let search = '';
  let namespace = 'all namespaces';
  let namespaceOpen = false;
  let clusterPickerOpen = false;
  let commandOpen = false;
  let commandQuery = '';
  let globalSearchResults: GlobalSearchResult[] = [];
  let kubeconfigOpen = false;
  let kubeconfigInputMode: KubeconfigInputMode = 'file';
  let kubeconfigPath = '';
  let pastedKubeconfig = '';
  let kubeconfigSources: string[] = [];
  let sourceConfigured = false;
  let restoringWorkspace = true;
  let resourceSearch = '';
  let selectedCategory: ResourceCategory | 'All resources' = 'All resources';
  let sidebarWorkloadMenuOpen = false;
  let sidebarResourceMenuOpen = false;
  let sidebarTypeActivePanel: 'workload' | 'resource' = 'workload';
  let sidebarResourceSearch = '';
  let resourceDirectorySearch = '';
  // The sidebar tree is the primary way to reach API kinds, as in Lens.
  let sidebarTreeOpen: Record<string, boolean> = { resources: true, custom: false };
  let sidebarTreeSections: Record<string, boolean> = {};
  let recentResourceKeys: string[] = [];
  let sidebarResourceCategory: ResourceCategory | 'All resources' = 'All resources';
  let theme: ThemeMode = 'light';
  let uiScale = 0.9;
  let loadingCatalog = false;
  let selectedResource: ResourceDescriptor | null = null;
  let resourceObjects: ResourceObject[] = [];
  let selectedResourceObjectKeys: string[] = [];
  let selectedResourceObjects: ResourceObject[] = [];
  let allResourceObjectsSelected = false;
  let resourceObjectsSelectionPartial = false;
  let selectedWorkloadObjectKeys: string[] = [];
  let selectedWorkloadObjects: ResourceObject[] = [];
  let allWorkloadObjectsSelected = false;
  let workloadObjectsSelectionPartial = false;
  let loadingObjects = false;
  let resourceRequestGeneration = 0;
  let relatedPods: ResourceObject[] | null = null;
  let loadingRelatedPods = false;
  let logTarget: LogTarget | null = null;
  let logPods: ResourceObject[] = [];
  let logPorts: PodPort[] = [];
  let logScopeLabel = '';
  let logLines: string[] = [];
  let logSearch = '';
  let logSinceTime = '';
  let logContainers: string[] = [];
  let selectedLogContainer: string | undefined;
  let loadingLogs = false;
  let openingLogsTarget: OpeningLogsTarget | null = null;
  let logViewport: HTMLPreElement | undefined;
  let logRefreshTimer: ReturnType<typeof window.setInterval> | undefined;
  let logCopyResetTimer: ReturnType<typeof window.setTimeout> | undefined;
  let logsCopied = false;
  let downloadingLogs = false;
  let logRequestGeneration = 0;
  let logWorkspaceGeneration = 0;
  let workloadDetailMode: 'overview' | 'terminal' | 'logs' = 'overview';
  let terminalPods: ResourceObject[] = [];
  let terminalTarget: LogTarget | null = null;
  let terminalContainers: string[] = [];
  let terminalPorts: PodPort[] = [];
  let selectedTerminalContainer = '';
  let terminalCommand = 'id && uname -a';
  let terminalOutput = '';
  let loadingTerminalPods = false;
  let loadingTerminalRuntime = false;
  let runningTerminalCommand = false;
  let cliCommand = '';
  let cliLines: CliLine[] = [];
  let appVersion = '';
  let updateState: UpdateState = 'idle';
  let updateError = '';
  let updateProgress: number | null = null;
  let pendingUpdate: PendingUpdate | null = null;
  let updateCheckedAt = '';
  let updateCheckTimer: ReturnType<typeof window.setTimeout> | undefined;
  let runningCli = false;
  let cliRunId: string | null = null;
  let cliRunStartedAt = 0;
  let cliExpanded = false;
  let cliHistory: string[] = [];
  let cliHistoryIndex = -1;
  let cliHistoryDraft = '';
  let cliListenersReady: Promise<void> | null = null;
  const cliHistoryStorageKey = 'kuberniva.cli-history.v1';
  // Each cluster keeps its own terminal; the active one lives in the cli* variables.
  const cliSessions = new Map<string, CliSession>();
  let cliSessionClusterId = '';
  const cliMaxLines = 5000;
  const cliStarterCommands = ['kubectl get pods', 'kubectl get deploy', 'kubectl get events --sort-by=.lastTimestamp', 'kubectl top pods', 'helm list'];
  let cliOpen = false;
  let cliViewport: HTMLPreElement | undefined;
  let cliInput: HTMLTextAreaElement | undefined;
  let editorResource: ResourceDescriptor | null = null;
  let editorObject: ResourceObject | null = null;
  let editorManifest: Record<string, unknown> | null = null;
  let editorEntries: EditorEntry[] = [];
  let expandedEditorEntryIndex = 0;
  let editorEntrySearch = '';
  let editorCertificate: CertificateInfo | undefined;
  let loadingEditor = false;
  let savingEditor = false;
  let revealSecret = false;
  let configDiscardPrompt = false;
  let connectedKubeconfig = false;
  let toast = '';
  // Toasts are transient feedback. The bell is reserved for actionable items,
  // so it stays out of the header unless something genuinely needs attention.
  let notifications: string[] = [];
  let activeKubeconfigPath: string | undefined;
  let activeClusterId = '';
  let catalogError = '';
  let sidebarWidth = 280;
  let resourceNavigatorWidth = 272;
  let resourceObjectPaneWidth = 300;
  let accessDecisions: Record<string, AccessReviewDecision> = {};
  let catalogPermissionsReady = false;
  let loadingPermissions = false;
  let permissionError = '';
  let permissionRequestsInFlight = 0;
  let workloadObjects: ResourceObject[] = [];
  let workloadResource: ResourceDescriptor | null = null;
  let loadingWorkloads = false;
  let workloadRequestGeneration = 0;
  let workloadSearch = '';
  let sidebarHidden = false;
  let persistedClusterNamespaces: Record<string, string> = {};
  let favoriteClusterIds: string[] = [];
  let favoriteClusterNames: Record<string, string> = {};
  let favoriteContextMenu: { clusterId: string; x: number; y: number } | null = null;
  let favoriteRenameId = '';
  let favoriteRenameValue = '';
  let selectedNodeName = '';
  let nodeDetailTab: NodeDetailTab = 'Overview';
  let clusterEvents: ClusterEvent[] = [];
  let loadingEvents = false;
  let eventsError = '';
  let eventsObservedAt = '';
  let eventsClusterId = '';
  let eventSearch = '';
  let eventTypeFilter: 'All' | 'Warning' | 'Normal' = 'All';
  let clusterOverview: ClusterOverview | null = null;
  let overviewError = '';
  let loadingOverview = false;
  let overviewRequestGeneration = 0;
  let eventsRequestGeneration = 0;
  let refreshingCluster = false;
  let overviewRefreshTimer: ReturnType<typeof window.setInterval> | undefined;
  let resourceWatchId = '';
  let resourceWatchKey = '';
  let resourceWatchClusterId = '';
  let resourceWatchView: 'Resources' | 'Workloads' | '' = '';
  let resourceWatchDataKey = '';
  let resourceWatchGeneration = 0;
  let resourceWatchRefreshTimer: ReturnType<typeof window.setTimeout> | undefined;
  let resourceWatchRefreshPending = false;
  let resourceWatchUnlisten: (() => void) | undefined;
  let resourceWatchListenerReady: Promise<void> | null = null;
  let resourceWatchErrorNotified = false;
  const resourceWatchSignalBuffer = new Map<string, ResourceWatchSignal>();
  let resourceWatchStatus: ResourceWatchStatus = 'idle';
  let liveDataStatus: LiveDataStatus = 'unavailable';
  let liveDataStatusMessage = '';
  let liveDataStatusText = 'Unavailable';
  let liveDataStatusTooltip = 'Live data is unavailable';
  let pageContextCopy = '';
  let resumeRecoveryPending = false;
  let resumeRecoveryContext: LiveRefreshContext | null = null;
  let lastHiddenAt = 0;
  let resumeRecoveryTimer: ReturnType<typeof window.setTimeout> | undefined;
  let stopWindowFocusListening: (() => void) | undefined;
  let relatedObject: ResourceObject | null = null;
  let yamlResource: ResourceDescriptor | null = null;
  let yamlObject: ResourceObject | null = null;
  let yamlText = '';
  let yamlOriginal = '';
  let yamlMode: 'view' | 'edit' = 'view';
  let yamlSearch = '';
  let loadingYaml = false;
  let savingYaml = false;
  let deletionTarget: DeletionTarget | null = null;
  let deletionStep: 1 | 2 = 1;
  let deletingResource = false;
  let bulkDeleteProgress: { completed: number; total: number; failed: number } | null = null;
  let deletionDialog: HTMLDivElement | undefined;
  let deletionConfirmButton: HTMLButtonElement | undefined;
  let deletionReturnFocus: HTMLElement | null = null;
  const catalogCache = new Map<string, ClusterCatalog>();
  // API catalogs from earlier launches let a cluster open instantly while discovery
  // revalidates in the background. Only the most recently used clusters are kept.
  const catalogStorageKey = 'kuberniva.catalog-cache.v1';
  const storedCatalogLimit = 8;
  const clusterSessionCache = new Map<string, ClusterSession>();
  const resourceObjectCache = new Map<string, ResourceObject[]>();
  const liveDataUpdatedAt = new Map<string, number>();
  const liveDataStaleAfterMs = 120_000;
  const workspaceStorageKey = 'kuberniva.workspace.v1';
  const themeStorageKey = 'kuberniva.theme.v1';
  const uiScaleStorageKey = 'kuberniva.ui-scale.v1';
  const resourceNavigatorWidthStorageKey = 'kuberniva.resource-navigator-width.v1';
  const resourceObjectPaneWidthStorageKey = 'kuberniva.resource-object-width.v1';

  const resourceCategories: ResourceCategory[] = ['Configuration', 'Access Control', 'Network', 'Gateway APIs', 'Storage', 'Cluster'];
  const resourceTreeCategories: ResourceCategory[] = ['Configuration', 'Access Control', 'Network', 'Storage', 'Cluster'];
  const nodeDetailTabs: NodeDetailTab[] = ['Overview', 'Allocation', 'Network', 'Health', 'Metadata'];
  let clusters: Cluster[] = [];
  let catalog: ClusterCatalog = { context: '', namespaces: [], resources: [] };

  $: favoriteClusters = favoriteClusterIds
    .map((id) => clusters.find((cluster) => cluster.id === id))
    .filter((cluster): cluster is Cluster => Boolean(cluster))
    .slice(0, 10);
  $: favoriteContextCluster = favoriteContextMenu ? clusters.find((cluster) => cluster.id === favoriteContextMenu?.clusterId) : null;
  $: selectedNode = clusterOverview?.nodes.find((node) => node.name === selectedNodeName) || clusterOverview?.nodes[0] || null;
  $: namespaceClusterEvents = clusterEvents.filter((event) =>
    namespace === 'all namespaces' || !event.namespace || event.namespace === namespace,
  );
  $: visibleClusterEvents = namespaceClusterEvents.filter((event) =>
    (eventTypeFilter === 'All' || event.eventType === eventTypeFilter) &&
    `${event.reason || ''} ${event.message || ''} ${event.involvedKind || ''} ${event.involvedName || ''} ${event.namespace || ''}`.toLowerCase().includes(eventSearch.toLowerCase()),
  );
  $: accessibleCatalogResources = catalog.resources.filter((resource) => resourceVisibleForCurrentScope(resource, accessDecisions, catalogPermissionsReady));
  $: resourceWorkspaceResources = accessibleCatalogResources.filter((resource) => resource.category !== 'Workloads' && resource.category !== 'Custom Resources');
  $: customApiResources = accessibleCatalogResources.filter((resource) => resource.category === 'Custom Resources');
  $: activeResourceCatalog = selectedCategory === 'Custom Resources' ? customApiResources : resourceWorkspaceResources;
  $: sidebarVisibleResources = activeResourceCatalog.filter((resource) =>
    resourceSearchText(resource).includes(sidebarResourceSearch.toLowerCase()),
  ).sort((left, right) => left.category.localeCompare(right.category) || left.kind.localeCompare(right.kind));
  $: globalSearchResults = buildGlobalSearchResults(commandQuery, [...resourceWorkspaceResources, ...customApiResources], selectedResource, resourceObjects, workloadResource, workloadObjects);
  $: selectedResourceObjects = resourceObjects.filter((object) => selectedResourceObjectKeys.includes(resourceObjectSelectionKey(object)));
  $: allResourceObjectsSelected = resourceObjects.length > 0 && selectedResourceObjects.length === resourceObjects.length;
  $: resourceObjectsSelectionPartial = selectedResourceObjects.length > 0 && !allResourceObjectsSelected;
  $: selectedWorkloadObjects = workloadObjects.filter((object) => selectedWorkloadObjectKeys.includes(resourceObjectSelectionKey(object)));
  $: allWorkloadObjectsSelected = workloadObjects.length > 0 && selectedWorkloadObjects.length === workloadObjects.length;
  $: workloadObjectsSelectionPartial = selectedWorkloadObjects.length > 0 && !allWorkloadObjectsSelected;
  $: liveDataStatusText = liveDataStatus === 'loading' ? 'Loading'
    : liveDataStatus === 'paused' ? 'Paused'
      : liveDataStatus === 'stale' ? 'Stale'
        : liveDataStatus === 'loaded' ? 'Loaded'
          : liveDataStatus === 'unavailable' ? 'Unavailable' : 'Live';
  $: liveDataStatusTooltip = liveDataStatusMessage
    || (liveDataStatus === 'live' ? 'Live updates are current'
      : liveDataStatus === 'loading' ? 'Loading the current Kubernetes API snapshot'
        : liveDataStatus === 'loaded' ? 'The current API snapshot is loaded; live updates are not connected.'
          : 'Live updates will resume when data is available and no active workflow is open');
  $: pageContextCopy = !activeClusterId ? (connectedKubeconfig ? 'Choose a cluster from the sidebar to connect.' : '')
    : !['Workloads', 'Resources'].includes(activeView) ? ''
      : liveDataStatus === 'paused' ? 'Live refresh is paused while your current workflow stays open.'
        : liveDataStatus === 'stale' ? 'Live data is stale. Refresh when you are ready.'
          : liveDataStatus === 'loaded' ? 'Current API snapshot loaded; live updates are unavailable. Refresh when needed.'
            : liveDataStatus === 'unavailable' ? 'Live data is unavailable. Kuberniva will preserve this workspace until it can refresh safely.' : '';
  $: categoryCounts = Object.fromEntries(resourceCategories.map((category) => [category, resourceWorkspaceResources.filter((resource) => resource.category === category).length]));
  $: customApiWorkspace = activeView === 'Resources' && selectedCategory === 'Custom Resources';
  $: resourceTreeSections = buildResourceDirectory(resourceWorkspaceResources, '', false);
  $: customTreeSections = buildResourceDirectory(customApiResources, '', true);
  $: resourceDirectorySections = buildResourceDirectory(activeResourceCatalog, resourceDirectorySearch, customApiWorkspace);
  $: recentDirectoryResources = resourceDirectorySearch.trim()
    ? []
    : recentResourceKeys.map((key) => activeResourceCatalog.find((resource) => resourceKey(resource) === key)).filter((resource): resource is ResourceDescriptor => Boolean(resource)).slice(0, 6);
  $: activeViewTitle = customApiWorkspace ? 'Custom APIs' : activeView;
  $: resourceNavigatorLabel = customApiWorkspace ? 'Custom APIs' : 'Resources';
  $: showClusterWorkspaceControls = Boolean(activeClusterId) && ['Overview', 'Events', 'Resources', 'Workloads', 'Logs'].includes(activeView);
  $: refreshingCurrentView = refreshingCluster || loadingCatalog || (activeView === 'Overview'
    ? loadingOverview
    : activeView === 'Events'
      ? loadingEvents
      : activeView === 'Resources'
        ? loadingObjects
        : activeView === 'Workloads'
          ? loadingWorkloads
          : activeView === 'Logs'
            ? loadingLogs
            : false);
  $: namespaceControlBusy = loadingCatalog
    || (activeView === 'Resources' && loadingObjects)
    || (activeView === 'Workloads' && loadingWorkloads)
    || (activeView === 'Logs' && loadingLogs)
    || loadingEditor
    || savingEditor
    || loadingYaml
    || savingYaml;
  $: readyNodeCount = clusterOverview?.nodes.filter((node) => node.ready).length || 0;
  $: workloadResources = accessibleCatalogResources
    .filter((resource) => resource.category === 'Workloads')
    .sort((left, right) => {
      const preferredOrder = ['Deployment', 'StatefulSet', 'DaemonSet', 'Job', 'CronJob', 'Pod', 'ReplicaSet', 'ReplicationController'];
      const leftIndex = preferredOrder.indexOf(left.kind);
      const rightIndex = preferredOrder.indexOf(right.kind);
      return (leftIndex === -1 ? 99 : leftIndex) - (rightIndex === -1 ? 99 : rightIndex) || left.kind.localeCompare(right.kind);
    });
  $: editorPermissionSet = resourcePermissionSet(editorResource, editorObject, accessDecisions);
  $: selectedResourcePermissionSet = resourcePermissionSet(selectedResource, null, accessDecisions);
  $: workloadPermissionSet = resourcePermissionSet(workloadResource, editorResource?.category === 'Workloads' ? editorObject : null, accessDecisions);
  $: yamlPermissionSet = resourcePermissionSet(yamlResource, yamlObject, accessDecisions);
  $: selectedResourceCanOpen = selectedResource?.kind === 'Pod' ? selectedResourcePermissionSet.canViewLogs : selectedResourcePermissionSet.canGet;
  // ConfigMaps and Secrets are edited in a focused dialog so the object table keeps its width.
  $: configModalOpen = activeView === 'Resources' && Boolean(editorResource && (editorResource.kind === 'ConfigMap' || editorResource.kind === 'Secret') && (editorObject || loadingEditor));
  $: configEditorDirty = Boolean(configModalOpen && editorManifest && !loadingEditor && editorEntriesSignature(editorEntries) !== editorDataSignature(editorManifest));
  $: if (activeClusterId !== cliSessionClusterId) swapCliSession(activeClusterId);
  $: workloadDetailOpen = (editorResource?.category === 'Workloads' && editorObject !== null) || (workloadDetailMode === 'logs' && logTarget !== null);
  $: workloadColumns = buildWorkloadColumns(workloadResource?.kind === 'Pod', namespace === 'all namespaces', workloadDetailOpen);
  $: workloadGridColumns = `${workloadColumns.map((column) => column.width).join(' ')} 18px`;
  $: visibleWorkloadObjects = workloadObjects.filter((workload) =>
    `${workload.name} ${workload.namespace || ''}`.toLowerCase().includes(workloadSearch.toLowerCase()),
  );
  $: selectedPodEvents = editorResource?.kind === 'Pod' && editorObject
    ? clusterEvents.filter((event) => event.involvedKind === 'Pod'
      && event.involvedName === editorObject?.name
      && (!event.namespace || event.namespace === editorObject?.namespace))
      .sort((left, right) => Date.parse(right.lastObserved || '') - Date.parse(left.lastObserved || ''))
      .slice(0, 10)
    : [];
  $: filteredEditorEntries = editorEntries
    .map((entry, index) => ({ entry, index }))
    .filter(({ entry }) => !editorEntrySearch.trim()
      || `${entry.key} ${editorEntryDisplayValue(entry, revealSecret)}`.toLowerCase().includes(editorEntrySearch.trim().toLowerCase()));
  $: normalizedLogSearch = logSearch.trim().toLowerCase();
  $: visibleLogLines = normalizedLogSearch
    ? logLines.filter((line) => line.toLowerCase().includes(normalizedLogSearch))
    : logLines;
  $: normalizedYamlSearch = yamlSearch.trim().toLowerCase();
  $: yamlSearchMatchCount = normalizedYamlSearch
    ? yamlText.toLowerCase().split(normalizedYamlSearch).length - 1
    : 0;
  $: editorLogsOpening = editorResource && editorObject
    ? isOpeningLogs(editorResource.kind, editorObject)
    : false;

  function autoSizeTextarea(node: HTMLTextAreaElement, _value: string) {
    const resize = () => {
      node.style.height = 'auto';
      const maxHeight = Math.min(560, Math.max(180, Math.round(window.innerHeight * 0.56)));
      const desiredHeight = Math.max(44, node.scrollHeight);
      node.style.height = `${Math.min(desiredHeight, maxHeight)}px`;
      node.style.overflowY = desiredHeight > maxHeight ? 'auto' : 'hidden';
    };
    resize();
    window.addEventListener('resize', resize);
    return {
      update: resize,
      destroy: () => window.removeEventListener('resize', resize),
    };
  }

  function autoSizeCliTextarea(node: HTMLTextAreaElement, _value: string) {
    const resize = () => {
      node.style.height = 'auto';
      const desiredHeight = Math.max(29, node.scrollHeight);
      node.style.height = `${Math.min(desiredHeight, 104)}px`;
      node.style.overflowY = desiredHeight > 104 ? 'auto' : 'hidden';
    };
    resize();
    return { update: resize };
  }

  function resourceAge(createdAt?: string) {
    if (!createdAt) return '—';
    const timestamp = Date.parse(createdAt);
    if (!Number.isFinite(timestamp)) return '—';
    const elapsed = Math.max(0, Date.now() - timestamp);
    const minutes = Math.floor(elapsed / 60_000);
    if (minutes < 1) return 'now';
    if (minutes < 60) return `${minutes}m`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours}h`;
    const days = Math.floor(hours / 24);
    if (days < 30) return `${days}d`;
    const months = Math.floor(days / 30);
    if (months < 12) return `${months}mo`;
    return `${Math.floor(months / 12)}y`;
  }

  function logOpeningKey(kind: string, object: ResourceObject) {
    return `${kind}\u0000${object.namespace || namespace}\u0000${object.name}`;
  }

  function isOpeningLogs(kind: string, object: ResourceObject) {
    return openingLogsTarget?.key === logOpeningKey(kind, object);
  }

  function workloadStatusLabel(object: ResourceObject) {
    return object.status || 'Live';
  }

  function workloadStatusTone(object: ResourceObject) {
    const status = (object.status || 'live').toLowerCase();
    if (['running', 'succeeded', 'live', 'available', 'ready', 'active', 'complete', 'completed', 'bound'].includes(status)) return 'running';
    if (['pending', 'unknown', 'progressing', 'containercreating', 'suspended', 'scheduled'].includes(status)) return 'pending';
    return 'failed';
  }

  function podContainerSummary(object: ResourceObject) {
    if (object.totalContainers === undefined) return '—';
    return `${object.readyContainers ?? 0}/${object.totalContainers}`;
  }

  function podMetricLabel(value?: string) {
    return value || '—';
  }

  function cpuMetricLabel(value?: string) {
    if (!value) return '—';
    const trimmed = value.trim();
    const match = trimmed.match(/^([+-]?(?:\d+(?:\.\d*)?|\.\d+))\s*(m|cores?|core)?$/i);
    if (!match) return value;
    const numeric = Number(match[1]);
    if (!Number.isFinite(numeric)) return value;
    const cores = match[2]?.toLowerCase() === 'm' ? numeric / 1_000 : numeric;
    if (cores < 1) {
      const rounded = Math.round(cores * 1_000) / 1_000;
      if (cores > 0 && rounded === 0) return '<0.001 cores';
      return `${rounded.toFixed(3).replace(/0+$/, '').replace(/\.$/, '')} cores`;
    }
    return `${cores.toFixed(2)} cores`;
  }

  function certificateRemainingLabel(certificate: CertificateInfo) {
    if (certificate.expired) {
      const days = Math.max(1, Math.abs(certificate.daysRemaining));
      return `${days} day${days === 1 ? '' : 's'} ago`;
    }
    if (certificate.daysRemaining <= 0) return 'Expires today';
    return `${certificate.daysRemaining} day${certificate.daysRemaining === 1 ? '' : 's'} remaining`;
  }

  function notify(message: string) {
    toast = message;
    window.setTimeout(() => (toast = ''), 2600);
  }

  function persistWorkspace() {
    try {
      if (activeClusterId) persistedClusterNamespaces = { ...persistedClusterNamespaces, [activeClusterId]: namespace };
      const workspace: PersistedWorkspace = {
        version: 7,
        sourceConfigured,
        kubeconfigPath,
        kubeconfigPaths: kubeconfigSources,
        clusters: clusters.map((cluster) => ({ ...cluster, status: 'Not connected', tone: 'gray' })),
        sidebarWidth,
        sidebarHidden,
        clusterNamespaces: persistedClusterNamespaces,
        favoriteClusterIds: favoriteClusterIds.filter((id) => clusters.some((cluster) => cluster.id === id)).slice(0, 10),
        favoriteClusterNames: Object.fromEntries(
          favoriteClusterIds
            .filter((id) => clusters.some((cluster) => cluster.id === id))
            .slice(0, 10)
            .flatMap((id) => {
              const label = favoriteClusterNames[id]?.trim();
              return label ? [[id, label]] : [];
            }),
        ),
        theme,
      };
      window.localStorage.setItem(workspaceStorageKey, JSON.stringify(workspace));
    } catch {
      // Workspace preferences are useful, but never required for a live connection.
    }
  }

  function loadWorkspacePreference(): PersistedWorkspace | null {
    try {
      const raw = window.localStorage.getItem(workspaceStorageKey);
      if (!raw) return null;
      const parsed = JSON.parse(raw) as {
        version?: number;
        sourceConfigured?: unknown;
        kubeconfigPath?: unknown;
        kubeconfigPaths?: unknown;
        clusters?: unknown;
        sidebarWidth?: unknown;
        sidebarHidden?: unknown;
        clusterNamespaces?: unknown;
        favoriteClusterIds?: unknown;
        favoriteClusterNames?: unknown;
        theme?: unknown;
      };
      if ((parsed.version !== 1 && parsed.version !== 2 && parsed.version !== 3 && parsed.version !== 4 && parsed.version !== 5 && parsed.version !== 6 && parsed.version !== 7) || typeof parsed.sourceConfigured !== 'boolean' || typeof parsed.kubeconfigPath !== 'string') return null;
      const kubeconfigPaths = parsed.version >= 2 && Array.isArray(parsed.kubeconfigPaths)
        ? [...new Set(parsed.kubeconfigPaths.filter((path): path is string => typeof path === 'string').map((path) => path.trim()))]
        : [parsed.kubeconfigPath.trim()];
      const cachedClusters = Array.isArray(parsed.clusters)
        ? parsed.clusters.flatMap((candidate) => {
          if (!candidate || typeof candidate !== 'object' || Array.isArray(candidate)) return [];
          const cluster = candidate as Partial<Cluster>;
          if (typeof cluster.id !== 'string' || typeof cluster.name !== 'string' || typeof cluster.provider !== 'string') return [];
          return [{
            id: cluster.id,
            name: cluster.name,
            provider: cluster.provider,
            status: 'Not connected',
            tone: 'gray',
            authMethod: typeof cluster.authMethod === 'string' ? cluster.authMethod : undefined,
            namespace: typeof cluster.namespace === 'string' ? cluster.namespace : undefined,
            kubeconfigPath: typeof cluster.kubeconfigPath === 'string' ? cluster.kubeconfigPath : undefined,
            sourceId: typeof cluster.sourceId === 'string' ? cluster.sourceId : undefined,
          }];
        })
        : [];
      const clusterNamespaces = parsed.clusterNamespaces && typeof parsed.clusterNamespaces === 'object' && !Array.isArray(parsed.clusterNamespaces)
        ? Object.fromEntries(Object.entries(parsed.clusterNamespaces).filter(([id, selectedNamespace]) => Boolean(id) && typeof selectedNamespace === 'string'))
        : {};
      const favoriteClusterIds = Array.isArray(parsed.favoriteClusterIds)
        ? [...new Set(parsed.favoriteClusterIds.filter((id): id is string => typeof id === 'string' && cachedClusters.some((cluster) => cluster.id === id)))].slice(0, 10)
        : [];
      const favoriteClusterNames = parsed.favoriteClusterNames && typeof parsed.favoriteClusterNames === 'object' && !Array.isArray(parsed.favoriteClusterNames)
        ? Object.fromEntries(Object.entries(parsed.favoriteClusterNames).filter(([id, label]) => favoriteClusterIds.includes(id) && typeof label === 'string' && label.trim()).map(([id, label]) => [id, String(label).trim().slice(0, 80)]))
        : {};
      const workspace: PersistedWorkspace = {
        version: 7,
        sourceConfigured: parsed.sourceConfigured,
        kubeconfigPath: parsed.kubeconfigPath,
        kubeconfigPaths: kubeconfigPaths.length ? kubeconfigPaths : [''],
        clusters: cachedClusters,
        sidebarWidth: typeof parsed.sidebarWidth === 'number' ? parsed.sidebarWidth : undefined,
        sidebarHidden: typeof parsed.sidebarHidden === 'boolean' ? parsed.sidebarHidden : undefined,
        clusterNamespaces,
        favoriteClusterIds,
        favoriteClusterNames,
        theme: parsed.theme === 'dark' ? 'dark' : 'light',
      };
      window.localStorage.setItem(workspaceStorageKey, JSON.stringify(workspace));
      return workspace;
    } catch {
      return null;
    }
  }

  function loadThemePreference(): ThemeMode {
    try {
      return window.localStorage.getItem(themeStorageKey) === 'dark' ? 'dark' : 'light';
    } catch {
      return 'light';
    }
  }

  function sourceKey(path: string) {
    return path.trim() || '__default_kubeconfig__';
  }

  function rememberKubeconfigSource(path: string) {
    const normalizedPath = path.trim();
    if (!kubeconfigSources.some((source) => sourceKey(source) === sourceKey(normalizedPath))) {
      kubeconfigSources = [...kubeconfigSources, normalizedPath];
    }
  }

  function applyKubeconfigSummary(summary: KubeconfigSummary, source = kubeconfigPath, replaceSource = false) {
    const sourceId = sourceKey(source);
    const incoming = summary.contexts.map((context) => ({
      id: `${context.sourcePath || source || 'default'}\u0000${context.name}`,
      name: context.name,
      provider: context.cluster,
      status: 'Not connected',
      tone: 'gray',
      authMethod: context.authMethod,
      namespace: context.namespace,
      kubeconfigPath: context.sourcePath || source || undefined,
      sourceId,
    }));
    const existingClusters = new Map(clusters.map((cluster) => [cluster.id, cluster]));
    const incomingIds = new Set(incoming.map((cluster) => cluster.id));
    const mergedIncoming = incoming.map((cluster) => {
      const existing = existingClusters.get(cluster.id);
      return existing ? { ...cluster, status: existing.status, tone: existing.tone } : cluster;
    });
    const retainedClusters = clusters.filter((cluster) => {
      if (incomingIds.has(cluster.id)) return false;
      if (!replaceSource) return true;
      return cluster.sourceId !== sourceId;
    });
    clusters = [...retainedClusters, ...mergedIncoming];
    connectedKubeconfig = clusters.length > 0;
    if (!clusters.some((cluster) => cluster.id === activeClusterId)) {
      resourceRequestGeneration += 1;
      workloadRequestGeneration += 1;
      overviewRequestGeneration += 1;
      eventsRequestGeneration += 1;
      loadingOverview = false;
      loadingEvents = false;
      loadingObjects = false;
      loadingWorkloads = false;
      stopLiveObjectRefresh();
      clearLiveDataFreshness();
      clearResourceObjectSelection();
      resourceObjects = [];
      workloadObjects = [];
      relatedPods = null;
      relatedObject = null;
      selectedResource = null;
      closeLogs();
      closeEditor();
      closeYamlEditor();
      activeClusterId = '';
      activeKubeconfigPath = undefined;
      activeCluster = clusters.length ? 'Select a cluster' : 'No cluster connected';
      clearResourceObjectSelection();
      stopOverviewRefresh();
    }
  }

  function resourceObjectCacheKey(clusterId: string, resource: ResourceDescriptor, resourceNamespace: string) {
    return [clusterId, resource.group, resource.version, resource.plural, resourceNamespace || 'all namespaces'].join('\u0000');
  }

  function clearLiveDataFreshness(clusterId?: string, resource?: ResourceDescriptor, resourceNamespace = namespace) {
    if (!clusterId) {
      liveDataUpdatedAt.clear();
      return;
    }
    if (resource) {
      liveDataUpdatedAt.delete(resourceObjectCacheKey(clusterId, resource, resourceNamespace));
      return;
    }
    for (const key of liveDataUpdatedAt.keys()) {
      if (key.startsWith(`${clusterId}\u0000`)) liveDataUpdatedAt.delete(key);
    }
  }

  function clearResourceObjectCacheEntry(clusterId: string, resource: ResourceDescriptor, resourceNamespace: string) {
    const key = resourceObjectCacheKey(clusterId, resource, resourceNamespace);
    resourceObjectCache.delete(key);
    clearLiveDataFreshness(clusterId, resource, resourceNamespace);
  }

  function resourceObjectSelectionKey(object: ResourceObject) {
    return `${object.namespace || ''}\u0000${object.name}\u0000${object.uid || ''}`;
  }

  function clearResourceObjectSelection() {
    selectedResourceObjectKeys = [];
    selectedWorkloadObjectKeys = [];
  }

  function reconcileResourceObjectSelection(objects: ResourceObject[]) {
    const available = new Set(objects.map(resourceObjectSelectionKey));
    selectedResourceObjectKeys = selectedResourceObjectKeys.filter((key) => available.has(key));
  }

  function toggleResourceObjectSelection(object: ResourceObject) {
    const key = resourceObjectSelectionKey(object);
    selectedResourceObjectKeys = selectedResourceObjectKeys.includes(key)
      ? selectedResourceObjectKeys.filter((candidate) => candidate !== key)
      : [...selectedResourceObjectKeys, key];
  }

  // Templates pass the key list explicitly so Svelte re-renders rows when it changes.
  function isResourceObjectSelected(object: ResourceObject, keys = selectedResourceObjectKeys) {
    return keys.includes(resourceObjectSelectionKey(object));
  }

  function toggleAllResourceObjects() {
    selectedResourceObjectKeys = allResourceObjectsSelected
      ? []
      : resourceObjects.map(resourceObjectSelectionKey);
  }

  function clearWorkloadObjectSelection() {
    selectedWorkloadObjectKeys = [];
  }

  function reconcileWorkloadObjectSelection(objects: ResourceObject[]) {
    const available = new Set(objects.map(resourceObjectSelectionKey));
    selectedWorkloadObjectKeys = selectedWorkloadObjectKeys.filter((key) => available.has(key));
  }

  function toggleWorkloadObjectSelection(object: ResourceObject) {
    const key = resourceObjectSelectionKey(object);
    selectedWorkloadObjectKeys = selectedWorkloadObjectKeys.includes(key)
      ? selectedWorkloadObjectKeys.filter((candidate) => candidate !== key)
      : [...selectedWorkloadObjectKeys, key];
  }

  function isWorkloadObjectSelected(object: ResourceObject, keys = selectedWorkloadObjectKeys) {
    return keys.includes(resourceObjectSelectionKey(object));
  }

  function toggleAllWorkloadObjects() {
    selectedWorkloadObjectKeys = allWorkloadObjectsSelected
      ? []
      : workloadObjects.map(resourceObjectSelectionKey);
  }

  function activeLiveDataKey() {
    if (!activeClusterId) return '';
    if (activeView === 'Workloads' && workloadResource) {
      return resourceObjectCacheKey(activeClusterId, workloadResource, namespace);
    }
    if (activeView === 'Resources' && selectedResource) {
      return resourceObjectCacheKey(activeClusterId, selectedResource, namespace);
    }
    return '';
  }

  function currentLiveRefreshContext(): LiveRefreshContext | null {
    const dataKey = activeLiveDataKey();
    if (!activeClusterId || !dataKey || !['Workloads', 'Resources'].includes(activeView)) return null;
    return { clusterId: activeClusterId, view: activeView, dataKey };
  }

  function liveRefreshContext(clusterId: string, view: 'Resources' | 'Workloads', resource: ResourceDescriptor, resourceNamespace: string): LiveRefreshContext {
    return { clusterId, view, dataKey: resourceObjectCacheKey(clusterId, resource, resourceNamespace) };
  }

  function liveRefreshContextMatchesCurrent(context: LiveRefreshContext) {
    const current = currentLiveRefreshContext();
    return Boolean(current
      && current.clusterId === context.clusterId
      && current.view === context.view
      && current.dataKey === context.dataKey);
  }

  function liveWatchMatchesContext(context: LiveRefreshContext) {
    return resourceWatchStatus === 'connected'
      && resourceWatchClusterId === context.clusterId
      && resourceWatchView === context.view
      && resourceWatchDataKey === context.dataKey;
  }

  function markResumeRecoveryPending() {
    const context = currentLiveRefreshContext();
    if (!context) return;
    resumeRecoveryPending = true;
    resumeRecoveryContext = context;
  }

  function markResourceWatchRefreshPending() {
    resourceWatchRefreshPending = true;
    if (!resumeRecoveryContext) resumeRecoveryContext = currentLiveRefreshContext();
  }

  function pendingResumeContextMatchesCurrentView() {
    const context = resumeRecoveryContext;
    const current = currentLiveRefreshContext();
    return Boolean(context && current
      && context.clusterId === current.clusterId
      && context.view === current.view
      && context.dataKey === current.dataKey);
  }

  function cancelPendingLiveRefresh() {
    resumeRecoveryPending = false;
    resourceWatchRefreshPending = false;
    resumeRecoveryContext = null;
    if (resumeRecoveryTimer) window.clearTimeout(resumeRecoveryTimer);
    resumeRecoveryTimer = undefined;
  }

  function markLiveDataAvailable(resource: ResourceDescriptor, clusterId: string, resourceNamespace: string, view: 'Resources' | 'Workloads') {
    if (!clusterId) return;
    const context = liveRefreshContext(clusterId, view, resource, resourceNamespace);
    liveDataUpdatedAt.set(context.dataKey, Date.now());
    if (liveRefreshContextMatchesCurrent(context)) {
      liveDataStatus = liveWatchMatchesContext(context) ? 'live' : 'loaded';
      liveDataStatusMessage = liveWatchMatchesContext(context) ? '' : 'Current API snapshot loaded; live watch is not connected.';
    }
  }

  function markLiveDataUnavailable(message = 'Live data is unavailable', context?: LiveRefreshContext) {
    if (context && !liveRefreshContextMatchesCurrent(context)) return;
    if (liveDataUpdatedAt.has(activeLiveDataKey())) {
      liveDataStatus = 'loaded';
      liveDataStatusMessage = `Current API snapshot loaded; ${message.toLowerCase()}.`;
      return;
    }
    liveDataStatus = 'unavailable';
    liveDataStatusMessage = message;
  }

  function markLiveDataStale(message = 'Live data is stale') {
    liveDataStatus = 'stale';
    liveDataStatusMessage = message;
  }

  function markLiveDataPaused(message = 'Refresh paused while an active workflow is open') {
    liveDataStatus = 'paused';
    liveDataStatusMessage = message;
  }

  function activeLiveDataNeedsRecovery() {
    const key = activeLiveDataKey();
    if (!key) return false;
    const currentContext = currentLiveRefreshContext();
    if (currentContext && liveWatchMatchesContext(currentContext)) return false;
    const lastUpdatedAt = liveDataUpdatedAt.get(key) || 0;
    const stale = !lastUpdatedAt || Date.now() - lastUpdatedAt >= liveDataStaleAfterMs;
    const unavailable = Boolean(catalogError) || resourceWatchStatus === 'error' || resourceWatchStatus === 'reconnecting'
      || resourceWatchStatus === 'idle' || !lastUpdatedAt;
    return stale || unavailable;
  }

  function hasProtectedWorkflow() {
    const modalOpen = kubeconfigOpen || commandOpen || Boolean(deletionTarget) || Boolean(favoriteContextMenu) || Boolean(favoriteRenameId);
    const connectionWorkflow = loadingCatalog || deletingResource;
    const resourceWorkflow = Boolean(editorResource || yamlResource || loadingEditor || savingEditor || loadingYaml || savingYaml || loadingRelatedPods || relatedObject);
    const workloadWorkflow = activeView === 'Workloads' && (
      workloadDetailMode === 'terminal'
      || workloadDetailMode === 'logs'
      || Boolean(terminalTarget || logTarget || openingLogsTarget)
      || loadingTerminalPods
      || loadingTerminalRuntime
      || runningTerminalCommand
      || loadingLogs
      || downloadingLogs
    );
    return modalOpen || connectionWorkflow || resourceWorkflow || workloadWorkflow;
  }

  function hasPreservedLiveState() {
    return Boolean(
      editorResource
      || yamlResource
      || loadingEditor
      || savingEditor
      || loadingYaml
      || savingYaml
      || relatedObject
      || loadingRelatedPods
      || terminalTarget
      || loadingTerminalPods
      || loadingTerminalRuntime
      || runningTerminalCommand
      || logTarget
      || openingLogsTarget
      || loadingLogs
      || downloadingLogs
      || workloadDetailMode === 'terminal'
      || workloadDetailMode === 'logs'
    );
  }

  function schedulePendingResumeRecovery() {
    if ((!resumeRecoveryPending && !resourceWatchRefreshPending) || resumeRecoveryTimer) return;
    resumeRecoveryTimer = window.setTimeout(() => {
      resumeRecoveryTimer = undefined;
      if ((!resumeRecoveryPending && !resourceWatchRefreshPending) || hasProtectedWorkflow()) return;
      if (!pendingResumeContextMatchesCurrentView()) {
        resumeRecoveryPending = false;
        resourceWatchRefreshPending = false;
        resumeRecoveryContext = null;
        return;
      }
      if (resumeRecoveryPending) {
        resumeRecoveryPending = false;
        resumeRecoveryContext = null;
        resourceWatchRefreshPending = false;
        void refreshCurrentView();
      } else {
        resourceWatchRefreshPending = false;
        resumeRecoveryContext = null;
        void refreshVisibleObjectList();
      }
    }, 250);
  }

  function rememberActiveClusterSession() {
    if (!activeClusterId) return;
    persistedClusterNamespaces = { ...persistedClusterNamespaces, [activeClusterId]: namespace };
    clusterSessionCache.set(activeClusterId, {
      namespace,
      selectedCategory,
      resourceSearch,
      workloadResource,
      workloadObjects,
      workloadSearch,
      clusterOverview,
    });
  }

  function restoreClusterSession(cluster: Cluster) {
    const session = clusterSessionCache.get(cluster.id);
    namespace = session?.namespace || persistedClusterNamespaces[cluster.id] || cluster.namespace || 'all namespaces';
    selectedCategory = session?.selectedCategory === 'Workloads' ? 'All resources' : session?.selectedCategory || 'All resources';
    sidebarResourceCategory = selectedCategory;
    resourceSearch = session?.resourceSearch || '';
    workloadResource = session?.workloadResource || null;
    workloadObjects = session?.workloadObjects || [];
    workloadSearch = session?.workloadSearch || '';
    clusterOverview = session?.clusterOverview || null;
    if (session?.workloadResource && session.workloadObjects.length) {
      liveDataUpdatedAt.set(resourceObjectCacheKey(cluster.id, session.workloadResource, namespace), Date.now());
    }
    reconcileWorkloadObjectSelection(workloadObjects);
  }

  function clearClusterObjectCache(clusterId: string) {
    for (const cacheKey of resourceObjectCache.keys()) {
      if (cacheKey.startsWith(`${clusterId}\u0000`)) resourceObjectCache.delete(cacheKey);
    }
    clearLiveDataFreshness(clusterId);
  }

  async function restoreVisualQaScenario() {
    if (!import.meta.env.DEV) return false;
    const scenario = new URLSearchParams(window.location.search).get('visual-qa');
    if (!scenario || !['overview', 'workloads', 'workloads-first-open', 'workload-details', 'pod-details', 'workload-logs', 'workload-yaml', 'resources', 'custom-apis', 'configuration', 'configuration-many', 'secret', 'permissions-readonly'].includes(scenario)) return false;
    const fixtures = await import('./dev/visual-qa-fixtures');
    const qaCluster = fixtures.visualQaCluster as Cluster;
    const qaResources = fixtures.visualQaResources as ResourceDescriptor[];
    clusters = fixtures.visualQaFavoriteClusters as Cluster[];
    favoriteClusterIds = clusters.map((cluster) => cluster.id);
    favoriteClusterNames = { [qaCluster.id]: 'Production West' };
    connectedKubeconfig = true;
    sourceConfigured = true;
    kubeconfigPath = qaCluster.kubeconfigPath || '';
    activeKubeconfigPath = qaCluster.kubeconfigPath;
    activeClusterId = qaCluster.id;
    activeCluster = qaCluster.name;
    namespace = 'platform';
    catalog = { context: qaCluster.name, namespaces: ['platform', 'payments'], resources: qaResources };
    catalogCache.set(qaCluster.id, catalog);
    seedVisualQaPermissions(qaResources, scenario === 'permissions-readonly');
    catalogError = '';
    loadingCatalog = false;
    if (scenario === 'overview') {
      activeView = 'Overview';
      clusterOverview = fixtures.visualQaOverview as ClusterOverview;
      selectedNodeName = clusterOverview.nodes[0]?.name || '';
    } else if (scenario === 'workloads' || scenario === 'workloads-first-open' || scenario === 'workload-details' || scenario === 'pod-details' || scenario === 'workload-logs' || scenario === 'workload-yaml') {
      const showWorkloadDetail = scenario === 'workload-details' || scenario === 'pod-details' || scenario === 'workload-logs' || scenario === 'workload-yaml';
      const showPodDetail = scenario === 'pod-details';
      activeView = 'Workloads';
      workloadResource = qaResources.find((resource) => resource.kind === (showWorkloadDetail && !showPodDetail ? 'Deployment' : 'Pod')) || null;
      workloadObjects = scenario === 'workloads-first-open'
        ? []
        : showWorkloadDetail && !showPodDetail
          ? fixtures.visualQaDeployments as ResourceObject[]
          : fixtures.visualQaPods as ResourceObject[];
      loadingWorkloads = scenario === 'workloads-first-open';
      if (workloadResource) {
        const key = resourceObjectCacheKey(qaCluster.id, workloadResource, namespace);
        if (scenario === 'workloads-first-open') {
          liveDataStatus = 'loading';
          liveDataStatusMessage = 'Loading the current Kubernetes API snapshot';
          window.setTimeout(() => {
            if (activeView !== 'Workloads' || activeClusterId !== qaCluster.id || !workloadResource) return;
            workloadObjects = fixtures.visualQaPods as ResourceObject[];
            loadingWorkloads = false;
            resourceObjectCache.set(key, workloadObjects);
            markLiveDataAvailable(workloadResource, qaCluster.id, namespace, 'Workloads');
          }, 1_200);
          window.setTimeout(() => {
            if (activeView !== 'Workloads' || activeClusterId !== qaCluster.id || !workloadResource) return;
            resourceWatchStatus = 'connected';
            resourceWatchClusterId = qaCluster.id;
            resourceWatchView = 'Workloads';
            resourceWatchDataKey = key;
            markLiveDataAvailable(workloadResource, qaCluster.id, namespace, 'Workloads');
          }, 2_200);
        } else {
          resourceObjectCache.set(key, workloadObjects);
          liveDataUpdatedAt.set(key, Date.now());
          resourceWatchStatus = 'connected';
          resourceWatchClusterId = qaCluster.id;
          resourceWatchView = 'Workloads';
          resourceWatchDataKey = key;
          liveDataStatus = 'live';
          selectedWorkloadObjectKeys = [resourceObjectSelectionKey(workloadObjects[1])];
          if (showWorkloadDetail) {
            editorResource = workloadResource;
            editorObject = workloadObjects[0];
            editorManifest = (showPodDetail ? fixtures.visualQaPodManifest : fixtures.visualQaWorkloadManifest) as Record<string, unknown>;
            loadingEditor = false;
            if (showPodDetail) {
              clusterEvents = fixtures.visualQaPodEvents as ClusterEvent[];
              eventsClusterId = qaCluster.id;
              eventsObservedAt = new Date().toISOString();
            }
            if (scenario === 'workload-logs') {
              workloadDetailMode = 'logs';
              logPods = fixtures.visualQaPods as ResourceObject[];
              logTarget = { pod: logPods[0].name, namespace: logPods[0].namespace || namespace };
              logScopeLabel = `Deployment · ${editorObject.name}`;
              logLines = fixtures.visualQaLogLines as string[];
              logContainers = ['api', 'telemetry-sidecar'];
              selectedLogContainer = 'api';
              logPorts = [{ container: 'api', name: 'http', port: 8080, protocol: 'TCP' }];
            } else if (scenario === 'workload-yaml') {
              yamlResource = workloadResource;
              yamlObject = editorObject;
              yamlText = fixtures.visualQaWorkloadYaml as string;
              yamlOriginal = yamlText;
              yamlMode = 'view';
            }
          }
        }
      }
    } else {
      activeView = 'Resources';
      const selectedKind = scenario === 'secret' ? 'Secret' : scenario === 'custom-apis' ? 'TenantPolicy' : 'ConfigMap';
      selectedResource = qaResources.find((resource) => resource.kind === selectedKind) || null;
      selectedCategory = scenario === 'custom-apis' ? 'Custom Resources' : 'Configuration';
      sidebarResourceCategory = selectedCategory;
      resourceObjects = (scenario === 'custom-apis' ? fixtures.visualQaCustomObjects : fixtures.visualQaConfigMaps) as ResourceObject[];
      loadingObjects = false;
      if (selectedResource) {
        const key = resourceObjectCacheKey(qaCluster.id, selectedResource, namespace);
        resourceObjectCache.set(key, resourceObjects);
        liveDataUpdatedAt.set(key, Date.now());
        resourceWatchStatus = 'connected';
        resourceWatchClusterId = qaCluster.id;
        resourceWatchView = 'Resources';
        resourceWatchDataKey = key;
        liveDataStatus = 'live';
        selectedResourceObjectKeys = [resourceObjectSelectionKey(resourceObjects[1])];
        if (scenario === 'configuration' || scenario === 'configuration-many' || scenario === 'secret' || scenario === 'permissions-readonly') {
          const values = scenario === 'secret' ? fixtures.visualQaSecretValues : scenario === 'configuration-many' ? fixtures.visualQaLargeConfigValues : fixtures.visualQaConfigValues;
          editorResource = selectedResource;
          editorObject = resourceObjects[0];
          editorManifest = { apiVersion: selectedResource.apiVersion, kind: selectedResource.kind, metadata: { name: editorObject.name, namespace: editorObject.namespace }, data: values };
          editorEntries = Object.entries(values).map(([keyName, value]) => ({ key: keyName, value: String(value) }));
          expandedEditorEntryIndex = editorEntries.length > 8 ? -1 : 0;
          editorEntrySearch = '';
          revealSecret = false;
          loadingEditor = false;
        }
      }
    }
    restoringWorkspace = false;
    return true;
  }

  async function restoreWorkspace() {
    if (await restoreVisualQaScenario()) return;
    if (!('__TAURI_INTERNALS__' in window)) {
      restoringWorkspace = false;
      return;
    }
    const workspace = loadWorkspacePreference();
    if (!workspace) {
      restoringWorkspace = false;
      return;
    }
    applyTheme(workspace.theme || 'light');
    kubeconfigPath = workspace.kubeconfigPath;
    kubeconfigSources = workspace.kubeconfigPaths;
    sourceConfigured = workspace.sourceConfigured;
    sidebarWidth = Math.min(460, Math.max(230, workspace.sidebarWidth || sidebarWidth));
    sidebarHidden = workspace.sidebarHidden || false;
    persistedClusterNamespaces = workspace.clusterNamespaces || {};
    favoriteClusterIds = workspace.favoriteClusterIds || [];
    favoriteClusterNames = workspace.favoriteClusterNames || {};
    // Startup deliberately restores only the saved local snapshot. It never
    // rescans files or folders, so removed contexts stay removed and opening
    // Kuberniva remains immediate. Source reads happen only through Add or Sync.
    clusters = workspace.clusters;
    connectedKubeconfig = clusters.length > 0;
    activeCluster = clusters.length ? 'Select a cluster' : 'No cluster connected';
    activeView = clusters.length ? 'Clusters' : 'Overview';
    restoringWorkspace = false;
    if (clusters.length) {
      notify(`${clusters.length} cached context${clusters.length === 1 ? '' : 's'} restored locally. Sync sources only when you choose to.`);
    }
  }

  function usagePercentLabel(percent?: number) {
    if (percent === undefined) return 'No live metric';
    if (percent > 0 && percent < 1) return '<1% used';
    return `${Math.round(percent)}% used`;
  }

  function formatObservedTime(value?: string) {
    if (!value) return 'Time unavailable';
    const date = new Date(value);
    return Number.isNaN(date.getTime()) ? value : date.toLocaleString();
  }

  function eventTone(eventType: string) {
    return eventType.toLowerCase() === 'warning' ? 'warning' : 'normal';
  }

  function remainingPercentLabel(percent?: number) {
    if (percent === undefined) return '';
    const remaining = Math.max(0, 100 - percent);
    return `${Math.round(remaining)}% left`;
  }

  function stopOverviewRefresh() {
    if (overviewRefreshTimer) window.clearInterval(overviewRefreshTimer);
    overviewRefreshTimer = undefined;
  }

  function stopLiveObjectRefresh() {
    resourceWatchGeneration += 1;
    if (resourceWatchRefreshTimer) window.clearTimeout(resourceWatchRefreshTimer);
    resourceWatchRefreshTimer = undefined;
    resourceWatchRefreshPending = false;
    if (!resumeRecoveryPending) resumeRecoveryContext = null;
    const watchId = resourceWatchId;
    resourceWatchId = '';
    resourceWatchKey = '';
    resourceWatchClusterId = '';
    resourceWatchView = '';
    resourceWatchDataKey = '';
    resourceWatchErrorNotified = false;
    resourceWatchStatus = 'idle';
    resourceWatchSignalBuffer.clear();
    if (watchId && '__TAURI_INTERNALS__' in window) {
      void import('@tauri-apps/api/core')
        .then(({ invoke }) => invoke('stop_resource_watch', { watchId }))
        .catch(() => undefined);
    }
  }

  function startLiveObjectRefresh() {
    stopLiveObjectRefresh();
    if (!activeClusterId || !['Workloads', 'Resources'].includes(activeView)) return;
    const resource = activeView === 'Workloads' ? workloadResource : selectedResource;
    if (!resource) return;
    const permissions = resourcePermissionSet(resource, null, accessDecisions);
    if (permissions.resolved && !permissions.canWatch) {
      resourceWatchStatus = 'error';
      liveDataStatus = 'loaded';
      liveDataStatusMessage = 'Snapshot loaded · live watch is not permitted';
      return;
    }
    void startResourceWatch(resource);
  }

  function resourceWatchRequest(resource: ResourceDescriptor) {
    return {
      kubeconfigPath: activeKubeconfigPath || kubeconfigPath || null,
      context: activeCluster,
      group: resource.group,
      version: resource.version,
      kind: resource.kind,
      plural: resource.plural,
      namespaced: resource.namespaced,
      namespace,
    };
  }

  async function startResourceWatch(resource: ResourceDescriptor) {
    if (!activeClusterId || !('__TAURI_INTERNALS__' in window)) return;
    if (resourceWatchListenerReady) await resourceWatchListenerReady;
    const requestClusterId = activeClusterId;
    const requestNamespace = namespace;
    const nextWatchKey = `${requestClusterId}\u0000${resourceObjectCacheKey(requestClusterId, resource, requestNamespace)}`;
    if (resourceWatchId && resourceWatchKey === nextWatchKey) return;
    if (resourceWatchId) stopLiveObjectRefresh();
    const watchGeneration = resourceWatchGeneration;
    const requestView = resource.category === 'Workloads' ? 'Workloads' : 'Resources';
    const requestContext = liveRefreshContext(requestClusterId, requestView, resource, requestNamespace);
    resourceWatchStatus = 'connecting';
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const watchId = await invoke<string>('start_resource_watch', {
        request: resourceWatchRequest(resource),
      });
      if (
        watchGeneration !== resourceWatchGeneration
        ||
        requestClusterId !== activeClusterId
        || requestNamespace !== namespace
        || activeView !== (resource.category === 'Workloads' ? 'Workloads' : 'Resources')
        || (resource.category === 'Workloads' && (!workloadResource || resourceKey(workloadResource) !== resourceKey(resource)))
        || (resource.category !== 'Workloads' && (!selectedResource || resourceKey(selectedResource) !== resourceKey(resource)))
      ) {
        await invoke('stop_resource_watch', { watchId }).catch(() => undefined);
        return;
      }
      resourceWatchId = watchId;
      resourceWatchKey = nextWatchKey;
      resourceWatchClusterId = requestClusterId;
      resourceWatchView = requestView;
      resourceWatchDataKey = resourceObjectCacheKey(requestClusterId, resource, requestNamespace);
      resourceWatchErrorNotified = false;
      const bufferedSignal = resourceWatchSignalBuffer.get(watchId);
      resourceWatchSignalBuffer.delete(watchId);
      if (bufferedSignal) scheduleResourceWatchRefresh(bufferedSignal);
    } catch (error) {
      if (!liveRefreshContextMatchesCurrent(requestContext)) return;
      resourceWatchStatus = 'error';
      markLiveDataUnavailable(`Live updates unavailable for ${resource.kind}`, requestContext);
      // A resource can be listable without watch permission. Keep the current
      // list usable and surface the limitation once instead of retrying loudly.
      if (!resourceWatchErrorNotified) {
        resourceWatchErrorNotified = true;
        notify(`Live updates unavailable for ${resource.kind}: ${String(error)}`);
      }
    }
  }

  function scheduleResourceWatchRefresh(signal: ResourceWatchSignal) {
    if (
      signal.watchId !== resourceWatchId
      || !resourceWatchClusterId
      || resourceWatchClusterId !== activeClusterId
      || resourceWatchView !== activeView
      || resourceWatchDataKey !== activeLiveDataKey()
    ) return;
    if (signal.action === 'connected') {
      resourceWatchStatus = 'connected';
      resourceWatchErrorNotified = false;
      if (resourceWatchDataKey && liveDataUpdatedAt.has(resourceWatchDataKey)) {
        liveDataStatus = 'live';
        liveDataStatusMessage = '';
      }
      return;
    }
    if (signal.error && !resourceWatchErrorNotified) {
      resourceWatchErrorNotified = true;
      resourceWatchStatus = 'reconnecting';
      markLiveDataUnavailable('Live updates paused; refresh is available', {
        clusterId: resourceWatchClusterId,
        view: resourceWatchView,
        dataKey: resourceWatchDataKey,
      });
      notify(`Live updates paused: ${signal.error}`);
    }
    if (signal.error) resourceWatchStatus = 'reconnecting';
    if (!['added', 'modified', 'deleted'].includes(signal.action)) return;
    markResourceWatchRefreshPending();
    if (resourceWatchRefreshTimer) window.clearTimeout(resourceWatchRefreshTimer);
    resourceWatchRefreshTimer = window.setTimeout(() => {
      resourceWatchRefreshTimer = undefined;
      resourceWatchRefreshPending = false;
      if (!resumeRecoveryPending) resumeRecoveryContext = null;
      void refreshVisibleObjectList();
    }, 120);
  }

  async function setupResourceWatchListener() {
    if (!('__TAURI_INTERNALS__' in window)) return;
    try {
      const { listen } = await import('@tauri-apps/api/event');
      resourceWatchUnlisten = await listen<ResourceWatchSignal>('kuberniva://resource-watch', ({ payload }) => {
        if (payload.watchId !== resourceWatchId) {
          resourceWatchSignalBuffer.set(payload.watchId, payload);
          return;
        }
        scheduleResourceWatchRefresh(payload);
      });
    } catch {
      resourceWatchUnlisten = undefined;
    }
  }

  function startOverviewRefresh() {
    stopOverviewRefresh();
    if (!activeClusterId || activeView !== 'Overview') return;
    overviewRefreshTimer = window.setInterval(() => void loadClusterOverview(), 60_000);
  }

  async function loadClusterOverview(force = false) {
    if (!activeClusterId || (loadingOverview && !force)) return;
    const overviewClusterId = activeClusterId;
    const requestGeneration = ++overviewRequestGeneration;
    loadingOverview = true;
    overviewError = '';
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const response = await invoke<ClusterOverview>('read_cluster_overview', {
        kubeconfigPath: activeKubeconfigPath || kubeconfigPath || null,
        context: activeCluster,
      });
      if (requestGeneration === overviewRequestGeneration && overviewClusterId === activeClusterId) {
        clusterOverview = response;
        selectedNodeName = response.nodes.some((node) => node.name === selectedNodeName) ? selectedNodeName : response.nodes[0]?.name || '';
      }
    } catch (error) {
      if (requestGeneration === overviewRequestGeneration && overviewClusterId === activeClusterId) overviewError = String(error);
    } finally {
      if (requestGeneration === overviewRequestGeneration && overviewClusterId === activeClusterId) loadingOverview = false;
    }
  }

  async function loadClusterEvents(force = false) {
    if (!activeClusterId || (loadingEvents && !force)) return;
    if (!force && clusterEvents.length && eventsObservedAt && activeClusterId === eventsClusterId) return;
    const requestClusterId = activeClusterId;
    const requestGeneration = ++eventsRequestGeneration;
    loadingEvents = true;
    eventsError = '';
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const response = await invoke<ClusterEvent[]>('read_cluster_events', {
        kubeconfigPath: activeKubeconfigPath || kubeconfigPath || null,
        context: activeCluster,
      });
      if (requestGeneration === eventsRequestGeneration && requestClusterId === activeClusterId) {
        clusterEvents = response;
        eventsClusterId = requestClusterId;
        eventsObservedAt = new Date().toISOString();
      }
    } catch (error) {
      if (requestGeneration === eventsRequestGeneration && requestClusterId === activeClusterId) eventsError = String(error);
    } finally {
      if (requestGeneration === eventsRequestGeneration && requestClusterId === activeClusterId) loadingEvents = false;
    }
  }

  async function chooseKubeconfig(directory: boolean) {
    if (!('__TAURI_INTERNALS__' in window)) {
      notify('The file picker is available in the Kuberniva desktop app');
      return;
    }
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const selectedPath = await open({
        directory,
        multiple: false,
        title: directory ? 'Choose the kubeconfig folder' : 'Choose a kubeconfig file',
      });
      if (typeof selectedPath === 'string') kubeconfigPath = selectedPath;
    } catch (error) {
      notify(`Could not open the picker: ${String(error)}`);
    }
  }

  function closeKubeconfigModal(force = false) {
    if (loadingCatalog && !force) return;
    kubeconfigOpen = false;
    pastedKubeconfig = '';
    schedulePendingResumeRecovery();
  }

  function handleKubeconfigModalKeydown(event: KeyboardEvent) {
    if (event.key !== 'Escape') return;
    event.preventDefault();
    closeKubeconfigModal();
  }

  function handleKubeconfigSourceTabKeydown(event: KeyboardEvent, mode: KubeconfigInputMode) {
    const modes: KubeconfigInputMode[] = ['file', 'folder', 'paste'];
    const currentIndex = modes.indexOf(mode);
    let nextIndex: number | undefined;
    if (event.key === 'ArrowRight' || event.key === 'ArrowDown') nextIndex = (currentIndex + 1) % modes.length;
    if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') nextIndex = (currentIndex - 1 + modes.length) % modes.length;
    if (event.key === 'Home') nextIndex = 0;
    if (event.key === 'End') nextIndex = modes.length - 1;
    if (nextIndex === undefined) return;
    event.preventDefault();
    kubeconfigInputMode = modes[nextIndex];
    void tick().then(() => document.getElementById(`kubeconfig-source-tab-${modes[nextIndex!]}`)?.focus());
  }

  function startSidebarResize(event: PointerEvent) {
    if (sidebarHidden) return;
    const initialX = event.clientX;
    const initialWidth = sidebarWidth;
    const resize = (moveEvent: PointerEvent) => {
      sidebarWidth = Math.min(460, Math.max(230, initialWidth + moveEvent.clientX - initialX));
    };
    const stopResize = () => {
      window.removeEventListener('pointermove', resize);
      window.removeEventListener('pointerup', stopResize);
      persistWorkspace();
    };
    window.addEventListener('pointermove', resize);
    window.addEventListener('pointerup', stopResize, { once: true });
  }

  function savePaneSize(key: string, value: number) {
    try { window.localStorage.setItem(key, String(Math.round(value))); } catch { /* Optional layout preference. */ }
  }

  function loadPaneSize(key: string, fallback: number, minimum: number, maximum: number) {
    try {
      const raw = window.localStorage.getItem(key);
      if (raw === null) return fallback;
      const saved = Number(raw);
      return Number.isFinite(saved) ? Math.min(maximum, Math.max(minimum, saved)) : fallback;
    } catch {
      return fallback;
    }
  }

  function startResourceNavigatorResize(event: PointerEvent) {
    const initialX = event.clientX;
    const initialWidth = resourceNavigatorWidth;
    const resize = (moveEvent: PointerEvent) => {
      resourceNavigatorWidth = Math.min(440, Math.max(220, initialWidth + moveEvent.clientX - initialX));
    };
    const stopResize = () => {
      window.removeEventListener('pointermove', resize);
      window.removeEventListener('pointerup', stopResize);
      savePaneSize(resourceNavigatorWidthStorageKey, resourceNavigatorWidth);
    };
    window.addEventListener('pointermove', resize);
    window.addEventListener('pointerup', stopResize, { once: true });
  }

  function startResourceObjectPaneResize(event: PointerEvent) {
    const initialX = event.clientX;
    const initialWidth = resourceObjectPaneWidth;
    const resize = (moveEvent: PointerEvent) => {
      resourceObjectPaneWidth = Math.min(520, Math.max(180, initialWidth + moveEvent.clientX - initialX));
    };
    const stopResize = () => {
      window.removeEventListener('pointermove', resize);
      window.removeEventListener('pointerup', stopResize);
      savePaneSize(resourceObjectPaneWidthStorageKey, resourceObjectPaneWidth);
    };
    window.addEventListener('pointermove', resize);
    window.addEventListener('pointerup', stopResize, { once: true });
  }

  function toggleSidebar() {
    sidebarHidden = !sidebarHidden;
    persistWorkspace();
  }

  function isFavoriteCluster(clusterId: string) {
    return favoriteClusterIds.includes(clusterId);
  }

  function favoriteLabel(cluster: Cluster) {
    return favoriteClusterNames[cluster.id] || cluster.name;
  }

  function openFavoriteContextMenu(event: MouseEvent, cluster: Cluster) {
    event.preventDefault();
    const menuWidth = 190;
    const menuHeight = 92;
    favoriteContextMenu = {
      clusterId: cluster.id,
      x: Math.min(event.clientX, window.innerWidth - menuWidth - 12),
      y: Math.min(event.clientY, window.innerHeight - menuHeight - 12),
    };
  }

  function startFavoriteRename(clusterId: string) {
    const cluster = clusters.find((candidate) => candidate.id === clusterId);
    if (!cluster) return;
    favoriteRenameId = clusterId;
    favoriteRenameValue = favoriteLabel(cluster);
    favoriteContextMenu = null;
    void tick().then(() => document.querySelector<HTMLInputElement>('.favorite-rename input')?.select());
  }

  function cancelFavoriteRename() {
    favoriteRenameId = '';
    favoriteRenameValue = '';
    schedulePendingResumeRecovery();
  }

  function saveFavoriteRename() {
    const cluster = clusters.find((candidate) => candidate.id === favoriteRenameId);
    if (!cluster) return cancelFavoriteRename();
    const label = favoriteRenameValue.trim().slice(0, 80);
    favoriteClusterNames = { ...favoriteClusterNames };
    if (label && label !== cluster.name) favoriteClusterNames[cluster.id] = label;
    else delete favoriteClusterNames[cluster.id];
    persistWorkspace();
    notify(label && label !== cluster.name ? `Shortcut renamed to ${label}.` : 'Shortcut reset to the cluster name.');
    cancelFavoriteRename();
  }

  function toggleFavoriteCluster(cluster: Cluster) {
    if (isFavoriteCluster(cluster.id)) {
      favoriteClusterIds = favoriteClusterIds.filter((id) => id !== cluster.id);
      const { [cluster.id]: _removedFavoriteName, ...remainingFavoriteNames } = favoriteClusterNames;
      favoriteClusterNames = remainingFavoriteNames;
      notify(`${cluster.name} removed from Favorites.`);
    } else if (favoriteClusterIds.length >= 10) {
      notify('Favorites is limited to 10 cluster shortcuts. Remove one before adding another.');
      return;
    } else {
      favoriteClusterIds = [...favoriteClusterIds, cluster.id];
      notify(`${cluster.name} added to Favorites.`);
    }
    persistWorkspace();
  }

  function updateCluster(id: string, changes: Partial<Cluster>) {
    clusters = clusters.map((cluster) => cluster.id === id ? { ...cluster, ...changes } : cluster);
  }

  async function chooseNamespace(nextNamespace: string) {
    if (namespace === nextNamespace) {
      namespaceOpen = false;
      return;
    }
    cancelPendingLiveRefresh();
    clearResourceObjectSelection();
    const resourceToReload = selectedResource;
    if (activeClusterId && resourceToReload) clearLiveDataFreshness(activeClusterId, resourceToReload, namespace);
    if (activeClusterId && workloadResource) clearLiveDataFreshness(activeClusterId, workloadResource, namespace);
    namespace = nextNamespace;
    catalogPermissionsReady = false;
    permissionError = '';
    namespaceOpen = false;
    clusterPickerOpen = false;
    selectedResource = null;
    resourceObjects = [];
    closeEditor();
    closeYamlEditor();
    if (activeView === 'Workloads') {
      await navigateTo('Workloads');
    } else if (activeView === 'Resources' && resourceToReload) {
      await openResource(resourceToReload);
    } else if (activeView === 'Logs') {
      closeLogs();
      await navigateTo('Workloads');
    }
    void loadCatalogPermissions();
    rememberActiveClusterSession();
    persistWorkspace();
  }

  function selectResourceCategory(category: ResourceCategory | 'All resources') {
    cancelPendingLiveRefresh();
    clearResourceObjectSelection();
    if (activeClusterId && selectedResource) clearLiveDataFreshness(activeClusterId, selectedResource, namespace);
    selectedCategory = category;
    sidebarResourceCategory = category;
    resourceSearch = '';
    clusterPickerOpen = false;
    selectedResource = null;
    resourceObjects = [];
    closeEditor();
    closeYamlEditor();
    const firstResource = resourceWorkspaceResources
      .filter((resource) => category === 'All resources' || resource.category === category)
      .sort((left, right) => left.kind.localeCompare(right.kind))[0];
    if (firstResource) void openResource(firstResource);
  }

  async function refreshClusterConnection(cluster: Cluster) {
    if (!activeClusterId || activeClusterId !== cluster.id) return false;
    const requestClusterId = cluster.id;
    resourceRequestGeneration += 1;
    workloadRequestGeneration += 1;
    overviewRequestGeneration += 1;
    eventsRequestGeneration += 1;
    loadingOverview = false;
    loadingEvents = false;
    stopLiveObjectRefresh();
    clearResourceObjectSelection();
    loadingObjects = false;
    loadingWorkloads = false;
    invalidateCurrentAccessDecisions();
    clearClusterObjectCache(requestClusterId);
    catalogCache.delete(requestClusterId);
    resourceObjects = [];
    workloadObjects = [];
    relatedPods = null;
    relatedObject = null;
    selectedResource = null;
    closeLogs();
    closeEditor();
    closeYamlEditor();
    liveDataStatus = 'loading';
    liveDataStatusMessage = 'Waiting for a fresh live snapshot';
    catalogError = '';
    overviewError = '';
    eventsError = '';
    loadingCatalog = true;
    updateCluster(requestClusterId, { status: 'Connecting', tone: 'blue' });
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('invalidate_cluster_client', {
        kubeconfigPath: cluster.kubeconfigPath || kubeconfigPath || null,
        context: cluster.name,
      }).catch(() => undefined);
      const response = await invoke<ClusterCatalog>('discover_cluster_catalog', {
        kubeconfigPath: cluster.kubeconfigPath || kubeconfigPath || null,
        context: cluster.name,
      });
      if (requestClusterId !== activeClusterId) return false;
      catalog = response;
      catalogCache.set(requestClusterId, response);
      storeCatalog(requestClusterId, response);
      await loadCatalogPermissions(true);
      updateCluster(requestClusterId, { status: 'Connected', tone: 'green' });
      return true;
    } catch (error) {
      if (requestClusterId === activeClusterId) {
        catalogError = String(error);
        updateCluster(requestClusterId, { status: 'Connection failed', tone: 'red' });
        notify(`Could not refresh ${cluster.name}: ${catalogError}`);
      }
      return false;
    } finally {
      if (requestClusterId === activeClusterId) loadingCatalog = false;
    }
  }

  async function refreshCurrentView() {
    const cluster = clusters.find((candidate) => candidate.id === activeClusterId);
    if (!cluster || refreshingCluster || loadingCatalog) return;
    if (hasProtectedWorkflow()) {
      markResumeRecoveryPending();
      markLiveDataPaused();
      return;
    }
    const requestClusterId = cluster.id;
    const requestView = activeView;
    const requestNamespace = namespace;
    const previousWorkloadResource = workloadResource;
    const previousSelectedResource = selectedResource;
    refreshingCluster = true;
    try {
      if (!await refreshClusterConnection(cluster)) return;
      if (requestClusterId !== activeClusterId || requestView !== activeView || requestNamespace !== namespace) return;
      if (requestView === 'Overview') {
        await loadClusterOverview(true);
        startOverviewRefresh();
        return;
      }
      if (requestView === 'Events') {
        await loadClusterEvents(true);
        return;
      }
      if (requestView === 'Workloads') {
        const resource = workloadResources.find((candidate) => previousWorkloadResource && resourceKey(candidate) === resourceKey(previousWorkloadResource))
          || workloadResources.find((candidate) => candidate.kind === 'Deployment')
          || workloadResources[0];
        if (!resource) return;
        clearResourceObjectCacheEntry(requestClusterId, resource, namespace);
        await loadWorkloadResource(resource);
        startLiveObjectRefresh();
        return;
      }
      if (requestView === 'Resources') {
        const resource = accessibleCatalogResources.find((candidate) => previousSelectedResource && resourceKey(candidate) === resourceKey(previousSelectedResource));
        if (!resource) {
          selectedResource = null;
          resourceObjects = [];
          clearResourceObjectSelection();
          return;
        }
        selectedResource = resource;
        clearResourceObjectCacheEntry(requestClusterId, resource, namespace);
        await openResource(resource);
        return;
      }
      if (requestView === 'Logs') {
        logRequestGeneration += 1;
        loadingLogs = false;
        await loadLogs(true);
      }
    } catch (error) {
      const failedResource = requestView === 'Workloads' ? previousWorkloadResource : previousSelectedResource;
      if (requestClusterId === activeClusterId && requestView === activeView && requestNamespace === namespace && failedResource && (requestView === 'Resources' || requestView === 'Workloads')) {
        markLiveDataUnavailable('Could not refresh live data', liveRefreshContext(requestClusterId, requestView, failedResource, requestNamespace));
      }
      notify(`Refresh failed for ${cluster.name}: ${String(error)}`);
    } finally {
      refreshingCluster = false;
    }
  }

  async function refreshVisibleObjectList() {
    if (!activeClusterId) return;
    if (
      loadingCatalog
      || loadingWorkloads
      || loadingObjects
      || loadingEditor
      || savingEditor
      || loadingYaml
      || savingYaml
      || deletingResource
    ) {
      markResourceWatchRefreshPending();
      if (hasPreservedLiveState()) markLiveDataPaused();
      return;
    }
    if (activeView === 'Workloads' && workloadResource) {
      const resource = workloadResource;
      clearResourceObjectCacheEntry(activeClusterId, resource, namespace);
      await loadWorkloadResource(resource, true);
      return;
    }
    if (activeView === 'Resources' && selectedResource) {
      const resource = selectedResource;
      clearResourceObjectCacheEntry(activeClusterId, resource, namespace);
      await openResource(resource, { silent: true });
    }
  }

  function flushPendingResourceWatchRefresh() {
    if (!resourceWatchRefreshPending || loadingCatalog || loadingWorkloads || loadingObjects || loadingEditor || savingEditor || loadingYaml || savingYaml || deletingResource) return;
    resourceWatchRefreshPending = false;
    if (!resumeRecoveryPending) resumeRecoveryContext = null;
    void refreshVisibleObjectList();
  }

  function queueLiveResumeRecovery() {
    if (!activeClusterId || !['Workloads', 'Resources'].includes(activeView) || refreshingCluster || loadingCatalog || !activeLiveDataNeedsRecovery()) return;
    if (liveDataStatus !== 'unavailable') markLiveDataStale('Live data is stale; refresh is available');
    if (hasProtectedWorkflow()) {
      markResumeRecoveryPending();
      markLiveDataPaused('Refresh paused while your current workflow is open');
      return;
    }
    markResumeRecoveryPending();
    if (resumeRecoveryTimer) window.clearTimeout(resumeRecoveryTimer);
    resumeRecoveryTimer = window.setTimeout(() => {
      resumeRecoveryTimer = undefined;
      if (!resumeRecoveryPending || hasProtectedWorkflow()) return;
      if (!pendingResumeContextMatchesCurrentView()) {
        resumeRecoveryPending = false;
        resumeRecoveryContext = null;
        return;
      }
      resumeRecoveryPending = false;
      resumeRecoveryContext = null;
      void refreshCurrentView();
    }, 250);
  }

  async function setupWindowFocusListener() {
    if (!('__TAURI_INTERNALS__' in window)) return;
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      stopWindowFocusListening = await getCurrentWindow().onFocusChanged(({ payload: focused }) => {
        if (!focused) {
          lastHiddenAt = Date.now();
          return;
        }
        const hiddenDuration = lastHiddenAt ? Date.now() - lastHiddenAt : 0;
        lastHiddenAt = 0;
        if (hiddenDuration > 5_000) queueLiveResumeRecovery();
      });
    } catch {
      stopWindowFocusListening?.();
      stopWindowFocusListening = undefined;
    }
  }

  function applyTheme(nextTheme: ThemeMode) {
    theme = nextTheme;
    if (typeof document !== 'undefined') document.documentElement.dataset.theme = nextTheme;
    try {
      window.localStorage.setItem(themeStorageKey, nextTheme);
    } catch {
      // A theme preference is optional and should never block the workspace.
    }
  }

  function revealWindow() {
    if (!('__TAURI_INTERNALS__' in window)) return;
    const fontsReady = Promise.race([document.fonts.ready, new Promise((resolve) => window.setTimeout(resolve, 300))]);
    void fontsReady.then(() => requestAnimationFrame(() => {
      void import('@tauri-apps/api/window')
        .then(({ getCurrentWindow }) => getCurrentWindow().show())
        .catch(() => undefined);
    }));
  }

  async function loadAppVersion() {
    if (!('__TAURI_INTERNALS__' in window)) return;
    try {
      const { getVersion } = await import('@tauri-apps/api/app');
      appVersion = await getVersion();
    } catch {
      appVersion = '';
    }
  }

  function friendlyUpdateError(error: unknown) {
    const message = String(error);
    if (/404|valid release JSON|Not Found/i.test(message)) return 'No update has been published yet.';
    if (/network|connect|dns|offline|timed out/i.test(message)) return 'Could not reach GitHub. Check your connection and try again.';
    if (/signature/i.test(message)) return 'The downloaded update failed its signature check and was not installed.';
    return message.replace(/^Error:\s*/, '');
  }

  async function checkForUpdates(silent = false) {
    if (!('__TAURI_INTERNALS__' in window) || updateState === 'checking' || updateState === 'downloading' || updateState === 'ready') return;
    updateState = 'checking';
    updateError = '';
    try {
      const { check } = await import('@tauri-apps/plugin-updater');
      const update = await check();
      updateCheckedAt = new Date().toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
      if (update) {
        pendingUpdate = update as unknown as PendingUpdate;
        updateState = 'available';
        notify(`Kuberniva ${update.version} is available. Open Settings to update.`);
      } else {
        pendingUpdate = null;
        updateState = 'current';
        if (!silent) notify('Kuberniva is up to date.');
      }
    } catch (error) {
      // A quiet launch check never interrupts; a manual check explains what happened.
      updateState = silent ? 'idle' : 'error';
      updateError = silent ? '' : friendlyUpdateError(error);
    }
  }

  async function installUpdate() {
    if (!pendingUpdate || updateState === 'downloading') return;
    updateState = 'downloading';
    updateProgress = 0;
    let total = 0;
    let received = 0;
    try {
      await pendingUpdate.downloadAndInstall((event) => {
        if (event.event === 'Started') total = event.data?.contentLength || 0;
        if (event.event === 'Progress') {
          received += event.data?.chunkLength || 0;
          updateProgress = total ? Math.min(1, received / total) : null;
        }
      });
      updateState = 'ready';
      updateProgress = 1;
    } catch (error) {
      updateState = 'error';
      updateError = friendlyUpdateError(error);
      updateProgress = null;
    }
  }

  async function restartToUpdate() {
    const { relaunch } = await import('@tauri-apps/plugin-process');
    await relaunch();
  }

  function toggleTheme() {
    applyTheme(theme === 'light' ? 'dark' : 'light');
    persistWorkspace();
    notify(`${theme === 'dark' ? 'Dark' : 'Light'} mode enabled.`);
  }

  function loadUiScalePreference() {
    try {
      const saved = Number(window.localStorage.getItem(uiScaleStorageKey));
      return Number.isFinite(saved) && saved >= 0.8 && saved <= 1.25 ? saved : 0.9;
    } catch {
      return 0.9;
    }
  }

  async function applyUiScale(nextScale: number, save = true) {
    uiScale = Math.min(1.25, Math.max(0.8, Math.round(nextScale * 20) / 20));
    if (save) {
      try { window.localStorage.setItem(uiScaleStorageKey, String(uiScale)); } catch { /* Optional preference. */ }
    }
    if ('__TAURI_INTERNALS__' in window) {
      try {
        const { getCurrentWebview } = await import('@tauri-apps/api/webview');
        await getCurrentWebview().setZoom(uiScale);
        return;
      } catch (error) {
        if (save) notify(`Could not resize the interface: ${String(error)}`);
      }
    }
    document.documentElement.style.zoom = String(uiScale);
  }

  function adjustUiScale(change: number) {
    void applyUiScale(uiScale + change);
  }

  function openCommandSearch() {
    commandQuery = '';
    commandOpen = true;
    void tick().then(() => document.getElementById('global-command-search')?.focus());
  }

  function closeCommandSearch() {
    commandOpen = false;
    commandQuery = '';
    schedulePendingResumeRecovery();
  }

  function handleCommandKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault();
      closeCommandSearch();
      return;
    }
    if (event.key === 'Enter' && globalSearchResults[0]) {
      event.preventDefault();
      void openGlobalSearchResult(globalSearchResults[0]);
    }
  }

  async function openGlobalSearchResult(result: GlobalSearchResult) {
    closeCommandSearch();
    if (result.resource.category === 'Workloads') {
      workloadResource = result.resource;
      await navigateTo('Workloads');
      if (result.type === 'object' && result.object) await openObject(result.resource, result.object);
      return;
    }
    selectedCategory = result.resource.category;
    sidebarResourceCategory = result.resource.category;
    await navigateTo('Resources');
    await openResource(result.resource);
    if (result.type === 'object' && result.object) await openObject(result.resource, result.object);
  }

  function selectSidebarResourceCategory(category: ResourceCategory | 'All resources') {
    sidebarResourceCategory = category;
    sidebarResourceSearch = '';
  }

  function toggleClusterCli() {
    if (!activeClusterId) {
      notify('Select a cluster before opening the terminal.');
      return;
    }
    cliOpen = !cliOpen;
    if (cliOpen) void tick().then(() => cliInput?.focus());
  }

  function loadCliHistory() {
    try {
      const parsed = JSON.parse(window.localStorage.getItem(cliHistoryStorageKey) || '[]');
      return Array.isArray(parsed) ? parsed.filter((entry): entry is string => typeof entry === 'string').slice(0, 100) : [];
    } catch {
      return [];
    }
  }

  function rememberCliCommand(command: string) {
    cliHistory = [command, ...cliHistory.filter((entry) => entry !== command)].slice(0, 100);
    cliHistoryIndex = -1;
    try {
      window.localStorage.setItem(cliHistoryStorageKey, JSON.stringify(cliHistory));
    } catch {
      // History is a convenience only.
    }
  }

  async function appendCliLines(lines: CliLine[]) {
    const nearBottom = !cliViewport || cliViewport.scrollHeight - cliViewport.scrollTop - cliViewport.clientHeight < 48;
    const merged = cliLines.concat(lines);
    cliLines = merged.length > cliMaxLines ? merged.slice(merged.length - cliMaxLines) : merged;
    await tick();
    if (nearBottom && cliViewport) cliViewport.scrollTop = cliViewport.scrollHeight;
  }

  function stripAnsi(text: string) {
    // eslint-disable-next-line no-control-regex
    return text.replace(/\x1b\[[0-9;?]*[A-Za-z]/g, '');
  }

  function swapCliSession(nextClusterId: string) {
    if (cliSessionClusterId) {
      cliSessions.set(cliSessionClusterId, { lines: cliLines, runId: cliRunId, running: runningCli, startedAt: cliRunStartedAt, draft: cliCommand, open: cliOpen, expanded: cliExpanded });
    }
    const next = nextClusterId ? cliSessions.get(nextClusterId) : undefined;
    cliLines = next?.lines ?? [];
    cliRunId = next?.runId ?? null;
    runningCli = next?.running ?? false;
    cliRunStartedAt = next?.startedAt ?? 0;
    cliCommand = next?.draft ?? '';
    cliOpen = next?.open ?? false;
    cliExpanded = next?.expanded ?? false;
    cliHistoryIndex = -1;
    cliSessionClusterId = nextClusterId;
  }

  function parkedCliSession(runId: string) {
    for (const session of cliSessions.values()) if (session.runId === runId) return session;
    return undefined;
  }

  function cliExitSummary(payload: CliExitEvent, startedAt: number): CliLine {
    const seconds = ((Date.now() - startedAt) / 1000).toFixed(1);
    const text = payload.cancelled
      ? `^C stopped after ${seconds}s`
      : payload.error
        ? `Failed: ${payload.error}`
        : payload.success ? `✓ done in ${seconds}s` : `✗ exited with code ${payload.exitCode ?? 'unknown'} after ${seconds}s`;
    return { stream: payload.success || payload.cancelled ? 'meta' : 'stderr', text };
  }

  function setupCliListeners() {
    cliListenersReady ??= (async () => {
      const { listen } = await import('@tauri-apps/api/event');
      await listen<CliOutputEvent>('kuberniva://cli-output', ({ payload }) => {
        const lines = payload.chunks.map((chunk): CliLine => ({ stream: chunk.stream, text: stripAnsi(chunk.text.replace(/\r?\n$/, '')) }));
        if (payload.runId === cliRunId) {
          void appendCliLines(lines);
          return;
        }
        // Commands keep running while their cluster's terminal is parked.
        const parked = parkedCliSession(payload.runId);
        if (parked) parked.lines = parked.lines.concat(lines).slice(-cliMaxLines);
      });
      await listen<CliExitEvent>('kuberniva://cli-exit', ({ payload }) => {
        if (payload.runId !== cliRunId) {
          const parked = parkedCliSession(payload.runId);
          if (parked) {
            parked.lines = parked.lines.concat(cliExitSummary(payload, parked.startedAt)).slice(-cliMaxLines);
            parked.runId = null;
            parked.running = false;
          }
          return;
        }
        void appendCliLines([cliExitSummary(payload, cliRunStartedAt)]);
        cliRunId = null;
        runningCli = false;
        schedulePendingResumeRecovery();
        void tick().then(() => cliInput?.focus());
      });
    })();
    return cliListenersReady;
  }

  function recallCliHistory(direction: 1 | -1) {
    if (!cliHistory.length) return;
    if (cliHistoryIndex === -1) cliHistoryDraft = cliCommand;
    const nextIndex = Math.min(cliHistory.length - 1, Math.max(-1, cliHistoryIndex + direction));
    cliHistoryIndex = nextIndex;
    cliCommand = nextIndex === -1 ? cliHistoryDraft : cliHistory[nextIndex];
    void tick().then(() => cliInput?.setSelectionRange(cliCommand.length, cliCommand.length));
  }

  function handleClusterCliKeydown(event: KeyboardEvent) {
    const input = event.currentTarget as HTMLTextAreaElement;
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      void runClusterCli();
      return;
    }
    if (event.ctrlKey && event.key.toLowerCase() === 'c' && runningCli && input.selectionStart === input.selectionEnd) {
      event.preventDefault();
      void cancelClusterCli();
      return;
    }
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'l') {
      event.preventDefault();
      clearClusterCli();
      return;
    }
    const onFirstLine = !input.value.slice(0, input.selectionStart ?? 0).includes('\n');
    const onLastLine = !input.value.slice(input.selectionEnd ?? 0).includes('\n');
    if (event.key === 'ArrowUp' && onFirstLine) {
      event.preventDefault();
      recallCliHistory(1);
    } else if (event.key === 'ArrowDown' && onLastLine && cliHistoryIndex !== -1) {
      event.preventDefault();
      recallCliHistory(-1);
    }
  }

  function clearClusterCli() {
    cliLines = [];
  }

  async function copyClusterCliOutput() {
    const text = cliLines.map((line) => line.text).join('\n');
    if (!text) return;
    try {
      const { writeText } = await import('@tauri-apps/plugin-clipboard-manager');
      await writeText(text, { label: 'Kuberniva CLI output' });
    } catch {
      await navigator.clipboard.writeText(text).catch(() => undefined);
    }
    notify('CLI output copied.');
  }

  async function cancelClusterCli() {
    if (!cliRunId) return;
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('cancel_cluster_command', { runId: cliRunId }).catch(() => undefined);
  }

  async function runClusterCli(preset?: string) {
    const command = (preset ?? cliCommand).trim();
    if (!activeClusterId) {
      notify('Select a cluster before opening the CLI.');
      return;
    }
    if (!command) {
      notify('Enter a command, for example: kubectl get pods or helm list');
      return;
    }
    if (runningCli) {
      notify('A command is still running. Press Ctrl+C or Stop to end it first.');
      return;
    }
    if (command === 'clear') {
      clearClusterCli();
      cliCommand = '';
      return;
    }
    const promptScope = namespace === 'all namespaces' ? activeCluster : `${activeCluster}/${namespace}`;
    rememberCliCommand(command);
    cliCommand = '';
    const runId = typeof crypto.randomUUID === 'function' ? crypto.randomUUID() : `cli-${Date.now()}-${Math.random().toString(36).slice(2)}`;
    cliRunId = runId;
    cliRunStartedAt = Date.now();
    runningCli = true;
    await appendCliLines([{ stream: 'prompt', text: `${promptScope} $ ${command}` }]);
    try {
      await setupCliListeners();
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke<string>('start_cluster_command', {
        runId,
        request: {
          kubeconfigPath: activeKubeconfigPath || kubeconfigPath || null,
          context: activeCluster,
          namespace: namespace === 'all namespaces' ? null : namespace,
          command,
          shell: true,
        },
      });
    } catch (error) {
      if (cliRunId === runId) {
        cliRunId = null;
        runningCli = false;
        await appendCliLines([{ stream: 'stderr', text: String(error) }]);
      }
    }
  }

  async function navigateTo(view: View) {
    // Detail workflows belong to their view, while the independent API
    // navigator stays open until the user explicitly closes it.
    cancelPendingLiveRefresh();
    clearResourceObjectSelection();
    clusterPickerOpen = false;
    namespaceOpen = false;
    commandOpen = false;
    commandQuery = '';
    if (view !== 'Logs') closeLogs();
    closeEditor();
    closeYamlEditor();
    selectedResource = null;
    relatedPods = null;
    relatedObject = null;
    activeView = view;
    if (view === 'Workloads' || view === 'Resources') {
      liveDataStatus = 'loading';
      liveDataStatusMessage = 'Waiting for live data';
    }
    if (view === 'Overview' && activeClusterId) {
      stopLiveObjectRefresh();
      void loadClusterOverview();
      startOverviewRefresh();
      return;
    }
    if (view === 'Events' && activeClusterId) {
      stopLiveObjectRefresh();
      void loadClusterEvents();
      return;
    }
    if (view !== 'Overview') stopOverviewRefresh();
    if (view === 'Resources') {
      startLiveObjectRefresh();
      return;
    }
    if (view !== 'Workloads' || !activeClusterId) {
      stopLiveObjectRefresh();
      return;
    }
    if (loadingWorkloads) return;
    const currentWorkloadResource = workloadResource;
    const preferredResource = (currentWorkloadResource && workloadResources.some((resource) => resourceKey(resource) === resourceKey(currentWorkloadResource)) ? currentWorkloadResource : null)
      || workloadResources.find((resource) => resource.kind === 'Deployment')
      || workloadResources[0];
    if (!preferredResource) return;
    await loadWorkloadResource(preferredResource);
    startLiveObjectRefresh();
  }

  async function loadWorkloadResource(resource: ResourceDescriptor, silent = false) {
    const requestGeneration = ++workloadRequestGeneration;
    const requestClusterId = activeClusterId;
    const requestNamespace = namespace;
    const requestView = activeView;
    const requestResourceKey = resourceKey(resource);
    if (!await mayListResource(resource)) {
      if (requestGeneration === workloadRequestGeneration && requestClusterId === activeClusterId && requestNamespace === namespace) {
        workloadResource = null;
        workloadObjects = [];
        loadingWorkloads = false;
        liveDataStatus = 'loaded';
        liveDataStatusMessage = 'This workload type is not available to the current identity';
      }
      return;
    }
    workloadResource = resource;
    const cacheKey = resourceObjectCacheKey(requestClusterId, resource, requestNamespace);
    if (!silent && activeView === 'Workloads') {
      liveDataStatus = 'loading';
      liveDataStatusMessage = 'Loading live data';
    }
    const cachedObjects = resourceObjectCache.get(cacheKey);
    if (cachedObjects) {
      workloadObjects = cachedObjects;
      reconcileWorkloadObjectSelection(cachedObjects);
      loadingWorkloads = false;
      if (requestView === 'Workloads') markLiveDataAvailable(resource, requestClusterId, requestNamespace, 'Workloads');
      return;
    }
    if (!silent) workloadObjects = [];
    if (!silent) loadingWorkloads = true;
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const response = await invoke<ResourceObject[]>('list_resource_objects', {
        request: {
          kubeconfigPath: activeKubeconfigPath || kubeconfigPath || null,
          context: activeCluster,
          group: resource.group,
          version: resource.version,
          kind: resource.kind,
          plural: resource.plural,
          namespaced: resource.namespaced,
          namespace: requestNamespace,
        },
      });
      if (requestGeneration !== workloadRequestGeneration
        || requestClusterId !== activeClusterId
        || requestNamespace !== namespace
        || requestView !== activeView
        || !workloadResource
        || resourceKey(workloadResource) !== requestResourceKey) return;
      workloadObjects = response;
      resourceObjectCache.set(cacheKey, response);
      reconcileWorkloadObjectSelection(response);
      markLiveDataAvailable(resource, requestClusterId, requestNamespace, 'Workloads');
      if (
        editorResource
        && editorObject
        && resourceKey(editorResource) === requestResourceKey
        && !response.some((object) => object.name === editorObject?.name && object.namespace === editorObject?.namespace)
      ) {
        if (!hasPreservedLiveState()) {
          const removedName = editorObject.name;
          closeEditor(false);
          closeYamlEditor();
          notify(resource.kind + ' ' + removedName + ' is no longer present in this namespace');
        } else {
          markLiveDataPaused(`${resource.kind} changed while your workflow is open`);
          markResourceWatchRefreshPending();
        }
      }
    } catch (error) {
      if (requestGeneration === workloadRequestGeneration && requestClusterId === activeClusterId && requestView === activeView) {
        markLiveDataUnavailable(`Could not load ${resource.kind}`, liveRefreshContext(requestClusterId, 'Workloads', resource, requestNamespace));
        notify(`Could not load ${resource.kind}s: ${String(error)}`);
      }
    } finally {
      if (requestGeneration === workloadRequestGeneration && requestClusterId === activeClusterId && requestView === activeView && !silent) loadingWorkloads = false;
      if (requestGeneration === workloadRequestGeneration && requestClusterId === activeClusterId && requestNamespace === namespace && requestView === activeView && workloadResource && resourceKey(workloadResource) === requestResourceKey) {
        flushPendingResourceWatchRefresh();
      }
    }
  }

  async function selectWorkloadResource(resource: ResourceDescriptor) {
    cancelPendingLiveRefresh();
    clearResourceObjectSelection();
    if (activeClusterId && workloadResource) clearLiveDataFreshness(activeClusterId, workloadResource, namespace);
    workloadSearch = '';
    // A resource-type switch starts a fresh workload workspace. Do not leave a
    // Pod/deployment inspector, terminal, or log stream attached to the
    // previous type.
    closeWorkloadLogs();
    closeEditor();
    closeYamlEditor();
    relatedPods = null;
    relatedObject = null;
    await loadWorkloadResource(resource);
    startLiveObjectRefresh();
  }

  type SidebarTypeMenu = 'workload' | 'resource';

  function sidebarTypeMenuId(menu: SidebarTypeMenu) {
    return `sidebar-${menu}-type-options`;
  }

  function sidebarTypeTriggerId(menu: SidebarTypeMenu) {
    return `sidebar-${menu}-type-trigger`;
  }

  function closeSidebarTypeMenus(restoreFocus?: SidebarTypeMenu) {
    sidebarWorkloadMenuOpen = false;
    sidebarResourceMenuOpen = false;
    sidebarResourceSearch = '';
    if (restoreFocus) void tick().then(() => document.getElementById(sidebarTypeTriggerId(restoreFocus))?.focus());
  }

  function closeSidebarTypeMenu(menu: SidebarTypeMenu, restoreFocus = false) {
    if (menu === 'workload') {
      sidebarWorkloadMenuOpen = false;
      if (sidebarResourceMenuOpen) sidebarTypeActivePanel = 'resource';
    } else {
      sidebarResourceMenuOpen = false;
      if (sidebarWorkloadMenuOpen) sidebarTypeActivePanel = 'workload';
    }
    if (restoreFocus) void tick().then(() => document.getElementById(sidebarTypeTriggerId(menu))?.focus());
  }

  async function openSidebarTypeMenu(menu: SidebarTypeMenu, focusLast = false) {
    if (menu === 'workload') sidebarWorkloadMenuOpen = true;
    else sidebarResourceMenuOpen = true;
    sidebarTypeActivePanel = menu;
    await tick();
    const options = Array.from(document.querySelectorAll<HTMLButtonElement>(`#${sidebarTypeMenuId(menu)} [role="option"]`));
    if (!options.length) return;
    const selected = options.find((option) => option.getAttribute('aria-selected') === 'true');
    (focusLast ? options.at(-1) : selected || options[0])?.focus();
  }

  function activateSidebarTypeSection(menu: SidebarTypeMenu) {
    if (menu === 'resource' && customApiWorkspace) {
      void openResourceWorkspace(false);
      return;
    }
    const isOpen = menu === 'workload' ? sidebarWorkloadMenuOpen : sidebarResourceMenuOpen;
    const targetView: View = menu === 'workload' ? 'Workloads' : 'Resources';
    if (activeView === targetView) {
      if (menu === 'workload') {
        sidebarWorkloadMenuOpen = !isOpen;
        if (!sidebarWorkloadMenuOpen && sidebarResourceMenuOpen) sidebarTypeActivePanel = 'resource';
      } else {
        sidebarResourceMenuOpen = !isOpen;
        if (!sidebarResourceMenuOpen && sidebarWorkloadMenuOpen) sidebarTypeActivePanel = 'workload';
      }
      if (!isOpen) sidebarTypeActivePanel = menu;
      return;
    }
    void navigateTo(targetView).then(() => {
      if (menu === 'workload') sidebarWorkloadMenuOpen = true;
      else sidebarResourceMenuOpen = true;
    });
    if (menu === 'workload') sidebarWorkloadMenuOpen = true;
    else sidebarResourceMenuOpen = true;
    sidebarTypeActivePanel = menu;
  }

  async function openResourceWorkspace(custom: boolean) {
    const category: ResourceCategory | 'All resources' = custom ? 'Custom Resources' : 'All resources';
    selectedCategory = category;
    sidebarResourceCategory = category;
    sidebarResourceSearch = '';
    sidebarResourceMenuOpen = true;
    sidebarTypeActivePanel = 'resource';
    await navigateTo('Resources');
    const firstResource = (custom ? customApiResources : resourceWorkspaceResources)
      .slice()
      .sort((left, right) => left.kind.localeCompare(right.kind))[0];
    if (firstResource) await openResource(firstResource);
  }

  function handleSidebarTypeTriggerKeydown(event: KeyboardEvent, menu: SidebarTypeMenu) {
    if (event.key !== 'ArrowDown' && event.key !== 'ArrowUp') return;
    event.preventDefault();
    void openSidebarTypeMenu(menu, event.key === 'ArrowUp');
  }

  function handleSidebarTypeMenuKeydown(event: KeyboardEvent, menu: SidebarTypeMenu) {
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      closeSidebarTypeMenu(menu, true);
      return;
    }
    const options = Array.from(document.querySelectorAll<HTMLButtonElement>(`#${sidebarTypeMenuId(menu)} [role="option"]`));
    if (!options.length) return;
    const current = event.target instanceof Element ? event.target.closest<HTMLButtonElement>('[role="option"]') : null;
    const currentIndex = current ? options.indexOf(current) : -1;
    let nextIndex: number | null = null;
    if (event.key === 'ArrowDown') nextIndex = currentIndex < 0 ? 0 : (currentIndex + 1) % options.length;
    else if (event.key === 'ArrowUp') nextIndex = currentIndex < 0 ? options.length - 1 : (currentIndex - 1 + options.length) % options.length;
    else if (current && event.key === 'Home') nextIndex = 0;
    else if (current && event.key === 'End') nextIndex = options.length - 1;
    if (nextIndex === null) return;
    event.preventDefault();
    options[nextIndex]?.focus();
  }

  async function selectSidebarResourceType(resource: ResourceDescriptor) {
    selectedCategory = resource.category;
    sidebarResourceCategory = resource.category;
    sidebarTypeActivePanel = 'resource';
    await navigateTo('Resources');
    await openResource(resource);
    sidebarResourceMenuOpen = true;
  }

  async function selectSidebarWorkloadType(resource: ResourceDescriptor) {
    sidebarTypeActivePanel = 'workload';
    if (activeView !== 'Workloads') {
      workloadResource = resource;
      await navigateTo('Workloads');
      sidebarWorkloadMenuOpen = true;
      return;
    }
    await selectWorkloadResource(resource);
    sidebarWorkloadMenuOpen = true;
  }

  function objectNamespace(object: ResourceObject) {
    return object.namespace || (namespace === 'all namespaces' ? '' : namespace);
  }

  // Everyday kinds lead their section; everything else follows alphabetically.
  const commonResourceKinds = ['ConfigMap', 'Secret', 'Service', 'Ingress', 'PersistentVolumeClaim', 'ServiceAccount', 'Role', 'RoleBinding', 'NetworkPolicy', 'Gateway', 'HTTPRoute', 'StorageClass', 'PersistentVolume', 'Namespace', 'Node'];
  const recentResourcesStorageKey = 'kuberniva.recent-resources.v1';

  type WorkloadColumnKey = 'name' | 'namespace' | 'node' | 'status' | 'ready' | 'restarts' | 'cpu' | 'memory' | 'age';

  // Each fact gets its own aligned column. Namespace appears only when it varies (all
  // namespaces); node and metrics step aside while the details pane is open.
  function buildWorkloadColumns(pod: boolean, allNamespaces: boolean, detailOpen: boolean) {
    const columns: { key: WorkloadColumnKey; label: string; width: string }[] = [{ key: 'name', label: 'Name', width: 'minmax(170px, 2fr)' }];
    if (allNamespaces) columns.push({ key: 'namespace', label: 'Namespace', width: 'minmax(90px, .7fr)' });
    if (pod && !detailOpen) columns.push({ key: 'node', label: 'Node', width: 'minmax(110px, 1.1fr)' });
    columns.push({ key: 'status', label: 'Status', width: '96px' });
    if (pod) {
      columns.push({ key: 'ready', label: 'Ready', width: '52px' }, { key: 'restarts', label: 'Restarts', width: '64px' });
      if (!detailOpen) columns.push({ key: 'cpu', label: 'CPU', width: '84px' }, { key: 'memory', label: 'Memory', width: '72px' });
    }
    columns.push({ key: 'age', label: 'Age', width: '44px' });
    return columns;
  }

  const shortKindLabels: Record<string, string> = { HorizontalPodAutoscaler: 'HPAs', ReplicationController: 'RCs', PodDisruptionBudget: 'PDBs' };

  function kindTabLabel(resource: ResourceDescriptor) {
    return shortKindLabels[resource.kind] || kindLabel(resource);
  }

  function kindLabel(resource: ResourceDescriptor) {
    const kind = resource.kind;
    if (resource.plural.toLowerCase() === kind.toLowerCase()) return kind;
    if (/(s|x|ch|sh)$/i.test(kind)) return `${kind}es`;
    if (/[^aeiou]y$/i.test(kind)) return `${kind.slice(0, -1)}ies`;
    return `${kind}s`;
  }

  function sortResourcesForBrowsing(resources: ResourceDescriptor[]) {
    const rank = (resource: ResourceDescriptor) => {
      const index = commonResourceKinds.indexOf(resource.kind);
      return index === -1 ? commonResourceKinds.length : index;
    };
    return [...resources].sort((left, right) => rank(left) - rank(right) || left.kind.localeCompare(right.kind));
  }

  function buildResourceDirectory(resources: ResourceDescriptor[], search: string, custom: boolean) {
    const query = search.trim().toLowerCase();
    const matches = query
      ? resources.filter((resource) => [resource.kind, resource.plural, resource.group, resource.apiVersion].some((value) => value.toLowerCase().includes(query)))
      : resources;
    const sections = new Map<string, ResourceDescriptor[]>();
    for (const resource of matches) {
      // Custom APIs read best by owning API group; built-ins by operational area.
      const section = custom ? resource.group || 'core' : resource.category === 'Gateway APIs' ? 'Network' : resource.category;
      sections.set(section, [...(sections.get(section) || []), resource]);
    }
    const order = custom ? [...sections.keys()].sort() : resourceTreeCategories.filter((category) => sections.has(category));
    return order.map((title) => ({ title, resources: sortResourcesForBrowsing(sections.get(title) || []) }));
  }

  function loadRecentResourceKeys() {
    try {
      const parsed = JSON.parse(window.localStorage.getItem(recentResourcesStorageKey) || '[]');
      return Array.isArray(parsed) ? parsed.filter((key): key is string => typeof key === 'string').slice(0, 12) : [];
    } catch {
      return [];
    }
  }

  function rememberRecentResource(resource: ResourceDescriptor) {
    const key = resourceKey(resource);
    recentResourceKeys = [key, ...recentResourceKeys.filter((candidate) => candidate !== key)].slice(0, 12);
    try {
      window.localStorage.setItem(recentResourcesStorageKey, JSON.stringify(recentResourceKeys));
    } catch {
      // Recent types are a convenience only.
    }
  }

  type TreeSection = { title: string; resources: ResourceDescriptor[] };

  function treeKeyFor(view: string) {
    return view === 'Custom APIs' ? 'custom' : 'resources';
  }

  function treeSectionsFor(view: string) {
    return view === 'Custom APIs' ? customTreeSections : resourceTreeSections;
  }

  // Sections open on demand, and the one holding the selected kind opens by default.
  function isTreeSectionOpen(view: string, section: TreeSection, openState: Record<string, boolean>, selected: ResourceDescriptor | null) {
    const stored = openState[`${treeKeyFor(view)}:${section.title}`];
    if (stored !== undefined) return stored;
    return selected !== null && section.resources.some((resource) => resourceKey(resource) === resourceKey(selected));
  }

  function toggleTreeSection(view: string, section: TreeSection) {
    const key = `${treeKeyFor(view)}:${section.title}`;
    sidebarTreeSections = { ...sidebarTreeSections, [key]: !isTreeSectionOpen(view, section, sidebarTreeSections, selectedResource) };
  }

  function isSidebarResourceSelected(resource: ResourceDescriptor, view: View, selected: ResourceDescriptor | null) {
    return view === 'Resources' && selected !== null && resourceKey(selected) === resourceKey(resource);
  }

  async function openResourcesHome(custom: boolean) {
    const category: ResourceCategory | 'All resources' = custom ? 'Custom Resources' : 'All resources';
    sidebarTreeOpen = { ...sidebarTreeOpen, [custom ? 'custom' : 'resources']: true };
    if (activeView !== 'Resources') await navigateTo('Resources');
    selectedCategory = category;
    sidebarResourceCategory = category;
    resourceDirectorySearch = '';
    showResourceDirectory();
  }

  async function openTreeResource(resource: ResourceDescriptor) {
    if (activeView !== 'Resources') await navigateTo('Resources');
    await openDirectoryResource(resource);
  }

  async function openDirectoryResource(resource: ResourceDescriptor) {
    selectedCategory = resource.category;
    sidebarResourceCategory = resource.category;
    await openResource(resource);
  }

  function showResourceDirectory() {
    resourceRequestGeneration += 1;
    stopLiveObjectRefresh();
    clearResourceObjectSelection();
    closeEditor();
    closeYamlEditor();
    selectedResource = null;
    resourceObjects = [];
    loadingObjects = false;
  }

  function resourceKey(resource: ResourceDescriptor) {
    return `${resource.group}\u0000${resource.version}\u0000${resource.plural}`;
  }

  function permissionNamespace(resource: ResourceDescriptor, object?: ResourceObject | null) {
    if (!resource.namespaced) return undefined;
    return object?.namespace || (namespace === 'all namespaces' ? undefined : namespace);
  }

  function accessReviewKey(check: Omit<AccessReviewCheck, 'key'>) {
    return [
      activeClusterId,
      activeKubeconfigPath || kubeconfigPath,
      activeCluster,
      check.group,
      check.version,
      check.resource,
      check.subresource || '',
      check.verb,
      check.namespace || '',
      check.name || '',
    ].join('\u0000');
  }

  function resourceAccessCheck(resource: ResourceDescriptor, verb: string, object?: ResourceObject | null): AccessReviewCheck {
    const check = {
      group: resource.group,
      version: resource.version,
      resource: resource.plural,
      verb,
      namespace: permissionNamespace(resource, object),
      name: object?.name,
    };
    return { key: accessReviewKey(check), ...check };
  }

  function podSubresourceAccessCheck(verb: 'get' | 'create', subresource: 'log' | 'exec', object?: ResourceObject | null): AccessReviewCheck {
    const check = {
      group: '',
      version: 'v1',
      resource: 'pods',
      verb,
      namespace: object?.namespace || (namespace === 'all namespaces' ? undefined : namespace),
      name: object?.name,
      subresource,
    };
    return { key: accessReviewKey(check), ...check };
  }

  function accessDecision(check: AccessReviewCheck, decisions: Record<string, AccessReviewDecision>) {
    const direct = decisions[check.key];
    if (direct || !check.name) return direct;
    const generic = { ...check, name: undefined };
    generic.key = accessReviewKey(generic);
    return decisions[generic.key];
  }

  function accessAllowed(check: AccessReviewCheck, decisions: Record<string, AccessReviewDecision>) {
    return accessDecision(check, decisions)?.allowed === true;
  }

  function resourcePermissionSet(resource: ResourceDescriptor | null, object: ResourceObject | null, decisions: Record<string, AccessReviewDecision>): ResourcePermissionSet {
    if (!resource) return { resolved: false, canList: false, canWatch: false, canGet: false, canCreate: false, canUpdate: false, canPatch: false, canDelete: false, canViewLogs: false, canExec: false };
    const checks = {
      list: resourceAccessCheck(resource, 'list'),
      watch: resourceAccessCheck(resource, 'watch'),
      get: resourceAccessCheck(resource, 'get', object),
      create: resourceAccessCheck(resource, 'create'),
      update: resourceAccessCheck(resource, 'update', object),
      patch: resourceAccessCheck(resource, 'patch', object),
      delete: resourceAccessCheck(resource, 'delete', object),
      logs: podSubresourceAccessCheck('get', 'log', resource.kind === 'Pod' ? object : null),
      exec: podSubresourceAccessCheck('create', 'exec', resource.kind === 'Pod' ? object : null),
    };
    return {
      resolved: Boolean(accessDecision(checks.get, decisions) || accessDecision(checks.list, decisions)),
      canList: accessAllowed(checks.list, decisions),
      canWatch: accessAllowed(checks.watch, decisions),
      canGet: accessAllowed(checks.get, decisions),
      canCreate: accessAllowed(checks.create, decisions),
      canUpdate: accessAllowed(checks.update, decisions),
      canPatch: accessAllowed(checks.patch, decisions),
      canDelete: accessAllowed(checks.delete, decisions),
      canViewLogs: accessAllowed(checks.logs, decisions),
      canExec: accessAllowed(checks.exec, decisions),
    };
  }

  function resourceVisibleForCurrentScope(resource: ResourceDescriptor, decisions: Record<string, AccessReviewDecision>, ready: boolean) {
    if (!ready) return true;
    const decision = decisions[resourceAccessCheck(resource, 'list').key];
    if (!decision || decision.evaluationError) return true;
    return decision.allowed;
  }

  async function reviewAccess(checks: AccessReviewCheck[], force = false) {
    const uniqueChecks = [...new Map(checks.map((check) => [check.key, check])).values()];
    const pendingChecks = force ? uniqueChecks : uniqueChecks.filter((check) => !accessDecisions[check.key]);
    if (!pendingChecks.length || !activeClusterId) return accessDecisions;
    if (!('__TAURI_INTERNALS__' in window)) return accessDecisions;
    const requestClusterId = activeClusterId;
    const requestNamespace = namespace;
    permissionRequestsInFlight += 1;
    loadingPermissions = true;
    permissionError = '';
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const response = (await Promise.all(
        Array.from({ length: Math.ceil(pendingChecks.length / 200) }, (_, chunkIndex) => pendingChecks.slice(chunkIndex * 200, (chunkIndex + 1) * 200))
          .map((checks) => invoke<AccessReviewDecision[]>('check_resource_permissions', {
            request: {
              kubeconfigPath: activeKubeconfigPath || kubeconfigPath || null,
              context: activeCluster,
              checks,
            },
          })),
      )).flat();
      const merged = { ...accessDecisions };
      response.forEach((decision) => (merged[decision.key] = decision));
      accessDecisions = merged;
      return merged;
    } catch (error) {
      if (requestClusterId === activeClusterId && requestNamespace === namespace) permissionError = String(error);
      return accessDecisions;
    } finally {
      permissionRequestsInFlight = Math.max(0, permissionRequestsInFlight - 1);
      loadingPermissions = permissionRequestsInFlight > 0;
    }
  }

  async function loadCatalogPermissions(force = false) {
    if (!activeClusterId || !catalog.resources.length) return;
    const clusterId = activeClusterId;
    const requestedNamespace = namespace;
    catalogPermissionsReady = false;
    await reviewAccess(catalog.resources.map((resource) => resourceAccessCheck(resource, 'list')), force);
    if (clusterId !== activeClusterId || requestedNamespace !== namespace) return;
    catalogPermissionsReady = true;
    if (selectedResource && !resourceVisibleForCurrentScope(selectedResource, accessDecisions, true)) {
      selectedResource = null;
      resourceObjects = [];
      clearResourceObjectSelection();
      closeEditor();
      closeYamlEditor();
    }
    if (workloadResource && !resourceVisibleForCurrentScope(workloadResource, accessDecisions, true)) {
      workloadResource = null;
      workloadObjects = [];
      clearWorkloadObjectSelection();
      closeWorkloadLogs();
      closeEditor();
    }
  }

  async function loadResourcePermissions(resource: ResourceDescriptor, object?: ResourceObject | null, force = false) {
    const checks = ['list', 'watch', 'get', 'create', 'update', 'patch', 'delete']
      .map((verb) => resourceAccessCheck(resource, verb, object));
    if (resource.category === 'Workloads') {
      checks.push(podSubresourceAccessCheck('get', 'log', resource.kind === 'Pod' ? object : null));
      checks.push(podSubresourceAccessCheck('create', 'exec', resource.kind === 'Pod' ? object : null));
    }
    return reviewAccess(checks, force);
  }

  async function mayListResource(resource: ResourceDescriptor) {
    const decisions = await loadResourcePermissions(resource);
    const decision = decisions[resourceAccessCheck(resource, 'list').key];
    if (!decision || decision.evaluationError) return true;
    if (decision.allowed) return true;
    catalogPermissionsReady = true;
    notify(`Your current Kubernetes identity cannot list ${resource.kind} in ${permissionNamespace(resource) || 'all namespaces'}.`);
    return false;
  }

  async function mayGetResource(resource: ResourceDescriptor, object: ResourceObject) {
    const decisions = await loadResourcePermissions(resource, object);
    const decision = decisions[resourceAccessCheck(resource, 'get', object).key];
    if (!decision || decision.evaluationError) return true;
    if (decision.allowed) return true;
    notify(`Your current Kubernetes identity cannot open ${resource.kind} ${object.name}.`);
    return false;
  }

  async function mayUsePodSubresource(verb: 'get' | 'create', subresource: 'log' | 'exec', pod: ResourceObject) {
    const check = podSubresourceAccessCheck(verb, subresource, pod);
    const decisions = await reviewAccess([check]);
    const decision = decisions[check.key];
    if (decision?.allowed) return true;
    notify(`Your current Kubernetes identity cannot ${subresource === 'log' ? 'read logs from' : 'open a terminal in'} Pod ${pod.name}.`);
    return false;
  }

  function invalidateCurrentAccessDecisions() {
    const prefix = `${activeClusterId}\u0000`;
    accessDecisions = Object.fromEntries(Object.entries(accessDecisions).filter(([key]) => !key.startsWith(prefix)));
    catalogPermissionsReady = false;
    permissionError = '';
    permissionRequestsInFlight = 0;
    loadingPermissions = false;
  }

  function permissionFailure(error: unknown) {
    const message = String(error);
    return /forbidden|authorization|not allow|permission/i.test(message);
  }

  function refreshPermissionsAfterFailure(error: unknown, resource?: ResourceDescriptor | null, object?: ResourceObject | null) {
    if (!permissionFailure(error)) return;
    invalidateCurrentAccessDecisions();
    void loadCatalogPermissions(true);
    if (resource) void loadResourcePermissions(resource, object, true);
  }

  function seedVisualQaPermissions(resources: ResourceDescriptor[], readOnly = false) {
    const decisions: Record<string, AccessReviewDecision> = {};
    resources.forEach((resource) => {
      ['list', 'watch', 'get', 'create', 'update', 'patch', 'delete'].forEach((verb) => {
        const check = resourceAccessCheck(resource, verb);
        const allowed = readOnly ? ['list', 'watch', 'get'].includes(verb) : true;
        decisions[check.key] = { key: check.key, allowed, denied: !allowed, reason: allowed ? 'Visual QA permission granted' : 'Visual QA read-only profile' };
      });
    });
    const podResource = resources.find((resource) => resource.kind === 'Pod');
    if (podResource) {
      const logCheck = podSubresourceAccessCheck('get', 'log');
      const execCheck = podSubresourceAccessCheck('create', 'exec');
      decisions[logCheck.key] = { key: logCheck.key, allowed: true, denied: false };
      decisions[execCheck.key] = { key: execCheck.key, allowed: !readOnly, denied: readOnly };
    }
    accessDecisions = decisions;
    catalogPermissionsReady = true;
  }

  function resourceSearchText(resource: ResourceDescriptor) {
    return `${resource.kind} ${resource.plural} ${resource.group} ${resource.version} ${resource.apiVersion} ${resource.category}`.toLowerCase();
  }

  function buildGlobalSearchResults(
    query: string,
    resources: ResourceDescriptor[],
    currentResource: ResourceDescriptor | null,
    currentObjects: ResourceObject[],
    currentWorkloadResource: ResourceDescriptor | null,
    currentWorkloadObjects: ResourceObject[],
  ): GlobalSearchResult[] {
    const normalized = query.trim().toLowerCase();
    if (!normalized) return [];
    const results: GlobalSearchResult[] = [];
    for (const resource of resources) {
      if (!resourceSearchText(resource).includes(normalized)) continue;
      results.push({
        type: 'resource',
        resource,
        title: resource.kind,
        detail: `${resource.apiVersion} · ${resource.category}${resource.namespaced ? ' · namespaced' : ' · cluster-wide'}`,
      });
    }
    const objectCandidates: Array<{ resource: ResourceDescriptor; object: ResourceObject }> = [];
    if (currentResource) currentObjects.forEach((object) => objectCandidates.push({ resource: currentResource!, object }));
    if (currentWorkloadResource) currentWorkloadObjects.forEach((object) => objectCandidates.push({ resource: currentWorkloadResource!, object }));
    const seenObjects = new Set<string>();
    for (const { resource, object } of objectCandidates) {
      const key = `${resourceKey(resource)}\u0000${object.namespace || ''}\u0000${object.name}`;
      if (seenObjects.has(key)) continue;
      seenObjects.add(key);
      const objectText = `${object.name} ${object.namespace || ''} ${resource.kind} ${resource.apiVersion}`.toLowerCase();
      if (!objectText.includes(normalized)) continue;
      results.push({
        type: 'object',
        resource,
        object,
        title: object.name,
        detail: `${resource.kind} · ${object.namespace || 'cluster-scoped'} · ${resource.apiVersion}`,
      });
    }
    return results
      .sort((left, right) => {
        const leftExact = left.title.toLowerCase() === normalized ? 0 : 1;
        const rightExact = right.title.toLowerCase() === normalized ? 0 : 1;
        return leftExact - rightExact || left.title.localeCompare(right.title);
      })
      .slice(0, 28);
  }

  function decodeSecret(value: string) {
    try {
      const bytes = Uint8Array.from(atob(value), (character) => character.charCodeAt(0));
      return new TextDecoder().decode(bytes);
    } catch {
      return value;
    }
  }

  function encodeSecret(value: string) {
    const bytes = new TextEncoder().encode(value);
    let binary = '';
    bytes.forEach((byte) => (binary += String.fromCharCode(byte)));
    return btoa(binary);
  }

  function updateEditorEntryKey(index: number, key: string) {
    editorEntries = editorEntries.map((entry, entryIndex) => entryIndex === index ? { ...entry, key } : entry);
  }

  function updateEditorEntryValue(index: number, value: string) {
    editorEntries = editorEntries.map((entry, entryIndex) => entryIndex === index
      ? { ...entry, value: editorResource?.kind === 'Secret' && revealSecret ? encodeSecret(value) : value }
      : entry,
    );
  }

  function editorEntryDisplayValue(entry: EditorEntry, showDecoded = revealSecret) {
    return editorResource?.kind === 'Secret' && showDecoded ? decodeSecret(entry.value) : entry.value;
  }

  function editorEntryFormat(entry: EditorEntry, showDecoded = revealSecret) {
    const key = entry.key.toLowerCase();
    const value = editorEntryDisplayValue(entry, showDecoded).trim();
    if (/\.(crt|pem|cer|key)$/.test(key)) return { short: 'CRT', label: 'Certificate', tone: 'certificate' };
    if (editorResource?.kind === 'Secret' && !showDecoded) return { short: 'B64', label: 'Base64', tone: 'base64' };
    if (/\.json$/.test(key) || ((value.startsWith('{') || value.startsWith('[')) && value.endsWith(value.startsWith('{') ? '}' : ']'))) return { short: 'JSN', label: 'JSON', tone: 'json' };
    if (/\.(yaml|yml)$/.test(key) || (/^[\w.-]+:\s/m.test(value) && value.includes('\n'))) return { short: 'YML', label: 'YAML', tone: 'yaml' };
    const environmentLines = value.split('\n').filter(Boolean);
    if (environmentLines.length && environmentLines.every((line) => /^[A-Za-z_][A-Za-z0-9_]*=/.test(line))) return { short: 'ENV', label: 'Environment', tone: 'environment' };
    return { short: 'TXT', label: 'Plain text', tone: 'text' };
  }

  function editorEntryPreview(entry: EditorEntry, showDecoded = revealSecret) {
    const value = editorEntryDisplayValue(entry, showDecoded).replace(/\s+/g, ' ').trim();
    return value.length > 110 ? `${value.slice(0, 107)}…` : value || 'Empty value';
  }

  function toggleEditorEntry(index: number) {
    expandedEditorEntryIndex = expandedEditorEntryIndex === index ? -1 : index;
  }

  function addEditorEntry() {
    const value = editorResource?.kind === 'Secret' && revealSecret ? encodeSecret('') : '';
    editorEntrySearch = '';
    editorEntries = [...editorEntries, { key: 'new-key', value }];
    expandedEditorEntryIndex = editorEntries.length - 1;
  }

  function removeEditorEntry(index: number) {
    editorEntries = editorEntries.filter((_, entryIndex) => entryIndex !== index);
    if (!editorEntries.length) expandedEditorEntryIndex = -1;
    else if (expandedEditorEntryIndex >= editorEntries.length) expandedEditorEntryIndex = editorEntries.length - 1;
    else if (index < expandedEditorEntryIndex) expandedEditorEntryIndex -= 1;
  }

  function asRecord(value: unknown): Record<string, unknown> {
    return value && typeof value === 'object' && !Array.isArray(value) ? value as Record<string, unknown> : {};
  }

  function asString(value: unknown): string | undefined {
    return typeof value === 'string' && value.trim() ? value : undefined;
  }

  function asArray(value: unknown): unknown[] {
    return Array.isArray(value) ? value : [];
  }

  function resourceLabels(manifest: Record<string, unknown> | null) {
    const labels = asRecord(asRecord(manifest).metadata).labels;
    return Object.entries(asRecord(labels)).slice(0, 16);
  }

  function networkAddressFacts(manifest: Record<string, unknown> | null): NetworkFact[] {
    const facts: NetworkFact[] = [];
    const seen = new Set<string>();
    const resource = asRecord(manifest);
    const spec = asRecord(resource.spec);
    const status = asRecord(resource.status);
    const add = (label: string, value: unknown, tone: NetworkFact['tone'] = 'neutral') => {
      const text = asString(value);
      if (!text || text.toLowerCase() === '<none>') return;
      const key = `${label}\u0000${text}`;
      if (!seen.has(key)) {
        seen.add(key);
        facts.push({ label, value: text, tone });
      }
    };
    asArray(spec.clusterIPs).forEach((value) => add('Cluster IP', value, 'primary'));
    if (!asArray(spec.clusterIPs).length) add('Cluster IP', spec.clusterIP, 'primary');
    asArray(spec.externalIPs).forEach((value) => add('External IP', value, 'external'));
    add('External hostname', spec.externalName, 'external');
    add('Load balancer IP', spec.loadBalancerIP, 'external');
    asArray(spec.rules).forEach((rule) => add('Host', asRecord(rule).host, 'primary'));
    asArray(spec.hostnames).forEach((hostname) => add('Host', hostname, 'primary'));
    asArray(spec.tls).forEach((entry) => asArray(asRecord(entry).hosts).forEach((value) => add('TLS host', value, 'primary')));
    asArray(spec.listeners).forEach((listener) => add('Listener host', asRecord(listener).hostname, 'primary'));
    asArray(spec.addresses).forEach((entry) => {
      if (typeof entry === 'string') add('Address', entry, 'primary');
      else {
        const address = asRecord(entry);
        add(asString(address.type) || 'Address', address.address ?? address.value, 'primary');
      }
    });
    asArray(spec.endpoints).forEach((entry) => {
      if (typeof entry === 'string') add('Endpoint', entry, 'primary');
      else {
        const endpoint = asRecord(entry);
        asArray(endpoint.addresses).forEach((address) => add('Endpoint', address, 'primary'));
        add('Endpoint', endpoint.address, 'primary');
      }
    });
    // Legacy Endpoints objects store backend addresses in root-level subsets,
    // rather than in spec or status. EndpointSlice uses root-level endpoints.
    // Handle both shapes so the inspector never hides live backend addresses.
    asArray(resource.subsets).forEach((entry) => {
      const subset = asRecord(entry);
      asArray(subset.addresses).forEach((address) => {
        const endpoint = asRecord(address);
        add('Endpoint IP', endpoint.ip, 'primary');
        add('Endpoint host', endpoint.hostname, 'primary');
      });
      asArray(subset.notReadyAddresses).forEach((address) => {
        const endpoint = asRecord(address);
        add('Not ready endpoint', endpoint.ip, 'neutral');
        add('Not ready host', endpoint.hostname, 'neutral');
      });
    });
    asArray(resource.endpoints).forEach((entry) => {
      const endpoint = asRecord(entry);
      asArray(endpoint.addresses).forEach((address) => add('Endpoint IP', address, 'primary'));
      add('Endpoint host', endpoint.hostname, 'primary');
    });
    asArray(asRecord(status.loadBalancer).ingress).forEach((entry) => {
      const ingress = asRecord(entry);
      add('Load balancer', ingress.hostname, 'external');
      add('Load balancer', ingress.ip, 'external');
    });
    asArray(status.addresses).forEach((entry) => {
      if (typeof entry === 'string') add('Address', entry, 'primary');
      else {
        const address = asRecord(entry);
        add(asString(address.type) || 'Address', address.address ?? address.value, 'primary');
      }
    });
    return facts;
  }

  function networkHosts(manifest: Record<string, unknown> | null) {
    return networkAddressFacts(manifest).map((fact) => `${fact.label} · ${fact.value}`);
  }

  function networkPortFacts(manifest: Record<string, unknown> | null): NetworkFact[] {
    const resource = asRecord(manifest);
    const spec = asRecord(resource.spec);
    const facts: NetworkFact[] = [];
    const seen = new Set<string>();
    const add = (label: string, value: string, tone: NetworkFact['tone'] = 'neutral') => {
      const key = `${label}\u0000${value}`;
      if (!seen.has(key)) {
        seen.add(key);
        facts.push({ label, value, tone });
      }
    };
    const addPort = (entry: unknown, fallbackLabel = 'Port') => {
      const port = asRecord(entry);
      const number = port.port ?? port.targetPort ?? port.containerPort;
      const target = port.targetPort && port.port !== port.targetPort ? ` → ${port.targetPort}` : '';
      const nodePort = port.nodePort ? ` · node ${port.nodePort}` : '';
      add(asString(port.name) || fallbackLabel, `${number ?? '—'}${target}${port.protocol ? `/${port.protocol}` : ''}${nodePort}`, port.nodePort ? 'external' : 'neutral');
    };
    asArray(spec.ports).forEach((entry) => addPort(entry));
    // Endpoints uses subsets[].ports; EndpointSlice publishes ports at the
    // object root. Both are backend listener facts, not service spec fields.
    asArray(resource.subsets).forEach((entry) => asArray(asRecord(entry).ports).forEach((port) => addPort(port, 'Endpoint port')));
    asArray(resource.ports).forEach((port) => addPort(port, 'Endpoint port'));
    asArray(spec.listeners).forEach((entry) => {
      const listener = asRecord(entry);
      const number = listener.port ?? listener.targetPort ?? '—';
      const host = asString(listener.hostname) ? ` · ${listener.hostname}` : '';
      add(asString(listener.name) || 'Listener', `${number}${listener.protocol ? `/${listener.protocol}` : ''}${host}`, 'primary');
    });
    asArray(spec.rules).forEach((rule) => {
      asArray(asRecord(rule).backendRefs).forEach((entry) => {
        const backend = asRecord(entry);
        const name = asString(backend.name) || 'Backend';
        const port = backend.port;
        if (port !== undefined && port !== null) add(`Backend · ${name}`, `${port}`, 'primary');
      });
    });
    return facts;
  }

  function networkPorts(manifest: Record<string, unknown> | null) {
    return networkPortFacts(manifest).map((fact) => `${fact.label} · ${fact.value}`);
  }

  function networkServiceType(manifest: Record<string, unknown> | null) {
    return asString(asRecord(asRecord(manifest).spec).type) || 'ClusterIP';
  }

  function networkServiceClusterIp(manifest: Record<string, unknown> | null) {
    const spec = asRecord(asRecord(manifest).spec);
    const clusterIps = asArray(spec.clusterIPs)
      .map(asString)
      .filter((value): value is string => Boolean(value));
    return clusterIps[0] || asString(spec.clusterIP) || 'Assigned by Kubernetes';
  }

  function networkServiceExternalEndpoints(manifest: Record<string, unknown> | null) {
    const endpoints = new Set<string>();
    const resource = asRecord(manifest);
    const spec = asRecord(resource.spec);
    const status = asRecord(resource.status);
    const add = (value: unknown) => {
      const endpoint = asString(value);
      if (endpoint && endpoint.toLowerCase() !== '<none>') endpoints.add(endpoint);
    };
    asArray(spec.externalIPs).forEach(add);
    add(spec.externalName);
    add(spec.loadBalancerIP);
    asArray(asRecord(status.loadBalancer).ingress).forEach((entry) => {
      add(asRecord(entry).ip);
      add(asRecord(entry).hostname);
    });
    return [...endpoints];
  }

  function networkServiceTrafficPolicy(manifest: Record<string, unknown> | null) {
    const spec = asRecord(asRecord(manifest).spec);
    return asString(spec.externalTrafficPolicy) || asString(spec.internalTrafficPolicy) || 'Cluster routing';
  }

  function networkServiceExposure(manifest: Record<string, unknown> | null) {
    const type = networkServiceType(manifest);
    const external = networkServiceExternalEndpoints(manifest);
    if (external.length) return 'Reachable outside the cluster';
    if (type === 'LoadBalancer') return 'Waiting for an external address';
    if (type === 'NodePort') return 'Exposed through node ports';
    if (type === 'ExternalName') return 'Routes to an external hostname';
    return 'Internal cluster service';
  }

  function genericResourcePreview(manifest: Record<string, unknown> | null) {
    const resource = asRecord(manifest);
    const preview = resource.spec || resource.status || {};
    return JSON.stringify(preview, null, 2);
  }

  function workloadPodSpec(manifest: Record<string, unknown> | null) {
    const resource = asRecord(manifest);
    const spec = asRecord(resource.spec);
    return resource.kind === 'Pod' ? spec : asRecord(asRecord(spec.template).spec);
  }

  function workloadImages(manifest: Record<string, unknown> | null) {
    const podSpec = workloadPodSpec(manifest);
    return [...asArray(podSpec.initContainers), ...asArray(podSpec.containers)]
      .map((container) => {
        const entry = asRecord(container);
        return { name: asString(entry.name) || 'container', image: asString(entry.image) || 'image unavailable', init: asArray(podSpec.initContainers).includes(container) };
      });
  }

  function workloadAttachments(manifest: Record<string, unknown> | null) {
    const configMaps = new Set<string>();
    const secrets = new Set<string>();
    const podSpec = workloadPodSpec(manifest);
    const addName = (target: Set<string>, value: unknown) => { const name = asString(value); if (name) target.add(name); };
    const scanContainer = (container: unknown) => {
      const entry = asRecord(container);
      asArray(entry.envFrom).forEach((source) => {
        const ref = asRecord(source);
        addName(configMaps, asRecord(ref.configMapRef).name);
        addName(secrets, asRecord(ref.secretRef).name);
      });
      asArray(entry.env).forEach((variable) => {
        const valueFrom = asRecord(asRecord(variable).valueFrom);
        addName(configMaps, asRecord(valueFrom.configMapKeyRef).name);
        addName(secrets, asRecord(valueFrom.secretKeyRef).name);
      });
    };
    [...asArray(podSpec.initContainers), ...asArray(podSpec.containers)].forEach(scanContainer);
    asArray(podSpec.imagePullSecrets).forEach((entry) => addName(secrets, asRecord(entry).name));
    asArray(podSpec.volumes).forEach((volume) => {
      const entry = asRecord(volume);
      addName(configMaps, asRecord(entry.configMap).name);
      addName(secrets, asRecord(entry.secret).secretName);
      asArray(asRecord(entry.projected).sources).forEach((source) => {
        addName(configMaps, asRecord(asRecord(source).configMap).name);
        addName(secrets, asRecord(asRecord(source).secret).name);
      });
    });
    return { configMaps: [...configMaps], secrets: [...secrets] };
  }

  function workloadVolumes(manifest: Record<string, unknown> | null) {
    const podSpec = workloadPodSpec(manifest);
    const mountsByVolume = new Map<string, string[]>();
    const addMount = (volumeName: unknown, location: unknown, readOnly = false) => {
      const name = asString(volumeName);
      const path = asString(location);
      if (!name || !path) return;
      const mounts = mountsByVolume.get(name) || [];
      mounts.push(`${path}${readOnly ? ' · read-only' : ''}`);
      mountsByVolume.set(name, mounts);
    };
    [...asArray(podSpec.initContainers), ...asArray(podSpec.containers)].forEach((container) => {
      const entry = asRecord(container);
      asArray(entry.volumeMounts).forEach((mount) => {
        const volumeMount = asRecord(mount);
        addMount(volumeMount.name, volumeMount.mountPath, volumeMount.readOnly === true);
      });
      asArray(entry.volumeDevices).forEach((device) => {
        const volumeDevice = asRecord(device);
        addMount(volumeDevice.name, volumeDevice.devicePath);
      });
    });
    return asArray(podSpec.volumes).map((volume, index) => {
      const entry = asRecord(volume);
      const name = asString(entry.name) || `volume-${index + 1}`;
      const configMap = asRecord(entry.configMap);
      const secret = asRecord(entry.secret);
      const claim = asRecord(entry.persistentVolumeClaim);
      const hostPath = asRecord(entry.hostPath);
      const csi = asRecord(entry.csi);
      const nfs = asRecord(entry.nfs);
      const projectedSources = asArray(asRecord(entry.projected).sources);
      const configMapName = asString(configMap.name);
      const secretName = asString(secret.secretName);
      const claimName = asString(claim.claimName);
      const hostPathName = asString(hostPath.path);
      const csiDriver = asString(csi.driver);
      const nfsServer = asString(nfs.server);
      let type = 'Volume';
      let source = 'Kubernetes-managed source';
      if (configMapName) {
        type = 'ConfigMap';
        source = configMapName;
      } else if (secretName) {
        type = 'Secret';
        source = secretName;
      } else if (claimName) {
        type = 'PersistentVolumeClaim';
        source = claimName;
      } else if (entry.emptyDir !== undefined) {
        type = 'EmptyDir';
        source = asString(asRecord(entry.emptyDir).medium) || 'Node-backed ephemeral storage';
      } else if (hostPathName) {
        type = 'HostPath';
        source = hostPathName;
      } else if (csiDriver) {
        type = 'CSI';
        source = csiDriver;
      } else if (nfsServer) {
        type = 'NFS';
        source = `${nfsServer}:${asString(nfs.path) || '/'}`;
      } else if (projectedSources.length) {
        type = 'Projected';
        source = `${projectedSources.length} projected source${projectedSources.length === 1 ? '' : 's'}`;
      } else if (entry.downwardAPI !== undefined) {
        type = 'Downward API';
        source = 'Pod metadata';
      } else if (entry.ephemeral !== undefined) {
        type = 'Ephemeral claim';
        source = 'Generated from a volume claim template';
      }
      return { name, type, source, mounts: mountsByVolume.get(name) || [] };
    });
  }

  function podConditionDiagnostics(manifest: Record<string, unknown> | null) {
    const status = asRecord(asRecord(manifest).status);
    return asArray(status.conditions).map((condition) => {
      const entry = asRecord(condition);
      return {
        type: asString(entry.type) || 'Condition',
        status: asString(entry.status) || 'Unknown',
        reason: asString(entry.reason) || '',
        message: asString(entry.message) || '',
      };
    });
  }

  function podContainerDiagnostics(manifest: Record<string, unknown> | null) {
    const status = asRecord(asRecord(manifest).status);
    return [...asArray(status.initContainerStatuses), ...asArray(status.containerStatuses), ...asArray(status.ephemeralContainerStatuses)].map((container) => {
      const entry = asRecord(container);
      const state = asRecord(entry.state);
      const waiting = asRecord(state.waiting);
      const terminated = asRecord(state.terminated);
      const running = asRecord(state.running);
      const stateName = Object.keys(waiting).length ? 'Waiting'
        : Object.keys(terminated).length ? 'Terminated'
          : Object.keys(running).length ? 'Running'
            : 'Unknown';
      return {
        name: asString(entry.name) || 'container',
        image: asString(entry.image) || '',
        ready: entry.ready === true,
        restarts: typeof entry.restartCount === 'number' ? entry.restartCount : 0,
        state: stateName,
        reason: asString(waiting.reason) || asString(terminated.reason) || '',
        message: asString(waiting.message) || asString(terminated.message) || '',
      };
    });
  }

  function workloadReplicaSummary(manifest: Record<string, unknown> | null) {
    const spec = asRecord(asRecord(manifest).spec);
    const status = asRecord(asRecord(manifest).status);
    const desired = spec.replicas;
    const ready = status.readyReplicas ?? status.availableReplicas;
    return desired === undefined && ready === undefined ? 'Managed by Kubernetes' : `${ready ?? 0} ready · ${desired ?? '—'} desired`;
  }

  function resetWorkloadTerminal() {
    workloadDetailMode = 'overview';
    terminalPods = [];
    terminalTarget = null;
    terminalContainers = [];
    terminalPorts = [];
    selectedTerminalContainer = '';
    terminalOutput = '';
    loadingTerminalPods = false;
    loadingTerminalRuntime = false;
    runningTerminalCommand = false;
    schedulePendingResumeRecovery();
  }

  async function listWorkloadPods(resource: ResourceDescriptor, object: ResourceObject) {
    const resourceNamespace = objectNamespace(object);
    if (!resourceNamespace) throw new Error('A namespace is required to find workload Pods');
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<ResourceObject[]>('list_workload_pods', {
      request: {
        kubeconfigPath: activeKubeconfigPath || kubeconfigPath || null,
        context: activeCluster,
        group: resource.group,
        version: resource.version,
        kind: resource.kind,
        plural: resource.plural,
        namespace: resourceNamespace,
        name: object.name,
      },
    });
  }

  async function openWorkloadLogs(resource: ResourceDescriptor, object: ResourceObject) {
    if (openingLogsTarget) return;
    const openingKey = logOpeningKey(resource.kind, object);
    const openingGeneration = logWorkspaceGeneration;
    openingLogsTarget = { key: openingKey, label: `${resource.kind} · ${object.name}` };
    await tick();
    try {
      const pods = resource.kind === 'Pod' ? [object] : await listWorkloadPods(resource, object);
      if (openingGeneration !== logWorkspaceGeneration || openingLogsTarget?.key !== openingKey) return;
      if (!pods.length) {
        notify(`No live Pods match ${resource.kind} ${object.name}`);
        return;
      }
      if (!await mayUsePodSubresource('get', 'log', pods[0])) return;
      const podNamespace = objectNamespace(pods[0]);
      const podsInNamespace = podNamespace ? await namespacePodIndex(podNamespace, pods) : pods;
      if (openingGeneration !== logWorkspaceGeneration || openingLogsTarget?.key !== openingKey) return;
      await openPodLogsWorkspace(pods[0], podsInNamespace, `${resource.kind} · ${object.name}`);
    } catch (error) {
      if (openingLogsTarget?.key === openingKey) notify(`Could not open logs for ${object.name}: ${String(error)}`);
    } finally {
      if (openingLogsTarget?.key === openingKey) openingLogsTarget = null;
    }
  }

  async function selectTerminalPod(object: ResourceObject) {
    const podNamespace = objectNamespace(object);
    if (!podNamespace) {
      notify('A namespace is required to open a Pod terminal');
      return;
    }
    if (!await mayUsePodSubresource('create', 'exec', object)) return;
    terminalTarget = { pod: object.name, namespace: podNamespace };
    terminalContainers = [];
    terminalPorts = [];
    selectedTerminalContainer = '';
    terminalOutput = '';
    loadingTerminalRuntime = true;
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const runtime = await invoke<PodRuntime>('get_pod_runtime', {
        request: { kubeconfigPath: activeKubeconfigPath || kubeconfigPath || null, context: activeCluster, namespace: podNamespace, pod: object.name },
      });
      terminalContainers = runtime.containers;
      terminalPorts = runtime.ports || [];
      selectedTerminalContainer = runtime.containers[0] || '';
    } catch (error) {
      notify(`Could not inspect ${object.name}: ${String(error)}`);
      terminalTarget = null;
    } finally {
      loadingTerminalRuntime = false;
    }
  }

  async function openWorkloadTerminal(resource: ResourceDescriptor, object: ResourceObject) {
    resetWorkloadTerminal();
    workloadDetailMode = 'terminal';
    loadingTerminalPods = true;
    try {
      terminalPods = resource.kind === 'Pod' ? [object] : await listWorkloadPods(resource, object);
      if (!terminalPods.length) {
        notify(`No live Pods match ${resource.kind} ${object.name}`);
        return;
      }
      if (!await mayUsePodSubresource('create', 'exec', terminalPods[0])) return;
      await selectTerminalPod(terminalPods[0]);
    } catch (error) {
      notify(`Could not prepare a terminal for ${object.name}: ${String(error)}`);
    } finally {
      loadingTerminalPods = false;
    }
  }

  async function runTerminalCommand() {
    if (!terminalTarget || !terminalCommand.trim()) return;
    runningTerminalCommand = true;
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const response = await invoke<PodExecResponse>('exec_pod_command', {
        request: {
          kubeconfigPath: activeKubeconfigPath || kubeconfigPath || null,
          context: activeCluster,
          namespace: terminalTarget.namespace,
          pod: terminalTarget.pod,
          container: selectedTerminalContainer || null,
          command: terminalCommand,
        },
      });
      const output = `${response.stdout || ''}${response.stderr ? `${response.stdout ? '\n' : ''}${response.stderr}` : ''}`.trimEnd();
      terminalOutput = `$ ${terminalCommand}\n${output || '(command completed without output)'}`;
    } catch (error) {
      refreshPermissionsAfterFailure(error, workloadResource);
      terminalOutput = `$ ${terminalCommand}\n${String(error)}`;
    } finally {
      runningTerminalCommand = false;
    }
  }

  async function openResourceEditor(resource: ResourceDescriptor, object: ResourceObject) {
    const resourceNamespace = objectNamespace(object);
    if (resource.namespaced && !resourceNamespace) {
      notify('A namespace is required to open this resource');
      return;
    }
    if (!await mayGetResource(resource, object)) return;
    resetWorkloadTerminal();
    editorResource = resource;
    editorObject = object;
    editorManifest = null;
    editorEntries = [];
    expandedEditorEntryIndex = 0;
    editorEntrySearch = '';
    editorCertificate = undefined;
    revealSecret = false;
    configDiscardPrompt = false;
    loadingEditor = true;
    if (resource.kind === 'Pod') void loadClusterEvents();
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const detail = await invoke<ResourceDetail>('get_resource_detail', {
        request: {
          kubeconfigPath: activeKubeconfigPath || kubeconfigPath || null,
          context: activeCluster,
          group: resource.group,
          version: resource.version,
          kind: resource.kind,
          plural: resource.plural,
          namespaced: resource.namespaced,
          namespace: resourceNamespace || null,
          name: object.name,
        },
      });
      editorManifest = detail.manifest;
      editorCertificate = detail.certificate;
      const data = detail.manifest.data;
      if (data && typeof data === 'object' && !Array.isArray(data)) {
        editorEntries = Object.entries(data).map(([key, value]) => ({ key, value: String(value) }));
        expandedEditorEntryIndex = editorEntries.length > 8 ? -1 : 0;
      }
    } catch (error) {
      notify(`Could not open ${resource.kind}: ${String(error)}`);
      editorResource = null;
      editorObject = null;
    } finally {
      loadingEditor = false;
      flushPendingResourceWatchRefresh();
    }
  }

  function editorEntriesSignature(entries: EditorEntry[]) {
    return JSON.stringify(entries.map((entry) => [entry.key.trim(), entry.value]));
  }

  function editorDataSignature(manifest: Record<string, unknown>) {
    const data = manifest.data;
    return JSON.stringify(data && typeof data === 'object' && !Array.isArray(data) ? Object.entries(data).map(([key, value]) => [key, String(value)]) : []);
  }

  function requestCloseConfigEditor() {
    if (savingEditor) return;
    if (configEditorDirty) {
      configDiscardPrompt = true;
      return;
    }
    discardConfigEditor();
  }

  function discardConfigEditor() {
    configDiscardPrompt = false;
    closeEditor();
  }

  async function saveEditor() {
    if (!editorResource || !editorObject || !editorManifest) return;
    const normalizedKeys = editorEntries.map((entry) => entry.key.trim());
    if (normalizedKeys.some((key) => !key)) {
      notify('Every entry needs a key before saving');
      return;
    }
    if (new Set(normalizedKeys).size !== normalizedKeys.length) {
      notify('Keys must be unique before saving');
      return;
    }
    savingEditor = true;
    try {
      const resourceNamespace = objectNamespace(editorObject);
      const manifest = { ...editorManifest, data: Object.fromEntries(editorEntries.map((entry, index) => [normalizedKeys[index], entry.value])) };
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('save_resource_detail', {
        request: {
          kubeconfigPath: activeKubeconfigPath || kubeconfigPath || null,
          context: activeCluster,
          group: editorResource.group,
          version: editorResource.version,
          kind: editorResource.kind,
          plural: editorResource.plural,
          namespaced: editorResource.namespaced,
          namespace: resourceNamespace || null,
          name: editorObject.name,
          manifest,
        },
      });
      // Values are always stored base64-encoded; decoded Secret text never outlives a save.
      const hidDecodedValues = editorResource.kind === 'Secret' && revealSecret;
      revealSecret = false;
      configDiscardPrompt = false;
      notify(hidDecodedValues
        ? `Secret saved to ${activeCluster} as base64 · decoded values hidden again`
        : `${editorResource.kind} saved to ${activeCluster}`);
      editorManifest = manifest;
      editorEntries = editorEntries.map((entry, index) => ({ ...entry, key: normalizedKeys[index] }));
    } catch (error) {
      refreshPermissionsAfterFailure(error, editorResource, editorObject);
      notify(`Could not save ${editorResource.kind}: ${String(error)}`);
    } finally {
      savingEditor = false;
      flushPendingResourceWatchRefresh();
    }
  }

  async function invokeDeleteResourceObject(target: ResourceDeletionTarget, object: ResourceObject) {
    const resourceNamespace = object.namespace || (target.namespaceScope === 'all namespaces' ? '' : target.namespaceScope);
    if (target.resource.namespaced && !resourceNamespace) {
      throw new Error('A namespace is required to delete this resource');
    }
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('delete_resource_object', {
      request: {
        kubeconfigPath: target.kubeconfigPath || null,
        context: target.cluster,
        group: target.resource.group,
        version: target.resource.version,
        kind: target.resource.kind,
        plural: target.resource.plural,
        namespaced: target.resource.namespaced,
        namespace: resourceNamespace || null,
        name: object.name,
        uid: object.uid || null,
        // UID prevents a same-name replacement from being deleted after review.
        // Resource versions change frequently for live workloads, so they are
        // intentionally not used as a destructive precondition.
        resourceVersion: null,
      },
    });
  }

  type ResourceDeletionTarget = Extract<DeletionTarget, { type: 'resource' }> | Extract<DeletionTarget, { type: 'bulk-resource' }>;

  function deletionTargetContextMatches(target: ResourceDeletionTarget) {
    const currentResource = target.view === 'Resources' ? selectedResource : workloadResource;
    const currentKubeconfigPath = activeKubeconfigPath || kubeconfigPath || undefined;
    return activeClusterId === target.clusterId
      && activeCluster === target.cluster
      && currentKubeconfigPath === target.kubeconfigPath
      && activeView === target.view
      && namespace === target.namespaceScope
      && Boolean(currentResource && resourceKey(currentResource) === resourceKey(target.resource));
  }

  async function deleteResourceTarget(target: Extract<DeletionTarget, { type: 'resource' }>) {
    if (!deletionTargetContextMatches(target)) {
      notify('Deletion canceled because the active cluster, namespace, or resource changed.');
      return;
    }
    deletingResource = true;
    try {
      await invokeDeleteResourceObject(target, target.object);
      if (!deletionTargetContextMatches(target)) {
        notify('Deletion completed, but the workspace changed before its list could be updated. Refresh the current view to verify.');
        return;
      }
      clearClusterObjectCache(target.clusterId);
      if (target.view === 'Resources' && selectedResource && resourceKey(selectedResource) === resourceKey(target.resource)) {
        resourceObjects = resourceObjects.filter((candidate) => resourceObjectSelectionKey(candidate) !== resourceObjectSelectionKey(target.object));
        selectedResourceObjectKeys = selectedResourceObjectKeys.filter((key) => key !== resourceObjectSelectionKey(target.object));
      }
      if (target.view === 'Workloads' && workloadResource && resourceKey(workloadResource) === resourceKey(target.resource)) {
        workloadObjects = workloadObjects.filter((candidate) => resourceObjectSelectionKey(candidate) !== resourceObjectSelectionKey(target.object));
        selectedWorkloadObjectKeys = selectedWorkloadObjectKeys.filter((key) => key !== resourceObjectSelectionKey(target.object));
      }
      markResourceWatchRefreshPending();
      closeEditor();
      closeYamlEditor();
      notify(`${target.resource.kind} ${target.object.name} was deleted from ${target.cluster}`);
    } catch (error) {
      refreshPermissionsAfterFailure(error, target.resource, target.object);
      notify(`Could not delete ${target.resource.kind} ${target.object.name}: ${String(error)}`);
    } finally {
      deletingResource = false;
      deletionTarget = null;
      deletionStep = 1;
      restoreDeletionFocus();
      schedulePendingResumeRecovery();
    }
  }

  async function deleteBulkResourceObjects(target: Extract<DeletionTarget, { type: 'bulk-resource' }>) {
    deletingResource = true;
    bulkDeleteProgress = { completed: 0, total: target.objects.length, failed: 0 };
    const failed: Array<{ object: ResourceObject; error: string }> = [];
    const successfulKeys = new Set<string>();
    let successfulCount = 0;
    let completed = 0;
    try {
      for (const object of target.objects) {
        if (!deletionTargetContextMatches(target)) {
          failed.push({ object, error: 'Workspace context changed before this object was deleted' });
        } else {
          try {
            await invokeDeleteResourceObject(target, object);
            successfulKeys.add(resourceObjectSelectionKey(object));
            successfulCount += 1;
          } catch (error) {
            failed.push({ object, error: String(error) });
          }
        }
        completed += 1;
        bulkDeleteProgress = { completed, total: target.objects.length, failed: failed.length };
      }
      const targetContextStillCurrent = deletionTargetContextMatches(target);
      if (targetContextStillCurrent) markResourceWatchRefreshPending();
      if (successfulCount && targetContextStillCurrent) {
        clearClusterObjectCache(target.clusterId);
        if (target.view === 'Resources' && selectedResource && resourceKey(selectedResource) === resourceKey(target.resource)) {
          resourceObjects = resourceObjects.filter((object) => !successfulKeys.has(resourceObjectSelectionKey(object)));
          selectedResourceObjectKeys = selectedResourceObjectKeys.filter((key) => !successfulKeys.has(key));
          reconcileResourceObjectSelection(resourceObjects);
        }
        if (target.view === 'Workloads' && workloadResource && resourceKey(workloadResource) === resourceKey(target.resource)) {
          workloadObjects = workloadObjects.filter((object) => !successfulKeys.has(resourceObjectSelectionKey(object)));
          selectedWorkloadObjectKeys = selectedWorkloadObjectKeys.filter((key) => !successfulKeys.has(key));
          reconcileWorkloadObjectSelection(workloadObjects);
        }
        if (
          editorResource
          && editorObject
          && resourceKey(editorResource) === resourceKey(target.resource)
          && successfulKeys.has(resourceObjectSelectionKey(editorObject))
        ) {
          closeEditor();
          closeYamlEditor();
        }
        if (
          yamlResource
          && yamlObject
          && resourceKey(yamlResource) === resourceKey(target.resource)
          && successfulKeys.has(resourceObjectSelectionKey(yamlObject))
        ) closeYamlEditor();
      }
      if (successfulCount && !targetContextStillCurrent) {
        const remainingNames = failed.slice(0, 4).map(({ object }) => bulkDeletionObjectLabel(target, object)).join(', ');
        const remainingSuffix = failed.length > 4 ? `, +${failed.length - 4} more` : '';
        notify(`${successfulCount} deletion${successfulCount === 1 ? '' : 's'} completed; ${failed.length} failed or skipped (${remainingNames}${remainingSuffix}). Workspace changed, so refresh to verify.`);
      } else if (failed.length) {
        if (failed.some(({ error }) => permissionFailure(error))) refreshPermissionsAfterFailure(failed[0]?.error, target.resource);
        const failedNames = failed.slice(0, 4).map(({ object }) => bulkDeletionObjectLabel(target, object)).join(', ');
        const suffix = failed.length > 4 ? `, +${failed.length - 4} more` : '';
        const reason = failed[0]?.error ? ` Reason: ${failed[0].error.slice(0, 120)}` : '';
        notify(`${successfulCount} deleted; ${failed.length} failed (${failedNames}${suffix}). Failed items remain selected.${reason}`);
      } else {
        notify(`Deleted ${successfulCount} ${target.resource.kind}${successfulCount === 1 ? '' : 's'} from ${target.cluster}`);
      }
    } finally {
      deletingResource = false;
      bulkDeleteProgress = null;
      deletionTarget = null;
      deletionStep = 1;
      restoreDeletionFocus();
      schedulePendingResumeRecovery();
    }
  }

  function closeEditor(cancelOpeningLogs = true) {
    if (savingEditor) return;
    if (cancelOpeningLogs && openingLogsTarget) {
      openingLogsTarget = null;
      logWorkspaceGeneration += 1;
    }
    if (workloadDetailMode === 'logs') {
      closeLogs(false);
      workloadDetailMode = 'overview';
    }
    resetWorkloadTerminal();
    revealSecret = false;
    editorResource = null;
    editorObject = null;
    editorManifest = null;
    editorEntries = [];
    expandedEditorEntryIndex = 0;
    editorEntrySearch = '';
    schedulePendingResumeRecovery();
  }

  function yamlSearchParts(value: string) {
    if (!normalizedYamlSearch) return [{ text: value, match: false }];
    const parts: Array<{ text: string; match: boolean }> = [];
    const normalizedValue = value.toLowerCase();
    let cursor = 0;
    while (cursor < value.length) {
      const matchIndex = normalizedValue.indexOf(normalizedYamlSearch, cursor);
      if (matchIndex < 0) {
        parts.push({ text: value.slice(cursor), match: false });
        break;
      }
      if (matchIndex > cursor) parts.push({ text: value.slice(cursor, matchIndex), match: false });
      const matchEnd = matchIndex + normalizedYamlSearch.length;
      parts.push({ text: value.slice(matchIndex, matchEnd), match: true });
      cursor = matchEnd;
    }
    return parts.length ? parts : [{ text: value, match: false }];
  }

  async function openYamlEditor(resource: ResourceDescriptor, object: ResourceObject) {
    const resourceNamespace = objectNamespace(object);
    if (resource.namespaced && !resourceNamespace) {
      notify('A namespace is required to view this resource');
      return;
    }
    if (!await mayGetResource(resource, object)) return;
    yamlResource = resource;
    yamlObject = object;
    yamlText = '';
    yamlOriginal = '';
    yamlMode = 'view';
    yamlSearch = '';
    loadingYaml = true;
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const detail = await invoke<ResourceDetail>('get_resource_detail', {
        request: {
          kubeconfigPath: activeKubeconfigPath || kubeconfigPath || null,
          context: activeCluster,
          group: resource.group,
          version: resource.version,
          kind: resource.kind,
          plural: resource.plural,
          namespaced: resource.namespaced,
          namespace: resourceNamespace || null,
          name: object.name,
        },
      });
      yamlText = detail.yaml;
      yamlOriginal = detail.yaml;
    } catch (error) {
      notify(`Could not load YAML for ${object.name}: ${String(error)}`);
      yamlResource = null;
      yamlObject = null;
    } finally {
      loadingYaml = false;
      flushPendingResourceWatchRefresh();
    }
  }

  async function saveYamlEditor() {
    if (!yamlResource || !yamlObject || !yamlText.trim()) return;
    savingYaml = true;
    try {
      const resourceNamespace = objectNamespace(yamlObject);
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('save_resource_yaml', {
        request: {
          kubeconfigPath: activeKubeconfigPath || kubeconfigPath || null,
          context: activeCluster,
          group: yamlResource.group,
          version: yamlResource.version,
          kind: yamlResource.kind,
          plural: yamlResource.plural,
          namespaced: yamlResource.namespaced,
          namespace: resourceNamespace || null,
          name: yamlObject.name,
          yaml: yamlText,
        },
      });
      yamlOriginal = yamlText;
      yamlMode = 'view';
      notify(`${yamlResource.kind} saved to ${activeCluster}`);
    } catch (error) {
      refreshPermissionsAfterFailure(error, yamlResource, yamlObject);
      notify(`Could not save YAML: ${String(error)}`);
    } finally {
      savingYaml = false;
      flushPendingResourceWatchRefresh();
    }
  }

  function closeYamlEditor() {
    if (savingYaml) return;
    yamlResource = null;
    yamlObject = null;
    yamlText = '';
    yamlOriginal = '';
    yamlMode = 'view';
    yamlSearch = '';
    schedulePendingResumeRecovery();
  }

  function closeLogs(clearOpening = true) {
    if (logRefreshTimer) window.clearInterval(logRefreshTimer);
    if (logCopyResetTimer) window.clearTimeout(logCopyResetTimer);
    logRefreshTimer = undefined;
    logCopyResetTimer = undefined;
    logsCopied = false;
    logWorkspaceGeneration += 1;
    logRequestGeneration += 1;
    if (clearOpening) openingLogsTarget = null;
    loadingLogs = false;
    logTarget = null;
    logPods = [];
    logPorts = [];
    logScopeLabel = '';
    logLines = [];
    logSearch = '';
    logSinceTime = '';
    logContainers = [];
    selectedLogContainer = undefined;
    schedulePendingResumeRecovery();
  }

  function closeWorkloadLogs() {
    closeLogs();
    workloadDetailMode = 'overview';
    schedulePendingResumeRecovery();
  }

  function returnToWorkloadList() {
    closeWorkloadLogs();
    closeEditor(false);
  }

  function logLineTimestamp(line: string) {
    const match = line.match(/^(\d{4}-\d{2}-\d{2}T\S+)/);
    if (!match) return '';
    const timestamp = Date.parse(match[1]);
    return Number.isFinite(timestamp) ? new Date(timestamp).toISOString() : '';
  }

  function appendLogSnapshot(existing: string[], incoming: string[]) {
    if (!existing.length) return incoming.slice(-5_000);
    if (!incoming.length) return existing.slice(-5_000);
    const prefix = new Array<number>(incoming.length).fill(0);
    for (let index = 1, matched = 0; index < incoming.length; index += 1) {
      while (matched > 0 && incoming[index] !== incoming[matched]) matched = prefix[matched - 1];
      if (incoming[index] === incoming[matched]) matched += 1;
      prefix[index] = matched;
    }
    let overlap = 0;
    for (let index = 0, matched = 0; index < existing.length; index += 1) {
      while (matched > 0 && existing[index] !== incoming[matched]) matched = prefix[matched - 1];
      if (existing[index] === incoming[matched]) matched += 1;
      if (matched === incoming.length) {
        overlap = matched;
        if (index < existing.length - 1) matched = prefix[matched - 1];
      }
    }
    return [...existing, ...incoming.slice(overlap)].slice(-5_000);
  }

  async function loadLogs(reset = false) {
    if (!logTarget || loadingLogs) return;
    const requestGeneration = ++logRequestGeneration;
    const target = { ...logTarget };
    const clusterId = activeClusterId;
    const context = activeCluster;
    loadingLogs = true;
    const wasAtBottom = !logViewport || logViewport.scrollHeight - logViewport.scrollTop - logViewport.clientHeight < 24;
    const previousHeight = logViewport?.scrollHeight || 0;
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const response = await invoke<PodLogResponse>('read_pod_logs', {
        request: {
          kubeconfigPath: activeKubeconfigPath || kubeconfigPath || null,
          context,
          namespace: target.namespace,
          pod: target.pod,
          container: selectedLogContainer || null,
          sinceTime: reset ? null : logSinceTime || null,
          tailLines: reset || !logSinceTime ? 750 : null,
        },
      });
      const isStillActive = requestGeneration === logRequestGeneration
        && clusterId === activeClusterId
        && context === activeCluster
        && logTarget?.pod === target.pod
        && logTarget?.namespace === target.namespace;
      if (!isStillActive) return;
      logContainers = response.containers;
      logPorts = response.ports || [];
      selectedLogContainer = response.selectedContainer;
      if (reset) {
        logLines = response.lines;
      } else {
        logLines = appendLogSnapshot(logLines, response.lines);
      }
      const lastTimestamp = [...response.lines].reverse().map(logLineTimestamp).find(Boolean);
      if (lastTimestamp) logSinceTime = lastTimestamp;
      else if (reset) logSinceTime = '';
      await tick();
      if (logViewport) {
        if (wasAtBottom) logViewport.scrollTop = logViewport.scrollHeight;
        else logViewport.scrollTop += logViewport.scrollHeight - previousHeight;
      }
    } catch (error) {
      if (requestGeneration === logRequestGeneration && clusterId === activeClusterId) {
        refreshPermissionsAfterFailure(error, workloadResource);
        notify(`Could not read logs for ${target.pod}: ${String(error)}`);
      }
    } finally {
      if (requestGeneration === logRequestGeneration) loadingLogs = false;
    }
  }

  async function copyLogs() {
    const text = logLines.join('\n');
    if (!text) {
      notify('There are no log lines to copy yet');
      return;
    }
    try {
      let copied = false;
      if ('__TAURI_INTERNALS__' in window) {
        const { writeText } = await import('@tauri-apps/plugin-clipboard-manager');
        await writeText(text, { label: 'Kuberniva logs' });
        copied = true;
      } else if (navigator.clipboard?.writeText) {
        try {
          await navigator.clipboard.writeText(text);
          copied = true;
        } catch {
          // Browser previews can deny clipboard access; keep a selection fallback.
        }
      }
      if (!copied) {
        const textarea = document.createElement('textarea');
        textarea.value = text;
        textarea.setAttribute('readonly', '');
        textarea.style.position = 'fixed';
        textarea.style.opacity = '0';
        textarea.style.pointerEvents = 'none';
        document.body.appendChild(textarea);
        textarea.select();
        copied = document.execCommand('copy');
        textarea.remove();
      }
      if (!copied) throw new Error('The system clipboard did not accept the text');
      logsCopied = true;
      if (logCopyResetTimer) window.clearTimeout(logCopyResetTimer);
      logCopyResetTimer = window.setTimeout(() => {
        logsCopied = false;
        logCopyResetTimer = undefined;
      }, 1_800);
      notify(`${logLines.length} log line${logLines.length === 1 ? '' : 's'} copied`);
    } catch (error) {
      notify(`Could not copy logs: ${String(error)}`);
    }
  }

  function logDownloadFilename() {
    const pod = (logTarget?.pod || 'pod').replace(/[^a-zA-Z0-9._-]+/g, '-');
    const container = (selectedLogContainer || 'default').replace(/[^a-zA-Z0-9._-]+/g, '-');
    const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
    return `${pod}_${container}_${timestamp}.log`;
  }

  async function downloadLogs() {
    if (!logLines.length || downloadingLogs) return;
    const content = `${logLines.join('\n')}\n`;
    const lineCount = logLines.length;
    downloadingLogs = true;
    try {
      const { save } = await import('@tauri-apps/plugin-dialog');
      const path = await save({
        defaultPath: logDownloadFilename(),
        filters: [{ name: 'Log file', extensions: ['log', 'txt'] }],
      });
      if (!path) return;
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('save_log_file', { request: { path, content } });
      notify(`Saved ${lineCount} log line${lineCount === 1 ? '' : 's'}`);
    } catch (error) {
      notify(`Could not download logs: ${String(error)}`);
    } finally {
      downloadingLogs = false;
    }
  }

  function logPodKey(object: ResourceObject) {
    return `${object.namespace || (namespace === 'all namespaces' ? '' : namespace)}\u0000${object.name}`;
  }

  function logTargetKey(target: LogTarget) {
    return `${target.namespace}\u0000${target.pod}`;
  }

  function availableLogPods(primary: ResourceObject, candidates: ResourceObject[]) {
    const pods = new Map<string, ResourceObject>();
    [...candidates, primary].forEach((pod) => pods.set(logPodKey(pod), pod));
    return [...pods.values()].sort((left, right) => logPodKey(left).localeCompare(logPodKey(right)));
  }

  async function namespacePodIndex(resourceNamespace: string, fallback: ResourceObject[]) {
    const podResource = catalog.resources.find((resource) => resource.kind === 'Pod' && resource.namespaced);
    if (!podResource) return fallback;
    const cacheKey = resourceObjectCacheKey(activeClusterId, podResource, resourceNamespace);
    const cachedPods = resourceObjectCache.get(cacheKey);
    if (cachedPods) return cachedPods;
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const pods = await invoke<ResourceObject[]>('list_resource_objects', {
        request: {
          kubeconfigPath: activeKubeconfigPath || kubeconfigPath || null,
          context: activeCluster,
          group: podResource.group,
          version: podResource.version,
          kind: podResource.kind,
          plural: podResource.plural,
          namespaced: true,
          namespace: resourceNamespace,
        },
      });
      resourceObjectCache.set(cacheKey, pods);
      return pods.length ? pods : fallback;
    } catch {
      // The selected workload still has its authoritative Pod match list available.
      return fallback;
    }
  }

  async function selectLogPod(object: ResourceObject) {
    const objectNamespace = object.namespace || (namespace === 'all namespaces' ? '' : namespace);
    if (!objectNamespace) {
      notify('A namespace is required to read pod logs');
      return;
    }
    if (!await mayUsePodSubresource('get', 'log', object)) return;
    logRequestGeneration += 1;
    loadingLogs = false;
    logTarget = { pod: object.name, namespace: objectNamespace };
    logLines = [];
    logSinceTime = '';
    selectedLogContainer = undefined;
    logContainers = [];
    logPorts = [];
    logsCopied = false;
    if (logCopyResetTimer) window.clearTimeout(logCopyResetTimer);
    logCopyResetTimer = undefined;
    await loadLogs(true);
  }

  function selectLogPodByKey(key: string) {
    const pod = logPods.find((candidate) => logPodKey(candidate) === key);
    if (pod) void selectLogPod(pod);
  }

  async function openPodLogsWorkspace(object: ResourceObject, candidates: ResourceObject[] = [], scopeLabel = 'Pod') {
    closeLogs(false);
    const workspaceGeneration = logWorkspaceGeneration;
    stopOverviewRefresh();
    // Logs live in the Workloads inspector so the workload-type rail and the
    // selected object list remain available while an operator follows a stream.
    closeYamlEditor();
    logPods = availableLogPods(object, candidates);
    logScopeLabel = scopeLabel;
    selectedResource = null;
    relatedPods = null;
    relatedObject = null;
    clusterPickerOpen = false;
    namespaceOpen = false;
    if (activeView !== 'Workloads') {
      const podResource = catalog.resources.find((resource) => resource.category === 'Workloads' && resource.kind === 'Pod');
      if (podResource) {
        workloadResource = podResource;
        workloadObjects = logPods;
        clearWorkloadObjectSelection();
        workloadSearch = '';
      }
      closeEditor(false);
      activeView = 'Workloads';
      if (podResource && activeClusterId) {
        resourceObjectCache.set(resourceObjectCacheKey(activeClusterId, podResource, namespace), logPods);
        markLiveDataAvailable(podResource, activeClusterId, namespace, 'Workloads');
        startLiveObjectRefresh();
      }
    }
    workloadDetailMode = 'logs';
    await selectLogPod(object);
    if (workspaceGeneration !== logWorkspaceGeneration || activeView !== 'Workloads' || !logTarget) return;
    logRefreshTimer = window.setInterval(() => loadLogs(), 30_000);
  }

  async function openPodLogs(object: ResourceObject, candidates: ResourceObject[] = [], scopeLabel = 'Pod') {
    if (openingLogsTarget) return;
    const openingKey = logOpeningKey('Pod', object);
    openingLogsTarget = { key: openingKey, label: `Pod · ${object.name}` };
    await tick();
    try {
      if (openingLogsTarget?.key !== openingKey) return;
      await openPodLogsWorkspace(object, candidates, scopeLabel);
    } finally {
      if (openingLogsTarget?.key === openingKey) openingLogsTarget = null;
    }
  }

  async function openObject(resource: ResourceDescriptor, object: ResourceObject) {
    if (resource.kind === 'Secret' || resource.kind === 'ConfigMap' || resource.kind === 'Certificate') {
      await openResourceEditor(resource, object);
      return;
    }
    // In the Workloads workspace every object first opens its live detail pane.
    // Logs and terminal access are deliberate actions from there; a Deployment
    // is never itself a log source.
    if (activeView === 'Workloads' && resource.category === 'Workloads') {
      closeWorkloadLogs();
      await openResourceEditor(resource, object);
      return;
    }
    if (resource.kind === 'Pod') {
      const candidatePods = workloadObjects.some((candidate) => logPodKey(candidate) === logPodKey(object))
        ? workloadObjects
        : resourceObjects;
      await openPodLogs(object, candidatePods, 'Pod');
      return;
    }
    const workloadKindsWithPodSelectors = new Set(['Deployment', 'StatefulSet', 'DaemonSet', 'ReplicaSet', 'ReplicationController', 'Job']);
    if (resource.category !== 'Workloads' || !workloadKindsWithPodSelectors.has(resource.kind)) {
      if (activeView === 'Resources') {
        await openResourceEditor(resource, object);
        return;
      }
      await openYamlEditor(resource, object);
      return;
    }
    await openResourceEditor(resource, object);
  }

  onDestroy(() => {
    rememberActiveClusterSession();
    persistWorkspace();
    closeLogs();
    stopOverviewRefresh();
    stopLiveObjectRefresh();
    resourceWatchUnlisten?.();
    resourceWatchUnlisten = undefined;
    stopWindowFocusListening?.();
    stopWindowFocusListening = undefined;
  });

  onMount(() => {
    applyTheme(loadThemePreference());
    // The native window starts hidden; reveal it once the zoomed first frame is painted.
    void applyUiScale(loadUiScalePreference(), false).finally(revealWindow);
    recentResourceKeys = loadRecentResourceKeys();
    cliHistory = loadCliHistory();
    void loadAppVersion();
    updateCheckTimer = window.setTimeout(() => void checkForUpdates(true), 8_000);
    resourceNavigatorWidth = loadPaneSize(resourceNavigatorWidthStorageKey, 272, 220, 440);
    resourceObjectPaneWidth = loadPaneSize(resourceObjectPaneWidthStorageKey, 300, 180, 520);
    void restoreWorkspace();
    void setupWindowFocusListener();
    resourceWatchListenerReady = setupResourceWatchListener();
    const closeFloatingMenus = (event: PointerEvent) => {
      const target = event.target instanceof Element ? event.target : null;
      if (!target?.closest('.cluster-selector')) clusterPickerOpen = false;
      if (!target?.closest('.namespace-picker')) namespaceOpen = false;
      if (!target?.closest('.favorite-context-menu, .favorite-shortcut, .favorite-card-open')) favoriteContextMenu = null;
    };
    const closeFloatingMenusOnEscape = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
        event.preventDefault();
        openCommandSearch();
        return;
      }
      if ((event.metaKey || event.ctrlKey) && !event.shiftKey && !event.altKey && /^[1-9]$/.test(event.key)) {
        const favorite = favoriteClusters[Number(event.key) - 1];
        if (favorite) {
          event.preventDefault();
          favoriteContextMenu = null;
          if (favorite.id !== activeClusterId) void selectCluster(favorite.id);
          return;
        }
      }
      if (event.metaKey || event.ctrlKey) {
        if (event.key === '+' || event.key === '=') {
          event.preventDefault();
          adjustUiScale(0.05);
          return;
        }
        if (event.key === '-') {
          event.preventDefault();
          adjustUiScale(-0.05);
          return;
        }
        if (event.key === '0') {
          event.preventDefault();
          void applyUiScale(0.9);
          return;
        }
      }
      if (event.key !== 'Escape') return;
      if (favoriteContextMenu) {
        favoriteContextMenu = null;
        return;
      }
      if (configModalOpen && !commandOpen && !deletionTarget && !yamlResource) {
        requestCloseConfigEditor();
        return;
      }
      if (commandOpen) {
        commandOpen = false;
        commandQuery = '';
      }
      closeSidebarTypeMenus();
    };
    const schedulePendingAfterClick = () => {
      if (resumeRecoveryPending || resourceWatchRefreshPending) schedulePendingResumeRecovery();
    };
    window.addEventListener('pointerdown', closeFloatingMenus);
    window.addEventListener('keydown', closeFloatingMenusOnEscape);
    window.addEventListener('click', schedulePendingAfterClick);
    const handleVisibilityChange = () => {
      if (document.visibilityState === 'hidden') {
        lastHiddenAt = Date.now();
        return;
      }
      const hiddenDuration = lastHiddenAt ? Date.now() - lastHiddenAt : 0;
      lastHiddenAt = 0;
      if (hiddenDuration > 5_000) queueLiveResumeRecovery();
    };
    const handleWindowFocus = () => {
      const hiddenDuration = lastHiddenAt ? Date.now() - lastHiddenAt : 0;
      lastHiddenAt = 0;
      if (hiddenDuration > 5_000) queueLiveResumeRecovery();
    };
    const handleNetworkOnline = () => queueLiveResumeRecovery();
    document.addEventListener('visibilitychange', handleVisibilityChange);
    window.addEventListener('focus', handleWindowFocus);
    window.addEventListener('online', handleNetworkOnline);
    return () => {
      window.removeEventListener('pointerdown', closeFloatingMenus);
      window.removeEventListener('keydown', closeFloatingMenusOnEscape);
      window.removeEventListener('click', schedulePendingAfterClick);
      document.removeEventListener('visibilitychange', handleVisibilityChange);
      window.removeEventListener('focus', handleWindowFocus);
      window.removeEventListener('online', handleNetworkOnline);
      if (resumeRecoveryTimer) window.clearTimeout(resumeRecoveryTimer);
      resumeRecoveryTimer = undefined;
      if (updateCheckTimer) window.clearTimeout(updateCheckTimer);
    };
  });

  function readStoredCatalogs(): Record<string, ClusterCatalog> {
    try {
      const parsed = JSON.parse(window.localStorage.getItem(catalogStorageKey) || '{}');
      return parsed && typeof parsed === 'object' ? parsed : {};
    } catch {
      return {};
    }
  }

  function loadStoredCatalog(clusterId: string) {
    const stored = readStoredCatalogs()[clusterId];
    return stored && Array.isArray(stored.resources) && Array.isArray(stored.namespaces) ? stored : null;
  }

  function storeCatalog(clusterId: string, value: ClusterCatalog | null) {
    const { [clusterId]: _previous, ...others } = readStoredCatalogs();
    const entries = Object.entries(others).slice(-(storedCatalogLimit - 1));
    if (value) entries.push([clusterId, value]);
    try {
      window.localStorage.setItem(catalogStorageKey, JSON.stringify(Object.fromEntries(entries)));
    } catch {
      // The cache only speeds up the next launch; live discovery remains authoritative.
    }
  }

  function startClusterViewLoads() {
    if (activeView === 'Overview') {
      void loadClusterOverview();
      startOverviewRefresh();
    } else if (activeView === 'Events') {
      void loadClusterEvents(true);
    }
  }

  async function loadCluster(cluster: Cluster, force = false) {
    // A Pod name belongs to exactly one cluster context. Never carry its stream across a switch.
    resourceRequestGeneration += 1;
    workloadRequestGeneration += 1;
    overviewRequestGeneration += 1;
    eventsRequestGeneration += 1;
    loadingOverview = false;
    loadingEvents = false;
    stopLiveObjectRefresh();
    clearLiveDataFreshness();
    clearResourceObjectSelection();
    resourceObjects = [];
    workloadObjects = [];
    relatedPods = null;
    relatedObject = null;
    selectedResource = null;
    loadingObjects = false;
    loadingWorkloads = false;
    closeLogs();
    closeEditor();
    closeYamlEditor();
    clusterPickerOpen = false;
    namespaceOpen = false;
    catalogPermissionsReady = false;
    permissionError = '';
    permissionRequestsInFlight = 0;
    loadingPermissions = false;
    activeClusterId = cluster.id;
    activeCluster = cluster.name;
    activeKubeconfigPath = cluster.kubeconfigPath;
    liveDataStatus = 'loading';
    liveDataStatusMessage = 'Waiting for live data';
    resumeRecoveryPending = false;
    resumeRecoveryContext = null;
    if (eventsClusterId !== cluster.id) {
      clusterEvents = [];
      eventsObservedAt = '';
      eventsError = '';
      eventsClusterId = '';
    }
    restoreClusterSession(cluster);
    catalogError = '';
    overviewError = '';
    selectedResource = null;
    const cachedCatalog = catalogCache.get(cluster.id);
    if (cachedCatalog && !force) {
      catalog = cachedCatalog;
      void loadCatalogPermissions();
      updateCluster(cluster.id, { status: 'Connected', tone: 'green' });
      notify(`Switched to ${cluster.name} · ${catalog.resources.length} resources`);
      startClusterViewLoads();
      return;
    }

    // Show the last known catalog immediately; discovery below confirms or replaces it.
    const storedCatalog = force ? null : loadStoredCatalog(cluster.id);
    catalog = storedCatalog || { context: cluster.name, namespaces: [], resources: [] };
    loadingCatalog = !storedCatalog;
    updateCluster(cluster.id, { status: 'Connecting', tone: 'blue' });
    if (storedCatalog) {
      void loadCatalogPermissions();
      startClusterViewLoads();
    }
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const sourcePath = cluster.kubeconfigPath || kubeconfigPath || null;
      const discovered = await invoke<ClusterCatalog>('discover_cluster_catalog', { kubeconfigPath: sourcePath, context: cluster.name });
      if (activeClusterId !== cluster.id) return;
      catalogCache.set(cluster.id, discovered);
      storeCatalog(cluster.id, discovered);
      updateCluster(cluster.id, { status: 'Connected', tone: 'green' });
      if (storedCatalog && JSON.stringify(storedCatalog) === JSON.stringify(discovered)) return;
      catalog = discovered;
      void loadCatalogPermissions();
      notify(`Connected to ${cluster.name} · ${catalog.resources.length} resources discovered`);
      if (!storedCatalog) startClusterViewLoads();
    } catch (error) {
      if (activeClusterId !== cluster.id) return;
      catalogError = String(error);
      updateCluster(cluster.id, { status: 'Connection failed', tone: 'red' });
      notify(`Could not connect to ${cluster.name}: ${catalogError}`);
    } finally {
      if (activeClusterId === cluster.id) loadingCatalog = false;
    }
  }

  async function selectCluster(id: string) {
    const cluster = clusters.find((candidate) => candidate.id === id);
    if (!cluster) return;
    rememberActiveClusterSession();
    clusterPickerOpen = false;
    stopOverviewRefresh();
    closeEditor();
    closeYamlEditor();
    selectedResource = null;
    nodeDetailTab = 'Overview';
    activeView = 'Overview';
    await loadCluster(cluster);
  }

  function deletionTargetName(target = deletionTarget) {
    if (!target) return '';
    if (target.type === 'resource') return target.object.name;
    if (target.type === 'bulk-resource') return `${target.objects.length} ${target.resource.kind} objects`;
    return target.cluster.name;
  }

  function bulkDeletionObjectLabel(target: DeletionTarget | null, object: ResourceObject) {
    if (!target || target.type !== 'bulk-resource') return object.name;
    const objectNamespace = object.namespace || (target.namespaceScope === 'all namespaces' ? '' : target.namespaceScope);
    return objectNamespace ? `${objectNamespace}/${object.name}` : object.name;
  }

  function beginDeletionFocus() {
    deletionReturnFocus = typeof document !== 'undefined' && document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null;
    void tick().then(() => deletionDialog?.focus());
  }

  function restoreDeletionFocus() {
    const previous = deletionReturnFocus;
    deletionReturnFocus = null;
    void tick().then(() => {
      if (previous?.isConnected) {
        previous.focus();
        return;
      }
      document.querySelector<HTMLElement>('.resource-object-columns input, .workload-object-list-header input')?.focus();
    });
  }

  function requestResourceDeletion(resource: ResourceDescriptor, object: ResourceObject) {
    if (savingEditor || loadingEditor) return;
    const view = activeView === 'Resources' || activeView === 'Workloads' ? activeView : null;
    if (!activeClusterId || !view) return;
    deletionTarget = {
      type: 'resource',
      resource,
      object: { ...object },
      clusterId: activeClusterId,
      cluster: activeCluster,
      kubeconfigPath: activeKubeconfigPath || kubeconfigPath || undefined,
      namespaceScope: namespace,
      view,
    };
    bulkDeleteProgress = null;
    deletionStep = 1;
    beginDeletionFocus();
  }

  function requestBulkResourceDeletion(resource: ResourceDescriptor, objects = selectedResourceObjects) {
    if (deletingResource || loadingEditor || savingEditor || loadingYaml || savingYaml || objects.length < 1) return;
    const view = activeView === 'Resources' || activeView === 'Workloads' ? activeView : null;
    if (!activeClusterId || !view) return;
    deletionTarget = {
      type: 'bulk-resource',
      resource,
      objects: objects.map((object) => ({ ...object })),
      namespaceScope: namespace,
      clusterId: activeClusterId,
      cluster: activeCluster,
      kubeconfigPath: activeKubeconfigPath || kubeconfigPath || undefined,
      view,
    };
    bulkDeleteProgress = null;
    deletionStep = 1;
    beginDeletionFocus();
  }

  function requestClusterRemoval(cluster: Cluster) {
    deletionTarget = { type: 'cluster', cluster };
    deletionStep = 1;
    beginDeletionFocus();
  }

  function removeCluster(id: string) {
    const cluster = clusters.find((candidate) => candidate.id === id);
    if (cluster) requestClusterRemoval(cluster);
  }

  function cancelDeletion() {
    if (deletingResource) return;
    deletionTarget = null;
    bulkDeleteProgress = null;
    deletionStep = 1;
    restoreDeletionFocus();
    schedulePendingResumeRecovery();
  }

  function continueDeletion() {
    deletionStep = 2;
    void tick().then(() => deletionConfirmButton?.focus());
  }

  function handleDeletionDialogKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.preventDefault();
      cancelDeletion();
      return;
    }
    if (event.key !== 'Tab' || !deletionDialog) return;
    const focusable = [...deletionDialog.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), [tabindex]:not([tabindex="-1"])')];
    if (!focusable.length) return;
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  async function confirmDeletion() {
    const target = deletionTarget;
    if (!target || deletionStep !== 2 || deletingResource) return;
    if (target.type === 'resource') await deleteResourceTarget(target);
    else if (target.type === 'bulk-resource') await deleteBulkResourceObjects(target);
    else await removeClusterContext(target.cluster);
  }

  async function removeClusterContext(cluster: Cluster) {
    const currentCluster = clusters.find((candidate) => candidate.id === cluster.id);
    if (!currentCluster || currentCluster.name !== cluster.name || currentCluster.kubeconfigPath !== cluster.kubeconfigPath) {
      notify('Context removal canceled because the tracked cluster changed.');
      deletionTarget = null;
      deletionStep = 1;
      restoreDeletionFocus();
      return;
    }
    deletingResource = true;
    try {
      const wasActive = activeClusterId === cluster.id;
      clusters = clusters.filter((candidate) => candidate.id !== cluster.id);
      catalogCache.delete(cluster.id);
      storeCatalog(cluster.id, null);
      clusterSessionCache.delete(cluster.id);
      const { [cluster.id]: _removedNamespace, ...remainingNamespaces } = persistedClusterNamespaces;
      persistedClusterNamespaces = remainingNamespaces;
      favoriteClusterIds = favoriteClusterIds.filter((id) => id !== cluster.id);
      const { [cluster.id]: _removedFavoriteName, ...remainingFavoriteNames } = favoriteClusterNames;
      favoriteClusterNames = remainingFavoriteNames;
      clearClusterObjectCache(cluster.id);
      if (wasActive) {
        resourceRequestGeneration += 1;
        workloadRequestGeneration += 1;
        loadingObjects = false;
        loadingWorkloads = false;
        stopOverviewRefresh();
        stopLiveObjectRefresh();
        clearLiveDataFreshness();
        resourceObjects = [];
        workloadObjects = [];
        relatedPods = null;
        relatedObject = null;
        clearResourceObjectSelection();
        closeLogs();
        activeClusterId = '';
        activeCluster = clusters.length ? 'Select a cluster' : 'No cluster connected';
        activeKubeconfigPath = undefined;
        catalog = { context: '', namespaces: [], resources: [] };
        clusterOverview = null;
        clusterEvents = [];
        eventsObservedAt = '';
        eventsError = '';
        eventsClusterId = '';
        selectedResource = null;
        clearResourceObjectSelection();
        closeEditor();
        closeYamlEditor();
        activeView = clusters.length ? 'Clusters' : 'Overview';
      }
      if (!clusters.length) {
        connectedKubeconfig = false;
        sourceConfigured = kubeconfigSources.length > 0;
        if (!kubeconfigSources.length) kubeconfigPath = '';
      }
      persistWorkspace();
      notify(`Removed ${cluster.name} from Kuberniva. The source kubeconfig was not changed.`);
    } catch (error) {
      notify(`Could not remove ${cluster.name}: ${String(error)}`);
    } finally {
      deletingResource = false;
      deletionTarget = null;
      deletionStep = 1;
      restoreDeletionFocus();
      schedulePendingResumeRecovery();
    }
  }

  async function refreshActiveCluster() {
    await refreshCurrentView();
  }

  async function connectKubeconfig() {
    loadingCatalog = true;
    try {
      if (!('__TAURI_INTERNALS__' in window)) {
        throw new Error('Kubeconfig connections are available in the Kuberniva desktop app');
      } else {
        const { invoke } = await import('@tauri-apps/api/core');
        const summary = await invoke<KubeconfigSummary>('read_kubeconfig_contexts', { kubeconfigPath: kubeconfigPath || null });
        if (!summary.contexts.length) throw new Error('No contexts found in this kubeconfig');
        const clusterCountBeforeAdd = clusters.length;
        rememberKubeconfigSource(kubeconfigPath);
        applyKubeconfigSummary(summary, kubeconfigPath);
        sourceConfigured = true;
        persistWorkspace();
        await navigateTo('Clusters');
        closeKubeconfigModal(true);
        const addedCount = clusters.length - clusterCountBeforeAdd;
        notify(`${addedCount > 0 ? `${addedCount} new context${addedCount === 1 ? '' : 's'} added` : 'Source refreshed'} · ${clusters.length} context${clusters.length === 1 ? '' : 's'} tracked locally.`);
      }
    } catch (error) {
      notify(`Could not connect: ${String(error)}`);
    } finally {
      loadingCatalog = false;
    }
  }

  async function importPastedKubeconfig() {
    const content = pastedKubeconfig.trim();
    if (!content) {
      notify('Paste kubeconfig YAML before importing');
      return;
    }
    loadingCatalog = true;
    try {
      if (!('__TAURI_INTERNALS__' in window)) {
        throw new Error('Pasted kubeconfig import is available in the Kuberniva desktop app');
      }
      const { invoke } = await import('@tauri-apps/api/core');
      const summary = await invoke<KubeconfigSummary>('import_pasted_kubeconfig', { content });
      if (!summary.contexts.length) throw new Error('No contexts found in the pasted kubeconfig');
      const savedSource = summary.contexts[0]?.sourcePath;
      if (!savedSource) throw new Error('The pasted kubeconfig could not be saved locally');

      const clusterCountBeforeAdd = clusters.length;
      kubeconfigPath = savedSource;
      rememberKubeconfigSource(savedSource);
      applyKubeconfigSummary(summary, savedSource);
      sourceConfigured = true;
      persistWorkspace();
      await navigateTo('Clusters');
      closeKubeconfigModal(true);
      const addedCount = clusters.length - clusterCountBeforeAdd;
      notify(`${addedCount > 0 ? `${addedCount} new context${addedCount === 1 ? '' : 's'} imported` : 'Pasted source refreshed'} · ${clusters.length} context${clusters.length === 1 ? '' : 's'} tracked locally.`);
    } catch (error) {
      notify(`Could not import kubeconfig: ${String(error)}`);
    } finally {
      loadingCatalog = false;
    }
  }

  async function syncKubeconfigSources() {
    if (!kubeconfigSources.length) {
      notify('Add a kubeconfig file or folder before syncing');
      return;
    }
    loadingCatalog = true;
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const results = await Promise.allSettled(kubeconfigSources.map((source) =>
        invoke<KubeconfigSummary>('read_kubeconfig_contexts', { kubeconfigPath: source || null }),
      ));
      let syncedSourceCount = 0;
      results.forEach((result, index) => {
        if (result.status !== 'fulfilled') return;
        applyKubeconfigSummary(result.value, kubeconfigSources[index], true);
        syncedSourceCount += 1;
      });
      if (!syncedSourceCount) {
        const firstFailure = results.find((result) => result.status === 'rejected');
        throw new Error(firstFailure && firstFailure.status === 'rejected' ? String(firstFailure.reason) : 'No source could be read');
      }
      sourceConfigured = true;
      persistWorkspace();
      notify(`Synced ${syncedSourceCount} source${syncedSourceCount === 1 ? '' : 's'} · ${clusters.length} context${clusters.length === 1 ? '' : 's'} tracked locally.`);
    } catch (error) {
      notify(`Could not sync kubeconfig sources: ${String(error)}`);
    } finally {
      loadingCatalog = false;
    }
  }

  async function openResource(resource: ResourceDescriptor, options: { silent?: boolean } = {}) {
    const silent = options.silent === true;
    const requestGeneration = ++resourceRequestGeneration;
    const requestClusterId = activeClusterId;
    const requestNamespace = namespace;
    const requestView = activeView;
    const requestResourceKey = resourceKey(resource);
    if (!await mayListResource(resource)) {
      if (requestGeneration === resourceRequestGeneration && requestClusterId === activeClusterId && requestNamespace === namespace) {
        selectedResource = null;
        resourceObjects = [];
        loadingObjects = false;
        liveDataStatus = 'loaded';
        liveDataStatusMessage = 'This resource type is not available to the current identity';
      }
      return;
    }
    const previousResourceKey = selectedResource ? resourceKey(selectedResource) : '';
    if (!silent && previousResourceKey !== requestResourceKey) {
      if (activeClusterId && selectedResource) clearLiveDataFreshness(activeClusterId, selectedResource, namespace);
      clearResourceObjectSelection();
    }
    if (!silent) {
      rememberRecentResource(resource);
      closeEditor();
      closeYamlEditor();
    }
    selectedResource = resource;
    if (!silent && activeView === 'Resources') {
      liveDataStatus = 'loading';
      liveDataStatusMessage = 'Loading live data';
    }
    if (!silent) {
      relatedPods = null;
      relatedObject = null;
    }
    const cacheKey = resourceObjectCacheKey(requestClusterId, resource, requestNamespace);
    const cachedObjects = resourceObjectCache.get(cacheKey);
    if (cachedObjects) {
      resourceObjects = cachedObjects;
      reconcileResourceObjectSelection(cachedObjects);
      loadingObjects = false;
      if (requestView === 'Resources') markLiveDataAvailable(resource, requestClusterId, requestNamespace, 'Resources');
      if (!silent && activeView === 'Resources') startLiveObjectRefresh();
      return;
    }
    if (!silent) {
      loadingObjects = true;
      resourceObjects = [];
    }
    try {
      if (!('__TAURI_INTERNALS__' in window)) {
        throw new Error('Resource listing is available in the Kuberniva desktop app');
      } else {
        const { invoke } = await import('@tauri-apps/api/core');
        const response = await invoke<ResourceObject[]>('list_resource_objects', {
          request: {
            kubeconfigPath: activeKubeconfigPath || kubeconfigPath || null,
            context: activeCluster,
            group: resource.group,
            version: resource.version,
            kind: resource.kind,
            plural: resource.plural,
            namespaced: resource.namespaced,
            namespace: requestNamespace,
          },
        });
        if (requestGeneration !== resourceRequestGeneration
          || requestClusterId !== activeClusterId
          || requestNamespace !== namespace
          || requestView !== activeView
          || !selectedResource
          || resourceKey(selectedResource) !== requestResourceKey) return;
        resourceObjects = response;
        resourceObjectCache.set(cacheKey, response);
        reconcileResourceObjectSelection(response);
        markLiveDataAvailable(resource, requestClusterId, requestNamespace, 'Resources');
        if (!silent && activeView === 'Resources') startLiveObjectRefresh();
        if (
          editorResource
          && editorObject
          && resourceKey(editorResource) === requestResourceKey
          && !response.some((object) => object.name === editorObject?.name && object.namespace === editorObject?.namespace)
        ) {
          if (!hasPreservedLiveState()) {
            const removedName = editorObject.name;
            closeEditor(false);
            closeYamlEditor();
            notify(resource.kind + ' ' + removedName + ' is no longer present in this namespace');
          } else {
            markLiveDataPaused(`${resource.kind} changed while your workflow is open`);
            markResourceWatchRefreshPending();
          }
        }
      }
    } catch (error) {
      if (requestGeneration === resourceRequestGeneration && requestClusterId === activeClusterId && requestView === activeView) {
        markLiveDataUnavailable(`Could not list ${resource.kind}`, liveRefreshContext(requestClusterId, 'Resources', resource, requestNamespace));
        notify(`Could not list ${resource.kind}: ${String(error)}`);
      }
    } finally {
      if (requestGeneration === resourceRequestGeneration && requestClusterId === activeClusterId && requestView === activeView && !silent) loadingObjects = false;
      if (requestGeneration === resourceRequestGeneration && requestClusterId === activeClusterId && requestNamespace === namespace && requestView === activeView && selectedResource && resourceKey(selectedResource) === requestResourceKey) {
        flushPendingResourceWatchRefresh();
      }
    }
  }
</script>

<svelte:head>
  <title>Kuberniva — Kubernetes, in focus</title>
  <meta name="description" content="A calm, fast Kubernetes control surface." />
</svelte:head>

<main class:sidebar-collapsed={sidebarHidden} class:theme-dark={theme === 'dark'} style:--sidebar-width={`${sidebarHidden ? 0 : sidebarWidth}px`} style:--navigator-width={`${resourceNavigatorWidth}px`} style:--resource-object-width={`${resourceObjectPaneWidth}px`}>
  <aside class:sidebar-hidden={sidebarHidden} class:sidebar-flyout-open={sidebarWorkloadMenuOpen || sidebarResourceMenuOpen} class="sidebar" style:width={`${sidebarWidth}px`} style:flex-basis={`${sidebarWidth}px`}>
    <div class="brand">
      <img class="brand-mark" src="/kuberniva-mark.svg" alt="" />
      <span class="brand-wordmark"><strong>Kube</strong><span>rniva</span></span>
    </div>

    <nav aria-label="Cluster navigation">
      <p class="eyebrow">Cluster workspace</p>
      {#each ['Overview', 'Events', 'Workloads', 'Resources', 'Custom APIs'] as view}
        {#if view === 'Workloads'}
          <button class:active={activeView === 'Workloads'} class="nav-item" type="button" on:click={() => navigateTo('Workloads')}><span class="nav-icon"><Workflow size={17} strokeWidth={1.8} /></span>Workloads{#if workloadObjects.length}<span class="count">{workloadObjects.length}</span>{/if}</button>
        {:else if view === 'Resources' || view === 'Custom APIs'}
          <div class="sidebar-tree-group">
            <div class="sidebar-tree-trigger">
              <button class:active={activeView === 'Resources' && customApiWorkspace === (view === 'Custom APIs')} class="nav-item" type="button" on:click={() => openResourcesHome(view === 'Custom APIs')}><span class="nav-icon">{#if view === 'Custom APIs'}<Boxes size={17} strokeWidth={1.8} />{:else}<Database size={17} strokeWidth={1.8} />{/if}</span>{view}<span class="count">{activeClusterId ? (view === 'Custom APIs' ? customApiResources.length : resourceWorkspaceResources.length) : 0}</span></button>
              {#if activeClusterId && treeSectionsFor(view).length}<button class:sidebar-tree-toggle-open={sidebarTreeOpen[treeKeyFor(view)]} class="sidebar-tree-toggle" type="button" aria-label={`${sidebarTreeOpen[treeKeyFor(view)] ? 'Collapse' : 'Expand'} ${view}`} aria-expanded={sidebarTreeOpen[treeKeyFor(view)]} on:click={() => (sidebarTreeOpen = { ...sidebarTreeOpen, [treeKeyFor(view)]: !sidebarTreeOpen[treeKeyFor(view)] })}><ChevronDown size={14} /></button>{/if}
            </div>
            {#if activeClusterId && sidebarTreeOpen[treeKeyFor(view)]}
              <div class="sidebar-tree" role="tree" aria-label={`${view} types`}>
                {#each treeSectionsFor(view) as section}
                  <button class="sidebar-tree-section" type="button" role="treeitem" aria-selected="false" aria-expanded={isTreeSectionOpen(view, section, sidebarTreeSections, selectedResource)} on:click={() => toggleTreeSection(view, section)}><ChevronRight size={12} class={isTreeSectionOpen(view, section, sidebarTreeSections, selectedResource) ? 'sidebar-tree-chevron-open' : ''} /><span>{section.title}</span><small>{section.resources.length}</small></button>
                  {#if isTreeSectionOpen(view, section, sidebarTreeSections, selectedResource)}
                    {#each section.resources as resource}
                      <button class:sidebar-tree-item-selected={isSidebarResourceSelected(resource, activeView, selectedResource)} class="sidebar-tree-item" type="button" role="treeitem" aria-selected={isSidebarResourceSelected(resource, activeView, selectedResource)} title={`${resource.kind} · ${resource.apiVersion}`} on:click={() => openTreeResource(resource)}>{kindLabel(resource)}</button>
                    {/each}
                  {/if}
                {/each}
              </div>
            {/if}
          </div>
        {:else}
          <button class:active={activeView === view} class="nav-item" on:click={() => navigateTo(view as View)}><span class="nav-icon">{#if view === 'Overview'}<LayoutDashboard size={17} strokeWidth={1.8} />{:else}<ScrollText size={17} strokeWidth={1.8} />{/if}</span>{view}{#if view === 'Events' && namespaceClusterEvents.length}<span class="count">{namespaceClusterEvents.length}</span>{/if}</button>
        {/if}
      {/each}
    </nav>

    <section class="sidebar-favorites" aria-label="Favorite cluster shortcuts">
      <div class="sidebar-favorites-heading">
        <p class="eyebrow">Favorites</p>
        <button class:active={activeView === 'Favorites'} class="sidebar-favorites-manage" type="button" title="Manage favorites" on:click={() => navigateTo('Favorites')}>{favoriteClusters.length}/10</button>
      </div>
      {#if favoriteClusters.length}
        <div class="favorite-list" role="list">
          {#each favoriteClusters as cluster, index}
            <div class:favorite-row-active={cluster.id === activeClusterId} class="favorite-row" role="listitem">
              <button class="favorite-shortcut" type="button" aria-current={cluster.id === activeClusterId ? 'true' : undefined} title={`${favoriteLabel(cluster)} · ${cluster.status}`} on:click={() => { favoriteContextMenu = null; void selectCluster(cluster.id); }} on:contextmenu={(event) => openFavoriteContextMenu(event, cluster)}><i class="status-dot {cluster.tone}"></i><span>{favoriteLabel(cluster)}</span>{#if index < 9}<kbd>⌘{index + 1}</kbd>{/if}</button>
              <button class="favorite-row-menu" type="button" aria-label={`Rename or remove ${favoriteLabel(cluster)}`} title="Rename or remove" on:click={(event) => openFavoriteContextMenu(event, cluster)}>⋯</button>
            </div>
          {/each}
        </div>
      {:else}<p class="favorite-shortcut-none">Star clusters in the cluster manager to pin them here.</p>{/if}
      {#if favoriteRenameId}
        <form class="favorite-rename favorite-rename-grid" on:submit|preventDefault={saveFavoriteRename}><input bind:value={favoriteRenameValue} maxlength="80" aria-label="Rename favorite shortcut" /><button type="submit" aria-label="Save shortcut name" title="Save">✓</button><button type="button" aria-label="Cancel shortcut rename" title="Cancel" on:click={cancelFavoriteRename}>×</button></form>
      {/if}
    </section>

    {#if favoriteContextMenu && favoriteContextCluster}
      <div class="favorite-context-menu" style:left={`${favoriteContextMenu.x}px`} style:top={`${favoriteContextMenu.y}px`} role="menu" aria-label={`Actions for ${favoriteLabel(favoriteContextCluster)}`}>
        <strong>{favoriteLabel(favoriteContextCluster)}</strong>
        <button on:click={() => startFavoriteRename(favoriteContextCluster.id)}>Rename shortcut</button>
        <button on:click={() => { toggleFavoriteCluster(favoriteContextCluster); favoriteContextMenu = null; }}>Remove from Favorites</button>
      </div>
    {/if}

    <div class="sidebar-bottom">
      <button class:active={activeView === 'Settings'} class="nav-item" on:click={() => navigateTo('Settings')}><span class="nav-icon"><Settings2 size={17} strokeWidth={1.8} /></span>Settings{#if updateState === 'available' || updateState === 'ready'}<i class="nav-update-dot" aria-label="Update available"></i>{/if}</button>
    </div>
  </aside>
  <div class:sidebar-hidden={sidebarHidden} class="sidebar-resizer" role="separator" aria-orientation="vertical" aria-label="Resize sidebar" on:pointerdown={startSidebarResize}></div>

  <section class="app-shell">
    <header class="topbar">
      <div class="toolbar-leading">
        <button class="sidebar-toggle" aria-label={sidebarHidden ? 'Show sidebar' : 'Hide sidebar'} title={sidebarHidden ? 'Show sidebar' : 'Hide sidebar'} on:click={toggleSidebar}><Menu size={17} strokeWidth={2} /></button>
        <div class="cluster-selector topbar-cluster-selector">
          <button class:cluster-picker-open={clusterPickerOpen} class="cluster-picker" aria-expanded={clusterPickerOpen} on:click={() => (clusterPickerOpen = !clusterPickerOpen)}><span class="cluster-dot"></span><span class="cluster-picker-copy"><small>Active cluster</small><span class="cluster-picker-name">{activeCluster}</span></span><span class="chevron">⌄</span></button>
          {#if clusterPickerOpen}
            <div class="cluster-selector-menu" role="menu" aria-label="Select cluster"><div class="cluster-selector-heading"><span>Clusters</span><b>{clusters.length}</b></div>{#if clusters.length}<div class="cluster-selector-items">{#each clusters as cluster}<button class:chosen={cluster.id === activeClusterId} title={`${cluster.authMethod || cluster.provider} · ${cluster.status}`} on:click={() => selectCluster(cluster.id)}><span class="status-dot {cluster.tone}"></span><span><strong>{cluster.name}</strong><small>{cluster.authMethod || cluster.provider}</small></span>{#if cluster.id === activeClusterId}<i>✓</i>{/if}</button>{/each}</div>{:else}<p class="cluster-selector-empty">No kubeconfig contexts added yet.</p>{/if}<button class="cluster-selector-manage" on:click={() => { clusterPickerOpen = false; void navigateTo('Clusters') }}>Manage clusters <span>→</span></button></div>
          {/if}
        </div>
        {#if showClusterWorkspaceControls}
          <div class="namespace-picker global-namespace-picker">
            <button class="global-namespace-trigger" disabled={namespaceControlBusy} aria-label={`Namespace: ${namespace === 'all namespaces' ? 'All namespaces' : namespace}`} aria-expanded={namespaceOpen} on:click={() => (namespaceOpen = !namespaceOpen)}><span class="namespace-dot"></span><span class="global-namespace-copy"><small>Namespace</small><strong>{namespace === 'all namespaces' ? 'All namespaces' : namespace}</strong></span><span class="chevron">⌄</span></button>
            {#if namespaceOpen}
              <div class="namespace-menu global-namespace-menu" role="menu" aria-label="Select namespace"><button class:namespace-selected={namespace === 'all namespaces'} on:click={() => chooseNamespace('all namespaces')}><span>All namespaces</span>{#if namespace === 'all namespaces'}✓{/if}</button>{#each catalog.namespaces as availableNamespace}<button class:namespace-selected={namespace === availableNamespace} on:click={() => chooseNamespace(availableNamespace)}><span>{availableNamespace}</span>{#if namespace === availableNamespace}✓{/if}</button>{/each}</div>
            {/if}
          </div>
        {/if}
      </div>
      <div class="top-actions">
        {#if showClusterWorkspaceControls}<button class="topbar-refresh" disabled={refreshingCurrentView || hasProtectedWorkflow()} aria-label="Reconnect and refresh current view" title={hasProtectedWorkflow() ? 'Refresh paused while the current workflow is open' : 'Reconnect and refresh'} on:click={refreshCurrentView}><RefreshCw size={16} class={refreshingCurrentView ? 'animate-spin' : ''} /></button>{/if}
        <button class="command-button" aria-label="Search resources" title="Search resources · ⌘ K" on:click={openCommandSearch}><Search size={15} strokeWidth={2} /><span>Search resources</span><kbd>⌘ K</kbd></button>
        <button class="icon-button theme-toggle" aria-label={theme === 'dark' ? 'Switch to light mode' : 'Switch to dark mode'} title={theme === 'dark' ? 'Switch to light mode' : 'Switch to dark mode'} on:click={toggleTheme}>{#if theme === 'dark'}<Sun size={17} strokeWidth={1.9} />{:else}<Moon size={17} strokeWidth={1.9} />{/if}</button>
        {#if notifications.length}
          <button class="icon-button" aria-label={`${notifications.length} notification${notifications.length === 1 ? '' : 's'}`} title="Clear notifications" on:click={() => (notifications = [])}><Bell size={18} strokeWidth={1.9} /><i></i></button>
        {/if}
      </div>
    </header>

    <div class:resource-workspace-content={activeView === 'Resources'} class:workload-workspace-content={activeView === 'Workloads'} class="content">
      <div class:cluster-page-heading={activeView === 'Clusters'} class:resource-page-heading={activeView === 'Resources'} class="page-heading">
        <div>
          <div class="title-line"><h1>{activeViewTitle}</h1>{#if activeClusterId && !catalogError}{#if ['Workloads', 'Resources'].includes(activeView)}<span class:live-status-loading={liveDataStatus === 'loading'} class:live-status-loaded={liveDataStatus === 'loaded'} class:live-status-stale={liveDataStatus === 'stale'} class:live-status-paused={liveDataStatus === 'paused'} class:live-status-unavailable={liveDataStatus === 'unavailable'} class="live-pill live-status-pill" title={liveDataStatusTooltip} aria-label={liveDataStatusTooltip} aria-live="polite"><b></b> {liveDataStatusText}</span>{:else}<span class="live-pill"><b></b> Live</span>{/if}{/if}</div>
          {#if pageContextCopy}<p>{pageContextCopy}</p>{/if}
        </div>
      </div>

      {#if activeView === 'Settings'}
        <section class="settings-theme-panel panel"><div><p class="eyebrow">Appearance</p><h2>{theme === 'dark' ? 'Dark mode' : 'Light mode'}</h2><p>{theme === 'dark' ? 'A low-light palette for long operational sessions.' : 'A bright daylight palette for quick scanning.'}</p></div><button class="secondary settings-theme-button" on:click={toggleTheme}>{#if theme === 'dark'}<Sun size={15} /> Switch to light{:else}<Moon size={15} /> Switch to dark{/if}</button></section>
        <section class="settings-theme-panel settings-update-panel panel" aria-live="polite">
          <div>
            <p class="eyebrow">Updates</p>
            <h2>{updateState === 'available' && pendingUpdate ? `Kuberniva ${pendingUpdate.version} is available` : updateState === 'downloading' ? 'Downloading update…' : updateState === 'ready' ? 'Update installed' : `Kuberniva ${appVersion || ''}`.trim()}</h2>
            <p class="settings-update-copy">{#if updateState === 'available'}You have {appVersion || 'an older version'}. The update downloads in the background and applies when Kuberniva restarts.{:else if updateState === 'ready'}Restart Kuberniva to finish updating.{:else if updateState === 'current'}You're on the latest version. Checked at {updateCheckedAt}.{:else if updateState === 'error'}{updateError}{:else if updateState === 'checking'}Checking GitHub for a newer version…{:else if updateState === 'downloading'}Keep Kuberniva open until the download finishes.{:else}Kuberniva checks for updates each time it starts.{/if}</p>
            {#if updateState === 'available' && pendingUpdate?.body}<details class="update-notes"><summary>What's new</summary><pre>{pendingUpdate.body}</pre></details>{/if}
            {#if updateState === 'downloading'}<div class="update-progress" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={updateProgress === null ? undefined : Math.round(updateProgress * 100)}><i class:update-progress-indeterminate={updateProgress === null} style:width={updateProgress === null ? '35%' : `${Math.round(updateProgress * 100)}%`}></i></div>{/if}
          </div>
          {#if updateState === 'available'}<button class="primary" on:click={installUpdate}><Download size={15} /> Download and install</button>
          {:else if updateState === 'ready'}<button class="primary" on:click={restartToUpdate}><RefreshCw size={15} /> Restart now</button>
          {:else if updateState === 'downloading'}<button class="secondary" disabled>{updateProgress === null ? 'Downloading…' : `${Math.round(updateProgress * 100)}%`}</button>
          {:else}<button class="secondary settings-theme-button" disabled={updateState === 'checking'} on:click={() => checkForUpdates()}><RefreshCw size={15} class={updateState === 'checking' ? 'animate-spin' : ''} /> {updateState === 'checking' ? 'Checking…' : 'Check for updates'}</button>{/if}
        </section>
        <section class="settings-panel panel"><div class="panel-heading"><div><h2>Workspace settings</h2><p>Connection and display preferences for this device.</p></div></div><div class="settings-row"><div><strong>Kubeconfig sources</strong><small>{kubeconfigSources.length ? `${kubeconfigSources.length} tracked source${kubeconfigSources.length === 1 ? '' : 's'} · newest: ${kubeconfigPath || 'Default: ~/.kube/config'}` : 'No kubeconfig source added'}</small></div><button class="secondary" on:click={() => (kubeconfigOpen = true)}>+ Add source</button></div><div class="settings-row"><div><strong>Manual source sync</strong><small>Startup uses the local context snapshot. Sync only when you want Kuberniva to rescan saved files and folders.</small></div><button class="secondary" disabled={loadingCatalog || !kubeconfigSources.length} on:click={syncKubeconfigSources}>{loadingCatalog ? 'Syncing…' : 'Sync sources'}</button></div><div class="settings-row"><div><strong>Loaded contexts</strong><small>{clusters.length ? `${clusters.length} available locally; OIDC is requested only when one is selected` : 'No kubeconfig context is currently available'}</small></div><button class="secondary" on:click={() => activeClusterId ? refreshActiveCluster() : (kubeconfigOpen = true)}>{activeClusterId ? 'Refresh current' : 'Add source'}</button></div></section>
      {:else if restoringWorkspace}
        <section class="overview-loading"><i></i><div><h2>Restoring your workspace…</h2><p>Reading your saved kubeconfig source locally. Kuberniva will not connect to a cluster or start OIDC until you select one.</p></div></section>
      {:else if !connectedKubeconfig}
        <section class="empty-view connect-empty"><div class="explore-orbit"><i></i><i></i><b>⌁</b></div><h2>{kubeconfigSources.length ? 'No cached contexts yet' : 'Your workspace is empty'}</h2><p>{kubeconfigSources.length ? 'Your saved sources are not scanned at startup. Sync them only when you want to discover their current contexts.' : 'Add a kubeconfig to discover the contexts and API resources you can access.'}</p><button class="primary" on:click={() => kubeconfigSources.length ? syncKubeconfigSources() : (kubeconfigOpen = true)}>{kubeconfigSources.length ? 'Sync saved sources' : '+ Add kubeconfig'}</button></section>
      {:else if activeView === 'Clusters'}
        <section class="clusters-landing">
          <div class="clusters-landing-heading"><div><p class="eyebrow">Cluster manager</p><h2>{clusters.length} available cluster{clusters.length === 1 ? '' : 's'}</h2></div><button class="secondary" on:click={() => (kubeconfigOpen = true)}>+ Add kubeconfig</button></div>
          {#if clusters.length}
            <div class="cluster-list" role="list" aria-label="Tracked clusters">
              <div class="cluster-list-header" aria-hidden="true"><span>Cluster</span><span>Authentication</span><span>Source</span><span>Status</span><span>Favorite</span><span></span></div>
              {#each clusters as cluster}
                <article class:cluster-list-active={cluster.id === activeClusterId} class="cluster-list-row" role="listitem">
                  <button class="cluster-list-open" on:click={() => selectCluster(cluster.id)} title={`Open ${cluster.name}`}><span class="cluster-list-mark"><i class="status-dot {cluster.tone}"></i></span><span class="cluster-list-name"><strong>{cluster.name}</strong><small>{cluster.provider}</small></span><span class="cluster-list-arrow">Open overview →</span></button>
                  <span class="cluster-list-auth">{cluster.authMethod || 'Credentials unavailable'}</span>
                  <small class="cluster-list-source" title={cluster.kubeconfigPath || ''}>{cluster.kubeconfigPath || 'Source path unavailable'}</small>
                  <span class="cluster-list-status"><i class="status-dot {cluster.tone}"></i>{cluster.status}</span>
                  <button class:favorite-toggle-active={isFavoriteCluster(cluster.id)} class="favorite-toggle" aria-pressed={isFavoriteCluster(cluster.id)} aria-label={`${isFavoriteCluster(cluster.id) ? 'Remove' : 'Add'} ${cluster.name} ${isFavoriteCluster(cluster.id) ? 'from' : 'to'} Favorites`} title={isFavoriteCluster(cluster.id) ? 'Remove from Favorites' : 'Add to Favorites'} on:click|stopPropagation={() => toggleFavoriteCluster(cluster)}><Star size={16} fill={isFavoriteCluster(cluster.id) ? 'currentColor' : 'none'} /></button>
                  <button class="remove-cluster-list" title={`Remove ${cluster.name} from Kuberniva only`} on:click={() => removeCluster(cluster.id)}>Remove</button>
                </article>
              {/each}
            </div>
          {:else}
            <div class="cluster-management-empty"><h3>No clusters are currently tracked</h3><p>Connect a kubeconfig source to discover contexts again.</p><button class="primary" on:click={() => (kubeconfigOpen = true)}>Choose kubeconfig source</button></div>
          {/if}
        </section>
      {:else if activeView === 'Favorites'}
        <section class="favorites-page">
          <div class="favorites-page-heading">
            <div><p class="eyebrow">Pinned shortcuts</p><h2>Favorite clusters</h2><p>Keep up to 10 frequently used clusters one click away. Favorites are stored locally on this device.</p></div>
            <span class="favorites-limit"><Star size={16} fill="currentColor" /> {favoriteClusters.length}/10</span>
          </div>
          {#if favoriteClusters.length}
            <div class="favorites-list" role="list" aria-label="Favorite clusters">
              {#each favoriteClusters as cluster}
                <article class:favorite-card-active={cluster.id === activeClusterId} class="favorite-card" role="listitem">
                  <button class="favorite-card-open" title={`Open ${favoriteLabel(cluster)} · right-click to rename`} on:click={() => { favoriteContextMenu = null; void selectCluster(cluster.id); }} on:contextmenu={(event) => openFavoriteContextMenu(event, cluster)}>
                    <span class="favorite-card-mark"><i class="status-dot {cluster.tone}"></i></span>
                    <span class="favorite-card-copy"><strong>{favoriteLabel(cluster)}</strong><small>{cluster.name} · {cluster.provider} · {cluster.authMethod || 'Credentials unavailable'}</small><em>{cluster.status}</em></span>
                    <span class="favorite-card-arrow">Open overview →</span>
                  </button>
                  <button class="favorite-card-remove" aria-label={`Remove ${cluster.name} from Favorites`} title="Remove shortcut" on:click={() => toggleFavoriteCluster(cluster)}><Star size={17} fill="currentColor" /></button>
                </article>
              {/each}
            </div>
          {:else}
            <section class="empty-view favorites-empty"><div class="explore-orbit"><Star size={28} /></div><h2>No favorite clusters yet</h2><p>Open Cluster manager and use the star in a cluster row to create a shortcut here.</p><button class="primary" on:click={() => navigateTo('Clusters')}>Open Cluster manager</button></section>
          {/if}
        </section>
      {:else if activeView === 'Events'}
        <section class="events-page panel">
          <div class="events-heading">
            <div><p class="eyebrow">Cluster activity</p><h2>Events</h2><p>{namespace === 'all namespaces' ? `Recent Kubernetes events across ${activeCluster}.` : `Recent events for ${namespace}, including cluster-scoped activity.`} Warnings stay visible until the API server expires them.</p></div>
            <div class="events-actions"><small>{eventsObservedAt ? `Updated ${formatObservedTime(eventsObservedAt)}` : 'Not loaded yet'}</small></div>
          </div>
          {#if !activeClusterId}
            <div class="events-empty"><ScrollText size={28} /><h3>Select a cluster first</h3><p>Events are fetched only after a live cluster is selected.</p></div>
          {:else if loadingEvents && !clusterEvents.length}
            <div class="events-loading"><i></i><div><h3>Reading cluster events…</h3><p>Fetching the latest Kubernetes Event objects.</p></div></div>
          {:else if eventsError}
            <div class="events-error"><ScrollText size={24} /><div><h3>Events could not be loaded</h3><p>{eventsError}</p></div><button class="secondary" on:click={() => loadClusterEvents(true)}>Try again</button></div>
          {:else}
            <div class="events-toolbar">
              <label class="events-search"><Search size={15} /><input bind:value={eventSearch} placeholder="Filter events, reasons, objects…" aria-label="Filter cluster events" /></label>
              <div class="events-filter" role="group" aria-label="Event severity"><button class:active={eventTypeFilter === 'All'} on:click={() => (eventTypeFilter = 'All')}>All <b>{namespaceClusterEvents.length}</b></button><button class:active={eventTypeFilter === 'Warning'} on:click={() => (eventTypeFilter = 'Warning')}>Warnings <b>{namespaceClusterEvents.filter((event) => event.eventType === 'Warning').length}</b></button><button class:active={eventTypeFilter === 'Normal'} on:click={() => (eventTypeFilter = 'Normal')}>Normal <b>{namespaceClusterEvents.filter((event) => event.eventType !== 'Warning').length}</b></button></div>
            </div>
            {#if visibleClusterEvents.length}
              <div class="events-list" role="list" aria-label="Cluster events">
                {#each visibleClusterEvents as event}
                  <article class:event-warning={eventTone(event.eventType) === 'warning'} class="event-card" role="listitem">
                    <span class="event-severity">{event.eventType === 'Warning' ? '!' : '·'}</span>
                    <div class="event-card-main"><div class="event-card-title"><strong>{event.reason || event.eventType}</strong><span>{event.involvedKind || 'Cluster'}{event.involvedName ? ` · ${event.involvedName}` : ''}</span></div><p>{event.message || 'No event message was supplied.'}</p><div class="event-card-meta"><span>{event.namespace || 'cluster scope'}</span>{#if event.source}<span>{event.source}</span>{/if}{#if event.action}<span>{event.action}</span>{/if}{#if event.count && event.count > 1}<span>×{event.count}</span>{/if}</div></div>
                    <time datetime={event.lastObserved || event.firstObserved || ''}>{formatObservedTime(event.lastObserved || event.firstObserved)}</time>
                  </article>
                {/each}
              </div>
            {:else}
              <div class="events-empty"><ScrollText size={28} /><h3>{namespaceClusterEvents.length ? 'No matching events' : 'No recent events'}</h3><p>{namespaceClusterEvents.length ? 'Try a different filter or severity.' : namespace === 'all namespaces' ? 'This cluster has not returned any retained Kubernetes events.' : `No retained events were returned for ${namespace}.`}</p></div>
            {/if}
          {/if}
        </section>
      {:else if activeView === 'Overview'}
        {#if !activeClusterId}
          <section class="empty-view"><div class="explore-orbit"><i></i><i></i><b>⌁</b></div><h2>Select a cluster</h2><p>{clusters.length} kubeconfig context{clusters.length === 1 ? '' : 's'} loaded locally. Click one in the sidebar when you are ready to authenticate and connect.</p></section>
        {:else if catalogError}
          <section class="empty-view"><div class="explore-orbit"><i></i><i></i><b>!</b></div><h2>{activeCluster} needs attention</h2><p>{catalogError}</p><button class="primary" on:click={refreshActiveCluster}>Try connecting again</button></section>
        {:else if loadingOverview && !clusterOverview}
          <section class="overview-loading"><i></i><div><h2>Reading cluster signals…</h2><p>Loading Nodes and current metrics only for {activeCluster}.</p></div></section>
        {:else if overviewError}
          <section class="overview-error panel"><div><span>!</span><div><h2>Node overview is unavailable</h2><p>{overviewError}</p></div></div><button class="secondary" disabled={loadingOverview} on:click={() => loadClusterOverview(true)}>Try again</button></section>
        {:else if clusterOverview}
          <section class="cluster-overview-dashboard">
          <section class="cluster-hero">
            <div class="cluster-hero-mark"><img class="brand-mark" src="/kuberniva-mark.svg" alt="" /></div>
            <div class="cluster-hero-copy"><p class="eyebrow">Cluster overview</p><h2>{activeCluster}</h2></div>
            <div class="overview-summary"><div><strong>{clusterOverview.nodes.length}</strong><span>Nodes</span></div><div><strong>{readyNodeCount}</strong><span>Ready</span></div><div><strong>{clusterOverview.metricsAvailable ? `${clusterOverview.totals.metricNodes}/${clusterOverview.nodes.length}` : '—'}</strong><span>Metrics nodes</span></div></div>
          </section>
          <section class="node-overview panel">
            <div class="panel-heading"><div><p class="eyebrow">Infrastructure</p><h2>Nodes</h2></div><div class="overview-actions"><small>Updated {new Date(clusterOverview.observedAt).toLocaleTimeString()}</small></div></div>
            {#if clusterOverview.nodes.length === 0}
              <div class="node-empty">No Nodes were returned by this cluster.</div>
            {:else}
              <div class="node-workbench">
                <aside class="node-list" aria-label="Cluster nodes">
                  {#each clusterOverview.nodes as node}
                    <button class:node-list-active={node.name === selectedNode?.name} on:click={() => { selectedNodeName = node.name; nodeDetailTab = 'Overview'; }}>
                      <span class="node-list-mark"><i class:ready={node.ready}></i></span>
                      <span><strong>{node.name}</strong><small>{node.roles.length ? node.roles.join(' · ') : 'Worker node'} · {node.architecture || 'architecture unavailable'}</small></span>
                      <b>›</b>
                    </button>
                  {/each}
                </aside>
                {#if selectedNode}
                  <div class="node-inspector">
                    <div class="node-inspector-heading"><div><p class="eyebrow">Node details</p><h3>{selectedNode.name}</h3><p>{selectedNode.ready ? 'Ready and accepting workloads' : 'Not ready · scheduling may be affected'}</p></div><span class:node-inspector-not-ready={!selectedNode.ready} class="node-inspector-status">{selectedNode.ready ? 'Ready' : 'Not ready'}</span></div>
                    <div class="node-detail-tabs" role="tablist" aria-label="Node detail sections">
                      {#each nodeDetailTabs as tab}
                        <button class:node-detail-tab-active={nodeDetailTab === tab} role="tab" aria-selected={nodeDetailTab === tab} on:click={() => (nodeDetailTab = tab)}>{tab}</button>
                      {/each}
                    </div>
                    <div class="node-detail-tab-panel" role="tabpanel">
                      {#if nodeDetailTab === 'Overview'}
                        <div class="node-detail-grid">
                          <div><span>Architecture</span><strong>{selectedNode.architecture || '—'}</strong></div><div><span>OS image</span><strong>{selectedNode.osImage || '—'}</strong></div><div><span>Kubelet</span><strong>{selectedNode.kubeletVersion || '—'}</strong></div><div><span>Runtime</span><strong>{selectedNode.containerRuntimeVersion || '—'}</strong></div>
                        </div>
                        <div class="node-metrics-grid">
                          <section class="node-metric-card node-metric-cpu"><div class="node-metric-heading"><span>CPU usage</span><strong>{cpuMetricLabel(selectedNode.cpuUsage)}</strong><b>{usagePercentLabel(selectedNode.cpuUsagePercent)}</b></div>{#if selectedNode.cpuUsagePercent !== undefined}<div class="usage-meter" aria-label={`CPU ${usagePercentLabel(selectedNode.cpuUsagePercent)}`}><i style:width={`${selectedNode.cpuUsagePercent}%`}></i></div>{/if}<small>{selectedNode.cpuUsage ? `${cpuMetricLabel(selectedNode.cpuUsage)} used of ${cpuMetricLabel(selectedNode.cpuCapacity)} · ${remainingPercentLabel(selectedNode.cpuUsagePercent)}` : `Capacity ${cpuMetricLabel(selectedNode.cpuCapacity)}`}</small></section>
                          <section class="node-metric-card node-metric-memory"><div class="node-metric-heading"><span>Memory usage</span><strong>{selectedNode.memoryUsage || '—'}</strong><b>{usagePercentLabel(selectedNode.memoryUsagePercent)}</b></div>{#if selectedNode.memoryUsagePercent !== undefined}<div class="usage-meter memory-meter" aria-label={`Memory ${usagePercentLabel(selectedNode.memoryUsagePercent)}`}><i style:width={`${selectedNode.memoryUsagePercent}%`}></i></div>{/if}<small>{selectedNode.memoryUsage ? `${selectedNode.memoryUsage} used of ${selectedNode.memoryCapacity || 'unknown capacity'} · ${remainingPercentLabel(selectedNode.memoryUsagePercent)}` : `Capacity ${selectedNode.memoryCapacity || 'unavailable'}`}</small></section>
                        </div>
                      {:else if nodeDetailTab === 'Allocation'}
                        <section class="node-detail-section node-detail-section-expanded"><div class="node-detail-section-heading"><strong>Capacity & allocation</strong><small>{selectedNode.unschedulable ? 'Cordoned' : 'Schedulable'}</small></div><div class="node-property-list">{#each selectedNode.capacity as property}<div><span>{property.key}</span><strong>{property.key === 'cpu' ? cpuMetricLabel(property.value) : property.value}</strong><small>allocatable {property.key === 'cpu' ? cpuMetricLabel(selectedNode.allocatable.find((candidate) => candidate.key === property.key)?.value) : selectedNode.allocatable.find((candidate) => candidate.key === property.key)?.value || '—'}</small></div>{/each}</div></section>
                      {:else if nodeDetailTab === 'Network'}
                        <section class="node-detail-section node-detail-section-expanded"><div class="node-detail-section-heading"><strong>Network & identity</strong><small>{selectedNode.podCidrs.length ? selectedNode.podCidrs.join(' · ') : 'Pod CIDR unavailable'}</small></div><div class="node-property-list">{#each selectedNode.addresses as address}<div><span>{address.type}</span><strong>{address.address}</strong></div>{/each}{#if selectedNode.providerId}<div><span>Provider</span><strong>{selectedNode.providerId}</strong></div>{/if}{#if selectedNode.uid}<div><span>UID</span><strong>{selectedNode.uid}</strong></div>{/if}</div></section>
                      {:else if nodeDetailTab === 'Health'}
                        <div class="node-detail-columns node-health-grid"><section class="node-detail-section"><div class="node-detail-section-heading"><strong>Conditions</strong><small>{selectedNode.conditions.length}</small></div><div class="node-condition-list">{#each selectedNode.conditions as condition}<div><span class:condition-false={condition.status !== 'True'}>{condition.status === 'True' ? '●' : '○'}</span><strong>{condition.type}</strong><small>{condition.reason || condition.message || condition.status}</small></div>{/each}</div></section><section class="node-detail-section"><div class="node-detail-section-heading"><strong>Taints</strong><small>{selectedNode.taints.length}</small></div>{#if selectedNode.taints.length}<div class="node-condition-list">{#each selectedNode.taints as taint}<div><span class="condition-false">!</span><strong>{taint.key}</strong><small>{taint.value ? `${taint.value} · ` : ''}{taint.effect}</small></div>{/each}</div>{:else}<p class="node-detail-empty">No taints are currently applied.</p>{/if}</section></div>
                      {:else}
                        <div class="node-metadata-panel"><section><div class="node-detail-section-heading"><strong>Labels</strong><small>{selectedNode.labels.length}</small></div>{#if selectedNode.labels.length}<div class="node-metadata-grid">{#each selectedNode.labels as property}<span><b>{property.key}</b><em>{property.value}</em></span>{/each}</div>{:else}<p class="node-detail-empty">No labels were returned.</p>{/if}</section><section><div class="node-detail-section-heading"><strong>Annotations</strong><small>{selectedNode.annotations.length}</small></div>{#if selectedNode.annotations.length}<div class="node-metadata-grid">{#each selectedNode.annotations as property}<span><b>{property.key}</b><em>{property.value}</em></span>{/each}</div>{:else}<p class="node-detail-empty">No annotations were returned.</p>{/if}</section></div>
                      {/if}
                    </div>
                  </div>
                {/if}
              </div>
            {/if}
          </section>
          <section class="overview-quick-actions"><button class="overview-workloads-tile" on:click={() => navigateTo('Workloads')}><span>▦</span><strong>Workloads</strong><b>→</b></button><button class="overview-resources-tile" on:click={() => openResourceWorkspace(false)}><span>▤</span><strong>Resources</strong><b>→</b></button></section>
          </section>
        {:else}
          <section class="overview-loading"><i></i><div><h2>Preparing cluster overview…</h2><p>Waiting for the first live node response.</p></div></section>
        {/if}
      {:else if activeView === 'Resources'}
        <section class="resource-workbench panel">
          {#if !activeClusterId}
            <div class="connection-error"><strong>Select a cluster to begin</strong><p>Kuberniva has only read your local kubeconfig metadata. No cluster connection or OIDC login has been started.</p></div>
          {:else if catalogError}
            <div class="connection-error"><strong>Unable to connect to {activeCluster}</strong><p>{catalogError}</p><button class="secondary" on:click={refreshActiveCluster}>Try again</button></div>
          {:else if loadingCatalog}
            <div class="connection-error"><strong>Connecting to {activeCluster}…</strong><p>Reading the live API catalog and namespaces.</p></div>
          {:else}
            {#if !selectedResource}
              <div class="resource-directory">
                <header class="resource-directory-header">
                  <div><p class="eyebrow">{customApiWorkspace ? 'Custom APIs' : 'Resources'}</p><h2>{customApiWorkspace ? 'Browse custom APIs' : 'Browse resources'}</h2><p>{activeResourceCatalog.length} API types in {activeCluster}. Pick one to load its objects.</p></div>
                  <label class="resource-directory-search"><Search size={15} /><input bind:value={resourceDirectorySearch} placeholder="Find a kind, group, or version" aria-label="Find a resource type" spellcheck="false" /></label>
                </header>
                {#if recentDirectoryResources.length}
                  <section class="resource-directory-section"><h3>Recently opened</h3><div class="resource-directory-grid">{#each recentDirectoryResources as resource}<button type="button" class="resource-directory-card resource-directory-card-recent" on:click={() => openDirectoryResource(resource)}><strong>{kindLabel(resource)}</strong><small>{resource.apiVersion}</small><span>{resource.namespaced ? 'Namespaced' : 'Cluster-wide'}</span></button>{/each}</div></section>
                {/if}
                {#each resourceDirectorySections as section}
                  <section class="resource-directory-section"><h3>{section.title}<b>{section.resources.length}</b></h3><div class="resource-directory-grid">{#each section.resources as resource}<button type="button" class="resource-directory-card" on:click={() => openDirectoryResource(resource)}><strong>{kindLabel(resource)}</strong><small>{resource.apiVersion}</small><span>{resource.namespaced ? 'Namespaced' : 'Cluster-wide'}</span></button>{/each}</div></section>
                {:else}
                  <div class="resource-object-empty"><span>⌕</span><strong>{resourceDirectorySearch.trim() ? 'No matching API types' : 'No API types available'}</strong><p>{resourceDirectorySearch.trim() ? 'Try a different kind, group, or version.' : 'Your identity cannot list any resource types in this scope.'}</p></div>
                {/each}
              </div>
            {:else}
            <div class:resource-workbench-inspecting={Boolean((editorObject || loadingEditor) && !configModalOpen)} class="resource-workbench-body resource-workbench-body-focused">
              <aside class="resource-object-browser" aria-label="Resource objects">
                {#if selectedResource}
                  <div class="resource-object-heading resource-pane-heading"><button type="button" class="resource-directory-back" aria-label="Back to all resource types" title="All resource types" on:click={showResourceDirectory}>←</button><div><span class:custom={selectedResource.crd} class="resource-pane-icon">{selectedResource.crd ? '◇' : '○'}</span><div><strong>{selectedResource.kind} objects</strong><small>{selectedResource.apiVersion} · {selectedResource.namespaced ? (namespace === 'all namespaces' ? 'All namespaces' : namespace) : 'Cluster-wide'}</small></div></div><span class:live-status-loading={liveDataStatus === 'loading'} class:live-status-loaded={liveDataStatus === 'loaded'} class:live-status-stale={liveDataStatus === 'stale'} class:live-status-paused={liveDataStatus === 'paused'} class:live-status-unavailable={liveDataStatus === 'unavailable'} class="live-list-status resource-live-status" title={liveDataStatusTooltip} aria-label={liveDataStatusTooltip} aria-live="polite"><i></i></span><b>{resourceObjects.length}</b></div>
                  <div class="resource-object-columns" role="row" aria-label="Select loaded resource objects">{#if selectedResourcePermissionSet.canDelete}<label class:resource-select-all-partial={resourceObjectsSelectionPartial} class="resource-select-all"><input type="checkbox" checked={allResourceObjectsSelected} disabled={deletingResource || loadingObjects || !resourceObjects.length} aria-checked={resourceObjectsSelectionPartial ? 'mixed' : allResourceObjectsSelected ? 'true' : 'false'} aria-label={`Select all loaded ${selectedResource.plural}`} on:change={toggleAllResourceObjects} /></label>{:else}<span class="resource-permission-spacer"></span>{/if}<span>Name</span><span>Namespace</span><span>Age</span><span>Action</span></div>
                  {#if selectedResourceObjects.length && selectedResourcePermissionSet.canDelete}<div class="resource-bulk-toolbar" role="region" aria-label="Bulk resource actions"><span><strong>{selectedResourceObjects.length}</strong> selected</span><button class="destructive" type="button" disabled={deletingResource || loadingEditor || savingEditor || loadingYaml || savingYaml} on:click={() => requestBulkResourceDeletion(selectedResource!)}>Delete {selectedResourceObjects.length}</button><button class="resource-bulk-clear" type="button" disabled={deletingResource} on:click={clearResourceObjectSelection}>Clear</button></div>{/if}
                  {#if loadingObjects}<div class="drawer-state"><i></i>Listing {selectedResource.plural}…</div>{:else if resourceObjects.length === 0}<div class="resource-object-empty"><span>○</span><strong>No {selectedResource.plural} found</strong><p>Try another namespace or use Refresh in the top bar.</p></div>{:else}<div class="object-list">{#each resourceObjects as object}<div class:resource-object-row-selected={isResourceObjectSelected(object, selectedResourceObjectKeys)} class="resource-object-row">{#if selectedResourcePermissionSet.canDelete}<label class="resource-object-select"><input type="checkbox" checked={isResourceObjectSelected(object, selectedResourceObjectKeys)} disabled={deletingResource} aria-label={`Select ${selectedResource.kind} ${object.name}`} on:change={() => toggleResourceObjectSelection(object)} /></label>{:else}<span class="resource-permission-spacer"></span>{/if}<button type="button" disabled={!selectedResourceCanOpen} aria-busy={selectedResource.kind === 'Pod' && isOpeningLogs('Pod', object)} class:object-selected={editorObject?.name === object.name && editorObject?.namespace === object.namespace} on:click={() => openObject(selectedResource!, object)}><div class="resource-object-primary"><strong>{object.name}</strong></div><span class="resource-object-namespace">{object.namespace || 'cluster scoped'}</span><small class="resource-object-age">{object.createdAt ? resourceAge(object.createdAt) : '—'}</small><span class="resource-object-action">{selectedResourceCanOpen ? (selectedResource.kind === 'Pod' ? (isOpeningLogs('Pod', object) ? 'Opening…' : 'Logs →') : 'Open →') : ''}</span></button></div>{/each}</div>{/if}
                {:else}
                  <div class="resource-object-empty"><span>⌘</span><strong>Select a resource kind</strong><p>Choose a kind from the left. Kuberniva loads only that API.</p></div>
                {/if}
              </aside>
              <div class="resource-pane-resizer" role="separator" aria-orientation="vertical" aria-label="Resize resource object list" on:pointerdown={startResourceObjectPaneResize}></div>
              {#if configModalOpen}<div class="config-modal-backdrop" role="presentation" on:click={requestCloseConfigEditor}></div>{/if}
              <aside class:resource-inspector-modal={configModalOpen} class="resource-inspector" aria-label="Resource details" role={configModalOpen ? 'dialog' : undefined} aria-modal={configModalOpen ? 'true' : undefined}>
                <div class="resource-inspector-surface">
                {#if configModalOpen && editorResource && editorObject}<header class="config-modal-heading"><p class="eyebrow">{editorResource.kind}</p><h2 title={editorObject.name}>{editorObject.name}</h2><small>{editorObject.namespace || 'cluster scoped'} · {activeCluster}{#if configEditorDirty} · <b>unsaved changes</b>{/if}</small></header>{/if}
                {#if !(editorResource && (editorResource.kind === 'Secret' || editorResource.kind === 'ConfigMap'))}<div class="resource-details-heading resource-pane-heading"><span>02</span><div><strong>Details</strong><small>Live properties and actions</small></div></div>{/if}
                {#if editorResource && editorObject}
                  {#if editorResource.kind !== 'Secret' && editorResource.kind !== 'ConfigMap'}<div class="drawer-heading inspector-heading"><div><span class:custom={editorResource.custom}>⌁</span><div><h2>{editorObject.name}</h2><p>{editorResource.kind} · {editorObject.namespace || 'cluster scoped'}</p></div></div><div class="inspector-heading-actions">{#if editorPermissionSet.canGet}<button class="secondary" disabled={loadingEditor} on:click={() => openYamlEditor(editorResource!, editorObject!)}>YAML</button>{/if}<button aria-label="Back to resource objects" on:click={() => closeEditor()}>×</button></div></div>{/if}
                  {#if loadingEditor}
                    <div class="drawer-state"><i></i>Loading live resource details…</div>
                  {:else}
                    {#if editorCertificate}
                      <section class:expired={editorCertificate.expired} class="certificate-card"><div><span>⌁</span><div><strong>{editorCertificate.expired ? 'Certificate expired' : 'TLS certificate'}</strong><p>Expires {editorCertificate.expiresAt}</p></div></div><b>{certificateRemainingLabel(editorCertificate)}</b></section>
                    {/if}
                    {#if editorResource.kind === 'Secret' || editorResource.kind === 'ConfigMap'}
                      <section class="configuration-values-editor">
                        <div class="configuration-values-toolbar"><small>{editorEntries.length} {editorEntries.length === 1 ? 'key' : 'keys'}{editorEntrySearch ? ` · ${filteredEditorEntries.length} shown` : ''}{editorResource.kind === 'Secret' ? (revealSecret ? ' · decoded' : ' · base64') : ''}</small><div>{#if editorEntries.length > 8}<label class="configuration-key-search"><Search size={12} /><input bind:value={editorEntrySearch} placeholder="Filter keys" aria-label="Filter configuration keys" spellcheck="false" />{#if editorEntrySearch}<button type="button" aria-label="Clear key filter" on:click={() => (editorEntrySearch = '')}>×</button>{/if}</label>{/if}{#if editorResource.kind === 'Secret'}<button class="reveal-button" on:click={() => (revealSecret = !revealSecret)}>{revealSecret ? 'Hide decoded' : 'Reveal decoded'}</button>{/if}{#if editorPermissionSet.canUpdate}<button class="configuration-add-key" type="button" on:click={addEditorEntry}>＋ Add key</button>{/if}<button class="configuration-toolbar-button" type="button" disabled={loadingEditor} on:click={() => openYamlEditor(editorResource!, editorObject!)}>YAML</button><button class="configuration-toolbar-close" type="button" aria-label="Back to resource objects" on:click={requestCloseConfigEditor}>×</button></div></div>
                        {#if editorEntries.length === 0}
                          <div class="configuration-values-empty">No values yet.{#if editorPermissionSet.canUpdate}<button type="button" on:click={addEditorEntry}>Add the first key</button>{/if}</div>
                        {:else}
                          <div class:configuration-preview-dense={editorEntries.length > 8} class="configuration-preview-list">{#each filteredEditorEntries as { entry, index }}{@const format = editorEntryFormat(entry, revealSecret)}<article class:configuration-preview-open={expandedEditorEntryIndex === index} class="configuration-preview-card"><button class="configuration-preview-toggle" type="button" aria-expanded={expandedEditorEntryIndex === index} on:click={() => toggleEditorEntry(index)}><span class={`configuration-preview-type configuration-preview-type-${format.tone}`}>{format.short}</span><span><strong>{entry.key || `Entry ${index + 1}`}</strong><small>{format.label}</small></span><code>{editorEntryPreview(entry, revealSecret)}</code><b>{expandedEditorEntryIndex === index ? '⌃' : '⌄'}</b></button>{#if expandedEditorEntryIndex === index}{#if editorPermissionSet.canUpdate}<div class="configuration-preview-editor"><label><span>Key</span><input value={entry.key} on:input={(event) => updateEditorEntryKey(index, event.currentTarget.value)} spellcheck="false" /></label><label><span>Value</span><textarea use:autoSizeTextarea={editorEntryDisplayValue(entry, revealSecret)} value={editorEntryDisplayValue(entry, revealSecret)} on:input={(event) => updateEditorEntryValue(index, event.currentTarget.value)} spellcheck="false"></textarea></label><button class="configuration-preview-remove" type="button" on:click={() => removeEditorEntry(index)}>Remove key</button></div>{:else}<div class="configuration-preview-readonly"><pre>{editorEntryDisplayValue(entry, revealSecret)}</pre></div>{/if}{/if}</article>{:else}<div class="configuration-filter-empty"><Search size={16} /><span>No keys match “{editorEntrySearch}”</span></div>{/each}</div>
                        {/if}
                      </section>
                    {:else}
                      <div class="inspector-overview">
                        <section class="inspector-summary-grid"><div><span>Kind</span><strong>{editorResource.kind}</strong></div><div><span>Scope</span><strong>{editorObject.namespace || 'Cluster-wide'}</strong></div><div><span>API</span><strong>{editorResource.apiVersion}</strong></div></section>
                        {#if editorResource.category === 'Network' || editorResource.category === 'Gateway APIs'}
                          {#if editorResource.kind === 'Service'}
                            <section class="service-overview-card">
                              <div class="service-overview-heading"><div><span class="service-overview-icon">⇄</span><div><strong>Service routing</strong><small>Exposure, address, and traffic policy</small></div></div><b class="service-type-badge">{networkServiceType(editorManifest)}</b></div>
                              <div class="service-fact-grid"><div class="service-fact service-fact-type"><span>Service type</span><strong>{networkServiceType(editorManifest)}</strong><small>{networkServiceExposure(editorManifest)}</small></div><div class="service-fact"><span>Cluster IP</span><strong>{networkServiceClusterIp(editorManifest)}</strong><small>{networkPorts(editorManifest).length} declared {networkPorts(editorManifest).length === 1 ? 'port' : 'ports'}</small></div><div class="service-fact"><span>Traffic policy</span><strong>{networkServiceTrafficPolicy(editorManifest)}</strong><small>How traffic is routed</small></div></div>
                              {#if networkServiceExternalEndpoints(editorManifest).length}
                                <div class="service-external-card service-external-card-active"><span>↗</span><div><strong>External endpoint{networkServiceExternalEndpoints(editorManifest).length === 1 ? '' : 's'}</strong><small>Reachable outside the cluster</small><div class="service-endpoint-list">{#each networkServiceExternalEndpoints(editorManifest) as endpoint}<code>{endpoint}</code>{/each}</div></div></div>
                              {:else if networkServiceType(editorManifest) === 'LoadBalancer'}
                                <div class="service-external-card service-external-card-pending"><span>…</span><div><strong>External address pending</strong><small>The LoadBalancer has not received an IP or hostname yet.</small></div></div>
                              {:else}
                                <div class="service-external-card"><span>●</span><div><strong>No external endpoint</strong><small>This service is currently reachable only through cluster networking.</small></div></div>
                              {/if}
                            </section>
                          {/if}
                          <section class="network-inspector-card"><div><strong>Hosts &amp; addresses</strong><small>Resolved from service status, endpoint subsets, and Gateway route hosts</small></div>{#if networkAddressFacts(editorManifest).length}<div class="network-fact-list">{#each networkAddressFacts(editorManifest) as fact}<div class={`network-fact network-fact-${fact.tone}`}><span>{fact.label}</span><strong>{fact.value}</strong></div>{/each}</div>{:else}<p>No host or address is declared on this resource.</p>{/if}</section>
                          <section class="network-inspector-card"><div><strong>Ports &amp; listeners</strong><small>Service ports, endpoint ports, route backends, and Gateway listeners</small></div>{#if networkPortFacts(editorManifest).length}<div class="network-fact-list">{#each networkPortFacts(editorManifest) as fact}<div class={`network-fact network-fact-${fact.tone}`}><span>{fact.label}</span><strong>{fact.value}</strong></div>{/each}</div>{:else}<p>No ports or listeners are declared on this resource.</p>{/if}</section>
                        {/if}
                        {#if resourceLabels(editorManifest).length}<section class="resource-labels"><strong>Labels</strong><div class="inspector-chip-list">{#each resourceLabels(editorManifest) as [key, value]}<span><b>{key}</b>{value}</span>{/each}</div></section>{/if}
                        <details class="resource-properties" open><summary>Resource properties</summary><pre>{genericResourcePreview(editorManifest)}</pre></details>
                      </div>
                    {/if}
                  {/if}
                  {#if configDiscardPrompt}<div class="config-discard-bar" role="alertdialog" aria-label="Unsaved changes"><span>Discard unsaved changes to {editorObject.name}?</span><button type="button" class="secondary" on:click={() => (configDiscardPrompt = false)}>Keep editing</button><button type="button" class="destructive" on:click={discardConfigEditor}>Discard</button></div>{/if}
                  <div class="drawer-footer drawer-footer-compact"><div class="editor-footer-actions">{#if editorPermissionSet.resolved && !editorPermissionSet.canUpdate && !editorPermissionSet.canDelete}<span class="permission-readonly-badge">Read only</span>{/if}{#if editorResource.kind !== 'Secret' && editorResource.kind !== 'ConfigMap' && editorPermissionSet.canGet}<button class="secondary" disabled={loadingEditor} on:click={() => openYamlEditor(editorResource!, editorObject!)}>YAML</button>{/if}{#if editorPermissionSet.canDelete}<button class="destructive" disabled={loadingEditor || savingEditor} on:click={() => requestResourceDeletion(editorResource!, editorObject!)}>Delete</button>{/if}{#if (editorResource.kind === 'Secret' || editorResource.kind === 'ConfigMap') && editorPermissionSet.canUpdate}<button class="primary" disabled={loadingEditor || savingEditor} on:click={saveEditor}>{savingEditor ? 'Saving…' : 'Save'}</button>{/if}</div></div>
                {:else}
                  <div class="inspector-empty"><span>{selectedResource?.crd ? '◇' : '⌁'}</span><h3>{selectedResource ? `Choose a ${selectedResource.kind}` : 'Ready when you are'}</h3><p>{selectedResource ? 'Select an object from the list to view its live properties, edit supported data, or open YAML.' : 'Pick an API type to load its objects. Kuberniva does not fan out requests in the background.'}</p></div>
                {/if}
              </div>
            </aside>
            </div>
            {/if}
          {/if}
        </section>
      {:else if activeView === 'Workloads'}
        {#if !activeClusterId}
          <section class="empty-view"><div class="explore-orbit"><i></i><i></i><b>▦</b></div><h2>Select a cluster first</h2><p>Workload inventory is loaded only for the cluster and namespace you choose.</p></section>
        {:else}
          <section class="workloads-page font-sans">
            {#if workloadResources.length > 1}
              <nav class="kind-tabs" aria-label="Workload types">{#each workloadResources as resource}<button type="button" aria-pressed={workloadResource !== null && resourceKey(workloadResource) === resourceKey(resource)} class:kind-tab-active={workloadResource !== null && resourceKey(workloadResource) === resourceKey(resource)} title={kindLabel(resource)} on:click={() => selectWorkloadResource(resource)}>{kindTabLabel(resource)}</button>{/each}</nav>
            {/if}
            <div class:workload-detail-open={(editorResource?.category === 'Workloads' && editorObject !== null) || (workloadDetailMode === 'logs' && logTarget !== null)} class:workload-logs-open={workloadDetailMode === 'logs' && logTarget !== null} class="workload-grid grid min-h-[560px]">
              <div class="workload-list-panel overflow-hidden rounded-2xl border border-white/10 bg-[#151924]/90 shadow-2xl shadow-black/10"><div class="workload-list-header flex items-center justify-between gap-4 border-b border-white/10 px-5 py-4"><div><div class="flex items-center gap-2"><Container size={18} class="text-cyan-300" /><h3 class="m-0 text-lg font-semibold text-white">{workloadResource?.kind || 'Select a type'}</h3></div><p class="mb-0 mt-1 text-xs text-slate-400">{namespace} · {workloadResource?.apiVersion || 'Kubernetes API'}{#if workloadResource?.kind === 'Pod'} · CPU/memory from Metrics API{/if}</p></div><span class:live-status-loading={liveDataStatus === 'loading'} class:live-status-loaded={liveDataStatus === 'loaded'} class:live-status-stale={liveDataStatus === 'stale'} class:live-status-paused={liveDataStatus === 'paused'} class:live-status-unavailable={liveDataStatus === 'unavailable'} class="live-list-status" title={liveDataStatusTooltip} aria-label={liveDataStatusTooltip} aria-live="polite"><i></i>{liveDataStatusText}</span><label class="flex h-10 w-72 items-center gap-2 rounded-lg border border-white/10 bg-black/20 px-3 text-slate-500 focus-within:border-indigo-400 focus-within:bg-black/30 focus-within:ring-2 focus-within:ring-indigo-500/20"><Search size={16} /><input class="min-w-0 flex-1 border-0 bg-transparent text-sm text-slate-100 outline-none placeholder:text-slate-500" bind:value={workloadSearch} placeholder={`Filter ${workloadResource?.plural || 'workloads'}`} /></label></div>
                {#if loadingWorkloads}
                  <div class="grid min-h-96 place-items-center text-sm text-slate-400"><div class="flex items-center gap-3"><RefreshCw size={18} class="animate-spin text-cyan-300" />Loading {workloadResource?.plural || 'workloads'}…</div></div>
                {:else if visibleWorkloadObjects.length === 0}
                  <div class="grid min-h-96 place-items-center px-6 text-center"><div><div class="mx-auto grid h-12 w-12 place-items-center rounded-2xl bg-indigo-500/15 text-cyan-300"><Boxes size={22} /></div><h4 class="mb-0 mt-4 text-base font-semibold text-slate-100">{workloadObjects.length ? 'No matching workloads' : `No ${workloadResource?.plural || 'workloads'} found`}</h4><p class="mb-0 mt-2 text-sm text-slate-400">{workloadObjects.length ? 'Try a different name or namespace filter.' : `Nothing was returned for ${namespace}.`}</p></div></div>
                {:else}
                  <div class="workload-object-list" aria-label={`${workloadResource?.kind || 'Workload'} list`}>
                    <div class:workload-pod-row={workloadResource?.kind === 'Pod'} class="workload-object-list-header workload-selection-header" style:--workload-columns={`28px ${workloadGridColumns}`} aria-label="Workload columns">{#if workloadPermissionSet.canDelete}<label class:resource-select-all-partial={workloadObjectsSelectionPartial} class="resource-select-all"><input type="checkbox" checked={allWorkloadObjectsSelected} disabled={deletingResource || loadingWorkloads || !workloadObjects.length} aria-checked={workloadObjectsSelectionPartial ? 'mixed' : allWorkloadObjectsSelected ? 'true' : 'false'} aria-label={`Select all loaded ${workloadResource?.plural || 'workloads'}`} on:change={toggleAllWorkloadObjects} /></label>{:else}<span class="resource-permission-spacer"></span>{/if}{#each workloadColumns as column}<span>{column.label}</span>{/each}<span></span></div>
                    {#if selectedWorkloadObjects.length && workloadPermissionSet.canDelete}<div class="resource-bulk-toolbar workload-bulk-toolbar" role="region" aria-label="Bulk workload actions"><span><strong>{selectedWorkloadObjects.length}</strong> selected</span><button class="destructive" type="button" disabled={deletingResource || loadingEditor || savingEditor || loadingYaml || savingYaml} on:click={() => workloadResource && requestBulkResourceDeletion(workloadResource, selectedWorkloadObjects)}>Delete {selectedWorkloadObjects.length}</button><button class="resource-bulk-clear" type="button" disabled={deletingResource} on:click={clearWorkloadObjectSelection}>Clear</button></div>{/if}
                    {#each visibleWorkloadObjects as workload}
                      <div class:workload-pod-row={workloadResource?.kind === 'Pod'} class:resource-object-row-selected={isWorkloadObjectSelected(workload, selectedWorkloadObjectKeys)} class="resource-object-row workload-selection-row">
                        {#if workloadPermissionSet.canDelete}<label class="resource-object-select"><input type="checkbox" checked={isWorkloadObjectSelected(workload, selectedWorkloadObjectKeys)} disabled={deletingResource} aria-label={`Select ${workloadResource?.kind || 'workload'} ${workload.name}`} on:change={() => toggleWorkloadObjectSelection(workload)} /></label>{:else}<span class="resource-permission-spacer"></span>{/if}
                        <button
                          class:workload-pod-row={workloadResource?.kind === 'Pod'}
                          class:workload-object-selected={Boolean(
                            isWorkloadObjectSelected(workload, selectedWorkloadObjectKeys)
                            || (editorResource
                              && editorObject
                              && workloadResource
                              && resourceKey(editorResource) === resourceKey(workloadResource)
                              && editorObject.name === workload.name
                              && editorObject.namespace === workload.namespace)
                          )}
                          class="workload-object-row group"
                          style:--workload-columns={workloadGridColumns}
                          disabled={!workloadPermissionSet.canGet}
                          on:click={() => workloadResource && openObject(workloadResource, workload)}
                        >
                          {#each workloadColumns as column}
                            {#if column.key === 'name'}<div class="workload-object-name"><strong title={workload.name}>{workload.name}</strong></div>
                            {:else if column.key === 'namespace'}<div class="workload-row-fact workload-row-muted"><b title={workload.namespace || 'Cluster scoped'}>{workload.namespace || '—'}</b></div>
                            {:else if column.key === 'node'}<div class="workload-row-fact workload-row-muted workload-row-node"><b title={workload.nodeName || 'Not scheduled'}><bdi>{workload.nodeName || '—'}</bdi></b></div>
                            {:else if column.key === 'status'}<div class="workload-row-fact"><b class={`workload-status-label ${workloadStatusTone(workload)}`} title={workloadStatusLabel(workload)}>{workloadStatusLabel(workload)}</b></div>
                            {:else if column.key === 'ready'}<div class="workload-row-fact"><b>{podContainerSummary(workload)}</b></div>
                            {:else if column.key === 'restarts'}<div class="workload-row-fact"><b class:workload-restarts-warn={(workload.restarts || 0) > 0}>{workload.restarts ?? 0}</b></div>
                            {:else if column.key === 'cpu'}<div class="workload-row-fact"><b title={cpuMetricLabel(workload.cpuUsage)}>{cpuMetricLabel(workload.cpuUsage)}</b></div>
                            {:else if column.key === 'memory'}<div class="workload-row-fact"><b title={workload.memoryUsage || 'Metrics unavailable'}>{podMetricLabel(workload.memoryUsage)}</b></div>
                            {:else}<div class="workload-row-fact workload-row-age"><b>{resourceAge(workload.createdAt)}</b></div>{/if}
                          {/each}
                          <ChevronRight size={17} class="workload-row-arrow" />
                        </button>
                      </div>
                    {/each}
                  </div>
                {/if}
              </div>
              {#if workloadDetailMode === 'logs' && logTarget}
                <aside class="workload-inspector workload-log-inspector" aria-label={`${logTarget.pod} logs`}>
                  <div class="workload-inspector-heading workload-log-heading"><div><p class="eyebrow">Live logs</p><h3>{logTarget.pod}</h3><p>{activeCluster} · {logScopeLabel || 'Pod'} · {logTarget.namespace}</p></div><div class="workload-inspector-actions">{#if editorResource && editorObject}<button class="secondary workload-log-back" on:click={closeWorkloadLogs}>← Details</button>{/if}<button aria-label="Close logs" on:click={closeWorkloadLogs}>×</button></div></div>
                  <div class="workload-log-body">
                    <nav class="workload-log-level-bar" aria-label="Log navigation"><button type="button" on:click={returnToWorkloadList}>← All {workloadResource?.plural || 'workloads'}</button><span>{logScopeLabel || 'Pod'} / {logTarget.pod}</span></nav>
                    <section class="workload-log-pod-picker"><div><div><strong>Pod stream</strong><small>{logPods.length} available for this workload</small></div><label>Switch Pod<select value={logTargetKey(logTarget)} on:change={(event) => selectLogPodByKey(event.currentTarget.value)}>{#each logPods as pod}<option value={logPodKey(pod)}>{pod.name} · {pod.namespace || namespace}</option>{/each}</select></label></div></section>
                    <section class="workload-log-toolbar"><div><strong>Live logs</strong><small>{openingLogsTarget ? 'Opening the first stream…' : loadingLogs ? 'Refreshing now…' : 'Auto-refreshes every 30 seconds · select, copy, or download'}</small></div><div class="log-search" role="search"><Search size={13} /><input bind:value={logSearch} placeholder="Search logs" aria-label="Search log output" spellcheck="false" />{#if logSearch}<span>{visibleLogLines.length}/{logLines.length}</span><button type="button" aria-label="Clear log search" on:click={() => (logSearch = '')}>×</button>{/if}</div><div class="workload-log-toolbar-actions">{#if logContainers.length > 1}<label>Container <select bind:value={selectedLogContainer} on:change={() => loadLogs(true)}>{#each logContainers as container}<option value={container}>{container}</option>{/each}</select></label>{/if}<button class="log-tool-button" disabled={loadingLogs || !logTarget} aria-label="Refresh logs now" title="Refresh logs now" on:click={() => loadLogs(true)}><RefreshCw size={13} class={loadingLogs ? 'animate-spin' : ''} /><span>{loadingLogs ? 'Refreshing' : 'Refresh'}</span></button><button class:log-tool-button-copied={logsCopied} class="log-tool-button" disabled={!logLines.length} aria-label="Copy all logs" title="Copy all logs" on:click={copyLogs}>{#if logsCopied}<Check size={13} />{:else}<Copy size={13} />{/if}<span>{logsCopied ? 'Copied' : 'Copy'}</span></button><button class="log-tool-button" disabled={!logLines.length || downloadingLogs} aria-label="Download current logs" title="Download current logs" on:click={downloadLogs}><Download size={13} /><span>{downloadingLogs ? 'Saving' : 'Download'}</span></button></div></section>
                    {#if logPorts.length}<section class="workload-log-ports"><strong>Container ports</strong><div>{#each logPorts as port}<span title={`${port.container}${port.name ? ` · ${port.name}` : ''} · ${port.protocol}`}>{port.port}/{port.protocol}<small>{port.container}</small></span>{/each}</div></section>{/if}
                    {#if loadingLogs && logLines.length === 0}<div class="workload-log-opening"><RefreshCw size={18} class="animate-spin" /><div><strong>Opening logs…</strong><small>Connecting to {logTarget.pod}{selectedLogContainer ? ` · ${selectedLogContainer}` : ''}</small></div></div>{:else}<pre bind:this={logViewport} class="workload-log-output" aria-label={`${logTarget.pod} logs`}>{logLines.length ? (visibleLogLines.length ? visibleLogLines.join('\n') : `No log lines match “${logSearch}”.`) : 'No log lines returned yet.'}</pre>{/if}
                  </div>
                </aside>
              {:else if editorResource && editorObject && editorResource.category === 'Workloads'}
                <aside class="workload-inspector" aria-label="Workload details">
                  <div class="workload-inspector-heading workload-details-heading"><div><h3 title={editorObject.name}>{editorObject.name}</h3><p>{editorResource.kind} · {editorObject.namespace || 'cluster scoped'} <b class={`workload-status-label workload-details-status ${workloadStatusTone(editorObject)}`}>{workloadStatusLabel(editorObject)}</b>{#if editorPermissionSet.resolved && !editorPermissionSet.canUpdate && !editorPermissionSet.canDelete}<span class="permission-readonly-badge">Read only</span>{/if}</p></div><div class="workload-inspector-actions"><button aria-label="Close workload details" title="Close" on:click={() => closeEditor()}>×</button></div></div>
                  {#if !loadingEditor && workloadDetailMode !== 'terminal'}<nav class="workload-details-toolbar" aria-label="Workload actions">{#if editorPermissionSet.canGet}<button type="button" on:click={() => openYamlEditor(editorResource!, editorObject!)}><Command size={13} />YAML</button>{/if}{#if editorPermissionSet.canDelete}<button type="button" class="workload-details-delete" on:click={() => requestResourceDeletion(editorResource!, editorObject!)}>Delete</button>{/if}</nav>{/if}
                  {#if loadingEditor}
                    <div class="drawer-state"><i></i>Loading live workload details…</div>
                  {:else if workloadDetailMode === 'terminal'}
                    <section class="workload-terminal">
                      <div class="workload-terminal-heading"><button on:click={resetWorkloadTerminal}>← Details</button><div><strong>Pod terminal</strong><small>Kubernetes exec — runs in the selected container</small></div></div>
                      {#if loadingTerminalPods}<div class="drawer-state"><i></i>Finding live Pods…</div>{:else if terminalPods.length === 0}<div class="workload-terminal-empty"><strong>No live Pod is available</strong><p>Wait for a replica to become ready, then try again.</p></div>{:else}
                        <div class="terminal-pod-strip"><span>Pod</span><div>{#each terminalPods as pod}<button class:terminal-pod-selected={terminalTarget?.pod === pod.name && terminalTarget?.namespace === (pod.namespace || namespace)} on:click={() => selectTerminalPod(pod)}>{pod.name}</button>{/each}</div></div>
                        {#if loadingTerminalRuntime}<div class="drawer-state"><i></i>Inspecting containers…</div>{:else if terminalTarget}
                          <div class="terminal-target"><div><span>Connected target</span><strong>{terminalTarget.namespace}/{terminalTarget.pod}</strong></div>{#if terminalContainers.length > 1}<label>Container<select bind:value={selectedTerminalContainer}>{#each terminalContainers as container}<option value={container}>{container}</option>{/each}</select></label>{:else}<small>{selectedTerminalContainer || 'Container unavailable'}</small>{/if}</div>
                          {#if terminalPorts.length}<div class="terminal-port-hints"><span>Declared ports</span><div>{#each terminalPorts as port}<small>{port.container} · {port.port}/{port.protocol}</small>{/each}</div></div>{/if}
                          <form class="terminal-command" on:submit|preventDefault={runTerminalCommand}><label>Command<input bind:value={terminalCommand} placeholder="e.g. printenv | sort" spellcheck="false" /></label><button class="primary" disabled={runningTerminalCommand}>{runningTerminalCommand ? 'Running…' : 'Run'}</button></form>
                          <pre class="terminal-output">{terminalOutput || 'Run a command to inspect this container. Commands use /bin/sh -lc and require pods/exec permission.'}</pre>
                        {/if}
                      {/if}
                    </section>
                  {:else}
                    <div class="workload-inspector-body">
                      {#if editorPermissionSet.canViewLogs || editorPermissionSet.canExec}<section class="workload-action-grid">{#if editorPermissionSet.canViewLogs}<button class:workload-action-loading={editorLogsOpening} class="workload-action-card workload-logs-action" disabled={Boolean(openingLogsTarget)} aria-busy={editorLogsOpening} on:click={() => openWorkloadLogs(editorResource!, editorObject!)}><span>{#if editorLogsOpening}<RefreshCw size={18} class="workload-action-spinner" />{:else}≡{/if}</span><div><strong>{editorLogsOpening ? 'Opening logs…' : 'View logs'}</strong><small>{editorLogsOpening ? `Preparing ${editorResource.kind} logs and the first live stream` : editorResource.kind === 'Pod' ? 'Keep workload types visible beside this Pod stream' : 'Choose a live Pod and stream its output without leaving Workloads'}</small></div><b>{editorLogsOpening ? '•••' : '→'}</b></button>{/if}{#if editorPermissionSet.canExec}<button class="workload-action-card workload-terminal-action" disabled={loadingTerminalPods || Boolean(openingLogsTarget)} on:click={() => openWorkloadTerminal(editorResource!, editorObject!)}><span>⌘</span><div><strong>Terminal</strong><small>Tunnel into a Pod container with Kubernetes exec</small></div><b>→</b></button>{/if}</section>{/if}
                      <dl class="workload-properties">
                        <dt>Status</dt><dd><b class={`workload-status-label ${workloadStatusTone(editorObject)}`}>{workloadStatusLabel(editorObject)}</b></dd>
                        {#if editorResource.kind === 'Pod'}
                          <dt>Ready</dt><dd>{podContainerSummary(editorObject)}</dd>
                          <dt>Restarts</dt><dd>{editorObject.restarts ?? 0}</dd>
                          {#if editorObject.nodeName}<dt>Node</dt><dd>{editorObject.nodeName}</dd>{/if}
                          {#if editorObject.cpuUsage}<dt>CPU</dt><dd>{cpuMetricLabel(editorObject.cpuUsage)}</dd>{/if}
                          {#if editorObject.memoryUsage}<dt>Memory</dt><dd>{editorObject.memoryUsage}</dd>{/if}
                        {:else}
                          <dt>Replicas</dt><dd>{workloadReplicaSummary(editorManifest)}</dd>
                        {/if}
                        <dt>Age</dt><dd>{resourceAge(editorObject.createdAt)}</dd>
                        <dt>Namespace</dt><dd>{editorObject.namespace || 'cluster scoped'}</dd>
                        <dt>API</dt><dd>{editorResource.apiVersion}</dd>
                      </dl>
                      {#if editorResource.kind === 'Pod'}<section class="pod-diagnostics-card"><div class="pod-diagnostics-heading"><strong>Diagnostics</strong><span>{selectedPodEvents.filter((event) => event.eventType === 'Warning').length} warnings</span></div>{#if podContainerDiagnostics(editorManifest).length}<div class="pod-container-diagnostics">{#each podContainerDiagnostics(editorManifest) as container}<article class:pod-diagnostic-warning={!container.ready || container.state !== 'Running'}><span class="pod-diagnostic-dot"></span><div><strong>{container.name}</strong><small>{container.state}{container.reason ? ` · ${container.reason}` : ''}{container.restarts ? ` · ${container.restarts} restarts` : ''}</small>{#if container.message}<p>{container.message}</p>{/if}</div></article>{/each}</div>{/if}{#if podConditionDiagnostics(editorManifest).length}<div class="pod-condition-strip">{#each podConditionDiagnostics(editorManifest) as condition}<span class:pod-condition-false={condition.status !== 'True'} title={condition.message || condition.reason}><b>{condition.type}</b>{condition.status}</span>{/each}</div>{/if}<div class="pod-event-list">{#each selectedPodEvents as event}<article class:pod-event-warning={event.eventType === 'Warning'}><span>{event.eventType === 'Warning' ? '!' : '✓'}</span><div><strong>{event.reason || event.action || event.eventType}</strong><p>{event.message || 'No event message returned.'}</p></div><time>{resourceAge(event.lastObserved)}</time></article>{:else}{#if loadingEvents}<div class="pod-events-loading"><i></i></div>{:else}<small class="pod-events-empty">No recent Pod events.</small>{/if}{/each}</div></section>{/if}
                      <section class="workload-detail-card"><div class="workload-detail-card-heading"><div><strong>Containers</strong><small>Images declared on the Pod template</small></div><b>{workloadImages(editorManifest).length}</b></div>{#if workloadImages(editorManifest).length}<div class="workload-image-list">{#each workloadImages(editorManifest) as container}<div><span>{container.init ? 'Init' : 'App'}</span><strong>{container.name}</strong><small title={container.image}>{container.image}</small></div>{/each}</div>{:else}<p>No container image is declared on this resource.</p>{/if}</section>
                      {#if workloadAttachments(editorManifest).configMaps.length || workloadAttachments(editorManifest).secrets.length || workloadVolumes(editorManifest).length}
                        <section class="workload-detail-card workload-config-card"><div class="workload-detail-card-heading"><div><strong>Configuration</strong><small>ConfigMaps, Secrets, and volumes this workload uses</small></div></div>
                          {#if workloadAttachments(editorManifest).configMaps.length}<div class="workload-config-group"><span>ConfigMaps</span><div class="workload-reference-list">{#each workloadAttachments(editorManifest).configMaps as configMap}<span>◇ {configMap}</span>{/each}</div></div>{/if}
                          {#if workloadAttachments(editorManifest).secrets.length}<div class="workload-config-group"><span>Secrets</span><div class="workload-reference-list secret-reference-list">{#each workloadAttachments(editorManifest).secrets as secret}<span>◈ {secret}</span>{/each}</div></div>{/if}
                          {#if workloadVolumes(editorManifest).length}<div class="workload-config-group"><span>Volumes</span><div class="workload-volume-list">{#each workloadVolumes(editorManifest) as volume}<article><div><strong>{volume.name}</strong><small>{volume.type} · {volume.source}</small></div><div>{#if volume.mounts.length}{#each volume.mounts as mount}<code>{mount}</code>{/each}{:else}<em>Declared but not mounted</em>{/if}</div></article>{/each}</div></div>{/if}
                        </section>
                      {/if}
                      {#if resourceLabels(editorManifest).length}<section class="resource-labels workload-labels"><strong>Labels</strong><div class="inspector-chip-list">{#each resourceLabels(editorManifest) as [key, value]}<span><b>{key}</b>{value}</span>{/each}</div></section>{/if}
                    </div>
                  {/if}
                </aside>
              {/if}
            </div>
          </section>
        {/if}
      {:else if activeView === 'Explore'}
        <section class="empty-view"><div class="explore-orbit"><i></i><i></i><b>⌕</b></div><h2>Explore without memorizing paths</h2><p>Ask for a resource, filter it, and move between related objects in one place.</p><button class="primary" on:click={openCommandSearch}>Search resources</button></section>
      {:else}
        {#if logTarget}
          <section class="logs-workspace panel">
            <aside class="log-pod-sidebar" aria-label="Pods with logs">
              <div class="log-pod-sidebar-heading"><div><p class="eyebrow">{logScopeLabel || 'Pod'} stream</p><h2>Pods</h2><p>{logPods.length} available in this view</p></div><div class="log-pod-heading-actions"><span>{logPods.length}</span></div></div>
              <div class="log-pod-list">{#each logPods as pod}<button class:log-pod-selected={logTarget.pod === pod.name && logTarget.namespace === (pod.namespace || namespace)} on:click={() => selectLogPod(pod)}><span class="log-pod-dot"></span><div><strong>{pod.name}</strong><small>{pod.namespace || namespace}</small></div><span class="log-pod-arrow">→</span></button>{/each}</div>
              {#if logPorts.length}<section class="log-port-section"><div><span>Container ports</span><small>{logPorts.length}</small></div>{#each logPorts as port}<span class="log-port-chip" title={`${port.container}${port.name ? ` · ${port.name}` : ''} · ${port.protocol}`}><b>{port.port}/{port.protocol}</b><small>{port.container}{port.name ? ` · ${port.name}` : ''}</small></span>{/each}</section>{/if}
              <div class="log-pod-sidebar-footer">Switch Pods without leaving the log stream.</div>
            </aside>
            <div class="log-stream-panel">
              <div class="log-stream-heading"><div><p class="eyebrow">Streaming output</p><h2>{logTarget.pod}</h2><p>{activeCluster} · {logScopeLabel || 'Pod'} · {logTarget.namespace}</p></div><div class="table-actions"><button class="secondary" on:click={() => { closeLogs(); void navigateTo('Workloads') }}>← Back to workloads</button></div></div>
              <div class="log-toolbar"><div><strong>Live logs</strong><small>{openingLogsTarget ? 'Opening the first live stream…' : loadingLogs ? 'Refreshing now…' : 'Auto-refreshes every 30 seconds · select, copy, or download'}</small></div><div class="log-search" role="search"><Search size={13} /><input bind:value={logSearch} placeholder="Search logs" aria-label="Search log output" spellcheck="false" />{#if logSearch}<span>{visibleLogLines.length}/{logLines.length}</span><button type="button" aria-label="Clear log search" on:click={() => (logSearch = '')}>×</button>{/if}</div><div class="log-toolbar-actions">{#if logContainers.length > 1}<label>Container <select bind:value={selectedLogContainer} on:change={() => loadLogs(true)}>{#each logContainers as container}<option value={container}>{container}</option>{/each}</select></label>{/if}<button class="log-tool-button" disabled={loadingLogs || !logTarget} on:click={() => loadLogs(true)}><RefreshCw size={13} class={loadingLogs ? 'animate-spin' : ''} /><span>{loadingLogs ? 'Refreshing' : 'Refresh'}</span></button><button class:log-tool-button-copied={logsCopied} class="log-tool-button" disabled={!logLines.length} on:click={copyLogs}>{#if logsCopied}<Check size={13} />{:else}<Copy size={13} />{/if}<span>{logsCopied ? 'Copied' : 'Copy'}</span></button><button class="log-tool-button" disabled={!logLines.length || downloadingLogs} on:click={downloadLogs}><Download size={13} /><span>{downloadingLogs ? 'Saving' : 'Download'}</span></button></div></div>
              {#if loadingLogs && logLines.length === 0}<div class="log-opening-state"><span><RefreshCw size={22} class="animate-spin" /></span><div><p class="eyebrow">Opening logs</p><h3>{logTarget.pod}</h3><p>Connecting to the Pod and preparing the first live output. You can switch Pods from the left after it opens.</p></div></div>{:else}<pre class="live-log-output" bind:this={logViewport} aria-label={`${logTarget.pod} logs`}>{#if logLines.length}{visibleLogLines.length ? visibleLogLines.join('\n') : `No log lines match “${logSearch}”.`}{:else}The Pod returned no log lines for this container yet.{/if}</pre>{/if}
            </div>
          </section>
        {:else}
          <section class="empty-view"><div class="explore-orbit"><i></i><i></i><b>≡</b></div><h2>No logs selected</h2><p>Open a Pod directly or open a workload to choose one of its Pods. Logs stay in this full workspace tab.</p><button class="primary" on:click={() => navigateTo('Workloads')}>Browse workloads</button></section>
        {/if}
      {/if}
    </div>
    {#if cliOpen}
      <section class:cli-drawer-expanded={cliExpanded} class="cli-drawer" aria-label="Cluster terminal">
        <header><div><Terminal size={15} /><strong>Terminal</strong><span>{activeCluster} · {namespace === 'all namespaces' ? 'kubeconfig default namespace' : namespace}</span>{#if runningCli}<em class="cli-running-badge">running</em>{/if}</div><div class="cli-drawer-actions"><button type="button" aria-label="Copy output" title="Copy output" disabled={!cliLines.length} on:click={copyClusterCliOutput}><Copy size={13} /></button><button type="button" aria-label="Clear output" title="Clear output · Ctrl+L" disabled={!cliLines.length} on:click={clearClusterCli}>⌫</button><button type="button" aria-label={cliExpanded ? 'Shrink terminal' : 'Expand terminal'} title={cliExpanded ? 'Shrink' : 'Expand'} on:click={() => (cliExpanded = !cliExpanded)}>{cliExpanded ? '▾' : '▴'}</button><button type="button" aria-label="Close terminal" title="Close" on:click={() => (cliOpen = false)}>×</button></div></header>
        <pre bind:this={cliViewport} class="cli-drawer-output" aria-live="polite">{#if cliLines.length}{#each cliLines as line}<span class="cli-line cli-line-{line.stream}">{line.text}{'\n'}</span>{/each}{/if}</pre>
        {#if !cliLines.length}<div class="cli-starters" aria-label="Suggested commands">{#each cliStarterCommands as starter}<button type="button" on:click={() => runClusterCli(starter)}>{starter}</button>{/each}</div>{/if}
        <form class="cli-drawer-command" on:submit|preventDefault={() => runClusterCli()}><span>$</span><textarea id="cluster-cli-input" bind:this={cliInput} bind:value={cliCommand} use:autoSizeCliTextarea={cliCommand} rows="1" aria-label="Cluster terminal command" placeholder={runningCli ? 'Running… Ctrl+C to stop' : 'kubectl get pods'} spellcheck="false" autocapitalize="off" autocomplete="off" on:keydown={handleClusterCliKeydown}></textarea>{#if runningCli}<button type="button" class="cli-stop" aria-label="Stop command" title="Stop · Ctrl+C" on:click={cancelClusterCli}>■</button>{:else}<button type="submit" aria-label="Run command" title="Run · Shift+Enter adds a line">↵</button>{/if}</form>
      </section>
    {/if}
    <footer class="workspace-statusbar"><button class:workspace-cli-active={cliOpen} type="button" disabled={!activeClusterId} title={activeClusterId ? `Open terminal for ${activeCluster}` : 'Select a cluster first'} on:click={toggleClusterCli}><Terminal size={14} /><span>CLI</span></button>{#if activeClusterId}<small>{activeCluster} · {namespace === 'all namespaces' ? 'all namespaces' : namespace}</small>{/if}<div class="workspace-zoom-controls" role="group" aria-label="Interface size"><button type="button" disabled={uiScale <= 0.8} aria-label="Decrease interface size" title="Decrease interface size" on:click={() => adjustUiScale(-0.05)}>−</button><output aria-live="polite">{Math.round(uiScale * 100)}%</output><button type="button" disabled={uiScale >= 1.25} aria-label="Increase interface size" title="Increase interface size" on:click={() => adjustUiScale(0.05)}>+</button></div></footer>
  </section>

  {#if deletionTarget}
    <div class="modal-backdrop deletion-backdrop" role="presentation" on:click={cancelDeletion}>
      <div bind:this={deletionDialog} class="deletion-modal" role="dialog" aria-modal="true" aria-labelledby="deletion-dialog-title" aria-describedby="deletion-dialog-description" tabindex="-1" on:click|stopPropagation on:keydown|stopPropagation={handleDeletionDialogKeydown}>
        <div class="deletion-modal-mark">!</div>
        {#if deletionStep === 1}
          <p class="eyebrow">Review deletion request</p>
          <h2 id="deletion-dialog-title">{deletionTarget.type === 'resource' ? `Delete ${deletionTarget.resource.kind}?` : deletionTarget.type === 'bulk-resource' ? `Delete ${deletionTarget.objects.length} ${deletionTarget.resource.kind}s?` : 'Remove kubeconfig context?'}</h2>
          <p id="deletion-dialog-description" class="deletion-intro">{deletionTarget.type === 'resource' ? 'This sends a Kubernetes DELETE request only to the cluster and namespace shown below.' : deletionTarget.type === 'bulk-resource' ? 'This sends one Kubernetes DELETE request per selected object. The list stays scoped to the current resource kind and namespace.' : 'This removes the context from Kuberniva only. The original kubeconfig source remains unchanged.'}</p>
          <dl class="deletion-target-summary">
            <div><dt>Target</dt><dd>{deletionTargetName()}</dd></div>
            {#if deletionTarget.type === 'resource'}
              <div><dt>Cluster</dt><dd>{deletionTarget.cluster}</dd></div><div><dt>Namespace</dt><dd>{deletionTarget.namespaceScope === 'all namespaces' ? (deletionTarget.object.namespace || 'all namespaces') : deletionTarget.namespaceScope}</dd></div>
            {:else if deletionTarget.type === 'bulk-resource'}
              <div><dt>Cluster</dt><dd>{deletionTarget.cluster}</dd></div><div><dt>Kind</dt><dd>{deletionTarget.resource.kind}</dd></div><div><dt>Scope</dt><dd>{deletionTarget.namespaceScope === 'all namespaces' ? 'All namespaces' : deletionTarget.namespaceScope}</dd></div><div><dt>Count</dt><dd>{deletionTarget.objects.length}</dd></div><div><dt>Selected</dt><dd class="deletion-selected-names" title={deletionTarget.objects.map((object) => bulkDeletionObjectLabel(deletionTarget, object)).join(', ')}>{deletionTarget.objects.map((object) => bulkDeletionObjectLabel(deletionTarget, object)).join(', ')}</dd></div>
            {:else}
              <div><dt>Source</dt><dd title={deletionTarget.cluster.kubeconfigPath}>{deletionTarget.cluster.kubeconfigPath}</dd></div>
            {/if}
          </dl>
          <div class="deletion-warning">{deletionTarget.type === 'resource' || deletionTarget.type === 'bulk-resource' ? 'Deletion cannot be undone. Kubernetes may handle dependents according to the resource’s configured deletion policy.' : 'You can bring this context back later by manually syncing its kubeconfig source. No source file is edited or deleted.'}</div>
          <div class="deletion-actions"><button class="secondary" disabled={deletingResource} on:click={cancelDeletion}>Cancel</button><button class="destructive" disabled={deletingResource} on:click={continueDeletion}>Continue</button></div>
        {:else}
          <p class="eyebrow">Final confirmation</p>
          <h2 id="deletion-dialog-title">Are you sure?</h2>
          <p id="deletion-dialog-description" class="deletion-intro">This is the final confirmation. Kubernetes will receive the exact deletion request described above, and the action cannot be undone.</p>
          {#if bulkDeleteProgress}<p class="bulk-delete-progress" aria-live="polite">Deleting {bulkDeleteProgress.completed} of {bulkDeleteProgress.total}… {bulkDeleteProgress.failed ? `${bulkDeleteProgress.failed} failed so far` : 'No failures so far'}</p>{/if}
          <div class="deletion-actions"><button class="secondary" disabled={deletingResource} on:click={() => { deletionStep = 1; void tick().then(() => deletionDialog?.focus()); }}>Back</button><button bind:this={deletionConfirmButton} class="destructive" disabled={deletingResource} on:click={confirmDeletion}>{deletingResource ? 'Deleting…' : deletionTarget.type === 'resource' ? 'Delete resource' : deletionTarget.type === 'bulk-resource' ? `Delete ${deletionTarget.objects.length} objects` : 'Remove from Kuberniva'}</button></div>
        {/if}
      </div>
    </div>
  {/if}

  {#if commandOpen}
    <div class="modal-backdrop" role="presentation" on:click={() => (commandOpen = false)}>
      <div class="command-modal" role="dialog" aria-modal="true" aria-label="Search Kuberniva resources" tabindex="-1" on:click|stopPropagation on:keydown|stopPropagation={handleCommandKeydown}>
        <div class="command-input"><Search size={16} /><input id="global-command-search" bind:value={commandQuery} aria-label="Search Kubernetes resources" placeholder="Search kind, object, API group, or version…" /><kbd>esc</kbd></div>
        {#if commandQuery.trim()}
          <p class="eyebrow">{globalSearchResults.length ? `${globalSearchResults.length} matches` : 'No matches'}</p>
          <div class="command-results" role="listbox" aria-label="Search results">
            {#each globalSearchResults as result}
              <button class="command-result" type="button" role="option" aria-selected={false} on:click={() => openGlobalSearchResult(result)}><span class="command-result-mark">{result.type === 'object' ? '○' : '◇'}</span><span><strong>{result.title}</strong><small>{result.detail}</small></span><b>{result.type === 'object' ? 'Open' : 'Browse'}</b></button>
            {:else}
              <div class="command-empty"><Search size={18} /><strong>No discovered resource matches “{commandQuery}”</strong><small>Try a kind, object name, API group, or version such as v1beta1.</small></div>
            {/each}
          </div>
        {:else}
          <p class="eyebrow">Search the active cluster</p>
          <div class="command-hint"><Search size={18} /><div><strong>Find any discovered API or loaded object</strong><small>Search is case-insensitive and includes alpha/beta API versions.</small></div></div>
          <button class="command-secondary-action" type="button" on:click={() => { commandOpen = false; void navigateTo('Resources') }}>▤ <span>Browse all discovered resources</span><kbd>↵</kbd></button>
          <button class="command-secondary-action" type="button" on:click={() => { commandOpen = false; kubeconfigOpen = true }}>⌁ <span>Add a kubeconfig</span></button>
        {/if}
      </div>
    </div>
  {/if}

  {#if kubeconfigOpen}
    <div class="modal-backdrop kubeconfig-backdrop" role="presentation" on:click={() => closeKubeconfigModal()}>
      <div class="kubeconfig-modal" role="dialog" aria-modal="true" aria-labelledby="kubeconfig-modal-title" aria-describedby="kubeconfig-modal-description" tabindex="-1" on:click|stopPropagation on:keydown|stopPropagation={handleKubeconfigModalKeydown}>
        <div class="kubeconfig-modal-heading"><div class="modal-kube-mark">⌁</div><div><h2 id="kubeconfig-modal-title">Add kubeconfig</h2><p id="kubeconfig-modal-description">Add one source without replacing any clusters already in your workspace.</p></div></div>

        <div class="kubeconfig-source-modes" role="tablist" aria-label="Kubeconfig source type">
          <button id="kubeconfig-source-tab-file" type="button" class:source-mode-active={kubeconfigInputMode === 'file'} role="tab" aria-selected={kubeconfigInputMode === 'file'} aria-controls="kubeconfig-source-panel" tabindex={kubeconfigInputMode === 'file' ? 0 : -1} on:click={() => (kubeconfigInputMode = 'file')} on:keydown={(event) => handleKubeconfigSourceTabKeydown(event, 'file')}><span>▤</span><strong>File</strong><small>One kubeconfig</small></button>
          <button id="kubeconfig-source-tab-folder" type="button" class:source-mode-active={kubeconfigInputMode === 'folder'} role="tab" aria-selected={kubeconfigInputMode === 'folder'} aria-controls="kubeconfig-source-panel" tabindex={kubeconfigInputMode === 'folder' ? 0 : -1} on:click={() => (kubeconfigInputMode = 'folder')} on:keydown={(event) => handleKubeconfigSourceTabKeydown(event, 'folder')}><span>▱</span><strong>Folder</strong><small>Many files</small></button>
          <button id="kubeconfig-source-tab-paste" type="button" class:source-mode-active={kubeconfigInputMode === 'paste'} role="tab" aria-selected={kubeconfigInputMode === 'paste'} aria-controls="kubeconfig-source-panel" tabindex={kubeconfigInputMode === 'paste' ? 0 : -1} on:click={() => (kubeconfigInputMode = 'paste')} on:keydown={(event) => handleKubeconfigSourceTabKeydown(event, 'paste')}><span>⌘</span><strong>Paste YAML</strong><small>From clipboard</small></button>
        </div>

        {#if kubeconfigInputMode === 'paste'}
          <div id="kubeconfig-source-panel" class="kubeconfig-source-panel paste-source-panel" role="tabpanel" aria-labelledby="kubeconfig-source-tab-paste">
            <div class="source-panel-heading"><div><strong>Paste kubeconfig YAML</strong><small>Contexts are saved locally and become reusable after restarting Kuberniva.</small></div><b>YAML</b></div>
            <textarea aria-label="Kubeconfig YAML" bind:value={pastedKubeconfig} spellcheck="false" placeholder={'apiVersion: v1\nkind: Config\nclusters:\n  - cluster: …\ncontexts:\n  - context: …'}></textarea>
            <p class="paste-security-note"><span>✓</span>Your existing sources remain untouched. The pasted configuration is stored only in Kuberniva's local app data.</p>
          </div>
        {:else}
          <div id="kubeconfig-source-panel" class="kubeconfig-source-panel" role="tabpanel" aria-labelledby={`kubeconfig-source-tab-${kubeconfigInputMode}`}>
            <div class="source-panel-heading"><div><strong>{kubeconfigInputMode === 'folder' ? 'Choose a kubeconfig folder' : 'Choose a kubeconfig file'}</strong><small>{kubeconfigInputMode === 'folder' ? 'Every readable kubeconfig in the folder is discovered.' : 'Paths can be absolute, relative to home, or begin with ~/.'}</small></div></div>
            <label>{kubeconfigInputMode === 'folder' ? 'Folder path' : 'File path'}<input bind:value={kubeconfigPath} placeholder={kubeconfigInputMode === 'folder' ? '~/clusters' : 'Default: ~/.kube/config'} /></label>
            <div class="source-picker-actions"><button class="secondary" on:click={() => chooseKubeconfig(kubeconfigInputMode === 'folder')}>{kubeconfigInputMode === 'folder' ? 'Choose folder…' : 'Choose file…'}</button></div>
          </div>
        {/if}

        <div class="kubeconfig-modal-footer"><button class="secondary" disabled={loadingCatalog} on:click={() => closeKubeconfigModal()}>Cancel</button><button class="primary" disabled={loadingCatalog || (kubeconfigInputMode === 'paste' && !pastedKubeconfig.trim())} on:click={() => kubeconfigInputMode === 'paste' ? importPastedKubeconfig() : connectKubeconfig()}>{loadingCatalog ? (kubeconfigInputMode === 'paste' ? 'Importing…' : 'Discovering…') : (kubeconfigInputMode === 'paste' ? 'Import contexts' : 'Add & discover')}</button></div>
      </div>
    </div>
  {/if}

  {#if selectedResource && activeView !== 'Resources'}
      <aside class="resource-detail-tab" aria-label={`${selectedResource.kind} objects`}>
      <div class="resource-drawer">
        <div class="drawer-heading"><div><span class:custom={selectedResource.custom}>{selectedResource.custom ? '◇' : '○'}</span><div><h2>{selectedResource.kind}</h2><p>{selectedResource.apiVersion} · {selectedResource.namespaced ? namespace : 'cluster scope'}</p></div></div><button aria-label="Close resource drawer" on:click={() => (selectedResource = null)}>×</button></div>
        {#if loadingObjects}
          <div class="drawer-state"><i></i>Listing {selectedResource.plural}…</div>
        {:else if loadingRelatedPods}
          <div class="drawer-state"><i></i>Finding Pods for this {selectedResource.kind}…</div>
        {:else if relatedPods !== null}
          <div class="related-pods"><div class="related-heading"><button on:click={() => { relatedPods = null; relatedObject = null }}>← Back</button><span>Pods selected by this {selectedResource.kind}</span>{#if relatedObject}<button class="inline-yaml" on:click={() => selectedResource && relatedObject && openYamlEditor(selectedResource, relatedObject)}>View YAML</button>{/if}</div>{#if relatedPods.length === 0}<div class="drawer-state">No matching Pods found.</div>{:else}<div class="object-list">{#each relatedPods as pod}<button disabled={Boolean(openingLogsTarget)} aria-busy={isOpeningLogs('Pod', pod)} on:click={() => openPodLogs(pod, relatedPods || [], `${selectedResource?.kind || 'Workload'} · ${relatedObject?.name || 'workload'}`)}><span class="object-icon">□</span><div><strong>{pod.name}</strong><small>{pod.namespace || 'namespace unavailable'}</small></div><span>{isOpeningLogs('Pod', pod) ? 'Opening logs…' : 'Logs →'}</span></button>{/each}</div>{/if}</div>
        {:else if resourceObjects.length === 0}
          <div class="drawer-state">No {selectedResource.plural} found in this scope.</div>
        {:else}
          <div class="object-list">{#each resourceObjects as object}<button aria-busy={selectedResource.kind === 'Pod' && isOpeningLogs('Pod', object)} on:click={() => openObject(selectedResource!, object)}><span class="object-icon">□</span><div><strong>{object.name}</strong><small>{object.namespace || 'cluster scoped'} {object.createdAt ? `· ${object.createdAt}` : ''}</small></div><span>{selectedResource.kind === 'Pod' ? (isOpeningLogs('Pod', object) ? 'Opening logs…' : 'Logs →') : selectedResource.category === 'Workloads' ? 'Pods →' : 'Details →'}</span></button>{/each}</div>
        {/if}
        <div class="drawer-footer"><span>Demand-loaded · no background fan-out</span><span>Open an object for its live details.</span></div>
      </div>
      </aside>
  {/if}


  {#if yamlResource && yamlObject}
    <aside class="resource-yaml-tab" aria-label={`${yamlObject.name} YAML`}>
      <div class="yaml-surface">
        <div class="drawer-heading yaml-heading"><div><span>{yamlMode === 'edit' ? '✎' : '⌘'}</span><div><h2>{yamlObject.name}</h2><p>{yamlResource.kind} · {yamlObject.namespace || 'cluster scoped'} · {yamlMode === 'edit' ? 'editing YAML' : 'live YAML'}</p></div></div><div class="yaml-heading-actions">{#if yamlMode === 'view' && !loadingYaml}<div class="yaml-search" role="search"><Search size={14} /><input bind:value={yamlSearch} placeholder="Search YAML" aria-label="Search YAML" spellcheck="false" />{#if yamlSearch}<span>{yamlSearchMatchCount}</span><button type="button" aria-label="Clear YAML search" on:click={() => (yamlSearch = '')}>×</button>{/if}</div>{/if}<button class="yaml-close" aria-label="Close YAML" on:click={closeYamlEditor}>×</button></div></div>
        {#if loadingYaml}
          <div class="drawer-state"><i></i>Loading live YAML…</div>
        {:else if yamlMode === 'edit'}
          <textarea class="yaml-editor" bind:value={yamlText} spellcheck="false" aria-label="Resource YAML"></textarea>
        {:else}
          <pre class:yaml-output-searching={Boolean(normalizedYamlSearch)} class="yaml-output">{#each yamlText.split('\n') as line, lineIndex}<span class:yaml-line-hit={Boolean(normalizedYamlSearch) && line.toLowerCase().includes(normalizedYamlSearch)} class="yaml-line"><b>{lineIndex + 1}</b><code>{#each yamlSearchParts(line) as part}{#if part.match}<mark>{part.text}</mark>{:else}{part.text}{/if}{/each}</code></span>{/each}</pre>
        {/if}
        <div class="yaml-footer"><span>{yamlMode === 'edit' ? 'Saving replaces this resource using its current resource version.' : 'Loaded from the Kubernetes API.'}</span><div>{#if yamlMode === 'edit'}<button class="secondary" disabled={savingYaml} on:click={() => { yamlText = yamlOriginal; yamlMode = 'view' }}>Discard</button>{#if yamlPermissionSet.canUpdate}<button class="primary" disabled={savingYaml || yamlText === yamlOriginal} on:click={saveYamlEditor}>{savingYaml ? 'Saving…' : 'Save YAML'}</button>{/if}{:else if yamlPermissionSet.canUpdate}<button class="primary" on:click={() => (yamlMode = 'edit')}>Edit YAML</button>{:else}<span class="permission-readonly-badge">Read only</span>{/if}</div></div>
      </div>
    </aside>
  {/if}

  {#if editorResource && editorObject && activeView !== 'Resources' && activeView !== 'Workloads'}
    <aside class="resource-editor-tab" aria-label={`${editorResource.kind} editor`}>
      <div class="resource-editor">
        <div class="drawer-heading"><div><span class:custom={editorResource.custom}>{editorResource.kind === 'Secret' ? '◈' : editorResource.kind === 'ConfigMap' ? '◇' : '⌁'}</span><div><h2>{editorObject.name}</h2><p>{editorResource.kind} · {editorObject.namespace || 'cluster scoped'}</p></div></div><div class="inspector-heading-actions">{#if editorPermissionSet.canGet}<button class="secondary" disabled={loadingEditor} on:click={() => openYamlEditor(editorResource!, editorObject!)}>YAML</button>{/if}<button aria-label="Close editor" on:click={() => closeEditor()}>×</button></div></div>
        {#if loadingEditor}
          <div class="drawer-state"><i></i>Loading live resource data…</div>
        {:else}
          {#if editorCertificate}
            <section class:expired={editorCertificate.expired} class="certificate-card"><div><span>⌁</span><div><strong>{editorCertificate.expired ? 'Certificate expired' : 'TLS certificate'}</strong><p>Expires {editorCertificate.expiresAt}</p></div></div><b>{certificateRemainingLabel(editorCertificate)}</b></section>
          {/if}
          {#if editorResource.kind === 'Secret' || editorResource.kind === 'ConfigMap'}
            <section class="configuration-values-editor">
              <div class="configuration-values-toolbar"><small>{editorEntries.length} {editorEntries.length === 1 ? 'key' : 'keys'}{editorEntrySearch ? ` · ${filteredEditorEntries.length} shown` : ''}{editorResource.kind === 'Secret' ? (revealSecret ? ' · decoded' : ' · base64') : ''}</small><div>{#if editorEntries.length > 8}<label class="configuration-key-search"><Search size={12} /><input bind:value={editorEntrySearch} placeholder="Filter keys" aria-label="Filter configuration keys" spellcheck="false" />{#if editorEntrySearch}<button type="button" aria-label="Clear key filter" on:click={() => (editorEntrySearch = '')}>×</button>{/if}</label>{/if}{#if editorResource.kind === 'Secret'}<button class="reveal-button" on:click={() => (revealSecret = !revealSecret)}>{revealSecret ? 'Hide decoded' : 'Reveal decoded'}</button>{/if}{#if editorPermissionSet.canUpdate}<button class="configuration-add-key" type="button" on:click={addEditorEntry}>＋ Add key</button>{/if}</div></div>
              {#if editorEntries.length === 0}<div class="configuration-values-empty">No values yet.{#if editorPermissionSet.canUpdate}<button type="button" on:click={addEditorEntry}>Add the first key</button>{/if}</div>{:else}<div class:configuration-preview-dense={editorEntries.length > 8} class="configuration-preview-list">{#each filteredEditorEntries as { entry, index }}{@const format = editorEntryFormat(entry, revealSecret)}<article class:configuration-preview-open={expandedEditorEntryIndex === index} class="configuration-preview-card"><button class="configuration-preview-toggle" type="button" aria-expanded={expandedEditorEntryIndex === index} on:click={() => toggleEditorEntry(index)}><span class={`configuration-preview-type configuration-preview-type-${format.tone}`}>{format.short}</span><span><strong>{entry.key || `Entry ${index + 1}`}</strong><small>{format.label}</small></span><code>{editorEntryPreview(entry, revealSecret)}</code><b>{expandedEditorEntryIndex === index ? '⌃' : '⌄'}</b></button>{#if expandedEditorEntryIndex === index}{#if editorPermissionSet.canUpdate}<div class="configuration-preview-editor"><label><span>Key</span><input value={entry.key} on:input={(event) => updateEditorEntryKey(index, event.currentTarget.value)} spellcheck="false" /></label><label><span>Value</span><textarea use:autoSizeTextarea={editorEntryDisplayValue(entry, revealSecret)} value={editorEntryDisplayValue(entry, revealSecret)} on:input={(event) => updateEditorEntryValue(index, event.currentTarget.value)} spellcheck="false"></textarea></label><button class="configuration-preview-remove" type="button" on:click={() => removeEditorEntry(index)}>Remove key</button></div>{:else}<div class="configuration-preview-readonly"><pre>{editorEntryDisplayValue(entry, revealSecret)}</pre></div>{/if}{/if}</article>{:else}<div class="configuration-filter-empty"><Search size={16} /><span>No keys match “{editorEntrySearch}”</span></div>{/each}</div>{/if}
            </section>
          {:else if !editorCertificate}
            <div class="drawer-state">This resource has no editable data view yet.</div>
          {/if}
        {/if}
        <div class="drawer-footer drawer-footer-compact"><div class="editor-footer-actions">{#if editorPermissionSet.resolved && !editorPermissionSet.canUpdate && !editorPermissionSet.canDelete}<span class="permission-readonly-badge">Read only</span>{/if}{#if editorPermissionSet.canGet}<button class="secondary" disabled={loadingEditor} on:click={() => openYamlEditor(editorResource!, editorObject!)}>YAML</button>{/if}{#if (editorResource.kind === 'Secret' || editorResource.kind === 'ConfigMap') && editorPermissionSet.canUpdate}<button class="primary" disabled={loadingEditor || savingEditor} on:click={saveEditor}>{savingEditor ? 'Saving…' : 'Save'}</button>{:else if editorResource.kind !== 'Secret' && editorResource.kind !== 'ConfigMap'}<button class="secondary" on:click={() => closeEditor()}>Close</button>{/if}</div></div>
      </div>
    </aside>
  {/if}

  {#if toast}<div class="toast"><span>✓</span>{toast}</div>{/if}
</main>
