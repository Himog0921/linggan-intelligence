function normalizeText(value = '') {
  return String(value || '').trim();
}

function normalizeArray(value) {
  if (Array.isArray(value)) return value;
  if (value == null || value === '') return [];
  return [value];
}

function normalizeCodePart(value = '') {
  return normalizeText(value)
    .toLowerCase()
    .replace(/[\s:/\\|?#[\]{}()'"`]+/g, '')
    .replace(/[^\p{L}\p{N}._-]+/gu, '');
}

function normalizeNumber(value) {
  if (typeof value === 'number' && Number.isFinite(value)) return value;
  if (typeof value === 'string' && value.trim()) {
    const text = value.trim().toLowerCase().replace(/,/g, '');
    const match = text.match(/\d+(?:\.\d+)?/);
    if (!match) return null;
    const parsed = Number(match[0]);
    if (!Number.isFinite(parsed)) return null;
    const suffix = text.slice((match.index || 0) + match[0].length).trim();
    const multiplier = suffix.startsWith('万') || suffix.startsWith('w')
      ? 10000
      : suffix.startsWith('千') || suffix.startsWith('k')
        ? 1000
        : 1;
    return Math.round(parsed * multiplier);
  }
  return null;
}

function normalizeIso(value) {
  if (value == null || value === '') return '';
  if (typeof value === 'number' && Number.isFinite(value)) {
    const ms = value < 1000000000000 ? value * 1000 : value;
    const date = new Date(ms);
    return Number.isFinite(date.getTime()) ? date.toISOString() : '';
  }
  const date = new Date(value);
  return Number.isFinite(date.getTime()) ? date.toISOString() : '';
}

function inferPlatform(record = {}) {
  const explicit = normalizeText(record.platform).toLowerCase();
  if (explicit === 'xhs' || explicit === 'douyin') return explicit;
  const url = normalizeText(record.url || record.canonicalUrl || record.rawUrl || record.noteUrl);
  if (/douyin\.com/i.test(url)) return 'douyin';
  return 'xhs';
}

function inferPlatformContentId(record = {}) {
  const explicit = normalizeText(record.platformContentId);
  if (explicit) return explicit.replace(/^(xhs_|dy_|douyin_)/, '');
  const noteId = normalizeText(record.noteId || record.contentId || record.videoId || record.awemeId || record.id);
  return noteId.replace(/^(xhs_|dy_|douyin_)/, '');
}

function inferContentType(record = {}) {
  const explicit = normalizeText(record.contentType || record.type).toLowerCase();
  if (explicit === 'video' || explicit === 'image_text' || explicit === 'image' || explicit === 'note') {
    return explicit === 'image' ? 'image_text' : explicit;
  }
  if (
    normalizeText(record.videoUrl || record.video || record.videoPlayUrl || record.videoDownloadUrl)
    || normalizeArray(record.videoStreams).length > 0
  ) {
    return 'video';
  }
  return 'image_text';
}

function firstNumber(record = {}, fields = []) {
  for (const field of fields) {
    const value = normalizeNumber(record[field]);
    if (value !== null) return value;
  }
  return null;
}

function firstIso(record = {}, fields = []) {
  for (const field of fields) {
    const value = normalizeIso(record[field]);
    if (value) return value;
  }
  return '';
}

function firstText(record = {}, fields = []) {
  for (const field of fields) {
    const value = normalizeText(record[field]);
    if (value) return value;
  }
  return '';
}

function firstMediaUrl(record = {}, fields = []) {
  for (const field of fields) {
    const value = record[field];
    if (typeof value !== 'string') continue;
    const normalized = value.trim();
    if (normalized && normalized !== '[object Object]') return normalized;
  }
  return '';
}

function collectMediaUrls(value, output = []) {
  if (typeof value === 'string') {
    const normalized = value.trim();
    if (normalized && normalized !== '[object Object]') output.push(normalized);
    return output;
  }
  if (Array.isArray(value)) {
    for (const item of value) collectMediaUrls(item, output);
    return output;
  }
  if (!value || typeof value !== 'object') return output;
  for (const key of ['originUrl', 'urlDefault', 'downloadUrl', 'url', 'src', 'href']) {
    collectMediaUrls(value[key], output);
  }
  return output;
}

function buildRawMediaEvidence(record = {}) {
  const existingRawData = record.rawData;
  if (existingRawData && typeof existingRawData === 'object' && !Array.isArray(existingRawData)) {
    return {
      ...existingRawData,
      ...(existingRawData.imageCandidates === undefined && record.imageCandidates !== undefined
        ? { imageCandidates: record.imageCandidates }
        : {}),
    };
  }

  const { rawData: _rawData, ...rawRecord } = record;
  return rawRecord;
}

function canonicalImageUrls(record = {}) {
  const urls = [];
  for (const field of [
    'imageUrls',
    'images',
    'imageList',
    'coverImages',
    'thumbnails',
  ]) {
    const value = record[field];
    // 数组最外层表达图片槽位；单个槽位可能带有多个 URL，只取首个可用来源。
    const slots = Array.isArray(value) ? value : [value];
    for (const slot of slots) {
      const [url] = collectMediaUrls(slot);
      if (url) urls.push(url);
    }
  }

  // `imageCandidates` 是来源回退信息，不是另一套图片列表。只有采集记录没有
  // 明确图片列表时才作为补洞：嵌套数组表示多个图片槽位；扁平数组则是一张图
  // 的主、备地址。两种形态都不能把备用 URL 展开成虚假的 image:1。
  if (urls.length === 0) {
    const candidates = record.imageCandidates;
    const entries = Array.isArray(candidates) ? candidates : [candidates];
    const groups = entries.some(Array.isArray) ? entries : [entries];
    for (const group of groups) {
      const [url] = collectMediaUrls(group);
      if (url) urls.push(url);
    }
  }
  return [...new Set(urls)];
}

function normalizeKeyword(value = '') {
  return normalizeText(value).replace(/^#+/, '').trim();
}

function extractKeywords(record = {}) {
  const values = [
    ...normalizeArray(record.keywords),
    ...normalizeArray(record.hashtags),
    ...normalizeArray(record.tags),
    ...normalizeArray(record.topics),
    ...normalizeArray(record.searchKeywords),
    normalizeText(record.searchKeyword),
  ];
  return [...new Set(values.map(normalizeKeyword).filter(Boolean))];
}

function extractMediaUnderstanding(record = {}) {
  const transcript = firstText(record, ['transcript', 'videoTranscript', 'asrText', 'speechText']);
  const visualSummary = firstText(record, ['visualSummary', 'videoVisualSummary', 'frameSummary', 'imageDescription']);
  const ocrText = firstText(record, ['ocrText', 'coverOcrText', 'imageOcrText', 'doubaoOcrText']);
  if (!transcript && !visualSummary && !ocrText) return undefined;
  return {
    ...(transcript ? { transcript } : {}),
    ...(visualSummary ? { visualSummary } : {}),
    ...(ocrText ? { ocrText } : {}),
  };
}

function buildContentCode({ platform, contentType, platformContentId }) {
  const normalizedPlatform = normalizeCodePart(platform);
  const normalizedType = normalizeCodePart(contentType);
  const normalizedId = normalizeCodePart(platformContentId);
  if (!normalizedPlatform || !normalizedType || !normalizedId) return '';
  return `cw-content:global:${normalizedPlatform}:${normalizedType}:${normalizedId}`;
}

function buildAuthorCode({ platform, authorId }) {
  const normalizedPlatform = normalizeCodePart(platform);
  const normalizedId = normalizeCodePart(authorId);
  if (!normalizedPlatform || !normalizedId) return '';
  return `cw-author:global:${normalizedPlatform}:${normalizedId}`;
}

export function enrichNoteWithDataFoundationPayload(record = {}, context = {}) {
  const platform = inferPlatform(record);
  const platformContentId = inferPlatformContentId(record);
  const contentType = inferContentType(record);
  // 采集器内部可以保留平台原始字段；跨端协议只传递三个规范媒体源字段。
  // 后端据此写入唯一媒体账本并异步物化，不由插件另行上传封面或视频。
  const coverUrl = firstMediaUrl(record, [
    'sourceCoverUrl',
    'originalCoverUrl',
    'coverUrl',
    'coverImage',
    'cover',
    'coverImg',
    'thumbnailUrl',
    'thumbnail',
  ]);
  const videoUrl = firstMediaUrl(record, [
    'videoUrl',
    'video',
    'videoPlayUrl',
    'videoDownloadUrl',
    'playUrl',
  ]);
  const imageUrls = canonicalImageUrls(record);
  const rawData = buildRawMediaEvidence(record);
  const authorId = firstText(record, ['authorId', 'authorPlatformId', 'userId']);
  const authorFans = firstNumber(record, [
    'authorFans',
    'fans',
    'fanCount',
    'followerCount',
    'followers',
    'authorFollowerCount',
  ]);
  const authorFansCollectedAt = firstIso(record, [
    'authorFansCollectedAt',
    'fansCollectedAt',
    'authorCollectedAt',
    'collectedAt',
    'updatedAt',
  ]);
  const keywords = extractKeywords(record);
  const mediaUnderstanding = extractMediaUnderstanding(record);
  const taskId = normalizeText(context.taskId);
  const recordId = normalizeText(context.externalRecordId || platformContentId || record.noteId || record.url);
  const pluginRunId = normalizeText(context.pluginRunId || record.collectionRunId);
  const sourceRun = taskId && recordId ? {
    source: normalizeText(context.source) || 'plugin_task_delta',
    taskId,
    recordId,
  } : undefined;
  const standardContentCode = buildContentCode({ platform, contentType, platformContentId });
  const standardAuthorCode = buildAuthorCode({ platform, authorId });
  // 采集器原始字段可以保留在 rawData 供排障，但跨端的业务协议不能继续把同一
  // 媒体作为多组同义字段发送；否则后端无法判断哪一项是唯一事实来源。
  const {
    rawData: _rawData,
    sourceCoverUrl: _sourceCoverUrl,
    source_cover_url: _sourceCoverUrlSnake,
    originalCoverUrl: _originalCoverUrl,
    coverUrl: _coverUrl,
    cover_url: _coverUrlSnake,
    coverImage: _coverImage,
    cover: _cover,
    coverImg: _coverImg,
    thumbnailUrl: _thumbnailUrl,
    thumbnail: _thumbnail,
    thumbUrl: _thumbUrl,
    image: _image,
    imageUrl: _imageUrl,
    images: _images,
    imageUrls: _imageUrls,
    image_urls: _imageUrlsSnake,
    imageList: _imageList,
    image_list: _imageListSnake,
    imageCandidates: _imageCandidates,
    coverImages: _coverImages,
    thumbnails: _thumbnails,
    videoUrl: _videoUrl,
    video_url: _videoUrlSnake,
    video: _video,
    videoPlayUrl: _videoPlayUrl,
    video_play_url: _videoPlayUrlSnake,
    videoDownloadUrl: _videoDownloadUrl,
    video_download_url: _videoDownloadUrlSnake,
    playUrl: _playUrl,
    play_url: _playUrlSnake,
    playAddr: _playAddr,
    play_addr: _playAddrSnake,
    downloadUrl: _downloadUrl,
    download_url: _downloadUrlSnake,
    videoStreams: _videoStreams,
    ...transportRecord
  } = record;

  return {
    ...transportRecord,
    rawData,
    platform,
    platformContentId: platformContentId || record.platformContentId,
    contentType,
    ...(coverUrl ? { coverUrl } : {}),
    ...(imageUrls.length > 0 ? { imageUrls } : {}),
    ...(videoUrl ? { videoUrl } : {}),
    ...(standardContentCode ? { standardContentCode } : {}),
    ...(standardAuthorCode ? { standardAuthorCode } : {}),
    ...(keywords.length > 0 ? { keywords } : {}),
    ...(authorFans !== null ? { authorFans } : {}),
    ...(authorFansCollectedAt ? { authorFansCollectedAt } : {}),
    ...(mediaUnderstanding ? { mediaUnderstanding } : {}),
    ...(sourceRun ? { sourceRun } : {}),
    dataFoundation: {
      schemaVersion: 'data-foundation-plugin-v1',
      ...(standardContentCode ? { standardContentCode } : {}),
      ...(standardAuthorCode ? { standardAuthorCode } : {}),
      ...(sourceRun ? { sourceRun } : {}),
      ...(pluginRunId ? { pluginRunId } : {}),
      evidenceFields: {
        hasAuthorFans: authorFans !== null,
        hasKeywords: keywords.length > 0,
        hasMediaUnderstanding: Boolean(mediaUnderstanding),
      },
    },
  };
}
