/**
 * A Svelte action for splitters and column edges: drag with the pointer, nudge with the
 * arrow keys (Home/End jump to the limits), double-click to reset. Values are clamped.
 */
export type DragResizeOptions = {
  value: number;
  min: number;
  max: number;
  reset: number;
  /** Keyboard step, in the same unit as `value`. */
  step?: number;
  /** Maps a pointer position to a value; `start` is the value when the drag began. */
  fromPointer: (event: PointerEvent, start: { value: number; x: number; node: HTMLElement }) => number;
  onChange: (value: number) => void;
  onCommit?: (value: number) => void;
};

export function clampValue(value: number, min: number, max: number) {
  return Math.min(max, Math.max(min, value));
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
    const start = { value: options.value, x: event.clientX, node };
    let latest = options.value;
    try { node.setPointerCapture(event.pointerId); } catch { /* synthetic pointers */ }
    document.body.classList.add('is-resizing');
    const move = (pointer: PointerEvent) => {
      latest = clampValue(options.fromPointer(pointer, start), options.min, options.max);
      options.onChange(latest);
    };
    const stop = () => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', stop);
      window.removeEventListener('pointercancel', stop);
      document.body.classList.remove('is-resizing');
      options.onCommit?.(latest);
      stopDrag = undefined;
    };
    stopDrag = stop;
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', stop);
    window.addEventListener('pointercancel', stop);
  };
  const onKeyDown = (event: KeyboardEvent) => {
    const step = options.step ?? 2;
    const next = event.key === 'ArrowLeft' ? options.value - step
      : event.key === 'ArrowRight' ? options.value + step
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
export function percentOfContainer(event: PointerEvent, start: { node: HTMLElement }) {
  const bounds = (start.node.parentElement || start.node).getBoundingClientRect();
  return bounds.width ? (100 * (event.clientX - bounds.left)) / bounds.width : 0;
}

/** Starting width plus horizontal pointer travel (column edges, sidebars). */
export function widthFromDrag(event: PointerEvent, start: { value: number; x: number }) {
  return start.value + (event.clientX - start.x);
}
