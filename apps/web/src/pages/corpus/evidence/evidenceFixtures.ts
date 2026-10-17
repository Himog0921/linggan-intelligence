/**
 * 证据库测试用的合成数据（服务器响应的形状）。形状照 `GET /api/local/work-resources` 与
 * `…/{ref}` 的真实输出（由隔离库上的合成夹具实际调用得到），内容全是合成的，不是任何真实作者、作品或原声。
 */
export const DOMAIN = '00000000-0000-4000-8000-000000000001';
export const OTHER_DOMAIN = '00000000-0000-4000-8000-000000000002';

export function workRef(n: number): string {
  return `10000000-0000-4000-8000-${String(n).padStart(12, '0')}`;
}

export function mediaRef(n: number): string {
  return `20000000-0000-4000-8000-${String(n).padStart(12, '0')}`;
}

const SHA = 'a'.repeat(64);

export function mediaHandle(n: number): string {
  return `/api/local/media/${mediaRef(n)}/${SHA}`;
}

export interface SlotOptions {
  purpose?: string;
  ordinal?: number;
  bytesState?: string;
  handle?: string | null;
  deliveryState?: string;
  mime?: string;
  slotKey?: string;
  displayOrderState?: string;
}

/** 媒体槽位（列表层 `media.images[]`／`coverCandidates[]`，详情层 `inspector.mediaSlots[]` 同形）。 */
export function slotJson(n: number, ref: string, options: SlotOptions = {}) {
  const purpose = options.purpose ?? 'body_image';
  const ordinal = options.ordinal ?? n;
  return {
    blob: {
      byteSize: 999,
      declaredMimeType: options.mime ?? 'image/png',
      deliveryMimeType: options.mime ?? 'image/png',
      deliveryState: options.deliveryState ?? 'INLINE_SAFE',
      detectedMimeType: null,
      durationMs: null,
      intrinsicDimensions: { height: null, state: 'UNKNOWN', width: null },
      sha256: SHA,
    },
    bytesState: options.bytesState ?? 'ACQUIRED',
    components: null,
    displayOrderBasis: 'producer_global_sequence_unverified',
    displayOrderState: options.displayOrderState ?? 'UNKNOWN',
    displayOrdinal: null,
    dispositionState: 'UNKNOWN',
    localAssetUrl: options.handle === undefined ? mediaHandle(n) : options.handle,
    producerOrdinal: ordinal,
    purpose,
    relationship: purpose === 'cover' ? 'content.cover' : 'content.image',
    relationshipKind: purpose === 'cover' ? 'content.cover' : 'content.image',
    relationshipOrdinal: ordinal,
    replica: { isCurrentOrigin: true, materializationRef: mediaRef(n), originObservationRef: mediaRef(n + 1000) },
    replicaState: 'VERIFIED_AT_MATERIALIZATION',
    slotKey: options.slotKey ?? `xhs:${ref}:${purpose === 'cover' ? 'cover' : 'image'}:${ordinal}`,
    subjectExternalId: ref,
    subjectKind: 'content',
  };
}

export interface MediaOptions {
  state?: string;
  cover?: 'available' | 'none';
  images?: number;
  videos?: number;
  avatar?: 'inline' | 'blocked' | 'none';
}

export function mediaJson(n: number, options: MediaOptions = {}) {
  const ref = workRef(n);
  const coverAvailable = options.cover === 'available';
  const images = Array.from({ length: options.images ?? 0 }, (_, index) => slotJson(n * 10 + index + 1, ref, { ordinal: index + 1 }));
  const videos = Array.from({ length: options.videos ?? 0 }, (_, index) => slotJson(n * 10 + 50 + index, ref, { purpose: 'video', mime: 'video/mp4', ordinal: index + 1 }));
  const avatar = options.avatar ?? 'none';
  return {
    avatar:
      avatar === 'none'
        ? { localAssetUrl: null, relationship: 'author.avatar', state: 'NOT_OBSERVED' }
        : {
            blob: { deliveryState: avatar === 'inline' ? 'INLINE_SAFE' : 'UNSAFE_MIME', deliveryMimeType: 'image/png' },
            bytesState: 'ACQUIRED',
            localAssetUrl: mediaHandle(n * 10 + 90),
            state: 'ACQUIRED',
          },
    commentImages: { items: [], relationship: 'comment.image', state: 'NOT_OBSERVED' },
    contractVersion: 'linggan.media-resource.v1',
    cover: coverAvailable
      ? { localAssetUrl: mediaHandle(n * 10), relationship: 'content.cover', selectedBy: 'explicit_cover', state: 'AVAILABLE' }
      : { localAssetUrl: null, relationship: 'content.cover', selectedBy: 'none', state: 'NOT_OBSERVED' },
    coverCandidates: coverAvailable ? [slotJson(n * 10, ref, { purpose: 'cover', ordinal: 1 })] : [],
    images,
    ocr: { relationship: 'content.ocr', resources: [], state: 'UNKNOWN' },
    state: options.state ?? (images.length + videos.length > 0 || coverAvailable ? 'OBSERVED' : 'NOT_OBSERVED'),
    transcript: { relationship: 'content.transcript', resources: [], state: 'UNKNOWN' },
    video: { items: videos, relationship: 'content.video', state: videos.length ? 'OBSERVED' : 'NOT_OBSERVED' },
  };
}

export function laneJson(lane: string, state: string, retained: number | null = null, valueState = state === 'UNKNOWN' ? 'UNKNOWN' : 'KNOWN') {
  return { lane, state, valueState, retained, observed: retained, failed: null, limitations: [], latestObservedAt: null };
}

const ALL_LANES = ['discovery', 'detail', 'comments', 'replies', 'author', 'media_slots', 'media_bytes', 'ocr', 'asr'];

export interface ItemOptions {
  title?: string | null;
  titleState?: string;
  titleSource?: string;
  author?: string | null;
  platform?: string;
  published?: string | null;
  publishedState?: string;
  publishedPrecision?: string;
  publishedSourceText?: string | null;
  likes?: number | null;
  comments?: number | null;
  collects?: number | null;
  shares?: number | null;
  lanes?: Record<string, string | [string, number]>;
  media?: MediaOptions;
  fragment?: Record<string, unknown> | null;
  observedAt?: string;
  role?: string;
  detailUrl?: string | null;
  context?: Record<string, unknown>;
}

function metricFields(prefix: string, value: number | null | undefined, fallback: number) {
  const known = value === undefined ? fallback : value;
  return { [`${prefix}Count`]: known, [`${prefix}CountState`]: known === null ? 'UNKNOWN' : 'KNOWN' };
}

/** 一个列表项（服务器形状）。没有特别指定的字段取一组合理的默认值。 */
export function listItemJson(n: number, options: ItemOptions = {}) {
  const ref = workRef(n);
  const lanes = { detail: 'SEARCHABLE', ...(options.lanes ?? {}) };
  const title = options.title === undefined ? `合成作品 ${n}` : options.title;
  return {
    collectionContext: {
      authorIdentityMatchState: 'NOT_VERIFIED',
      relationshipState: 'UNKNOWN',
      targetDisplayName: null,
      targetDisplayState: 'UNKNOWN',
      targetKind: null,
      targetRef: null,
      workOrderRef: null,
      ...(options.context ?? {}),
    },
    detailUrl: options.detailUrl === undefined ? `/api/local/work-resources/${ref}?domain=${DOMAIN}` : options.detailUrl,
    display: {
      creatorDisplayName: options.author === undefined ? `合成作者 ${n}` : options.author,
      creatorState: (options.author === undefined ? `合成作者 ${n}` : options.author) === null ? 'UNKNOWN' : 'KNOWN',
      engagement: {
        ...metricFields('like', options.likes, 1200 + n),
        ...metricFields('comment', options.comments, 8),
        ...metricFields('collect', options.collects, 30),
        ...metricFields('share', options.shares, 2),
      },
      publishedAt: options.published === undefined ? '2026-10-13 13:28' : options.published,
      publishedAtParserVersion: 'xhs-detail-time-v2',
      publishedAtPrecision: options.publishedPrecision ?? 'second',
      publishedAtReferenceObservedAt: null,
      publishedAtSourceField: 'publishTime',
      publishedAtSourceKind: 'platform_epoch',
      publishedAtSourceText: options.publishedSourceText === undefined ? '1791869289' : options.publishedSourceText,
      publishedAtState: options.publishedState ?? ((options.published === undefined ? '2026-10-13 13:28' : options.published) === null ? 'UNKNOWN' : 'KNOWN'),
      title,
      titleMediaDisplayOrdinal: null,
      titleSource: options.titleSource ?? 'platform_title',
      titleState: options.titleState ?? (title === null ? 'UNKNOWN' : 'KNOWN'),
    },
    domainUsage: { basisKinds: ['legacy_domain_migration'], role: options.role ?? 'primary' },
    evidenceFragment:
      options.fragment === undefined
        ? {
            matchLength: null,
            matchOffset: null,
            selectionBasis: 'FIRST_AVAILABLE',
            slotKey: null,
            sourceKind: 'detail_body',
            sourceRef: null,
            text: `合成正文 ${n}：先做两分钟，再决定要不要继续。`,
            truncated: false,
          }
        : options.fragment,
    identity: { contentExternalId: `ext-${n}`, platform: options.platform ?? 'xhs', publicRef: ref },
    laneSummaries: ALL_LANES.map(lane => {
      const entry = lanes[lane as keyof typeof lanes];
      return Array.isArray(entry) ? laneJson(lane, entry[0], entry[1]) : laneJson(lane, entry ?? 'UNKNOWN', lane === 'detail' ? 1 : null);
    }),
    matchedFields: [],
    media: mediaJson(n, options.media),
    preview: { alt: '没有已验证的本地媒体副本', bytesState: 'UNKNOWN', localAssetUrl: null, observedSourceState: 'UNKNOWN', slotPurpose: null },
    summary: { lastObservedAt: options.observedAt ?? '2026-10-10 17:00', primaryLimitation: 'OTHER_LANES_NOT_EVALUATED', restrictionState: 'UNKNOWN' },
  };
}

export interface ListOptions {
  cursor?: string | null;
  truncated?: boolean;
  scanLimited?: boolean;
  scannedCount?: number;
  asOf?: string;
}

export function listJson(items: unknown[], options: ListOptions = {}) {
  return {
    asOf: options.asOf ?? '2026-10-16 13:28:09.89457+08',
    cursor: options.cursor ?? null,
    items,
    queryScope: 'accepted_typed_material_text_only',
    scanLimited: options.scanLimited ?? false,
    scannedCount: options.scannedCount ?? items.length,
    truncated: options.truncated ?? options.cursor !== undefined,
  };
}

// ───────────────────────── 详情 ─────────────────────────

const SOURCE = { materialRef: mediaRef(500), observedAt: '2026-10-10T09:00:00Z', packageRef: mediaRef(501), recordOrdinal: 0, recordedAt: '2026-10-16 13:28' };

function fieldJson(value: string | null, state = value === null ? 'UNKNOWN' : 'KNOWN') {
  return { source: SOURCE, state, value };
}

function currentMetric(value: number | null, previous: number | null = null) {
  return value === null
    ? { current: null, delta: null, deltaState: 'UNKNOWN', previous: null, state: 'UNKNOWN' }
    : {
        current: { observedAt: '2026-10-10T09:00:00Z', packageRef: mediaRef(501), recordedAt: '2026-10-16 13:28:09.519695+08', sourceLane: 'detail', value },
        delta: previous === null ? null : value - previous,
        deltaState: previous === null ? 'UNKNOWN' : 'KNOWN',
        previous: previous === null ? null : { observedAt: '2026-10-09T09:00:00Z', value: previous },
        state: 'KNOWN',
      };
}

export interface DetailOptions extends ItemOptions {
  body?: string | null;
  slots?: ReturnType<typeof slotJson>[];
  derivatives?: unknown[];
  commentsUrl?: string | null;
  reobservation?: Partial<{ available: boolean; eligible: boolean; supported: boolean; reason: string; url: string | null; mediaPolicy: string }>;
  limitations?: string[];
  timeline?: unknown[];
  commentsCoverage?: Record<string, unknown> | null;
  commentsHistory?: unknown[];
  authorContext?: Record<string, unknown> | null;
  provenance?: Record<string, unknown> | null;
}

const RECEIPT = { limit: 64, nextCursor: null, returned: 0, total: 0, truncated: false };

/** `GET /api/local/work-resources/{ref}` 的响应（`{ item, channels }`）。 */
export function detailJson(n: number, options: DetailOptions = {}) {
  const ref = workRef(n);
  const base = listItemJson(n, options);
  const body = options.body === undefined ? `合成正文 ${n}：先做两分钟，再决定要不要继续。` : options.body;
  const likes = options.likes === undefined ? 1200 + n : options.likes;
  const inspector = {
    authorContext: options.authorContext === undefined ? null : options.authorContext,
    commentThreads: [],
    commentsAccess: { accessLevel: 'RESTRICTED_SOURCE', bodyReturned: false, externalIdentityReturned: false },
    commentsCoverage:
      options.commentsCoverage === undefined
        ? { count: 45, countState: 'KNOWN', retained: 45, stoppedReason: 'fixture_complete', uniqueCollectedCount: null, collectionState: null }
        : options.commentsCoverage,
    commentsCoverageHistory: options.commentsHistory ?? [],
    commentsReceipt: { detailRequired: true, nextCursor: null, returned: 0, total: 48, truncated: true },
    derivatives: options.derivatives ?? [],
    derivativesReceipt: RECEIPT,
    detailCurrent: {
      body: fieldJson(body),
      creator: fieldJson(options.author === undefined ? `合成作者 ${n}` : options.author),
      publishedAt: { ...fieldJson('2026-10-13 13:28'), parserVersion: 'xhs-detail-time-v2', precision: 'second', sourceField: 'publishTime', sourceKind: 'platform_epoch', sourceText: '1791869289' },
      title: fieldJson(options.title === undefined ? `合成作品 ${n}` : options.title),
    },
    displayPolicy: 'MINIMUM_NECESSARY',
    engagementCurrent: {
      metrics: { collectCount: currentMetric(30), commentCount: currentMetric(8), likeCount: currentMetric(likes, likes === null ? null : likes - 5), shareCount: currentMetric(2) },
      selection: 'LATEST_KNOWN_OBSERVATION_PER_METRIC',
    },
    engagementTimeline: options.timeline ?? [
      { collectCount: 30, collectCountState: 'KNOWN', commentCount: 8, commentCountState: 'KNOWN', likeCount: likes, likeCountState: likes === null ? 'UNKNOWN' : 'KNOWN', observedAt: '2026-10-10T09:00:00Z', shareCount: 2, shareCountState: 'KNOWN', sourceLane: 'detail' },
    ],
    limitations: options.limitations ?? [],
    mediaSlots: options.slots ?? [],
    mediaSlotsReceipt: { ...RECEIPT, returned: options.slots?.length ?? 0, total: options.slots?.length ?? 0 },
    overview: { fields: [] },
    permissions: { displayPolicy: 'MINIMUM_NECESSARY', remoteCandidateUris: 'NOT_EXPOSED' },
    provenance:
      options.provenance === undefined
        ? {
            attemptRefs: [mediaRef(601)],
            coverageRefs: [],
            packageRefs: [mediaRef(501)],
            producers: [mediaRef(603)],
            receipt: { limit: 20, nextCursor: null, returned: 1, total: 1, truncated: false },
            receiptRefs: [mediaRef(604)],
            recordRefs: [],
            stationAccountLens: { state: 'UNKNOWN' },
            taskRefs: [mediaRef(605)],
          }
        : options.provenance,
    repliesCoverage: { count: 3, countState: 'KNOWN', retained: 3, stoppedReason: 'fixture_complete' },
    repliesCoverageHistory: [],
  };
  const reobservation = { available: false, eligible: false, mediaPolicy: 'EXISTING_ASSETS_REUSED', reason: 'TARGET_LINKED_ACTIVE_DEEP_ARCHIVE_AUTHORIZATION_REQUIRED', requires: 'TARGET_LINKED_ACTIVE_DEEP_ARCHIVE_AUTHORIZATION', supported: true, url: null, ...(options.reobservation ?? {}) };
  return {
    channels: {
      comments: {
        receipt: { detailRequired: true, nextCursor: null, returned: 0, total: 48, truncated: true },
        url: options.commentsUrl === undefined ? `/api/local/work-resources/${ref}/comments?domain=${DOMAIN}` : options.commentsUrl,
      },
      derivatives: { receipt: RECEIPT },
      media: { receipt: { ...RECEIPT, returned: options.slots?.length ?? 0, total: options.slots?.length ?? 0 } },
      provenance: { receipt: { limit: 20, nextCursor: null, returned: 1, total: 1, truncated: false } },
      reobservation,
    },
    item: { ...base, inspector },
  };
}

export function commentsJson(count: number, options: { cursor?: string | null; total?: number; offset?: number } = {}) {
  const offset = options.offset ?? 0;
  return {
    accessLevel: 'LOCAL_AUTHORIZED_RESEARCH',
    channel: 'comments',
    externalAuthorIdentityReturned: false,
    items: Array.from({ length: count }, (_, index) => ({
      anonymousAuthorContext: { identity: 'WITHHELD', platformUserIdReturned: false },
      body: `合成评论 ${offset + index}：我也是这样开始的。`,
      bodyState: 'KNOWN',
      bodyTruncated: false,
      observedAt: '2026-08-28T10:00:00Z',
      relation: (offset + index) % 10 === 9 ? 'REPLY' : 'ROOT',
      sourceRef: `30000000-0000-4000-8000-${String(offset + index).padStart(12, '0')}`,
    })),
    limit: 20,
    nextCursor: options.cursor ?? null,
    orderBasis: 'stable_source_ref_not_platform_order',
    query: null,
    returned: count,
    searchableOriginalText: true,
    total: options.total ?? count,
    truncated: options.cursor !== undefined && options.cursor !== null,
  };
}

// ───────────────────────── 底稿 ─────────────────────────

export const REVISION = 'r'.repeat(64);

export function planJson(overrides: Partial<Record<string, unknown>> = {}) {
  return { alreadyCurrent: 0, expectedRevision: REVISION, imageJobs: 4, imageSlots: 4, layoutCount: 1, maximumLayoutsPerWork: 100, needsOcr: 3, pendingLayouts: 1, workCount: 1, ...overrides };
}

export function dossierMediaJson(n: number, options: { layered?: boolean; contextOnly?: boolean; handle?: string | null; other?: string } = {}) {
  const lines = [
    { bbox: [0.12, 0.28, 0.88, 0.305], confidence: 0.98, lineRef: mediaRef(700 + n), ordinal: 0, text: '第0行连续的图片正文，完整可用。' },
    { bbox: [0.1, 0.1, 0.85, 0.17], confidence: 0.98, lineRef: mediaRef(710 + n), ordinal: 1, text: '封面大字独立保留' },
    { bbox: [0, 0.9, 0.2, 0.92], confidence: 0.6, lineRef: mediaRef(720 + n), ordinal: 2, text: '平台水印' },
  ];
  return {
    contextOnly: options.contextOnly ?? false,
    derivativeRef: mediaRef(800 + n),
    displayText: '第0行连续的图片正文，完整可用。',
    kind: 'ocr_text',
    ocrLayering:
      options.layered === false
        ? null
        : {
            cleanCorpusEligible: false,
            coverHeadline: null,
            decisionSource: 'rules',
            engineVersion: 'synthetic-paddle',
            excludedLines: [{ classification: 'platform_watermark', lineRef: mediaRef(720 + n) }],
            imageHeight: 320,
            imageSubstantiveText: null,
            imageWidth: 240,
            layerVersion: 'rules-v1',
            layeringRef: mediaRef(730 + n),
            layoutRef: mediaRef(740 + n),
            lines,
            retainedLines: [
              { classification: 'substantive_text', lineRef: mediaRef(700 + n) },
              { classification: 'primary_copy', lineRef: mediaRef(710 + n) },
            ],
            state: 'PARTIAL',
          },
    originalAssetUrl: options.handle === undefined ? mediaHandle(n) : options.handle,
    originalMaterializationRef: mediaRef(n),
    otherProcessing: options.other ? { state: options.other } : undefined,
    processorKind: 'image_ocr',
    researchText: '第0行连续的图片正文，完整可用。',
    selectionBasis: 'readable_source,image_dimensions',
    slotKey: `xhs:ext:image:${n}`,
    slotOrdinal: n,
    state: 'ACQUIRED',
  };
}

export function taskInputJson(kind: 'creator' | 'comments' | 'topics') {
  const fragment = { field: 'title', characterCount: 6, end: 6, fragmentId: `f-${kind}`, sourceRef: mediaRef(500), sourceVersion: 'v', start: 0, text: '合成作品标题' };
  return kind === 'comments'
    ? { characterBudget: 20000, includedCharacterCount: 6, omitted: [], sources: [{ characterCount: 6, kind: 'native_title', text: '合成作品标题' }], truncated: false }
    : { characterBudget: 4000, fragments: [fragment], includedCharacterCount: 6, markdown: `## 平台标题\n\n[S1]\n合成作品标题\n`, truncated: kind === 'creator' ? false : true };
}

export interface DossierOptions {
  fragments?: { field: string; text: string; id?: string }[];
  media?: unknown[];
  plan?: Record<string, unknown>;
  coverage?: Record<string, unknown>;
  metadata?: Record<string, unknown>;
}

export function dossierJson(n: number, options: DossierOptions = {}) {
  const ref = workRef(n);
  const fragments = (options.fragments ?? [{ field: 'title', text: '合成作品标题' }, { field: 'body', text: '合成正文。'.repeat(120) }]).map(({ field, text, id }, index) => ({
    end: Array.from(text).length,
    field,
    fragmentId: id ?? `${ref}.${field}`,
    sourceRef: mediaRef(500 + index),
    sourceVersion: `version-${index}`,
    start: 0,
    text,
  }));
  return {
    dossier: {
      asOf: '2026-10-16 13:28:10.18622+08',
      contract: 'research-dossier.v1',
      coverage: { body: 'KNOWN', comments: 'separate_authorized_research_channel', mediaAvailable: 1, mediaCount: 8, sampling: 'collected_material_only', title: 'KNOWN', titleQualification: 'source_value', uncertainImages: 1, ...(options.coverage ?? {}) },
      fragments,
      materialRevision: 'm'.repeat(64),
      media: options.media ?? [dossierMediaJson(1)],
      metadata: {
        authorDisplayName: '合成作者',
        authorExternalId: 'synthetic-author-id',
        collects: { observedAt: '2026-10-10T09:00:00Z', state: 'KNOWN', value: 30 },
        comments: { observedAt: '2026-10-10T09:00:00Z', state: 'KNOWN', value: 321 },
        externalId: `ext-${n}`,
        likes: { observedAt: '2026-10-10T09:00:00Z', state: 'KNOWN', value: 1200 },
        observedAt: '2026-10-10 17:00',
        platform: 'xhs',
        publishedAt: '2026-10-13 13:28',
        publishedPrecision: 'second',
        shares: { observedAt: '2026-10-10T09:00:00Z', state: 'KNOWN', value: 2 },
        ...(options.metadata ?? {}),
      },
      revision: REVISION,
      sections: [
        { fragmentIds: fragments.filter(f => f.field === 'title').map(f => f.fragmentId), kind: 'title', label: '平台标题', sourceState: 'KNOWN', state: 'available' },
        { fragmentIds: [], kind: 'cover_headline', label: '封面与图片大字', sourceState: null, state: 'unknown' },
        { fragmentIds: fragments.filter(f => f.field === 'body').map(f => f.fragmentId), kind: 'body', label: '正文', sourceState: 'KNOWN', state: 'available' },
        { fragmentIds: [], kind: 'comments', label: '评论', sourceState: null, state: 'separate' },
      ],
      workRef: ref,
    },
    inputs: { comments: taskInputJson('comments'), creator: taskInputJson('creator'), topics: taskInputJson('topics') },
    markdown: '# 合成底稿\n\n合成作品标题\n',
    relayerPlan: planJson(options.plan),
  };
}

export function dossierCommentsJson(count: number, options: { cursor?: string | null; offset?: number; total?: number } = {}) {
  const offset = options.offset ?? 0;
  return {
    accessLevel: 'LOCAL_AUTHORIZED_RESEARCH',
    items: Array.from({ length: count }, (_, index) => ({
      anonymousAuthorContext: { identity: 'WITHHELD', platformUserIdReturned: false },
      body: `合成评论 ${offset + index}`,
      bodyState: 'KNOWN',
      bodyTruncated: false,
      likeCount: 5,
      likeState: 'KNOWN',
      observedAt: '2026-08-28T10:00:00Z',
      parentSourceRef: null,
      relation: 'ROOT',
      sourceRef: `40000000-0000-4000-8000-${String(offset + index).padStart(12, '0')}`,
    })),
    limit: 30,
    nextCursor: options.cursor ?? null,
    returned: count,
    total: options.total ?? count,
  };
}

export function relayerReceiptJson(overrides: Partial<Record<string, unknown>> = {}) {
  return { alreadyCurrent: 0, layerVersion: 'rules-v2', needsOcr: 0, results: [], scopeCount: 1, skipped: 0, updated: 1, workRef: workRef(1), ...overrides };
}

export function creatorPreviewJson(overrides: Partial<Record<string, unknown>> = {}) {
  return { available: true, estimatedInputTokensUpperBound: 6195, inputTokenLimit: 8192, input: { materials: '## 平台标题\n\n合成作品标题' }, stage: 'prepared_request', system: '合成系统提示词（诊断用）', ...overrides };
}

// ───────────────────────── 复观测 ─────────────────────────

export function operationJson(overrides: Partial<Record<string, unknown>> = {}) {
  return {
    admission: 'ADMITTED',
    admissionReason: '已在作品原先关联的有效授权范围内准入',
    decisionRef: mediaRef(901),
    execution: 'QUEUED',
    expiresAt: null,
    leaseRef: null,
    media: { reason: 'EXISTING_ASSETS_REUSED', state: 'NOT_REQUESTED' },
    publicRef: workRef(1),
    requestRef: mediaRef(902),
    targetRef: mediaRef(903),
    tasks: [],
    workOrderRef: mediaRef(904),
    ...overrides,
  };
}

export function taskJson(state: string, overrides: Partial<Record<string, unknown>> = {}) {
  return { acquireMedia: 'not_requested', attemptId: null, capability: 'content_detail', claimedAt: null, commentLimit: 'not_requested', packageRef: null, receiptRef: null, state, taskId: mediaRef(950), ...overrides };
}

export function shellJson(overrides: Record<string, unknown> = {}) {
  return {
    domains: [
      { domainRef: DOMAIN, name: '合成领域一', status: 'active', targetCount: 3, sampleCount: 80 },
      { domainRef: OTHER_DOMAIN, name: '合成领域二', status: 'active', targetCount: 1, sampleCount: null },
    ],
    selectedDomain: DOMAIN,
    collectionState: '观察中',
    ...overrides,
  };
}
