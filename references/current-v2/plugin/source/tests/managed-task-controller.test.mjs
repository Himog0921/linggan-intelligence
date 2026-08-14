import test from 'node:test';
import assert from 'node:assert/strict';

import { createManagedTaskController } from '../src/shared/managedTaskController.js';

test('managed task controller releases paused task on stop and runs onFinally', async () => {
  const events = [];
  let shouldStopAfterResume = false;
  let resolveFinally = null;
  const finished = new Promise((resolve) => {
    resolveFinally = resolve;
  });

  const controller = createManagedTaskController(async ({ shouldStop, waitIfPaused }) => {
    events.push('started');
    controller.pause();
    await waitIfPaused();
    shouldStopAfterResume = shouldStop();
    events.push('resumed');
  }, {
    onFinally: () => {
      events.push('finally');
      resolveFinally();
    },
  });

  controller.start();
  await Promise.resolve();
  await Promise.resolve();

  controller.stop();
  await finished;

  assert.equal(shouldStopAfterResume, true);
  assert.equal(controller.isRunning, false);
  assert.deepEqual(events, ['started', 'resumed', 'finally']);
});
