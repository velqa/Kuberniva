/**
 * A Svelte action for splitters and column edges: drag with the pointer, nudge with the
 * arrow keys (Home/End jump to the limits), double-click to reset. Values are clamped.
 *
 * Drags start from the size actually rendered and convert pointer travel into layout
 * pixels using the element's real scale, so the edge tracks the pointer 1:1 at any
 * interface zoom. Updates are applied at most once per animation frame.
 */
export type DragStart = {
  value: number;
  x: number;
  node: HTMLElement;
  /** Rendered width of the resized element in layout (CSS) pixels. */
  layoutWidth: number;
  /** Screen pixels per layout pixel (interface zoom). */
  scale: number;
};

export type DragResizeOptions = {
  value: number;
  min: number;
  max: number;
  reset: number;
  /** Keyboard step, in the same unit as `value`. */
  step?: number;
  /** The element whose width is being resized (defaults to the handle's parent). */
  target?: (node: HTMLElement) => HTMLElement | null;
  /** Upper bound computed when a drag starts (for example, the space other columns can give). */
  limit?: (node: HTMLElement, start: DragStart) => number;
  /** Runs once when a drag starts, before the first update (e.g. freeze neighbouring columns). */
  onStart?: (node: HTMLElement, start: DragStart) => void;
  /** Maps a pointer position to a value. */
  fromPointer: (event: { clientX: number }, start: DragStart) => number;
  onChange: (value: number) => void;
  onCommit?: (value: number) => void;
};

export function clampValue(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
}

/** Screen pixels per layout pixel for an element (1 without zoom). */
export function renderScale(element: HTMLElement) {
  const layout = element.offsetWidth;
  const rendered = element.getBoundingClientRect().width;
  return layout > 0 && rendered > 0 ? rendered / layout : 1;
}

export function dragResize(node: HTMLElement, initial: DragResizeOptions) {
  let options = initial;
  let stopDrag: (() => void) | undefined;
  const set = (value: number, commit = false) => {
    const next = clampValue(value, options.min, options.max);
    options.onChange(next);
    if (commit) options.onCommit?.(next);
  };
  const onPointerDown = (event: PointerEvent) => {
    if (event.button !== 0) return;
    event.preventDefault();
    event.stopPropagation();
    stopDrag?.();
    const target = options.target?.(node) || node.parentElement || node;
    const start: DragStart = {
      value: options.value,
      x: event.clientX,
      node,
      layoutWidth: target.offsetWidth,
      scale: renderScale(target),
    };
    options.onStart?.(node, start);
    const max = Math.min(options.max, options.limit ? options.limit(node, start) : Number.POSITIVE_INFINITY);
    let latest = options.value;
    let pendingX: number | null = null;
    let frame = 0;
    try { node.setPointerCapture(event.pointerId); } catch { /* synthetic pointers */ }
    document.body.classList.add('is-resizing');
    const apply = () => {
      frame = 0;
      if (pendingX === null) return;
      latest = clampValue(options.fromPointer({ clientX: pendingX }, start), options.min, max);
      pendingX = null;
      options.onChange(latest);
    };
    const move = (pointer: PointerEvent) => {
      pendingX = pointer.clientX;
      if (!frame) frame = requestAnimationFrame(apply);
    };
    const stop = (pointer?: PointerEvent) => {
      if (pointer) pendingX = pointer.clientX;
      if (frame) cancelAnimationFrame(frame);
      apply();
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', stop);
      window.removeEventListener('pointercancel', stop);
      document.body.classList.remove('is-resizing');
      options.onCommit?.(latest);
      stopDrag = undefined;
    };
    stopDrag = () => stop();
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', stop);
    window.addEventListener('pointercancel', stop);
  };
  const onKeyDown = (event: KeyboardEvent) => {
    const step = options.step ?? 2;
    const base = options.value || (options.target?.(node) || node.parentElement || node).offsetWidth;
    const next = event.key === 'ArrowLeft' ? base - step
      : event.key === 'ArrowRight' ? base + step
        : event.key === 'Home' ? options.min
          : event.key === 'End' ? options.max : null;
    if (next === null) return;
    event.preventDefault();
    set(next, true);
  };
  const onDoubleClick = (event: MouseEvent) => {
    event.stopPropagation();
    set(options.reset, true);
  };
  node.addEventListener('pointerdown', onPointerDown);
  node.addEventListener('keydown', onKeyDown);
  node.addEventListener('dblclick', onDoubleClick);
  return {
    update(next: DragResizeOptions) {
      options = next;
    },
    destroy() {
      stopDrag?.();
      node.removeEventListener('pointerdown', onPointerDown);
      node.removeEventListener('keydown', onKeyDown);
      node.removeEventListener('dblclick', onDoubleClick);
    },
  };
}

/** Pointer position as a percentage of the handle's container (a split grid). */
export function percentOfContainer(event: { clientX: number }, start: { node: HTMLElement }) {
  const bounds = (start.node.parentElement || start.node).getBoundingClientRect();
  return bounds.width ? (100 * (event.clientX - bounds.left)) / bounds.width : 0;
}

/** Starting width plus horizontal pointer travel (sidebars), corrected for zoom. */
export function widthFromDrag(event: { clientX: number }, start: { value: number; x: number; scale?: number }) {
  return start.value + (event.clientX - start.x) / (start.scale || 1);
}

/** A column edge: the rendered width plus pointer travel in layout pixels. */
export function columnFromDrag(event: { clientX: number }, start: DragStart) {
  return start.layoutWidth + (event.clientX - start.x) / start.scale;
}

/**
 * How wide a column may grow in a table that does not scroll: its own width plus what the
 * columns to its right can still give up before reaching their minimums (`data-min`).
 * Columns to the left are frozen for the drag, so the edge follows the pointer.
 */
export function columnGrowthLimit(node: HTMLElement, start: DragStart) {
  const cell = node.parentElement;
  const row = cell?.parentElement;
  if (!cell || !row) return Number.POSITIVE_INFINITY;
  const cells = Array.from(row.children) as HTMLElement[];
  let slack = 0;
  for (const sibling of cells.slice(cells.indexOf(cell) + 1)) {
    if (!sibling.dataset.min || sibling.offsetParent === null) continue;
    slack += Math.max(0, sibling.offsetWidth - Number(sibling.dataset.min));
  }
  return start.layoutWidth + slack;
}

/** Rendered widths (layout px) of the resizable columns left of a handle's cell, by data-column key. */
export function leftColumnWidths(node: HTMLElement) {
  const cell = node.parentElement;
  const row = cell?.parentElement;
  const widths: Record<string, number> = {};
  if (!cell || !row) return widths;
  const cells = Array.from(row.children) as HTMLElement[];
  for (const sibling of cells.slice(0, cells.indexOf(cell))) {
    if (sibling.dataset.column && sibling.offsetParent !== null) widths[sibling.dataset.column] = sibling.offsetWidth;
  }
  return widths;
}
