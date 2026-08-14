import test from 'node:test';
import assert from 'node:assert/strict';

import { MSG } from '../src/shared/constants.js';
import { createContentMessageHandlers } from '../src/content/messageHandlers.js';

function createFacadeHandlers(overrides = {}) {
  return createContentMessageHandlers({
    MSG,
    isDouyinPage: () => false,
    collectNote: async () => ({}),
    collectComments: async () => ({ total: 0 }),
    collectAuthor: async () => ({}),
    collectDouyinVideo: async () => ({ ok: true, data: {} }),
    collectDouyinComments: async () => ({ total: 0, comments: [] }),
    downloadDouyinCommentImages: async () => ({ success: true, total: 0 }),
    collectDouyinAuthor: async () => ({ ok: true, data: {} }),
    BatchNoteController: class {},
    noteStore: {
      count: async () => 2,
      getAll: async () => [],
      getById: async () => ({ noteId: 'note_1' }),
      deleteById: async () => true,
      clear: async () => true,
    },
    commentStore: {
      count: async () => 3,
      getAll: async () => [],
      deleteById: async () => true,
      clear: async () => true,
    },
    authorStore: {
      count: async () => 4,
      getAll: async () => [],
      deleteById: async () => true,
      clear: async () => true,
    },
    reportDone: () => {},
    reportProgress: () => {},
    batchMessageHandlers: {
      [MSG.START_BATCH_NOTES]: () => ({ success: true, source: 'batch' }),
    },
    getBatchNoteCtrl: () => null,
    getBatchCommentCtrl: () => null,
    extractNoteId: () => '',
    downloadNoteMediaFromRecord: async () => ({}),
    generateCsv: () => '',
    downloadFile: () => {},
    backfillLegacyAiReadyFields: async () => ({}),
    getPageContext: async () => ({ platform: 'xhs', pageType: 'detail' }),
    collectionRunStore: {},
    packageWorkbenchResult: async ({ externalTaskId }) => ({ externalTaskId }),
    discoverXhsSurfaceNotes: async () => [],
    discoverDouyinSurfaceTargets: async () => [],
    ...overrides,
  });
}

test('content message facade composes data and batch handlers without changing call shape', async () => {
  const handlers = createFacadeHandlers();

  assert.equal(typeof handlers[MSG.GET_STATS], 'function');
  assert.equal(typeof handlers[MSG.START_BATCH_NOTES], 'function');
  assert.equal(typeof handlers[MSG.WORKBENCH_TASK_CONTROL], 'function');

  assert.deepEqual(await handlers[MSG.GET_STATS](), {
    notes: 2,
    comments: 3,
    authors: 4,
  });
  assert.deepEqual(await handlers[MSG.START_BATCH_NOTES](), {
    success: true,
    source: 'batch',
  });
});

test('content message facade keeps workbench result packaging available through the legacy import path', async () => {
  const handlers = createFacadeHandlers();

  const result = await handlers[MSG.WORKBENCH_GET_RESULT_PACKAGE]({
    externalTaskId: 'wb_result_1',
  });

  assert.equal(result.success, true);
  assert.equal(result.result.externalTaskId, 'wb_result_1');
});
