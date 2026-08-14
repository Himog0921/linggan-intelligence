import { COMMENT_DEPTH_MODE } from '../../shared/constants.js';
import {
  MONITOR_RECORD_MODE,
  MONITOR_TASK_STRATEGY,
  WORKBENCH_RECORD_TYPE,
} from '../../workbench/protocol/schema.js';
import {
  buildDouyinSurfaceNoteRecords,
  buildXhsSurfaceNoteRecords,
} from '../../workbench/runtime/monitorTask.js';
import { resolveCollectionProfile } from '../../workbench/runtime/collectionProfile.js';
import { extractProfileIdentityFromUrl } from '../../shared/targetIdentity.js';
import { buildDouyinSingleCommentRunPatch } from '../../platforms/douyin/commentTaskSupport.js';
import { checkXhsAuthorMonitorTarget } from '../../platforms/xhs/batchController.js';
import { checkDouyinAuthorMonitorTarget } from '../../platforms/douyin/batchController.js';

const buildAuthorBaselineShortfallNote = ({
  requestedCount = 0,
  discoveredCount = 0,
  succeededCount = 0,
} = {}) => {
  const requested = Math.max(0, Number(requestedCount || 0) || 0);
  const discovered = Math.max(0, Number(discoveredCount || 0) || 0);
  const succeeded = Math.max(0, Number(succeededCount || 0) || 0);
  if (requested <= 0 || discovered >= requested) return '';
  if (succeeded > 0 && succeeded !== discovered) {
    return `这轮原计划建档 ${requested} 条，但当前主页最终只发现 ${discovered} 条可采作品，实际写入 ${succeeded} 条，所以先按现有作品完成建档。`;
  }
  return `这轮原计划建档 ${requested} 条，但当前主页最终只发现 ${discovered} 条可采作品，所以先按现有作品完成建档。`;
};

const getMonitorMetaFromMessage = (msg = {}) => msg.monitorMeta || msg.externalTaskMeta?.monitorMeta || null;

const FINAL_RESULT_PACKAGE_ONLY_PROFILES = new Set([
  'author_links',
  'author_profile',
  'comment_probe',
  'list_scan',
  'note_detail',
  'note_full',
]);

function shouldReportStreamingWorkbenchRecords(msg = {}) {
  const externalTaskMeta = msg.externalTaskMeta || {};
  const profile = resolveCollectionProfile({
    collectionProfile: msg.collectionProfile || externalTaskMeta.collectionProfile || '',
    taskType: msg.taskType || externalTaskMeta.externalTaskType || '',
    payload: msg.payload,
  });
  return !FINAL_RESULT_PACKAGE_ONLY_PROFILES.has(profile);
}

function ensurePositiveInteger(value, fallback = 0) {
  const num = Number(value);
  if (!Number.isFinite(num) || num <= 0) return fallback;
  return Math.floor(num);
}

function xhsCaptureProducer({
  reason = '',
  requested = 0,
  emitted = 0,
  failed = 0,
  failureReason = '',
  slotReports = null,
} = {}) {
  const safeEmitted = Math.max(0, Number(emitted || 0) || 0);
  return {
    reason: String(reason || '').trim(),
    failureReason: String(failureReason || '').trim(),
    slotReports,
    counters: {
      requested: Math.max(0, Number(requested || 0) || 0),
      discovered: safeEmitted,
      emitted: safeEmitted,
      deduplicated: 0,
      failed: Math.max(0, Number(failed || 0) || 0),
    },
  };
}

function firstRecordId(...values) {
  for (const value of values) {
    const text = String(value || '').trim();
    if (text) return text;
  }
  return '';
}

function normalizeCollectedAt(value) {
  if (!value) return '';
  if (typeof value === 'string') return value;
  const timestamp = Number(value);
  if (!Number.isFinite(timestamp)) return '';
  return new Date(timestamp).toISOString();
}

function emitWorkbenchRecordDelta(reportWorkbenchRecord, {
  recordType = '',
  record = {},
  collectionRunId = '',
  externalTaskId = '',
  sequence = Date.now(),
} = {}) {
  if (typeof reportWorkbenchRecord !== 'function') return;
  const payload = record && typeof record === 'object' && !Array.isArray(record) ? record : {};
  if (Object.keys(payload).length === 0) return;
  const externalRecordId = recordType === WORKBENCH_RECORD_TYPE.AUTHOR
    ? firstRecordId(payload.platformAuthorId, payload.authorId, payload.userId, payload.profileUrl)
    : firstRecordId(payload.noteId, payload.platformContentId, payload.contentId, payload.url);
  try {
    reportWorkbenchRecord({
      recordType,
      externalRecordId,
      record: payload,
      collectionRunId,
      externalTaskId,
      sequence,
      collectedAt: normalizeCollectedAt(payload.collectedAt),
    });
  } catch (error) {
    console.warn('[灵感爆爆爆] 工作台记录增量上报失败:', error);
  }
}

const inferSingleNoteFailureReasonCode = (message = '') => {
  if (/未稳定就绪|未找到笔记数据|数据结构异常|加载/i.test(message)) {
    return 'page_data_not_ready';
  }
  if (/目标|expected|actual|不一致|mismatch/i.test(message)) {
    return 'target_mismatch';
  }
  if (/登录|授权|风控|安全验证|访问受限|300017/i.test(message)) {
    return 'account_or_platform_blocked';
  }
  return 'single_note_collection_failed';
};

const buildSingleNoteFailureDiagnostic = ({
  error,
  expectedNoteId = '',
  monitorMeta = null,
} = {}) => {
  const technicalMessage = String(error?.message || error || '单篇笔记采集失败').trim();
  const reasonCode = inferSingleNoteFailureReasonCode(technicalMessage);
  const pageUrl = String(globalThis.window?.location?.href || '').trim();
  const userMessage = reasonCode === 'page_data_not_ready'
    ? '目标笔记页面没有加载出可采数据'
    : reasonCode === 'target_mismatch'
      ? '打开的页面和目标笔记不一致'
      : reasonCode === 'account_or_platform_blocked'
        ? '当前账号或平台访问状态不适合继续采集'
        : '单篇笔记采集失败';
  return {
    stage: 'collecting',
    failureCategory: reasonCode === 'account_or_platform_blocked' ? 'waiting_resource' : 'retry_wait',
    reasonCode,
    userMessage,
    technicalMessage,
    recommendedAction: reasonCode === 'account_or_platform_blocked'
      ? '检查账号状态，恢复后再继续采集'
      : '稍后自动重试，或改用作者页重新定位该笔记',
    evidence: {
      expectedNoteId: String(expectedNoteId || '').trim(),
      pageUrl,
      monitorId: String(monitorMeta?.monitorId || '').trim(),
      taskStrategy: String(monitorMeta?.taskStrategy || '').trim(),
    },
  };
};

const resolveXhsSurfaceContainerSelector = () => {
  const pathname = String(globalThis.window?.location?.pathname || '').trim();
  return /\/user\/profile\//i.test(pathname) ? '#userPostedFeeds' : '.feeds-container';
};

function extractXsecTokenFromUrl(url = '') {
  try {
    return String(new URL(String(url || ''), globalThis.window?.location?.origin || 'https://www.xiaohongshu.com').searchParams.get('xsec_token') || '').trim();
  } catch {
    return '';
  }
}

function resolveCurrentPageUrl() {
  return String(globalThis.window?.location?.href || '').trim();
}

function resolveAuthorPlatformIdFromMessage(msg = {}, pageUrl = '') {
  return String(
    msg.authorPlatformId
    || msg.platformAuthorId
    || msg.authorId
    || extractProfileIdentityFromUrl(msg.profileUrl || pageUrl, {
      baseUrl: globalThis.window?.location?.origin || 'https://www.xiaohongshu.com',
    })
    || '',
  ).trim();
}

function assertAuthorNoteLinkTarget(msg = {}, pageUrl = '') {
  const targetAuthorId = resolveAuthorPlatformIdFromMessage(msg, msg.profileUrl || pageUrl);
  const currentAuthorId = extractProfileIdentityFromUrl(pageUrl, {
    baseUrl: globalThis.window?.location?.origin || 'https://www.xiaohongshu.com',
  });
  if (!targetAuthorId || !currentAuthorId || targetAuthorId === currentAuthorId) return;

  const error = new Error('当前小红书主页和深度建档任务目标不一致');
  error.code = 'target_mismatch';
  error.reasonCode = 'target_mismatch';
  error.eventType = 'target_mismatch';
  error.userMessage = error.message;
  error.targetAuthorId = targetAuthorId;
  error.currentAuthorId = currentAuthorId;
  throw error;
}

function buildAuthorNoteLinkRecords(cards = [], {
  collectionRunId = '',
  limit = 0,
  sourcePageUrl = '',
  authorPlatformId = '',
  authorName = '',
  authorArchiveJobId = '',
  authorArchiveStage = '',
} = {}) {
  return buildXhsSurfaceNoteRecords(cards, {
    collectionRunId,
    mode: MONITOR_RECORD_MODE.AUTHOR_SURFACE,
    limit,
    sourcePageUrl,
  }).map((record, index) => {
    const sourceUrl = String(record.url || record.canonicalUrl || '').trim();
    const xsecToken = extractXsecTokenFromUrl(sourceUrl);
    const signedUrl = xsecToken ? sourceUrl : '';
    const identityText = String(record.title || record.content || record.bodyText || record.platformContentId || record.noteId || sourceUrl).trim();
    return {
      ...record,
      title: String(record.title || identityText).trim(),
      content: String(record.content || identityText).trim(),
      bodyText: String(record.bodyText || identityText).trim(),
      sourceUrl,
      rawUrl: sourceUrl,
      signedUrl: signedUrl || undefined,
      xsecToken: xsecToken || undefined,
      authorId: String(record.authorId || authorPlatformId).trim(),
      authorPlatformId: String(record.authorPlatformId || authorPlatformId).trim(),
      platformAuthorId: String(record.platformAuthorId || authorPlatformId).trim(),
      authorName: String(record.authorName || authorName).trim(),
      profileUrl: String(record.profileUrl || sourcePageUrl).trim(),
      batchRank: index + 1,
      rank: index + 1,
      dataSource: 'author_note_link_discovery',
      qualityReason: 'author_note_link_discovery',
      authorArchiveJobId: String(authorArchiveJobId || '').trim() || undefined,
      authorArchiveStage: String(authorArchiveStage || '').trim() || undefined,
    };
  });
}

const buildAuthorNoteLinksShortfallNote = ({
  requestedCount = 0,
  discoveredCount = 0,
  discoverySummary = null,
} = {}) => {
  const requested = Math.max(0, Number(requestedCount || 0) || 0);
  const discovered = Math.max(0, Number(discoveredCount || 0) || 0);
  if (requested <= 0 || discovered >= requested) return '';
  const stopReason = String(discoverySummary?.stopReason || '').trim();
  if (stopReason === 'max_rounds_reached') {
    return `这轮原计划发现 ${requested} 条博主历史笔记链接，已到安全滚动上限，当前先发现 ${discovered} 条可采作品；页面后面可能还有内容，建议后续继续补跑。`;
  }
  if (stopReason === 'bottom_confirmed') {
    return `这轮原计划发现 ${requested} 条博主历史笔记链接，当前主页多轮确认到底后只发现 ${discovered} 条可采作品，所以先按现有链接进入后续补采。`;
  }
  if (stopReason === 'stable_no_new') {
    return `这轮原计划发现 ${requested} 条博主历史笔记链接，连续滚动没有新增后只发现 ${discovered} 条可采作品，所以先按现有链接进入后续补采。`;
  }
  return `这轮原计划发现 ${requested} 条博主历史笔记链接，但当前主页最终只发现 ${discovered} 条可采作品，所以先按现有链接进入后续补采。`;
};

function readDiscoveryMeta(cards = []) {
  return cards && typeof cards === 'object' && cards.discoveryMeta && typeof cards.discoveryMeta === 'object'
    ? cards.discoveryMeta
    : null;
}

function normalizeDiscoverySummary(meta = null, {
  requestedCount = 0,
  discoveredCount = 0,
} = {}) {
  const source = meta && typeof meta === 'object' && !Array.isArray(meta) ? meta : {};
  const fieldQuality = source.fieldQuality && typeof source.fieldQuality === 'object' && !Array.isArray(source.fieldQuality)
    ? source.fieldQuality
    : {};
  return {
    method: String(source.method || '').trim(),
    stopReason: String(source.stopReason || '').trim(),
    requestedCount: Math.max(0, Number(requestedCount || 0) || 0),
    discoveredCount: Math.max(0, Number(discoveredCount || 0) || 0),
    totalNotes: Math.max(0, Number(source.totalNotes || discoveredCount || 0) || 0),
    preferredCount: Math.max(0, Number(source.preferredCount || 0) || 0),
    scrollCount: Math.max(0, Number(source.scrollCount || 0) || 0),
    rounds: Math.max(0, Number(source.rounds || 0) || 0),
    maxRounds: Math.max(0, Number(source.maxRounds || 0) || 0),
    canLoadMore: source.canLoadMore === undefined ? undefined : Boolean(source.canLoadMore),
    isFinished: source.isFinished === undefined ? undefined : Boolean(source.isFinished),
    fieldQuality,
  };
}

function assertAuthorMonitorTarget({ isDouyinPage, monitorMeta = null } = {}) {
  if (!monitorMeta) return;
  const check = isDouyinPage()
    ? checkDouyinAuthorMonitorTarget(monitorMeta)
    : checkXhsAuthorMonitorTarget(monitorMeta, { mode: 'profile' });
  if (check?.ok || check?.code !== 'target_mismatch') return;
  const error = new Error(String(check.message || '当前页博主身份与任务目标不一致'));
  error.code = 'target_mismatch';
  error.reasonCode = 'target_mismatch';
  error.eventType = 'target_mismatch';
  error.userMessage = error.message;
  error.targetAuthorId = check.targetAuthorId || '';
  error.currentAuthorId = check.currentAuthorId || '';
  throw error;
}

export function createCollectionHandlers({
  MSG,
  isDouyinPage,
  ensurePluginAuthorized,
  collectNote,
  collectComments,
  collectAuthor,
  collectDouyinVideo,
  collectDouyinComments,
  collectDouyinAuthor,
  BatchNoteController,
  noteStore,
  reportDone,
  reportProgress,
  reportWorkbenchRecord,
  reportTaskError,
  batchMessageHandlers,
  extractNoteId,
  createRemoteRun,
  finalizeRemoteRun,
  collectionRunStore,
  remoteControlRegistry,
  discoverXhsSurfaceNotes,
  discoverDouyinSurfaceTargets,
} = {}) {
  return {
    [MSG.COLLECT_SINGLE_NOTE]: async (msg = {}) => {
      await ensurePluginAuthorized();
      const triggerSource = String(msg.triggerSource || 'popup_manual').trim() || 'popup_manual';
      const monitorMeta = getMonitorMetaFromMessage(msg);
      const expectedNoteId = String(
        msg.expectedNoteId
        || monitorMeta?.targetNoteId
        || '',
      ).trim().replace(/^xhs_/, '');
      const currentWindow = globalThis.window;

      if (isDouyinPage()) {
        const result = await collectDouyinVideo();
        if (!result?.ok) {
          throw new Error(result?.error || '抖音视频采集失败');
        }
        reportDone('note', 1, { platform: 'douyin' });
        return { success: true, note: result.data };
      }

      const createRemoteNoteRun = () => createRemoteRun({
        platform: 'xhs',
        triggerSource,
        remoteTaskMeta: msg.externalTaskMeta,
        taskType: 'singleNote',
        config: {
          expectedNoteId: expectedNoteId || undefined,
          monitorMeta: monitorMeta || undefined,
        },
      });

      const runNoteCollection = async (remoteRun = null) => {
        try {
          const note = await collectNote(currentWindow, {
            collectionRunId: remoteRun?.collectionRunId || '',
            expectedNoteId,
            monitorMeta,
          });
          await finalizeRemoteRun(remoteRun, 'done', {
            itemsPlanned: 1,
            itemsSucceeded: 1,
            itemsFailed: 0,
            targetIds: [String(note?.noteId || note?.platformContentId || expectedNoteId || '').trim()].filter(Boolean),
            contentIds: [String(note?.contentId || '').trim()].filter(Boolean),
            captureProducer: xhsCaptureProducer({
              reason: 'target_reached',
              requested: 1,
              emitted: 1,
              slotReports: [
                { slotId: 'note', status: 'observed', reason: null },
                { slotId: 'comments', status: 'not_applicable', reason: 'not_requested_by_plan' },
              ],
            }),
          });
          reportDone('note', 1);
          return {
            success: true,
            note,
            collectionRunId: remoteRun?.collectionRunId || '',
          };
        } catch (error) {
          const diagnostic = buildSingleNoteFailureDiagnostic({
            error,
            expectedNoteId,
            monitorMeta,
          });
          await finalizeRemoteRun(remoteRun, 'failed', {
            error: diagnostic.technicalMessage,
            errorMessage: diagnostic.technicalMessage,
            userMessage: diagnostic.userMessage,
            diagnostic,
            itemsPlanned: 1,
            itemsSucceeded: 0,
            itemsFailed: 1,
            targetIds: [expectedNoteId].filter(Boolean),
            captureProducer: xhsCaptureProducer({
              requested: 1,
              failed: 1,
              failureReason: diagnostic.reasonCode,
            }),
          });
          throw error;
        }
      };

      const isRemoteDispatch = Boolean(String(msg.externalTaskMeta?.externalTaskId || '').trim());
      if (!isRemoteDispatch) {
        const note = await collectNote(currentWindow, {
          expectedNoteId,
          monitorMeta,
        });
        reportDone('note', 1);
        return { success: true, note };
      }

      const remoteRun = await createRemoteNoteRun();
      if (msg.asyncDispatch) {
        Promise.resolve()
          .then(() => runNoteCollection(remoteRun))
          .then(() => {
            reportProgress?.(1, 1, '完成', { taskState: 'done', collectionRunId: remoteRun?.collectionRunId });
          })
          .catch((error) => {
            console.error('[灵感爆爆爆] 远程单篇笔记采集失败:', error);
            reportTaskError?.(error, { collectionRunId: remoteRun?.collectionRunId, phase: 'collection' });
            if (remoteRun?.collectionRunId && collectionRunStore?.updateStatus) {
              collectionRunStore.updateStatus(remoteRun.collectionRunId, 'failed', {
                errorMessage: String(error?.message || error),
              }).catch(() => {});
            }
          });
        return {
          success: true,
          accepted: true,
          pending: true,
          collectionRunId: remoteRun?.collectionRunId || '',
        };
      }

      return runNoteCollection(remoteRun);
    },

    [MSG.COLLECT_SINGLE_COMMENT]: async (msg = {}) => {
      await ensurePluginAuthorized();
      const triggerSource = String(msg.triggerSource || 'popup_manual').trim() || 'popup_manual';
      const commentDepthMode = String(msg.commentDepthMode || COMMENT_DEPTH_MODE.TWO_LEVEL) === COMMENT_DEPTH_MODE.ALL_REPLIES
        ? COMMENT_DEPTH_MODE.ALL_REPLIES
        : COMMENT_DEPTH_MODE.TWO_LEVEL;
      if (isDouyinPage()) {
        const maxTotal = Math.max(0, Number(msg.maxTotal || 0) || 0);
        const maxSubComments = commentDepthMode === COMMENT_DEPTH_MODE.ALL_REPLIES ? 0 : Math.max(0, Number(msg.maxSubComments || 0) || 0);
        const sortMode = String(msg.sortMode || 'hot').trim() || 'hot';
        const createRemoteCommentRun = () => createRemoteRun({
          platform: 'douyin',
          triggerSource,
          remoteTaskMeta: msg.externalTaskMeta,
          taskType: 'singleComments',
          config: {
            maxTotal,
            maxSubComments,
            commentDepthMode,
            sortMode,
          },
        });
        const runCommentCollection = async (remoteRun = null) => {
          const controlBinding = remoteControlRegistry.bindRemoteControl({
            remoteRun,
            remoteTaskMeta: msg.externalTaskMeta,
          });
          try {
            const result = await collectDouyinComments({
              maxTotal,
              maxSubComments,
              sortMode,
              triggerSource,
              commentDepthMode,
              collectionRunId: remoteRun?.collectionRunId || '',
              manageCollectionRun: !remoteRun?.collectionRunId,
              shouldStop: controlBinding.control.shouldStop,
              waitIfPaused: controlBinding.control.waitIfPaused,
            });
            await finalizeRemoteRun(
              remoteRun,
              result?.stopped ? 'stopped' : 'done',
              buildDouyinSingleCommentRunPatch({
                stopped: Boolean(result?.stopped),
                totalComments: result.total || 0,
                note: result?.note || {},
              }),
            );
            reportDone('comment', result.total, { platform: 'douyin' });
            return {
              success: true,
              total: result.total,
              comments: result.comments,
              collectionRunId: remoteRun?.collectionRunId || result.collectionRunId || '',
            };
          } catch (error) {
            await finalizeRemoteRun(remoteRun, 'failed', {
              error: String(error?.message || error),
              itemsPlanned: 1,
              itemsSucceeded: 0,
              itemsFailed: 1,
            });
            throw error;
          } finally {
            controlBinding.release();
          }
        };

        if (msg.asyncDispatch) {
          const remoteRun = await createRemoteCommentRun();
          Promise.resolve()
            .then(() => runCommentCollection(remoteRun))
            .catch((error) => {
              console.error('[灵感爆爆爆] 远程抖音单条评论采集失败:', error);
            });
          return {
            success: true,
            accepted: true,
            pending: true,
            collectionRunId: remoteRun?.collectionRunId || '',
          };
        }

        const remoteRun = await createRemoteCommentRun();
        return runCommentCollection(remoteRun);
      }
      const result = await collectComments({
        noteId: extractNoteId(window.location.href),
        maxTotal: Math.max(0, Number(msg.maxTotal || 0) || 0),
        maxSubComments: commentDepthMode === COMMENT_DEPTH_MODE.ALL_REPLIES ? 0 : Math.max(0, Number(msg.maxSubComments || 0) || 0),
        commentDepthMode,
      });
      reportDone('comment', result.total);
      return { success: true, total: result.total };
    },

    [MSG.DISCOVER_AUTHOR_NOTE_LINKS]: async (msg = {}) => {
      await ensurePluginAuthorized();
      if (isDouyinPage()) {
        throw new Error('深度博主历史笔记发现当前只支持小红书主页');
      }

      const triggerSource = String(msg.triggerSource || 'workbench_dispatch').trim() || 'workbench_dispatch';
      const limit = ensurePositiveInteger(msg.maxLinks ?? msg.limit ?? msg.count, 200);
      const maxScrolls = ensurePositiveInteger(msg.maxScrolls, 30);
      const currentPageUrl = resolveCurrentPageUrl();
      const profileUrl = String(msg.profileUrl || currentPageUrl).trim();
      const authorPlatformId = resolveAuthorPlatformIdFromMessage(msg, profileUrl);
      const authorName = String(msg.authorName || '').trim();
      const authorArchiveJobId = String(msg.authorArchiveJobId || '').trim();
      const authorArchiveStage = String(msg.authorArchiveStage || '').trim();

      const createRemoteAuthorNoteLinkRun = () => createRemoteRun({
        platform: 'xhs',
        triggerSource,
        remoteTaskMeta: msg.externalTaskMeta,
        taskType: 'authorNoteLinks',
        config: {
          requestedCount: limit,
          maxScrolls,
          profileUrl,
          authorPlatformId,
          authorName,
          authorArchiveJobId: authorArchiveJobId || undefined,
          authorArchiveStage: authorArchiveStage || undefined,
        },
      });

      const runAuthorNoteLinkDiscovery = async (remoteRun = null) => {
        const controlBinding = remoteRun && remoteControlRegistry?.bindRemoteControl
          ? remoteControlRegistry.bindRemoteControl({
            remoteRun,
            remoteTaskMeta: msg.externalTaskMeta,
          })
          : null;
        const shouldStop = typeof controlBinding?.control?.shouldStop === 'function'
          ? controlBinding.control.shouldStop
          : () => false;
        const waitIfPaused = typeof controlBinding?.control?.waitIfPaused === 'function'
          ? controlBinding.control.waitIfPaused
          : async () => {};

        try {
          if (typeof discoverXhsSurfaceNotes !== 'function') {
            throw new Error('当前插件缺少博主主页笔记发现能力');
          }
          assertAuthorNoteLinkTarget({ ...msg, authorPlatformId, profileUrl }, currentPageUrl);
          await waitIfPaused();
          if (shouldStop()) {
            await finalizeRemoteRun(remoteRun, 'stopped', {
              itemsPlanned: 0,
              itemsSucceeded: 0,
              itemsFailed: 0,
              requestedCount: limit,
              discoveredCount: 0,
              shortfallCount: limit,
              captureProducer: xhsCaptureProducer({ requested: limit }),
            });
            return {
              success: true,
              stopped: true,
              total: 0,
              collectionRunId: remoteRun?.collectionRunId || '',
            };
          }

          const cards = await discoverXhsSurfaceNotes(resolveXhsSurfaceContainerSelector(), maxScrolls, {
            expectedCount: limit,
          });
          const discoverySummary = normalizeDiscoverySummary(readDiscoveryMeta(cards), {
            requestedCount: limit,
            discoveredCount: Array.isArray(cards) ? cards.length : 0,
          });
          const records = buildAuthorNoteLinkRecords(cards, {
            collectionRunId: remoteRun?.collectionRunId || '',
            limit,
            sourcePageUrl: profileUrl || currentPageUrl,
            authorPlatformId,
            authorName,
            authorArchiveJobId,
            authorArchiveStage,
          });
          if (records.length > 0) await noteStore.bulkUpsert(records);
          if (shouldReportStreamingWorkbenchRecords(msg)) {
            records.forEach((record, index) => {
              emitWorkbenchRecordDelta(reportWorkbenchRecord, {
                recordType: WORKBENCH_RECORD_TYPE.NOTE,
                record,
                collectionRunId: remoteRun?.collectionRunId || '',
                externalTaskId: msg.externalTaskMeta?.externalTaskId || '',
                sequence: Date.now() + index,
              });
            });
          }
          const targetIds = records
            .map((record) => String(record.platformContentId || record.noteId || '').trim())
            .filter(Boolean);
          const contentIds = records
            .map((record) => String(record.contentId || '').trim())
            .filter(Boolean);
          const stopped = shouldStop();
          await finalizeRemoteRun(remoteRun, stopped ? 'stopped' : 'done', {
            itemsPlanned: records.length,
            itemsSucceeded: records.length,
            itemsFailed: 0,
            targetIds,
            contentIds,
            requestedCount: limit,
            discoveredCount: records.length,
            shortfallCount: Math.max(0, limit - records.length),
            discoverySummary,
            completionNote: buildAuthorNoteLinksShortfallNote({
              requestedCount: limit,
              discoveredCount: records.length,
              discoverySummary,
            }) || undefined,
            captureProducer: xhsCaptureProducer({
              reason: discoverySummary.stopReason,
              requested: limit,
              emitted: records.length,
              slotReports: [{ slotId: 'note_links', status: 'observed', reason: null }],
            }),
          });
          if (!stopped) reportDone('note', records.length, { platform: 'xhs', taskType: 'authorNoteLinks' });
          return {
            success: true,
            stopped,
            total: records.length,
            records,
            collectionRunId: remoteRun?.collectionRunId || '',
          };
        } catch (error) {
          await finalizeRemoteRun(remoteRun, 'failed', {
            error: String(error?.message || error),
            itemsPlanned: limit,
            itemsSucceeded: 0,
            itemsFailed: limit,
            requestedCount: limit,
            discoveredCount: 0,
            shortfallCount: limit,
          });
          throw error;
        } finally {
          controlBinding?.release?.();
        }
      };

      if (msg.asyncDispatch) {
        const remoteRun = await createRemoteAuthorNoteLinkRun();
        Promise.resolve()
          .then(() => runAuthorNoteLinkDiscovery(remoteRun))
          .catch((error) => {
            console.error('[灵感爆爆爆] 远程小红书博主历史笔记链接发现失败:', error);
            reportTaskError?.(error, { collectionRunId: remoteRun?.collectionRunId, phase: 'author_note_link_discovery' });
          });
        return {
          success: true,
          accepted: true,
          pending: true,
          collectionRunId: remoteRun?.collectionRunId || '',
        };
      }

      const remoteRun = await createRemoteAuthorNoteLinkRun();
      return runAuthorNoteLinkDiscovery(remoteRun);
    },

    [MSG.COLLECT_AUTHOR]: async (msg = {}) => {
      await ensurePluginAuthorized();
      const triggerSource = String(msg.triggerSource || 'popup_manual').trim() || 'popup_manual';
      const monitorMeta = getMonitorMetaFromMessage(msg);
      const isMonitorSurface = Boolean(monitorMeta?.surfaceOnly);
      const isAuthorBaselineMonitor = !isDouyinPage()
        && String(monitorMeta?.taskStrategy || '').trim() === MONITOR_TASK_STRATEGY.AUTHOR_BASELINE;
      const scanLimit = Math.max(1, Number(msg.count || monitorMeta?.scanLimit || monitorMeta?.limit || 30) || 30);
      const baselineBatchCount = Math.max(1, Number(monitorMeta?.scanLimit || monitorMeta?.limit || 50) || 50);
      const collectAuthorSurfaceRecords = async ({
        platform,
        collectionRunId,
        shouldStop = () => false,
        waitIfPaused = async () => {},
      }) => {
        if (!isMonitorSurface || !collectionRunId || isAuthorBaselineMonitor) return [];
        await waitIfPaused();
        if (shouldStop()) return [];
        const pageUrl = String(globalThis.window?.location?.href || '').trim();
        if (platform === 'douyin' && typeof discoverDouyinSurfaceTargets === 'function') {
          const targets = await discoverDouyinSurfaceTargets({
            maxCount: scanLimit,
            topByLikes: false,
            shouldStop,
            waitIfPaused,
          });
          if (shouldStop()) return [];
          const records = buildDouyinSurfaceNoteRecords(targets, {
            monitorMeta,
            collectionRunId,
            mode: MONITOR_RECORD_MODE.AUTHOR_SURFACE,
            limit: scanLimit,
            searchKeyword: '',
            searchPageUrl: '',
          });
          if (records.length > 0) await noteStore.bulkUpsert(records);
          return records;
        }
        if (platform === 'xhs' && typeof discoverXhsSurfaceNotes === 'function') {
          const cards = await discoverXhsSurfaceNotes(resolveXhsSurfaceContainerSelector(), 10, {
            expectedCount: scanLimit,
          });
          if (shouldStop()) return [];
          const records = buildXhsSurfaceNoteRecords(cards, {
            monitorMeta,
            collectionRunId,
            mode: MONITOR_RECORD_MODE.AUTHOR_SURFACE,
            limit: scanLimit,
            sourcePageUrl: pageUrl,
          });
          if (records.length > 0) await noteStore.bulkUpsert(records);
          return records;
        }
        return [];
      };
      const createRemoteAuthorRun = () => createRemoteRun({
        platform: isDouyinPage() ? 'douyin' : 'xhs',
        triggerSource,
        remoteTaskMeta: msg.externalTaskMeta,
        taskType: 'collectAuthor',
        config: monitorMeta ? {
          surfaceOnly: isMonitorSurface && !isAuthorBaselineMonitor,
          scanLimit,
          baselineBatchCount: isAuthorBaselineMonitor ? baselineBatchCount : undefined,
          monitorMeta: monitorMeta || undefined,
        } : {},
      });
      const runXhsBaselineBatchCollection = async (remoteRun = null, control = {}) => {
        const shouldStop = typeof control.shouldStop === 'function' ? control.shouldStop : () => false;
        if (!isAuthorBaselineMonitor || !BatchNoteController) {
          return {
            planned: 0,
            succeeded: 0,
            failed: 0,
            targetIds: [],
            contentIds: [],
            stopped: false,
            completionNote: '',
            requestedCount: baselineBatchCount,
            discoveredCount: 0,
            shortfallCount: baselineBatchCount,
          };
        }
        if (shouldStop()) {
          return {
            planned: 0,
            succeeded: 0,
            failed: 0,
            targetIds: [],
            contentIds: [],
            stopped: true,
            completionNote: '',
            requestedCount: baselineBatchCount,
            discoveredCount: 0,
            shortfallCount: baselineBatchCount,
          };
        }
        const controller = new BatchNoteController();
        let stopWatcher = null;
        try {
          stopWatcher = setInterval(() => {
            if (shouldStop()) controller.stop?.();
          }, 250);
          await controller.start('profile', () => {}, {
            count: baselineBatchCount,
            topByLikes: false,
            triggerSource,
            collectionRunId: remoteRun?.collectionRunId || '',
            monitorMeta,
            surfaceOnly: false,
          });
        } finally {
          if (stopWatcher) clearInterval(stopWatcher);
        }
        const targetIds = (Array.isArray(controller.noteList) ? controller.noteList : [])
          .map((item) => String(item?.noteId || '').trim())
          .filter(Boolean);
        const contentIds = (Array.isArray(controller.collected) ? controller.collected : [])
          .map((item) => String(item?.contentId || '').trim())
          .filter(Boolean);
        return {
          planned: targetIds.length,
          succeeded: Array.isArray(controller.collected) ? controller.collected.length : 0,
          failed: Array.isArray(controller.failed) ? controller.failed.length : 0,
          targetIds,
          contentIds,
          stopped: Boolean(controller?._stoppedByUser || shouldStop()),
          completionNote: buildAuthorBaselineShortfallNote({
            requestedCount: baselineBatchCount,
            discoveredCount: targetIds.length,
            succeededCount: Array.isArray(controller.collected) ? controller.collected.length : 0,
          }),
          requestedCount: baselineBatchCount,
          discoveredCount: targetIds.length,
          shortfallCount: Math.max(0, baselineBatchCount - targetIds.length),
        };
      };
      const runAuthorCollection = async (remoteRun = null) => {
        const controlBinding = remoteRun && remoteControlRegistry?.bindRemoteControl
          ? remoteControlRegistry.bindRemoteControl({
            remoteRun,
            remoteTaskMeta: msg.externalTaskMeta,
          })
          : null;
        const shouldStop = typeof controlBinding?.control?.shouldStop === 'function'
          ? controlBinding.control.shouldStop
          : () => false;
        const waitIfPaused = typeof controlBinding?.control?.waitIfPaused === 'function'
          ? controlBinding.control.waitIfPaused
          : async () => {};

        try {
          if (isDouyinPage()) {
            let douyinAuthor = null;
            try {
              assertAuthorMonitorTarget({ isDouyinPage, monitorMeta });
              await waitIfPaused();
              if (shouldStop()) {
                await finalizeRemoteRun(remoteRun, 'stopped', {
                  itemsPlanned: 1,
                  itemsSucceeded: 0,
                  itemsFailed: 0,
                });
                return {
                  success: true,
                  stopped: true,
                  collectionRunId: remoteRun?.collectionRunId || '',
                };
              }
              const result = await collectDouyinAuthor({
                collectionRunId: remoteRun?.collectionRunId || '',
                triggerSource,
                externalTaskMeta: msg.externalTaskMeta || {},
                monitorMeta,
                shouldStop,
                waitIfPaused,
              });
              if (!result?.ok) {
                throw new Error(result?.error || '抖音博主采集失败');
              }
              douyinAuthor = result.data;
              emitWorkbenchRecordDelta(reportWorkbenchRecord, {
                recordType: WORKBENCH_RECORD_TYPE.AUTHOR,
                record: douyinAuthor,
                collectionRunId: remoteRun?.collectionRunId || '',
                externalTaskId: msg.externalTaskMeta?.externalTaskId || '',
              });
              await waitIfPaused();
              const surfaceRecords = shouldStop()
                ? []
                : await collectAuthorSurfaceRecords({
                  platform: 'douyin',
                  collectionRunId: remoteRun?.collectionRunId || '',
                  shouldStop,
                  waitIfPaused,
                });
              surfaceRecords.forEach((record, index) => {
                emitWorkbenchRecordDelta(reportWorkbenchRecord, {
                  recordType: WORKBENCH_RECORD_TYPE.NOTE,
                  record,
                  collectionRunId: remoteRun?.collectionRunId || '',
                  externalTaskId: msg.externalTaskMeta?.externalTaskId || '',
                  sequence: Date.now() + index + 1,
                });
              });
              const stopped = shouldStop();
              const surfaceTargetIds = surfaceRecords
                .map((record) => String(record.platformContentId || record.noteId || '').trim())
                .filter(Boolean);
              const surfaceContentIds = surfaceRecords
                .map((record) => String(record.contentId || '').trim())
                .filter(Boolean);
              const summaryPatch = {
                itemsPlanned: 1 + surfaceRecords.length,
                itemsSucceeded: 1 + surfaceRecords.length,
                itemsFailed: 0,
                targetIds: [
                  String(result?.data?.platformAuthorId || result?.data?.userId || '').trim(),
                  ...surfaceTargetIds,
                ].filter(Boolean),
                contentIds: surfaceContentIds,
              };
              if (isMonitorSurface) {
                summaryPatch.requestedCount = scanLimit;
                summaryPatch.discoveredCount = surfaceRecords.length;
                summaryPatch.shortfallCount = Math.max(0, scanLimit - surfaceRecords.length);
              }
              await finalizeRemoteRun(remoteRun, stopped ? 'stopped' : 'done', summaryPatch);
              if (!stopped) reportDone('author', 1, { platform: 'douyin' });
              return {
                success: true,
                author: result.data,
                stopped,
                collectionRunId: remoteRun?.collectionRunId || '',
              };
            } catch (error) {
              if (shouldStop()) {
                await finalizeRemoteRun(remoteRun, 'stopped', {
                  itemsPlanned: 1,
                  itemsSucceeded: douyinAuthor ? 1 : 0,
                  itemsFailed: 0,
                  targetIds: [String(douyinAuthor?.platformAuthorId || douyinAuthor?.userId || '').trim()].filter(Boolean),
                });
                return {
                  success: true,
                  author: douyinAuthor,
                  stopped: true,
                  collectionRunId: remoteRun?.collectionRunId || '',
                };
              }
              await finalizeRemoteRun(remoteRun, 'failed', {
                error: String(error?.message || error),
                itemsPlanned: 1,
                itemsSucceeded: 0,
                itemsFailed: 1,
              });
              throw error;
            }
          }

          let author = null;
          let batchResult = {
            planned: 0,
            succeeded: 0,
            failed: 0,
            targetIds: [],
            contentIds: [],
            stopped: false,
            completionNote: '',
            requestedCount: 0,
            discoveredCount: 0,
            shortfallCount: 0,
          };
          try {
            assertAuthorMonitorTarget({ isDouyinPage, monitorMeta });
            await waitIfPaused();
            if (shouldStop()) {
              await finalizeRemoteRun(remoteRun, 'stopped', {
                itemsPlanned: 1,
                itemsSucceeded: 0,
                itemsFailed: 0,
              });
              return {
                success: true,
                stopped: true,
                collectionRunId: remoteRun?.collectionRunId || '',
              };
            }
            author = await collectAuthor({
              collectionRunId: remoteRun?.collectionRunId || '',
              triggerSource,
              externalTaskMeta: msg.externalTaskMeta || {},
              monitorMeta,
              shouldStop,
              waitIfPaused,
            });
            batchResult = isAuthorBaselineMonitor
              ? await runXhsBaselineBatchCollection(remoteRun, { shouldStop, waitIfPaused })
              : { planned: 0, succeeded: 0, failed: 0, targetIds: [], contentIds: [], stopped: false };
            const surfaceRecords = shouldStop()
              ? []
              : await collectAuthorSurfaceRecords({
                platform: 'xhs',
                collectionRunId: remoteRun?.collectionRunId || '',
                shouldStop,
                waitIfPaused,
              });
            const authorTargetId = String(author?.platformAuthorId || author?.userId || '').trim();
            const stopped = Boolean(batchResult.stopped || shouldStop());
            const finalStatus = stopped ? 'stopped' : 'done';
            await finalizeRemoteRun(remoteRun, finalStatus, {
              itemsPlanned: 1 + batchResult.planned + surfaceRecords.length,
              itemsSucceeded: 1 + batchResult.succeeded + surfaceRecords.length,
              itemsFailed: batchResult.failed,
              targetIds: [authorTargetId, ...batchResult.targetIds].filter(Boolean),
              contentIds: batchResult.contentIds,
              completionNote: batchResult.completionNote || undefined,
              requestedCount: batchResult.requestedCount || undefined,
              discoveredCount: batchResult.discoveredCount || undefined,
              shortfallCount: batchResult.shortfallCount || undefined,
              captureProducer: xhsCaptureProducer({
                reason: 'target_reached',
                requested: 1 + batchResult.requestedCount,
                emitted: 1 + batchResult.succeeded + surfaceRecords.length,
                failed: batchResult.failed,
                slotReports: [
                  { slotId: 'author', status: 'observed', reason: null },
                  isAuthorBaselineMonitor || isMonitorSurface
                    ? { slotId: 'note_list', status: 'observed', reason: null }
                    : { slotId: 'note_list', status: 'not_applicable', reason: 'not_requested_by_plan' },
                ],
              }),
            });
            if (!stopped) reportDone('author', 1);
            return {
              success: true,
              author,
              stopped,
              collectionRunId: remoteRun?.collectionRunId || '',
            };
          } catch (error) {
            const authorTargetId = String(author?.platformAuthorId || author?.userId || '').trim();
            if (shouldStop()) {
              await finalizeRemoteRun(remoteRun, 'stopped', {
                itemsPlanned: 1 + batchResult.planned,
                itemsSucceeded: (author ? 1 : 0) + batchResult.succeeded,
                itemsFailed: 0,
                targetIds: [authorTargetId, ...batchResult.targetIds].filter(Boolean),
                contentIds: batchResult.contentIds,
              });
              return {
                success: true,
                author,
                stopped: true,
                collectionRunId: remoteRun?.collectionRunId || '',
              };
            }
            const plannedItems = isAuthorBaselineMonitor
              ? 1 + Math.max(batchResult.planned, baselineBatchCount)
              : 1;
            const succeededItems = author ? 1 + batchResult.succeeded : 0;
            const failedItems = Math.max(
              1,
              plannedItems - succeededItems,
              batchResult.failed,
            );
            await finalizeRemoteRun(remoteRun, 'failed', {
              error: String(error?.message || error),
              itemsPlanned: plannedItems,
              itemsSucceeded: succeededItems,
              itemsFailed: failedItems,
              targetIds: [authorTargetId, ...batchResult.targetIds].filter(Boolean),
              contentIds: batchResult.contentIds,
            });
            throw error;
          }
        } finally {
          controlBinding?.release?.();
        }
      };

      if (msg.asyncDispatch) {
        const remoteRun = await createRemoteAuthorRun();
        Promise.resolve()
          .then(() => runAuthorCollection(remoteRun))
          .catch((error) => {
            console.error('[灵感爆爆爆] 远程博主采集失败:', error);
          });
        return {
          success: true,
          accepted: true,
          pending: true,
          collectionRunId: remoteRun?.collectionRunId || '',
        };
      }

      const remoteRun = await createRemoteAuthorRun();
      return runAuthorCollection(remoteRun);
    },

    ...batchMessageHandlers,
  };
}
