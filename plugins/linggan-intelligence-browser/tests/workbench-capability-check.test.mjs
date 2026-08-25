import test from 'node:test';
import assert from 'node:assert/strict';

import { canDispatchTaskFromCapabilityReport } from '../src/workbench/runtime/capabilityCheck.js';
import {
  mapTaskEnvelopeToCapabilityCheck,
  mapTaskEnvelopeToInternalCommand,
} from '../src/workbench/runtime/taskEnvelopeMapper.js';
import { buildCapabilityReport } from '../src/workbench/runtime/capabilityReportBuilder.js';
import { createXhsPlatformAdapter } from '../src/platforms/xhs/adapter.js';

test('canDispatchTaskFromCapabilityReport accepts ready task types', () => {
  const result = canDispatchTaskFromCapabilityReport({
    readiness: { ready: true, reasonCode: '', reasonMessage: '' },
    capabilities: {
      canRunTaskTypes: ['douyin.batchComments'],
    },
  }, 'douyin.batchComments', { pageType: 'search' });

  assert.equal(result.accepted, true);
  assert.equal(result.reasonCode, '');
});

test('capability report exposes xhs author note link discovery on profile pages', () => {
  const report = buildCapabilityReport({
    platform: 'xhs',
    mode: 'profile',
    pageType: 'profile',
    url: 'https://www.xiaohongshu.com/user/profile/author_1',
    capabilities: {
      canBatchNotes: true,
      canCollectAuthor: true,
    },
  });

  assert.deepEqual(report.capabilities.canRunTaskTypes, [
    'xhs.batchNotes',
    'xhs.collectAuthor',
    'xhs.authorNoteLinks',
  ]);
  assert.equal(report.readiness.ready, true);
});

test('xhs capability report exposes app-scan verification as login required', async () => {
  const adapter = createXhsPlatformAdapter({
    detectPage: () => ({
      type: 'unknown',
      url: 'https://www.xiaohongshu.com/login',
    }),
    getWindow: () => ({
      location: { href: 'https://www.xiaohongshu.com/login' },
      document: {
        title: '小红书登录',
        body: { innerText: '为保护账号安全，请使用已登录该账号的小红书APP扫码验证身份' },
      },
    }),
  });

  const report = await adapter.checkCapability({}, {});

  assert.equal(report.readiness.ready, false);
  assert.equal(report.readiness.reasonCode, 'login_required');
  assert.match(report.readiness.reasonMessage, /扫码验证身份/);
  assert.deepEqual(report.capabilities.canRunTaskTypes, []);
});

test('canDispatchTaskFromCapabilityReport rejects unsupported task types', () => {
  const result = canDispatchTaskFromCapabilityReport({
    readiness: { ready: true, reasonCode: '', reasonMessage: '' },
    capabilities: {
      canRunTaskTypes: ['douyin.batchNotes'],
    },
  }, 'douyin.batchComments', { pageType: 'search' });

  assert.equal(result.accepted, false);
  assert.equal(result.reasonCode, 'unsupported_task_type');
});

test('canDispatchTaskFromCapabilityReport accepts profile-native xhs task names through capability aliases', () => {
  const noteFull = canDispatchTaskFromCapabilityReport({
    mode: 'detail',
    readiness: { ready: true, reasonCode: '', reasonMessage: '' },
    capabilities: {
      canRunTaskTypes: ['xhs.batchNotes'],
    },
  }, 'xhs.note_full', { pageType: 'detail' });
  const authorLinks = canDispatchTaskFromCapabilityReport({
    mode: 'profile',
    readiness: { ready: true, reasonCode: '', reasonMessage: '' },
    capabilities: {
      canRunTaskTypes: ['xhs.authorNoteLinks'],
    },
  }, 'xhs.author_links', { pageType: 'profile' });

  assert.equal(noteFull.accepted, true);
  assert.equal(authorLinks.accepted, true);
});

test('canDispatchTaskFromCapabilityReport forwards readiness failures', () => {
  const result = canDispatchTaskFromCapabilityReport({
    readiness: {
      ready: false,
      reasonCode: 'search_list_unstable',
      reasonMessage: '搜索结果列表尚未形成稳定状态',
    },
    capabilities: {
      canRunTaskTypes: ['douyin.batchComments'],
    },
  }, 'douyin.batchComments', { pageType: 'search' });

  assert.equal(result.accepted, false);
  assert.equal(result.reasonCode, 'search_list_unstable');
});

test('canDispatchTaskFromCapabilityReport preserves unavailable profile-page readiness over unsupported author links', () => {
  const result = canDispatchTaskFromCapabilityReport({
    platform: 'xhs',
    mode: 'unknown',
    pageType: 'unknown',
    url: 'https://www.xiaohongshu.com/login',
    readiness: {
      ready: false,
      reasonCode: 'page_context_unavailable',
      reasonMessage: '当前页面未形成可执行上下文',
    },
    capabilities: {
      canRunTaskTypes: [],
    },
  }, 'xhs.authorNoteLinks', {
    pageType: 'profile',
    url: 'https://www.xiaohongshu.com/user/profile/6926d8f4000000003702c666',
  });

  assert.equal(result.accepted, false);
  assert.equal(result.reasonCode, 'page_context_unavailable');
  assert.equal(result.reasonMessage, '当前页面未形成可执行上下文');
});

test('canDispatchTaskFromCapabilityReport prioritizes platform security verification over unsupported task type', () => {
  const result = canDispatchTaskFromCapabilityReport({
    readiness: {
      ready: false,
      reasonCode: 'platform_security_challenge',
      reasonMessage: '检测到抖音安全验证，请先完成验证后继续操作',
    },
    contextSnapshot: {
      platformBlocked: true,
    },
    capabilities: {
      canRunTaskTypes: [],
    },
  }, 'douyin.singleComments', { pageType: 'detail' });

  assert.equal(result.accepted, false);
  assert.equal(result.reasonCode, 'platform_security_challenge');
  assert.match(result.reasonMessage, /安全验证/);
});

test('canDispatchTaskFromCapabilityReport keeps unavailable detail page reason over unsupported task type', () => {
  const result = canDispatchTaskFromCapabilityReport({
    mode: 'unknown',
    url: 'https://www.xiaohongshu.com/explore/deleted-note',
    readiness: {
      ready: false,
      reasonCode: 'content_not_found',
      reasonMessage: '当前笔记已删除或不可访问',
    },
    capabilities: {
      canRunTaskTypes: [],
    },
  }, 'xhs.batchComments', {
    pageType: 'detail',
    url: 'https://www.xiaohongshu.com/explore/deleted-note',
  });

  assert.equal(result.accepted, false);
  assert.equal(result.reasonCode, 'content_not_found');
  assert.equal(result.reasonMessage, '当前笔记已删除或不可访问');
});

test('canDispatchTaskFromCapabilityReport rejects page type mismatches for remote detail tasks', () => {
  const result = canDispatchTaskFromCapabilityReport({
    mode: 'profile',
    readiness: { ready: true, reasonCode: '', reasonMessage: '' },
    capabilities: {
      canRunTaskTypes: ['xhs.batchNotes'],
    },
  }, 'xhs.batchNotes', { pageType: 'detail' });

  assert.equal(result.accepted, false);
  assert.equal(result.reasonCode, 'page_type_mismatch');
});

test('canDispatchTaskFromCapabilityReport rejects mismatched profile targets for author tasks', () => {
  const result = canDispatchTaskFromCapabilityReport({
    mode: 'profile',
    url: 'https://www.xiaohongshu.com/user/profile/current-author',
    readiness: { ready: true, reasonCode: '', reasonMessage: '' },
    capabilities: {
      canRunTaskTypes: ['xhs.collectAuthor'],
    },
  }, 'xhs.collectAuthor', {
    pageType: 'profile',
    url: 'https://www.xiaohongshu.com/user/profile/target-author',
  });

  assert.equal(result.accepted, false);
  assert.equal(result.reasonCode, 'page_target_mismatch');
});

test('canDispatchTaskFromCapabilityReport rejects mismatched detail targets', () => {
  const result = canDispatchTaskFromCapabilityReport({
    mode: 'detail',
    url: 'https://www.xiaohongshu.com/discovery/item/69f27f63000000003601c448?xsec_token=old',
    readiness: { ready: true, reasonCode: '', reasonMessage: '' },
    capabilities: {
      canRunTaskTypes: ['xhs.batchNotes'],
    },
  }, 'xhs.batchNotes', {
    pageType: 'detail',
    url: 'https://www.xiaohongshu.com/explore/69fdb9db000000001b021e8d?xsec_token=target',
  });

  assert.equal(result.accepted, false);
  assert.equal(result.reasonCode, 'page_target_mismatch');
});

test('canDispatchTaskFromCapabilityReport accepts matching detail targets across xhs url forms', () => {
  const result = canDispatchTaskFromCapabilityReport({
    mode: 'detail',
    url: 'https://www.xiaohongshu.com/discovery/item/69fdb9db000000001b021e8d?xsec_token=current',
    readiness: { ready: true, reasonCode: '', reasonMessage: '' },
    capabilities: {
      canRunTaskTypes: ['xhs.batchNotes'],
    },
  }, 'xhs.batchNotes', {
    pageType: 'detail',
    url: 'https://www.xiaohongshu.com/explore/69fdb9db000000001b021e8d?xsec_token=target',
  });

  assert.equal(result.accepted, true);
  assert.equal(result.reasonCode, '');
});

test('canDispatchTaskFromCapabilityReport accepts matching xhs profile relay detail targets', () => {
  const result = canDispatchTaskFromCapabilityReport({
    mode: 'detail',
    url: 'https://www.xiaohongshu.com/explore/69fdb9db000000001b021e8d?xsec_token=current',
    readiness: { ready: true, reasonCode: '', reasonMessage: '' },
    capabilities: {
      canRunTaskTypes: ['xhs.batchNotes'],
    },
  }, 'xhs.batchNotes', {
    pageType: 'detail',
    url: 'https://www.xiaohongshu.com/user/profile/6926d8f4000000003702c666/69fdb9db000000001b021e8d',
  });

  assert.equal(result.accepted, true);
  assert.equal(result.reasonCode, '');
});

test('canDispatchTaskFromCapabilityReport validates douyin note detail targets', () => {
  const result = canDispatchTaskFromCapabilityReport({
    mode: 'detail',
    url: 'https://www.douyin.com/note/7321309610927770930',
    readiness: { ready: true, reasonCode: '', reasonMessage: '' },
    capabilities: {
      canRunTaskTypes: ['douyin.singleComments'],
    },
  }, 'douyin.singleComments', {
    pageType: 'detail',
    url: 'https://www.douyin.com/note/7321309610927770931',
  });

  assert.equal(result.accepted, false);
  assert.equal(result.reasonCode, 'page_target_mismatch');
});

test('canDispatchTaskFromCapabilityReport 失效：detail 目标被重定向到首页 → CONTENT_NOT_FOUND（不报 unsupported）', () => {
  // 小红书失效笔记：导航后平台 302 → /404 → /explore 首页，URL 站不住（实机验证 2026-07-07）
  const result = canDispatchTaskFromCapabilityReport({
    mode: 'unknown',
    url: 'https://www.xiaohongshu.com/explore',
    readiness: { ready: false, reasonCode: 'page_context_unavailable', reasonMessage: '当前页面未形成可执行上下文' },
    capabilities: { canRunTaskTypes: [] },
  }, 'xhs.note_full', {
    pageType: 'detail',
    url: 'https://www.xiaohongshu.com/explore/6a22268e000000002202989e?xsec_token=ABtbxvaa',
  });

  assert.equal(result.accepted, false);
  assert.equal(result.reasonCode, 'content_not_found');
  assert.match(result.reasonMessage, /6a22268e/);
});

test('canDispatchTaskFromCapabilityReport 失效：detail 目标落到 /404 中间页 → CONTENT_NOT_FOUND', () => {
  const result = canDispatchTaskFromCapabilityReport({
    mode: 'unknown',
    url: 'https://www.xiaohongshu.com/404?error_code=300031&error_msg=%E5%BD%93%E5%89%8D%E7%AC%94%E8%AE%B0%E6%9A%82%E6%97%B6%E6%97%A0%E6%B3%95%E6%B5%8F%E8%A7%88',
    readiness: { ready: false, reasonCode: '', reasonMessage: '' },
    capabilities: { canRunTaskTypes: [] },
  }, 'xhs.batchNotes', {
    pageType: 'detail',
    url: 'https://www.xiaohongshu.com/explore/6a3cef89000000000702aab2',
  });

  assert.equal(result.accepted, false);
  assert.equal(result.reasonCode, 'content_not_found');
});

test('canDispatchTaskFromCapabilityReport detail 目标正常站住 → 不误判失效', () => {
  const result = canDispatchTaskFromCapabilityReport({
    mode: 'detail',
    url: 'https://www.xiaohongshu.com/explore/67fc0296000000001c010a8f?xsec_token=AB8R',
    readiness: { ready: true, reasonCode: '', reasonMessage: '' },
    capabilities: { canRunTaskTypes: ['xhs.batchNotes'] },
  }, 'xhs.note_full', {
    pageType: 'detail',
    url: 'https://www.xiaohongshu.com/explore/67fc0296000000001c010a8f?xsec_token=AB8R',
  });

  assert.equal(result.accepted, true);
});

test('canDispatchTaskFromCapabilityReport 失效盲区：复用 tab URL 站住但 title 是失效页 → CONTENT_NOT_FOUND（2.0.76）', () => {
  // 复用已有 tab 时 URL 可能停在原地址（contentId 还在，失效检测 URL 条件不成立），
  // 但页面 title 已变"页面不见了/暂时无法浏览"——2.0.76 补 title 判断覆盖此盲区。
  const result = canDispatchTaskFromCapabilityReport({
    mode: 'detail',
    url: 'https://www.xiaohongshu.com/discovery/item/6a3bb945000000001603fcd6?xsec_token=ABE9',
    title: '小红书 - 你访问的页面不见了',
    readiness: { ready: true, reasonCode: '', reasonMessage: '' },
    capabilities: { canRunTaskTypes: ['xhs.batchNotes'] },
  }, 'xhs.batchNotes', {
    pageType: 'detail',
    url: 'https://www.xiaohongshu.com/discovery/item/6a3bb945000000001603fcd6?xsec_token=ABE9',
  });

  assert.equal(result.accepted, false);
  assert.equal(result.reasonCode, 'content_not_found');
});

test('canDispatchTaskFromCapabilityReport 失效盲区：抖音 title 含"视频已删除" → CONTENT_NOT_FOUND', () => {
  const result = canDispatchTaskFromCapabilityReport({
    mode: 'detail',
    url: 'https://www.douyin.com/video/7321309610927770930',
    title: '视频已删除 - 抖音',
    readiness: { ready: true, reasonCode: '', reasonMessage: '' },
    capabilities: { canRunTaskTypes: ['douyin.batchNotes'] },
  }, 'douyin.batchNotes', {
    pageType: 'detail',
    url: 'https://www.douyin.com/video/7321309610927770930',
  });

  assert.equal(result.accepted, false);
  assert.equal(result.reasonCode, 'content_not_found');
});

test('canDispatchTaskFromCapabilityReport 正常笔记 title 不含失效词 → 不误判（title 判断不误伤）', () => {
  const result = canDispatchTaskFromCapabilityReport({
    mode: 'detail',
    url: 'https://www.xiaohongshu.com/explore/67fc0296000000001c010a8f?xsec_token=AB8R',
    title: 'ber 连起床喝水都觉得困难你让我去跑步 - 小红书',
    readiness: { ready: true, reasonCode: '', reasonMessage: '' },
    capabilities: { canRunTaskTypes: ['xhs.batchNotes'] },
  }, 'xhs.note_full', {
    pageType: 'detail',
    url: 'https://www.xiaohongshu.com/explore/67fc0296000000001c010a8f?xsec_token=AB8R',
  });

  assert.equal(result.accepted, true);
});

test('canDispatchTaskFromCapabilityReport 非 detail（search）任务 URL 无 contentId → 保留页面未就绪原因', () => {
  const result = canDispatchTaskFromCapabilityReport({
    mode: 'unknown',
    url: 'https://www.xiaohongshu.com/explore',
    readiness: { ready: false, reasonCode: 'page_context_unavailable', reasonMessage: '' },
    capabilities: { canRunTaskTypes: [] },
  }, 'xhs.batchNotes', {
    pageType: 'search',
    url: 'https://www.xiaohongshu.com/search_result?keyword=x',
  });

  assert.equal(result.accepted, false);
  assert.equal(result.reasonCode, 'page_context_unavailable');
});

test('canDispatchTaskFromCapabilityReport accepts matching douyin note detail targets', () => {
  const result = canDispatchTaskFromCapabilityReport({
    mode: 'detail',
    url: 'https://www.douyin.com/note/7321309610927770930?previous_page=app_code_link',
    readiness: { ready: true, reasonCode: '', reasonMessage: '' },
    capabilities: {
      canRunTaskTypes: ['douyin.singleComments'],
    },
  }, 'douyin.singleComments', {
    pageType: 'detail',
    url: 'https://www.douyin.com/note/7321309610927770930',
  });

  assert.equal(result.accepted, true);
  assert.equal(result.reasonCode, '');
});

test('mapTaskEnvelopeToCapabilityCheck converts task envelope into capability check payload', () => {
  const capabilityCheck = mapTaskEnvelopeToCapabilityCheck({
    type: 'task.envelope',
    protocolVersion: 'v1',
    taskId: 'task_1',
    taskType: 'xhs.collectAuthor',
    platform: 'xhs',
    target: {
      pageType: 'profile',
      url: 'https://www.xiaohongshu.com/user/profile/demo',
    },
    payload: {},
  });

  assert.deepEqual(capabilityCheck, {
    type: 'capability.check',
    protocolVersion: 'v1',
    taskType: 'xhs.collectAuthor',
    platform: 'xhs',
    target: {
      pageType: 'profile',
      url: 'https://www.xiaohongshu.com/user/profile/demo',
    },
  });
});

test('mapTaskEnvelopeToInternalCommand maps xhs author note link discovery to async content task', () => {
  const command = mapTaskEnvelopeToInternalCommand({
    type: 'task.envelope',
    protocolVersion: 'v1',
    taskId: 'task_author_links_1',
    taskType: 'xhs.authorNoteLinks',
    platform: 'xhs',
    taskStrategy: 'deep_collect',
    target: {
      pageType: 'profile',
      url: 'https://www.xiaohongshu.com/user/profile/author_1',
    },
    payload: {
      authorArchiveJobId: 'archive_job_1',
      authorArchiveStage: 'link_discovery',
      authorPlatformId: 'author_1',
      authorName: '目标博主',
      limit: 214,
      maxScrolls: 35,
    },
  });

  assert.equal(command.dispatchTarget, 'content');
  assert.equal(command.action, 'discoverAuthorNoteLinks');
  assert.equal(command.payload.asyncDispatch, true);
  assert.equal(command.payload.maxLinks, 214);
  assert.equal(command.payload.maxScrolls, 35);
  assert.equal(command.payload.authorArchiveJobId, 'archive_job_1');
  assert.equal(command.payload.authorArchiveStage, 'link_discovery');
  assert.equal(command.payload.authorPlatformId, 'author_1');
  assert.equal(command.payload.authorName, '目标博主');
  assert.equal(command.payload.externalTaskMeta.externalTaskId, 'task_author_links_1');
  assert.equal(command.payload.externalTaskMeta.monitorMeta, undefined);
});

test('mapTaskEnvelopeToInternalCommand forwards xhs batch note search filters', () => {
  const command = mapTaskEnvelopeToInternalCommand({
    type: 'task.envelope',
    protocolVersion: 'v1',
    taskId: 'task_xhs_filtered_notes',
    taskType: 'xhs.batchNotes',
    platform: 'xhs',
    target: {
      pageType: 'search',
      url: 'https://www.xiaohongshu.com/search_result?keyword=A',
    },
    payload: {
      limit: 30,
      searchFilters: {
        sortBasis: 'most_commented',
        noteType: 'image',
        publishTime: 'one_week',
      },
    },
  });

  assert.equal(command.action, 'startBatchNotes');
  assert.equal(command.payload.mode, 'search');
  assert.deepEqual(command.payload.searchFilters, {
    sortBasis: 'most_commented',
    noteType: 'image',
    publishTime: 'one_week',
  });
});

test('mapTaskEnvelopeToInternalCommand forwards attached comments for xhs batch notes', () => {
  const command = mapTaskEnvelopeToInternalCommand({
    type: 'task.envelope',
    protocolVersion: 'v1',
    taskId: 'task_xhs_notes_with_comments',
    taskType: 'xhs.batchNotes',
    platform: 'xhs',
    target: {
      pageType: 'search',
      url: 'https://www.xiaohongshu.com/search_result?keyword=A',
    },
    payload: {
      limit: 30,
      includeComments: true,
      commentLimit: 20,
      commentDepthMode: 'allReplies',
    },
  });

  assert.equal(command.action, 'startBatchNotes');
  assert.equal(command.payload.mode, 'search');
  assert.equal(command.payload.includeComments, true);
  assert.equal(command.payload.commentLimit, 20);
  assert.equal(command.payload.commentDepthMode, 'allReplies');
});

test('mapTaskEnvelopeToInternalCommand forwards xhs batch comment search filters', () => {
  const command = mapTaskEnvelopeToInternalCommand({
    type: 'task.envelope',
    protocolVersion: 'v1',
    taskId: 'task_xhs_filtered_comments',
    taskType: 'xhs.batchComments',
    platform: 'xhs',
    target: {
      pageType: 'search',
      url: 'https://www.xiaohongshu.com/search_result?keyword=A',
    },
    payload: {
      limit: 20,
      commentLimit: 40,
      searchFilters: {
        sortBasis: 'latest',
        noteType: 'video',
        publishTime: 'one_day',
      },
    },
  });

  assert.equal(command.action, 'startBatchComments');
  assert.equal(command.payload.mode, 'search');
  assert.deepEqual(command.payload.searchFilters, {
    sortBasis: 'latest',
    noteType: 'video',
    publishTime: 'one_day',
  });
});

test('mapTaskEnvelopeToInternalCommand scopes xhs detail comment probes to the target note', () => {
  const command = mapTaskEnvelopeToInternalCommand({
    type: 'task.envelope',
    protocolVersion: 'v1',
    taskId: 'task_comment_detail_1',
    taskType: 'xhs.batchComments',
    platform: 'xhs',
    taskStrategy: 'detail_probe',
    target: {
      pageType: 'detail',
      url: 'https://www.xiaohongshu.com/explore/69fdb9db000000001b021e8d?xsec_token=target',
    },
    payload: {
      commentLimit: 50,
      noteId: '69fdb9db000000001b021e8d',
    },
  });

  assert.equal(command.action, 'startBatchComments');
  assert.equal(command.payload.mode, 'detail');
  assert.equal(command.payload.commentLimit, 50);
  assert.deepEqual(command.payload.noteList, [
    {
      noteId: '69fdb9db000000001b021e8d',
      url: 'https://www.xiaohongshu.com/explore/69fdb9db000000001b021e8d?xsec_token=target',
    },
  ]);
});
