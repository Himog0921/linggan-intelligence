import test from 'node:test';
import assert from 'node:assert/strict';

import { emitDouyinBatchProgress } from '../src/platforms/douyin/batchController.js';

test('emitDouyinBatchProgress accepts synchronous progress callbacks', () => {
  const events = [];

  assert.doesNotThrow(() => {
    emitDouyinBatchProgress((payload) => {
      events.push(payload);
    }, {
      taskState: 'idle',
      stage: 'done',
      message: '已停止：成功 0 条，失败 0 条（共 5 条）',
    });
  });

  assert.deepEqual(events, [{
    taskState: 'idle',
    stage: 'done',
    message: '已停止：成功 0 条，失败 0 条（共 5 条）',
  }]);
});

test('emitDouyinBatchProgress ignores missing progress callbacks', () => {
  assert.doesNotThrow(() => {
    emitDouyinBatchProgress(null, { taskState: 'done' });
    emitDouyinBatchProgress(undefined, { taskState: 'done' });
  });
});
