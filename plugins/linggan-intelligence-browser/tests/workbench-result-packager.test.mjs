import test from 'node:test';
import assert from 'node:assert/strict';

import { buildResultSummary } from '../src/workbench/runtime/resultSummaryBuilder.js';
import { createResultPackager } from '../src/workbench/runtime/resultPackager.js';

test('buildResultSummary counts records and failed items', () => {
  const summary = buildResultSummary({
    notes: [{ dataQuality: 'seed', sourceTier: 'seed' }, { dataQuality: 'full', sourceTier: 'api' }],
    comments: [
      { dataQuality: 'degraded', sourceTier: 'dom' },
      { dataQuality: 'full', sourceTier: 'api' },
      { dataQuality: 'full', sourceTier: 'api' },
    ],
    authors: [{ dataQuality: 'degraded', sourceTier: 'mixed' }],
    mediaAssets: [
      { downloadStatus: '已下载' },
      { downloadStatus: '失败' },
    ],
    runRecord: {
      itemsPlanned: 5,
      itemsSucceeded: 3,
      itemsFailed: 2,
      totalComments: 7,
      targetIds: [' n1 ', '', 'n2'],
      contentIds: [' xhs_n1 ', 'xhs_n2'],
      failedTargets: [{ noteId: 'n3', error: 'timeout' }, null],
      completionNote: '这轮原计划建档 50 条，但当前主页最终只发现 28 条可采作品，所以先按现有作品完成建档。',
      requestedCount: 50,
      discoveredCount: 28,
      shortfallCount: 22,
      discoverySummary: {
        method: 'dom_scroll_persistent_map',
        stopReason: 'bottom_confirmed',
        rounds: 18,
      },
    },
  });

  assert.equal(summary.notes, 2);
  assert.equal(summary.comments, 3);
  assert.equal(summary.authors, 1);
  assert.equal(summary.mediaAssets, 2);
  assert.equal(summary.downloadedMediaAssets, 1);
  assert.equal(summary.itemsSucceeded, 3);
  assert.equal(summary.failedItems, 2);
  assert.equal(summary.totalComments, 7);
  assert.deepEqual(summary.targetIds, ['n1', 'n2']);
  assert.deepEqual(summary.contentIds, ['xhs_n1', 'xhs_n2']);
  assert.deepEqual(summary.failedTargets, [{ noteId: 'n3', error: 'timeout' }]);
  assert.equal(summary.completionNote, '这轮原计划建档 50 条，但当前主页最终只发现 28 条可采作品，所以先按现有作品完成建档。');
  assert.equal(summary.requestedCount, 50);
  assert.equal(summary.discoveredCount, 28);
  assert.equal(summary.shortfallCount, 22);
  assert.deepEqual(summary.discoverySummary, {
    method: 'dom_scroll_persistent_map',
    stopReason: 'bottom_confirmed',
    rounds: 18,
  });
  assert.deepEqual(summary.dataQualityBreakdown, {
    full: 3,
    degraded: 2,
    seed: 1,
  });
  assert.deepEqual(summary.sourceTierBreakdown, {
    api: 3,
    dom: 1,
    mixed: 1,
    seed: 1,
  });
});

test('createResultPackager packages one run with records and summary', async () => {
  const calls = [];
  const packager = createResultPackager({
    collectionRunStore: {
      getById: async () => ({
        collectionRunId: 'run_1',
        platform: 'douyin',
        taskType: 'batchComments',
        status: 'done',
        itemsPlanned: 2,
        itemsSucceeded: 1,
        itemsFailed: 1,
        totalComments: 2,
        targetIds: ['7001', '7002'],
        contentIds: ['dy_7001'],
        failedTargets: [{ awemeId: '7002', error: 'comment api blocked' }],
      }),
      markResultUploadStatus: async (runId, status, patch = {}) => {
        calls.push([runId, status, patch]);
      },
    },
    noteStore: {
      getByCollectionRunId: async () => [{ noteId: 'n1' }],
    },
    commentStore: {
      getByCollectionRunId: async () => [{ commentId: 'c1' }, { commentId: 'c2' }],
    },
    authorStore: {
      getByCollectionRunId: async () => [{ userId: 'u1' }],
    },
    mediaAssetStore: {
      getByCollectionRunId: async () => [{ assetId: 'a1', downloadStatus: '已下载' }],
    },
  });

  const result = await packager.packageByCollectionRunId('run_1');

  assert.equal(result.collectionRunId, 'run_1');
  assert.equal(result.platform, 'douyin');
  assert.equal(result.resultSummary.comments, 2);
  assert.deepEqual(result.resultSummary.targetIds, ['7001', '7002']);
  assert.deepEqual(result.resultSummary.contentIds, ['dy_7001']);
  assert.deepEqual(result.resultSummary.failedTargets, [{ awemeId: '7002', error: 'comment api blocked' }]);
  assert.equal(result.resultSummary.totalComments, 2);
  assert.equal(result.records.notes.length, 1);
  assert.equal(result.records.authors.length, 1);
  assert.equal(calls[0][0], 'run_1');
  assert.equal(calls[0][1], 'packaged');
  assert.equal(typeof calls[0][2].packagedAt, 'number');
});

test('createResultPackager carries failed run diagnostics from the stored run record', async () => {
  const packager = createResultPackager({
    collectionRunStore: {
      getById: async () => ({
        collectionRunId: 'run_failed_detail_1',
        externalTaskId: 'task_failed_detail_1',
        platform: 'xhs',
        taskType: 'singleNote',
        status: 'failed',
        error: '笔记数据未稳定就绪: expected=69fd330a actual=',
        diagnostic: {
          stage: 'collecting',
          failureCategory: 'retry_wait',
          reasonCode: 'page_data_not_ready',
          userMessage: '目标笔记页面没有加载出可采数据',
          technicalMessage: '笔记数据未稳定就绪: expected=69fd330a actual=',
          recommendedAction: '稍后自动重试，或改用作者页重新定位该笔记',
          evidence: {
            expectedNoteId: '69fd330a',
            currentNoteId: '',
          },
        },
      }),
      markResultUploadStatus: async () => null,
    },
    noteStore: { getByCollectionRunId: async () => [] },
    commentStore: { getByCollectionRunId: async () => [] },
    authorStore: { getByCollectionRunId: async () => [] },
    mediaAssetStore: { getByCollectionRunId: async () => [] },
  });

  const result = await packager.packageByCollectionRunId('run_failed_detail_1');

  assert.equal(result.status, 'failed');
  assert.equal(result.errorMessage, '笔记数据未稳定就绪: expected=69fd330a actual=');
  assert.equal(result.userMessage, '目标笔记页面没有加载出可采数据');
  assert.equal(result.diagnostic.stage, 'collecting');
  assert.equal(result.diagnostic.failureCategory, 'retry_wait');
  assert.equal(result.diagnostic.reasonCode, 'page_data_not_ready');
  assert.equal(result.diagnostic.evidence.expectedNoteId, '69fd330a');
});

test('createResultPackager can resolve latest run by externalTaskId', async () => {
  const calls = [];
  const packager = createResultPackager({
    collectionRunStore: {
      getById: async (id) => ({
        collectionRunId: id,
        externalTaskId: 'wb_task_1',
        platform: 'douyin',
        taskType: 'batchNotes',
        status: 'done',
      }),
      getLatestByExternalTaskId: async () => ({
        collectionRunId: 'run_external_1',
        externalTaskId: 'wb_task_1',
        platform: 'douyin',
        taskType: 'batchNotes',
        status: 'done',
      }),
      markResultUploadStatus: async (runId, status, patch = {}) => {
        calls.push([runId, status, patch]);
      },
    },
    noteStore: {
      getByCollectionRunId: async () => [{ noteId: 'n1' }, { noteId: 'n2' }],
    },
    commentStore: {
      getByCollectionRunId: async () => [],
    },
    authorStore: {
      getByCollectionRunId: async () => [],
    },
    mediaAssetStore: {
      getByCollectionRunId: async () => [],
    },
  });

  const result = await packager.packageByExternalTaskId('wb_task_1');

  assert.equal(result.collectionRunId, 'run_external_1');
  assert.equal(result.externalTaskId, 'wb_task_1');
  assert.equal(result.resultSummary.notes, 2);
  assert.equal(calls[0][0], 'run_external_1');
  assert.equal(calls[0][1], 'packaged');
});
