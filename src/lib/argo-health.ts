/**
 * Health analysis for the Argo CD application graph, following Argo CD's own model:
 * every node has a health (Healthy, Progressing, Degraded, Suspended, Missing, Unknown) and,
 * for managed resources, a sync status. A problem deep in the tree (a crash-looping Pod) is
 * also surfaced on every ancestor, so the path from the Application to the failure is visible
 * at a glance, and failures are listed so they can be jumped to.
 */
import type { GraphInput } from './argo-graph';

export type HealthRank = 0 | 1 | 2 | 3;
export type PodFacts = { state: string; ready: number; total: number; restarts: number };
export type NodeHealth = {
  /** The node's own status, normalized (Pods derive it from their state). */
  health: string;
  /** Worst status of the node and everything below it. */
  rollup: string;
  /** Unhealthy descendants (not counting the node itself). */
  problemsBelow: number;
  pod?: PodFacts;
  /** Why the node is unhealthy, when known. */
  reason: string;
};
export type GraphFailure = { id: string; kind: string; name: string; health: string; reason: string };
export type GraphSummary = { health: Record<string, number>; outOfSync: number; failures: GraphFailure[] };

const RANK: Record<string, HealthRank> = { Degraded: 3, Missing: 3, Progressing: 2, Suspended: 1, Unknown: 1, Healthy: 0 };
const POD_FAILURE = /CrashLoopBackOff|ImagePullBackOff|ErrImagePull|CreateContainerConfigError|CreateContainerError|InvalidImageName|OOMKilled|Error|Failed|Evicted/;

export const healthRank = (health: string) => RANK[health] ?? 0;
export const isProblem = (health: string) => healthRank(health) >= 2 || health === 'Missing';

/** "CrashLoopBackOff · 0/2 ready · 12 restarts" → structured Pod facts. */
export function podFacts(info = ''): PodFacts | undefined {
  const ready = /(\d+)\/(\d+) ready/.exec(info);
  if (!ready) return undefined;
  const restarts = /(\d+) restarts?/.exec(info);
  return { state: info.split(' · ')[0] || '', ready: Number(ready[1]), total: Number(ready[2]), restarts: restarts ? Number(restarts[1]) : 0 };
}

function ownHealth(node: GraphInput, pod?: PodFacts) {
  if (node.health) return node.health;
  if (pod) {
    if (POD_FAILURE.test(pod.state)) return 'Degraded';
    if (pod.state === 'Succeeded' || pod.state === 'Completed') return 'Healthy';
    return pod.ready === pod.total && pod.total > 0 ? 'Healthy' : 'Progressing';
  }
  return '';
}

export function analyzeGraph(nodes: GraphInput[]) {
  const children = new Map<string, GraphInput[]>();
  for (const node of nodes) if (node.parent) children.set(node.parent, [...(children.get(node.parent) || []), node]);
  const result = new Map<string, NodeHealth>();
  const visit = (node: GraphInput, depth = 0): NodeHealth => {
    const cached = result.get(node.id);
    if (cached) return cached;
    const pod = node.kind === 'Pod' ? podFacts(node.info) : undefined;
    const health = ownHealth(node, pod);
    let rollup = health;
    let problemsBelow = 0;
    if (depth < 12) {
      for (const child of children.get(node.id) || []) {
        const below = visit(child, depth + 1);
        problemsBelow += below.problemsBelow + (isProblem(below.health) ? 1 : 0);
        if (healthRank(below.rollup) > healthRank(rollup)) rollup = below.rollup;
      }
    }
    const reason = isProblem(health) ? (pod?.state && POD_FAILURE.test(pod.state) ? pod.state : node.info || health) : '';
    const entry = { health, rollup: rollup || health, problemsBelow, pod, reason };
    result.set(node.id, entry);
    return entry;
  };
  for (const node of nodes) visit(node);

  const summary: GraphSummary = { health: {}, outOfSync: 0, failures: [] };
  for (const node of nodes) {
    if (node.id === 'app' || node.more) continue;
    const entry = result.get(node.id)!;
    if (entry.health) summary.health[entry.health] = (summary.health[entry.health] || 0) + 1;
    if (node.sync === 'OutOfSync') summary.outOfSync += 1;
    if (isProblem(entry.health)) summary.failures.push({ id: node.id, kind: node.kind, name: node.name, health: entry.health, reason: entry.reason });
  }
  // Worst first: Degraded and Missing before Progressing; Pods (the root cause) before owners.
  summary.failures.sort((left, right) => healthRank(right.health) - healthRank(left.health) || Number(right.kind === 'Pod') - Number(left.kind === 'Pod') || left.name.localeCompare(right.name));
  return { nodes: result, summary };
}

/** Whether a node matches a health filter chip ('all', a health, or 'OutOfSync'). */
export function matchesFilter(filter: string, node: GraphInput, health?: NodeHealth) {
  if (filter === 'all') return true;
  if (filter === 'OutOfSync') return node.sync === 'OutOfSync';
  if (filter === 'problems') return Boolean(health && (isProblem(health.health) || health.problemsBelow > 0));
  return health?.health === filter;
}
