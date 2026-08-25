import test from 'node:test';
import assert from 'node:assert/strict';

import { MSG } from '../src/shared/constants.js';
import { REMOTE_TASK_CONTROL_ACTION } from '../src/workbench/protocol/schema.js';
import { createWorkbenchHandlers } from '../src/content/messageHandlers/workbenchHandlers.js';
import { createRemoteControlRegistry } from '../src/content/remoteControlRegistry.js';

test('workbench handlers route legacy control messages without taskControl to active batch controllers', async () => {
  const calls = [];
  const handlers = createWorkbenchHandlers({
    MSG,
    remoteControlRegistry: createRemoteControlRegistry(),
    getBatchNoteCtrl: () => ({
      stop: () => calls.push(['notes', 'stop']),
      pause: () => calls.push(['notes', 'pause']),
      resume: () => calls.push(['notes', 'resume']),
    }),
    getBatchCommentCtrl: () => ({
      stop: () => calls.push(['comments', 'stop']),
      pause: () => calls.push(['comments', 'pause']),
      resume: () => calls.push(['comments', 'resume']),
    }),
  });

  assert.deepEqual(
    await handlers[MSG.WORKBENCH_TASK_CONTROL]({ command: REMOTE_TASK_CONTROL_ACTION.STOP }),
    {
      success: true,
      accepted: true,
      taskId: '',
      controlAction: REMOTE_TASK_CONTROL_ACTION.STOP,
    },
  );
  await handlers[MSG.WORKBENCH_TASK_CONTROL]({ command: REMOTE_TASK_CONTROL_ACTION.PAUSE });
  await handlers[MSG.WORKBENCH_TASK_CONTROL]({ command: REMOTE_TASK_CONTROL_ACTION.RESUME });

  assert.deepEqual(calls, [
    ['notes', 'stop'],
    ['comments', 'stop'],
    ['notes', 'pause'],
    ['comments', 'pause'],
    ['notes', 'resume'],
    ['comments', 'resume'],
  ]);
});

test('workbench handlers apply taskControl through the shared remote control registry', async () => {
  const remoteControlRegistry = createRemoteControlRegistry();
  const binding = remoteControlRegistry.bindRemoteControl({
    remoteRun: { collectionRunId: 'run_control_1' },
    remoteTaskMeta: { externalTaskId: 'task_control_1' },
  });
  const handlers = createWorkbenchHandlers({
    MSG,
    remoteControlRegistry,
  });

  const pauseResult = await handlers[MSG.WORKBENCH_TASK_CONTROL]({
    taskControl: {
      taskId: 'task_control_1',
      action: REMOTE_TASK_CONTROL_ACTION.PAUSE,
    },
  });
  const resumeResult = await handlers[MSG.WORKBENCH_TASK_CONTROL]({
    taskControl: {
      collectionRunId: 'run_control_1',
      action: REMOTE_TASK_CONTROL_ACTION.RESUME,
    },
  });

  assert.equal(pauseResult.success, true);
  assert.equal(pauseResult.status, 'paused');
  assert.equal(resumeResult.success, true);
  assert.equal(resumeResult.status, 'running');

  binding.release();
});

test('workbench handlers proxy result packaging by external task id', async () => {
  const handlers = createWorkbenchHandlers({
    MSG,
    remoteControlRegistry: createRemoteControlRegistry(),
    packageWorkbenchResult: async ({ externalTaskId }) => ({
      externalTaskId,
      resultSummary: { authors: 1 },
    }),
  });

  const result = await handlers[MSG.WORKBENCH_GET_RESULT_PACKAGE]({
    externalTaskId: 'wb_result_1',
  });

  assert.equal(result.success, true);
  assert.equal(result.result.externalTaskId, 'wb_result_1');
  assert.equal(result.result.resultSummary.authors, 1);
});
