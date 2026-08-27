import test from 'node:test';
import assert from 'node:assert/strict';

import { MSG } from '../src/shared/constants.js';
import { createMediaHandlers } from '../src/content/messageHandlers/mediaHandlers.js';
import { createRemoteControlRegistry } from '../src/content/remoteControlRegistry.js';

function createRemoteRunHelpers(collectionRunStore) {
  return {
    createRemoteRun: async ({ remoteTaskMeta = {}, taskType } = {}) => {
      if (!remoteTaskMeta.externalTaskId || !collectionRunStore?.createRun) return null;
      return collectionRunStore.createRun({
        externalTaskId: remoteTaskMeta.externalTaskId,
        taskType,
      });
    },
    finalizeRemoteRun: async (run, status, patch = {}) => {
      const runId = String(run?.collectionRunId || '').trim();
      if (!runId) return;
      if (status === 'done') return collectionRunStore.markDone(runId, patch);
      if (status === 'stopped') return collectionRunStore.markStopped(runId, patch);
      if (status === 'failed') return collectionRunStore.markFailed(runId, patch.error || 'media_failed', patch);
    },
  };
}

function createMediaHandlerSet(overrides = {}) {
  const collectionRunStore = overrides.collectionRunStore || {};
  return createMediaHandlers({
    MSG,
    isDouyinPage: () => false,
    ensurePluginAuthorized: async () => null,
    downloadDouyinCommentImages: async () => ({ success: true, total: 0 }),
    noteStore: {
      getById: async () => ({ noteId: 'n1', title: '测试笔记' }),
    },
    downloadNoteMediaFromRecord: async () => ({ total: 0, success: 0, failed: 0 }),
    remoteControlRegistry: createRemoteControlRegistry(),
    ...createRemoteRunHelpers(collectionRunStore),
    ...overrides,
  });
}

test('media handlers forward selected media types into note media download', async () => {
  const downloadCalls = [];
  const handlers = createMediaHandlerSet({
    downloadNoteMediaFromRecord: async (note, options) => {
      downloadCalls.push({ note, options });
      return { total: 1, success: 1, failed: 0 };
    },
  });

  const result = await handlers[MSG.DOWNLOAD_NOTE_MEDIA]({
    noteId: 'n1',
    mediaTypes: ['cover'],
  });

  assert.equal(result.success, true);
  assert.deepEqual(result.summary, { total: 1, success: 1, failed: 0 });
  assert.equal(downloadCalls.length, 1);
  assert.deepEqual(downloadCalls[0].options, { mediaTypes: ['cover'] });
});

test('media handlers create remote comment image download runs and write success summary', async () => {
  const doneCalls = [];
  const handlers = createMediaHandlerSet({
    isDouyinPage: () => true,
    downloadDouyinCommentImages: async (options = {}) => ({
      success: true,
      downloaded: 3,
      failed: 1,
      total: 4,
      hdCount: 2,
      sdCount: 1,
      scannedImages: 4,
      note: { contentId: 'douyin_note_2', platformContentId: 'aweme_2' },
      collectionRunId: options.collectionRunId,
    }),
    collectionRunStore: {
      async createRun() {
        return { collectionRunId: 'run_dy_image_1' };
      },
      async markDone(runId, payload) {
        doneCalls.push([runId, payload]);
      },
      async markStopped() {
        throw new Error('markStopped should not be called');
      },
      async markFailed() {
        throw new Error('markFailed should not be called');
      },
    },
  });

  const result = await handlers[MSG.DOWNLOAD_CURRENT_COMMENT_IMAGES]({
    triggerSource: 'workbench_dispatch',
    maxTotal: 10,
    commentDepthMode: 'twoLevel',
    externalTaskMeta: {
      externalTaskId: 'wb_task_image_1',
      externalTaskType: 'douyin.commentImageDownload',
      protocolVersion: 'v1',
    },
  });

  assert.equal(result.success, true);
  assert.equal(result.total, 4);
  assert.equal(result.downloaded, 3);
  assert.deepEqual(doneCalls, [[
    'run_dy_image_1',
    {
      itemsPlanned: 4,
      itemsSucceeded: 3,
      itemsFailed: 1,
      totalComments: 0,
      totalImages: 4,
      scannedImages: 4,
      hdCount: 2,
      sdCount: 1,
      contentId: 'douyin_note_2',
      targetIds: ['aweme_2'],
      noImages: false,
      zipName: '',
    },
  ]]);
});
