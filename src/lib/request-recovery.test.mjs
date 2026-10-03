import assert from 'node:assert/strict';
import { test } from 'node:test';
import { readDeadlineMs, resumeAction, withRequestDeadline } from './request-recovery.ts';

const resume = (overrides) => resumeAction({ view: 'Workloads', watchStatus: 'connected', connectionFailed: false, snapshotAgeMs: 0, ...overrides });

test('coming back to a healthy live list does nothing, however long the window was away', () => {
  assert.equal(resume({}), 'none');
  assert.equal(resume({ view: 'Resources', watchStatus: 'reconnecting' }), 'none');
});

test('a watch that failed to start is restarted, and a real connection failure reconnects', () => {
  assert.equal(resume({ watchStatus: 'error' }), 'restart-watch');
  assert.equal(resume({ connectionFailed: true }), 'reconnect');
  assert.equal(resume({ view: 'Overview', connectionFailed: true }), 'reconnect');
});

test('snapshot views refresh quietly only when their data is old and not streaming', () => {
  assert.equal(resume({ view: 'Overview', watchStatus: 'connected', snapshotAgeMs: 3_600_000 }), 'none');
  assert.equal(resume({ view: 'Overview', watchStatus: 'idle', snapshotAgeMs: 30_000 }), 'none');
  assert.equal(resume({ view: 'Overview', watchStatus: 'idle', snapshotAgeMs: 61_000 }), 'quiet-refresh');
  assert.equal(resume({ view: 'Events', watchStatus: 'idle', snapshotAgeMs: 3_600_000 }), 'quiet-refresh');
  assert.equal(resume({ view: 'Settings', watchStatus: 'idle', snapshotAgeMs: 3_600_000 }), 'none');
});

test('a hung native request rejects and allows the loading workflow to finish', async () => {
  await assert.rejects(withRequestDeadline(new Promise(() => {}), 'Reading pods', 10), /Reading pods timed out/);
});

test('successful and rejected requests preserve their actual result', async () => {
  assert.equal(await withRequestDeadline(Promise.resolve('pods loaded'), 'Reading pods', 100), 'pods loaded');
  await assert.rejects(withRequestDeadline(Promise.reject(new Error('Forbidden')), 'Reading pods', 100), /Forbidden/);
});

test('SSO clusters outlast the backend sign-in window; others keep short deadlines', () => {
  assert.equal(readDeadlineMs(false, 'OIDC / exec'), 310_000);
  assert.equal(readDeadlineMs(true, 'OIDC provider'), 310_000);
  assert.equal(readDeadlineMs(false, 'Bearer token'), 40_000);
  assert.equal(readDeadlineMs(true, undefined), 95_000);
});
