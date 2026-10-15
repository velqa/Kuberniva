import assert from 'node:assert/strict';
import test from 'node:test';
import { columnTracks } from './column-tracks.ts';

const columns = [
  { key: 'name', width: 'minmax(170px, 2fr)' },
  { key: 'status', width: '96px' },
  { key: 'node', width: 'minmax(110px, 1.1fr)' },
];

test('tables nobody has resized keep their natural tracks', () => {
  assert.deepEqual(columnTracks(columns).map((column) => column.width), columns.map((column) => column.width));
});

test('a drag uses exact pixels so the edge follows the pointer', () => {
  const tracks = columnTracks(columns, { name: 300 }, true);
  assert.deepEqual(tracks.map((column) => column.width), ['300px', '96px', 'minmax(110px, 1.1fr)']);
  assert.equal(tracks[0].min, 300);
});

test('after a drag every column scales with the window in proportion, down to its minimum', () => {
  const tracks = columnTracks(columns, { name: 300, status: 120, node: 200, __ref: 640 });
  assert.deepEqual(tracks.map((column) => column.width), ['minmax(max(170px, 46.875%), 300fr)', 'minmax(max(96px, 18.75%), 120fr)', 'minmax(max(110px, 31.25%), 200fr)']);
  assert.deepEqual(tracks.map((column) => column.min), [170, 96, 110]);
  // A column dragged narrower than its natural minimum keeps the narrower width as its floor.
  assert.equal(columnTracks(columns, { name: 120 })[0].width, 'minmax(120px, 120fr)');
});

test('columns added after the last drag join in proportion to their minimum', () => {
  const tracks = columnTracks([...columns, { key: 'cpu', width: 'minmax(72px, .7fr)' }, { key: 'age', width: '44px' }], { name: 300, status: 120, node: 200 });
  assert.deepEqual(tracks.slice(3).map((column) => column.width), ['minmax(72px, 72fr)', '44px']);
});
