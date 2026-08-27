import test from 'node:test';
import assert from 'node:assert/strict';

const originalChrome = globalThis.chrome;
globalThis.chrome = {
  runtime: {
    onMessage: { addListener: () => {}, removeListener: () => {} },
    onStartup: { addListener: () => {} },
    onInstalled: { addListener: () => {} },
    lastError: null,
    getManifest: () => ({ version: '0.0.0-test' }),
  },
  tabs: {
    query: async () => [],
    sendMessage: () => {},
    update: async () => {},
  },
  downloads: {
    download: async () => 1,
    remove: async () => {},
    onChanged: { addListener: () => {}, removeListener: () => {} },
  },
  declarativeNetRequest: {
    updateDynamicRules: () => Promise.resolve(),
  },
  action: {
    setBadgeText: async () => {},
    setBadgeBackgroundColor: async () => {},
  },
  alarms: {
    create: () => {},
    onAlarm: { addListener: () => {} },
  },
  debugger: {
    attach: async () => {},
    sendCommand: async () => {},
    detach: async () => {},
  },
  cookies: {
    getAll: async () => [],
  },
  storage: {
    local: {
      get: async () => ({}),
      set: async () => {},
    },
  },
};

const {
  buildBatchCommentsDispatchMessage,
  buildBatchNotesDispatchMessage,
  buildContentScriptUnavailableCapabilityResponse,
  inferPageTypeFromTask,
  normalizeWorkbenchTaskTarget,
} = await import('../src/background/index.js');

globalThis.chrome = originalChrome;

test('xhs detail url maps batchNotes tasks to detail page type', () => {
  assert.equal(
    inferPageTypeFromTask({
      taskType: 'xhs.batchNotes',
      target: 'https://www.xiaohongshu.com/explore/note_123',
    }),
    'detail',
  );
});

test('douyin detail url maps batchNotes tasks to detail page type', () => {
  assert.equal(
    inferPageTypeFromTask({
      taskType: 'douyin.batchNotes',
      target: 'https://www.douyin.com/video/7260000000000000001',
    }),
    'detail',
  );
});

test('douyin note detail url maps batchNotes tasks to detail page type', () => {
  assert.equal(
    inferPageTypeFromTask({
      taskType: 'douyin.batchNotes',
      target: 'https://www.douyin.com/note/7321309610927770930',
    }),
    'detail',
  );
});

test('keyword targets still map batchNotes tasks to search page type', () => {
  assert.equal(
    inferPageTypeFromTask({
      taskType: 'xhs.batchNotes',
      target: '数学启蒙',
    }),
    'search',
  );
  assert.equal(
    inferPageTypeFromTask({
      taskType: 'douyin.batchNotes',
      target: '数学启蒙',
    }),
    'search',
  );
});

test('xhs detail url maps batchComments detail probe tasks to detail page type', () => {
  assert.equal(
    inferPageTypeFromTask({
      taskType: 'xhs.batchComments',
      taskStrategy: 'detail_probe',
      target: 'https://www.xiaohongshu.com/explore/note_123',
    }),
    'detail',
  );
});

test('declared targetPageType from payload wins when monitor detail probe uses profile-style share url', () => {
  assert.equal(
    inferPageTypeFromTask({
      taskType: 'xhs.batchNotes',
      target: 'https://www.xiaohongshu.com/user/profile/6926d8f4000000003702c666/699db5ba000000000e00ff23',
      payload: {
        targetPageType: 'detail',
      },
    }),
    'detail',
  );
});

test('xhs author note link tasks map to profile page type', () => {
  assert.equal(
    inferPageTypeFromTask({
      taskType: 'xhs.authorNoteLinks',
      target: 'https://www.xiaohongshu.com/user/profile/author_1',
    }),
    'profile',
  );
  assert.deepEqual(
    normalizeWorkbenchTaskTarget({
      platform: 'xhs',
      taskType: 'xhs.authorNoteLinks',
      target: 'https://www.xiaohongshu.com/user/profile/author_1',
    }),
    {
      pageType: 'profile',
      url: 'https://www.xiaohongshu.com/user/profile/author_1',
    },
  );
});

test('profile-native xhs task names map to expected page types', () => {
  assert.equal(
    inferPageTypeFromTask({
      taskType: 'xhs.author_links',
      target: 'https://www.xiaohongshu.com/user/profile/author_1',
    }),
    'profile',
  );
  assert.equal(
    inferPageTypeFromTask({
      taskType: 'xhs.note_full',
      target: 'https://www.xiaohongshu.com/discovery/item/note_123',
    }),
    'detail',
  );
  assert.equal(
    inferPageTypeFromTask({
      taskType: 'xhs.list_scan',
      target: 'ADHD',
    }),
    'search',
  );
});

test('xhs detail probe keeps detail dispatch page type when target is an unsigned profile relay url', () => {
  assert.deepEqual(
    normalizeWorkbenchTaskTarget({
      platform: 'xhs',
      taskType: 'xhs.batchNotes',
      taskStrategy: 'detail_probe',
      target: 'https://www.xiaohongshu.com/user/profile/6926d8f4000000003702c666/69baad5e00000000230055ef',
      payload: {
        targetPageType: 'detail',
      },
    }),
    {
      pageType: 'detail',
      url: 'https://www.xiaohongshu.com/user/profile/6926d8f4000000003702c666/69baad5e00000000230055ef',
    },
  );
});

test('xhs detail probe keeps detail dispatch page type when target is a signed profile relay url', () => {
  assert.deepEqual(
    normalizeWorkbenchTaskTarget({
      platform: 'xhs',
      taskType: 'xhs.batchNotes',
      taskStrategy: 'detail_probe',
      target: 'https://www.xiaohongshu.com/user/profile/6926d8f4000000003702c666/69baad5e00000000230055ef?xsec_token=abc123&xsec_source=pc_user',
      payload: {
        targetPageType: 'detail',
      },
    }),
    {
      pageType: 'detail',
      url: 'https://www.xiaohongshu.com/user/profile/6926d8f4000000003702c666/69baad5e00000000230055ef?xsec_token=abc123&xsec_source=pc_user',
    },
  );
});

test('background preserves target note metadata when forwarding xhs profile relay batch notes', () => {
  assert.deepEqual(
    buildBatchNotesDispatchMessage({
      mode: 'detail',
      count: 1,
      targetNoteId: '69baad5e00000000230055ef',
      triggerSource: 'workbench_dispatch',
      externalTaskMeta: {
        externalTaskId: 'task_1',
        externalTaskType: 'xhs.batchNotes',
      },
      monitorMeta: {
        monitorMode: 'detail_probe',
        targetNoteId: '69baad5e00000000230055ef',
      },
      surfaceOnly: false,
    }),
    {
      action: 'startBatchNotes',
      mode: 'detail',
      count: 1,
      targetNoteId: '69baad5e00000000230055ef',
      triggerSource: 'workbench_dispatch',
      externalTaskMeta: {
        externalTaskId: 'task_1',
        externalTaskType: 'xhs.batchNotes',
      },
      monitorMeta: {
        monitorMode: 'detail_probe',
        targetNoteId: '69baad5e00000000230055ef',
      },
      surfaceOnly: false,
    },
  );
});

test('background keeps xhs detail-mode batch notes on the batch route', () => {
  assert.deepEqual(
    buildBatchNotesDispatchMessage({
      mode: 'detail',
      targetNoteId: '69baad5e00000000230055ef',
      triggerSource: 'workbench_dispatch',
      externalTaskMeta: {
        externalTaskId: 'task_2',
        externalTaskType: 'xhs.batchNotes',
      },
      monitorMeta: {
        monitorMode: 'detail_probe',
      },
    }),
    {
      action: 'startBatchNotes',
      mode: 'detail',
      targetNoteId: '69baad5e00000000230055ef',
      triggerSource: 'workbench_dispatch',
      externalTaskMeta: {
        externalTaskId: 'task_2',
        externalTaskType: 'xhs.batchNotes',
      },
      monitorMeta: {
        monitorMode: 'detail_probe',
      },
    },
  );
});

test('background keeps xhs detail-mode batch notes on batch route when attached comments are requested', () => {
  assert.deepEqual(
    buildBatchNotesDispatchMessage({
      mode: 'detail',
      targetNoteId: '69baad5e00000000230055ef',
      triggerSource: 'workbench_dispatch',
      externalTaskMeta: {
        externalTaskId: 'task_2',
        externalTaskType: 'xhs.batchNotes',
      },
      monitorMeta: {
        monitorMode: 'detail_probe',
      },
      includeComments: true,
      commentLimit: 20,
      commentDepthMode: 'twoLevel',
    }),
    {
      action: 'startBatchNotes',
      mode: 'detail',
      targetNoteId: '69baad5e00000000230055ef',
      triggerSource: 'workbench_dispatch',
      externalTaskMeta: {
        externalTaskId: 'task_2',
        externalTaskType: 'xhs.batchNotes',
      },
      monitorMeta: {
        monitorMode: 'detail_probe',
      },
      includeComments: true,
      commentLimit: 20,
      commentDepthMode: 'twoLevel',
    },
  );
});

test('background keeps batch comment fields intact when forwarding to content', () => {
  assert.deepEqual(
    buildBatchCommentsDispatchMessage({
      mode: 'profile',
      count: 3,
      topByLikes: true,
      commentLimit: 50,
      commentDepthMode: 'twoLevel',
      triggerSource: 'workbench_dispatch',
      externalTaskMeta: {
        externalTaskId: 'task_3',
        externalTaskType: 'xhs.batchComments',
      },
    }),
    {
      action: 'startBatchComments',
      mode: 'profile',
      count: 3,
      topByLikes: true,
      commentLimit: 50,
      commentDepthMode: 'twoLevel',
      triggerSource: 'workbench_dispatch',
      externalTaskMeta: {
        externalTaskId: 'task_3',
        externalTaskType: 'xhs.batchComments',
      },
    },
  );
});

test('background capability check returns a readable rejection when content script is missing', () => {
  const result = buildContentScriptUnavailableCapabilityResponse({
    task: {
      taskType: 'douyin.batchComments',
      platform: 'douyin',
      target: {
        pageType: 'search',
        url: 'https://www.douyin.com/search/%E5%92%96%E5%95%A1',
      },
    },
    error: new Error('Could not establish connection. Receiving end does not exist.'),
  });

  assert.equal(result.success, true);
  assert.equal(result.accepted, false);
  assert.equal(result.reasonCode, 'page_context_unavailable');
  assert.match(result.reasonMessage, /没有加载插件内容脚本/);
  assert.equal(result.report.platform, 'douyin');
  assert.equal(result.report.pageType, 'search');
  assert.equal(result.report.contextSnapshot.contentScriptLoaded, false);
  assert.deepEqual(result.report.capabilities.canRunTaskTypes, []);
});
