/**
 * 证据库的展示规则：状态词表、材料完整度、作者规则、时间与数量的写法。全部是纯函数，
 * 与旧页 `evidence_library.js` 同一套判断（有意改动在行内标明），页面不重算任何业务结论。
 */
import { humanMoment } from '../../../shared/formatting';
import type { CurrentMetric, DetailField, Engagement, Fragment, LaneSummary, ListMedia, Metric, Receipt, TimelinePoint, WorkDisplay, WorkItem } from './evidenceTypes';

export type Tone = 'info' | 'success' | 'warning' | 'danger' | 'unknown';

/**
 * 状态词表。状态用文字＋圆点＋颜色表达，不只靠颜色。词表外的值写「来源未完整表达」，原码另在 `data-state`
 * 与技术事实里保留。
 *
 * `RETIRED` 保留：它不再是筛选项（DR-1，读模型不认），但详情里的派生物仍可能带这个状态。
 * `PAGE_UNAVAILABLE`、`DETAIL_READ_BLOCKED`、`NOT_STARTED` 是 DR-3 补的三条文案，含义取自后端
 * `content_reobservation.rs::task_state`：生产方确认页面不在／页面打开过但没读成／没有形成工单、尚未开始。
 */
export const STATE_LABELS: Readonly<Record<string, readonly [string, Tone]>> = {
  UNKNOWN: ['当前未知', 'unknown'],
  SOURCE_TEXT_ONLY: ['仅有来源时间文本', 'warning'],
  NOT_REQUESTED: ['尚未请求', 'unknown'],
  QUEUED: ['已排队', 'info'],
  LEASED: ['租约已签发', 'info'],
  ACTIVE: ['租约有效', 'info'],
  MERGED: ['已合并到在途工作', 'info'],
  RELEASED: ['租约已释放', 'warning'],
  CLAIMED: ['工位已认领', 'info'],
  RUNNING: ['执行中', 'info'],
  ACCEPTED: ['已接纳回执', 'success'],
  EXPIRED: ['租约已过期', 'warning'],
  EXPIRED_WITHOUT_RECEIPT: ['租约结束，未见回执', 'warning'],
  COMPLETED_WITHOUT_RECEIPT: ['任务结束，未见回执', 'warning'],
  INPUT_BLOCKED: ['输入不可执行，已停止', 'warning'],
  PAGE_UNAVAILABLE: ['平台页面已不存在（采集端确认）', 'warning'],
  DETAIL_READ_BLOCKED: ['详情页已打开，但没有读成', 'danger'],
  NOT_STARTED: ['尚未开始', 'unknown'],
  NOT_OBSERVED: ['尚未形成观察', 'unknown'],
  OBSERVED: ['已观察', 'info'],
  PARTIAL: ['部分取得', 'warning'],
  ACQUIRED: ['已取得', 'success'],
  PROCESSING: ['处理中', 'info'],
  NOT_ENABLED: ['处理器未启用', 'unknown'],
  SEARCHABLE: ['可检索', 'success'],
  KNOWN_EMPTY: ['已处理，未识别到内容', 'info'],
  FAILED: ['执行失败', 'danger'],
  RISK_CONTROL: ['风险控制停止', 'danger'],
  BYTES_CLEANED: ['字节已清理', 'warning'],
  RETIRED: ['旧 OCR 已停用', 'warning'],
  WITHDRAWN_OR_RESTRICTED: ['已撤回或限制读取', 'danger'],
  AVAILABLE: ['本地副本可用', 'success'],
};

export function stateMeta(state: string | null | undefined): readonly [string, Tone] {
  return (state ? STATE_LABELS[state] : undefined) ?? ['来源未完整表达', 'unknown'];
}

export const LANE_ORDER = ['discovery', 'detail', 'comments', 'replies', 'author', 'media_slots', 'media_bytes', 'ocr', 'asr'] as const;

export const LANE_LABELS: Readonly<Record<string, string>> = {
  discovery: '发现',
  detail: '详情',
  comments: '评论',
  replies: '回复',
  author: '作者',
  media_slots: '媒体槽位',
  media_bytes: '媒体字节',
  ocr: '图片文字',
  asr: '视频转录',
};

export const FRAGMENT_SOURCE_LABELS: Readonly<Record<string, string>> = {
  detail_body: '正文',
  comment_body: '评论',
  ocr_text: '图片文字',
  image_substantive_text: '图片实质文字',
  frame_ocr_text: '视频画面文字',
  asr_text: '视频转录',
};

export function fragmentSourceLabel(kind: string | null | undefined): string {
  return (kind ? FRAGMENT_SOURCE_LABELS[kind] : undefined) ?? '来源未表达';
}

const PURPOSE_LABELS: Readonly<Record<string, string>> = {
  cover: '封面',
  body_image: '正文图片',
  video: '视频',
  live_photo: '实况图片',
  author_avatar: '作者头像',
};

export function purposeLabel(purpose: string): string {
  return PURPOSE_LABELS[purpose] ?? purpose;
}

const LIMITATION_LABELS: Readonly<Record<string, string>> = {
  OTHER_LANES_NOT_EVALUATED: '其他材料通道尚未评估',
  SOURCE_INCOMPLETE: '来源信息不完整',
  ACTUAL_CANDIDATE_NOT_REPORTED_BY_PRODUCER: '采集端没有回报实际使用的候选地址',
  COMMENTS_NOT_EVALUATED: '评论尚未评估',
  MEDIA_NOT_EVALUATED: '媒体尚未评估',
  RAW_BODY_NOT_RETURNED: '正文原文不在本次读取范围内',
  LANE_NOT_EVALUATED: '该通道尚未评估',
};

/** 限制码的中文说明。词表外的码写「来源未完整表达」，原码另作技术旁注。 */
export function limitationCopy(code: string): string {
  return LIMITATION_LABELS[code] ?? '来源未完整表达';
}

/** 停止原因：保留原始码作旁注，前面给出中文含义；未知的码不猜成「完成」。来源 `stopped_reason_markup`。 */
export function stoppedReasonMeaning(raw: string): string {
  switch (raw) {
    case 'quota_reached':
    case 'maximum_quota':
      return '已达到本次配额';
    case 'surface_ended':
      return '当前页面内容已结束';
    case 'surface_read_complete':
      return '当前页面读取完成';
    case 'risk_control':
      return '平台风险控制导致停止';
    case 'manual_stop':
      return '用户手动停止';
    case 'unknown':
      return '停止原因未知';
    default:
      return '停止原因未归类';
  }
}

// ───────────────────────── 文本与数量 ─────────────────────────

/** 作品标题：状态已知才显示，否则写「标题当前未知」，不拿别的字段顶替。 */
export function workTitle(item: Pick<WorkItem, 'display'>): string {
  return knownText(item.display.title, item.display.titleState, '标题当前未知');
}

/** 状态为 KNOWN 且有值才显示值；否则显示「未知」文案，不补成 0 或空串。 */
export function knownText(value: string | number | null | undefined, state: string, unknownCopy = '当前未知'): string {
  return state === 'KNOWN' && value !== null && value !== undefined && value !== '' ? String(value) : unknownCopy;
}

export function formatNumber(value: number): string {
  return value.toLocaleString('zh-CN');
}

/** 互动数字：状态已知才显示数字，未知写「未知」，不是 0。 */
export function metricReading(metric: Metric): string {
  return metric.state === 'KNOWN' && metric.value !== null ? formatNumber(metric.value) : '未知';
}

/** 紧凑计数，仅用于侧栏徽标的悬停说明。 */
export function compactCount(value: number): string {
  if (value < 1000) return String(value);
  if (value < 1_000_000) return `${(value / 1000).toFixed(value < 10_000 ? 1 : 0)}K`;
  return `${(value / 1_000_000).toFixed(1)}M`;
}

export const ENGAGEMENT_KINDS = [
  ['like', '点赞'],
  ['comment', '评论'],
  ['collect', '收藏'],
  ['share', '分享'],
] as const;

export type EngagementKind = (typeof ENGAGEMENT_KINDS)[number][0];

export function engagementMetric(engagement: Engagement, kind: EngagementKind): Metric {
  return engagement[kind];
}

// ───────────────────────── 时间 ─────────────────────────

/** 列表层时间服务器已按北京时间给出：行内只到分钟，完整值放 `title`。 */
export function compactMoment(value: string | null | undefined): string | null {
  if (typeof value !== 'string' || value === '') return null;
  const match = /^(\d{4})-(\d{2})-(\d{2})[T ](\d{2}):(\d{2})/.exec(value);
  return match ? `${match[1]}-${match[2]}-${match[3]} ${match[4]}:${match[5]}` : value;
}

export function compactDay(value: string | null | undefined): string | null {
  if (typeof value !== 'string' || value === '') return null;
  const match = /^(\d{4}-\d{2}-\d{2})/.exec(value);
  return match?.[1] ?? value;
}

/** 封面铭牌的观察时刻：`MM-DD HH:MM`。 */
export function coverObservationMoment(value: string | null | undefined): string {
  if (typeof value !== 'string' || value === '') return '未知';
  const match = /^\d{4}-(\d{2})-(\d{2})[T ](\d{2}):(\d{2})/.exec(value);
  return match ? `${match[1]}-${match[2]} ${match[3]}:${match[4]}` : (compactMoment(value) ?? '未知');
}

interface PublishedOptions {
  /** 行内只写到日。 */
  compact?: boolean;
  /** 未知时写「发布时间当前未知」（带字段名）还是「当前未知」。 */
  withFieldName?: boolean;
}

/**
 * 发布时间的写法（H14）。精度为「日」时只显示日期，不把低精度补成整分钟；只有来源文本时写
 * 「来源时间：<来源原文>」；未知写「当前未知」。
 */
export function publishedCopy(display: WorkDisplay, { compact = false, withFieldName = true }: PublishedOptions = {}): string {
  if (display.publishedAtState === 'KNOWN') {
    const raw = display.publishedAt;
    if (!raw) return '发布时间已知';
    return compact || display.publishedAtPrecision === 'day' ? (compactDay(raw) ?? raw) : raw;
  }
  if (display.publishedAtState === 'SOURCE_TEXT_ONLY') return `来源时间：${display.publishedAtSourceText || '已观察'}`;
  return withFieldName ? '发布时间当前未知' : '当前未知';
}

/** 封面铭牌上的发布日：只有来源文本时写「来源时间」，其余同紧凑写法。 */
export function coverPublishedCopy(display: WorkDisplay): string {
  return display.publishedAtState === 'SOURCE_TEXT_ONLY' ? '来源时间' : publishedCopy(display, { compact: true, withFieldName: false });
}

/** 数据库原串（互动时间线、覆盖历史、租约到期、作者观察时间）统一成 `YYYY-MM-DD HH:MM` 北京时间。 */
export function momentText(value: string | null | undefined, empty = '当前未知'): string {
  return value === null || value === undefined || value.trim() === '' ? empty : humanMoment(value);
}

// ───────────────────────── 作者 ─────────────────────────

/**
 * 作者规则（作者合并决定 2026-09-04）：详情里的作者名优先；其次 creator 型监控目标的名称——创作者是
 * 通过自己的主页地址被监控的，在那里采到的作品就是他的；已报告 `MISMATCH` 时不回退。关键词型目标是搜索词
 * 而不是人，永不填作者。都没有写「当前未知」。
 */
export function effectiveAuthor(item: Pick<WorkItem, 'display' | 'context'>): string {
  if (item.display.creatorState === 'KNOWN' && item.display.creatorName) return item.display.creatorName;
  const context = item.context;
  const matched = context.authorIdentityMatchState !== 'MISMATCH';
  if (matched && context.targetKind === 'creator' && context.targetDisplayState === 'KNOWN' && context.targetDisplayName) {
    return context.targetDisplayName;
  }
  return '当前未知';
}

const TARGET_KIND_LABELS: Readonly<Record<string, string>> = { creator: '博主监控', keyword: '关键词监控' };

/** 列表行只显示一个作者；「这篇作品怎么来的」留在检查器的技术事实里。 */
export function monitoringOriginCopy(item: Pick<WorkItem, 'context'>): string {
  const context = item.context;
  if (context.targetDisplayState !== 'KNOWN' || !context.targetDisplayName) return '当前未知';
  const kind = context.targetKind ? TARGET_KIND_LABELS[context.targetKind] : undefined;
  return kind ? `${context.targetDisplayName}（${kind}）` : context.targetDisplayName;
}

// ───────────────────────── 材料完整度 ─────────────────────────

const READY = new Set(['ACQUIRED', 'SEARCHABLE', 'COMPLETE', 'KNOWN_EMPTY']);
const PARTIAL = new Set(['PARTIAL', 'BYTES_CLEANED']);
const FAILED = new Set(['FAILED', 'RISK_CONTROL', 'WITHDRAWN_OR_RESTRICTED']);
const PENDING = new Set(['QUEUED', 'PROCESSING', 'OBSERVED', 'NOT_OBSERVED', 'NOT_REQUESTED', 'NOT_ENABLED']);

export type Fill = 'ready' | 'partial' | 'failed' | 'pending' | 'unknown' | 'not-applicable';

export function materialFill(state: string): Exclude<Fill, 'not-applicable'> {
  if (READY.has(state)) return 'ready';
  if (PARTIAL.has(state)) return 'partial';
  if (FAILED.has(state)) return 'failed';
  if (PENDING.has(state)) return 'pending';
  return 'unknown';
}

interface MediaFacts {
  mediaObserved: boolean;
  hasImagery: boolean;
  hasVideo: boolean;
}

function mediaFacts(media: ListMedia): MediaFacts {
  return {
    mediaObserved: media.state !== null && media.state !== 'NOT_OBSERVED' && media.state !== 'UNKNOWN',
    hasImagery: media.imageCount > 0 || media.coverCandidateCount > 0 || media.coverState === 'AVAILABLE',
    hasVideo: media.videoCount > 0,
  };
}

/**
 * 五个固定维度。`applies` 决定它算不算分母：确定不可能存在的维度（没有视频的作品没有转录）画成轮廓、
 * 不计入，所以图文作品读 4/4 而不是 4/5；还没看过媒体时仍按「未知」计入——「没检查」不等于「不适用」。
 */
const DIMENSIONS = [
  { key: 'body', label: '正文', lane: 'detail', applies: () => true },
  { key: 'comments', label: '评论', lane: 'comments', applies: () => true },
  { key: 'media', label: '媒体', lane: 'media_bytes', applies: () => true },
  { key: 'ocr', label: '图片文字', lane: 'ocr', applies: (facts: MediaFacts) => !facts.mediaObserved || facts.hasImagery },
  { key: 'asr', label: '转录', lane: 'asr', applies: (facts: MediaFacts) => !facts.mediaObserved || facts.hasVideo },
] as const;

export interface MaterialSegment {
  key: string;
  label: string;
  state: string;
  applicable: boolean;
  fill: Fill;
}

export interface MaterialSummary {
  segments: MaterialSegment[];
  applicable: number;
  /** 全不知道时是 null，而不是系统无法担保的 0。 */
  ready: number | null;
  hasGap: boolean;
  failed: boolean;
  partial: boolean;
  localObjectCount: number;
}

export function laneSummary(lanes: readonly LaneSummary[], lane: string): LaneSummary | null {
  return lanes.find(summary => summary.lane === lane) ?? null;
}

export function materialSummary(item: Pick<WorkItem, 'media' | 'lanes'>): MaterialSummary {
  const facts = mediaFacts(item.media);
  const segments: MaterialSegment[] = DIMENSIONS.map(dimension => {
    const applicable = dimension.applies(facts);
    const state = laneSummary(item.lanes, dimension.lane)?.state ?? 'UNKNOWN';
    return { key: dimension.key, label: dimension.label, state, applicable, fill: applicable ? materialFill(state) : 'not-applicable' };
  });
  const applicable = segments.filter(segment => segment.applicable);
  const ready = applicable.filter(segment => segment.fill === 'ready').length;
  const anyKnown = applicable.some(segment => segment.fill !== 'unknown');
  return {
    segments,
    applicable: applicable.length,
    ready: anyKnown ? ready : null,
    hasGap: anyKnown && ready < applicable.length,
    failed: segments.some(segment => segment.fill === 'failed'),
    partial: segments.some(segment => segment.fill === 'partial'),
    localObjectCount: item.media.localObjectCount,
  };
}

/** 单个维度的状态读数；不适用的维度明说不计入分母。 */
export function segmentReading(segment: MaterialSegment): string {
  return segment.applicable ? stateMeta(segment.state)[0] : '本作品不适用，不计入分母';
}

export function segmentTitle(segment: MaterialSegment): string {
  return `${segment.label}：${segmentReading(segment)}`;
}

/** 侧栏徽标：本次读取 N 个作品，及三个读数（作品总数不是库里的总数——读模型不给总数）。 */
export interface ReadoutFacts {
  works: number;
  commentsRetained: number | null;
  localObjects: number;
  gaps: number;
}

export function readoutFacts(items: readonly Pick<WorkItem, 'media' | 'lanes'>[]): ReadoutFacts {
  let commentsRetained: number | null = null;
  let localObjects = 0;
  let gaps = 0;
  for (const item of items) {
    const comments = laneSummary(item.lanes, 'comments');
    if (comments?.valueState === 'KNOWN' && comments.retained !== null) commentsRetained = (commentsRetained ?? 0) + comments.retained;
    const summary = materialSummary(item);
    localObjects += summary.localObjectCount;
    if (summary.hasGap || summary.failed) gaps += 1;
  }
  return { works: items.length, commentsRetained, localObjects, gaps };
}

export function readoutTitle(facts: ReadoutFacts): string {
  const retained = facts.commentsRetained === null ? '未知' : compactCount(facts.commentsRetained);
  return `本次读取 ${facts.works} 个作品 · 已留存评论 ${retained} · 本地材料 ${facts.localObjects} · 部分或缺口 ${facts.gaps}`;
}

// ───────────────────────── 原声 ─────────────────────────

export interface QuoteParts {
  before: string;
  match: string;
  after: string;
}

/**
 * 检索命中按字符偏移高亮。偏移是字符偏移（CJK 与表情不止一个 JavaScript 下标），所以用 `Array.from`
 * 切片。没有有效命中范围时整段原样返回。
 */
export function quoteParts(fragment: Pick<Fragment, 'text' | 'matchOffset' | 'matchLength'>): QuoteParts | null {
  const characters = Array.from(fragment.text);
  const offset = fragment.matchOffset ?? -1;
  const length = fragment.matchLength ?? 0;
  if (offset < 0 || length <= 0 || offset + length > characters.length) return null;
  return {
    before: characters.slice(0, offset).join(''),
    match: characters.slice(offset, offset + length).join(''),
    after: characters.slice(offset + length).join(''),
  };
}

// ───────────────────────── 检查器事实 ─────────────────────────

export type FactRowData = readonly [label: string, value: string | number, code?: string | null];

/**
 * 互动当前值与变化：状态已知且有当前值才写数字；没有前值写「无可比较前值」，变化未知写「变化未知」——
 * 不把缺失写成 0，也不自己算差。
 */
export function currentMetricRow(label: string, metric: CurrentMetric | null): FactRowData {
  if (metric === null || metric.state !== 'KNOWN' || metric.value === null) return [label, '当前未知', 'UNKNOWN'];
  const when = metric.observedAt ? `（${momentText(metric.observedAt)}）` : '';
  const previous = metric.previous !== null ? `；前值 ${formatNumber(metric.previous)}` : '；无可比较前值';
  const delta = metric.deltaState === 'KNOWN' && metric.delta !== null ? `；变化 ${metric.delta > 0 ? '+' : ''}${formatNumber(metric.delta)}` : '；变化未知';
  return [label, `${formatNumber(metric.value)}${when}${previous}${delta}`, 'KNOWN'];
}

/**
 * 详情字段的来源事实：状态、值与来源引用。正文已在阅读区完整给出，这里只写字数与来源，不再把整段正文
 * 塞进一行事实里。
 */
export function detailFieldRow(label: string, field: DetailField | null, kind: 'text' | 'body' = 'text'): FactRowData {
  if (field === null || field.state === 'UNKNOWN') return [label, '当前未知', 'UNKNOWN'];
  const source = field.sourceRef ?? '来源引用未返回';
  if (field.state === 'SOURCE_TEXT_ONLY') return [label, `仅有来源文本：${field.sourceText ?? '当前未知'}；${source}`, field.state];
  const value = kind === 'body' ? `已知（${Array.from(field.value ?? '').length} 字符）` : (field.value ?? '已知');
  return [label, `${value}；${source}`, field.state];
}

/** 互动时间线某个时点的数字行：只列已知的，写「赞／评／藏／转」。 */
export function timelineCopy(point: TimelinePoint): string {
  const parts = ([['赞', point.like], ['评', point.comment], ['藏', point.collect], ['转', point.share]] as const)
    .filter(([, metric]) => metric.state === 'KNOWN' && metric.value !== null)
    .map(([label, metric]) => `${label} ${formatNumber(metric.value ?? 0)}`);
  return parts.length > 0 ? parts.join(' · ') : '本时点互动字段均为当前未知';
}

export interface TimelineRun {
  copy: string;
  lane: string;
  first: string;
  last: string;
  count: number;
}

/**
 * 这是变化的时间线，所以连续读起来完全相同的观察是同一个事实被观察了几次，而不是几个事实。每一段保留首末两个观察时间，
 * 没有一次观察被丢掉。
 */
export function timelineRuns(points: readonly TimelinePoint[]): TimelineRun[] {
  const runs: TimelineRun[] = [];
  for (const point of points) {
    const copy = timelineCopy(point);
    const lane = (point.sourceLane ?? 'source').toUpperCase();
    const observed = momentText(point.observedAt, '观察时间当前未知');
    const previous = runs[runs.length - 1];
    if (previous && previous.copy === copy && previous.lane === lane) {
      previous.last = observed;
      previous.count += 1;
    } else {
      runs.push({ copy, lane, first: observed, last: observed, count: 1 });
    }
  }
  return runs;
}

/** 通道回执的三行：总数、本次返回、是否截断。 */
export function receiptRows(receipt: Receipt | null): FactRowData[] {
  const number = (value: number | null) => (value === null ? '当前未知' : value);
  return [
    ['总数', number(receipt?.total ?? null), receipt?.total === null || receipt === null ? 'UNKNOWN' : null],
    ['本次返回', number(receipt?.returned ?? null), receipt?.returned === null || receipt === null ? 'UNKNOWN' : null],
    ['是否截断', receipt?.truncated === true ? '是，可继续读取' : receipt?.truncated === false ? '否' : '当前未知'],
  ];
}
