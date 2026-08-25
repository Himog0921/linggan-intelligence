import test from 'node:test';
import assert from 'node:assert/strict';

import { MSG } from '../src/shared/constants.js';
import { createCollectionHandlers } from '../src/content/messageHandlers/collectionHandlers.js';
import { createRemoteControlRegistry } from '../src/content/remoteControlRegistry.js';

function createRemoteRunHelpers(collectionRunStore, getPageContext = async () => ({ platform: 'xhs', pageType: 'detail' })) {
  return {
    createRemoteRun: async ({
      platform,
      triggerSource,
      remoteTaskMeta = {},
      taskType,
      config = {},
      meta = {},
    } = {}) => {
      const externalTaskId = String(remoteTaskMeta.externalTaskId || '').trim();
      if (!externalTaskId || !collectionRunStore?.createRun) return null;
      const pageContext = await getPageContext();
      return collectionRunStore.createRun({
        externalTaskId,
        externalTaskType: String(remoteTaskMeta.externalTaskType || '').trim(),
        executorInstanceId: String(remoteTaskMeta.executorInstanceId || '').trim(),
        protocolVersion: String(remoteTaskMeta.protocolVersion || '').trim(),
        platform: String(platform || pageContext?.platform || '').trim(),
        taskType,
        pageType: String(pageContext?.pageType || pageContext?.mode || '').trim(),
        triggerSource,
        resultUploadStatus: 'pending_upload',
        lastHeartbeatAt: Date.now(),
        config,
        meta: {
          pageUrl: String(globalThis.window?.location?.href || '').trim(),
          ...meta,
        },
      });
    },
    finalizeRemoteRun: async (run, status, patch = {}) => {
      const runId = String(run?.collectionRunId || '').trim();
      if (!runId) return;
      if (status === 'done') return collectionRunStore.markDone(runId, patch);
      if (status === 'stopped') return collectionRunStore.markStopped(runId, patch);
      if (status === 'failed') return collectionRunStore.markFailed(runId, patch.error || '博主采集失败', patch);
      return null;
    },
  };
}

function createCollectionHandlerSet(overrides = {}) {
  const collectionRunStore = overrides.collectionRunStore || {};
  const getPageContext = overrides.getPageContext || (async () => ({ platform: 'xhs', pageType: 'detail' }));
  const remoteHelpers = createRemoteRunHelpers(collectionRunStore, getPageContext);

  return createCollectionHandlers({
    MSG,
    isDouyinPage: () => false,
    ensurePluginAuthorized: async () => null,
    collectNote: async () => null,
    collectComments: async () => ({ total: 0 }),
    collectAuthor: async () => ({}),
    collectDouyinVideo: async () => ({ ok: true, data: {} }),
    collectDouyinComments: async () => ({ total: 0, comments: [] }),
    collectDouyinAuthor: async () => ({ ok: true, data: {} }),
    BatchNoteController: class {},
    noteStore: { bulkUpsert: async () => {} },
    reportDone: () => {},
    reportProgress: () => {},
    batchMessageHandlers: {},
    extractNoteId: () => '',
    remoteControlRegistry: createRemoteControlRegistry(),
    discoverXhsSurfaceNotes: async () => [],
    discoverDouyinSurfaceTargets: async () => [],
    ...remoteHelpers,
    ...overrides,
  });
}

test('collection handlers keep partial stopped summary for remote douyin single comments', async () => {
  const stoppedCalls = [];
  const doneCalls = [];
  const handlers = createCollectionHandlerSet({
    isDouyinPage: () => true,
    collectDouyinComments: async () => ({
      stopped: true,
      total: 17,
      comments: [{ commentId: 'comment_1' }],
      note: {
        contentId: 'dy_content_1',
        platformContentId: 'video_1',
      },
    }),
    getPageContext: async () => ({ platform: 'douyin', pageType: 'detail' }),
    collectionRunStore: {
      async createRun() {
        return { collectionRunId: 'run_remote_comment_1' };
      },
      async markStopped(runId, patch) {
        stoppedCalls.push([runId, patch]);
      },
      async markDone(runId, patch) {
        doneCalls.push([runId, patch]);
      },
      async markFailed() {
        throw new Error('markFailed should not be called');
      },
    },
  });

  const result = await handlers[MSG.COLLECT_SINGLE_COMMENT]({
    triggerSource: 'workbench_dispatch',
    externalTaskMeta: {
      externalTaskId: 'task_remote_comment_1',
      externalTaskType: 'douyin.singleComments',
      executorInstanceId: 'executor_1',
      protocolVersion: 'v1',
    },
  });

  assert.equal(result.success, true);
  assert.equal(result.total, 17);
  assert.equal(doneCalls.length, 0);
  assert.deepEqual(stoppedCalls, [[
    'run_remote_comment_1',
    {
      itemsPlanned: 1,
      itemsSucceeded: 1,
      itemsFailed: 0,
      totalComments: 17,
      contentId: 'dy_content_1',
      targetIds: ['video_1'],
    },
  ]]);
});

test('collection handlers create a remote xhs single-note run and write success summary', async () => {
  const doneCalls = [];
  const note = {
    noteId: '69baad5e00000000230055ef',
    platformContentId: '69baad5e00000000230055ef',
    contentId: 'xhs_69baad5e00000000230055ef',
    title: '示例笔记',
  };
  const handlers = createCollectionHandlerSet({
    collectNote: async () => note,
    collectionRunStore: {
      async createRun() {
        return { collectionRunId: 'run_remote_note_1' };
      },
      async markDone(runId, patch) {
        doneCalls.push([runId, patch]);
      },
      async markStopped() {
        throw new Error('markStopped should not be called');
      },
      async markFailed() {
        throw new Error('markFailed should not be called');
      },
    },
  });

  const result = await handlers[MSG.COLLECT_SINGLE_NOTE]({
    triggerSource: 'workbench_dispatch',
    expectedNoteId: '69baad5e00000000230055ef',
    externalTaskMeta: {
      externalTaskId: 'task_remote_note_1',
      externalTaskType: 'xhs.batchNotes',
      executorInstanceId: 'executor_1',
      protocolVersion: 'v1',
    },
  });

  assert.equal(result.success, true);
  assert.equal(result.note.title, '示例笔记');
  assert.deepEqual(doneCalls, [[
    'run_remote_note_1',
    {
      itemsPlanned: 1,
      itemsSucceeded: 1,
      itemsFailed: 0,
      targetIds: ['69baad5e00000000230055ef'],
      contentIds: ['xhs_69baad5e00000000230055ef'],
    },
  ]]);
});

test('collection handlers write structured diagnostics for remote xhs single-note failures', async () => {
  const failedCalls = [];
  const previousWindow = globalThis.window;
  globalThis.window = {
    location: {
      href: 'https://www.xiaohongshu.com/discovery/item/69fd330a?xsec_token=token',
    },
  };

  try {
    const handlers = createCollectionHandlerSet({
      collectNote: async () => {
        throw new Error('笔记数据未稳定就绪: expected=69fd330a actual=');
      },
      collectionRunStore: {
        async createRun() {
          return { collectionRunId: 'run_remote_note_failed' };
        },
        async markDone() {
          throw new Error('markDone should not be called');
        },
        async markStopped() {
          throw new Error('markStopped should not be called');
        },
        async markFailed(runId, error, patch) {
          failedCalls.push([runId, error, patch]);
        },
      },
    });

    await assert.rejects(
      () => handlers[MSG.COLLECT_SINGLE_NOTE]({
        triggerSource: 'workbench_dispatch',
        expectedNoteId: '69fd330a',
        externalTaskMeta: {
          externalTaskId: 'task_remote_note_failed',
          externalTaskType: 'xhs.batchNotes',
          executorInstanceId: 'executor_1',
          protocolVersion: 'v1',
          monitorMeta: {
            monitorId: 'monitor_1',
            taskStrategy: 'detail_probe',
          },
        },
      }),
    );

    assert.equal(failedCalls.length, 1);
    assert.equal(failedCalls[0][0], 'run_remote_note_failed');
    assert.equal(failedCalls[0][1], '笔记数据未稳定就绪: expected=69fd330a actual=');
    assert.equal(failedCalls[0][2].userMessage, '目标笔记页面没有加载出可采数据');
    assert.equal(failedCalls[0][2].diagnostic.reasonCode, 'page_data_not_ready');
    assert.equal(failedCalls[0][2].diagnostic.evidence.expectedNoteId, '69fd330a');
    assert.equal(failedCalls[0][2].diagnostic.evidence.monitorId, 'monitor_1');
  } finally {
    globalThis.window = previousWindow;
  }
});

test('collection handlers fail remote xhs author collection when profile target mismatches', async () => {
  const failedCalls = [];
  const previousWindow = globalThis.window;

  globalThis.window = {
    location: {
      href: 'https://www.xiaohongshu.com/user/profile/current_author',
      origin: 'https://www.xiaohongshu.com',
    },
  };

  try {
    const handlers = createCollectionHandlerSet({
      collectAuthor: async () => {
        throw new Error('collectAuthor should not run');
      },
      getPageContext: async () => ({ platform: 'xhs', pageType: 'profile' }),
      collectionRunStore: {
        async createRun() {
          return { collectionRunId: 'run_remote_author_1' };
        },
        async markDone() {
          throw new Error('markDone should not be called');
        },
        async markStopped() {
          throw new Error('markStopped should not be called');
        },
        async markFailed(runId, error, patch) {
          failedCalls.push([runId, error, patch]);
        },
      },
    });

    await assert.rejects(
      () => handlers[MSG.COLLECT_AUTHOR]({
        triggerSource: 'workbench_dispatch',
        externalTaskMeta: {
          externalTaskId: 'task_remote_author_1',
          externalTaskType: 'xhs.collectAuthor',
          executorInstanceId: 'executor_1',
          protocolVersion: 'v1',
          monitorMeta: {
            monitorId: 'monitor_author_1',
            taskStrategy: 'author_baseline',
            targetUrl: 'https://www.xiaohongshu.com/user/profile/target_author',
            display: { name: '目标博主' },
          },
        },
      }),
      (error) => {
        assert.equal(error.code, 'target_mismatch');
        return true;
      },
    );

    assert.equal(failedCalls[0][0], 'run_remote_author_1');
    assert.equal(failedCalls[0][2].itemsPlanned, 51);
    assert.equal(failedCalls[0][2].itemsSucceeded, 0);
    assert.equal(failedCalls[0][2].itemsFailed, 51);
  } finally {
    globalThis.window = previousWindow;
  }
});

test('collection handlers pass profile selector and scan limit into xhs surface discovery', async () => {
  const discoverCalls = [];
  const bulkUpsertCalls = [];
  const doneCalls = [];
  const previousWindow = globalThis.window;

  globalThis.window = {
    location: {
      href: 'https://www.xiaohongshu.com/user/profile/target_author',
      pathname: '/user/profile/target_author',
      origin: 'https://www.xiaohongshu.com',
    },
  };

  try {
    const handlers = createCollectionHandlerSet({
      collectAuthor: async () => ({
        platformAuthorId: 'target_author',
        userId: 'target_author',
        name: '目标博主',
      }),
      noteStore: {
        async bulkUpsert(records) {
          bulkUpsertCalls.push(records);
        },
      },
      getPageContext: async () => ({ platform: 'xhs', pageType: 'profile' }),
      collectionRunStore: {
        async createRun() {
          return { collectionRunId: 'run_author_surface_1' };
        },
        async markDone(runId, patch) {
          doneCalls.push([runId, patch]);
        },
        async markStopped() {
          throw new Error('markStopped should not be called');
        },
        async markFailed() {
          throw new Error('markFailed should not be called');
        },
      },
      discoverXhsSurfaceNotes: async (...args) => {
        discoverCalls.push(args);
        return [
          { noteId: 'note_1', url: '/explore/note_1', title: '第一条' },
          { noteId: 'note_2', url: '/explore/note_2', title: '第二条' },
        ];
      },
    });

    const result = await handlers[MSG.COLLECT_AUTHOR]({
      triggerSource: 'workbench_dispatch',
      externalTaskMeta: {
        externalTaskId: 'task_author_surface_1',
        externalTaskType: 'xhs.collectAuthor',
        executorInstanceId: 'executor_1',
        protocolVersion: 'v1',
        monitorMeta: {
          monitorId: 'monitor_author_surface_1',
          taskStrategy: 'author_patrol',
          targetUrl: 'https://www.xiaohongshu.com/user/profile/target_author',
          scanLimit: 7,
          limit: 7,
          surfaceOnly: true,
          surfaceMode: 'author_surface',
          monitorMode: 'author_surface',
          display: { name: '目标博主' },
        },
      },
    });

    assert.equal(result.success, true);
    assert.deepEqual(discoverCalls, [['#userPostedFeeds', 10, { expectedCount: 7 }]]);
    assert.equal(bulkUpsertCalls.length, 1);
    assert.equal(bulkUpsertCalls[0].length, 2);
    assert.equal(doneCalls[0][1].itemsPlanned, 3);
    assert.equal(doneCalls[0][1].itemsSucceeded, 3);
  } finally {
    globalThis.window = previousWindow;
  }
});

test('collection handlers keep xhs author note links in the terminal result package', async () => {
  const createRunCalls = [];
  const discoverCalls = [];
  const bulkUpsertCalls = [];
  const reportedRecords = [];
  const doneCalls = [];
  const previousWindow = globalThis.window;

  globalThis.window = {
    location: {
      href: 'https://www.xiaohongshu.com/user/profile/target_author',
      pathname: '/user/profile/target_author',
      origin: 'https://www.xiaohongshu.com',
    },
  };

  try {
    const handlers = createCollectionHandlerSet({
      noteStore: {
        async bulkUpsert(records) {
          bulkUpsertCalls.push(records);
        },
      },
      reportWorkbenchRecord: (record) => {
        reportedRecords.push(record);
      },
      getPageContext: async () => ({ platform: 'xhs', pageType: 'profile' }),
      collectionRunStore: {
        async createRun(input) {
          createRunCalls.push(input);
          return { collectionRunId: 'run_author_note_links_1' };
        },
        async markDone(runId, patch) {
          doneCalls.push([runId, patch]);
        },
        async markStopped() {
          throw new Error('markStopped should not be called');
        },
        async markFailed() {
          throw new Error('markFailed should not be called');
        },
      },
      discoverXhsSurfaceNotes: async (...args) => {
        discoverCalls.push(args);
        const cards = [
          {
            noteId: 'note_1',
            url: 'https://www.xiaohongshu.com/explore/note_1?xsec_token=token_1&xsec_source=pc_user',
            title: '第一条历史笔记',
            likes: '1,234',
            cover: 'https://img.example.com/1.jpg',
          },
          {
            noteId: 'note_2',
            url: '/explore/note_2',
            title: '第二条历史笔记',
            likes: '99',
          },
        ];
        Object.defineProperty(cards, 'discoveryMeta', {
          value: {
            method: 'dom_scroll_persistent_map',
            stopReason: 'max_rounds_reached',
            totalNotes: 2,
            rounds: 12,
            maxRounds: 12,
            canLoadMore: true,
            isFinished: false,
            fieldQuality: {
              totalNotes: 2,
              withTitle: 2,
              withLikeText: 2,
              withLikeCount: 2,
              withCover: 1,
              withXsecToken: 1,
              pinnedCount: 0,
            },
          },
        });
        return cards;
      },
    });

    const result = await handlers[MSG.DISCOVER_AUTHOR_NOTE_LINKS]({
      triggerSource: 'workbench_dispatch',
      maxLinks: 3,
      maxScrolls: 12,
      profileUrl: 'https://www.xiaohongshu.com/user/profile/target_author',
      authorPlatformId: 'target_author',
      authorName: '目标博主',
      authorArchiveJobId: 'archive_job_1',
      authorArchiveStage: 'link_discovery',
      externalTaskMeta: {
        externalTaskId: 'task_author_note_links_1',
        externalTaskType: 'xhs.authorNoteLinks',
        executorInstanceId: 'executor_1',
        protocolVersion: 'v1',
      },
    });

    assert.equal(result.success, true);
    assert.equal(result.total, 2);
    assert.equal(createRunCalls[0].taskType, 'authorNoteLinks');
    assert.equal(createRunCalls[0].config.requestedCount, 3);
    assert.equal(createRunCalls[0].config.authorArchiveJobId, 'archive_job_1');
    assert.deepEqual(discoverCalls, [['#userPostedFeeds', 12, { expectedCount: 3 }]]);
    assert.equal(bulkUpsertCalls.length, 1);
    assert.equal(bulkUpsertCalls[0].length, 2);
    assert.equal(bulkUpsertCalls[0][0].dataSource, 'author_note_link_discovery');
    assert.equal(bulkUpsertCalls[0][0].signedUrl, 'https://www.xiaohongshu.com/explore/note_1?xsec_token=token_1&xsec_source=pc_user');
    assert.equal(bulkUpsertCalls[0][0].xsecToken, 'token_1');
    assert.equal(bulkUpsertCalls[0][0].authorPlatformId, 'target_author');
    assert.equal(bulkUpsertCalls[0][0].authorArchiveJobId, 'archive_job_1');
    assert.equal(bulkUpsertCalls[0][1].sourceUrl, 'https://www.xiaohongshu.com/explore/note_2');
    assert.equal(bulkUpsertCalls[0][1].signedUrl, undefined);
    assert.equal(reportedRecords.length, 0);
    assert.equal(doneCalls[0][0], 'run_author_note_links_1');
    assert.equal(doneCalls[0][1].itemsPlanned, 2);
    assert.equal(doneCalls[0][1].itemsSucceeded, 2);
    assert.deepEqual(doneCalls[0][1].targetIds, ['note_1', 'note_2']);
    assert.deepEqual(doneCalls[0][1].contentIds, ['xhs_note_1', 'xhs_note_2']);
    assert.equal(doneCalls[0][1].requestedCount, 3);
    assert.equal(doneCalls[0][1].discoveredCount, 2);
    assert.equal(doneCalls[0][1].shortfallCount, 1);
    assert.equal(doneCalls[0][1].discoverySummary.stopReason, 'max_rounds_reached');
    assert.equal(doneCalls[0][1].discoverySummary.rounds, 12);
    assert.equal(doneCalls[0][1].discoverySummary.fieldQuality.withXsecToken, 1);
    assert.match(doneCalls[0][1].completionNote, /安全滚动上限/);
  } finally {
    globalThis.window = previousWindow;
  }
});

test('collection handlers write douyin author surface records for monitor author tasks', async () => {
  const discoverCalls = [];
  const bulkUpsertCalls = [];
  const doneCalls = [];
  const reportedRecords = [];
  const previousWindow = globalThis.window;

  globalThis.window = {
    location: {
      href: 'https://www.douyin.com/user/MS4wLjABAAAAauthor_1?from_tab_name=main',
      pathname: '/user/MS4wLjABAAAAauthor_1',
      search: '?from_tab_name=main',
      origin: 'https://www.douyin.com',
    },
  };

  try {
    const handlers = createCollectionHandlerSet({
      isDouyinPage: () => true,
      collectDouyinAuthor: async (options = {}) => ({
        ok: true,
        data: {
          platformAuthorId: 'MS4wLjABAAAAauthor_1',
          userId: 'MS4wLjABAAAAauthor_1',
          name: '抖音作者',
          collectionRunId: options.collectionRunId,
        },
      }),
      noteStore: {
        async bulkUpsert(records) {
          bulkUpsertCalls.push(records);
        },
      },
      reportWorkbenchRecord: (record) => {
        reportedRecords.push(record);
      },
      getPageContext: async () => ({ platform: 'douyin', pageType: 'profile' }),
      collectionRunStore: {
        async createRun() {
          return { collectionRunId: 'run_douyin_author_surface_1' };
        },
        async markDone(runId, patch) {
          doneCalls.push([runId, patch]);
        },
        async markStopped() {
          throw new Error('markStopped should not be called');
        },
        async markFailed() {
          throw new Error('markFailed should not be called');
        },
      },
      discoverDouyinSurfaceTargets: async (...args) => {
        discoverCalls.push(args);
        return [
          { awemeId: '7601151661356158258', titleHint: '爸爸怎么指导我考上高中', likes: 66, comments: 23 },
          { awemeId: '7601089919720656178', titleHint: '辅导作业adhd家长谈', likes: 21, comments: 10 },
        ];
      },
    });

    const result = await handlers[MSG.COLLECT_AUTHOR]({
      triggerSource: 'workbench_dispatch',
      count: 5,
      externalTaskMeta: {
        externalTaskId: 'task_douyin_author_surface_1',
        externalTaskType: 'douyin.collectAuthor',
        executorInstanceId: 'executor_1',
        protocolVersion: 'v1',
        monitorMeta: {
          monitorId: 'monitor_douyin_author_1',
          taskStrategy: 'author_baseline',
          targetUrl: 'https://www.douyin.com/user/MS4wLjABAAAAauthor_1?from_tab_name=main',
          scanLimit: 5,
          limit: 5,
          surfaceOnly: true,
          surfaceMode: 'author_surface',
          monitorMode: 'author_surface',
          display: { name: '抖音作者' },
        },
      },
    });

    assert.equal(result.success, true);
    assert.equal(discoverCalls.length, 1);
    assert.equal(discoverCalls[0][0].maxCount, 5);
    assert.equal(discoverCalls[0][0].topByLikes, false);
    assert.equal(typeof discoverCalls[0][0].shouldStop, 'function');
    assert.equal(typeof discoverCalls[0][0].waitIfPaused, 'function');
    assert.equal(bulkUpsertCalls.length, 1);
    assert.equal(bulkUpsertCalls[0].length, 2);
    assert.equal(bulkUpsertCalls[0][0].platform, 'douyin');
    assert.equal(bulkUpsertCalls[0][0].collectionRunId, 'run_douyin_author_surface_1');
    assert.equal(bulkUpsertCalls[0][0].qualityReason, 'monitor_surface_seed');
    assert.equal(reportedRecords.length, 3);
    assert.equal(reportedRecords[0].recordType, 'author');
    assert.equal(reportedRecords[0].collectionRunId, 'run_douyin_author_surface_1');
    assert.equal(reportedRecords[0].externalTaskId, 'task_douyin_author_surface_1');
    assert.equal(reportedRecords[0].externalRecordId, 'MS4wLjABAAAAauthor_1');
    assert.equal(reportedRecords[1].recordType, 'note');
    assert.equal(reportedRecords[1].collectionRunId, 'run_douyin_author_surface_1');
    assert.equal(reportedRecords[1].externalTaskId, 'task_douyin_author_surface_1');
    assert.equal(reportedRecords[1].externalRecordId, '7601151661356158258');
    assert.equal(reportedRecords[2].recordType, 'note');
    assert.equal(reportedRecords[2].externalRecordId, '7601089919720656178');
    assert.equal(doneCalls[0][0], 'run_douyin_author_surface_1');
    assert.equal(doneCalls[0][1].itemsPlanned, 3);
    assert.equal(doneCalls[0][1].itemsSucceeded, 3);
    assert.deepEqual(doneCalls[0][1].targetIds, [
      'MS4wLjABAAAAauthor_1',
      '7601151661356158258',
      '7601089919720656178',
    ]);
    assert.deepEqual(doneCalls[0][1].contentIds, [
      'dy_7601151661356158258',
      'dy_7601089919720656178',
    ]);
    assert.equal(doneCalls[0][1].requestedCount, 5);
    assert.equal(doneCalls[0][1].discoveredCount, 2);
    assert.equal(doneCalls[0][1].shortfallCount, 3);
  } finally {
    globalThis.window = previousWindow;
  }
});

test('collection handlers reject collection when plugin authorization is missing', async () => {
  const handlers = createCollectionHandlerSet({
    ensurePluginAuthorized: async () => {
      const error = new Error('当前浏览器还没有插件授权。请先去内容工作台设置生成授权码，再回到插件激活。');
      error.code = 'plugin_authorization_required';
      throw error;
    },
    collectNote: async () => {
      throw new Error('collectNote should not run');
    },
  });

  await assert.rejects(
    () => handlers[MSG.COLLECT_SINGLE_NOTE]({ triggerSource: 'popup_manual' }),
    (error) => {
      assert.equal(error.code, 'plugin_authorization_required');
      return true;
    },
  );
});
