// The shared Browser Producer Runtime Adapter.
//
// Collectors remain platform-owned and keep their existing page-reading/recovery behaviour. This
// module is their only Linggan-facing boundary: it creates flat TaskSpecs, freezes one immutable
// package per attempt and sends it through the durable outbox. It intentionally knows nothing
// about Topic, research or market reasoning.

import { createTaskSpec } from './adapter.js';

export const PRODUCER_CAPABILITY = Object.freeze({
  DISCOVERY_SEARCH: 'discovery_search',
  PROFILE_DISCOVERY: 'profile_discovery',
  CONTENT_DETAIL: 'content_detail',
  COMMENTS: 'comments',
  REPLIES: 'replies',
  AUTHOR_PROFILE: 'author_profile',
  MEDIA_SLOTS: 'media_slots',
  MEDIA_BYTES: 'media_bytes',
  BATCH_CHECKPOINT: 'batch_checkpoint',
});

const PACKAGE_VERSION = 'linggan.producer.capture-package.v1';

function asArray(value) {
  return Array.isArray(value) ? value : [];
}

function isoNow(now = () => new Date()) {
  return now().toISOString();
}

function numeric(value) {
  const parsed = Number(value);
  return Number.isFinite(parsed) && parsed >= 0 ? Math.trunc(parsed) : 0;
}

function boundedCoverageLayer(capability, input = {}) {
  const observed = numeric(input.observed);
  const attempted = Math.min(observed, numeric(input.attempted));
  const acquired = Math.min(attempted, numeric(input.acquired));
  const verified = Math.min(acquired, numeric(input.verified));
  return {
    capability,
    observed,
    attempted,
    acquired,
    verified,
    failed: numeric(input.failed),
    notAttempted: numeric(input.notAttempted),
    unknown: numeric(input.unknown),
    stoppedReason: String(input.stoppedReason || 'unknown'),
  };
}

export function createCapturePackage({
  packageRef = crypto.randomUUID(),
  packageKind,
  platform,
  observedAt = isoNow(),
  capturedAt = isoNow(),
  target = { basis: 'unknown' },
  coverage = {},
  records = [],
  checkpoint,
} = {}) {
  if (!Object.values(PRODUCER_CAPABILITY).includes(packageKind)) {
    throw new Error('unsupported_linggan_producer_capability');
  }
  if (!['xhs', 'douyin'].includes(platform)) {
    throw new Error('unsupported_linggan_producer_platform');
  }
  return {
    contractVersion: PACKAGE_VERSION,
    packageRef,
    packageKind,
    platform,
    observedAt,
    capturedAt,
    coverage: {
      target: target && typeof target === 'object' && !Array.isArray(target) ? target : { basis: 'unknown' },
      layers: [boundedCoverageLayer(packageKind, coverage)],
    },
    records: asArray(records).slice(0, 2048),
    ...(checkpoint === undefined ? {} : { checkpoint }),
  };
}

export function createManualRuntimeTask({
  platform,
  pageType,
  target,
  capabilitiesRequested,
  maximumQuota = 1,
  commentLimit = 'not_requested',
  acquireMedia = 'not_requested',
  stopConditions = ['manual_stop', 'maximum_quota'],
  taskId,
} = {}) {
  return createTaskSpec({
    taskId,
    source: 'manual',
    platform,
    pageType,
    target,
    capabilitiesRequested,
    maximumQuota,
    commentLimit,
    acquireMedia,
    riskPolicy: 'local_trusted_user_initiated',
    stopConditions,
  });
}

export function createScheduledRuntimeTask(input = {}) {
  // This creates a contract object only. It never starts a poller, lease or station process.
  return createTaskSpec({ ...input, source: 'scheduled' });
}

export function packageContentDetail({ platform, note, observedAt, capturedAt } = {}) {
  const media = asArray(note?.images).concat(note?.video ? [note.video] : []);
  return createCapturePackage({
    packageKind: PRODUCER_CAPABILITY.CONTENT_DETAIL,
    platform,
    observedAt,
    capturedAt,
    target: { basis: 'known_set', contentExternalId: String(note?.noteId || note?.id || note?.contentId || '') },
    coverage: { observed: 1, attempted: 1, acquired: 1, verified: 0, unknown: media.length ? 1 : 0, stoppedReason: 'detail_read_complete' },
    records: [{ kind: 'content_detail', sourceObject: normalizeSourceObject(platform, note), payload: note || {} }],
  });
}

export function packageComments({ platform, result, noteId, observedAt, capturedAt } = {}) {
  const comments = asArray(result?.comments || result?.data || result)
    .filter((comment) => !isReplyRecord(comment));
  const requested = numeric(result?.requested ?? result?.maxTotal ?? comments.length);
  return createCapturePackage({
    packageKind: PRODUCER_CAPABILITY.COMMENTS,
    platform,
    observedAt,
    capturedAt,
    target: { basis: 'known_set', contentExternalId: String(noteId || result?.noteId || '') },
    coverage: {
      observed: numeric(result?.discovered ?? comments.length), attempted: numeric(result?.attempted ?? comments.length),
      acquired: comments.length, verified: 0, failed: numeric(result?.failed),
      notAttempted: Math.max(0, requested - numeric(result?.attempted ?? comments.length)), unknown: numeric(result?.unknown),
      stoppedReason: String(result?.stopReason || 'collector_complete'),
    },
    records: comments.map((comment) => ({ kind: 'comment', payload: comment })),
  });
}

// Replies are a distinct producer capability even when the retained page reader returns them
// in the same array as top-level comments.  This keeps a parent/reply relationship from being
// silently flattened into a generic comment count or a second Content observation.
export function packageReplies({ platform, result, noteId, observedAt, capturedAt } = {}) {
  const replies = asArray(result?.comments || result?.data || result)
    .filter((comment) => isReplyRecord(comment));
  const requested = numeric(result?.requested ?? result?.maxTotal ?? replies.length);
  return createCapturePackage({
    packageKind: PRODUCER_CAPABILITY.REPLIES,
    platform,
    observedAt,
    capturedAt,
    target: { basis: 'known_set', contentExternalId: String(noteId || result?.noteId || '') },
    coverage: {
      observed: replies.length,
      attempted: replies.length,
      acquired: replies.length,
      verified: 0,
      failed: 0,
      // The collector's requested amount refers to the combined comment tree.  It cannot be
      // used to invent a reply-only remainder, so only expose it when a dedicated count exists.
      notAttempted: numeric(result?.replyNotAttempted),
      unknown: numeric(result?.replyUnknown),
      stoppedReason: String(result?.replyStopReason || result?.stopReason || (requested ? 'collector_complete' : 'unknown')),
    },
    records: replies.map((reply) => ({ kind: 'reply', payload: reply })),
  });
}

export function packageAuthorProfile({ platform, author, observedAt, capturedAt } = {}) {
  return createCapturePackage({
    packageKind: PRODUCER_CAPABILITY.AUTHOR_PROFILE,
    platform,
    observedAt,
    capturedAt,
    target: { basis: 'known_set', authorExternalId: String(author?.userId || author?.id || '') },
    coverage: { observed: 1, attempted: 1, acquired: 1, verified: 0, stoppedReason: 'profile_read_complete' },
    records: [{ kind: 'author_profile', sourceObject: normalizeSourceObject(platform, author, 'author'), payload: author || {} }],
  });
}

export function packageMediaSlots({ platform, note, observedAt, capturedAt } = {}) {
  const sources = collectMediaCandidates(note);
  const sourceObject = normalizeSourceObject(platform, note);
  const contentExternalId = sourceObject.externalId;
  return createCapturePackage({
    packageKind: PRODUCER_CAPABILITY.MEDIA_SLOTS,
    platform,
    observedAt,
    capturedAt,
    target: { basis: 'known_set', contentExternalId: String(note?.noteId || note?.id || '') },
    coverage: { observed: sources.length, attempted: 0, acquired: 0, verified: 0, notAttempted: sources.length, stoppedReason: 'media_acquisition_not_started' },
    records: sources.map((candidate, ordinal) => {
      const slotOrdinal = ordinal + 1;
      // Slot identity says "this content's nth image/video". A URL is intentionally only an
      // observation; it can change without replacing the slot or a previously acquired blob.
      const slotKey = `${platform}:${encodeURIComponent(contentExternalId)}:${candidate.role}:${slotOrdinal}`;
      return {
        kind: 'media_slot',
        slotKey,
        observationRef: crypto.randomUUID(),
        slot: { role: candidate.role, ordinal: slotOrdinal },
        observation: { externalUri: candidate.url, candidateUris: candidate.candidateUris, observedAt },
        sourceObject,
      };
    }),
  });
}

export function packageBatchCheckpoint({ platform, kind, progress, observedAt, capturedAt } = {}) {
  const total = numeric(progress?.total);
  const current = Math.min(total || Number.MAX_SAFE_INTEGER, numeric(progress?.current));
  return createCapturePackage({
    packageKind: PRODUCER_CAPABILITY.BATCH_CHECKPOINT,
    platform,
    observedAt,
    capturedAt,
    target: { basis: 'execution_checkpoint', taskType: String(kind || '') },
    coverage: { observed: total, attempted: current, acquired: current, verified: 0, unknown: Math.max(0, total - current), stoppedReason: String(progress?.taskState || progress?.status || 'running') },
    records: [],
    checkpoint: { taskType: kind || '', current, total, taskState: progress?.taskState || progress?.status || 'running', message: progress?.message || '' },
  });
}

function normalizeSourceObject(platform, value = {}, type = 'content') {
  const externalId = value?.noteId || value?.id || value?.contentId || value?.userId || value?.authorId || '';
  return { platform, type, externalId: String(externalId || '') };
}

function isReplyRecord(value = {}) {
  return Boolean(
    String(value?.replyToCommentId || value?.parentCommentId || value?.rootCommentId || '').trim(),
  );
}

function collectMediaCandidates(note = {}) {
  const output = [];
  const push = (role, value) => {
    const values = typeof value === 'string'
      ? [value]
      : [
        value?.url, value?.urlDefault, value?.originUrl, value?.downloadUrl,
        value?.urlList?.[0], value?.url_list?.[0], value?.uri,
      ];
    const candidateUris = [...new Set(values
      .flatMap((candidate) => Array.isArray(candidate) ? candidate : [candidate])
      .map((candidate) => String(candidate || '').trim())
      .filter(Boolean))];
    if (candidateUris.length) output.push({ role, url: candidateUris[0], candidateUris });
  };
  asArray(note.images).forEach((value) => push('image', value));
  if (note.cover || note.coverUrl) push('cover', note.cover || note.coverUrl);
  if (note.video) push('video', note.video);
  asArray(note.livePhotoStreams).forEach((value) => push('live_photo', value));
  return output;
}
