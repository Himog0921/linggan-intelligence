/**
 * 证据库页面用到的接口响应形状（白名单解码后的结果）。
 *
 * 只收页面渲染需要的字段；服务器多给的字段（包括平台用户标识、账号镜头）一律丢弃，因此页面不会比
 * 旧页多显示任何字段（P10）。可选字段缺失是 `null`——未知就是未知，绝不补成 0 或空串。
 */

/** 一个带状态的计数：状态不是 KNOWN 时，数值不可用。 */
export interface Metric {
  value: number | null;
  state: string;
}

export interface Engagement {
  like: Metric;
  comment: Metric;
  collect: Metric;
  share: Metric;
}

export interface WorkDisplay {
  title: string | null;
  titleState: string;
  /** `platform_title` 或 `cover_ocr`（仅展示用的封面 OCR 回退）。 */
  titleSource: string;
  creatorName: string | null;
  creatorState: string;
  publishedAt: string | null;
  publishedAtState: string;
  publishedAtSourceText: string | null;
  publishedAtSourceField: string | null;
  publishedAtSourceKind: string;
  publishedAtPrecision: string;
  publishedAtParserVersion: string | null;
  engagement: Engagement;
}

export interface CollectionContext {
  authorIdentityMatchState: string;
  targetKind: string | null;
  targetDisplayState: string;
  targetDisplayName: string | null;
}

export interface LaneSummary {
  lane: string;
  state: string;
  valueState: string;
  retained: number | null;
}

export interface Fragment {
  text: string;
  sourceKind: string;
  selectionBasis: string;
  truncated: boolean;
  sourceRef: string | null;
  slotKey: string | null;
  matchOffset: number | null;
  matchLength: number | null;
}

/** 列表层的媒体事实：地址已经过同源媒体路径校验，不安全的一律是 null。 */
export interface ListMedia {
  state: string | null;
  coverState: string;
  coverUrl: string | null;
  avatarState: string;
  /** 头像只在 `INLINE_SAFE` 时才使用地址。 */
  avatarUrl: string | null;
  imageCount: number;
  coverCandidateCount: number;
  videoCount: number;
  /** 与旧页同一算法：图片、封面候选、视频里带受控媒体句柄的个数，再加封面本身的一个。 */
  localObjectCount: number;
}

export interface WorkItem {
  ref: string;
  platform: string;
  /** 服务器原样给出的详情地址；使用前必须经过 `detailPath` 校验。 */
  detailUrl: string | null;
  display: WorkDisplay;
  context: CollectionContext;
  /** `primary` 或 `reference`。 */
  role: string | null;
  media: ListMedia;
  lanes: readonly LaneSummary[];
  fragment: Fragment | null;
  lastObservedAt: string | null;
  primaryLimitation: string | null;
}

export interface WorkList {
  items: WorkItem[];
  cursor: string | null;
  truncated: boolean;
  scanLimited: boolean;
  scannedCount: number | null;
  asOf: string | null;
}

// ───────────────────────── 详情 ─────────────────────────

export interface Receipt {
  total: number | null;
  returned: number | null;
  truncated: boolean | null;
  nextCursor: string | null;
}

/** 详情字段的当前事实（标题、作者、发布时间、正文）：状态、值，以及来源引用。 */
export interface DetailField {
  state: string;
  value: string | null;
  sourceText: string | null;
  /** 来源引用：优先包引用，其次材料引用。 */
  sourceRef: string | null;
}

export interface CurrentMetric {
  state: string;
  value: number | null;
  observedAt: string | null;
  previous: number | null;
  delta: number | null;
  deltaState: string;
}

export interface TimelinePoint {
  observedAt: string | null;
  sourceLane: string | null;
  like: Metric;
  comment: Metric;
  collect: Metric;
  share: Metric;
}

export interface Coverage {
  count: number | null;
  countState: string;
  retained: number | null;
  uniqueCollectedCount: number | null;
  stoppedReason: string | null;
  collectionState: string | null;
}

export interface CoverageEntry {
  observedAt: string | null;
  state: string;
  collectionScope: string | null;
  requestedLimit: number | null;
  pageCommentCount: number | null;
  expectedCount: number | null;
  uniqueCollectedCount: number | null;
  collectionState: string | null;
  producerAcquired: number | null;
  retained: number | null;
  failed: number | null;
  taskRef: string | null;
  attemptRef: string | null;
  packageRef: string | null;
  receiptRef: string | null;
}

export interface AuthorContext {
  displayName: string | null;
  displayNameState: string;
  biographyState: string;
  followerCount: number | null;
  followerCountState: string;
  observedAt: string | null;
}

export interface OcrLayering {
  layoutRef: string | null;
  state: string;
  decisionSource: string | null;
  coverHeadline: string | null;
  cleanCorpusEligible: boolean;
  imageSubstantiveText: string | null;
}

export interface Derivative {
  kind: string | null;
  displayText: string | null;
  state: string;
  slotKey: string | null;
  layering: OcrLayering | null;
}

export interface MediaSlot {
  slotKey: string | null;
  purpose: string;
  producerOrdinal: number | null;
  relationshipOrdinal: number | null;
  bytesState: string;
  /** 受控本地句柄（已过 `/api/local/media/` 校验）；没有或不安全是 null。 */
  assetUrl: string | null;
  /** 有句柄但服务器没有标记为可内联。 */
  hasHandle: boolean;
  deliveryState: string | null;
  mime: string;
  /** 本地字节的内容指纹（64 位十六进制）；没有取得字节或形状不对是 null。同一指纹就是同一份文件。 */
  blobSha256: string | null;
  materializationRef: string | null;
  replicaState: string | null;
  displayOrderState: string | null;
}

export interface Provenance {
  packageRefs: string[];
  taskRefs: string[];
  attemptRefs: string[];
  receiptRefs: string[];
  targetRefs: string[];
  workOrderRefs: string[];
  producers: string[];
  stationAccountLensState: string;
  receipt: Receipt | null;
}

export interface Inspector {
  sourceNoteUrl: string | null;
  detail: Record<'title' | 'creator' | 'publishedAt' | 'body', DetailField | null>;
  engagement: Record<'likeCount' | 'commentCount' | 'collectCount' | 'shareCount', CurrentMetric | null>;
  timeline: TimelinePoint[];
  commentsCoverage: Coverage | null;
  repliesCoverage: Coverage | null;
  commentsHistory: CoverageEntry[];
  repliesHistory: CoverageEntry[];
  commentsReceipt: Receipt | null;
  authorContext: AuthorContext | null;
  sharedAuthorContext: AuthorContext | null;
  derivatives: Derivative[];
  mediaSlots: MediaSlot[];
  mediaSlotsReceipt: Receipt | null;
  provenance: Provenance | null;
  limitations: string[];
}

export interface Reobservation {
  supported: boolean;
  eligible: boolean;
  reason: string;
  url: string | null;
  mediaPolicy: string | null;
}

export interface Channels {
  commentsUrl: string | null;
  commentsReceipt: Receipt | null;
  mediaReceipt: Receipt | null;
  provenanceReceipt: Receipt | null;
  reobservation: Reobservation;
}

export interface WorkDetail {
  item: WorkItem;
  inspector: Inspector;
  channels: Channels;
}

// ───────────────────────── 评论 ─────────────────────────

export interface Comment {
  sourceRef: string;
  relation: 'ROOT' | 'REPLY';
  body: string | null;
  bodyState: string;
  bodyTruncated: boolean;
}

export interface CommentPage {
  items: Comment[];
  total: number | null;
  returned: number | null;
  truncated: boolean;
  nextCursor: string | null;
}
