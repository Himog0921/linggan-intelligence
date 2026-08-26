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
      database: { state: 'READY', schema: 'PLUGIN_RUNTIME_001_SCHEMA_READY' },
      routes: {
        localProducer: {
          taskCreation: '/api/local/producer/tasks',
          attemptStart: '/api/local/producer/runtime-attempts',
          submission: '/api/local/producer/runtime-submissions',
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
