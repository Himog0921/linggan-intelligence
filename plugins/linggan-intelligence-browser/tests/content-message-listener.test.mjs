import test from 'node:test';
import assert from 'node:assert/strict';

import { MSG } from '../src/shared/constants.js';
import { REMOTE_TASK_CONTROL_ACTION } from '../src/workbench/protocol/schema.js';
import { createRuntimeMessageListener } from '../src/content/messageListener.js';

test('content message listener routes legacy workbench task control through runtime handlers', async () => {
  const handledCommands = [];
  const listener = createRuntimeMessageListener({
    isContextValid: () => true,
    loadContentDataRuntime: async () => ({
      messageHandlers: {
        [MSG.WORKBENCH_TASK_CONTROL]: async (message) => {
          handledCommands.push(message.command);
          return {
            success: true,
            accepted: true,
            taskId: '',
            controlAction: message.command,
          };
        },
      },
      dashboardBridge: {
        toggleDashboard: async () => {},
      },
    }),
  });

  const response = await new Promise((resolve) => {
    const keepAlive = listener({
      action: MSG.WORKBENCH_TASK_CONTROL,
      command: REMOTE_TASK_CONTROL_ACTION.STOP,
    }, {}, resolve);

    assert.equal(keepAlive, true);
  });

  assert.deepEqual(handledCommands, [REMOTE_TASK_CONTROL_ACTION.STOP]);
  assert.equal(response.success, true);
  assert.equal(response.accepted, true);
  assert.equal(response.controlAction, REMOTE_TASK_CONTROL_ACTION.STOP);
  assert.deepEqual(response.data, {
    accepted: true,
    taskId: '',
    controlAction: REMOTE_TASK_CONTROL_ACTION.STOP,
  });
});
