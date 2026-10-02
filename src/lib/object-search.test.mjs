import assert from 'node:assert/strict';
import { test } from 'node:test';
import { matchesSearch, searchTokens } from './object-search.ts';

const pod = (name, status, namespace = 'team-01', node = 'worker-1') => [name, namespace, status, node];
const pods = [pod('api-1', 'Running'), pod('job-7', 'Failed'), pod('job-8', 'Failed', 'team-02'), pod('web-1', 'Pending')];
const find = (query) => pods.filter((fields) => matchesSearch(searchTokens(query), fields)).map((fields) => fields[0]);

test('status matches regardless of case', () => {
  assert.deepEqual(find('failed'), ['job-7', 'job-8']);
  assert.deepEqual(find('FAILED'), ['job-7', 'job-8']);
  assert.deepEqual(find('running'), ['api-1']);
});

test('every word must match, across different fields', () => {
  assert.deepEqual(find('failed team-02'), ['job-8']);
  assert.deepEqual(find('  pending   web '), ['web-1']);
  assert.deepEqual(find('worker-1 running'), ['api-1']);
});

test('an empty search keeps everything and missing fields are ignored', () => {
  assert.equal(find('').length, 4);
  assert.equal(matchesSearch(searchTokens('undefined'), ['a', undefined, null]), false);
});
