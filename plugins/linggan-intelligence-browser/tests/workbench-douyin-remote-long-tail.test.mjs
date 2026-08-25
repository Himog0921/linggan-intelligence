import test from 'node:test';
import assert from 'node:assert/strict';

import { buildCapabilityReport } from '../src/workbench/runtime/capabilityReportBuilder.js';
import { mapTaskEnvelopeToInternalCommand } from '../src/workbench/runtime/taskEnvelopeMapper.js';
import { mapTaskControlToInternalCommand } from '../src/workbench/runtime/taskControlMapper.js';
import { createContentMessageHandlers } from '../src/content/messageHandlers.js';
import { MSG } from '../src/shared/constants.js';
import {
  REMOTE_ERROR_CODE,
  REMOTE_TASK_CONTROL_ACTION,
  WORKBENCH_MESSAGE_TYPE,
  WORKBENCH_PROTOCOL_VERSION,
} from '../src/workbench/protocol/schema.js';

test('buildCapabilityReport declares douyin single comments and comment image download task types on detail page', () => {
  const report = buildCapabilityReport({
    platform: 'douyin',
    mode: 'detail',
    pageType: 'detail',
    url: 'https://www.douyin.com/video/demo',
    isDyVideoPage: true,
    isDyStrictDetailPage: true,
    capabilities: {
      canCollectComments: true,
      canDownloadCommentImages: true,
    },
  });

  assert.deepEqual(report.capabilities.canRunTaskTypes.sort(), [
    'douyin.commentImageDownload',
    'douyin.singleComments',
  ]);
});

test('buildCapabilityReport blocks douyin tasks while platform security verification is visible', () => {
  const report = buildCapabilityReport({
    platform: 'douyin',
    mode: 'unknown',
    pageType: 'unknown',
    url: 'https://www.douyin.com/video/demo',
    platformBlocked: true,
    blockReasonCode: REMOTE_ERROR_CODE.PLATFORM_SECURITY_CHALLENGE,
    blockReasonMessage: '检测到抖音安全验证，请先完成验证后继续操作',
    capabilities: {
      canCollectComments: true,
      canDownloadCommentImages: true,
    },
  });

  assert.equal(report.readiness.ready, false);
  assert.equal(report.readiness.reasonCode, REMOTE_ERROR_CODE.PLATFORM_SECURITY_CHALLENGE);
  assert.equal(report.recommendedNextAction, 'resolve_platform_security_challenge');
  assert.deepEqual(report.capabilities.canRunTaskTypes, []);
  assert.equal(report.contextSnapshot.platformBlocked, true);
});

test('buildCapabilityReport declares xhs detail note and comment probes on detail pages', () => {
  const report = buildCapabilityReport({
    platform: 'xhs',
    mode: 'detail',
    pageType: 'noteDetail',
    url: 'https://www.xiaohongshu.com/discovery/item/demo_note',
    capabilities: {
      canCollectPrimary: true,
      canCollectComments: true,
    },
  });

  assert.deepEqual(report.capabilities.canRunTaskTypes, [
    'xhs.batchNotes',
    'xhs.batchComments',
  ]);
});

test('mapTaskEnvelopeToInternalCommand maps douyin.singleComments to async content comment collection', () => {
  const command = mapTaskEnvelopeToInternalCommand({
    type: 'task.envelope',
    protocolVersion: 'v1',
    taskId: 'wb_task_comment_1',
    taskType: 'douyin.singleComments',
    platform: 'douyin',
    triggerSource: 'workbench_dispatch',
    target: {
      pageType: 'detail',
      url: 'https://www.douyin.com/video/demo',
    },
    payload: {
      commentLimit: 20,
      commentDepthMode: 'twoLevel',
    },
  }, { tabId: 99 });

  assert.equal(command.dispatchTarget, 'content');
  assert.equal(command.action, MSG.COLLECT_SINGLE_COMMENT);
  assert.equal(command.payload.asyncDispatch, true);
  assert.equal(command.payload.maxTotal, 20);
  assert.equal(command.payload.commentDepthMode, 'twoLevel');
  assert.equal(command.payload.externalTaskMeta.externalTaskId, 'wb_task_comment_1');
});

test('mapTaskControlToInternalCommand maps douyin detail long-task controls to content runtime control', () => {
  const commentCommand = mapTaskControlToInternalCommand({
    type: WORKBENCH_MESSAGE_TYPE.TASK_CONTROL,
    protocolVersion: WORKBENCH_PROTOCOL_VERSION,
    taskId: 'wb_task_comment_control_1',
    taskType: 'douyin.singleComments',
    action: REMOTE_TASK_CONTROL_ACTION.PAUSE,
  }, { tabId: 99 });

  assert.equal(commentCommand.dispatchTarget, 'content');
  assert.equal(commentCommand.action, MSG.WORKBENCH_TASK_CONTROL);
  assert.equal(commentCommand.payload.command, REMOTE_TASK_CONTROL_ACTION.PAUSE);
  assert.equal(commentCommand.payload.taskControl.taskId, 'wb_task_comment_control_1');

  const imageCommand = mapTaskControlToInternalCommand({
    type: WORKBENCH_MESSAGE_TYPE.TASK_CONTROL,
    protocolVersion: WORKBENCH_PROTOCOL_VERSION,
    taskId: 'wb_task_image_control_1',
    taskType: 'douyin.commentImageDownload',
    action: REMOTE_TASK_CONTROL_ACTION.STOP,
  }, { tabId: 99 });

  assert.equal(imageCommand.dispatchTarget, 'content');
  assert.equal(imageCommand.action, MSG.WORKBENCH_TASK_CONTROL);
  assert.equal(imageCommand.payload.command, REMOTE_TASK_CONTROL_ACTION.STOP);
  assert.equal(imageCommand.payload.taskControl.taskId, 'wb_task_image_control_1');
});

test('remote douyin single comment collection acknowledges async dispatch and binds remote collection run', async () => {
  const calls = [];
  let collectorOptions = null;
  let releaseCollector;
  const collectorDone = new Promise((resolve) => {
    releaseCollector = resolve;
  });

  const handlers = createContentMessageHandlers({
    MSG,
    isDouyinPage: () => true,
    collectDouyinComments: async (options = {}) => {
      collectorOptions = options;
      await collectorDone;
      return {
        total: 12,
        comments: [{ commentId: 'c1' }],
        note: { contentId: 'douyin_note_1', platformContentId: 'aweme_1' },
        collectionRunId: options.collectionRunId,
      };
    },
    downloadDouyinCommentImages: async () => ({ success: true }),
    collectAuthor: async () => ({}),
    collectDouyinAuthor: async () => ({ ok: true, data: {} }),
    noteStore: { count: async () => 0, getAll: async () => [] },
    commentStore: { count: async () => 0, getAll: async () => [] },
    authorStore: { count: async () => 0, getAll: async () => [] },
    reportDone: () => {},
    batchMessageHandlers: {},
    extractNoteId: () => '',
    downloadNoteMediaFromRecord: async () => ({}),
    generateCsv: () => '',
    downloadFile: () => {},
    backfillLegacyAiReadyFields: async () => ({}),
    getPageContext: async () => ({ platform: 'douyin', pageType: 'detail' }),
    collectionRunStore: {
      async createRun(input) {
        calls.push(['createRun', input]);
        return { collectionRunId: 'run_dy_comment_1' };
      },
      async markDone(runId, payload) {
        calls.push(['markDone', runId, payload]);
      },
      async markFailed(runId, error, payload) {
        calls.push(['markFailed', runId, error, payload]);
      },
      async markStopped(runId, payload) {
        calls.push(['markStopped', runId, payload]);
      },
    },
  });

  const result = await handlers[MSG.COLLECT_SINGLE_COMMENT]({
    asyncDispatch: true,
    triggerSource: 'workbench_dispatch',
    maxTotal: 20,
    commentDepthMode: 'twoLevel',
    externalTaskMeta: {
      externalTaskId: 'wb_task_comment_1',
      externalTaskType: 'douyin.singleComments',
      protocolVersion: 'v1',
    },
  });

  assert.equal(result.success, true);
  assert.equal(result.accepted, true);
  assert.equal(result.pending, true);
  assert.equal(result.collectionRunId, 'run_dy_comment_1');
  assert.equal(calls.length, 1);
  assert.equal(calls[0][0], 'createRun');

  await new Promise((resolve) => setTimeout(resolve, 0));

  assert.equal(calls[0][1].externalTaskId, 'wb_task_comment_1');
  assert.equal(calls[0][1].externalTaskType, 'douyin.singleComments');
  assert.equal(calls[0][1].taskType, 'singleComments');
  assert.equal(calls[0][1].platform, 'douyin');
  assert.equal(calls[0][1].resultUploadStatus, 'pending_upload');
  assert.equal(collectorOptions.collectionRunId, 'run_dy_comment_1');
  assert.equal(collectorOptions.manageCollectionRun, false);

  releaseCollector();
  await new Promise((resolve) => setTimeout(resolve, 0));

  assert.equal(calls[1][0], 'markDone');
  assert.equal(calls[1][1], 'run_dy_comment_1');
  assert.equal(calls[1][2].totalComments, 12);
});

test('remote douyin single comment collection accepts workbench stop control', async () => {
  const calls = [];
  let collectorOptions = null;
  let releaseCollector;
  const collectorStarted = new Promise((resolve) => {
    releaseCollector = resolve;
  });

  const handlers = createContentMessageHandlers({
    MSG,
    isDouyinPage: () => true,
    collectDouyinComments: async (options = {}) => {
      collectorOptions = options;
      await collectorStarted;
      return {
        stopped: options.shouldStop(),
        total: 2,
        comments: [{ commentId: 'c1' }, { commentId: 'c2' }],
        note: { contentId: 'douyin_note_control', platformContentId: 'aweme_control' },
        collectionRunId: options.collectionRunId,
      };
    },
    downloadDouyinCommentImages: async () => ({ success: true }),
    collectAuthor: async () => ({}),
    collectDouyinAuthor: async () => ({ ok: true, data: {} }),
    noteStore: { count: async () => 0, getAll: async () => [] },
    commentStore: { count: async () => 0, getAll: async () => [] },
    authorStore: { count: async () => 0, getAll: async () => [] },
    reportDone: () => {},
    batchMessageHandlers: {},
    extractNoteId: () => '',
    downloadNoteMediaFromRecord: async () => ({}),
    generateCsv: () => '',
    downloadFile: () => {},
    backfillLegacyAiReadyFields: async () => ({}),
    getPageContext: async () => ({ platform: 'douyin', pageType: 'detail' }),
    collectionRunStore: {
      async createRun(input) {
        calls.push(['createRun', input]);
        return { collectionRunId: 'run_dy_comment_control' };
      },
      async markDone(runId, payload) {
        calls.push(['markDone', runId, payload]);
      },
      async markFailed(runId, error, payload) {
        calls.push(['markFailed', runId, error, payload]);
      },
      async markStopped(runId, payload) {
        calls.push(['markStopped', runId, payload]);
      },
    },
  });

  await handlers[MSG.COLLECT_SINGLE_COMMENT]({
    asyncDispatch: true,
    triggerSource: 'workbench_dispatch',
    maxTotal: 20,
    commentDepthMode: 'twoLevel',
    externalTaskMeta: {
      externalTaskId: 'wb_task_comment_control_1',
      externalTaskType: 'douyin.singleComments',
      protocolVersion: WORKBENCH_PROTOCOL_VERSION,
    },
  });

  await new Promise((resolve) => setTimeout(resolve, 0));
  assert.equal(typeof collectorOptions.shouldStop, 'function');
  assert.equal(typeof collectorOptions.waitIfPaused, 'function');

  const stopResult = await handlers[MSG.WORKBENCH_TASK_CONTROL]({
    command: REMOTE_TASK_CONTROL_ACTION.STOP,
    taskControl: {
      type: WORKBENCH_MESSAGE_TYPE.TASK_CONTROL,
      protocolVersion: WORKBENCH_PROTOCOL_VERSION,
      taskId: 'wb_task_comment_control_1',
      taskType: 'douyin.singleComments',
      action: REMOTE_TASK_CONTROL_ACTION.STOP,
    },
  });

  assert.equal(stopResult.success, true);
  assert.equal(stopResult.accepted, true);

  releaseCollector();
  await new Promise((resolve) => setTimeout(resolve, 0));

  const stopped = calls.find((entry) => entry[0] === 'markStopped');
  assert.ok(stopped);
  assert.equal(stopped[1], 'run_dy_comment_control');
  assert.equal(stopped[2].itemsSucceeded, 1);
});

test('remote douyin single comment collection waits while paused and resumes cleanly', async () => {
  const calls = [];
  let collectorOptions = null;
  let allowCollectorToReachPauseCheck;
  let waitStarted = false;
  let waitFinished = false;
  const collectorCanReachPauseCheck = new Promise((resolve) => {
    allowCollectorToReachPauseCheck = resolve;
  });

  const handlers = createContentMessageHandlers({
    MSG,
    isDouyinPage: () => true,
    collectDouyinComments: async (options = {}) => {
      collectorOptions = options;
      await collectorCanReachPauseCheck;
      waitStarted = true;
      await options.waitIfPaused();
      waitFinished = true;
      return {
        total: 1,
        comments: [{ commentId: 'c1' }],
        note: { contentId: 'douyin_note_pause', platformContentId: 'aweme_pause' },
        collectionRunId: options.collectionRunId,
      };
    },
    downloadDouyinCommentImages: async () => ({ success: true }),
    collectAuthor: async () => ({}),
    collectDouyinAuthor: async () => ({ ok: true, data: {} }),
    noteStore: { count: async () => 0, getAll: async () => [] },
    commentStore: { count: async () => 0, getAll: async () => [] },
    authorStore: { count: async () => 0, getAll: async () => [] },
    reportDone: () => {},
    batchMessageHandlers: {},
    extractNoteId: () => '',
    downloadNoteMediaFromRecord: async () => ({}),
    generateCsv: () => '',
    downloadFile: () => {},
    backfillLegacyAiReadyFields: async () => ({}),
    getPageContext: async () => ({ platform: 'douyin', pageType: 'detail' }),
    collectionRunStore: {
      async createRun(input) {
        calls.push(['createRun', input]);
        return { collectionRunId: 'run_dy_comment_pause' };
      },
      async markDone(runId, payload) {
        calls.push(['markDone', runId, payload]);
      },
      async markFailed(runId, error, payload) {
        calls.push(['markFailed', runId, error, payload]);
      },
      async markStopped(runId, payload) {
        calls.push(['markStopped', runId, payload]);
      },
    },
  });

  const result = await handlers[MSG.COLLECT_SINGLE_COMMENT]({
    asyncDispatch: true,
    triggerSource: 'workbench_dispatch',
    maxTotal: 20,
    commentDepthMode: 'twoLevel',
    externalTaskMeta: {
      externalTaskId: 'wb_task_comment_pause_1',
      externalTaskType: 'douyin.singleComments',
      protocolVersion: WORKBENCH_PROTOCOL_VERSION,
    },
  });

  assert.equal(result.success, true);
  assert.equal(result.accepted, true);
  assert.equal(result.pending, true);

  await new Promise((resolve) => setTimeout(resolve, 0));
  assert.equal(typeof collectorOptions.waitIfPaused, 'function');

  const pauseResult = await handlers[MSG.WORKBENCH_TASK_CONTROL]({
    command: REMOTE_TASK_CONTROL_ACTION.PAUSE,
    taskControl: {
      type: WORKBENCH_MESSAGE_TYPE.TASK_CONTROL,
      protocolVersion: WORKBENCH_PROTOCOL_VERSION,
      taskId: 'wb_task_comment_pause_1',
      taskType: 'douyin.singleComments',
      action: REMOTE_TASK_CONTROL_ACTION.PAUSE,
    },
  });

  assert.equal(pauseResult.success, true);
  assert.equal(pauseResult.accepted, true);
  assert.equal(pauseResult.status, 'paused');

  allowCollectorToReachPauseCheck();
  await new Promise((resolve) => setTimeout(resolve, 0));

  assert.equal(waitStarted, true);
  assert.equal(waitFinished, false);
  assert.equal(calls.some((entry) => entry[0] === 'markDone'), false);

  const resumeResult = await handlers[MSG.WORKBENCH_TASK_CONTROL]({
    command: REMOTE_TASK_CONTROL_ACTION.RESUME,
    taskControl: {
      type: WORKBENCH_MESSAGE_TYPE.TASK_CONTROL,
      protocolVersion: WORKBENCH_PROTOCOL_VERSION,
      taskId: 'wb_task_comment_pause_1',
      taskType: 'douyin.singleComments',
      action: REMOTE_TASK_CONTROL_ACTION.RESUME,
    },
  });

  assert.equal(resumeResult.success, true);
  assert.equal(resumeResult.accepted, true);
  assert.equal(resumeResult.status, 'running');

  await new Promise((resolve) => setTimeout(resolve, 0));

  assert.equal(waitFinished, true);
  const done = calls.find((entry) => entry[0] === 'markDone');
  assert.ok(done);
  assert.equal(done[1], 'run_dy_comment_pause');
  assert.equal(done[2].itemsSucceeded, 1);
});

test('remote douyin comment image download acknowledges async dispatch and binds remote collection run', async () => {
  const calls = [];
  let downloaderOptions = null;
  let releaseDownloader;
  const downloaderDone = new Promise((resolve) => {
    releaseDownloader = resolve;
  });

  const handlers = createContentMessageHandlers({
    MSG,
    isDouyinPage: () => true,
    collectDouyinComments: async () => ({ total: 0, comments: [] }),
    downloadDouyinCommentImages: async (options = {}) => {
      downloaderOptions = options;
      await downloaderDone;
      return {
        success: true,
        downloaded: 3,
        failed: 1,
        total: 4,
        hdCount: 2,
        sdCount: 1,
        scannedImages: 4,
        note: { contentId: 'douyin_note_2', platformContentId: 'aweme_2' },
        collectionRunId: options.collectionRunId,
      };
    },
    collectAuthor: async () => ({}),
    collectDouyinAuthor: async () => ({ ok: true, data: {} }),
    noteStore: { count: async () => 0, getAll: async () => [] },
    commentStore: { count: async () => 0, getAll: async () => [] },
    authorStore: { count: async () => 0, getAll: async () => [] },
    reportDone: () => {},
    batchMessageHandlers: {},
    extractNoteId: () => '',
    downloadNoteMediaFromRecord: async () => ({}),
    generateCsv: () => '',
    downloadFile: () => {},
    backfillLegacyAiReadyFields: async () => ({}),
    getPageContext: async () => ({ platform: 'douyin', pageType: 'detail' }),
    collectionRunStore: {
      async createRun(input) {
        calls.push(['createRun', input]);
        return { collectionRunId: 'run_dy_image_1' };
      },
      async markDone(runId, payload) {
        calls.push(['markDone', runId, payload]);
      },
      async markFailed(runId, error, payload) {
        calls.push(['markFailed', runId, error, payload]);
      },
      async markStopped(runId, payload) {
        calls.push(['markStopped', runId, payload]);
      },
    },
  });

  const result = await handlers[MSG.DOWNLOAD_CURRENT_COMMENT_IMAGES]({
    asyncDispatch: true,
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
  assert.equal(result.accepted, true);
  assert.equal(result.pending, true);
  assert.equal(result.collectionRunId, 'run_dy_image_1');
  assert.equal(calls.length, 1);
  assert.equal(calls[0][0], 'createRun');

  await new Promise((resolve) => setTimeout(resolve, 0));

  assert.equal(calls[0][1].taskType, 'commentImageDownload');
  assert.equal(calls[0][1].externalTaskId, 'wb_task_image_1');
  assert.equal(downloaderOptions.collectionRunId, 'run_dy_image_1');
  assert.equal(typeof downloaderOptions.shouldStop, 'function');
  assert.equal(typeof downloaderOptions.waitIfPaused, 'function');

  releaseDownloader();
  await new Promise((resolve) => setTimeout(resolve, 0));

  assert.equal(calls[1][0], 'markDone');
  assert.equal(calls[1][1], 'run_dy_image_1');
  assert.equal(calls[1][2].itemsSucceeded, 3);
  assert.equal(calls[1][2].totalImages, 4);
});

test('remote douyin comment image download waits while paused and resumes cleanly', async () => {
  const calls = [];
  let downloaderOptions = null;
  let allowDownloaderToReachPauseCheck;
  let waitStarted = false;
  let waitFinished = false;
  const downloaderCanReachPauseCheck = new Promise((resolve) => {
    allowDownloaderToReachPauseCheck = resolve;
  });

  const handlers = createContentMessageHandlers({
    MSG,
    isDouyinPage: () => true,
    collectDouyinComments: async () => ({ total: 0, comments: [] }),
    downloadDouyinCommentImages: async (options = {}) => {
      downloaderOptions = options;
      await downloaderCanReachPauseCheck;
      waitStarted = true;
      await options.waitIfPaused();
      waitFinished = true;
      return {
        success: true,
        downloaded: 1,
        failed: 0,
        total: 1,
        hdCount: 1,
        sdCount: 0,
        scannedImages: 1,
        note: { contentId: 'douyin_note_image_pause', platformContentId: 'aweme_image_pause' },
        collectionRunId: options.collectionRunId,
      };
    },
    collectAuthor: async () => ({}),
    collectDouyinAuthor: async () => ({ ok: true, data: {} }),
    noteStore: { count: async () => 0, getAll: async () => [] },
    commentStore: { count: async () => 0, getAll: async () => [] },
    authorStore: { count: async () => 0, getAll: async () => [] },
    reportDone: () => {},
    batchMessageHandlers: {},
    extractNoteId: () => '',
    downloadNoteMediaFromRecord: async () => ({}),
    generateCsv: () => '',
    downloadFile: () => {},
    backfillLegacyAiReadyFields: async () => ({}),
    getPageContext: async () => ({ platform: 'douyin', pageType: 'detail' }),
    collectionRunStore: {
      async createRun(input) {
        calls.push(['createRun', input]);
        return { collectionRunId: 'run_dy_image_pause' };
      },
      async markDone(runId, payload) {
        calls.push(['markDone', runId, payload]);
      },
      async markFailed(runId, error, payload) {
        calls.push(['markFailed', runId, error, payload]);
      },
      async markStopped(runId, payload) {
        calls.push(['markStopped', runId, payload]);
      },
    },
  });

  const result = await handlers[MSG.DOWNLOAD_CURRENT_COMMENT_IMAGES]({
    asyncDispatch: true,
    triggerSource: 'workbench_dispatch',
    maxTotal: 10,
    commentDepthMode: 'twoLevel',
    externalTaskMeta: {
      externalTaskId: 'wb_task_image_pause_1',
      externalTaskType: 'douyin.commentImageDownload',
      protocolVersion: WORKBENCH_PROTOCOL_VERSION,
    },
  });

  assert.equal(result.success, true);
  assert.equal(result.accepted, true);
  assert.equal(result.pending, true);
  assert.equal(result.collectionRunId, 'run_dy_image_pause');

  await new Promise((resolve) => setTimeout(resolve, 0));
  assert.equal(typeof downloaderOptions.waitIfPaused, 'function');

  const pauseResult = await handlers[MSG.WORKBENCH_TASK_CONTROL]({
    command: REMOTE_TASK_CONTROL_ACTION.PAUSE,
    taskControl: {
      type: WORKBENCH_MESSAGE_TYPE.TASK_CONTROL,
      protocolVersion: WORKBENCH_PROTOCOL_VERSION,
      collectionRunId: 'run_dy_image_pause',
      taskType: 'douyin.commentImageDownload',
      action: REMOTE_TASK_CONTROL_ACTION.PAUSE,
    },
  });

  assert.equal(pauseResult.success, true);
  assert.equal(pauseResult.accepted, true);
  assert.equal(pauseResult.status, 'paused');

  allowDownloaderToReachPauseCheck();
  await new Promise((resolve) => setTimeout(resolve, 0));

  assert.equal(waitStarted, true);
  assert.equal(waitFinished, false);
  assert.equal(calls.some((entry) => entry[0] === 'markDone'), false);

  const resumeResult = await handlers[MSG.WORKBENCH_TASK_CONTROL]({
    command: REMOTE_TASK_CONTROL_ACTION.RESUME,
    taskControl: {
      type: WORKBENCH_MESSAGE_TYPE.TASK_CONTROL,
      protocolVersion: WORKBENCH_PROTOCOL_VERSION,
      collectionRunId: 'run_dy_image_pause',
      taskType: 'douyin.commentImageDownload',
      action: REMOTE_TASK_CONTROL_ACTION.RESUME,
    },
  });

  assert.equal(resumeResult.success, true);
  assert.equal(resumeResult.accepted, true);
  assert.equal(resumeResult.status, 'running');

  await new Promise((resolve) => setTimeout(resolve, 0));

  assert.equal(waitFinished, true);
  const done = calls.find((entry) => entry[0] === 'markDone');
  assert.ok(done);
  assert.equal(done[1], 'run_dy_image_pause');
  assert.equal(done[2].itemsSucceeded, 1);
  assert.equal(done[2].totalImages, 1);
});
