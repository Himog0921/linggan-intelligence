import test from 'node:test';
import assert from 'node:assert/strict';

import {
  FAST_TASK_POLL_INTERVAL_MS,
  IDLE_POLL_JITTER_MIN_MS,
  MIN_CHROME_ALARM_INTERVAL_MS,
  POST_TASK_COOLDOWN_MAX_MS,
  SLOW_TASK_POLL_INTERVAL_MS,
  scheduleWorkbenchTaskPollAlarm,
  shouldRunWorkbenchTaskPollAfterHeartbeat,
  shouldRunWorkbenchTaskPollAfterHeartbeatResult,
} from '../src/workbench/runtime/taskPollSchedule.js';

test('task poll scheduler uses claim idle wait time for the next alarm', async () => {
  const calls = [];
  const alarmsApi = {
    create(name, options) {
      calls.push([name, options]);
    },
  };

  const config = scheduleWorkbenchTaskPollAlarm({
    alarmsApi,
    alarmName: 'workbench-task-poll',
    result: {
      idle: true,
      idleReasonCode: 'no_available_account',
      idleReasonMessage: '没有可用账号',
      nextPollAfterMs: 45_000,
    },
    consecutiveEmptyPolls: 0,
  });

  assert.equal(config.intervalMs, 45_000);
  assert.equal(config.periodInMinutes, 0.75);
  assert.equal(config.idleReasonCode, 'no_available_account');
  assert.equal(config.idleReasonMessage, '没有可用账号');
  assert.deepEqual(calls, [[
    'workbench-task-poll',
    { periodInMinutes: 0.75 },
  ]]);
});

test('task poll scheduler keeps the existing fallback cadence when claim provides no delay', async () => {
  const emptyCalls = [];
  const slowCalls = [];

  scheduleWorkbenchTaskPollAlarm({
    alarmsApi: {
      create(name, options) {
        emptyCalls.push([name, options]);
      },
    },
    alarmName: 'workbench-task-poll',
    result: { idle: true },
    consecutiveEmptyPolls: 0,
    randomFn: () => 0,
  });

  const slowConfig = scheduleWorkbenchTaskPollAlarm({
    alarmsApi: {
      create(name, options) {
        slowCalls.push([name, options]);
      },
    },
    alarmName: 'workbench-task-poll',
    result: { idle: true },
    consecutiveEmptyPolls: 3,
    randomFn: () => 0,
  });

  assert.equal(emptyCalls[0][1].periodInMinutes, (FAST_TASK_POLL_INTERVAL_MS + IDLE_POLL_JITTER_MIN_MS) / 60_000);
  assert.equal(slowConfig.intervalMs, SLOW_TASK_POLL_INTERVAL_MS + IDLE_POLL_JITTER_MIN_MS);
  assert.equal(slowCalls[0][1].periodInMinutes, (SLOW_TASK_POLL_INTERVAL_MS + IDLE_POLL_JITTER_MIN_MS) / 60_000);
});

test('task poll scheduler staggers empty claim waits without polling sooner than nextPollAfterMs', async () => {
  const calls = [];
  const config = scheduleWorkbenchTaskPollAlarm({
    alarmsApi: {
      create(name, options) {
        calls.push([name, options]);
      },
    },
    alarmName: 'workbench-task-poll',
    result: {
      idle: true,
      nextPollAfterMs: 120_000,
    },
    consecutiveEmptyPolls: 0,
    randomFn: () => 0,
  });

  assert.equal(config.intervalMs, 120_000 + IDLE_POLL_JITTER_MIN_MS);
  assert.deepEqual(calls, [[
    'workbench-task-poll',
    { periodInMinutes: (120_000 + IDLE_POLL_JITTER_MIN_MS) / 60_000 },
  ]]);
});

test('task poll scheduler respects server balancing waits without adding jitter', async () => {
  const calls = [];
  const config = scheduleWorkbenchTaskPollAlarm({
    alarmsApi: {
      create(name, options) {
        calls.push([name, options]);
      },
    },
    alarmName: 'workbench-task-poll',
    result: {
      idle: true,
      idleReasonCode: 'STATION_BALANCING_WAIT',
      nextPollAfterMs: 8_000,
    },
    consecutiveEmptyPolls: 0,
    randomFn: () => 1,
  });

  assert.equal(config.requestedIntervalMs, 8_000);
  assert.equal(config.intervalMs, MIN_CHROME_ALARM_INTERVAL_MS);
  assert.deepEqual(calls, [[
    'workbench-task-poll',
    { periodInMinutes: MIN_CHROME_ALARM_INTERVAL_MS / 60_000 },
  ]]);
});

test('task poll scheduler respects plugin backpressure waits without adding jitter', async () => {
  const calls = [];
  const config = scheduleWorkbenchTaskPollAlarm({
    alarmsApi: {
      create(name, options) {
        calls.push([name, options]);
      },
    },
    alarmName: 'workbench-task-poll',
    result: {
      idle: true,
      idleReasonCode: 'plugin_protocol_backpressure',
      nextPollAfterMs: 60_000,
    },
    consecutiveEmptyPolls: 0,
    randomFn: () => 1,
  });

  assert.equal(config.requestedIntervalMs, 60_000);
  assert.equal(config.intervalMs, 60_000);
  assert.deepEqual(calls, [[
    'workbench-task-poll',
    { periodInMinutes: 1 },
  ]]);
});

test('task poll scheduler applies a short cooldown after task completion', async () => {
  const calls = [];
  const config = scheduleWorkbenchTaskPollAlarm({
    alarmsApi: {
      create(name, options) {
        calls.push([name, options]);
      },
    },
    alarmName: 'workbench-task-poll',
    result: {
      success: true,
      final: true,
      status: 'completed',
    },
    consecutiveEmptyPolls: 0,
    randomFn: () => 1,
  });

  assert.equal(config.intervalMs, POST_TASK_COOLDOWN_MAX_MS);
  assert.deepEqual(calls, [[
    'workbench-task-poll',
    { periodInMinutes: POST_TASK_COOLDOWN_MAX_MS / 60_000 },
  ]]);
});

test('task poll scheduler respects explicit cooldown after task release', async () => {
  const calls = [];
  const config = scheduleWorkbenchTaskPollAlarm({
    alarmsApi: {
      create(name, options) {
        calls.push([name, options]);
      },
    },
    alarmName: 'workbench-task-poll',
    result: {
      success: true,
      released: true,
      reason: 'lease_conflict',
      nextPollAfterMs: 120_000,
    },
    consecutiveEmptyPolls: 0,
    randomFn: () => 0,
  });

  assert.equal(config.intervalMs, 120_000);
  assert.deepEqual(calls, [[
    'workbench-task-poll',
    { periodInMinutes: 2 },
  ]]);
});

test('heartbeat-triggered task polling waits for the scheduled idle poll time', async () => {
  assert.equal(shouldRunWorkbenchTaskPollAfterHeartbeat({
    activeTask: null,
    nextPollAtMs: 120_000,
    nowMs: 60_000,
  }), false);

  assert.equal(shouldRunWorkbenchTaskPollAfterHeartbeat({
    activeTask: null,
    nextPollAtMs: 120_000,
    nowMs: 120_000,
  }), true);
  assert.equal(shouldRunWorkbenchTaskPollAfterHeartbeat({
    activeTask: { taskId: 'task-1' },
    nextPollAtMs: 120_000,
    nowMs: 60_000,
  }), false);

  assert.equal(shouldRunWorkbenchTaskPollAfterHeartbeat({
    activeTask: { taskId: 'task-1' },
    nextPollAtMs: 120_000,
    nowMs: 120_000,
  }), true);
});

test('heartbeat-triggered task polling runs immediately when server reports pending work', async () => {
  assert.equal(shouldRunWorkbenchTaskPollAfterHeartbeat({
    activeTask: null,
    forcePoll: true,
    nextPollAtMs: 120_000,
    nowMs: 60_000,
  }), true);
});

test('heartbeat result only wakes task polling after a successful heartbeat', async () => {
  assert.equal(shouldRunWorkbenchTaskPollAfterHeartbeatResult({
    heartbeat: { success: false, shouldPollNow: true },
    nextPollAtMs: 120_000,
    nowMs: 60_000,
  }), false);

  assert.equal(shouldRunWorkbenchTaskPollAfterHeartbeatResult({
    heartbeat: { success: true, shouldPollNow: true },
    nextPollAtMs: 120_000,
    nowMs: 60_000,
  }), true);
});
