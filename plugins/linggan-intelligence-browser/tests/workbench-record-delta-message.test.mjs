import test from 'node:test';
import assert from 'node:assert/strict';

import { MSG } from '../src/shared/constants.js';
import { reportWorkbenchRecord } from '../src/shared/messaging.js';

test('reportWorkbenchRecord emits a workbench record delta message', () => {
  const messages = [];
  globalThis.chrome = {
    runtime: {
      id: 'extension-id',
      sendMessage: (message) => {
        messages.push(message);
      },
    },
  };

  try {
    reportWorkbenchRecord({
      recordType: 'note',
      externalRecordId: 'note_1',
      record: { noteId: 'note_1', title: 'ADHD 笔记' },
      collectionRunId: 'run_1',
      externalTaskId: 'task_1',
      sequence: 123,
      collectedAt: '2026-04-14T08:00:00.000Z',
    });
  } finally {
    delete globalThis.chrome;
  }

  assert.equal(messages.length, 1);
  assert.deepEqual(messages[0], {
    action: MSG.WORKBENCH_RECORD_DELTA,
    recordType: 'note',
    externalRecordId: 'note_1',
    record: { noteId: 'note_1', title: 'ADHD 笔记' },
    collectionRunId: 'run_1',
    externalTaskId: 'task_1',
    sequence: 123,
    collectedAt: '2026-04-14T08:00:00.000Z',
  });
});
