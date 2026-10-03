/**
 * Left-to-right layout for an Argo CD application graph: the app, its managed resources,
 * then live children (ReplicaSets, Pods, Jobs, EndpointSlices). Leaves take consecutive
 * rows; each parent is centred on its children. Long sibling lists collapse into a
 * "+N more" node so a 50-replica Deployment stays readable.
 */
export type GraphInput = {
  id: string;
  parent: string | null;
  kind: string;
  name: string;
  namespace?: string;
  health?: string;
  sync?: string;
  info?: string;
  managed?: boolean;
  /** For "+N more" nodes: how many siblings are hidden. */
  more?: number;
};
export type GraphNode = GraphInput & { x: number; y: number; depth: number };
export type GraphEdge = { from: string; to: string; path: string; tone: string };
export type GraphOptions = { columnWidth: number; columnGap: number; rowHeight: number; rowGap: number; maxChildren: number };

export const DEFAULT_GRAPH_OPTIONS: GraphOptions = { columnWidth: 232, columnGap: 64, rowHeight: 54, rowGap: 12, maxChildren: 8 };

export function layoutGraph(input: GraphInput[], expanded: ReadonlySet<string> = new Set(), options: GraphOptions = DEFAULT_GRAPH_OPTIONS) {
  const byId = new Map(input.map((node) => [node.id, node]));
  const children = new Map<string, GraphInput[]>();
  const roots: GraphInput[] = [];
  for (const node of input) {
    if (node.parent && byId.has(node.parent)) children.set(node.parent, [...(children.get(node.parent) || []), node]);
    else roots.push(node);
  }
  const placed: GraphNode[] = [];
  let row = 0;
  const step = options.rowHeight + options.rowGap;
  const place = (node: GraphInput, depth: number): number => {
    let kids = children.get(node.id) || [];
    // The application's own resources always show; long lists of live children collapse.
    if (node.parent && kids.length > options.maxChildren && !expanded.has(node.id)) {
      const shown = kids.slice(0, options.maxChildren - 1);
      const kind = kids.every((kid) => kid.kind === kids[0].kind) ? kids[0].kind : 'object';
      kids = [...shown, { id: `${node.id}::more`, parent: node.id, kind, name: `+${kids.length - shown.length} more`, more: kids.length - shown.length }];
    }
    let y: number;
    if (!kids.length) {
      y = row * step;
      row += 1;
    } else {
      const ys = kids.map((child) => place(child, depth + 1));
      y = (ys[0] + ys[ys.length - 1]) / 2;
    }
    placed.push({ ...node, depth, y, x: depth * (options.columnWidth + options.columnGap) });
    return y;
  };
  for (const root of roots) place(root, 0);
  const positions = new Map(placed.map((node) => [node.id, node]));
  const edges: GraphEdge[] = [];
  for (const node of placed) {
    const parent = node.parent ? positions.get(node.parent) : undefined;
    if (!parent) continue;
    const x1 = parent.x + options.columnWidth;
    const y1 = parent.y + options.rowHeight / 2;
    const x2 = node.x;
    const y2 = node.y + options.rowHeight / 2;
    const bend = (x2 - x1) / 2;
    edges.push({ from: parent.id, to: node.id, path: `M ${x1} ${y1} C ${x1 + bend} ${y1}, ${x2 - bend} ${y2}, ${x2} ${y2}`, tone: node.health || '' });
  }
  const depth = Math.max(0, ...placed.map((node) => node.depth));
  return {
    nodes: placed.sort((left, right) => left.depth - right.depth || left.y - right.y),
    edges,
    width: (depth + 1) * options.columnWidth + depth * options.columnGap,
    height: Math.max(1, row) * step - options.rowGap,
  };
}
