import assert from 'node:assert/strict';
import { test } from 'node:test';
import { parseInline, parseReleaseNotes } from './release-notes.ts';

const notes = `Kuberniva 0.5.3 stays connected.

## ✨ What's new

**Background updates**
- Live lists keep **streaming** in the background
- Quit with \`⌘Q\`

## 🔄 Updating

- **In the app:** Settings
Runs natively on Apple Silicon.

DMG SHA-256: \`abc\``;

test('release notes become headings, category labels, and lists without Markdown markers', () => {
  const blocks = parseReleaseNotes(notes);
  assert.deepEqual(blocks.map((block) => block.type), ['paragraph', 'heading', 'label', 'list']);
  assert.equal(blocks[1].text, "✨ What's new");
  assert.equal(blocks[2].text, 'Background updates');
  assert.deepEqual(blocks[3].items[0], [{ text: 'Live lists keep ' }, { text: 'streaming', bold: true }, { text: ' in the background' }]);
  assert.deepEqual(blocks[3].items[1][1], { text: '⌘Q', code: true });
});

test('install instructions and checksums are left out in the app', () => {
  const text = JSON.stringify(parseReleaseNotes(notes));
  assert.ok(!text.includes('Updating'));
  assert.ok(!text.includes('SHA-256'));
  assert.ok(!text.includes('Apple Silicon'));
});

test('markup in notes stays text', () => {
  assert.deepEqual(parseInline('<img src=x onerror=alert(1)> **ok**'), [{ text: '<img src=x onerror=alert(1)> ' }, { text: 'ok', bold: true }]);
});
