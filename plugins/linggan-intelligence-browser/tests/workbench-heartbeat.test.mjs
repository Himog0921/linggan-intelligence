import test from 'node:test';
import assert from 'node:assert/strict';

import {
  createCollectionRunHeartbeatReporter,
  createCollectionRunHeartbeatLoop,
} from '../src/workbench/runtime/heartbeat.js';

test('collection run heartbeat reporter throttles repeated updates for the same run', async () => {
  const calls = [];
  const reporter = createCollectionRunHeartbeatReporter({
    collectionRunStore: {
      async markHeartbeat(collectionRunId, timestamp, patch) {
        calls.push([collectionRunId, timestamp, patch]);
      },
    },
    intervalMs: 1000,
    now: (() => {
      const timestamps = [1000, 1500, 2300];
      return () => timestamps.shift();
    })(),
  });

  const first = await reporter.report('run_1', {
    taskState: 'running',
    stage: 'collecting',
    current: 1,
    total: 10,
    message: '正在采集第 1 条',
  });
  const second = await reporter.report('run_1', {
    taskState: 'running',
    stage: 'collecting',
    current: 2,
    total: 10,
    message: '正在采集第 2 条',
  });
  const third = await reporter.report('run_1', {
    taskState: 'running',
    stage: 'collecting',
    current: 3,
    total: 10,
    message: '正在采集第 3 条',
  });

  assert.equal(first, true);
  assert.equal(second, false);
  assert.equal(third, true);
  assert.equal(calls.length, 2);
  assert.deepEqual(calls[0], [
    'run_1',
    1000,
    {
      status: 'running',
      stage: 'collecting',
      message: '正在采集第 1 条',
    },
  ]);
  assert.deepEqual(calls[1], [
    'run_1',
    2300,
    {
      status: 'running',
      stage: 'collecting',
      message: '正在采集第 3 条',
    },
  ]);
});

test('collection run heartbeat reporter can force a refresh even inside the throttle window', async () => {
  const calls = [];
  const reporter = createCollectionRunHeartbeatReporter({
    collectionRunStore: {
      async markHeartbeat(collectionRunId, timestamp, patch) {
        calls.push([collectionRunId, timestamp, patch]);
      },
    },
    intervalMs: 5000,
    now: (() => {
      const timestamps = [1000, 1200];
      return () => timestamps.shift();
    })(),
  });

  await reporter.report('run_2', { taskState: 'running' });
  const forced = await reporter.report('run_2', {
    taskState: 'paused',
    message: '任务已暂停',
    force: true,
  });

  assert.equal(forced, true);
  assert.equal(calls.length, 2);
  assert.deepEqual(calls[1], [
    'run_2',
    1200,
    {
      status: 'paused',
      message: '任务已暂停',
    },
  ]);
});

test('collection run heartbeat loop emits keepalive updates even without progress callbacks', async () => {
  const reports = [];
  const scheduled = [];
  const cleared = [];
  const loop = createCollectionRunHeartbeatLoop({
    reporter: {
      async report(collectionRunId, patch) {
        reports.push([collectionRunId, patch]);
      },
    },
    intervalMs: 3000,
    setIntervalFn(handler, intervalMs) {
      scheduled.push([handler, intervalMs]);
      return 'timer_1';
    },
    clearIntervalFn(timerId) {
      cleared.push(timerId);
    },
  });

  loop.start('run_keepalive', () => ({
    taskState: 'running',
    stage: 'collecting',
    current: 2,
    total: 10,
    message: '正在等待评论区继续加载',
  }));

  assert.equal(scheduled.length, 1);
  assert.equal(scheduled[0][1], 3000);

  await scheduled[0][0]();
  assert.deepEqual(reports[0], [
    'run_keepalive',
    {
      taskState: 'running',
      stage: 'collecting',
      current: 2,
      total: 10,
      message: '正在等待评论区继续加载',
      force: true,
    },
  ]);

  loop.stop();
  assert.deepEqual(cleared, ['timer_1']);
});
