import test from 'node:test';
import assert from 'node:assert/strict';

import { describeCommentImageTaskMessage } from '../src/content/commentImageTask.js';
import { TASK_STATE } from '../src/shared/constants.js';

test('comment image scan pause message keeps discovered count', () => {
  const message = describeCommentImageTaskMessage({
    phase: 'scan',
    state: TASK_STATE.PAUSED,
    discovered: 12,
  });

  assert.equal(message, '扫描已暂停：已扫描 12 张图片');
});

test('comment image scan resume message keeps discovered count', () => {
  const message = describeCommentImageTaskMessage({
    phase: 'scan',
    state: TASK_STATE.RUNNING,
    discovered: 12,
  });

  assert.equal(message, '继续扫描中：已扫描 12 张图片');
});

test('comment image download pause message keeps packaged progress', () => {
  const message = describeCommentImageTaskMessage({
    phase: 'download',
    state: TASK_STATE.PAUSED,
    current: 3,
    total: 10,
  });

  assert.equal(message, '打包已暂停：已完成 3/10 张');
});
