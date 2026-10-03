import assert from 'node:assert/strict';
import { test } from 'node:test';
import { KeyedFeedStore, newestEvents } from './live-feed.ts';

test('a keyed feed applies upserts and deletes from batches', () => {
  const store = new KeyedFeedStore();
  store.reset([{ key: 'a', item: { name: 'a', v: 1 } }, { key: 'b', item: { name: 'b' } }]);
  assert.equal(store.apply([{ action: 'upsert', key: 'a', item: { name: 'a', v: 2 } }, { action: 'delete', key: 'b' }, { action: 'upsert', key: 'c', item: { name: 'c' } }]), true);
  assert.deepEqual(store.values().map((row) => row.name).sort(), ['a', 'c']);
  assert.equal(store.values().find((row) => row.name === 'a').v, 2);
  assert.equal(store.apply([{ action: 'delete', key: 'missing' }]), false);
});

test('events are newest first and capped', () => {
  const events = Array.from({ length: 600 }, (_, index) => ({ name: `e${index}`, lastObserved: new Date(Date.UTC(2026, 0, 1, 0, 0, index)).toISOString() }));
  const newest = newestEvents(events);
  assert.equal(newest.length, 500);
  assert.equal(newest[0].name, 'e599');
});
