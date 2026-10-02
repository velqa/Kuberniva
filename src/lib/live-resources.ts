export interface LiveRow {
  name: string;
  namespace?: string;
  uid?: string;
  resourceVersion?: string;
  cpuUsage?: string;
  memoryUsage?: string;
}

export type RowChange<T extends LiveRow> = { action: 'upsert' | 'delete'; object: T };
export const rowKey = (row: LiveRow) => `${row.namespace || ''}\u0000${row.name}`;

/** The watch updates this index; the UI receives one stable, ordered snapshot per batch. */
export class LiveResourceStore<T extends LiveRow> {
  private index = new Map<string, T>();
  private staging: Map<string, T> | null = null;
  private order: string[] = [];
  private orderDirty = true;

  constructor(rows: T[] = []) { this.replace(rows); }

  private merge(row: T, previous?: T): T {
    if (!previous || previous.uid !== row.uid) return row;
    if (row.resourceVersion && row.resourceVersion === previous.resourceVersion) return previous;
    return { ...row, cpuUsage: row.cpuUsage ?? previous.cpuUsage, memoryUsage: row.memoryUsage ?? previous.memoryUsage };
  }

  replace(rows: T[]) {
    const next = new Map<string, T>();
    for (const row of rows) next.set(rowKey(row), this.merge(row, this.index.get(rowKey(row))));
    this.index = next;
    this.orderDirty = true;
  }

  beginReset() { this.staging = new Map(); }
  stage(rows: T[]) {
    if (!this.staging) return;
    for (const row of rows) this.staging.set(rowKey(row), this.merge(row, this.index.get(rowKey(row))));
  }
  finishReset() {
    if (!this.staging) return false;
    this.index = this.staging;
    this.staging = null;
    this.orderDirty = true;
    return true;
  }
  abortReset() { this.staging = null; }

  apply(changes: RowChange<T>[]) {
    let changed = false;
    for (const { action, object } of changes) {
      const key = rowKey(object);
      const previous = this.index.get(key);
      if (action === 'delete') {
        // A delayed tombstone must never remove a replacement with the same name.
        if (previous && (!object.uid || previous.uid === object.uid)) {
          this.index.delete(key);
          this.orderDirty = true;
          changed = true;
        }
      } else {
        const next = this.merge(object, previous);
        if (next !== previous) {
          this.index.set(key, next);
          if (!previous) this.orderDirty = true;
          changed = true;
        }
      }
    }
    return changed;
  }

  applyMetrics(metrics: { name: string; namespace?: string; cpuUsage?: string; memoryUsage?: string }[]) {
    const incoming = new Map(metrics.map((metric) => [rowKey(metric), metric]));
    let changed = false;
    for (const [key, row] of this.index) {
      const metric = incoming.get(key);
      if (row.cpuUsage !== metric?.cpuUsage || row.memoryUsage !== metric?.memoryUsage) {
        this.index.set(key, { ...row, cpuUsage: metric?.cpuUsage, memoryUsage: metric?.memoryUsage });
        changed = true;
      }
    }
    return changed;
  }

  get(row: LiveRow) { return this.index.get(rowKey(row)); }
  rows(): T[] {
    if (this.orderDirty) {
      this.order = [...this.index.keys()].sort();
      this.orderDirty = false;
    }
    return this.order.map((key) => this.index.get(key)!);
  }
}

export function panePercent(value: number) { return Math.max(25, Math.min(70, value)); }
