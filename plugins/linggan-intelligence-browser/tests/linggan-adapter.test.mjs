import assert from 'node:assert/strict';
import test from 'node:test';

import {
  LINGGAN_LOCAL_ORIGIN,
  attemptStartIsAccepted,
  formatLingganRuntimeNotice,
  isTerminalLocalDeliveryResult,
  readLingganLocalReadiness,
  taskCreationIsAccepted,
  unavailableLingganStats,
} from '../src/linggan/adapter.js';

test('runtime notice never pretends that automatic scheduling already exists', () => {
  assert.match(formatLingganRuntimeNotice(), /本机可靠队列/);
  assert.match(formatLingganRuntimeNotice(), /自动调度尚未启动/);
});

test('unread Linggan stats stay explicitly unavailable instead of becoming zero', () => {
  assert.deepEqual(unavailableLingganStats(), {
    success: true,
    statsState: 'not_connected',
    notes: null,
    comments: null,
    authors: null,
    source: 'linggan_data_not_read',
  });
});

test('only created or replayed tasks and started or replayed attempts may continue to submission', () => {
  assert.equal(taskCreationIsAccepted({ ok: true, payload: { outcome: 'created' } }), true);
  assert.equal(taskCreationIsAccepted({ ok: true, payload: { outcome: 'replay' } }), true);
  assert.equal(taskCreationIsAccepted({ ok: true, payload: { outcome: 'conflict' } }), false);
  assert.equal(attemptStartIsAccepted({ ok: true, payload: { outcome: 'started' } }), true);
  assert.equal(attemptStartIsAccepted({ ok: true, payload: { outcome: 'replay' } }), true);
  assert.equal(attemptStartIsAccepted({ ok: true, payload: { outcome: 'conflict' } }), false);
  assert.equal(isTerminalLocalDeliveryResult({ ok: false, status: 409, payload: { code: 'task_spec_conflict' } }), true);
});

test('local readiness probes only Linggan loopback without credentials', async () => {
  let received = null;
  const result = await readLingganLocalReadiness(async (url, options) => {
    received = { url, options };
    return { ok: true };
  });
  assert.equal(result.connected, true);
  assert.equal(received.url, `${LINGGAN_LOCAL_ORIGIN}/health`);
  assert.equal(received.options.credentials, 'omit');
  assert.match(result.message, /Browser Producer Runtime/);
});
