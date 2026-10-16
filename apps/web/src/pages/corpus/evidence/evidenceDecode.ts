/**
 * 作品列表、作品详情与评论页的白名单解码。必需字段缺失或类型不对就报「返回内容无法解析」；
 * 可选字段缺失是 `null` 或 `UNKNOWN`——未知就是未知。多出来的字段（含平台用户标识）一律丢弃。
 */
import { isRecord } from '../../../shared/api';
import {
  decode,
  list,
  obj,
  optBool,
  optNum,
  optObj,
  optStr,
  str,
  strList,
  type Json,
} from '../../../shared/decode';
import { imagePath, mediaPath } from './evidencePaths';
import type {
  AuthorContext,
  Channels,
  Comment,
  CommentPage,
  Coverage,
  CoverageEntry,
  CurrentMetric,
  DetailField,
  Derivative,
  Fragment,
  Inspector,
  LaneSummary,
  ListMedia,
  MediaSlot,
  Metric,
  OcrLayering,
  Provenance,
  Receipt,
  Reobservation,
  TimelinePoint,
  WorkDetail,
  WorkDisplay,
  WorkItem,
  WorkList,
} from './evidenceTypes';

const UNKNOWN = 'UNKNOWN';

function stateOf(record: Json, name: string): string {
  return optStr(record, name) ?? UNKNOWN;
}

/** 数组里只收对象，其余元素忽略（媒体句柄等列表里不会有别的东西）。 */
function records(value: unknown): Json[] {
  return Array.isArray(value) ? value.filter(isRecord) : [];
}

function metric(record: Json, value: string, state: string): Metric {
  return { value: optNum(record, value), state: stateOf(record, state) };
}

function parseEngagement(record: Json | null) {
  const source = record ?? {};
  return {
    like: metric(source, 'likeCount', 'likeCountState'),
    comment: metric(source, 'commentCount', 'commentCountState'),
    collect: metric(source, 'collectCount', 'collectCountState'),
    share: metric(source, 'shareCount', 'shareCountState'),
  };
}

function parseDisplay(record: Json | null): WorkDisplay {
  const display = record ?? {};
  return {
    title: optStr(display, 'title'),
    titleState: stateOf(display, 'titleState'),
    titleSource: optStr(display, 'titleSource') ?? 'platform_title',
    creatorName: optStr(display, 'creatorDisplayName'),
    creatorState: stateOf(display, 'creatorState'),
    publishedAt: optStr(display, 'publishedAt'),
    publishedAtState: stateOf(display, 'publishedAtState'),
    publishedAtSourceText: optStr(display, 'publishedAtSourceText'),
    publishedAtSourceField: optStr(display, 'publishedAtSourceField'),
    publishedAtSourceKind: optStr(display, 'publishedAtSourceKind') ?? 'unknown',
    publishedAtPrecision: optStr(display, 'publishedAtPrecision') ?? 'unknown',
    publishedAtParserVersion: optStr(display, 'publishedAtParserVersion'),
    engagement: parseEngagement(optObj(display, 'engagement')),
  };
}

function parseListMedia(media: Json | null): ListMedia {
  const source = media ?? {};
  const cover = optObj(source, 'cover') ?? {};
  const avatar = optObj(source, 'avatar') ?? {};
  const avatarBlob = optObj(avatar, 'blob') ?? {};
  const images = records(source.images);
  const covers = records(source.coverCandidates);
  const videos = records(optObj(source, 'video')?.items);
  const local = [...images, ...covers, ...videos].filter(slot => mediaPath(slot.localAssetUrl) !== null).length;
  return {
    state: optStr(source, 'state'),
    coverState: optStr(cover, 'state') ?? 'NOT_OBSERVED',
    coverUrl: imagePath(cover.localAssetUrl),
    avatarState: optStr(avatar, 'state') ?? 'NOT_OBSERVED',
    avatarUrl: optStr(avatarBlob, 'deliveryState') === 'INLINE_SAFE' ? mediaPath(avatar.localAssetUrl) : null,
    imageCount: images.length,
    coverCandidateCount: covers.length,
    videoCount: videos.length,
    localObjectCount: local + (mediaPath(cover.localAssetUrl) !== null ? 1 : 0),
  };
}

function parseFragment(value: Json | null): Fragment | null {
  if (value === null) return null;
  const text = optStr(value, 'text');
  if (text === null || text === '') return null;
  const offset = optNum(value, 'matchOffset');
  const length = optNum(value, 'matchLength');
  return {
    text,
    sourceKind: optStr(value, 'sourceKind') ?? UNKNOWN,
    selectionBasis: optStr(value, 'selectionBasis') ?? UNKNOWN,
    truncated: optBool(value, 'truncated') === true,
    sourceRef: optStr(value, 'sourceRef'),
    slotKey: optStr(value, 'slotKey'),
    matchOffset: offset !== null && Number.isInteger(offset) ? offset : null,
    matchLength: length !== null && Number.isInteger(length) ? length : null,
  };
}

function parseLane(record: Json): LaneSummary {
  return { lane: str(record, 'lane'), state: stateOf(record, 'state'), valueState: stateOf(record, 'valueState'), retained: optNum(record, 'retained') };
}

export function parseWorkItem(item: Json): WorkItem {
  const identity = obj(item, 'identity');
  const context = optObj(item, 'collectionContext') ?? {};
  const summary = optObj(item, 'summary') ?? {};
  const usage = optObj(item, 'domainUsage');
  return {
    ref: str(identity, 'publicRef'),
    platform: str(identity, 'platform'),
    detailUrl: optStr(item, 'detailUrl'),
    display: parseDisplay(optObj(item, 'display')),
    context: {
      authorIdentityMatchState: stateOf(context, 'authorIdentityMatchState'),
      targetKind: optStr(context, 'targetKind'),
      targetDisplayState: stateOf(context, 'targetDisplayState'),
      targetDisplayName: optStr(context, 'targetDisplayName'),
    },
    role: usage === null ? null : optStr(usage, 'role'),
    media: parseListMedia(optObj(item, 'media')),
    lanes: item.laneSummaries === undefined || item.laneSummaries === null ? [] : list(item, 'laneSummaries', parseLane),
    fragment: parseFragment(optObj(item, 'evidenceFragment')),
    lastObservedAt: optStr(summary, 'lastObservedAt'),
    primaryLimitation: optStr(summary, 'primaryLimitation'),
  };
}

export function parseWorkList(value: unknown): WorkList {
  return decode(value, root => ({
    items: list(root, 'items', parseWorkItem),
    cursor: optStr(root, 'cursor'),
    truncated: optBool(root, 'truncated') === true,
    scanLimited: optBool(root, 'scanLimited') === true,
    scannedCount: optNum(root, 'scannedCount'),
    asOf: optStr(root, 'asOf'),
  }));
}

// ───────────────────────── 详情 ─────────────────────────

function parseReceipt(record: Json | null): Receipt | null {
  if (record === null) return null;
  return {
    total: optNum(record, 'total'),
    returned: optNum(record, 'returned'),
    truncated: optBool(record, 'truncated'),
    nextCursor: optStr(record, 'nextCursor'),
  };
}

function parseDetailField(record: Json | null): DetailField | null {
  if (record === null) return null;
  const source = optObj(record, 'source') ?? {};
  return {
    state: stateOf(record, 'state'),
    value: optStr(record, 'value'),
    sourceText: optStr(record, 'sourceText'),
    sourceRef: optStr(source, 'packageRef') ?? optStr(source, 'materialRef'),
  };
}

function parseCurrentMetric(record: Json | null): CurrentMetric | null {
  if (record === null) return null;
  const current = optObj(record, 'current');
  const previous = optObj(record, 'previous');
  return {
    state: stateOf(record, 'state'),
    value: current === null ? null : optNum(current, 'value'),
    observedAt: current === null ? null : optStr(current, 'observedAt'),
    previous: previous === null ? null : optNum(previous, 'value'),
    delta: optNum(record, 'delta'),
    deltaState: stateOf(record, 'deltaState'),
  };
}

function parseTimelinePoint(record: Json): TimelinePoint {
  return {
    observedAt: optStr(record, 'observedAt'),
    sourceLane: optStr(record, 'sourceLane'),
    like: metric(record, 'likeCount', 'likeCountState'),
    comment: metric(record, 'commentCount', 'commentCountState'),
    collect: metric(record, 'collectCount', 'collectCountState'),
    share: metric(record, 'shareCount', 'shareCountState'),
  };
}

function parseCoverage(record: Json | null): Coverage | null {
  if (record === null) return null;
  return {
    count: optNum(record, 'count'),
    countState: stateOf(record, 'countState'),
    retained: optNum(record, 'retained'),
    uniqueCollectedCount: optNum(record, 'uniqueCollectedCount'),
    stoppedReason: optStr(record, 'stoppedReason'),
    collectionState: optStr(record, 'collectionState'),
  };
}

function parseCoverageEntry(record: Json): CoverageEntry {
  return {
    observedAt: optStr(record, 'observedAt'),
    state: stateOf(record, 'state'),
    collectionScope: optStr(record, 'collectionScope'),
    requestedLimit: optNum(record, 'requestedLimit'),
    pageCommentCount: optNum(record, 'pageCommentCount'),
    expectedCount: optNum(record, 'expectedCount'),
    uniqueCollectedCount: optNum(record, 'uniqueCollectedCount'),
    collectionState: optStr(record, 'collectionState'),
    producerAcquired: optNum(record, 'producerAcquired'),
    retained: optNum(record, 'retained'),
    failed: optNum(record, 'failed'),
    taskRef: optStr(record, 'taskRef'),
    attemptRef: optStr(record, 'attemptRef'),
    packageRef: optStr(record, 'packageRef'),
    receiptRef: optStr(record, 'receiptRef'),
  };
}

function parseAuthorContext(record: Json | null): AuthorContext | null {
  if (record === null) return null;
  return {
    displayName: optStr(record, 'displayName'),
    displayNameState: stateOf(record, 'displayNameState'),
    biographyState: stateOf(record, 'biographyState'),
    followerCount: optNum(record, 'followerCount'),
    followerCountState: stateOf(record, 'followerCountState'),
    observedAt: optStr(record, 'observedAt'),
  };
}

function parseLayering(record: Json | null): OcrLayering | null {
  if (record === null) return null;
  return {
    layoutRef: optStr(record, 'layoutRef'),
    state: stateOf(record, 'state'),
    decisionSource: optStr(record, 'decisionSource'),
    coverHeadline: optStr(record, 'coverHeadline'),
    cleanCorpusEligible: optBool(record, 'cleanCorpusEligible') === true,
    imageSubstantiveText: optStr(record, 'imageSubstantiveText'),
  };
}

function parseDerivative(record: Json): Derivative {
  return {
    kind: optStr(record, 'kind'),
    displayText: optStr(record, 'displayText'),
    state: stateOf(record, 'state'),
    slotKey: optStr(record, 'slotKey'),
    layering: parseLayering(optObj(record, 'ocrLayering')),
  };
}

function parseMediaSlot(record: Json): MediaSlot {
  const blob = optObj(record, 'blob') ?? {};
  const replica = optObj(record, 'replica') ?? {};
  const handle = optStr(record, 'localAssetUrl');
  return {
    slotKey: optStr(record, 'slotKey'),
    purpose: optStr(record, 'purpose') ?? 'unknown',
    producerOrdinal: optNum(record, 'producerOrdinal'),
    relationshipOrdinal: optNum(record, 'relationshipOrdinal'),
    bytesState: stateOf(record, 'bytesState'),
    assetUrl: mediaPath(handle),
    hasHandle: handle !== null && handle !== '',
    deliveryState: optStr(blob, 'deliveryState'),
    mime: optStr(blob, 'deliveryMimeType') || optStr(blob, 'declaredMimeType') || '',
    materializationRef: optStr(replica, 'materializationRef'),
    replicaState: optStr(record, 'replicaState'),
    displayOrderState: optStr(record, 'displayOrderState'),
  };
}

function parseProvenance(record: Json | null): Provenance | null {
  if (record === null) return null;
  const lens = optObj(record, 'stationAccountLens');
  return {
    packageRefs: strList(record, 'packageRefs'),
    taskRefs: strList(record, 'taskRefs'),
    attemptRefs: strList(record, 'attemptRefs'),
    receiptRefs: strList(record, 'receiptRefs'),
    targetRefs: strList(record, 'targetRefs'),
    workOrderRefs: strList(record, 'workOrderRefs'),
    producers: strList(record, 'producers'),
    stationAccountLensState: lens === null ? UNKNOWN : stateOf(lens, 'state'),
    receipt: parseReceipt(optObj(record, 'receipt')),
  };
}

function optList<T>(record: Json, name: string, parse: (item: Json) => T): T[] {
  const value = record[name];
  return value === undefined || value === null ? [] : list(record, name, parse);
}

function parseInspector(record: Json): Inspector {
  const detail = optObj(record, 'detailCurrent') ?? {};
  const engagement = optObj(optObj(record, 'engagementCurrent') ?? {}, 'metrics') ?? {};
  return {
    detail: {
      title: parseDetailField(optObj(detail, 'title')),
      creator: parseDetailField(optObj(detail, 'creator')),
      publishedAt: parseDetailField(optObj(detail, 'publishedAt')),
      body: parseDetailField(optObj(detail, 'body')),
    },
    engagement: {
      likeCount: parseCurrentMetric(optObj(engagement, 'likeCount')),
      commentCount: parseCurrentMetric(optObj(engagement, 'commentCount')),
      collectCount: parseCurrentMetric(optObj(engagement, 'collectCount')),
      shareCount: parseCurrentMetric(optObj(engagement, 'shareCount')),
    },
    timeline: optList(record, 'engagementTimeline', parseTimelinePoint),
    commentsCoverage: parseCoverage(optObj(record, 'commentsCoverage')),
    repliesCoverage: parseCoverage(optObj(record, 'repliesCoverage')),
    commentsHistory: optList(record, 'commentsCoverageHistory', parseCoverageEntry),
    repliesHistory: optList(record, 'repliesCoverageHistory', parseCoverageEntry),
    commentsReceipt: parseReceipt(optObj(record, 'commentsReceipt')),
    authorContext: parseAuthorContext(optObj(record, 'authorContext')),
    sharedAuthorContext: parseAuthorContext(optObj(record, 'sharedCurrentAuthorContext')),
    derivatives: optList(record, 'derivatives', parseDerivative),
    mediaSlots: optList(record, 'mediaSlots', parseMediaSlot),
    mediaSlotsReceipt: parseReceipt(optObj(record, 'mediaSlotsReceipt')),
    provenance: parseProvenance(optObj(record, 'provenance')),
    limitations: strList(record, 'limitations'),
  };
}

function parseReobservation(record: Json | null): Reobservation {
  const channel = record ?? {};
  return {
    supported: optBool(channel, 'supported') === true,
    eligible: optBool(channel, 'eligible') === true,
    reason: optStr(channel, 'reason') ?? 'REOBSERVATION_ELIGIBILITY_UNKNOWN',
    url: optStr(channel, 'url'),
    mediaPolicy: optStr(channel, 'mediaPolicy'),
  };
}

function parseChannels(record: Json | null): Channels {
  const channels = record ?? {};
  const comments = optObj(channels, 'comments');
  const receipt = (name: string) => parseReceipt(optObj(optObj(channels, name) ?? {}, 'receipt'));
  return {
    commentsUrl: comments === null ? null : optStr(comments, 'url'),
    commentsReceipt: receipt('comments'),
    mediaReceipt: receipt('media'),
    provenanceReceipt: receipt('provenance'),
    reobservation: parseReobservation(optObj(channels, 'reobservation')),
  };
}

export function parseWorkDetail(value: unknown): WorkDetail {
  return decode(value, root => {
    const item = obj(root, 'item');
    return {
      item: parseWorkItem(item),
      inspector: parseInspector(optObj(item, 'inspector') ?? {}),
      channels: parseChannels(optObj(root, 'channels')),
    };
  });
}

// ───────────────────────── 评论 ─────────────────────────

function parseComment(record: Json): Comment {
  const relation = str(record, 'relation');
  return {
    sourceRef: str(record, 'sourceRef'),
    relation: relation === 'REPLY' ? 'REPLY' : 'ROOT',
    body: optStr(record, 'body'),
    bodyState: stateOf(record, 'bodyState'),
    bodyTruncated: optBool(record, 'bodyTruncated') === true,
  };
}

export function parseCommentPage(value: unknown): CommentPage {
  return decode(value, root => ({
    items: list(root, 'items', parseComment),
    total: optNum(root, 'total'),
    returned: optNum(root, 'returned'),
    truncated: optBool(root, 'truncated') === true,
    nextCursor: optStr(root, 'nextCursor'),
  }));
}
