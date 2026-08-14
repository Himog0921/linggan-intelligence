import test from 'node:test';
import assert from 'node:assert/strict';

import {
  inferTaskStage,
  describeTaskDetail,
  shouldDelayTaskbarHide,
  resolveTaskState,
} from '../src/shared/taskUi.js';
import { TASK_STATE } from '../src/shared/constants.js';

test('paused task detail appends progress summary when original message has no counts', () => {
  const detail = describeTaskDetail({
    taskState: TASK_STATE.PAUSED,
    message: '评论采集已暂停',
    current: 7,
    total: 0,
  });

  assert.equal(detail, '评论采集已暂停（已处理 7）');
});

test('idle task stage is rendered as stopped instead of in-progress', () => {
  const stage = inferTaskStage({
    taskState: TASK_STATE.IDLE,
    message: '批量评论已停止',
    current: 2,
    total: 5,
  });

  assert.equal(stage.label, '已停止');
  assert.equal(stage.tone, 'paused');
  assert.match(stage.helper, /2\/5/);
});

test('resolveTaskState falls back to TASK_STATE values when legacy terminal status leaks into taskState', () => {
  assert.equal(
    resolveTaskState({ taskState: 'stopped', status: TASK_STATE.IDLE }),
    TASK_STATE.IDLE,
  );
  assert.equal(
    resolveTaskState({ taskState: 'failed', status: TASK_STATE.ERROR }),
    TASK_STATE.ERROR,
  );
});

test('taskbar hide is delayed for stopped summaries', () => {
  assert.equal(shouldDelayTaskbarHide({ taskState: TASK_STATE.IDLE }), true);
  assert.equal(shouldDelayTaskbarHide({ taskState: TASK_STATE.DONE }), true);
  assert.equal(shouldDelayTaskbarHide({ taskState: TASK_STATE.RUNNING }), false);
});
