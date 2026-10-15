/**
 * CSS grid tracks for tables with draggable columns.
 *
 * While a column is being dragged, dragged widths are exact pixels so the edge follows the
 * pointer. Once the drag ends, every measured width W becomes a share of the table width at
 * that moment (`REF_WIDTH`): `minmax(max(floor, W/REF %), W fr)`. At the same window size the
 * columns are exactly as dragged; when the window grows or shrinks, all columns scale
 * together and keep their shape, down to each column's own minimum, after which the table
 * scrolls sideways.
 */
export const REF_WIDTH = '__ref';
export type TrackColumn = { key: string; width: string };
export type Track<T> = T & { natural: string; min: number };

/** Smallest width a grid track can take, for computing when a table needs to scroll. */
export function trackMinimum(width: string) {
  const fixed = width.match(/^(\d+(?:\.\d+)?)px$/);
  if (fixed) return Number(fixed[1]);
  const minimum = width.match(/^minmax\((\d+(?:\.\d+)?)px/);
  return minimum ? Math.max(48, Number(minimum[1])) : 48;
}

export function columnTracks<T extends TrackColumn>(columns: T[], widths: Record<string, number> = {}, dragging = false): Track<T>[] {
  const sized = columns.some((column) => widths[column.key] > 0);
  return columns.map((column) => {
    const natural = column.width;
    const floor = trackMinimum(natural);
    const width = widths[column.key];
    if (!sized) return { ...column, natural, min: floor, width: natural };
    if (dragging) return width ? { ...column, natural, min: width, width: `${width}px` } : { ...column, natural, min: floor, width: natural };
    if (width) {
      const min = Math.min(width, floor);
      const reference = widths[REF_WIDTH];
      const share = reference > 0 ? `max(${min}px, ${+((100 * width) / reference).toFixed(4)}%)` : `${min}px`;
      return { ...column, natural, min, width: `minmax(${share}, ${width}fr)` };
    }
    // A column that appeared after the last drag (a new server column, say) joins in
    // proportion to its own minimum; fixed-width columns stay fixed.
    if (/^\d+(?:\.\d+)?px$/.test(natural)) return { ...column, natural, min: floor, width: natural };
    return { ...column, natural, min: floor, width: `minmax(${floor}px, ${floor}fr)` };
  });
}

/** Rendered widths (layout px) of every resizable column in a header row, by data-column key, plus the row's content width. */
export function measureColumns(row: HTMLElement) {
  const style = getComputedStyle(row);
  const widths: Record<string, number> = { [REF_WIDTH]: Math.round(row.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight)) };
  for (const cell of Array.from(row.children) as HTMLElement[]) {
    if (cell.dataset.column && cell.offsetParent !== null && cell.querySelector('.column-resizer')) widths[cell.dataset.column] = Math.round(cell.offsetWidth);
  }
  return widths;
}

/** Keys of the columns right of a handle's cell; they give up space while it is dragged. */
export function columnsRightOf(handle: HTMLElement) {
  const cell = handle.parentElement;
  const row = cell?.parentElement;
  if (!cell || !row) return [];
  const cells = Array.from(row.children) as HTMLElement[];
  return cells.slice(cells.indexOf(cell) + 1).map((sibling) => sibling.dataset.column).filter((key): key is string => Boolean(key));
}
