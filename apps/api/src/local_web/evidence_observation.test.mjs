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

test('reobservation controller only starts canonical eligible actions and refreshes on a terminal receipt', async () => {
  const { observation, scheduled } = loadObservationModule();
  const changes = [];
  let terminalRefreshes = 0;
  const controller = observation.createController({
    apiRoot: '/api/local/work-resources',
    sameOriginPath: (url) => url || null,
    readJson: async () => ({ operation: { tasks: [{ state: 'ACCEPTED' }], leaseRef: 'lease-1' } }),
    postJson: async () => ({
      statusUrl: '/api/local/work-resources/work-1/reobserve/lease-1',
      operation: { tasks: [{ state: 'QUEUED' }], leaseRef: 'lease-1' },
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
  assert.equal(controller.snapshot().operation.tasks[0].state, 'QUEUED');
  assert.equal(scheduled.length, 1);
  assert.equal(scheduled[0].delay, 2500);
  scheduled[0].callback();
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(controller.snapshot().operation.tasks[0].state, 'ACCEPTED');
  assert.equal(terminalRefreshes, 1);
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
