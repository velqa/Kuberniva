/** Rows of a keyed live feed (Events, Argo CD Applications), updated from batched changes. */
export type FeedChange<T> = { action: 'upsert' | 'delete'; key: string; item?: T };

export class KeyedFeedStore<T> {
  private rows = new Map<string, T>();

  reset(items: { key: string; item: T }[]) {
    this.rows = new Map(items.map(({ key, item }) => [key, item]));
  }

  apply(changes: FeedChange<T>[]) {
    let changed = false;
    for (const change of changes) {
      if (change.action === 'delete') changed = this.rows.delete(change.key) || changed;
      else if (change.item !== undefined) {
        this.rows.set(change.key, change.item);
        changed = true;
      }
    }
    return changed;
  }

  values() {
    return [...this.rows.values()];
  }

  get size() {
    return this.rows.size;
  }
}

/** Newest events first, capped so a noisy cluster cannot grow the list without bound. */
export function newestEvents<T extends { lastObserved?: string }>(events: T[], limit = 500) {
  const time = (event: T) => Date.parse(event.lastObserved || '') || 0;
  return [...events].sort((left, right) => time(right) - time(left)).slice(0, limit);
}
