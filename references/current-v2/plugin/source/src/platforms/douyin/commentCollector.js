import { BATCH_CONFIG } from '../../shared/constants.js';
import { parseCount, randomDelay } from '../../shared/utils.js';
import { commentStore } from '../../db/commentStore.js';
import { mediaAssetStore } from '../../db/mediaAssetStore.js';
import { collectionRunStore } from '../../db/collectionRunStore.js';
import { noteStore } from '../../db/noteStore.js';
import { collectDouyinVideo, collectDouyinVideoById, collectDouyinVideoByAweme } from './videoCollector.js';
import { detectDouyinPageType, isStrictDouyinDetailPage, extractDouyinContentId } from './pageDetector.js';
import {
  fetchCommentListPage,
  fetchReplyListPage,
  mapDouyinCommentRecord,
} from './commentApi.js';
import {
  buildCommentImageRecordKey,
  buildCommentImageAssets,
  countCommentImageAssets,
  fetchImageBlob,
  inferImageExt,
  downloadBlob,
} from './commentMedia.js';
import { buildDouyinSingleCommentRunPatch } from './commentTaskSupport.js';
import { createCollectionRunHeartbeatReporter } from '../../workbench/runtime/heartbeat.js';
import {
  createDouyinSecurityChallengeError,
  detectDouyinSecurityChallenge,
  pauseForDouyinSecurityChallenge,
} from './securityChallenge.js';
import JSZip from 'jszip';

const reportHeartbeat = createCollectionRunHeartbeatReporter({ collectionRunStore });

function sanitizeFileSegment(value = '', fallback = 'douyin') {
  const normalized = String(value || '')
    .trim()
    .replace(/[\\/:*?"<>|\u0000-\u001F]/g, '_')
    .replace(/\s+/g, '_')
    .slice(0, 30);
  return normalized || fallback;
}

function buildCommentImagePackageBase(note = {}) {
  const keyword = sanitizeFileSegment(note?.searchKeyword || '', '');
  const contentId = sanitizeFileSegment(note?.platformContentId || note?.noteId || 'douyin');
  return keyword ? `${keyword}_${contentId}` : contentId;
}

async function resolveCurrentDouyinNote({ collectionRunId = '' } = {}) {
  // 视频详情页：URL 里已有 video ID，直接走轻量路径，不做 DOM 探测
  const videoId = extractDouyinContentId(window.location.href);
  console.log('[灵感爆爆爆] resolveCurrentDouyinNote: videoId =', videoId || '(空)');
  if (videoId) {
    console.log('[灵感爆爆爆] 尝试轻量路径: collectDouyinVideoById...');
    const idResult = await collectDouyinVideoById(videoId, {
      triggerSource: 'comment_collect',
      collectionRunId,
    });
    console.log('[灵感爆爆爆] collectDouyinVideoById 结果: ok =', idResult?.ok, ', hasData =', !!idResult?.data);
    if (idResult?.ok && idResult?.data) return idResult.data;
  }
  // 降级：走完整的 DOM + API 采集
  console.log('[灵感爆爆爆] 降级到完整 DOM 采集: collectDouyinVideo...');
  const videoResult = await collectDouyinVideo({
    triggerSource: 'comment_collect',
    collectionRunId,
  });
  console.log('[灵感爆爆爆] collectDouyinVideo 结果: ok =', videoResult?.ok, ', hasData =', !!videoResult?.data);
  if (!videoResult?.ok || !videoResult?.data) {
    throw new Error(videoResult?.error || '未能定位当前抖音视频');
  }
  return videoResult.data;
}

async function createSingleCommentRun({
  taskType = 'singleComments',
  triggerSource = 'manual',
  config = {},
  meta = {},
} = {}) {
  const page = detectDouyinPageType();
  return collectionRunStore.createRun({
    platform: 'douyin',
    taskType,
    pageType: page?.type || 'unknown',
    triggerSource,
    config,
    meta,
  });
}

async function collectDouyinCommentsForNote(note, {
  maxTotal = 0,
  maxSubComments = BATCH_CONFIG.maxSubComments,
  sortMode = 'hot',
  collectionRunId = '',
  persistComments = true,
  persistImageAssets = true,
  scanImages = true,
  progressLabel = '已采集',
  onProgress = null,
  onSecurityPause = null,
  shouldStop = () => false,
  waitIfPaused = async () => {},
} = {}) {
  const awemeId = String(note.platformContentId || '').trim();
  if (!awemeId) {
    throw new Error('未能解析当前视频 ID');
  }

  const allComments = [];
  const allRawComments = [];
  const imageComments = [];
  const imageRawComments = [];
  const seenIds = new Set();
  const seenImageCommentIds = new Set();
  let positionIndex = 0;
  let scannedImages = 0;
  const MAX_REPLY_DEPTH = 3;
  const MAX_EMPTY_PAGES = 2;

  function trackCommentImages(commentRecord, rawComment) {
    if (!scanImages) return;
    const imageCount = countCommentImageAssets([commentRecord], [rawComment]);
    if (imageCount === 0) return;
    const imageCommentKey = buildCommentImageRecordKey(commentRecord, rawComment);
    if (!imageCommentKey || seenImageCommentIds.has(imageCommentKey)) return;
    seenImageCommentIds.add(imageCommentKey);
    imageComments.push(commentRecord);
    imageRawComments.push(rawComment);
    scannedImages += imageCount;
  }

  async function handleSecurityChallenge(error) {
    const pauseResult = await pauseForDouyinSecurityChallenge(error, {
      current: allComments.length,
      total: maxTotal,
      scannedImages: scanImages ? scannedImages : 0,
      onPause: onSecurityPause,
      waitIfPaused,
      shouldStop,
    });
    if (pauseResult.handled) {
      onProgress?.({
        status: 'paused',
        taskState: 'paused',
        current: allComments.length,
        scannedImages,
        message: pauseResult.message,
      });
    }
    return pauseResult;
  }

  // Recursive reply collector supporting level 2+
  async function collectReplies(parentCommentId, rootCommentId, currentLevel, rawParentComment) {
    if (currentLevel > MAX_REPLY_DEPTH || shouldStop()) return;

    const replyTotal = Number(
      rawParentComment?.reply_comment_total
      ?? rawParentComment?.reply_comment_count
      ?? rawParentComment?.reply_count
      ?? rawParentComment?.replyCount
      ?? 0
    );

    let replyCursor = 0;
    let replyHasMore = replyTotal > 0 || currentLevel >= 3;
    let replyCollected = 0;
    let emptyRounds = 0;

    while (replyHasMore && !shouldStop()) {
      await waitIfPaused();
      if (shouldStop()) break;
      if (detectDouyinSecurityChallenge()) {
        const pauseResult = await handleSecurityChallenge(
          createDouyinSecurityChallengeError({ reason: 'dom_signal' }),
        );
        if (pauseResult.handled) {
          if (pauseResult.stopped) break;
          continue;
        }
        throw createDouyinSecurityChallengeError({ reason: 'dom_signal' });
      }
      if (maxSubComments > 0 && replyCollected >= maxSubComments) break;
      if (maxTotal > 0 && allComments.length >= maxTotal) break;

      let replyPage;
      try {
        replyPage = await fetchReplyListPage(awemeId, parentCommentId, {
          cursor: replyCursor,
          count: Math.min(20, maxSubComments > 0 ? Math.max(1, maxSubComments - replyCollected) : 20),
        });
      } catch (error) {
        const pauseResult = await handleSecurityChallenge(error);
        if (pauseResult.handled) {
          if (pauseResult.stopped) break;
          continue;
        }
        replyPage = { comments: [], cursor: 0, hasMore: false };
      }

      const replies = Array.isArray(replyPage.comments) ? replyPage.comments : [];
      if (replies.length === 0) break;

      let newRepliesThisRound = 0;
      for (const rawReply of replies) {
        await waitIfPaused();
        if (shouldStop()) break;
        if (maxSubComments > 0 && replyCollected >= maxSubComments) break;
        if (maxTotal > 0 && allComments.length >= maxTotal) break;

        const replyRecord = mapDouyinCommentRecord(rawReply, note, {
          parseCount,
          parentCommentId,
          rootCommentId,
          level: currentLevel,
          sortMode,
          positionIndex: ++positionIndex,
          collectionRunId,
        });

        if (!replyRecord.commentId) {
          replyRecord.commentId = `dy_fb_${awemeId}_${positionIndex}`;
          replyRecord.dataQuality = 'degraded';
          replyRecord.qualityReason = 'synthetic_comment_id';
        }

        trackCommentImages(replyRecord, rawReply);
        if (seenIds.has(replyRecord.commentId)) continue;
        seenIds.add(replyRecord.commentId);
        allComments.push(replyRecord);
        allRawComments.push(rawReply);
        replyCollected += 1;
        newRepliesThisRound += 1;

        await collectReplies(replyRecord.commentId, rootCommentId, currentLevel + 1, rawReply);
      }

      const prevReplyCursor = replyCursor;
      replyCursor = replyPage.cursor;
      if (replyCursor === prevReplyCursor) break;
      replyHasMore = Boolean(replyPage.hasMore) && (!maxSubComments || replyCollected < maxSubComments);
      if (newRepliesThisRound === 0) {
        emptyRounds += 1;
        if (emptyRounds >= MAX_EMPTY_PAGES) break;
      } else {
        emptyRounds = 0;
      }
      await randomDelay(180, 320);
    }
  }

  // Single sort-mode pass over parent comments
  async function collectParentPass(currentSortMode) {
    let cursor = 0;
    let hasMore = true;
    let emptyPages = 0;

    while (hasMore && !shouldStop()) {
      await waitIfPaused();
      if (shouldStop()) break;
      if (detectDouyinSecurityChallenge()) {
        const pauseResult = await handleSecurityChallenge(
          createDouyinSecurityChallengeError({ reason: 'dom_signal' }),
        );
        if (pauseResult.handled) {
          if (pauseResult.stopped) break;
          continue;
        }
        throw createDouyinSecurityChallengeError({ reason: 'dom_signal' });
      }
      if (maxTotal > 0 && allComments.length >= maxTotal) break;

      let page;
      try {
        page = await fetchCommentListPage(awemeId, {
          cursor,
          count: Math.min(20, maxTotal > 0 ? Math.max(1, maxTotal - allComments.length) : 20),
          sortMode: currentSortMode,
        });
      } catch (error) {
        const pauseResult = await handleSecurityChallenge(error);
        if (pauseResult.handled) {
          if (pauseResult.stopped) break;
          continue;
        }
        throw error;
      }

      const parents = Array.isArray(page.comments) ? page.comments : [];
      console.log(`[灵感爆爆爆] 评论翻页(${currentSortMode}): cursor=${cursor}, 本页 ${parents.length} 条, hasMore=${page.hasMore}, 累计 ${allComments.length} 条`);
      if (parents.length === 0) break;

      let newThisPage = 0;
      for (const rawParent of parents) {
        await waitIfPaused();
        if (shouldStop()) break;
        const parentRecord = mapDouyinCommentRecord(rawParent, note, {
          parseCount,
          parentCommentId: '',
          rootCommentId: '',
          level: 1,
          sortMode: currentSortMode,
          positionIndex: ++positionIndex,
          collectionRunId,
        });

        if (!parentRecord.commentId) {
          parentRecord.commentId = `dy_fb_${awemeId}_${positionIndex}`;
          parentRecord.dataQuality = 'degraded';
          parentRecord.qualityReason = 'synthetic_comment_id';
        }

        trackCommentImages(parentRecord, rawParent);
        if (seenIds.has(parentRecord.commentId)) continue;
        seenIds.add(parentRecord.commentId);
        allComments.push(parentRecord);
        allRawComments.push(rawParent);
        newThisPage += 1;

        await collectReplies(parentRecord.commentId, parentRecord.commentId, 2, rawParent);

        const progressMessage = scanImages
          ? `${progressLabel} ${allComments.length} 条评论，发现 ${scannedImages} 张图片${maxTotal > 0 ? `（上限 ${maxTotal}）` : ''}`
          : `${progressLabel} ${allComments.length} 条评论${maxTotal > 0 ? `（上限 ${maxTotal}）` : ''}`;
        onProgress?.({
          status: 'collecting',
          current: allComments.length,
          scannedImages,
          message: progressMessage,
        });
        void reportHeartbeat.report(collectionRunId, {
          taskState: shouldStop() ? 'stopping' : 'running',
          stage: 'collecting',
          current: allComments.length,
          total: maxTotal,
          message: progressMessage,
        }).catch(() => {});

        if (maxTotal > 0 && allComments.length >= maxTotal) break;
        await randomDelay(140, 260);
      }

      // Tolerate up to MAX_EMPTY_PAGES consecutive pages with 0 new comments
      if (newThisPage === 0) {
        emptyPages += 1;
        if (emptyPages >= MAX_EMPTY_PAGES) break;
      } else {
        emptyPages = 0;
      }

      const prevCursor = cursor;
      cursor = page.cursor;
      hasMore = Boolean(page.hasMore);
      if (cursor === prevCursor) break;
      await randomDelay(220, 360);
    }
  }

  // First pass with requested sort mode (default: hot)
  await collectParentPass(sortMode);

  // Second pass with alternate sort mode to catch missed comments
  const altSortMode = sortMode === 'latest' ? 'hot' : 'latest';
  if (!shouldStop()) {
    console.log(`[灵感爆爆爆] 开始第二轮(${altSortMode})补充采集，当前已采 ${allComments.length} 条`);
    await collectParentPass(altSortMode);
    console.log(`[灵感爆爆爆] 第二轮完成，总计 ${allComments.length} 条`);
  }

  if (persistComments && allComments.length > 0) {
    await commentStore.bulkUpsert(allComments);
  }

  const imageAssets = buildCommentImageAssets(imageComments, imageRawComments, { collectionRunId });
  scannedImages = imageAssets.length;
  if (persistImageAssets && imageAssets.length > 0) {
    await mediaAssetStore.bulkUpsert(imageAssets);
  }

  return {
    stopped: shouldStop(),
    total: allComments.length,
    comments: allComments,
    imageAssets,
    scannedImages,
    note,
  };
}

export async function collectDouyinComments({
  maxTotal = 0,
  maxSubComments = BATCH_CONFIG.maxSubComments,
  sortMode = 'hot',
  collectionRunId = '',
  manageCollectionRun = true,
  triggerSource = 'manual',
  onProgress = null,
  onSecurityPause = null,
  shouldStop = () => false,
  waitIfPaused = async () => {},
} = {}) {
  console.log('[灵感爆爆爆] collectDouyinComments: 开始');
  let run = null;
  let note = null;
  try {
    console.log('[灵感爆爆爆] collectDouyinComments: 开始定位当前视频...');
    note = await resolveCurrentDouyinNote({ collectionRunId });
    console.log('[灵感爆爆爆] collectDouyinComments: 视频定位完成, title =', (note?.title || '').slice(0, 30));
    if (!collectionRunId && manageCollectionRun) {
      run = await createSingleCommentRun({
        taskType: 'singleComments',
        triggerSource,
        config: {
          maxTotal,
          maxSubComments,
          sortMode,
        },
        meta: {
          contentId: note?.contentId || '',
          platformContentId: note?.platformContentId || '',
          noteId: note?.noteId || '',
        },
      });
      collectionRunId = run.collectionRunId;
    }
    if (collectionRunId && note) {
      note = {
        ...note,
        collectionRunId,
      };
      await noteStore.upsert(note);
    }

    const result = await collectDouyinCommentsForNote(note, {
      maxTotal,
      maxSubComments,
      sortMode,
      collectionRunId,
      persistComments: true,
      persistImageAssets: false,
      scanImages: false,
      progressLabel: '已采集',
      onProgress,
      onSecurityPause,
      shouldStop,
      waitIfPaused,
    });

    if (run) {
      if (result.stopped) {
        await collectionRunStore.markStopped(
          run.collectionRunId,
          buildDouyinSingleCommentRunPatch({
            stopped: true,
            totalComments: result.total || 0,
            note,
          }),
        );
      } else {
        await collectionRunStore.markDone(
          run.collectionRunId,
          buildDouyinSingleCommentRunPatch({
            stopped: false,
            totalComments: result.total || 0,
            note,
          }),
        );
      }
    }

    return {
      ...result,
      collectionRunId: collectionRunId || '',
    };
  } catch (err) {
    if (run) {
      await collectionRunStore.markFailed(run.collectionRunId, err, {
        itemsPlanned: 1,
        itemsSucceeded: 0,
        itemsFailed: 1,
        totalComments: 0,
        contentId: note?.contentId || '',
        targetIds: [note?.platformContentId || ''].filter(Boolean),
      });
    }
    throw err;
  }
}

export async function collectDouyinCommentsByVideoId(videoId, {
  note = null,
  aweme = null,
  url = '',
  titleHint = '',
  triggerSource = 'batch_profile_comment',
  collectionRunId = '',
  batchSelectionMode = '',
  batchRank = 0,
  batchLikesSnapshot = 0,
  searchKeyword = '',
  searchPageUrl = '',
  maxTotal = 0,
  maxSubComments = BATCH_CONFIG.maxSubComments,
  sortMode = 'hot',
  onProgress = null,
  onSecurityPause = null,
  shouldStop = () => false,
  waitIfPaused = async () => {},
} = {}) {
  let targetNote = note;
  if (!targetNote?.platformContentId) {
    const videoResult = aweme
      ? await collectDouyinVideoByAweme(aweme, {
          url,
          titleHint,
          triggerSource,
          collectionRunId,
          batchSelectionMode,
          batchRank,
          batchLikesSnapshot,
          searchKeyword,
          searchPageUrl,
        })
      : await collectDouyinVideoById(videoId, {
          url,
          titleHint,
          triggerSource,
          collectionRunId,
          batchSelectionMode,
          batchRank,
          batchLikesSnapshot,
          searchKeyword,
          searchPageUrl,
        });
    if (!videoResult?.ok || !videoResult?.data) {
      throw new Error(videoResult?.error || '未能定位目标视频');
    }
    targetNote = videoResult.data;
  }

  return collectDouyinCommentsForNote(targetNote, {
    maxTotal,
    maxSubComments,
    sortMode,
    collectionRunId,
    persistComments: true,
    persistImageAssets: true,
    progressLabel: '已采集',
    onProgress,
    onSecurityPause,
    shouldStop,
    waitIfPaused,
  });
}

export async function downloadDouyinCommentImages({
  maxTotal = 0,
  maxSubComments = BATCH_CONFIG.maxSubComments,
  collectionRunId = '',
  onProgress = null,
  onSecurityPause = null,
  shouldStop = () => false,
  waitIfPaused = async () => {},
} = {}) {
  console.log('[灵感爆爆爆] downloadDouyinCommentImages: 开始');
  if (!isStrictDouyinDetailPage()) {
    throw new Error('请先进入抖音视频详情页，再下载评论图片区');
  }
  let run = null;
  let result = null;
  let note = null;
  try {
    if (!collectionRunId) {
      run = await createSingleCommentRun({
        taskType: 'commentImageDownload',
        triggerSource: 'manual',
        config: {
          maxTotal,
          maxSubComments,
        },
      });
      collectionRunId = run.collectionRunId;
    }

    console.log('[灵感爆爆爆] downloadDouyinCommentImages: 开始定位视频...');
    note = await resolveCurrentDouyinNote();
    console.log('[灵感爆爆爆] downloadDouyinCommentImages: 视频定位完成, 开始采集评论...');
    result = await collectDouyinCommentsForNote(note, {
      maxTotal,
      maxSubComments,
      sortMode: 'hot',
      collectionRunId,
      persistComments: false,
      persistImageAssets: true,
      progressLabel: '已扫描',
      onSecurityPause,
      shouldStop,
      waitIfPaused,
      onProgress: (payload) => onProgress?.({
        phase: 'collect',
        current: payload.current || 0,
        total: 0,
        scannedImages: payload.scannedImages || 0,
        message: payload.message || `正在扫描评论区图片，已发现 ${payload.scannedImages || 0} 张...`,
      }),
    });

    const imageAssets = Array.isArray(result.imageAssets) ? result.imageAssets : [];
    const totalImages = imageAssets.length;
    const discoveredImages = totalImages;
    if (imageAssets.length === 0) {
      const stoppedWithoutImages = result.stopped;
      if (run) {
        const finalize = stoppedWithoutImages ? collectionRunStore.markStopped : collectionRunStore.markDone;
        await finalize.call(collectionRunStore, run.collectionRunId, {
          itemsPlanned: 0,
          itemsSucceeded: 0,
          itemsFailed: 0,
          totalComments: result.total || 0,
          totalImages: 0,
          scannedImages: result.scannedImages || 0,
          contentId: result.note?.contentId || '',
          targetIds: [result.note?.platformContentId || ''].filter(Boolean),
          noImages: true,
        });
      }
      return {
        success: false,
        total: 0,
        downloaded: 0,
        failed: 0,
        hdCount: 0,
        scannedImages: result.scannedImages || 0,
        collectionRunId: collectionRunId || '',
        stopped: stoppedWithoutImages,
        message: stoppedWithoutImages ? '已停止扫描，未发现可下载图片' : '未发现评论区图片',
      };
    }

    const zip = new JSZip();
    let success = 0;
    let failed = 0;
    let hdCount = 0;
    let sdCount = 0;
    const finalAssets = [];
    const zipEntries = new Array(imageAssets.length).fill(null);
    const stopRequestedDuringScan = Boolean(result.stopped);
    let stoppedDuringDownload = false;
    let nextIndex = 0;
    let completed = 0;
    const workerCount = Math.max(1, Math.min(4, imageAssets.length));

    onProgress?.({
      phase: 'download',
      current: 0,
        total: imageAssets.length,
      scannedImages: discoveredImages,
      message: `已发现 ${discoveredImages} 张图片，准备打包 0/${imageAssets.length}`,
      success,
      failed,
      hdCount,
    });
    void reportHeartbeat.report(collectionRunId, {
      taskState: 'running',
      stage: 'downloading',
      current: 0,
      total: imageAssets.length,
      message: `已发现 ${discoveredImages} 张图片，准备打包 0/${imageAssets.length}`,
    }).catch(() => {});

    async function worker() {
      while (true) {
        await waitIfPaused();
        const currentIndex = nextIndex;
        nextIndex += 1;
        if (currentIndex >= imageAssets.length) return;
        if (!stopRequestedDuringScan && shouldStop()) {
          stoppedDuringDownload = true;
        }
        const asset = imageAssets[currentIndex];
        const fetched = await fetchImageBlob(asset.candidateUrls || [asset.url]);
        if (!fetched.success) {
          failed += 1;
          finalAssets.push({
            ...asset,
            downloadStatus: '失败',
            quality: asset.quality || 'unknown',
            lastResolvedAt: Date.now(),
          });
        } else {
          const ext = inferImageExt(fetched.candidate, 'jpg');
          zipEntries[currentIndex] = {
            filename: `评论图片_${String(currentIndex + 1).padStart(3, '0')}.${ext}`,
            blob: fetched.blob,
          };
          success += 1;
          const quality = fetched.candidateIndex === 0 ? 'HD' : 'SD';
          if (quality === 'HD') hdCount += 1;
          else sdCount += 1;
          finalAssets.push({
            ...asset,
            url: fetched.candidate || asset.url,
            quality,
            downloadStatus: '已完成',
            lastResolvedAt: Date.now(),
          });
        }
        completed += 1;
        onProgress?.({
          phase: 'download',
          current: completed,
          total: imageAssets.length,
          scannedImages: discoveredImages,
          message: stoppedDuringDownload
            ? `停止中，正在打包已发现图片 ${completed}/${imageAssets.length}`
            : `已发现 ${discoveredImages} 张图片，正在下载 ${completed}/${imageAssets.length}`,
          success,
          failed,
          hdCount,
        });
        void reportHeartbeat.report(collectionRunId, {
          taskState: stoppedDuringDownload ? 'stopping' : 'running',
          stage: 'downloading',
          current: completed,
          total: imageAssets.length,
          message: stoppedDuringDownload
            ? `停止中，正在打包已发现图片 ${completed}/${imageAssets.length}`
            : `已发现 ${discoveredImages} 张图片，正在下载 ${completed}/${imageAssets.length}`,
        }).catch(() => {});
      }
    }

    await Promise.all(Array.from({ length: workerCount }, () => worker()));

    const stopped = stopRequestedDuringScan || stoppedDuringDownload || shouldStop();

    zipEntries.forEach((entry) => {
      if (!entry) return;
      zip.file(entry.filename, entry.blob);
    });

    if (finalAssets.length > 0) {
      await mediaAssetStore.bulkUpsert(finalAssets);
    }

    if (success === 0 && stopped) {
      if (run) {
        await collectionRunStore.markStopped(run.collectionRunId, {
          itemsPlanned: imageAssets.length,
          itemsSucceeded: 0,
          itemsFailed: failed,
          totalComments: result.total || 0,
          totalImages: imageAssets.length,
          scannedImages: result.scannedImages || imageAssets.length,
          hdCount,
          sdCount,
          contentId: result.note?.contentId || '',
          targetIds: [result.note?.platformContentId || ''].filter(Boolean),
        });
      }
      return {
        success: true,
        stopped: true,
        total: imageAssets.length,
        downloaded: 0,
        failed,
        hdCount,
        sdCount,
        scannedImages: result.scannedImages || imageAssets.length,
        note: result.note,
        collectionRunId: collectionRunId || '',
        message: '已停止打包，未成功获取任何图片',
      };
    }

    if (success === 0) {
      if (run) {
        await collectionRunStore.markStopped(run.collectionRunId, {
          itemsPlanned: totalImages,
          itemsSucceeded: 0,
          itemsFailed: failed,
          totalComments: result.total || 0,
          totalImages,
          scannedImages: result.scannedImages || totalImages,
          hdCount: 0,
          sdCount,
          contentId: result.note?.contentId || '',
          targetIds: [result.note?.platformContentId || ''].filter(Boolean),
        });
      }
      return {
        success: false,
        stopped: true,
        total: totalImages,
        downloaded: 0,
        failed,
        hdCount: 0,
        sdCount,
        scannedImages: result.scannedImages || totalImages,
        note: result.note,
        collectionRunId: collectionRunId || '',
        message: stopped
          ? `已停止扫描，已发现 ${totalImages} 张图片，但未完成任何图片下载`
          : '评论图片区下载失败，未成功下载任何图片',
      };
    }

    const blob = await zip.generateAsync({
      type: 'blob',
      compression: 'STORE',
    });
    const packageBase = buildCommentImagePackageBase(result.note);
    const zipName = `灵感爆爆爆_抖音评论图片区_${packageBase}_${Date.now()}.zip`;
    await downloadBlob(blob, zipName);

    if (run) {
      const finalize = stopped ? collectionRunStore.markStopped : collectionRunStore.markDone;
      await finalize.call(collectionRunStore, run.collectionRunId, {
        itemsPlanned: totalImages,
        itemsSucceeded: success,
        itemsFailed: failed,
        totalComments: result.total || 0,
        totalImages,
        scannedImages: result.scannedImages || totalImages,
        hdCount,
        sdCount,
        contentId: result.note?.contentId || '',
        targetIds: [result.note?.platformContentId || ''].filter(Boolean),
        zipName,
      });
    }

    return {
      success: true,
      stopped,
      total: totalImages,
      downloaded: success,
      failed,
      hdCount,
      sdCount,
      scannedImages: result.scannedImages || totalImages,
      note: result.note,
      collectionRunId: collectionRunId || '',
      zipName,
      message: stopped
        ? `已停止扫描，已打包 ${success}/${totalImages} 张已发现图片`
        : '',
    };
  } catch (err) {
    if (run) {
      await collectionRunStore.markFailed(run.collectionRunId, err, {
        itemsPlanned: 0,
        itemsSucceeded: 0,
        itemsFailed: 0,
        totalComments: result?.total || 0,
        contentId: note?.contentId || result?.note?.contentId || '',
        targetIds: [note?.platformContentId || result?.note?.platformContentId || ''].filter(Boolean),
      });
    }
    throw err;
  }
}
