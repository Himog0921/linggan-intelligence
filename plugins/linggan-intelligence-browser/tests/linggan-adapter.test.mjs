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

test('local readiness accepts only the ready local trusted producer contract without credentials', async () => {
  let received = null;
  const result = await readLingganLocalReadiness(async (url, options) => {
    received = { url, options };
    return {
      ok: true,
      json: async () => ({
        service: 'linggan-local-web',
        listener: 'loopback-only',
        dataState: 'LOCAL_TRUSTED_PRODUCER',
        database: { state: 'READY', schema: 'LOCAL_003_SCHEMA_READY' },
        routes: {
          localProducer: {
            taskCreation: '/api/local/producer/tasks',
            attemptStart: '/api/local/producer/runtime-attempts',
            submission: '/api/local/producer/runtime-submissions',
          },
        },
      }),
    };
  });
  assert.equal(result.connected, true);
  assert.equal(result.reachable, true);
  assert.deepEqual(result.producerRoutes, {
    taskCreation: '/api/local/producer/tasks',
    attemptStart: '/api/local/producer/runtime-attempts',
    submission: '/api/local/producer/runtime-submissions',
  });
  assert.equal(received.url, `${LINGGAN_LOCAL_ORIGIN}/health`);
  assert.equal(received.options.credentials, 'omit');
  assert.match(result.message, /LOCAL_TRUSTED_PRODUCER/);
});

test('local readiness does not treat an older local read projection as a ready producer runtime', async () => {
  const result = await readLingganLocalReadiness(async () => ({
    ok: true,
    json: async () => ({
      service: 'linggan-local-web',
      listener: 'loopback-only',
      dataState: 'LOCAL_DISCOVERY_READ_PROJECTION',
      database: { state: 'READY', schema: 'LOCAL_001_SCHEMA_READY' },
      routes: { localProducer: { taskCreation: '/api/local/discovery-packages' } },
    }),
  }));
  assert.equal(result.connected, false);
  assert.equal(result.reachable, false);
  assert.match(result.message, /可访问/);
});

test('local readiness refuses a partial producer route bundle instead of inventing a delivery path', async () => {
  const result = await readLingganLocalReadiness(async () => ({
    ok: true,
    json: async () => ({
      service: 'linggan-local-web', listener: 'loopback-only', dataState: 'LOCAL_TRUSTED_PRODUCER',
      database: { state: 'READY', schema: 'LOCAL_003_SCHEMA_READY' },
      routes: { localProducer: { taskCreation: '/api/local/producer/tasks' } },
    }),
  }));
  assert.equal(result.connected, false);
  assert.equal(result.reachable, false);
  assert.equal(result.producerRoutes, undefined);
});
