import assert from 'node:assert/strict';
import test from 'node:test';

import { readLingganLocalReadiness } from '../src/linggan/adapter.js';

const originalChrome = globalThis.chrome;
globalThis.chrome = {
  runtime: {
    onMessage: { addListener: () => {} },
    getManifest: () => ({ version: '0.0.0-test' }),
  },
  tabs: { query: async () => [], sendMessage: async () => null, create: async () => {} },
  storage: { local: { get: async () => ({}), set: async () => {} } },
};

const { flushLocalOutboxOnce } = await import('../src/linggan/background.js');
globalThis.chrome = originalChrome;

function entry() {
  return {
    submissionId: 'submission-1', taskSpec: { task: 'synthetic' }, attempt: { attempt: 'synthetic' },
    contractVersion: 'linggan.producer.capture-package.v1', producerInstanceId: 'producer-1',
    taskId: 'task-1', attemptId: 'attempt-1', capturePackage: { package: 'synthetic' },
  };
}

function memoryOutbox() {
  const calls = [];
  return {
    calls,
    due: async () => [entry()],
    markInFlight: async (id) => calls.push(['in_flight', id]),
    acknowledge: async (id, receipt) => calls.push(['acknowledged', id, receipt]),
    terminal: async (id, reason) => calls.push(['terminal', id, reason]),
    retry: async (id, reason) => calls.push(['retry', id, reason]),
    pendingCount: async () => 0,
  };
}

test('full Producer runtime health enables an exact route-bundle flush through receipt', async () => {
  const outbox = memoryOutbox();
  const requested = [];
  const readReadiness = () => readLingganLocalReadiness(async () => ({
    ok: true,
    json: async () => ({
      service: 'linggan-local-web',
      listener: 'loopback-only',
      dataState: 'LINGGAN_BROWSER_PRODUCER_RUNTIME',
      database: { state: 'READY', schema: 'PLUGIN_RUNTIME_002_SCHEMA_READY' },
      routes: {
        localProducer: {
          taskCreation: '/api/local/producer/tasks',
          attemptStart: '/api/local/producer/runtime-attempts',
          submission: '/api/local/producer/runtime-submissions',
          mediaAcquisitionClaim: '/api/local/producer/media-acquisitions/claim',
        },
      },
    }),
  }));
  const readiness = await readReadiness();
  assert.equal(readiness.connected, true);
  assert.equal(readiness.reachable, true);
  await flushLocalOutboxOnce({
    outbox,
    mediaOutbox: { pendingCount: async () => 0 },
    readReadiness: async () => readiness,
    post: async (path) => {
      requested.push(path);
      if (path === '/api/local/producer/tasks') return { ok: true, status: 200, payload: { outcome: 'created' } };
      if (path === '/api/local/producer/runtime-attempts') return { ok: true, status: 200, payload: { outcome: 'started' } };
      if (path === '/api/local/producer/runtime-submissions') return { ok: true, status: 200, payload: { delivery: 'acknowledged' } };
      throw new Error(`unexpected_route:${path}`);
    },
    flushMedia: async () => {},
  });
  assert.deepEqual(requested, [
    '/api/local/producer/tasks',
    '/api/local/producer/runtime-attempts',
    '/api/local/producer/runtime-submissions',
  ]);
  assert.equal(outbox.calls.some(([state]) => state === 'acknowledged'), true);
});

test('a missing health route bundle only retries the durable envelope and never claims a receipt', async () => {
  const outbox = memoryOutbox();
  let requested = false;
  await flushLocalOutboxOnce({
    outbox,
    mediaOutbox: { pendingCount: async () => 0 },
    readReadiness: async () => ({ connected: false, reachable: false }),
    post: async () => { requested = true; throw new Error('must not post'); },
    flushMedia: async () => {},
  });
  assert.equal(requested, false);
  assert.deepEqual(outbox.calls, [
    ['in_flight', 'submission-1'],
    ['retry', 'submission-1', 'local_producer_route_contract_not_ready'],
  ]);
});

test('legacy 003 health is reachable but cannot start full Producer delivery', async () => {
  const outbox = memoryOutbox();
  let requested = false;
  await flushLocalOutboxOnce({
    outbox,
    mediaOutbox: { pendingCount: async () => 0 },
    readReadiness: () => readLingganLocalReadiness(async () => ({
      ok: true,
      json: async () => ({
        service: 'linggan-local-web',
        listener: 'loopback-only',
        dataState: 'LOCAL_TRUSTED_PRODUCER',
        database: { state: 'READY', schema: 'LOCAL_003_SCHEMA_READY' },
        routes: { localProducer: null },
      }),
    })),
    post: async () => { requested = true; throw new Error('must not post'); },
    flushMedia: async () => {},
  });
  assert.equal(requested, false);
  assert.deepEqual(outbox.calls, [
    ['in_flight', 'submission-1'],
    ['retry', 'submission-1', 'local_producer_route_contract_not_ready'],
  ]);
});

test('a conclusively rejected scheduled Package ends its one server task without retrying platform work', async () => {
  const calls = [];
  const terminalRows = [];
  const scheduledEntry = {
    ...entry(),
    taskSpec: { source: 'scheduled' },
    submissionId: '11111111-1111-4111-8111-111111111111',
    taskId: '22222222-2222-4222-8222-222222222222',
    producerInstanceId: '33333333-3333-4333-8333-333333333333',
  };
  const outbox = {
    due: async () => [scheduledEntry],
    markInFlight: async () => calls.push('in_flight'),
    acknowledge: async () => calls.push('acknowledged'),
    retry: async () => calls.push('retry'),
    terminal: async (_submissionId, error, options) => {
      terminalRows.push({ ...scheduledEntry, error, dispatchFailureCode: options.dispatchFailureCode,
        dispatchFailureId: scheduledEntry.submissionId });
    },
    terminalDispatchFailures: async () => terminalRows,
    markTerminalDispatchFailureReported: async () => calls.push('failure_reported'),
    pendingCount: async () => 0,
  };
  const readiness = {
    deliveryReady: true,
    health: { routes: { dispatch: { failure: '/api/local/dispatch/failures' } } },
    producerRoutes: {
      taskCreation: '/api/local/producer/tasks',
      attemptStart: '/api/local/producer/runtime-attempts',
      submission: '/api/local/producer/runtime-submissions',
    },
  };
  let failure;
  await flushLocalOutboxOnce({
    outbox,
    mediaOutbox: { pendingCount: async () => 0 },
    readReadiness: async () => readiness,
    post: async (path) => {
      if (path === readiness.producerRoutes.taskCreation) return { ok: true, status: 200, payload: { outcome: 'created' } };
      if (path === readiness.producerRoutes.attemptStart) return { ok: true, status: 200, payload: { outcome: 'started' } };
      return { ok: false, status: 400, payload: { code: 'submission_invalid' } };
    },
    readCredential: async () => 'credential',
    reportDispatchFailure: async (request) => {
      failure = request;
      return { reported: true, outcome: 'unavailable' };
    },
    flushMedia: async () => {},
  });
  assert.deepEqual(failure, {
    installKey: scheduledEntry.producerInstanceId,
    installationCredential: 'credential',
    taskId: scheduledEntry.taskId,
    failureId: scheduledEntry.submissionId,
    failureCode: 'capture_delivery_rejected',
    health: readiness.health,
  });
  assert.deepEqual(calls, ['in_flight', 'failure_reported']);
  assert.equal(terminalRows[0].error, 'submission_invalid');
});

test('a claim or Attempt conflict is terminal locally but is never mislabeled as a rejected Package', async () => {
  const calls = [];
  const outbox = {
    due: async () => [{ ...entry(), taskSpec: { source: 'scheduled' } }],
    markInFlight: async () => {},
    acknowledge: async () => {},
    retry: async () => {},
    terminal: async (_id, _error, options) => calls.push(options.dispatchFailureCode),
    terminalDispatchFailures: async () => [],
    pendingCount: async () => 0,
  };
  const routes = {
    taskCreation: '/api/local/producer/tasks',
    attemptStart: '/api/local/producer/runtime-attempts',
    submission: '/api/local/producer/runtime-submissions',
  };
  await flushLocalOutboxOnce({
    outbox,
    mediaOutbox: { pendingCount: async () => 0 },
    readReadiness: async () => ({ deliveryReady: true, health: {}, producerRoutes: routes }),
    post: async (path) => {
      if (path === routes.taskCreation) return { ok: true, status: 200, payload: { outcome: 'created' } };
      if (path === routes.attemptStart) return { ok: true, status: 200, payload: { outcome: 'started' } };
      return { ok: false, status: 409, payload: { code: 'attempt_terminal_submission_conflict' } };
    },
    flushMedia: async () => {},
  });
  assert.deepEqual(calls, ['']);
});

test('a closed claim still asks the submission route and keeps the Receipt it already earned', async () => {
  const calls = [];
  const requested = [];
  const outbox = {
    due: async () => [{ ...entry(), taskSpec: { source: 'scheduled' } }],
    markInFlight: async () => calls.push('in_flight'),
    acknowledge: async () => calls.push('acknowledged'),
    retry: async (_id, reason) => calls.push(`retry:${reason}`),
    terminal: async (_id, error) => calls.push(`terminal:${error}`),
    terminalDispatchFailures: async () => [],
    pendingCount: async () => 0,
  };
  const routes = {
    taskCreation: '/api/local/producer/tasks',
    attemptStart: '/api/local/producer/runtime-attempts',
    submission: '/api/local/producer/runtime-submissions',
  };
  await flushLocalOutboxOnce({
    outbox,
    mediaOutbox: { pendingCount: async () => 0 },
    readReadiness: async () => ({ deliveryReady: true, health: {}, producerRoutes: routes }),
    post: async (path) => {
      requested.push(path);
      if (path === routes.taskCreation) return { ok: true, status: 200, payload: { outcome: 'created' } };
      if (path === routes.attemptStart) {
        return { ok: false, status: 409, payload: { code: 'scheduled_task_not_claimed_by_producer' } };
      }
      return { ok: true, status: 200, payload: { delivery: 'replay', receipt_ref: 'receipt-1' } };
    },
    flushMedia: async () => {},
  });
  assert.deepEqual(requested, [
    routes.taskCreation,
    routes.attemptStart,
    routes.submission,
  ]);
  assert.deepEqual(calls, ['in_flight', 'acknowledged']);
});

test('an Attempt identity conflict stays terminal and never reaches the submission route', async () => {
  const calls = [];
  const requested = [];
  const outbox = {
    due: async () => [{ ...entry(), taskSpec: { source: 'scheduled' } }],
    markInFlight: async () => {},
    acknowledge: async () => calls.push('acknowledged'),
    retry: async () => calls.push('retry'),
    terminal: async (_id, error) => calls.push(`terminal:${error}`),
    terminalDispatchFailures: async () => [],
    pendingCount: async () => 0,
  };
  const routes = {
    taskCreation: '/api/local/producer/tasks',
    attemptStart: '/api/local/producer/runtime-attempts',
    submission: '/api/local/producer/runtime-submissions',
  };
  await flushLocalOutboxOnce({
    outbox,
    mediaOutbox: { pendingCount: async () => 0 },
    readReadiness: async () => ({ deliveryReady: true, health: {}, producerRoutes: routes }),
    post: async (path) => {
      requested.push(path);
      if (path === routes.taskCreation) return { ok: true, status: 200, payload: { outcome: 'created' } };
      return { ok: false, status: 409, payload: { code: 'attempt_identity_conflict' } };
    },
    flushMedia: async () => {},
  });
  assert.deepEqual(requested, [routes.taskCreation, routes.attemptStart]);
  assert.deepEqual(calls, ['terminal:attempt_identity_conflict']);
});
