/**
 * Cluster topology layout. Unlike an Argo CD tree, a cluster is a many-to-many graph: two
 * Services can select one Deployment and ten Deployments can mount one ConfigMap. Objects
 * are split into groups ("apps") joined by routes, selectors, and ownership; each group is
 * layered left to right by its longest path (Gateway → Route → Service → Deployment →
 * ReplicaSet → Pod), and groups are stacked by namespace. A ConfigMap or Secret used by
 * several apps is drawn in each of them rather than tying them into one tangle. Long Pod
 * lists collapse into a "+N pods" node.
 */
export type TopologyNode = { id: string; kind: string; group: string; name: string; namespace: string; health: string; info: string; owner?: string | null; more?: number; moreOf?: string; sharedBy?: number };
export type TopologyEdge = { from: string; to: string; relation: 'routes' | 'selects' | 'owns' | 'uses' | string };
export type Topology = { nodes: TopologyNode[]; edges: TopologyEdge[]; warnings: string[] };
export type PlacedNode = TopologyNode & { x: number; y: number; layer: number };
export type PlacedEdge = TopologyEdge & { path: string; tone: string };
export type NamespaceBand = { namespace: string; y: number; groups: number };
export type TopologyLayout = { nodes: PlacedNode[]; edges: PlacedEdge[]; bands: NamespaceBand[]; width: number; height: number; groups: number; problems: number; hidden: number };
export type TopologyOptions = {
  columnWidth: number; columnGap: number; rowHeight: number; rowGap: number; groupGap: number; bandHeight: number;
  /** Pods shown per owner before the rest collapse. */
  maxPods: number;
  /** Nodes drawn before later groups are left out (filter or pick a namespace to see them). */
  maxNodes: number;
};

export const DEFAULT_TOPOLOGY_OPTIONS: TopologyOptions = { columnWidth: 220, columnGap: 56, rowHeight: 54, rowGap: 10, groupGap: 26, bandHeight: 34, maxPods: 4, maxNodes: 1200 };
export const PROBLEM_HEALTH = new Set(['Degraded', 'Missing']);
const HEALTH_RANK: Record<string, number> = { Degraded: 0, Missing: 0, Progressing: 1, Unknown: 2, Healthy: 3 };
const ENTRY_KINDS = ['Gateway', 'Ingress', 'HTTPRoute', 'GRPCRoute', 'Service', 'Deployment', 'StatefulSet', 'DaemonSet', 'CronJob', 'Job', 'ReplicaSet', 'Pod'];

const byName = (left: TopologyNode, right: TopologyNode) => left.name.localeCompare(right.name);
const kindRank = (kind: string) => {
  const index = ENTRY_KINDS.indexOf(kind);
  return index === -1 ? ENTRY_KINDS.length : index;
};

/** Keeps the first few Pods per owner (problems first) and folds the rest into one node. */
export function collapsePods(topology: Pick<Topology, 'nodes' | 'edges'>, expanded: ReadonlySet<string>, maxPods: number) {
  const podsByOwner = new Map<string, TopologyNode[]>();
  const nodeById = new Map(topology.nodes.map((node) => [node.id, node]));
  for (const edge of topology.edges) {
    const child = nodeById.get(edge.to);
    if (edge.relation === 'owns' && child?.kind === 'Pod') podsByOwner.set(edge.from, [...(podsByOwner.get(edge.from) || []), child]);
  }
  const hidden = new Set<string>();
  const extra: TopologyNode[] = [];
  const extraEdges: TopologyEdge[] = [];
  for (const [owner, pods] of podsByOwner) {
    if (pods.length <= maxPods || expanded.has(owner)) continue;
    const ordered = [...pods].sort((left, right) => (HEALTH_RANK[left.health] ?? 2) - (HEALTH_RANK[right.health] ?? 2) || byName(left, right));
    const rest = ordered.slice(maxPods - 1);
    for (const pod of rest) hidden.add(pod.id);
    const unhealthy = rest.filter((pod) => pod.health !== 'Healthy').length;
    const id = `${owner}::pods`;
    extra.push({ id, kind: 'Pod', group: '', name: `+${rest.length} pods`, namespace: pods[0].namespace, health: unhealthy ? 'Progressing' : 'Healthy', info: unhealthy ? `${unhealthy} not ready` : 'all healthy', more: rest.length, moreOf: owner });
    extraEdges.push({ from: owner, to: id, relation: 'owns' });
  }
  if (!hidden.size) return { nodes: topology.nodes, edges: topology.edges };
  return {
    nodes: [...topology.nodes.filter((node) => !hidden.has(node.id)), ...extra],
    edges: [...topology.edges.filter((edge) => !hidden.has(edge.from) && !hidden.has(edge.to)), ...extraEdges],
  };
}

/** Connected groups of objects, found with union-find over every relation. */
export function connectedGroups(nodes: TopologyNode[], edges: TopologyEdge[]) {
  const parent = new Map(nodes.map((node) => [node.id, node.id]));
  const find = (id: string): string => {
    let root = id;
    while (parent.get(root) !== root) root = parent.get(root)!;
    let current = id;
    while (parent.get(current) !== root) {
      const next = parent.get(current)!;
      parent.set(current, root);
      current = next;
    }
    return root;
  };
  for (const edge of edges) {
    if (!parent.has(edge.from) || !parent.has(edge.to)) continue;
    const [left, right] = [find(edge.from), find(edge.to)];
    if (left !== right) parent.set(left, right);
  }
  const groups = new Map<string, TopologyNode[]>();
  for (const node of nodes) groups.set(find(node.id), [...(groups.get(find(node.id)) || []), node]);
  return [...groups.values()];
}

/**
 * Groups joined by routes, selectors, and ownership. Objects that are only used (ConfigMaps,
 * Secrets, claims) join the group of each workload using them, as a copy when shared.
 */
export function appGroups(nodes: TopologyNode[], edges: TopologyEdge[]) {
  const flow = connectedGroups(nodes, edges.filter((edge) => edge.relation !== 'uses'));
  const groupOf = new Map<string, number>();
  flow.forEach((group, index) => group.forEach((node) => groupOf.set(node.id, index)));
  const users = new Map<string, Set<number>>();
  for (const edge of edges) {
    if (edge.relation !== 'uses' || !groupOf.has(edge.from) || !groupOf.has(edge.to)) continue;
    if (flow[groupOf.get(edge.to)!].length > 1) continue;
    users.set(edge.to, (users.get(edge.to) || new Set()).add(groupOf.get(edge.from)!));
  }
  const nodeById = new Map(nodes.map((node) => [node.id, node]));
  const copyId = (id: string, group: number) => (users.get(id)!.size > 1 ? `${id}@${group}` : id);
  const groups = flow.map((group) => group.filter((node) => !users.has(node.id)));
  for (const [id, using] of users) {
    for (const group of using) groups[group].push(using.size > 1 ? { ...nodeById.get(id)!, id: copyId(id, group), sharedBy: using.size } : nodeById.get(id)!);
  }
  const groupEdges = edges.map((edge) => (edge.relation === 'uses' && users.has(edge.to) ? { ...edge, to: copyId(edge.to, groupOf.get(edge.from)!) } : edge));
  return { groups: groups.filter((group) => group.length), edges: groupEdges };
}

/** The node a group is named after: its most "upstream" object, e.g. the Ingress or Deployment. */
function groupAnchor(group: TopologyNode[]) {
  return [...group].sort((left, right) => kindRank(left.kind) - kindRank(right.kind) || byName(left, right))[0];
}

export function layoutTopology(
  topology: Pick<Topology, 'nodes' | 'edges'>,
  { search = '', problemsOnly = false, expanded = new Set<string>() }: { search?: string; problemsOnly?: boolean; expanded?: ReadonlySet<string> } = {},
  options: TopologyOptions = DEFAULT_TOPOLOGY_OPTIONS,
): TopologyLayout {
  const collapsed = collapsePods(topology, expanded, options.maxPods);
  const { groups: allGroups, edges } = appGroups(collapsed.nodes, collapsed.edges);
  const query = search.trim().toLowerCase();
  const matches = (node: TopologyNode) => !query || `${node.kind} ${node.namespace}/${node.name} ${node.info}`.toLowerCase().includes(query);
  let groups = allGroups
    .filter((group) => group.some(matches))
    .filter((group) => !problemsOnly || group.some((node) => PROBLEM_HEALTH.has(node.health)));
  groups = groups
    .map((group) => ({ group, anchor: groupAnchor(group) }))
    .sort((left, right) => left.anchor.namespace.localeCompare(right.anchor.namespace) || kindRank(left.anchor.kind) - kindRank(right.anchor.kind) || byName(left.anchor, right.anchor))
    .map(({ group }) => group);

  const outgoing = new Map<string, TopologyEdge[]>();
  const incoming = new Map<string, TopologyEdge[]>();
  for (const edge of edges) {
    outgoing.set(edge.from, [...(outgoing.get(edge.from) || []), edge]);
    incoming.set(edge.to, [...(incoming.get(edge.to) || []), edge]);
  }
  const step = options.rowHeight + options.rowGap;
  const column = options.columnWidth + options.columnGap;
  const placed: PlacedNode[] = [];
  const bands: NamespaceBand[] = [];
  let top = 0;
  let maxLayer = 0;
  let drawn = 0;
  let hidden = 0;
  for (const group of groups) {
    if (drawn && drawn + group.length > options.maxNodes) {
      hidden += group.length;
      continue;
    }
    drawn += group.length;
    const namespace = group[0].namespace;
    if (bands.at(-1)?.namespace !== namespace) {
      if (bands.length) top += options.groupGap;
      bands.push({ namespace, y: top, groups: 0 });
      top += options.bandHeight;
    }
    bands.at(-1)!.groups += 1;
    // Longest-path layering; the iteration cap guards against unexpected cycles.
    const ids = new Set(group.map((node) => node.id));
    const layer = new Map(group.map((node) => [node.id, 0]));
    for (let pass = 0, changed = true; changed && pass < group.length + 1; pass += 1) {
      changed = false;
      for (const node of group) {
        for (const edge of outgoing.get(node.id) || []) {
          if (!ids.has(edge.to)) continue;
          const next = layer.get(node.id)! + 1;
          if (next > layer.get(edge.to)!) {
            layer.set(edge.to, next);
            changed = true;
          }
        }
      }
    }
    const layers: TopologyNode[][] = [];
    for (const node of group) (layers[layer.get(node.id)!] ||= []).push(node);
    // Order each layer by the average row of its parents, so children sit beside their parent.
    const rowOf = new Map<string, number>();
    const position = new Map<string, number>();
    layers.forEach((members = [], depth) => {
      const score = (node: TopologyNode) => {
        const parents = (incoming.get(node.id) || []).map((edge) => position.get(edge.from)).filter((value): value is number => value !== undefined);
        return parents.length ? parents.reduce((sum, value) => sum + value, 0) / parents.length : Number.POSITIVE_INFINITY;
      };
      const ordered = [...members].sort((left, right) => score(left) - score(right) || Number(Boolean(left.more)) - Number(Boolean(right.more)) || kindRank(left.kind) - kindRank(right.kind) || (HEALTH_RANK[left.health] ?? 2) - (HEALTH_RANK[right.health] ?? 2) || byName(left, right));
      let next = 0;
      ordered.forEach((node, index) => {
        const wanted = depth ? Math.round(Number.isFinite(score(node)) ? score(node) : next) : index;
        const row = Math.max(next, wanted);
        rowOf.set(node.id, row);
        position.set(node.id, row);
        next = row + 1;
      });
      maxLayer = Math.max(maxLayer, depth);
    });
    const rows = Math.max(1, ...group.map((node) => rowOf.get(node.id)! + 1));
    for (const node of group) placed.push({ ...node, layer: layer.get(node.id)!, x: layer.get(node.id)! * column, y: top + rowOf.get(node.id)! * step });
    top += rows * step - options.rowGap + options.groupGap;
  }
  const positions = new Map(placed.map((node) => [node.id, node]));
  const placedEdges: PlacedEdge[] = [];
  for (const edge of edges) {
    const from = positions.get(edge.from);
    const to = positions.get(edge.to);
    if (!from || !to) continue;
    const x1 = from.x + options.columnWidth;
    const y1 = from.y + options.rowHeight / 2;
    const x2 = to.x;
    const y2 = to.y + options.rowHeight / 2;
    const bend = Math.max(24, (x2 - x1) / 2);
    placedEdges.push({ ...edge, path: `M ${x1} ${y1} C ${x1 + bend} ${y1}, ${x2 - bend} ${y2}, ${x2} ${y2}`, tone: to.health });
  }
  return {
    nodes: placed,
    edges: placedEdges,
    bands,
    width: placed.length ? (maxLayer + 1) * column - options.columnGap : 0,
    height: Math.max(0, top - options.groupGap),
    groups: bands.reduce((sum, band) => sum + band.groups, 0),
    problems: placed.filter((node) => PROBLEM_HEALTH.has(node.health)).length,
    hidden,
  };
}
