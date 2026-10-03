import assert from 'node:assert/strict';
import { test } from 'node:test';
import { clampValue, widthFromDrag } from './drag-resize.ts';

test('drag values follow the pointer and stay within limits', () => {
  assert.equal(widthFromDrag({ clientX: 260 }, { value: 200, x: 200 }), 260);
  assert.equal(clampValue(widthFromDrag({ clientX: 900 }, { value: 200, x: 200 }), 140, 520), 520);
  assert.equal(clampValue(-5, 25, 70), 25);
});
