import assert from 'node:assert/strict';
import { test } from 'node:test';
import { clampValue, columnFromDrag, widthFromDrag } from './drag-resize.ts';

test('a column edge tracks the pointer 1:1 in layout pixels at any zoom', () => {
  const at = (scale, travel) => columnFromDrag({ clientX: 500 + travel }, { layoutWidth: 200, x: 500, scale, value: 0, node: null });
  assert.equal(at(1, 120), 320);
  // At 125% zoom, 125 screen pixels are 100 layout pixels.
  assert.equal(at(1.25, 125), 300);
  assert.equal(at(0.8, -80), 100);
});

test('drags start from the rendered width, not a stale saved width', () => {
  // A saved 900 px preference rendered at 300 px must not jump to 900 on the first move.
  assert.equal(columnFromDrag({ clientX: 10 }, { layoutWidth: 300, x: 10, scale: 1, value: 900, node: null }), 300);
});

test('sidebar drags and limits', () => {
  assert.equal(widthFromDrag({ clientX: 260 }, { value: 200, x: 200 }), 260);
  assert.equal(widthFromDrag({ clientX: 325 }, { value: 200, x: 200, scale: 1.25 }), 300);
  assert.equal(clampValue(-5, 25, 70), 25);
});
