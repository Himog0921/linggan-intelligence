import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import vm from 'node:vm';

const source = readFileSync(new URL('./evidence_observation.js', import.meta.url), 'utf8');

function loadObservationModule() {
  const scheduled = [];
  const window = {
    setTimeout(callback, delay) {
      const entry = { callback, delay, cleared: false };
      scheduled.push(entry);
      return entry;
    },
    clearTimeout(entry) {
      if (entry) entry.cleared = true;
    },
  };
  const context = { window, AbortController, URL, fetch: async () => { throw new Error('unexpected_fetch'); } };
  vm.runInNewContext(source, context, { filename: 'evidence_observation.js' });
  return { observation: window.LingganEvidenceObservation, scheduled };
}

test('reobservation controller queues canonical eligible actions without inventing a lease poll', async () => {
  const { observation, scheduled } = loadObservationModule();
  const changes = [];
  let terminalRefreshes = 0;
  const controller = observation.createController({
    apiRoot: '/api/local/work-resources',
    sameOriginPath: (url) => url || null,
    postJson: async () => ({
      statusUrl: null,
      operation: { tasks: [], workOrderRef: 'work-1', execution: 'QUEUED' },
    }),
    onChange: (state) => changes.push(state),
    onTerminal: async () => { terminalRefreshes += 1; },
    timer: {
      setTimeout(callback, delay) {
        const entry = { callback, delay, cleared: false };
        scheduled.push(entry);
        return entry;
      },
      clearTimeout(entry) {
        if (entry) entry.cleared = true;
      },
    },
  });

  const ineligible = controller.availability(
    { identity: { platform: 'xhs' } },
    { eligible: false, supported: true, reason: 'TARGET_LINKED_ACTIVE_DEEP_ARCHIVE_AUTHORIZATION_REQUIRED' },
  );
  assert.equal(ineligible.actionable, false);
  assert.equal(ineligible.actionUrl, null);
  assert.equal(ineligible.supported, true);
  assert.equal(ineligible.eligible, false);
  assert.equal(ineligible.reason, 'TARGET_LINKED_ACTIVE_DEEP_ARCHIVE_AUTHORIZATION_REQUIRED');

  await controller.request('/api/local/work-resources/work-1/reobserve');
  assert.equal(controller.snapshot().operation.execution, 'QUEUED');
  assert.equal(controller.snapshot().operation.workOrderRef, 'work-1');
  assert.equal(controller.snapshot().statusUrl, null);
  assert.equal(scheduled.length, 0);
  assert.equal(terminalRefreshes, 0);
  assert.equal(changes.at(-1).pollError, null);
});

test('reobservation controller cancels its timer and request on selection reset', async () => {
  const { observation, scheduled } = loadObservationModule();
  const controller = observation.createController({
    apiRoot: '/api/local/work-resources',
    sameOriginPath: (url) => url || null,
    readJson: async () => ({ operation: { tasks: [{ state: 'QUEUED' }] } }),
    postJson: async () => ({
      statusUrl: '/api/local/work-resources/work-1/reobserve/lease-1',
      operation: { tasks: [{ state: 'QUEUED' }], leaseRef: 'lease-1' },
    }),
    timer: {
      setTimeout(callback, delay) {
        const entry = { callback, delay, cleared: false };
        scheduled.push(entry);
        return entry;
      },
      clearTimeout(entry) {
        if (entry) entry.cleared = true;
      },
    },
  });

  await controller.request('/api/local/work-resources/work-1/reobserve');
  controller.reset();
  assert.equal(scheduled[0].cleared, true);
  assert.equal(controller.snapshot().operation, null);
  assert.equal(controller.snapshot().statusUrl, null);
});

// 迁移 0097 让「缺执行地址」成为一个可写状态：这类成员在 Attempt 之前就被停下，租约随之结束。
// 它不是「还没轮到」——不把它算终态，页面会对一张已经结束的租约一直轮询下去。
test('a lease stopped for missing execution input is terminal, not still-running', async () => {
  const { observation, scheduled } = loadObservationModule();
  let terminalRefreshes = 0;
  const controller = observation.createController({
    apiRoot: '/api/local/work-resources',
    sameOriginPath: (url) => url || null,
    readJson: async () => ({ operation: { tasks: [{ state: 'INPUT_BLOCKED' }] } }),
    postJson: async () => ({
      statusUrl: '/api/local/work-resources/work-1/reobserve/lease-1',
      operation: { tasks: [{ state: 'INPUT_BLOCKED' }], leaseRef: 'lease-1' },
    }),
    onTerminal: async () => { terminalRefreshes += 1; },
    timer: {
      setTimeout(callback, delay) {
        const entry = { callback, delay, cleared: false };
        scheduled.push(entry);
        return entry;
      },
      clearTimeout(entry) {
        if (entry) entry.cleared = true;
      },
    },
  });

  await controller.request('/api/local/work-resources/work-1/reobserve');
  assert.equal(controller.isTerminal({ state: 'INPUT_BLOCKED' }), true);
  assert.equal(scheduled.length, 0, '停止的租约不再轮询');
  assert.equal(terminalRefreshes, 1, '停在停止上也要刷新一次，让界面显示停止原因');
});
