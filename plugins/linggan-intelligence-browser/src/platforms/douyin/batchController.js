import { BATCH_CONFIG } from '../../shared/constants.js';
import { reportProgress } from '../../shared/messaging.js';
import { extractProfileIdentityFromUrl } from '../../shared/targetIdentity.js';
import { noteStore } from '../../db/noteStore.js';
import { collectionRunStore } from '../../db/collectionRunStore.js';
import { collectDouyinVideoByAweme, collectDouyinVideoById } from './videoCollector.js';
import { collectDouyinCommentsByVideoId } from './commentCollector.js';
import { detectDouyinPageType, detectDouyinSearchBatchContext, DY_PAGE_TYPE } from './pageDetector.js';
import {
  createDouyinBatchRun,
  createDouyinBatchPacer,
  discoverDouyinBatchTargets,
} from './batchDiscovery.js';
import { createCollectionRunHeartbeatReporter } from '../../workbench/runtime/heartbeat.js';
import { buildDouyinSurfaceNoteRecords } from '../../workbench/runtime/monitorTask.js';
import { pauseForDouyinSecurityChallenge } from './securityChallenge.js';
import { MONITOR_RECORD_MODE, MONITOR_TASK_STRATEGY } from '../../workbench/protocol/schema.js';
import {
  buildDouyinBatchCommentsProgressPatch,
  buildDouyinBatchCommentsRunPatch,
  buildDouyinBatchVideosProgressPatch,
  buildDouyinBatchVideosRunPatch,
} from '../../workbench/runtime/douyinBatchRunHelper.js';
import { resolveBatchResumeState } from '../../workbench/runtime/batchResume.js';
import { isTerminalCollectionRunStatus } from '../../db/collectionRunStatus.js';

const reportHeartbeat = createCollectionRunHeartbeatReporter({ collectionRunStore });

export function emitDouyinBatchProgress(onProgress, payload = {}) {
  if (typeof onProgress !== 'function') return;
  void Promise.resolve(onProgress(payload)).catch(() => {});
}

function normalizeTargetIdentity(value = '') {
  return String(value || '').trim().toLowerCase();
}

function extractDouyinProfileUserId(url = '') {
  return extractProfileIdentityFromUrl(url, { baseUrl: window.location.origin });
}

function getDouyinTargetProfileUrl(monitorMeta = {}) {
  const candidates = [
    monitorMeta?.targetUrl,
    monitorMeta?.targetPageUrl,
    monitorMeta?.pageUrl,
    monitorMeta?.sourcePageUrl,
  ];
  return candidates.find((value) => String(value || '').trim()) || '';
}

export function checkDouyinAuthorMonitorTarget(monitorMeta = {}, {
  page = detectDouyinPageType(),
  win = window,
} = {}) {
  const strategy = normalizeTargetIdentity(monitorMeta?.taskStrategy);
  const monitorMode = normalizeTargetIdentity(monitorMeta?.surfaceMode || monitorMeta?.monitorMode);
  const isAuthorMonitor = strategy === normalizeTargetIdentity(MONITOR_TASK_STRATEGY.AUTHOR_BASELINE)
    || monitorMode === normalizeTargetIdentity(MONITOR_RECORD_MODE.AUTHOR_SURFACE)
    || Boolean(monitorMeta?.surfaceOnly);

  if (!isAuthorMonitor || page.type !== DY_PAGE_TYPE.PROFILE) {
    return { ok: true, code: 'skipped' };
  }

  const currentUrl = String(win?.location?.href || window.location.href || '').trim();
  const targetUrl = getDouyinTargetProfileUrl(monitorMeta);
  const currentAuthorId = normalizeTargetIdentity(page.userId || extractDouyinProfileUserId(currentUrl));
  const targetAuthorId = normalizeTargetIdentity(extractDouyinProfileUserId(targetUrl));
  const targetLabel = String(
    monitorMeta?.display?.name
    || monitorMeta?.display?.nickname
    || monitorMeta?.display?.title
    || targetAuthorId
    || '',
  ).trim();

  if (!currentAuthorId || !targetAuthorId) {
    return {
      ok: false,
      code: 'target_mismatch',
      reasonCode: 'target_mismatch',
      eventType: 'target_mismatch',
      message: `当前页博主身份与任务目标无法对齐，已停止本轮采集：当前=${currentAuthorId || '未识别'}，目标=${targetLabel || targetAuthorId || '未识别'}`,
      currentAuthorId,
      targetAuthorId,
      currentUrl,
      targetUrl,
      targetLabel,
    };
  }

  if (currentAuthorId !== targetAuthorId) {
    return {
      ok: false,
      code: 'target_mismatch',
      reasonCode: 'target_mismatch',
      eventType: 'target_mismatch',
      message: `当前页博主身份与任务目标不一致，已停止本轮采集：当前=${currentAuthorId}，目标=${targetLabel || targetAuthorId}`,
      currentAuthorId,
      targetAuthorId,
      currentUrl,
      targetUrl,
      targetLabel,
    };
  }

  return {
    ok: true,
    code: 'ok',
    currentAuthorId,
    targetAuthorId,
    currentUrl,
    targetUrl,
    targetLabel,
  };
}

function createTargetMismatchError(check = {}) {
  const error = new Error(String(check.message || '当前页博主身份与任务目标不一致'));
  error.code = 'target_mismatch';
  error.reasonCode = 'target_mismatch';
  error.eventType = 'target_mismatch';
  error.userMessage = error.message;
  error.targetAuthorId = check.targetAuthorId || '';
  error.currentAuthorId = check.currentAuthorId || '';
  return error;
}

async function handleDouyinBatchSecurityChallenge(error, {
  onProgress = null,
  waitIfPaused = async () => {},
  shouldStop = () => false,
  taskType = 'batchComments',
  stage = 'collecting',
  current = 0,
  total = 0,
  message = '',
  collectionRunId = '',
} = {}) {
  return pauseForDouyinSecurityChallenge(error, {
    current,
    total,
    waitIfPaused,
    shouldStop,
    formatProgress: ({ current: currentValue, total: totalValue }) => {
      const normalizedCurrent = Number(currentValue || 0);
      const normalizedTotal = Number(totalValue || 0);
      if (normalizedTotal > 0) {
        return `当前已完成 ${normalizedCurrent}/${normalizedTotal} 条视频`;
      }
      if (normalizedCurrent > 0) {
        return `当前已完成 ${normalizedCurrent} 条视频`;
      }
      return '';
    },
    onPause: async (payload) => {
      onProgress?.({
        taskType,
        taskState: 'paused',
        stage,
        phase: stage,
        current,
        total,
        message: message || payload.message,
      });
      void reportHeartbeat.report(collectionRunId, {
        taskState: 'paused',
        stage,
        current,
        total,
        message: message || payload.message,
      }).catch(() => {});
    },
  });
}

async function createOrResumeDouyinBatchRun({
  taskType,
  pageType,
  triggerSource,
  externalTaskMeta = {},
  config = {},
  meta = {},
} = {}) {
  const externalTaskId = String(externalTaskMeta.externalTaskId || '').trim();
  if (externalTaskId) {
    const existing = await collectionRunStore.getLatestByExternalTaskId(externalTaskId).catch(() => null);
    if (
      existing?.collectionRunId
      && String(existing.taskType || '').trim() === String(taskType || '').trim()
      && !isTerminalCollectionRunStatus(existing.status)
    ) {
      return { run: existing, resumeRun: existing };
    }
  }
  const run = await createDouyinBatchRun({
    taskType,
    pageType,
    triggerSource,
    externalTaskMeta,
    config,
    meta,
  });
  return { run, resumeRun: null };
}

function normalizeDouyinTargetId(item = {}) {
  return String(
    item?.targetId
    || item?.awemeId
    || item?.platformContentId
    || item?.videoId
    || item?.noteId
    || item?.contentId
    || '',
  ).trim().replace(/^(dy_|douyin_)/i, '');
}

function hydrateDouyinResumeResults(runRecord = {}, targets = [], nextIndex = 0) {
  const safeRun = runRecord && typeof runRecord === 'object' && !Array.isArray(runRecord) ? runRecord : {};
  const targetIds = targets.map(normalizeDouyinTargetId);
  const completedIds = new Set(targetIds.slice(0, Math.max(0, Number(nextIndex || 0) || 0)));
  const statuses = Array.isArray(safeRun?.resumeCheckpoint?.resultStatuses)
    ? safeRun.resumeCheckpoint.resultStatuses
    : [];
  const statusById = new Map(statuses.map((item) => [normalizeDouyinTargetId(item), item]));
  return targetIds.slice(0, Math.max(0, Number(nextIndex || 0) || 0))
    .map((targetId) => {
      const status = statusById.get(targetId);
      if (status) {
        return {
          awemeId: targetId,
          ok: status.ok !== false,
          noteId: status.contentId || (status.ok !== false ? `dy_${targetId}` : ''),
          totalComments: Number(status.totalComments || 0) || 0,
          error: status.ok === false ? String(status.error || 'failed') : '',
        };
      }
      if ((Array.isArray(safeRun.contentIds) ? safeRun.contentIds : [])
        .map((value) => String(value || '').trim().replace(/^(dy_|douyin_)/i, ''))
        .includes(targetId)) {
        return { awemeId: targetId, ok: true, noteId: `dy_${targetId}`, totalComments: 0 };
      }
      const failed = (Array.isArray(safeRun.failedTargets) ? safeRun.failedTargets : [])
        .find((item) => normalizeDouyinTargetId(item) === targetId);
      if (failed) {
        return { awemeId: targetId, ok: false, totalComments: 0, error: String(failed.error || 'failed') };
      }
      if (completedIds.has(targetId)) {
        return { awemeId: targetId, ok: false, totalComments: 0, error: 'resume_state_missing_result' };
      }
      return null;
    })
    .filter(Boolean);
}

export async function collectDouyinBatchVideoTarget(target = {}, {
  isSearch = false,
  index = 0,
  topByLikes = false,
  selectionMode = '',
  triggerSource = '',
  collectionRunId = '',
  monitorMeta = null,
  searchPageUrl = '',
} = {}, {
  collectByAweme = collectDouyinVideoByAweme,
  collectById = collectDouyinVideoById,
} = {}) {
  const sharedOptions = {
    url: target.href,
    titleHint: target.titleHint,
    authorHint: target.authorHint,
    timeHint: target.timeHint,
    triggerSource,
    sourceUrl: target.sourceUrl,
    collectionRunId,
    monitorMeta,
    batchSelectionMode: selectionMode,
    batchRank: topByLikes ? Number(target.rank || 0) : index + 1,
    batchLikesSnapshot: Number(target.likes || 0),
    searchKeyword: String(target.searchKeyword || '').trim(),
    searchPageUrl: isSearch ? searchPageUrl : '',
    propagateSecurityChallenge: true,
  };

  if (target?.aweme) {
    return collectByAweme(target.aweme, sharedOptions);
  }

  return collectById(target.awemeId, sharedOptions);
}

export async function batchCollectDouyinProfileVideos({
  maxCount = Math.min(BATCH_CONFIG.maxPerSession, 20),
  topByLikes = false,
  externalTaskMeta = {},
  monitorMeta = null,
  surfaceOnly = false,
  onCollectionRun = null,
  onProgress = null,
  shouldStop = () => false,
  waitIfPaused = async () => {},
} = {}) {
  const page = detectDouyinPageType();
  const searchContext = detectDouyinSearchBatchContext();
  const isProfile = page.type === DY_PAGE_TYPE.PROFILE;
  const isSearch = searchContext.stableSearchList;
  if (!isProfile && !isSearch) {
    return { ok: false, error: '请在抖音博主主页，或带稳定搜索结果列表的抖音搜索页使用批量视频采集' };
  }
  const effectivePageType = isSearch ? DY_PAGE_TYPE.SEARCH : page.type;
  const effectiveSearchKeyword = isSearch ? searchContext.keyword : '';
  const safeMaxCount = Math.min(BATCH_CONFIG.maxPerSession, Math.max(1, Number(maxCount || 0) || 20));

  const targetCheck = checkDouyinAuthorMonitorTarget(monitorMeta, { page, searchContext });
  if (!targetCheck.ok && targetCheck.code === 'target_mismatch') {
    const error = createTargetMismatchError(targetCheck);
    onProgress?.({
      current: 0,
      total: safeMaxCount,
      message: error.message,
      taskState: 'error',
      stage: 'startup',
      phase: 'startup',
      errorCode: 'target_mismatch',
      reasonCode: 'target_mismatch',
    });
    reportProgress(0, safeMaxCount, error.message, {
      taskType: 'batchNotes',
      taskState: 'error',
      phase: 'startup',
      errorCode: 'target_mismatch',
      reasonCode: 'target_mismatch',
    });
    throw error;
  }
  const selectionMode = topByLikes
    ? (isSearch ? 'search_top_likes' : 'top_likes')
    : (isSearch ? 'search_order' : 'profile_order');
  const runConfig = {
    maxCount: safeMaxCount,
    topByLikes: Boolean(topByLikes),
  };
  if (monitorMeta) {
    runConfig.surfaceOnly = Boolean(surfaceOnly && monitorMeta);
    runConfig.monitorMeta = monitorMeta;
  }
  const { run, resumeRun } = await createOrResumeDouyinBatchRun({
    taskType: 'batchNotes',
    pageType: effectivePageType,
    triggerSource: topByLikes
      ? (isSearch ? 'batch_search_top_likes' : 'batch_profile_top_likes')
      : (isSearch ? 'batch_search' : 'batch_profile'),
    externalTaskMeta,
    config: runConfig,
    meta: {
      searchKeyword: effectiveSearchKeyword,
    },
  });
  onCollectionRun?.(run.collectionRunId);

  onProgress?.({
    current: 0,
    total: safeMaxCount,
    message: topByLikes
      ? (isSearch ? '正在获取搜索结果，并按点赞排序...' : '正在获取博主页作品列表，并按点赞排序...')
      : (isSearch ? '正在获取搜索结果作品列表...' : '正在获取博主页作品列表...'),
  });
  void reportHeartbeat.report(run.collectionRunId, {
    taskState: 'running',
    stage: 'discovering',
    current: 0,
    total: safeMaxCount,
    message: topByLikes
      ? (isSearch ? '正在获取搜索结果，并按点赞排序...' : '正在获取博主页作品列表，并按点赞排序...')
      : (isSearch ? '正在获取搜索结果作品列表...' : '正在获取博主页作品列表...'),
  }).catch(() => {});

  try {
    let targets = [];
    while (!shouldStop()) {
      try {
        targets = await discoverDouyinBatchTargets({
          maxCount: safeMaxCount,
          topByLikes: Boolean(topByLikes),
          shouldStop,
          waitIfPaused,
        });
        break;
      } catch (error) {
        const pauseResult = await handleDouyinBatchSecurityChallenge(error, {
          onProgress,
          waitIfPaused,
          shouldStop,
          taskType: 'batchNotes',
          stage: 'discovering',
          current: 0,
          total: safeMaxCount,
          collectionRunId: run.collectionRunId,
        });
        if (pauseResult.handled) {
          if (pauseResult.stopped) {
            await collectionRunStore.markStopped(run.collectionRunId, {
              itemsPlanned: 0,
              itemsSucceeded: 0,
              itemsFailed: 0,
            });
            return {
              ok: false,
              stopped: true,
              total: 0,
              success: 0,
              failed: 0,
              results: [],
              collectionRunId: run.collectionRunId,
            };
          }
          continue;
        }
        throw error;
      }
    }

    if (targets.length === 0) {
      const error = '当前主页没有发现可批量采集的视频作品';
      await collectionRunStore.markFailed(run.collectionRunId, error, {
        itemsPlanned: 0,
        itemsSucceeded: 0,
        itemsFailed: 0,
      });
      return { ok: false, error, collectionRunId: run.collectionRunId };
    }

    if (surfaceOnly && monitorMeta) {
      const records = buildDouyinSurfaceNoteRecords(targets, {
        monitorMeta,
        collectionRunId: run.collectionRunId,
        mode: monitorMeta.surfaceMode,
        limit: targets.length,
        searchKeyword: effectiveSearchKeyword || monitorMeta.keyword || '',
        searchPageUrl: isSearch ? window.location.href : '',
      });
      if (records.length > 0) {
        await noteStore.bulkUpsert(records);
      }
      const summary = {
        itemsPlanned: targets.length,
        itemsSucceeded: records.length,
        itemsFailed: 0,
        targetIds: targets.map((target) => target.awemeId).filter(Boolean),
        results: records.map((record) => ({
          ok: true,
          noteId: record.noteId,
          title: record.title,
          monitorMode: record.monitorMode,
        })),
      };
      await collectionRunStore.markDone(run.collectionRunId, summary);
      onProgress?.({
        current: records.length,
        total: targets.length,
        message: `表层扫描完成：发现 ${records.length} 条内容`,
      });
      return {
        ok: true,
        total: targets.length,
        success: records.length,
        failed: 0,
        results: summary.results,
        stopped: false,
        collectionRunId: run.collectionRunId,
      };
    }

    const resumeState = resolveBatchResumeState({
      runRecord: resumeRun,
      targets,
      getTargetId: (item) => item.awemeId,
    });
    targets = resumeState.targets;
    await collectionRunStore.updateById(run.collectionRunId, buildDouyinBatchVideosProgressPatch({
      targets,
      results: hydrateDouyinResumeResults(resumeRun, targets, resumeState.nextIndex),
      processedCount: resumeState.nextIndex,
    }));

    onProgress?.({
      current: resumeState.nextIndex,
      total: targets.length,
      message: resumeState.resumed
        ? `已从本地记录恢复，前 ${resumeState.nextIndex}/${targets.length} 条视频不重复采集`
        : (topByLikes
          ? `已发现作品并按点赞排序，开始采集前 ${targets.length} 条`
          : `已发现 ${targets.length} 条作品，开始采集`),
    });
    void reportHeartbeat.report(run.collectionRunId, {
      taskState: 'running',
      stage: 'discovering',
      current: resumeState.nextIndex,
      total: targets.length,
      message: topByLikes
        ? `已发现作品并按点赞排序，开始采集前 ${targets.length} 条`
        : `已发现 ${targets.length} 条作品，开始采集`,
    }).catch(() => {});

    const results = hydrateDouyinResumeResults(resumeRun, targets, resumeState.nextIndex);
    let success = results.filter((item) => item.ok).length;
    let failed = results.filter((item) => !item.ok).length;
    const videoPacer = createDouyinBatchPacer({
      baseRange: { min: 180, max: 280 },
    });

    for (let index = resumeState.nextIndex; index < targets.length; index += 1) {
      await waitIfPaused();
      if (shouldStop()) break;

      const target = targets[index];
      const targetLabel = String(target.titleHint || target.awemeId || `作品 ${index + 1}`).replace(/\s+/g, ' ').slice(0, 24);
      onProgress?.({
        current: index,
        total: targets.length,
        message: topByLikes
          ? `正在采集第 ${index + 1}/${targets.length} 条（Top ${target.rank}）· ${targetLabel}`
          : `正在采集第 ${index + 1}/${targets.length} 条 · ${targetLabel}`,
        target,
      });
      void reportHeartbeat.report(run.collectionRunId, {
        taskState: 'running',
        stage: 'collecting',
        current: index,
        total: targets.length,
        message: topByLikes
          ? `正在采集第 ${index + 1}/${targets.length} 条（Top ${target.rank}）· ${targetLabel}`
          : `正在采集第 ${index + 1}/${targets.length} 条 · ${targetLabel}`,
      }).catch(() => {});

      try {
        let collected = null;
        while (!shouldStop()) {
          try {
            collected = await collectDouyinBatchVideoTarget(target, {
              isSearch,
              index,
              topByLikes,
              selectionMode,
              triggerSource: topByLikes
                ? (isSearch ? 'batch_search_top_likes' : 'batch_profile_top_likes')
                : (isSearch ? 'batch_search' : 'batch_profile'),
              collectionRunId: run.collectionRunId,
              monitorMeta,
              searchPageUrl: window.location.href,
            });
            break;
          } catch (error) {
            const pauseResult = await handleDouyinBatchSecurityChallenge(error, {
              onProgress,
              waitIfPaused,
              shouldStop,
              taskType: 'batchNotes',
              stage: 'collecting',
              current: index,
              total: targets.length,
              collectionRunId: run.collectionRunId,
            });
            if (pauseResult.handled) {
              videoPacer.recordFailure();
              if (pauseResult.stopped) break;
              continue;
            }
            throw error;
          }
        }
        if (shouldStop()) break;

        if (!collected?.ok) {
          throw new Error(collected?.error || '采集失败');
        }

        videoPacer.recordSuccess();
        success += 1;
        results.push({
          ...target,
          ok: true,
          noteId: collected.data?.noteId || '',
          title: collected.data?.title || target.titleHint || '',
        });
      } catch (err) {
        videoPacer.recordFailure();
        failed += 1;
        results.push({
          ...target,
          ok: false,
          error: String(err?.message || err),
        });
      }

      onProgress?.({
        current: index + 1,
        total: targets.length,
        message: `已完成 ${index + 1}/${targets.length} 条${results[results.length - 1]?.ok ? '' : '，当前条失败'}`,
        target,
      });
      void reportHeartbeat.report(run.collectionRunId, {
        taskState: shouldStop() ? 'stopping' : 'running',
        stage: 'collecting',
        current: index + 1,
        total: targets.length,
        message: `已完成 ${index + 1}/${targets.length} 条${results[results.length - 1]?.ok ? '' : '，当前条失败'}`,
      }).catch(() => {});
      await collectionRunStore.updateById(run.collectionRunId, buildDouyinBatchVideosProgressPatch({
        targets,
        results,
        processedCount: index + 1,
      })).catch(() => {});

      await videoPacer.wait({ waitIfPaused, shouldStop });
    }

    const stopped = shouldStop();
    const firstError = results.find((item) => !item.ok && item.error)?.error || '';
    const summary = {
      ...buildDouyinBatchVideosRunPatch({ targets, results }),
      itemsSucceeded: success,
      itemsFailed: failed,
      results,
      error: firstError || undefined,
    };
    if (stopped) {
      await collectionRunStore.markStopped(run.collectionRunId, summary);
    } else if (success > 0) {
      await collectionRunStore.markDone(run.collectionRunId, summary);
    } else {
      await collectionRunStore.markFailed(run.collectionRunId, firstError || '批量视频采集全部失败', summary);
    }

    const finalLabel = stopped ? '已停止' : (success > 0 ? '已完成' : '失败');
    emitDouyinBatchProgress(onProgress, {
      taskState: stopped ? 'idle' : 'done',
      stage: 'done',
      current: targets.length,
      total: targets.length,
      message: `${finalLabel}：成功 ${success} 条，失败 ${failed} 条（共 ${targets.length} 条）`,
    });

    return {
      ok: success > 0,
      error: !success ? (firstError || '批量视频采集全部失败') : '',
      total: targets.length,
      success,
      failed,
      results,
      stopped,
      collectionRunId: run.collectionRunId,
    };
  } catch (err) {
    await collectionRunStore.markFailed(run.collectionRunId, err, {
      itemsPlanned: 0,
      itemsSucceeded: 0,
      itemsFailed: 0,
    });
    throw err;
  }
}

export async function batchCollectDouyinProfileComments({
  maxCount = 10,
  topByLikes = false,
  maxCommentsPerVideo = 0,
  maxSubComments = 0,
  externalTaskMeta = {},
  onCollectionRun = null,
  onProgress = null,
  shouldStop = () => false,
  waitIfPaused = async () => {},
} = {}) {
  const page = detectDouyinPageType();
  const searchContext = detectDouyinSearchBatchContext();
  const isProfile = page.type === DY_PAGE_TYPE.PROFILE;
  const isSearch = searchContext.stableSearchList;
  if (!isProfile && !isSearch) {
    return { ok: false, error: '请在抖音博主主页，或带稳定搜索结果列表的抖音搜索页使用批量评论采集' };
  }
  const effectivePageType = isSearch ? DY_PAGE_TYPE.SEARCH : page.type;
  const effectiveSearchKeyword = isSearch ? searchContext.keyword : '';

  const safeMaxCount = Math.min(20, Math.max(1, Number(maxCount || 0) || 10));
  const safeMaxCommentsPerVideo = Math.max(0, Number(maxCommentsPerVideo || 0) || 0);
  const selectionMode = topByLikes
    ? (isSearch ? 'search_top_likes' : 'top_likes')
    : (isSearch ? 'search_order' : 'profile_order');
  const { run, resumeRun } = await createOrResumeDouyinBatchRun({
    taskType: 'batchComments',
    pageType: effectivePageType,
    triggerSource: topByLikes
      ? (isSearch ? 'batch_search_comment_top_likes' : 'batch_profile_comment_top_likes')
      : (isSearch ? 'batch_search_comment' : 'batch_profile_comment'),
    externalTaskMeta,
    config: {
      maxCount: safeMaxCount,
      topByLikes: Boolean(topByLikes),
      maxCommentsPerVideo: safeMaxCommentsPerVideo,
      maxSubComments: Math.max(0, Number(maxSubComments || 0) || 0),
    },
    meta: {
      searchKeyword: effectiveSearchKeyword,
    },
  });
  onCollectionRun?.(run.collectionRunId);

  onProgress?.({
    current: 0,
    total: safeMaxCount,
    message: topByLikes
      ? (isSearch ? '正在获取搜索结果，并按点赞排序准备批量评论采集...' : '正在获取博主页作品列表，并按点赞排序准备批量评论采集...')
      : (isSearch ? '正在获取搜索结果，准备批量评论采集...' : '正在获取博主页作品列表，准备批量评论采集...'),
  });
  void reportHeartbeat.report(run.collectionRunId, {
    taskState: 'running',
    stage: 'discovering',
    current: 0,
    total: safeMaxCount,
    message: topByLikes
      ? (isSearch ? '正在获取搜索结果，并按点赞排序准备批量评论采集...' : '正在获取博主页作品列表，并按点赞排序准备批量评论采集...')
      : (isSearch ? '正在获取搜索结果，准备批量评论采集...' : '正在获取博主页作品列表，准备批量评论采集...'),
  }).catch(() => {});

  try {
    let targets = [];
    while (!shouldStop()) {
      try {
        targets = await discoverDouyinBatchTargets({
          maxCount: safeMaxCount,
          topByLikes: Boolean(topByLikes),
          shouldStop,
          waitIfPaused,
        });
        break;
      } catch (error) {
        const pauseResult = await handleDouyinBatchSecurityChallenge(error, {
          onProgress,
          waitIfPaused,
          shouldStop,
          taskType: 'batchComments',
          stage: 'discovering',
          current: 0,
          total: safeMaxCount,
          collectionRunId: run.collectionRunId,
        });
        if (pauseResult.handled) {
          if (pauseResult.stopped) {
            await collectionRunStore.markStopped(run.collectionRunId, {
              itemsPlanned: 0,
              itemsSucceeded: 0,
              itemsFailed: 0,
              totalComments: 0,
            });
            return {
              ok: false,
              stopped: true,
              totalVideos: 0,
              successVideos: 0,
              failedVideos: 0,
              totalComments: 0,
              results: [],
              collectionRunId: run.collectionRunId,
            };
          }
          continue;
        }
        throw error;
      }
    }

    if (targets.length === 0) {
      const error = '当前主页没有发现可批量采集评论的视频作品';
      await collectionRunStore.markFailed(run.collectionRunId, error, {
        itemsPlanned: 0,
        itemsSucceeded: 0,
        itemsFailed: 0,
        totalComments: 0,
      });
      return { ok: false, error, collectionRunId: run.collectionRunId };
    }

    const resumeState = resolveBatchResumeState({
      runRecord: resumeRun,
      targets,
      getTargetId: (item) => item.awemeId,
    });
    targets = resumeState.targets;

    const results = hydrateDouyinResumeResults(resumeRun, targets, resumeState.nextIndex);
    let success = results.filter((item) => item.ok).length;
    let failed = results.filter((item) => !item.ok).length;
    let totalComments = results.reduce((sum, item) => sum + Number(item.totalComments || 0), 0);
    const commentPacer = createDouyinBatchPacer({
      baseRange: { min: 140, max: 220 },
    });
    await collectionRunStore.updateById(run.collectionRunId, buildDouyinBatchCommentsProgressPatch({
      targets,
      results,
      totalComments,
      processedCount: resumeState.nextIndex,
    })).catch(() => {});

    if (resumeState.resumed) {
      onProgress?.({
        current: resumeState.nextIndex,
        total: targets.length,
        message: `已从本地记录恢复，前 ${resumeState.nextIndex}/${targets.length} 条视频评论不重复采集`,
      });
    }

    for (let index = resumeState.nextIndex; index < targets.length; index += 1) {
      await waitIfPaused();
      if (shouldStop()) break;

      const target = targets[index];
      const targetLabel = String(target.titleHint || target.awemeId || `作品 ${index + 1}`).replace(/\s+/g, ' ').slice(0, 24);
      onProgress?.({
        current: index,
        total: targets.length,
        message: topByLikes
          ? `正在采集第 ${index + 1}/${targets.length} 条评论（Top ${target.rank}）· ${targetLabel}`
          : `正在采集第 ${index + 1}/${targets.length} 条评论 · ${targetLabel}`,
        target,
      });
      void reportHeartbeat.report(run.collectionRunId, {
        taskState: 'running',
        stage: 'collecting',
        current: index,
        total: targets.length,
        message: topByLikes
          ? `正在采集第 ${index + 1}/${targets.length} 条评论（Top ${target.rank}）· ${targetLabel}`
          : `正在采集第 ${index + 1}/${targets.length} 条评论 · ${targetLabel}`,
      }).catch(() => {});

      try {
        let collected = null;
        while (!shouldStop()) {
          try {
            collected = await collectDouyinCommentsByVideoId(target.awemeId, {
              aweme: target.aweme,
              url: target.href,
              titleHint: target.titleHint,
              authorHint: target.authorHint,
              timeHint: target.timeHint,
              triggerSource: topByLikes
                ? (isSearch ? 'batch_search_comment_top_likes' : 'batch_profile_comment_top_likes')
                : (isSearch ? 'batch_search_comment' : 'batch_profile_comment'),
              collectionRunId: run.collectionRunId,
              maxTotal: safeMaxCommentsPerVideo,
              maxSubComments: Math.max(0, Number(maxSubComments || 0) || 0),
              batchSelectionMode: selectionMode,
              batchRank: topByLikes ? Number(target.rank || 0) : index + 1,
              batchLikesSnapshot: Number(target.likes || 0),
              searchKeyword: String(target.searchKeyword || '').trim(),
              searchPageUrl: isSearch ? window.location.href : '',
              shouldStop,
              waitIfPaused,
              onProgress: (payload) => {
                onProgress?.({
                  current: index,
                  total: targets.length,
                  taskState: payload.taskState || payload.status || 'running',
                  message: payload.message || `第 ${index + 1}/${targets.length} 条评论：已采集 ${payload.current || 0} 条`,
                  target,
                });
                void reportHeartbeat.report(run.collectionRunId, {
                  taskState: payload.taskState || payload.status || (shouldStop() ? 'stopping' : 'running'),
                  stage: 'collecting',
                  current: index,
                  total: targets.length,
                  message: payload.message || `第 ${index + 1}/${targets.length} 条评论：已采集 ${payload.current || 0} 条`,
                }).catch(() => {});
              },
            });
            break;
          } catch (error) {
            const pauseResult = await handleDouyinBatchSecurityChallenge(error, {
              onProgress,
              waitIfPaused,
              shouldStop,
              taskType: 'batchComments',
              stage: 'collecting',
              current: index,
              total: targets.length,
              collectionRunId: run.collectionRunId,
            });
            if (pauseResult.handled) {
              commentPacer.recordFailure();
              if (pauseResult.stopped) break;
              continue;
            }
            throw error;
          }
        }
        if (shouldStop()) break;

        commentPacer.recordSuccess();
        success += 1;
        totalComments += Number(collected?.total || 0);
        results.push({
          ...target,
          ok: true,
          noteId: collected?.note?.noteId || '',
          videoId: collected?.note?.platformContentId || '',
          title: collected?.note?.title || target.titleHint || '',
          totalComments: Number(collected?.total || 0),
          batchSelectionMode: selectionMode,
          batchRank: topByLikes ? Number(target.rank || 0) : index + 1,
          batchLikesSnapshot: Number(target.likes || 0),
        });
      } catch (err) {
        commentPacer.recordFailure();
        failed += 1;
        results.push({
          ...target,
          ok: false,
          totalComments: 0,
          batchSelectionMode: selectionMode,
          batchRank: topByLikes ? Number(target.rank || 0) : index + 1,
          batchLikesSnapshot: Number(target.likes || 0),
          error: String(err?.message || err),
        });
      }

      onProgress?.({
        current: index + 1,
        total: targets.length,
        message: `已完成 ${index + 1}/${targets.length} 条视频评论采集`,
        target,
      });
      void reportHeartbeat.report(run.collectionRunId, {
        taskState: shouldStop() ? 'stopping' : 'running',
        stage: 'collecting',
        current: index + 1,
        total: targets.length,
        message: `已完成 ${index + 1}/${targets.length} 条视频评论采集`,
      }).catch(() => {});
      await collectionRunStore.updateById(run.collectionRunId, buildDouyinBatchCommentsProgressPatch({
        targets,
        results,
        totalComments,
        processedCount: index + 1,
      })).catch(() => {});

      await commentPacer.wait({ waitIfPaused, shouldStop });
    }

    const stopped = shouldStop();
    const firstError = results.find((item) => !item.ok && item.error)?.error || '';
    const summary = {
      ...buildDouyinBatchCommentsRunPatch({ targets, results, totalComments }),
      itemsSucceeded: success,
      itemsFailed: failed,
      totalComments,
      results,
      error: firstError || undefined,
    };
    if (stopped) {
      await collectionRunStore.markStopped(run.collectionRunId, summary);
    } else if (success > 0) {
      await collectionRunStore.markDone(run.collectionRunId, summary);
    } else {
      await collectionRunStore.markFailed(run.collectionRunId, firstError || '批量评论采集全部失败', summary);
    }

    const commentFinalLabel = stopped ? '已停止' : (success > 0 ? '已完成' : '失败');
    emitDouyinBatchProgress(onProgress, {
      taskState: stopped ? 'idle' : 'done',
      stage: 'done',
      current: targets.length,
      total: targets.length,
      message: `${commentFinalLabel}：成功 ${success} 条，评论 ${totalComments} 条（共 ${targets.length} 个视频）`,
    });

    return {
      ok: success > 0,
      error: !success ? (firstError || '批量评论采集全部失败') : '',
      totalVideos: targets.length,
      successVideos: success,
      failedVideos: failed,
      totalComments,
      results,
      stopped,
      collectionRunId: run.collectionRunId,
    };
  } catch (err) {
    await collectionRunStore.markFailed(run.collectionRunId, err, {
      itemsPlanned: 0,
      itemsSucceeded: 0,
      itemsFailed: 0,
      totalComments: 0,
    });
    throw err;
  }
}
