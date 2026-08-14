import JSZip from 'jszip';

import {
  sanitizePathSegment,
  detectFileExt,
  normalizeCandidates,
  downloadBlob,
  basenameFromPath,
} from './mediaDownloadUtils.js';
import { mediaAssetStore } from '../db/mediaAssetStore.js';

export function createNoteMediaDownloadService({
  MSG,
  noteStore,
  sendToBackground,
  collectNote,
  loadDouyinRuntime,
  extractNoteId,
} = {}) {
  function decodeBase64ToBytes(base64 = '') {
    const value = String(base64 || '').trim();
    if (!value) return new Uint8Array();
    if (typeof atob === 'function') {
      const decoded = atob(value);
      const bytes = new Uint8Array(decoded.length);
      for (let i = 0; i < decoded.length; i += 1) {
        bytes[i] = decoded.charCodeAt(i);
      }
      return bytes;
    }
    if (typeof Buffer !== 'undefined') {
      return Uint8Array.from(Buffer.from(value, 'base64'));
    }
    throw new Error('base64_decoder_unavailable');
  }

  function parseDataUrl(dataUrl = '') {
    const raw = String(dataUrl || '').trim();
    const match = raw.match(/^data:([^;,]+)?((?:;[^;,=]+=[^;,]+)*)(;base64)?,([\s\S]*)$/i);
    if (!match) throw new Error('invalid_data_url');

    const mimeType = String(match[1] || 'application/octet-stream').trim() || 'application/octet-stream';
    const isBase64 = Boolean(match[3]);
    const payload = String(match[4] || '');
    if (isBase64) {
      return {
        mimeType,
        bytes: decodeBase64ToBytes(payload),
      };
    }
    return {
      mimeType,
      bytes: new TextEncoder().encode(decodeURIComponent(payload)),
    };
  }

  function buildNoteMediaZipName(note) {
    const noteId = sanitizePathSegment(note?.noteId || 'unknown');
    const title = sanitizePathSegment(note?.title || noteId, noteId);
    return `灵感爆爆爆_笔记媒体_${noteId}_${title}.zip`;
  }

  function getNoteImageSlots(note = {}) {
    if (Array.isArray(note?.imageCandidateSlots) && note.imageCandidateSlots.length > 0) {
      return note.imageCandidateSlots.map((slot) => ({
        ordinal: Number.isSafeInteger(slot?.ordinal) && slot.ordinal >= 0 ? slot.ordinal : null,
        candidates: normalizeCandidates(slot?.candidates),
      })).filter((slot) => slot.candidates.length > 0);
    }
    const legacyGroups = Array.isArray(note?.imageCandidates) && note.imageCandidates.length > 0
      ? note.imageCandidates
      : (Array.isArray(note?.images) ? note.images.map((url) => [url]) : []);
    return legacyGroups.map((group) => ({ ordinal: null, candidates: normalizeCandidates(group) }));
  }

  function getNoteImageGroups(note = {}) {
    return getNoteImageSlots(note).map((slot) => slot.candidates);
  }

  function getNoteCoverCandidates(note = {}, imageGroups = getNoteImageGroups(note)) {
    return normalizeCandidates([
      note?.cover,
      note?.coverImg,
      note?.coverUrl,
      note?.thumbnail,
      imageGroups?.[0],
      Array.isArray(note?.images) ? note.images[0] : '',
    ]);
  }

  function getNoteLivePhotoGroups(note = {}) {
    const streams = Array.isArray(note?.livePhotoStreams) ? note.livePhotoStreams : [];
    return streams.map((item, index) => {
      const imageIndex = Number(item?.imageIndex);
      const ordinal = Number.isSafeInteger(imageIndex) && imageIndex > 0 ? imageIndex - 1 : null;
      return {
        index: ordinal === null ? index + 1 : ordinal + 1,
        ordinal,
        candidates: normalizeCandidates([item?.candidates, item?.url, item?.streams]),
      };
    }).filter((item) => item.candidates.length > 0);
  }

  function getNoteVideoCandidates(note = {}) {
    const sortedVideoStreams = Array.isArray(note?.videoStreams)
      ? [...note.videoStreams].sort((a, b) => Number(b?.bitrate || 0) - Number(a?.bitrate || 0))
      : [];
    return normalizeCandidates([
      ...sortedVideoStreams.map((item) => item?.url).filter(Boolean),
      note?.videoDownloadUrl,
      note?.videoPlayUrl,
      note?.video,
    ]);
  }

  function getNoteMediaDownloadOptions(note = {}) {
    const imageSlots = getNoteImageSlots(note);
    const imageGroups = imageSlots.map((slot) => slot.candidates);
    const coverCandidates = getNoteCoverCandidates(note, imageGroups);
    const liveGroups = getNoteLivePhotoGroups(note);
    const videoCandidates = getNoteVideoCandidates(note);
    const isVideoNote = String(note?.type || '').trim() === 'video' || videoCandidates.length > 0;
    const options = [];

    if (coverCandidates.length > 0) {
      options.push({ value: 'cover', label: '下载封面', count: 1 });
    }
    if (imageGroups.length > 0 && (!isVideoNote || imageGroups.length > 1)) {
      options.push({ value: 'images', label: '下载所有图片', count: imageGroups.length });
    }
    if (liveGroups.length > 0) {
      options.push({ value: 'live', label: '下载 Live', count: liveGroups.length });
    }
    if (videoCandidates.length > 0) {
      options.push({ value: 'video', label: '下载视频', count: 1 });
    }

    return options;
  }

  function normalizeMediaTypeSelection(mediaTypes) {
    if (!Array.isArray(mediaTypes) || mediaTypes.length === 0) {
      return { all: true, values: new Set() };
    }

    const values = new Set(
      mediaTypes.map((item) => String(item || '').trim()).filter(Boolean),
    );
    return {
      all: values.has('all'),
      values,
    };
  }

  function buildNoteMediaDownloadQueue(note, options = {}) {
    const noteId = sanitizePathSegment(note?.noteId || 'unknown');
    const title = sanitizePathSegment(note?.title || noteId, noteId);
    const folder = `灵感爆爆爆/笔记媒体/${noteId}_${title}`;
    const queue = [];
    const selected = normalizeMediaTypeSelection(options.mediaTypes);
    const availableTypes = new Set(getNoteMediaDownloadOptions(note).map((item) => item.value));
    const shouldInclude = (type) => selected.all
      ? availableTypes.has(type)
      : selected.values.has(type);

    const imageSlots = getNoteImageSlots(note);
    const imageGroups = imageSlots.map((slot) => slot.candidates);
    const includeImages = shouldInclude('images');
    const includeCover = shouldInclude('cover') && !includeImages;

    if (includeCover) {
      const candidates = getNoteCoverCandidates(note, imageGroups);
      if (candidates.length > 0) {
        const ext = detectFileExt(candidates[0], 'jpg');
        queue.push({
          id: 'cover-1',
          type: 'cover',
          ordinal: 0,
          candidates,
          filename: `${folder}/封面.${ext}`,
        });
      }
    }

    if (includeImages) imageSlots.forEach((slot, index) => {
      const candidates = slot.candidates;
      if (candidates.length === 0) return;
      const ext = detectFileExt(candidates[0], 'jpg');
      queue.push({
        id: `image-${index + 1}`,
        type: 'image',
        ordinal: slot.ordinal,
        candidates,
        filename: `${folder}/图_${String(index + 1).padStart(2, '0')}.${ext}`,
      });
    });

    if (shouldInclude('live')) {
      getNoteLivePhotoGroups(note).forEach((item) => {
        const ext = detectFileExt(item.candidates[0], 'mp4');
        queue.push({
          id: `live-${item.index}`,
          type: 'live_photo',
          ordinal: item.ordinal,
          candidates: item.candidates,
          filename: `${folder}/Live_${String(item.index).padStart(2, '0')}.${ext}`,
        });
      });
    }

    const videoCandidates = getNoteVideoCandidates(note);
    if (shouldInclude('video') && videoCandidates.length > 0) {
      queue.push({
        id: 'video-1',
        type: 'video',
        ordinal: 0,
        candidates: videoCandidates,
        filename: `${folder}/${title}.mp4`,
      });
    }

    return queue;
  }

  function shouldPackageAsZip(note, queue = [], options = {}) {
    if (options.packageAsZip !== true) return false;
    const platform = String(note?.platform || 'xhs').trim();
    if (platform === 'douyin') return false;
    return !queue.some((task) => task?.type === 'video' || task?.type === 'live_photo');
  }

  function buildMediaDownloadHeaders(candidates = []) {
    const referer = String(window.location?.href || '').trim();
    if (!referer) return undefined;
    const needsReferer = normalizeCandidates(candidates).some((url) => (
      /xhscdn|xiaohongshu|xhslink|sns-video/i.test(String(url || ''))
    ));
    return needsReferer ? [{ name: 'Referer', value: referer }] : undefined;
  }

  function buildNoteMediaAssets(note, queue, {
    status = '待下载',
    summary = null,
    refreshed = false,
    collectionRunId = '',
  } = {}) {
    const contentId = String(note?.contentId || note?.noteId || '').trim();
    if (!contentId || !Array.isArray(queue) || queue.length === 0) return [];

    const summaryById = new Map((summary?.details || []).map((item) => [item?.id, item]));
    const now = Date.now();

    return queue.map((task) => {
      const detail = summaryById.get(task.id) || null;
      const result = detail?.result || null;
      const isSuccess = Boolean(result?.success);
      const downloadStatus = status === '下载中'
        ? '下载中'
        : (isSuccess ? '已完成' : '失败');
      const quality = isSuccess
        ? String(result?.quality || (task.candidates?.length > 0 ? 'download' : 'thumb')).trim()
        : String(task.candidates?.length > 0 ? 'download' : 'thumb');
      const sourceUrl = String(result?.sourceUrl || task.candidates?.[0] || '').trim();
      const assetType = task.type === 'cover' ? 'image' : (task.type || 'image');
      const role = task.type === 'cover'
        ? 'cover'
        : (task.type === 'live_photo' ? 'live_photo' : 'body');
      const coverProvenance = role === 'cover'
        ? String(note?.coverProvenance || '').trim()
        : undefined;

      return {
        assetId: `media_${contentId}_${task.id}`,
        contentId,
        collectionRunId: String(note?.collectionRunId || collectionRunId || '').trim() || undefined,
        assetType,
        role,
        ordinal: task.ordinal,
        coverProvenance,
        quality,
        downloadStatus,
        lastResolvedAt: now,
        createdAt: now,
        noteId: note?.noteId || undefined,
        noteUrl: note?.url || undefined,
        noteTitle: note?.title || undefined,
        filename: task.filename || undefined,
        sourceUrl: sourceUrl || undefined,
        candidateUrls: Array.isArray(task.candidates) ? task.candidates : [],
        via: result?.via || 'queue',
        refreshed: Boolean(refreshed),
      };
    });
  }

  async function persistNoteMediaAssets(note, queue, options = {}) {
    const assets = buildNoteMediaAssets(note, queue, options);
    if (assets.length === 0) return;
    await mediaAssetStore.bulkUpsert(assets);
  }

  async function tryDownloadByBlobFallback(task) {
    const candidates = normalizeCandidates(task?.candidates || []);
    if (candidates.length === 0) return { success: false, reason: 'no_candidates' };
    const targetName = basenameFromPath(task?.filename || '');

    // 1. 优先通过页面上下文下载（MAIN world fetch，带完整登录态）
    if (window.location?.host?.includes('douyin.com')) {
      const pageResult = await tryPageContextDownload(candidates, targetName);
      if (pageResult.success) return pageResult;
    }

    const referer = window.location.href || 'https://www.xiaohongshu.com/';
    const isDouyin = candidates.some((url) => /douyin/i.test(String(url || '')));

    // 2. 降级到 content script 原生 fetch（尝试 omit / include 两种配置）
    // 2026-04-15：Douyin 路径移除 open-ended Range，避免触发 CORS preflight 或 CDN 拒绝
    for (let i = 0; i < candidates.length; i += 1) {
      const candidate = candidates[i];
      const configs = isDouyin
        ? [
            { credentials: 'include', referrer: referer, headers: { Referer: referer } },
            { credentials: 'omit', referrer: referer, headers: { Referer: referer } },
            { credentials: 'include', referrer: referer, headers: { Referer: referer, Range: 'bytes=0-5242880' } },
          ]
        : [
            { credentials: 'include', referrer: referer },
            { credentials: 'omit', referrer: referer },
          ];

      for (const config of configs) {
        try {
          const resp = await fetch(candidate, config);
          if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
          const blob = await resp.blob();
          if (Number(blob.size || 0) <= 0) throw new Error('empty_blob');
          downloadBlob(blob, targetName);
          return {
            success: true,
            sourceUrl: candidate,
            candidateIndex: i,
            quality: i === 0 ? 'HD' : 'SD',
            via: 'blob-fallback',
          };
        } catch {
          // try next config
        }
      }
    }

    // 3. 再降级到 XHR（Douyin 专用，某些场景 XHR 能绕过 fetch 的 CORS/CORB 限制）
    if (isDouyin) {
      for (let i = 0; i < candidates.length; i += 1) {
        const candidate = candidates[i];
        try {
          const blob = await new Promise((resolve, reject) => {
            const xhr = new XMLHttpRequest();
            xhr.open('GET', candidate, true);
            xhr.responseType = 'blob';
            xhr.setRequestHeader('Referer', referer);
            xhr.onload = () => {
              if (xhr.status >= 200 && xhr.status < 300 && xhr.response && xhr.response.size > 0) {
                resolve(xhr.response);
              } else {
                reject(new Error(`HTTP ${xhr.status}`));
              }
            };
            xhr.onerror = () => reject(new Error('XHR failed'));
            xhr.send();
          });
          downloadBlob(blob, targetName);
          return {
            success: true,
            sourceUrl: candidate,
            candidateIndex: i,
            quality: i === 0 ? 'HD' : 'SD',
            via: 'blob-fallback-xhr',
          };
        } catch {
          // try next candidate
        }
      }
    }

    return { success: false, reason: 'blob_fallback_failed' };
  }

  /**
   * 通过注入脚本的 MAIN world fetch 下载文件（绕过 CDN 鉴权）
   * 注入脚本使用 origFetch（页面原生 fetch），携带页面完整 cookie
   */
  async function tryPageContextDownload(candidates, targetName) {
    const REQ_EVENT = '__lgboom_page_download_req__';
    const RES_EVENT = '__lgboom_page_download_res__';

    const requestId = Date.now().toString(36) + Math.random().toString(36).slice(2);

    return new Promise((resolve) => {
      const timeout = setTimeout(() => {
        window.removeEventListener(RES_EVENT, handler);
        resolve({ success: false, reason: 'page_context_timeout' });
      }, 120000);

      function handler(e) {
        const detail = e.detail || {};
        if (detail.requestId !== requestId) return;
        clearTimeout(timeout);
        window.removeEventListener(RES_EVENT, handler);
        if (detail.ok) {
          resolve({
            success: true,
            sourceUrl: detail.url || candidates[0],
            candidateIndex: 0,
            quality: 'HD',
            via: 'page-context',
          });
        } else {
          resolve({ success: false, reason: detail.error || 'page_context_failed' });
        }
      }

      window.addEventListener(RES_EVENT, handler);
      window.dispatchEvent(new CustomEvent(REQ_EVENT, {
        detail: { urls: candidates, filename: targetName, requestId },
      }));
    });
  }

  async function downloadQueueViaBackground(queue, {
    onProgress = null,
    shouldStop = () => false,
    waitIfPaused = async () => {},
    conflictAction = 'overwrite',
  } = {}) {
    const summary = {
      total: queue.length,
      success: 0,
      failed: 0,
      hdCount: 0,
      sdCount: 0,
      details: [],
      zipped: false,
    };

    const details = new Array(queue.length);
    let nextIndex = 0;

    async function worker() {
      while (true) {
        await waitIfPaused();
        if (shouldStop()) break;

        const i = nextIndex;
        nextIndex += 1;
        if (i >= queue.length) break;

        const task = queue[i];
        onProgress?.({ current: i + 1, total: queue.length, task });

        let result = null;
        const allCandidates = normalizeCandidates(task.candidates || []);
        const remoteCandidates = allCandidates.filter((url) => /^https?:\/\//i.test(String(url || '')));
        try {
          if (remoteCandidates.length > 0) {
            const isDouyin = remoteCandidates.some((url) => /douyin/i.test(String(url || '')));
            // Douyin CDN 对 chrome.downloads.download 不友好（不支持 Referer、触发 SERVER_FORBIDDEN），
            // 直接走 blob 降级路径（页面上下文下载 + content script fetch），避免产生多余的失败下载记录
            if (isDouyin) {
              result = { success: false, error: 'skip_chrome_downloads_for_douyin' };
            } else {
              result = await sendToBackground(MSG.DOWNLOAD_MEDIA_FILE, {
                candidates: remoteCandidates,
                filename: task.filename,
                saveAs: false,
                conflictAction,
                headers: buildMediaDownloadHeaders(remoteCandidates),
              });
            }
          } else {
            result = { success: false, error: 'no_remote_candidates' };
          }
        } catch (err) {
          result = { success: false, error: String(err?.message || err) };
        }

        const shouldUseBlobFallback = task.type === 'video'
          || remoteCandidates.some((url) => /douyin|xhscdn|xiaohongshu|xhslink|sns-img|sns-video/i.test(String(url || '')));

        if (result?.success) {
          summary.success += 1;
          if (result.quality === 'HD') summary.hdCount += 1;
          else summary.sdCount += 1;
        } else if (shouldUseBlobFallback) {
          const fallbackResult = await tryDownloadByBlobFallback(task);
          if (fallbackResult?.success) {
            result = fallbackResult;
            summary.success += 1;
            if (fallbackResult.quality === 'HD') summary.hdCount += 1;
            else summary.sdCount += 1;
          } else {
            summary.failed += 1;
          }
        } else {
          summary.failed += 1;
        }

        details[i] = {
          ...task,
          result,
        };
      }
    }

    const concurrency = 3;
    const workers = Array.from({ length: concurrency }, () => worker());
    await Promise.all(workers);

    summary.details = details.slice(0, nextIndex);
    return summary;
  }

  async function fetchBinaryBlobViaBackground(task) {
    const remoteCandidates = normalizeCandidates(task?.candidates || [])
      .filter((url) => /^https?:\/\//i.test(String(url || '')));
    if (remoteCandidates.length === 0) {
      return { success: false, error: 'no_remote_candidates' };
    }

    if (!MSG?.FETCH_BINARY_AS_DATA_URL) {
      return { success: false, error: 'fetch_binary_action_missing' };
    }

    let result = null;
    try {
      result = await sendToBackground(MSG.FETCH_BINARY_AS_DATA_URL, {
        candidates: remoteCandidates,
      });
    } catch (err) {
      return { success: false, error: String(err?.message || err) };
    }

    if (!result?.success || !result?.dataUrl) {
      return {
        success: false,
        error: String(result?.error || 'binary_fetch_failed'),
      };
    }

    try {
      const parsed = parseDataUrl(result.dataUrl);
      if (Number(parsed.bytes?.length || 0) <= 0) {
        return { success: false, error: 'empty_blob' };
      }
      const candidateIndex = Number(result?.candidateIndex || 0);
      return {
        success: true,
        bytes: parsed.bytes,
        mimeType: parsed.mimeType,
        sourceUrl: String(result?.candidate || remoteCandidates[candidateIndex] || remoteCandidates[0] || '').trim(),
        candidateIndex,
        quality: candidateIndex === 0 ? 'HD' : 'SD',
        via: 'background-data-url',
      };
    } catch (err) {
      return { success: false, error: String(err?.message || err) };
    }
  }

  async function downloadQueueAsZip(note, queue, {
    onProgress = null,
    shouldStop = () => false,
    waitIfPaused = async () => {},
  } = {}) {
    const summary = {
      total: queue.length,
      success: 0,
      failed: 0,
      hdCount: 0,
      sdCount: 0,
      details: [],
      zipped: false,
      zipName: '',
    };

    const zip = new JSZip();
    const details = [];

    for (let i = 0; i < queue.length; i += 1) {
      await waitIfPaused();
      if (shouldStop()) break;

      const task = queue[i];
      onProgress?.({ current: i + 1, total: queue.length, task });

      const result = await fetchBinaryBlobViaBackground(task);
      if (result?.success && result?.bytes) {
        zip.file(basenameFromPath(task.filename), result.bytes);
        summary.success += 1;
        if (result.quality === 'HD') summary.hdCount += 1;
        else summary.sdCount += 1;
      } else {
        summary.failed += 1;
      }

      const { bytes, ...publicResult } = result || {};
      details.push({
        ...task,
        result: publicResult,
      });
    }

    summary.details = details;

    if (summary.success > 0) {
      summary.zipName = buildNoteMediaZipName(note);
      const zipBlob = await zip.generateAsync({ type: 'blob' });
      downloadBlob(zipBlob, summary.zipName);
      summary.zipped = true;
    }

    return summary;
  }

  function inferMediaDownloadStatus(summary) {
    if (!summary || summary.total === 0) return '无媒体';
    if (summary.failed === 0 && summary.success > 0) return '已完成';
    if (summary.success === 0 && summary.failed > 0) return '失败';
    if (summary.success > 0 && summary.failed > 0) return '部分失败';
    return '待下载';
  }

  async function persistNoteDownloadSummary(noteId, summary, extras = {}) {
    const now = Date.now();
    await noteStore.updateById(noteId, {
      mediaDownloadStatus: inferMediaDownloadStatus(summary),
      mediaDownloadedAt: now,
      mediaDownloadSummary: {
        total: summary.total || 0,
        success: summary.success || 0,
        failed: summary.failed || 0,
        hdCount: summary.hdCount || 0,
        sdCount: summary.sdCount || 0,
        updatedAt: now,
        ...extras,
      },
    });
  }

  function mergeDownloadSummaries(baseSummary, retrySummary) {
    const retryMap = new Map((retrySummary?.details || []).map((item) => [item.id, item]));
    const details = (baseSummary?.details || []).map((item) => {
      if (item?.result?.success) {
        return { ...item, retried: false };
      }
      const retried = retryMap.get(item.id);
      if (!retried) return { ...item, retried: false };
      return {
        ...item,
        candidates: retried.candidates || item.candidates,
        result: retried.result || item.result,
        retried: true,
        previousResult: item.result,
      };
    });

    let success = 0;
    let failed = 0;
    let hdCount = 0;
    let sdCount = 0;
    details.forEach((item) => {
      if (item?.result?.success) {
        success += 1;
        if (item.result.quality === 'HD') hdCount += 1;
        else sdCount += 1;
      } else {
        failed += 1;
      }
    });
    return {
      total: Number(baseSummary?.total || details.length),
      success,
      failed,
      hdCount,
      sdCount,
      details,
      zipped: Boolean(baseSummary?.zipped),
      zipName: String(baseSummary?.zipName || '').trim(),
    };
  }

  function collectRetryQueue(baseSummary, refreshedQueue) {
    const refreshedMap = new Map((refreshedQueue || []).map((item) => [item.id, item]));
    return (baseSummary?.details || [])
      .filter((item) => !item?.result?.success)
      .map((item) => {
        const refreshed = refreshedMap.get(item.id);
        return {
          ...item,
          candidates: refreshed?.candidates || item.candidates,
        };
      });
  }

  async function refreshNoteMediaSnapshot(note, { timeoutMs = 24000 } = {}) {
    if (note.platform === 'douyin') {
      const douyinRuntime = await loadDouyinRuntime();
      const currentDouyinId = douyinRuntime.extractDouyinContentId(window.location.href);
      const currentNoteId = currentDouyinId ? `dy_${currentDouyinId}` : '';
      if (currentNoteId && note.noteId && currentNoteId === note.noteId) {
        try {
          const collected = await douyinRuntime.collectDouyinVideo();
          if (collected?.ok && collected?.data) return collected.data;
        } catch {
          // fallback to existing logic
        }
      }
      try {
        const refreshed = await douyinRuntime.refreshDouyinNoteMediaById(note);
        if (refreshed) return refreshed;
      } catch {
        // ignore and fallback to null
      }
      return null;
    }

    if (!note?.url) return null;

    const currentNoteId = extractNoteId(window.location.href);
    if (currentNoteId && note.noteId && currentNoteId === note.noteId) {
      try {
        return await collectNote(window);
      } catch {
        // fallback to iframe method
      }
    }

    const targetUrl = String(note.url || '').trim();
    if (!targetUrl) return null;

    return new Promise((resolve) => {
      let settled = false;
      let timeout = null;
      const iframe = document.createElement('iframe');
      iframe.style.cssText = 'position:fixed;left:-9999px;top:-9999px;width:1px;height:1px;opacity:0;pointer-events:none;';

      const cleanup = () => {
        if (timeout) clearTimeout(timeout);
        iframe.remove();
      };

      const finish = (data) => {
        if (settled) return;
        settled = true;
        cleanup();
        resolve(data || null);
      };

      timeout = setTimeout(() => finish(null), timeoutMs);

      iframe.onload = async () => {
        try {
          const refreshed = await collectNote(iframe.contentWindow);
          finish(refreshed);
        } catch {
          finish(null);
        }
      };

      iframe.onerror = () => finish(null);
      document.body.appendChild(iframe);
      iframe.src = targetUrl;
    });
  }

  return {
    async downloadNoteMediaFromRecord(note, options = {}) {
      const noteId = note?.noteId || '';
      let workingNote = note;
      let queue = buildNoteMediaDownloadQueue(workingNote, options);
      let refreshed = false;

      const shouldAttemptInitialRefresh = options.retryOnRefresh !== false
        && queue.length === 0
        && Boolean(workingNote?.platform === 'douyin' ? (workingNote?.platformContentId || workingNote?.noteId) : workingNote?.url);

      if (shouldAttemptInitialRefresh) {
        const refreshedNote = await refreshNoteMediaSnapshot(workingNote);
        if (refreshedNote) {
          workingNote = refreshedNote;
          queue = buildNoteMediaDownloadQueue(workingNote, options);
          refreshed = queue.length > 0;
          if (noteId) {
            await noteStore.updateById(noteId, refreshedNote);
          }
        }
      }

      if (queue.length === 0) {
        if (noteId) {
          await persistNoteDownloadSummary(noteId, { total: 0, success: 0, failed: 0, hdCount: 0, sdCount: 0 }, {
            refreshed,
          });
        }
        return { total: 0, success: 0, failed: 0, hdCount: 0, sdCount: 0, details: [], refreshed };
      }

      await persistNoteMediaAssets(workingNote, queue, {
        status: '下载中',
        refreshed,
        collectionRunId: options.collectionRunId,
      });

      if (noteId) {
        await noteStore.updateById(noteId, {
          mediaDownloadStatus: '下载中',
          mediaDownloadSummary: {
            total: queue.length,
            success: 0,
            failed: 0,
            hdCount: 0,
            sdCount: 0,
            updatedAt: Date.now(),
          },
        });
      }

      const isZipDownload = shouldPackageAsZip(workingNote, queue, options);
      const baseSummary = isZipDownload
        ? await downloadQueueAsZip(workingNote, queue, options)
        : await downloadQueueViaBackground(queue, options);
      let finalSummary = baseSummary;
      let retryRefreshed = false;
      let finalAssetNote = workingNote;
      let finalAssetQueue = queue;

      const shouldRetry = options.retryOnRefresh !== false
        && noteId
        && baseSummary.failed > 0
        && !isZipDownload
        && Boolean(workingNote?.platform === 'douyin'
          ? (workingNote?.platformContentId || workingNote?.noteId)
          : (workingNote?.url || workingNote?.noteId));
      if (shouldRetry) {
        const refreshedNote = await refreshNoteMediaSnapshot(workingNote);
        if (refreshedNote) {
          const refreshedQueue = buildNoteMediaDownloadQueue(refreshedNote, options);
          await persistNoteMediaAssets(refreshedNote, refreshedQueue, {
            status: '下载中',
            refreshed: true,
            collectionRunId: options.collectionRunId,
          });
          const retryQueue = collectRetryQueue(baseSummary, refreshedQueue);
          if (retryQueue.length > 0) {
            const retrySummary = await downloadQueueViaBackground(retryQueue, options);
            finalSummary = mergeDownloadSummaries(baseSummary, retrySummary);
            retryRefreshed = true;
            finalAssetNote = refreshedNote;
            finalAssetQueue = refreshedQueue;
          }
          await noteStore.updateById(noteId, refreshedNote);
        }
      }

      await persistNoteMediaAssets(finalAssetNote, finalAssetQueue, {
        summary: finalSummary,
        refreshed: refreshed || retryRefreshed,
        collectionRunId: options.collectionRunId,
      });

      if (noteId) {
        await persistNoteDownloadSummary(noteId, finalSummary, {
          refreshed: refreshed || retryRefreshed,
          zipped: Boolean(finalSummary?.zipped),
          zipName: String(finalSummary?.zipName || '').trim(),
        });
      }

      return {
        ...finalSummary,
        refreshed: refreshed || retryRefreshed,
      };
    },
  };
}
