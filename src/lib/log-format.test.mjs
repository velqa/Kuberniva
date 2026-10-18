import assert from 'node:assert/strict';
import test from 'node:test';
import { bracketDepth, groupLogLines, prettyJson } from './log-format.ts';

const at = (ms, text) => `2026-10-15T10:00:00.${String(ms).padStart(3, '0')}000000Z ${text}`;

test('a pretty-printed dict is one entry instead of one line per field', () => {
  const entries = groupLogLines([
    at(1, 'INFO request received'),
    at(2, "payload = {"),
    at(2, "    'user': 'ana',"),
    at(2, "    'items': [1, 2],"),
    at(2, '}'),
    at(3, 'INFO done'),
  ]);
  assert.equal(entries.length, 3);
  assert.equal(entries[1].text, "payload = {\n    'user': 'ana',\n    'items': [1, 2],\n}");
  assert.equal(entries[1].lines, 4);
  assert.equal(entries[2].text, 'INFO done');
});

test('a dict printed on its own starts with "{" and still groups', () => {
  const entries = groupLogLines([at(1, '{'), at(1, '  "status": "ok",'), at(1, '  "nested": {"a": 1}'), at(1, '}'), at(2, 'next')]);
  assert.deepEqual(entries.map((entry) => entry.lines), [4, 1]);
});

test('stack traces stay with the error that caused them, and keep its level', () => {
  const entries = groupLogLines([
    at(1, 'ERROR handler failed'),
    at(1, 'Traceback (most recent call last):'),
    at(1, '  File "app.py", line 3, in <module>'),
    at(1, 'ValueError: bad'),
    at(2, 'java.lang.IllegalStateException: boom'),
    at(2, '\tat com.acme.Api.call(Api.java:42)'),
    at(2, 'Caused by: java.io.IOException'),
  ]);
  assert.equal(entries[0].level, 'error');
  // Traceback headers are their own line; the indented frame joins it.
  assert.deepEqual(entries.map((entry) => entry.lines), [1, 2, 1, 3]);
});

test('brackets inside strings do not open an entry', () => {
  assert.equal(bracketDepth('msg="{ not json" level=info'), 0);
  assert.equal(groupLogLines([at(1, 'a "{" b'), at(2, 'next')]).length, 2);
});

test('single-line JSON can be expanded; anything else is left alone', () => {
  assert.equal(prettyJson('{"a":1,"b":[2]}'), '{\n  "a": 1,\n  "b": [\n    2\n  ]\n}');
  assert.equal(prettyJson('event {"a":1}'), 'event {\n  "a": 1\n}');
  assert.equal(prettyJson('plain text'), 'plain text');
  assert.equal(prettyJson("{'user': 'ana', 'ok': True, 'n': None, 'tags': ['a', \"it's\"]}"), '{\n  "user": "ana",\n  "ok": true,\n  "n": null,\n  "tags": [\n    "a",\n    "it\'s"\n  ]\n}');
  assert.equal(prettyJson('{not valid'), '{not valid');
});

test('lists, tuples, and indented blocks (YAML, nested output) group the same way', () => {
  const entries = groupLogLines([
    at(1, 'items = ['),
    at(1, "    {'id': 1},"),
    at(1, "    {'id': 2},"),
    at(1, ']'),
    at(2, 'point = (1,'),
    at(2, '         2)'),
    at(3, 'config:'),
    at(3, '  replicas: 3'),
    at(3, '  image: api:1.4'),
    at(4, 'next'),
  ]);
  assert.deepEqual(entries.map((entry) => entry.lines), [4, 2, 3, 1]);
});

test('search highlights every match in place and numbers them for next and previous', async () => {
  const { highlightMatches } = await import('./log-format.ts');
  const { pieces, total } = highlightMatches(['INFO request ok', 'no hit', 'Request then REQUEST'], 'request');
  assert.equal(total, 3);
  assert.deepEqual(pieces[0], [{ text: 'INFO ' }, { text: 'request', match: 0 }, { text: ' ok' }]);
  assert.equal(pieces[1], null);
  assert.deepEqual(pieces[2], [{ text: 'Request', match: 1 }, { text: ' then ' }, { text: 'REQUEST', match: 2 }]);
  assert.equal(highlightMatches(['abc'], '  ').total, 0);
});
