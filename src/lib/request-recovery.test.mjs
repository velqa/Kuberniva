import assert from 'node:assert/strict';
import { test } from 'node:test';
import { readDeadlineMs, shouldRecoverAfterResume, withRequestDeadline } from './request-recovery.ts';

test('an hour of sleep requires recovery even when the old watch says connected', () => {
  assert.equal(shouldRecoverAfterResume(3_600_000, 1_000, 3_601_000, false), true);
});

test('brief focus changes preserve a current connection', () => {
  assert.equal(shouldRecoverAfterResume(2_000, 10_000, 12_000, false), false);
});

test('a disconnected or stale connection is recovered without a long sleep', () => {
  assert.equal(shouldRecoverAfterResume(0, 10_000, 10_010, true), true);
  assert.equal(shouldRecoverAfterResume(0, 10_000, 130_000, false), true);
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
