import assert from 'node:assert/strict';
import test from 'node:test';

// Importing the MV3 entrypoint intentionally exercises its real scheduling seam.  The initial
// best-effort check-in is allowed to complete against this inert browser surface; the test below
// only asserts the schedule shape, not a platform action.
const originalChrome = globalThis.chrome;
const scheduledAlarms = [];
globalThis.chrome = {
  runtime: {
    onInstalled: { addListener() {} },
    onStartup: { addListener() {} },
    onMessage: { addListener() {} },
    getManifest: () => ({ version: '0.0.0-test' }),
  },
  alarms: {
    create: async (name, schedule) => { scheduledAlarms.push({ name, schedule }); },
    onAlarm: { addListener() {} },
  },
  permissions: { contains: async () => false },
  storage: { local: { get: async () => ({}), set: async () => {} } },
  tabs: { query: async () => [], sendMessage: async () => null, create: async () => {} },
  windows: { create: async () => ({}), remove: async () => {} },
};
const mockChrome = globalThis.chrome;

const { patrolAlarmSchedule, recoverPatrolWakeAfterLifecycleRestart } = await import('../src/linggan/background.js');
globalThis.chrome = originalChrome;

test('the local station recheck remains durable after MV3 worker collection', () => {
  assert.deepEqual(patrolAlarmSchedule(60), {
    delayInMinutes: 1,
    periodInMinutes: 1,
  });
});

test('the server still controls the durable patrol cadence', () => {
  assert.deepEqual(patrolAlarmSchedule(300), {
    delayInMinutes: 5,
    periodInMinutes: 5,
  });
  assert.deepEqual(patrolAlarmSchedule(0), {
    delayInMinutes: 1,
    periodInMinutes: 1,
  });
});

test('a lifecycle restart has a durable one-minute bootstrap before the first server answer', () => {
  scheduledAlarms.length = 0;
  globalThis.chrome = mockChrome;
  recoverPatrolWakeAfterLifecycleRestart();
  globalThis.chrome = originalChrome;
  assert.deepEqual(scheduledAlarms, [{
    name: 'linggan-patrol',
    schedule: {
      delayInMinutes: 1,
      periodInMinutes: 1,
    },
  }]);
});
