import test from 'node:test';
import assert from 'node:assert/strict';

import {
  normalizeNavigatedTaskTabsSnapshot,
  rememberNavigatedTaskTab,
  removeNavigatedTaskTabs,
  taskExecutionCleanupKeys,
} from '../src/workbench/runtime/taskExecutionCleanup.js';

test('task execution cleanup covers task id, external id, and plugin run id', () => {
  assert.deepEqual(taskExecutionCleanupKeys({
    taskId: 'task_1',
    externalTaskId: 'external_task_1',
    pluginRunId: 'run_1',
    pluginOpenedTabId: 501,
  }), {
    registryIds: ['task_1', 'external_task_1', 'run_1'],
    navigationIds: ['task_1', 'external_task_1', 'run_1'],
    tabIds: [501],
  });
});

test('task execution cleanup dedupes ids for direct workbench tasks', () => {
  assert.deepEqual(taskExecutionCleanupKeys({
    taskId: 'task_1',
    externalTaskId: 'task_1',
    pluginRunId: 'task_1',
  }), {
    registryIds: ['task_1'],
    navigationIds: ['task_1'],
    tabIds: [],
  });
});

test('task execution cleanup does not close reused user tabs without plugin ownership', () => {
  assert.deepEqual(taskExecutionCleanupKeys({
    taskId: 'task_1',
    tabId: 777,
  }), {
    registryIds: ['task_1'],
    navigationIds: ['task_1'],
    tabIds: [],
  });
});

test('task execution cleanup persists only valid plugin-opened task tabs', () => {
  const remembered = rememberNavigatedTaskTab(
    normalizeNavigatedTaskTabsSnapshot({
      task_1: 501,
      empty: 0,
      bad: 'not-a-tab',
    }),
    'task_2',
    502,
  );

  assert.deepEqual(remembered, {
    task_1: 501,
    task_2: 502,
  });
  assert.deepEqual(removeNavigatedTaskTabs(remembered, ['task_1', 'missing']), {
    task_2: 502,
  });
});
