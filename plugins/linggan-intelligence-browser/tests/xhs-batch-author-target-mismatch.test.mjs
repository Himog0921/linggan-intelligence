import test from 'node:test';
import assert from 'node:assert/strict';

import { BatchNoteController } from '../src/platforms/xhs/batchController.js';
import { COLLECT_MODE, TASK_STATE } from '../src/shared/constants.js';
import { MONITOR_TASK_STRATEGY } from '../src/workbench/protocol/schema.js';

test('xhs batch notes author monitor stops early when current profile does not match the target', async () => {
  const previousWindow = globalThis.window;
  const previousDocument = globalThis.document;
  const events = [];

  globalThis.window = {
    location: {
      href: 'https://www.xiaohongshu.com/user/profile/current_author',
      pathname: '/user/profile/current_author',
      origin: 'https://www.xiaohongshu.com',
    },
  };
  globalThis.document = {
    body: { innerText: '' },
  };

  try {
    const controller = new BatchNoteController();

    await assert.rejects(
      () => controller.start(COLLECT_MODE.PROFILE, (progress) => {
        events.push(progress);
      }, {
        count: 5,
        monitorMeta: {
          taskStrategy: MONITOR_TASK_STRATEGY.AUTHOR_BASELINE,
          surfaceOnly: true,
          targetUrl: 'https://www.xiaohongshu.com/user/profile/target_author',
          display: { name: '目标作者' },
        },
        externalTaskMeta: {
          externalTaskId: 'wb_xhs_author_mismatch_1',
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
    assert.equal(errorEvent.phase, 'startup');
    assert.match(errorEvent.message, /目标作者/);
  } finally {
    globalThis.window = previousWindow;
    globalThis.document = previousDocument;
  }
});
