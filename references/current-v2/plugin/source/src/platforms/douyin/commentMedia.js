import { MSG as RUNTIME_MSG } from '../../shared/constants.js';
import { sendToBackground as sendRuntimeToBackground } from '../../shared/messaging.js';
import { normalizeRemoteCandidates, normalizeRemoteUrl, looksLikeImageUrl } from './commentApi.js';

export function extractImageCandidatesFromComment(comment = {}) {
  const commentImages = Array.isArray(comment?.image_list) ? comment.image_list : [];
  if (commentImages.length === 0) return [];

  return commentImages
    .map((image) => normalizeRemoteCandidates([
      ...(image?.origin_url?.url_list || []),
      ...(image?.download_url?.url_list || []),
      ...(image?.medium_url?.url_list || []),
      ...(image?.crop_url?.url_list || []),
      ...(image?.thumb_url?.url_list || []),
    ]))
    .filter((candidates) => candidates.length > 0);
}

function isSyntheticCommentId(commentRecord = {}) {
  const commentId = String(commentRecord?.commentId || '').trim();
  const qualityReason = String(commentRecord?.qualityReason || '').trim();
  return qualityReason === 'synthetic_comment_id' || /^dy_fb_/i.test(commentId);
}

export function buildCommentImageRecordKey(commentRecord = {}, rawComment = {}) {
  const commentId = String(
    commentRecord?.commentId
    || rawComment?.cid
    || rawComment?.comment_id
    || rawComment?.id
    || ''
  ).trim();
  if (commentId && !isSyntheticCommentId(commentRecord)) {
    return `comment:${commentId}`;
  }

  const contentId = String(commentRecord?.contentId || commentRecord?.noteId || '').trim();
  const authorId = String(
    commentRecord?.authorId
    || rawComment?.user?.uid
    || rawComment?.user?.user_id
    || ''
  ).trim();
  const publishedAt = String(
    commentRecord?.publishedAt
    || rawComment?.create_time
    || rawComment?.createTime
    || ''
  ).trim();
  const text = String(
    commentRecord?.text
    || rawComment?.text
    || rawComment?.content
    || ''
  ).trim();
  const imageSignature = extractImageCandidatesFromComment(rawComment)
    .map((candidates) => normalizeRemoteCandidates(candidates).join('|'))
    .filter(Boolean)
    .join('||');

  const fallbackParts = [contentId, authorId, publishedAt, text, imageSignature]
    .map((value) => String(value || '').trim())
    .filter(Boolean);
  if (fallbackParts.length > 0) {
    return `fallback:${fallbackParts.join('::')}`;
  }
  return commentId ? `comment:${commentId}` : '';
}

function listCommentImageEntries(commentRecords = [], rawComments = []) {
  const entries = [];
  const seenAssetKeys = new Set();
  const recordAssetIndexes = new Map();

  rawComments.forEach((comment, index) => {
    const mapped = commentRecords[index];
    if (!mapped?.commentId || !mapped?.contentId) return;

    const recordKey = buildCommentImageRecordKey(mapped, comment);
    if (!recordKey) return;

    let assetIndex = recordAssetIndexes.get(recordKey) || 0;
    const imageCandidates = extractImageCandidatesFromComment(comment);
    imageCandidates.forEach((candidates) => {
      const candidateUrls = normalizeRemoteCandidates(candidates);
      if (candidateUrls.length === 0) return;
      const assetKey = `${recordKey}::${candidateUrls.join('|')}`;
      if (seenAssetKeys.has(assetKey)) return;
      seenAssetKeys.add(assetKey);
      assetIndex += 1;
      entries.push({
        mapped,
        assetIndex,
        candidateUrls,
      });
    });
    recordAssetIndexes.set(recordKey, assetIndex);
  });

  return entries;
}

export function countCommentImageAssets(commentRecords = [], rawComments = []) {
  return listCommentImageEntries(commentRecords, rawComments).length;
}

export function buildCommentImageAssets(commentRecords = [], rawComments = [], { collectionRunId = '' } = {}) {
  return listCommentImageEntries(commentRecords, rawComments).map(({ mapped, assetIndex, candidateUrls }) => ({
    assetId: `dy_comment_image_${mapped.contentId}_${mapped.commentId}_${assetIndex}`,
    contentId: mapped.contentId,
    commentEntityId: mapped.commentEntityId,
    commentId: mapped.commentId,
    assetType: 'comment_image',
    role: assetIndex === 1 ? 'primary' : 'fallback',
    url: candidateUrls[0] || '',
    candidateUrls,
    quality: 'unknown',
    collectionRunId: String(collectionRunId || '').trim() || undefined,
    downloadStatus: '待下载',
    lastResolvedAt: Date.now(),
    createdAt: Date.now(),
  }));
}

export async function downloadBlob(blob, filename) {
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = filename;
  anchor.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

export async function fetchImageBlob(candidates = []) {
  const urls = normalizeRemoteCandidates(candidates);
  const IMAGE_TYPES = /^image\//i;

  for (let i = 0; i < urls.length; i += 1) {
    const candidate = urls[i];
    try {
      let response = await fetch(candidate, { mode: 'cors', credentials: 'include' });
      if (!response.ok) {
        response = await fetch(candidate, { mode: 'no-cors', credentials: 'include' });
      }
      const blob = await response.blob();
      const hasData = Number(blob?.size || 0) > 0;
      if (!hasData) continue;
      // Skip non-image blobs (e.g. HTML error pages, opaque responses)
      if (blob.type && !IMAGE_TYPES.test(blob.type)) continue;
      return {
        success: true,
        blob,
        candidate,
        candidateIndex: i,
      };
    } catch {
      // try next candidate
    }
  }

  try {
    const fallback = await sendRuntimeToBackground(RUNTIME_MSG.FETCH_BINARY_AS_DATA_URL, { candidates: urls });
    if (fallback?.success && fallback?.dataUrl) {
      const blob = await fetch(fallback.dataUrl).then((resp) => resp.blob());
      if (blob && Number(blob.size || 0) > 0) {
        return {
          success: true,
          blob,
          candidate: fallback.candidate || urls[0] || '',
          candidateIndex: Number(fallback.candidateIndex || 0) || 0,
        };
      }
    }
  } catch {
    // ignore background fallback errors
  }

  return { success: false };
}

export function inferImageExt(url = '', fallback = 'jpg') {
  const clean = String(url || '').split('?')[0].split('#')[0];
  const ext = clean.match(/\.([a-z0-9]{2,5})$/i)?.[1];
  return (ext || fallback).toLowerCase();
}
