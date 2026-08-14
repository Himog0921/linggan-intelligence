function normalizeString(value) {
  return String(value || '').trim();
}

function normalizeArray(value) {
  if (Array.isArray(value)) return value;
  if (value == null || value === '') return [];
  return [value];
}

function normalizeNumber(value, fallback = 0) {
  const num = Number(value);
  return Number.isFinite(num) ? num : fallback;
}

function getPlatformPrefix(platform = '') {
  return normalizeString(platform) === 'douyin' ? 'dy' : 'xhs';
}

function inferPlatform(record = {}) {
  const explicit = normalizeString(record.platform).toLowerCase();
  if (explicit === 'xhs' || explicit === 'douyin') return explicit;

  const contentId = normalizeString(record.contentId || record.noteId || record.authorEntityId || '');
  if (contentId.startsWith('dy_') || /^douyin_/i.test(contentId)) return 'douyin';
  if (contentId.startsWith('xhs_')) return 'xhs';

  const url = normalizeString(record.url || record.canonicalUrl || record.noteUrl || record.profileUrl || record.rawUrl || '');
  if (/douyin\.com/i.test(url)) return 'douyin';
  if (/(xiaohongshu\.com|xhslink\.com)/i.test(url)) return 'xhs';

  return 'xhs';
}

function stripKnownPrefix(value = '') {
  const text = normalizeString(value);
  return text.replace(/^(xhs_|dy_|douyin_)/, '');
}

function inferPlatformContentId(record = {}, platform = inferPlatform(record)) {
  const explicit = normalizeString(record.platformContentId);
  if (explicit) return explicit;

  if (platform === 'douyin') {
    const fromContentId = stripKnownPrefix(record.contentId);
    if (fromContentId) return fromContentId;
    const fromNoteId = stripKnownPrefix(record.noteId);
    if (fromNoteId) return fromNoteId;
    return normalizeString(record.videoId);
  }

  const fromNoteId = normalizeString(record.noteId);
  if (fromNoteId && !/^dy_/.test(fromNoteId)) return fromNoteId;
  const fromContentId = stripKnownPrefix(record.contentId);
  if (fromContentId) return fromContentId;
  return normalizeString(record.id);
}

function buildContentId(platform, platformContentId) {
  const id = normalizeString(platformContentId);
  if (!id) return '';
  return `${getPlatformPrefix(platform)}_${id}`;
}

function inferNoteId(record = {}, platform = inferPlatform(record), platformContentId = '', contentId = '') {
  const explicit = normalizeString(record.noteId);
  if (explicit) return explicit;
  if (platform === 'douyin') return normalizeString(contentId) || buildContentId(platform, platformContentId);
  return normalizeString(platformContentId) || stripKnownPrefix(contentId);
}

function ensureRawFields(record = {}) {
  return {
    collectorVersion: normalizeString(record.collectorVersion),
    rawSource: normalizeString(record.rawSource),
    rawUrl: normalizeString(record.rawUrl),
    rawShareText: normalizeString(record.rawShareText),
    rawDomText: normalizeString(record.rawDomText),
    rawPayload: record.rawPayload ?? null,
  };
}

function inferSourceTier(record = {}) {
  const explicit = normalizeString(record.sourceTier);
  if (explicit) return explicit;

  const rawSource = normalizeString(record.rawSource || record.dataSource).toLowerCase();
  if (!rawSource) return '';
  if (/seed|search_dom_result|detail_api_fallback/.test(rawSource)) return 'seed';
  if (/dom/.test(rawSource)) return 'dom';
  if (/render|router|cache|mixed|\+/.test(rawSource)) return 'mixed';
  if (/api/.test(rawSource)) return 'api';
  return '';
}

function ensureQualityFields(record = {}) {
  return {
    dataQuality: normalizeString(record.dataQuality || 'full') || 'full',
    qualityReason: normalizeString(record.qualityReason),
    sourceTier: inferSourceTier(record),
  };
}

export function normalizeNoteRecord(record = {}) {
  const platform = inferPlatform(record);
  const platformContentId = inferPlatformContentId(record, platform);
  const contentId = normalizeString(record.contentId) || buildContentId(platform, platformContentId);
  const noteId = inferNoteId(record, platform, platformContentId, contentId);
  const url = normalizeString(record.url);
  const canonicalUrl = normalizeString(record.canonicalUrl) || url;
  const bodyText = normalizeString(record.bodyText || record.content || record.desc || record.title);
  const publishedAtText = normalizeString(record.publishedAtText || record.releaseDate || record.time);
  const collectedAt = normalizeNumber(record.collectedAt || record.updatedAt || record.createdAt, Date.now());
  const batchSelectionMode = normalizeString(record.batchSelectionMode);
  const searchKeyword = normalizeString(record.searchKeyword);
  const searchPageUrl = normalizeString(record.searchPageUrl);

  return {
    ...record,
    noteId,
    platform,
    platformContentId,
    contentId,
    canonicalUrl,
    bodyText,
    hashtags: normalizeArray(record.hashtags),
    topicIds: normalizeArray(record.topicIds),
    atUserList: normalizeArray(record.atUserList),
    imageCandidates: normalizeArray(record.imageCandidates),
    images: normalizeArray(record.images),
    videoStreams: normalizeArray(record.videoStreams),
    livePhotoStreams: normalizeArray(record.livePhotoStreams),
    authorEntityId: normalizeString(record.authorEntityId) || (normalizeString(record.authorId) ? `${getPlatformPrefix(platform)}_${normalizeString(record.authorId)}` : ''),
    cover: normalizeString(record.cover || record.coverImg || normalizeArray(record.images)[0] || ''),
    coverImg: normalizeString(record.coverImg || record.cover || normalizeArray(record.images)[0] || ''),
    publishedAt: normalizeNumber(record.publishedAt, 0),
    publishedAtText,
    collectedAt,
    updatedAt: normalizeNumber(record.updatedAt || collectedAt, collectedAt),
    createdAt: normalizeNumber(record.createdAt || collectedAt, collectedAt),
    collectionRunId: normalizeString(record.collectionRunId),
    batchSelectionMode: batchSelectionMode || undefined,
    batchRank: batchSelectionMode ? normalizeNumber(record.batchRank, 0) : undefined,
    batchLikesSnapshot: batchSelectionMode ? normalizeNumber(record.batchLikesSnapshot, 0) : undefined,
    searchKeyword: searchKeyword || undefined,
    searchPageUrl: searchPageUrl || undefined,
    dataSource: normalizeString(record.dataSource || record.rawSource),
    ...ensureQualityFields(record),
    ...ensureRawFields(record),
  };
}

export function normalizeCommentRecord(record = {}) {
  const platform = inferPlatform(record);
  const rawNoteId = normalizeString(record.noteId);
  const contentId = normalizeString(record.contentId)
    || (rawNoteId && /^(xhs_|dy_)/.test(rawNoteId) ? rawNoteId : buildContentId(platform, rawNoteId));
  const commentId = normalizeString(record.commentId || record.platformCommentId);
  const parentCommentId = normalizeString(record.parentCommentId);
  const level = normalizeNumber(record.level, parentCommentId ? 2 : 1);
  const rootCommentId = normalizeString(record.rootCommentId || parentCommentId || commentId);
  const publishedAt = normalizeNumber(record.publishedAt || record.createdAt, 0);
  const collectedAt = normalizeNumber(record.collectedAt || record.createdAt, Date.now());
  const noteId = rawNoteId || (platform === 'douyin' ? contentId : inferPlatformContentId({ contentId }, platform));
  const searchKeyword = normalizeString(record.searchKeyword);
  const searchPageUrl = normalizeString(record.searchPageUrl);

  return {
    ...record,
    platform,
    contentId,
    noteId,
    commentId,
    platformCommentId: normalizeString(record.platformCommentId || commentId),
    commentEntityId: normalizeString(record.commentEntityId) || (contentId && commentId ? `${platform}_${contentId}_${commentId}` : ''),
    authorEntityId: normalizeString(record.authorEntityId) || (normalizeString(record.authorId) ? `${platform}_${normalizeString(record.authorId)}` : ''),
    parentCommentId,
    rootCommentId,
    level,
    replyToCommentId: normalizeString(record.replyToCommentId || (level > 1 ? parentCommentId : '')),
    replyToUserName: normalizeString(record.replyToUserName),
    location: normalizeString(record.location || record.ipLocation),
    ipLocation: normalizeString(record.ipLocation || record.location),
    publishedAt,
    publishedAtText: normalizeString(record.publishedAtText || record.time),
    collectedAt,
    createdAt: normalizeNumber(record.createdAt || collectedAt, collectedAt),
    collectionRunId: normalizeString(record.collectionRunId),
    sortMode: normalizeString(record.sortMode || 'unknown'),
    searchKeyword: searchKeyword || undefined,
    searchPageUrl: searchPageUrl || undefined,
    ...ensureQualityFields(record),
    ...ensureRawFields(record),
  };
}

export function normalizeAuthorRecord(record = {}) {
  const platform = inferPlatform(record);
  const userId = normalizeString(record.userId || record.platformAuthorId);
  const handle = normalizeString(record.handle || record.redId || record.douyinId);
  const collectedAt = normalizeNumber(record.collectedAt || record.createdAt, Date.now());

  return {
    ...record,
    platform,
    userId,
    platformAuthorId: normalizeString(record.platformAuthorId || userId),
    authorEntityId: normalizeString(record.authorEntityId) || (userId ? `${getPlatformPrefix(platform)}_${userId}` : ''),
    handle,
    redId: platform === 'xhs' ? normalizeString(record.redId || handle) : normalizeString(record.redId),
    douyinId: platform === 'douyin' ? normalizeString(record.douyinId || handle) : normalizeString(record.douyinId),
    keywords: normalizeArray(record.keywords),
    collectedAt,
    createdAt: normalizeNumber(record.createdAt || collectedAt, collectedAt),
    collectionRunId: normalizeString(record.collectionRunId),
    profileUrl: normalizeString(record.profileUrl || record.url),
    ipLocation: normalizeString(record.ipLocation || record.location),
    location: normalizeString(record.location || record.ipLocation),
    ...ensureQualityFields(record),
    ...ensureRawFields(record),
  };
}

export function normalizeMediaAssetRecord(record = {}) {
  const platform = inferPlatform(record);
  const contentId = normalizeString(record.contentId)
    || (normalizeString(record.noteId) ? (normalizeString(record.noteId).startsWith('dy_') || normalizeString(record.noteId).startsWith('xhs_')
      ? normalizeString(record.noteId)
      : buildContentId(platform, normalizeString(record.noteId))) : '');
  const createdAt = normalizeNumber(record.createdAt || record.lastResolvedAt, Date.now());

  return {
    ...record,
    contentId,
    collectionRunId: normalizeString(record.collectionRunId),
    assetType: normalizeString(record.assetType || 'image'),
    role: normalizeString(record.role || 'body'),
    quality: normalizeString(record.quality || 'unknown'),
    downloadStatus: normalizeString(record.downloadStatus || '未知'),
    candidateUrls: normalizeArray(record.candidateUrls),
    createdAt,
    lastResolvedAt: normalizeNumber(record.lastResolvedAt || createdAt, createdAt),
  };
}
