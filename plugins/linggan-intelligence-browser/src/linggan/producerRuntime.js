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
  return Number.isFinite(parsed) && parsed >= 0 ? Math.trunc(parsed) : null;
}

function boundedCoverageLayer(capability, input = {}) {
  // A quota, a time budget, and a currently visible surface are not a known set.  Do not turn
  // a missing count into `0` or infer an unattempted remainder from it.  The numeric fields are
  // still required by the wire contract, so `unknown` explicitly carries the uncertainty.
  const values = ['observed', 'attempted', 'acquired', 'verified', 'failed', 'notAttempted', 'unknown']
    .reduce((result, key) => ({ ...result, [key]: numeric(input[key]) }), {});
  const invalidCount = Object.values(values).filter((value) => value === null).length;
  const observed = values.observed ?? 0;
  const attempted = Math.min(observed, values.attempted ?? 0);
  const acquired = Math.min(attempted, values.acquired ?? 0);
  const verified = Math.min(acquired, values.verified ?? 0);
  const knownSet = String(input.targetBasis || '').trim() === 'known_set';
  return {
    capability,
    observed,
    attempted,
    acquired,
    verified,
    failed: values.failed ?? 0,
    notAttempted: knownSet ? (values.notAttempted ?? 0) : 0,
    unknown: (values.unknown ?? 0) + invalidCount + (knownSet ? 0 : (input.unknown === 0 ? 0 : 1)),
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
      layers: [boundedCoverageLayer(packageKind, { ...coverage, targetBasis: target?.basis })],
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
  const collection = normalizedCommentCollectionReceipt(result, noteId);
  const comments = collection?.analysisUsability === 'not_usable'
    ? []
    : asArray(result?.comments || result?.data || result).filter((comment) => !isReplyRecord(comment));
  return createCapturePackage({
    packageKind: PRODUCER_CAPABILITY.COMMENTS,
    platform,
    observedAt,
    capturedAt,
    target: {
      basis: 'known_set',
      contentExternalId: String(noteId || result?.noteId || ''),
      ...(collection ? { commentCollection: collection } : {}),
    },
    coverage: {
      // The lane count is its own emitted record count.  Whole-comment-set completeness lives
      // in `target.commentCollection`, because top-level comments and replies are delivered
      // through two distinct packages and must never be added across Attempts.
      observed: comments.length, attempted: comments.length,
      acquired: comments.length, verified: 0, failed: numeric(result?.failed),
      notAttempted: 0, unknown: 0,
      stoppedReason: collection?.stopReason || String(result?.stopReason || 'unknown'),
    },
    records: comments.map((comment) => ({
      kind: 'comment',
      sourceObject: { platform, type: 'content', externalId: String(noteId || result?.noteId || '') },
      payload: comment,
    })),
  });
}

// Replies are a distinct producer capability even when the retained page reader returns them
// in the same array as top-level comments.  This keeps a parent/reply relationship from being
// silently flattened into a generic comment count or a second Content observation.
export function packageReplies({ platform, result, noteId, observedAt, capturedAt } = {}) {
  const collection = normalizedCommentCollectionReceipt(result, noteId);
  const replies = collection?.analysisUsability === 'not_usable'
    ? []
    : asArray(result?.comments || result?.data || result).filter((comment) => isReplyRecord(comment));
  return createCapturePackage({
    packageKind: PRODUCER_CAPABILITY.REPLIES,
    platform,
    observedAt,
    capturedAt,
    target: {
      basis: 'known_set',
      contentExternalId: String(noteId || result?.noteId || ''),
    },
    coverage: {
      observed: replies.length,
      attempted: replies.length,
      acquired: replies.length,
      verified: 0,
      failed: 0,
      // Whole-tree completion is carried by the comments package. The reply lane reports only
      // what this independently accepted package retained, so a missing/rejected reply package
      // can never be hidden by a copied whole-tree receipt.
      notAttempted: 0,
      unknown: 0,
      stoppedReason: String(result?.replyStopReason || result?.stopReason || 'unknown'),
    },
    records: replies.map((reply) => ({
      kind: 'reply',
      sourceObject: { platform, type: 'content', externalId: String(noteId || result?.noteId || '') },
      payload: reply,
    })),
  });
}

function normalizedCommentCollectionReceipt(result = {}, noteId = '') {
  const source = result?.collectionReceipt && typeof result.collectionReceipt === 'object'
    ? result.collectionReceipt
    : {};
  const explicitScope = String(source.scope || result?.collectionScope || '').trim();
  if (!explicitScope) return null;
  const total = numeric(source.uniqueCollectedCount ?? result?.total ?? asArray(result?.comments || result?.data || result).length) ?? 0;
  const pageCommentCount = numeric(source.pageCommentCount ?? result?.publicCommentCount);
  const expectedCount = numeric(source.expectedCount);
  const scope = explicitScope;
  const state = String(source.state || result?.collectionState || 'partial').trim() || 'partial';
  const analysisUsability = String(source.analysisUsability || result?.analysisUsability || (total > 0 ? 'usable' : 'empty')).trim();
  const targetIdentity = String(source.targetIdentity || result?.targetIdentity || 'matched').trim();
  return {
    version: 1,
    noteId: String(source.noteId || noteId || result?.noteId || '').trim(),
    scope,
    ...(numeric(source.requestedLimit) === null ? {} : { requestedLimit: numeric(source.requestedLimit) }),
    pageCommentCount,
    expectedCount,
    uniqueCollectedCount: total,
    state,
    analysisUsability,
    targetIdentity,
    stopReason: String(source.stopReason || result?.stopReason || 'unknown').trim() || 'unknown',
  };
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
    coverage: { observed: sources.length, attempted: 0, acquired: 0, verified: 0, notAttempted: sources.length, unknown: 0, stoppedReason: 'media_acquisition_not_started' },
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
        observation: {
          externalUri: candidate.url,
          candidateUris: candidate.candidateUris,
          ...(candidate.components ? { components: candidate.components } : {}),
          observedAt,
        },
        sourceObject,
      };
    }),
  });
}

export function packageBatchCheckpoint({ platform, kind, progress, observedAt, capturedAt } = {}) {
  const total = numeric(progress?.knownSetSize);
  const current = numeric(progress?.current) ?? 0;
  return createCapturePackage({
    packageKind: PRODUCER_CAPABILITY.BATCH_CHECKPOINT,
    platform,
    observedAt,
    capturedAt,
    target: { basis: 'execution_checkpoint', taskType: String(kind || '') },
    // A batch counter often reports a maximum quota or elapsed budget.  It is a checkpoint,
    // never a claim that `total - current` objects were not attempted.
    coverage: { observed: total ?? current, attempted: Math.min(total ?? current, current), acquired: Math.min(total ?? current, current), verified: 0, unknown: total === null ? 1 : 0, stoppedReason: String(progress?.taskState || progress?.status || 'running') },
    records: [],
    checkpoint: { taskType: kind || '', current, knownSetSize: total, taskState: progress?.taskState || progress?.status || 'running', message: progress?.message || '' },
  });
}

export function packageDiscovery({ platform, cards = [], query = '', authorExternalId = '', observedAt, capturedAt, surface = 'current_visible_surface', pageFacts = undefined } = {}) {
  const kind = authorExternalId ? PRODUCER_CAPABILITY.PROFILE_DISCOVERY : PRODUCER_CAPABILITY.DISCOVERY_SEARCH;
  const visible = asArray(cards).slice(0, 2048);
  return createCapturePackage({
    packageKind: kind,
    platform,
    observedAt,
    capturedAt,
    target: authorExternalId
      ? { basis: 'current_visible_surface', authorExternalId: String(authorExternalId), surface }
      : { basis: 'current_visible_surface', query: String(query || ''), surface },
    coverage: {
      observed: visible.length,
      attempted: visible.length,
      acquired: visible.length,
      verified: 0,
      // A visible page is not a complete search/profile result set.  `unknown` says that
      // explicitly; it is not a synthetic remaining count.
      unknown: 1,
      stoppedReason: 'surface_read_complete',
    },
    records: visible.map((card, ordinal) => {
      // DOM nodes are only a temporary aid for ordering the current surface. They are not
      // transportable evidence and must never enter the local outbox payload.
      const { element: _element, _top: _top, _left: _left, ...payload } = card && typeof card === 'object' ? card : {};
      return {
      kind: authorExternalId ? 'profile_discovery_card' : 'discovery_card',
      resultPosition: ordinal + 1,
      sourceObject: normalizeSourceObject(platform, payload),
      payload,
      };
    }),
    // The Producer contract rejects unknown top-level fields. Keep page facts in its existing
    // execution checkpoint slot so the receipt survives strict parsing and durable delivery.
    ...(pageFacts && typeof pageFacts === 'object' && !Array.isArray(pageFacts)
      ? { checkpoint: { kind: 'search_surface_receipt', surfaceReceipt: pageFacts } }
      : {}),
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
  const candidateValues = (value) => (typeof value === 'string'
    ? [value]
    : [
      value?.url, value?.urlDefault, value?.originUrl, value?.downloadUrl,
      value?.urlList, value?.url_list, value?.candidates, value?.uri,
    ]);
  const normalizeUris = (values) => [...new Set(values
    .flatMap((candidate) => Array.isArray(candidate) ? candidate : [candidate])
    .map((candidate) => String(candidate || '').trim())
    .filter(Boolean))];
  const push = (role, value) => {
    const candidateUris = normalizeUris(candidateValues(value));
    if (!candidateUris.length) return;
    if (role !== 'live_photo') {
      output.push({ role, url: candidateUris[0], candidateUris });
      return;
    }
    const stillCandidates = normalizeUris(candidateValues(value?.coverUrl || value?.still));
    const motionCandidates = candidateUris;
    output.push({
      role,
      url: motionCandidates[0],
      candidateUris: [...new Set([...motionCandidates, ...stillCandidates])],
      components: {
        ...(stillCandidates.length ? { still: { candidateUris: stillCandidates } } : {}),
        motion: { candidateUris: motionCandidates },
      },
    });
  };
  asArray(note.images).forEach((value) => push('image', value));
  if (note.cover || note.coverUrl) push('cover', note.cover || note.coverUrl);
  if (note.video) push('video', note.video);
  asArray(note.livePhotoStreams).forEach((value) => push('live_photo', value));
  return output;
}
