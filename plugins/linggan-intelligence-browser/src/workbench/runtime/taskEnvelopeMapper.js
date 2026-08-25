import { COMMENT_DEPTH_MODE } from '../../shared/constants.js';
import { extractContentIdentityFromUrl } from '../../shared/targetIdentity.js';
import { validateTaskEnvelope } from '../protocol/validator.js';
import {
  REMOTE_TARGET_PAGE_TYPE,
  REMOTE_TASK_TYPE,
  WORKBENCH_MESSAGE_TYPE,
  WORKBENCH_DISPATCH_TARGET,
  getSupportedRemoteTask,
} from '../protocol/schema.js';
import { buildMonitorTaskMeta } from './monitorTask.js';

function ensurePositiveInteger(value, fallback = 0) {
  const num = Number(value);
  if (!Number.isFinite(num) || num < 0) return fallback;
  return Math.floor(num);
}

function normalizeCommentDepthMode(value) {
  return String(value || COMMENT_DEPTH_MODE.TWO_LEVEL) === COMMENT_DEPTH_MODE.ALL_REPLIES
    ? COMMENT_DEPTH_MODE.ALL_REPLIES
    : COMMENT_DEPTH_MODE.TWO_LEVEL;
}

function normalizeSearchFiltersPayload(value) {
  return value && typeof value === 'object' && !Array.isArray(value)
    ? value
    : undefined;
}

function inferModeFromTarget(target = {}) {
  const pageType = String(target.pageType || '').trim();
  if (pageType === REMOTE_TARGET_PAGE_TYPE.SEARCH) return 'search';
  if (pageType === REMOTE_TARGET_PAGE_TYPE.PROFILE) return 'profile';
  if (pageType === REMOTE_TARGET_PAGE_TYPE.DETAIL) return 'detail';
  return 'unknown';
}

function isBatchNotesTaskType(taskType) {
  return new Set([
    REMOTE_TASK_TYPE.XHS_BATCH_NOTES,
    REMOTE_TASK_TYPE.XHS_LIST_SCAN,
    REMOTE_TASK_TYPE.XHS_NOTE_FULL,
    REMOTE_TASK_TYPE.DOUYIN_BATCH_NOTES,
  ]).has(taskType);
}

function isBatchCommentsTaskType(taskType) {
  return new Set([
    REMOTE_TASK_TYPE.XHS_BATCH_COMMENTS,
    REMOTE_TASK_TYPE.XHS_COMMENT_SCAN,
    REMOTE_TASK_TYPE.DOUYIN_BATCH_COMMENTS,
  ]).has(taskType);
}

function isCollectAuthorTaskType(taskType) {
  return new Set([
    REMOTE_TASK_TYPE.XHS_COLLECT_AUTHOR,
    REMOTE_TASK_TYPE.XHS_AUTHOR_PROFILE,
    REMOTE_TASK_TYPE.DOUYIN_COLLECT_AUTHOR,
  ]).has(taskType);
}

function buildBatchNotesPayload(task = {}) {
  const payload = task.payload || {};
  const monitorMeta = buildMonitorTaskMeta({
    platform: task.platform,
    taskType: task.taskType,
    taskStrategy: task.taskStrategy,
    payload,
    target: task.target,
  });
  const targetNoteId = String(
    monitorMeta?.targetNoteId
    || payload.platformContentId
    || payload.noteId
    || payload.contentId
    || '',
  ).trim().replace(/^xhs_/, '');
  const isMonitorDetailProbe = String(monitorMeta?.monitorMode || '').trim() === 'detail_probe';
  const includeComments = (
    task.taskType === REMOTE_TASK_TYPE.XHS_BATCH_NOTES ||
    task.taskType === REMOTE_TASK_TYPE.XHS_NOTE_FULL
  )
    && Boolean(payload.includeComments || payload.collectComments);
  return {
    mode: inferModeFromTarget(task.target),
    count: ensurePositiveInteger(
      isMonitorDetailProbe && targetNoteId
        ? 1
        : (monitorMeta?.limit ?? payload.limit ?? payload.count),
      isMonitorDetailProbe && targetNoteId ? 1 : 20,
    ),
    topByLikes: Boolean(payload.topByLikes),
    sortMode: String(payload.sortMode || '').trim() || undefined,
    searchFilters: normalizeSearchFiltersPayload(payload.searchFilters),
    ...(includeComments
      ? {
          includeComments: true,
          commentLimit: ensurePositiveInteger(payload.commentLimit, 30),
          commentDepthMode: normalizeCommentDepthMode(payload.commentDepthMode),
        }
      : {}),
    triggerSource: String(task.triggerSource || 'workbench_dispatch').trim() || 'workbench_dispatch',
    surfaceOnly: monitorMeta ? Boolean(monitorMeta.surfaceOnly) : undefined,
    targetNoteId: targetNoteId || undefined,
    monitorMeta: monitorMeta || undefined,
  };
}

function buildBatchCommentsPayload(task = {}) {
  const payload = task.payload || {};
  const mode = inferModeFromTarget(task.target);
  const targetUrl = String(task.target?.url || '').trim();
  const targetNoteId = String(
    payload.platformContentId
    || payload.noteId
    || payload.contentId
    || extractContentIdentityFromUrl(targetUrl)
    || '',
  ).trim().replace(/^xhs_/, '');
  return {
    mode,
    count: mode === 'detail' && targetNoteId
      ? 1
      : ensurePositiveInteger(payload.limit ?? payload.count, 10),
    topByLikes: Boolean(payload.topByLikes),
    commentLimit: ensurePositiveInteger(payload.commentLimit, 0),
    commentDepthMode: normalizeCommentDepthMode(payload.commentDepthMode),
    sortMode: String(payload.sortMode || '').trim() || undefined,
    searchFilters: normalizeSearchFiltersPayload(payload.searchFilters),
    triggerSource: String(task.triggerSource || 'workbench_dispatch').trim() || 'workbench_dispatch',
    noteList: mode === 'detail' && targetNoteId
      ? [{ noteId: targetNoteId, url: targetUrl }]
      : undefined,
  };
}

function buildCollectAuthorPayload(task = {}) {
  const payload = task.payload || {};
  const monitorMeta = buildMonitorTaskMeta({
    platform: task.platform,
    taskType: task.taskType,
    taskStrategy: task.taskStrategy,
    payload,
    target: task.target,
  });
  return {
    triggerSource: String(task.triggerSource || 'workbench_dispatch').trim() || 'workbench_dispatch',
    asyncDispatch: true,
    count: monitorMeta ? ensurePositiveInteger(monitorMeta.limit, 30) : undefined,
    surfaceOnly: monitorMeta ? Boolean(monitorMeta.surfaceOnly) : undefined,
    monitorMeta: monitorMeta || undefined,
  };
}

function buildAuthorNoteLinksPayload(task = {}) {
  const payload = task.payload || {};
  const targetUrl = String(task.target?.url || payload.profileUrl || payload.authorProfileUrl || '').trim();
  const limit = ensurePositiveInteger(
    payload.maxLinks ?? payload.linkLimit ?? payload.limit ?? payload.count,
    200,
  );
  return {
    triggerSource: String(task.triggerSource || 'workbench_dispatch').trim() || 'workbench_dispatch',
    asyncDispatch: true,
    count: limit,
    limit,
    maxLinks: limit,
    maxScrolls: ensurePositiveInteger(payload.maxScrolls, 30),
    authorArchiveJobId: String(payload.authorArchiveJobId || '').trim() || undefined,
    authorArchiveStage: String(payload.authorArchiveStage || '').trim() || undefined,
    profileUrl: String(payload.profileUrl || payload.authorProfileUrl || targetUrl).trim() || undefined,
    authorName: String(payload.authorName || payload.authorNickname || payload.nickname || '').trim() || undefined,
    authorPlatformId: String(payload.authorPlatformId || payload.platformAuthorId || payload.authorId || '').trim() || undefined,
  };
}

function buildSingleCommentsPayload(task = {}) {
  const payload = task.payload || {};
  const commentDepthMode = normalizeCommentDepthMode(payload.commentDepthMode);
  return {
    maxTotal: ensurePositiveInteger(payload.maxTotal ?? payload.commentLimit ?? payload.limit, 0),
    maxSubComments: commentDepthMode === COMMENT_DEPTH_MODE.ALL_REPLIES
      ? 0
      : ensurePositiveInteger(payload.maxSubComments, 0),
    commentDepthMode,
    sortMode: String(payload.sortMode || 'hot').trim() || 'hot',
    triggerSource: String(task.triggerSource || 'workbench_dispatch').trim() || 'workbench_dispatch',
    asyncDispatch: true,
  };
}

function buildCommentImageDownloadPayload(task = {}) {
  const payload = task.payload || {};
  return {
    maxTotal: ensurePositiveInteger(payload.maxTotal ?? payload.limit, 0),
    maxSubComments: ensurePositiveInteger(payload.maxSubComments, 0),
    commentDepthMode: normalizeCommentDepthMode(payload.commentDepthMode),
    triggerSource: String(task.triggerSource || 'workbench_dispatch').trim() || 'workbench_dispatch',
    asyncDispatch: true,
  };
}

function buildInternalPayload(task = {}) {
  switch (task.taskType) {
    case REMOTE_TASK_TYPE.XHS_BATCH_NOTES:
    case REMOTE_TASK_TYPE.XHS_LIST_SCAN:
    case REMOTE_TASK_TYPE.XHS_NOTE_FULL:
    case REMOTE_TASK_TYPE.DOUYIN_BATCH_NOTES:
      return buildBatchNotesPayload(task);
    case REMOTE_TASK_TYPE.XHS_BATCH_COMMENTS:
    case REMOTE_TASK_TYPE.XHS_COMMENT_SCAN:
    case REMOTE_TASK_TYPE.DOUYIN_BATCH_COMMENTS:
      return buildBatchCommentsPayload(task);
    case REMOTE_TASK_TYPE.XHS_COLLECT_AUTHOR:
    case REMOTE_TASK_TYPE.XHS_AUTHOR_PROFILE:
    case REMOTE_TASK_TYPE.DOUYIN_COLLECT_AUTHOR:
      return buildCollectAuthorPayload(task);
    case REMOTE_TASK_TYPE.XHS_AUTHOR_NOTE_LINKS:
    case REMOTE_TASK_TYPE.XHS_AUTHOR_LINKS:
      return buildAuthorNoteLinksPayload(task);
    case REMOTE_TASK_TYPE.DOUYIN_SINGLE_COMMENTS:
      return buildSingleCommentsPayload(task);
    case REMOTE_TASK_TYPE.DOUYIN_COMMENT_IMAGE_DOWNLOAD:
      return buildCommentImageDownloadPayload(task);
    default:
      return {};
  }
}

export function mapTaskEnvelopeToInternalCommand(taskEnvelope = {}, { tabId = null } = {}) {
  const validation = validateTaskEnvelope(taskEnvelope);
  if (!validation.valid) {
    const error = new Error('Invalid task envelope');
    error.validation = validation;
    throw error;
  }

  const taskConfig = validation.taskConfig || getSupportedRemoteTask(taskEnvelope.taskType);
  const internalPayload = buildInternalPayload(taskEnvelope);
  const monitorMeta = internalPayload.monitorMeta || (
    taskEnvelope.taskType === REMOTE_TASK_TYPE.XHS_AUTHOR_NOTE_LINKS ||
    taskEnvelope.taskType === REMOTE_TASK_TYPE.XHS_AUTHOR_LINKS
      ? null
      : buildMonitorTaskMeta({
          platform: taskEnvelope.platform,
          taskType: taskEnvelope.taskType,
          taskStrategy: taskEnvelope.taskStrategy,
          payload: taskEnvelope.payload || {},
          target: taskEnvelope.target || {},
        })
  );
  const externalTaskMeta = {
    externalTaskId: String(taskEnvelope.taskId || '').trim(),
    externalTaskType: String(taskEnvelope.taskType || '').trim(),
    protocolVersion: String(taskEnvelope.protocolVersion || '').trim(),
    triggerSource: String(taskEnvelope.triggerSource || 'workbench_dispatch').trim() || 'workbench_dispatch',
    monitorMeta: monitorMeta || undefined,
  };

  return {
    dispatchTarget: taskConfig.dispatchTarget || WORKBENCH_DISPATCH_TARGET.CONTENT,
    action: taskConfig.startAction,
    payload: {
      ...internalPayload,
      externalTaskMeta,
      tabId,
    },
    taskMeta: {
      externalTaskId: String(taskEnvelope.taskId || '').trim(),
      externalTaskType: String(taskEnvelope.taskType || '').trim(),
      protocolVersion: String(taskEnvelope.protocolVersion || '').trim(),
      triggerSource: String(taskEnvelope.triggerSource || 'workbench_dispatch').trim() || 'workbench_dispatch',
      targetUrl: String(taskEnvelope.target?.url || '').trim(),
      targetPageType: String(taskEnvelope.target?.pageType || '').trim(),
      idempotencyKey: String(taskEnvelope.idempotencyKey || '').trim(),
      monitorMeta: monitorMeta || undefined,
    },
  };
}

export function mapTaskEnvelopeToCapabilityCheck(taskEnvelope = {}) {
  const validation = validateTaskEnvelope(taskEnvelope);
  if (!validation.valid) {
    const error = new Error('Invalid task envelope');
    error.validation = validation;
    throw error;
  }

  return {
    type: WORKBENCH_MESSAGE_TYPE.CAPABILITY_CHECK,
    protocolVersion: String(taskEnvelope.protocolVersion || '').trim(),
    taskType: String(taskEnvelope.taskType || '').trim(),
    platform: String(taskEnvelope.platform || '').trim(),
    target: {
      ...(taskEnvelope.target || {}),
    },
  };
}

export const __taskEnvelopeMapperTesting = {
  isBatchNotesTaskType,
  isBatchCommentsTaskType,
  isCollectAuthorTaskType,
};
