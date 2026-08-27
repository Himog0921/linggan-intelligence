import test from 'node:test';
import assert from 'node:assert/strict';

import { batchCollectDouyinProfileVideos } from '../src/platforms/douyin/batchController.js';
import { TASK_STATE } from '../src/shared/constants.js';
import { MONITOR_TASK_STRATEGY } from '../src/workbench/protocol/schema.js';

test('douyin batch videos author monitor stops early when current profile does not match the target', async () => {
  const previousWindow = globalThis.window;
  const previousDocument = globalThis.document;
  const events = [];

  const document = {
    title: '抖音',
    querySelector: () => null,
    querySelectorAll: () => [],
  };

  globalThis.window = {
    location: {
      href: 'https://www.douyin.com/user/current_author',
      pathname: '/user/current_author',
      origin: 'https://www.douyin.com',
    },
    document,
  };
  globalThis.document = document;

  try {
    await assert.rejects(
      () => batchCollectDouyinProfileVideos({
        maxCount: 4,
        monitorMeta: {
          taskStrategy: MONITOR_TASK_STRATEGY.AUTHOR_BASELINE,
          surfaceOnly: true,
          targetUrl: 'https://www.douyin.com/user/target_author',
          display: { name: '目标抖音博主' },
        },
        externalTaskMeta: {
          externalTaskId: 'wb_douyin_author_mismatch_1',
        },
        onProgress: (progress = {}) => {
          events.push(progress);
        },
      }),
      (error) => {
        assert.equal(error.code, 'target_mismatch');
        assert.match(error.message, /不一致|无法对齐/);
        return true;
      },
    );

    const errorEvent = events.find((event) => event.taskState === TASK_STATE.ERROR && event.errorCode === 'target_mismatch');
    assert.ok(errorEvent);
    assert.equal(errorEvent.stage, 'startup');
    assert.match(errorEvent.message, /目标抖音博主/);
  } finally {
    globalThis.window = previousWindow;
    globalThis.document = previousDocument;
  }
});
