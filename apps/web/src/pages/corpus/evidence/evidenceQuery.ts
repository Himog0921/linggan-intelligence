/**
 * 证据库的查询状态：地址栏参数 ⇄ 页面状态 ⇄ 接口参数。
 *
 * 地址是唯一依据，缺省＝默认值：后退到不带参数的地址时不保留旧筛选。无法识别的参数值按默认处理，
 * 不会上送，也不让手改的地址把页面带坏。参数名与接口 `EvidenceQuery`
 * （`crates/contracts/src/discovery/query.rs`）一致，也是创作者页、采集页、插件等入站链接依赖的名字。
 *
 * DR-1（协调方裁定 2026-10-16，真实调用证据见计划 §4.8）：旧页的「相关度」排序、「近 7／30 天来源发布时间」
 * 窗口、「旧 OCR 已停用」状态在读模型里恒无有效结果，已移除；地址里残留的这些旧值按默认处理。
 */
export const LANES = [
  ['discovery', '发现'],
  ['detail', '详情'],
  ['comments', '评论'],
  ['replies', '回复'],
  ['author', '作者'],
  ['media_slots', '媒体槽位'],
  ['media_bytes', '媒体字节'],
  ['ocr', '图片文字'],
  ['asr', '视频转录'],
] as const;

export const LANE_STATES = [
  ['UNKNOWN', '当前未知'],
  ['NOT_REQUESTED', '尚未请求'],
  ['QUEUED', '已排队'],
  ['NOT_OBSERVED', '尚未观察'],
  ['OBSERVED', '已观察'],
  ['PARTIAL', '部分取得'],
  ['ACQUIRED', '已取得'],
  ['PROCESSING', '处理中'],
  ['NOT_ENABLED', '处理器未启用'],
  ['SEARCHABLE', '可检索'],
  ['FAILED', '执行失败'],
  ['RISK_CONTROL', '风险控制停止'],
  ['BYTES_CLEANED', '字节已清理'],
  ['WITHDRAWN_OR_RESTRICTED', '撤回或受限'],
] as const;

export const MEDIA_KINDS = [
  ['cover', '封面'],
  ['image', '正文图片'],
  ['video', '视频'],
  ['live_photo', '实况图片'],
] as const;

export const VIEWS = [
  ['all', '全部材料'],
  ['partial', '部分取得'],
  ['risk', '风险停止'],
  ['cleaned', '媒体已清理'],
  ['restricted', '撤回或受限'],
] as const;

export const LAYOUTS = [
  ['research', '研读'],
  ['table', '表格'],
  ['cover', '封面'],
] as const;

export const TABS = [
  ['overview', '作品'],
  ['dossier', '研究底稿'],
  ['trace', '采集记录'],
] as const;

export const PANELS = ['normal', 'wide', 'focus'] as const;

export type LaneKey = (typeof LANES)[number][0];
export type LaneStateKey = (typeof LANE_STATES)[number][0];
export type MediaKindKey = (typeof MEDIA_KINDS)[number][0];
export type ViewKey = (typeof VIEWS)[number][0];
export type LayoutKey = (typeof LAYOUTS)[number][0];
export type TabKey = (typeof TABS)[number][0];
export type PanelWidth = (typeof PANELS)[number];
/** 地址里的检查器状态：`closed` 或三档宽度；缺省（null）取本机偏好。 */
export type PanelParam = PanelWidth | 'closed' | null;

/** 系统视图展开成的接口条件。它会覆盖同名的手选筛选（旧页既有行为，D2 不改）。 */
export const VIEW_FILTERS: Record<ViewKey, Readonly<Record<string, string>>> = {
  all: {},
  partial: { laneState: 'PARTIAL' },
  risk: { laneState: 'RISK_CONTROL' },
  cleaned: { lane: 'media_bytes', laneState: 'BYTES_CLEANED' },
  restricted: { restriction: 'WITHDRAWN_OR_RESTRICTED' },
};

export interface EvidenceQuery {
  q: string;
  lane: LaneKey | '';
  laneState: LaneStateKey | '';
  mediaKind: MediaKindKey | '';
  view: ViewKey;
  layout: LayoutKey;
  /** 选中作品的稳定引用（只采用合法 UUID）。 */
  work: string | null;
  tab: TabKey;
  panel: PanelParam;
  /** 创作者作品集合：读后原样写回，并随列表请求上送。`has` 语义：参数存在即成立，值可以为空。 */
  creatorKey: string | null;
  creatorFilter: string | null;
  /** 只是导航状态，接口忽略它，不上送。 */
  returnTo: string | null;
  /** 本次采集的固定作品集合。 */
  collectionRunRef: string | null;
}

export const DEFAULT_QUERY: EvidenceQuery = {
  q: '',
  lane: '',
  laneState: '',
  mediaKind: '',
  view: 'all',
  layout: 'research',
  work: null,
  tab: 'overview',
  panel: null,
  creatorKey: null,
  creatorFilter: null,
  returnTo: null,
  collectionRunRef: null,
};

export const UUID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

function oneOf<T extends string>(table: readonly (readonly [T, string])[], value: string | null, fallback: T): T {
  return table.find(([key]) => key === value)?.[0] ?? fallback;
}

export function parseQuery(params: URLSearchParams): EvidenceQuery {
  const work = params.get('work');
  const panel = params.get('panel');
  return {
    q: params.get('q') ?? '',
    lane: oneOf(LANES, params.get('lane'), ''),
    laneState: oneOf(LANE_STATES, params.get('laneState'), ''),
    mediaKind: oneOf(MEDIA_KINDS, params.get('mediaKind'), ''),
    view: oneOf(VIEWS, params.get('view'), 'all'),
    layout: oneOf(LAYOUTS, params.get('layout'), 'research'),
    work: work !== null && UUID_PATTERN.test(work) ? work : null,
    // Legacy tab URLs remain valid; their old content moves to the nearest new panel.
    tab: params.get('tab') === 'evidence' ? 'dossier' : params.get('tab') === 'materials' ? 'trace' : oneOf(TABS, params.get('tab'), 'overview'),
    panel: panel === 'closed' ? 'closed' : (PANELS.find(width => width === panel) ?? null),
    creatorKey: params.get('creatorKey'),
    creatorFilter: params.get('creatorFilter'),
    returnTo: params.get('returnTo'),
    collectionRunRef: params.get('collectionRunRef'),
  };
}

function putIf(params: URLSearchParams, name: string, value: string | null) {
  if (value !== null && value !== '') params.set(name, value);
}

/** 筛选条件不属于「是否有」判断时才需要的辅助：这四个地址参数原样往返。 */
function putScope(params: URLSearchParams, query: EvidenceQuery) {
  putIf(params, 'creatorKey', query.creatorKey);
  putIf(params, 'creatorFilter', query.creatorFilter);
  putIf(params, 'returnTo', query.returnTo);
  putIf(params, 'collectionRunRef', query.collectionRunRef);
}

/**
 * 页面地址：只写与默认值不同的参数。`domain` 总是原样写回——领域由整个语料模块承载，任何一个子页
 * 都无权把它丢掉（否则领域会变成「选一次、下一次交互即失效」）。
 */
export function toPageSearch(domainParam: string | null, query: EvidenceQuery): URLSearchParams {
  const params = new URLSearchParams();
  putIf(params, 'domain', domainParam);
  putIf(params, 'q', query.q.trim());
  putIf(params, 'lane', query.lane);
  putIf(params, 'laneState', query.laneState);
  putIf(params, 'mediaKind', query.mediaKind);
  putScope(params, query);
  if (query.view !== 'all') params.set('view', query.view);
  if (query.layout !== 'research') params.set('layout', query.layout);
  putIf(params, 'work', query.work);
  if (query.tab !== 'overview') params.set('tab', query.tab);
  // 显式选过的宽度（含「标准」）原样写回：缺省才取本机偏好，否则「标准」会在下一次改动后变回偏好值。
  if (query.panel !== null) params.set('panel', query.panel);
  return params;
}

/** 接口参数。系统视图的展开条件最后写入，覆盖同名的手选筛选。 */
export function toApiParams(domain: string, query: EvidenceQuery, cursor: string | null = null): URLSearchParams {
  const params = new URLSearchParams();
  putIf(params, 'q', query.q.trim());
  putIf(params, 'lane', query.lane);
  putIf(params, 'laneState', query.laneState);
  putIf(params, 'mediaKind', query.mediaKind);
  putIf(params, 'creatorKey', query.creatorKey);
  putIf(params, 'creatorFilter', query.creatorFilter);
  putIf(params, 'collectionRunRef', query.collectionRunRef);
  putIf(params, 'cursor', cursor);
  params.set('domain', domain);
  for (const [name, value] of Object.entries(VIEW_FILTERS[query.view])) params.set(name, value);
  return params;
}

/** One indicator for all effective filter choices, including the former quick-view tabs. */
export function activeFilterCount(query: EvidenceQuery): number {
  return [query.lane, query.laneState, query.mediaKind].filter(value => value !== '').length + (query.view === 'all' ? 0 : 1);
}

/** 是否带着创作者作品集合（`creatorKey` 或 `creatorFilter` 存在即成立）。 */
export function hasCreatorScope(query: EvidenceQuery): boolean {
  return query.creatorKey !== null || query.creatorFilter !== null;
}

/**
 * 地址里的筛选条件本身无效（B20）：`collectionRunRef` 不是 UUID，或 `creatorFilter` 不是 JSON 对象。
 * 这是地址的问题，不是读取失败：不发请求，让用户清除即可。
 */
export function scopeProblem(query: EvidenceQuery): 'collectionRunRef' | 'creatorFilter' | null {
  if (query.collectionRunRef !== null && !UUID_PATTERN.test(query.collectionRunRef)) return 'collectionRunRef';
  if (query.creatorFilter !== null) {
    try {
      const parsed: unknown = JSON.parse(query.creatorFilter);
      if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) return 'creatorFilter';
    } catch {
      return 'creatorFilter';
    }
  }
  return null;
}

/** 清掉无效的筛选条件后的地址（保留领域与其他控件状态）。 */
export function withoutScope(query: EvidenceQuery, problem: 'collectionRunRef' | 'creatorFilter'): EvidenceQuery {
  return problem === 'collectionRunRef'
    ? { ...query, collectionRunRef: null }
    : { ...query, creatorKey: null, creatorFilter: null };
}

/** 「返回创作者」的目标：须是同源路径且以 `/corpus/creators?` 开头，否则回创作者页本身。 */
export function creatorsReturnHref(query: EvidenceQuery, domainParam: string | null): string {
  const target = query.returnTo;
  return target !== null && target.startsWith('/corpus/creators?')
    ? target
    : `/corpus/creators?domain=${encodeURIComponent(domainParam ?? '')}`;
}

/** 「返回采集任务」的目标：须以 `/collection/tasks?` 开头，否则回本次采集的任务页。 */
export function collectionReturnHref(query: EvidenceQuery, domainParam: string | null): string {
  const target = query.returnTo;
  return target !== null && target.startsWith('/collection/tasks?')
    ? target
    : `/collection/tasks?domain=${encodeURIComponent(domainParam ?? '')}&run=${encodeURIComponent(query.collectionRunRef ?? '')}`;
}
