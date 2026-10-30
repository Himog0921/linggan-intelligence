/**
 * 创作者页的查询状态：地址栏里的参数 ⇄ 页面状态 ⇄ 接口参数。
 *
 * 地址只放标识和可公开的筛选值（领域、搜索、筛选、页码）。参数名与接口 `CreatorScope`
 * （`crates/contracts/src/creator_discovery.rs`）一致，也是证据库回链（`creatorFilter`/`returnTo`）
 * 依赖的名字；`page` 与接口一样从 0 开始。无法识别的参数值按默认处理，不让手改的地址把页面带坏。
 */
import type { LeadRule } from './creatorsTypes';

export const PAGE_SIZES = [25, 50, 100] as const;
export const SORT_KEYS = ['recent', 'first_seen', 'related_works', 'high_likes', 'viral_works'] as const;
export const TRAIT_KEYS = ['personal_experience', 'professional_output', 'explicit_promotion', 'institution_or_brand'] as const;
export const OBSERVATION_KEYS = ['outside', 'inside', 'monitoring', 'paused', 'other_domains_only', 'dismissed'] as const;
export const RELEVANCE_KEYS = ['related', 'unknown', 'unrelated'] as const;
export const FOCUS_KEYS = ['vertical_tendency', 'multi_topic', 'unknown'] as const;

export type SortKey = (typeof SORT_KEYS)[number];
export type UsageRole = 'primary' | 'reference';

export interface CreatorQuery {
  usageRole: UsageRole;
  platform: 'xhs';
  searchMode: 'author' | 'work';
  query: string;
  sort: SortKey;
  /** 发表日期范围，`YYYY-MM-DD`，结束日不含；空串表示不限。 */
  publishedFrom: string;
  publishedTo: string;
  traits: string[];
  topicHint: string;
  observation: string;
  minLikes: number | null;
  relevance: string;
  focus: string;
  viral: boolean;
  highLikes: boolean;
  /** 首次发现不早于这一天（北京时间，`YYYY-MM-DD`）；空串表示不限。 */
  firstSeenFrom: string;
  /** 至少有多少篇作品符合当前作品条件；null 表示不限。 */
  minMatchedWorks: number | null;
  /** 从 0 开始。 */
  page: number;
  pageSize: number;
}

export const DEFAULT_QUERY: CreatorQuery = {
  usageRole: 'primary',
  platform: 'xhs',
  searchMode: 'author',
  query: '',
  sort: 'recent',
  publishedFrom: '',
  publishedTo: '',
  traits: [],
  topicHint: '',
  observation: '',
  minLikes: null,
  relevance: '',
  focus: '',
  viral: false,
  highLikes: false,
  firstSeenFrom: '',
  minMatchedWorks: null,
  page: 0,
  pageSize: 50,
};

const DATE = /^\d{4}-\d{2}-\d{2}$/;

function oneOf<T extends string>(allowed: readonly T[], value: string | null, fallback: T): T {
  return allowed.find(item => item === value) ?? fallback;
}

export function parseQuery(params: URLSearchParams): CreatorQuery {
  const traits = new Set((params.get('traits') ?? '').split(',').filter(Boolean));
  const minLikes = params.get('minLikes');
  const minMatchedWorks = params.get('minMatchedWorks');
  const page = params.get('page') ?? '';
  const size = Number(params.get('pageSize'));
  const date = (name: string) => {
    const value = params.get(name) ?? '';
    return DATE.test(value) ? value : '';
  };
  return {
    usageRole: oneOf(['primary', 'reference'], params.get('usageRole'), 'primary'),
    platform: 'xhs',
    searchMode: oneOf(['author', 'work'], params.get('searchMode'), 'author'),
    query: (params.get('query') ?? '').slice(0, 200),
    sort: oneOf(SORT_KEYS, params.get('sort'), 'recent'),
    publishedFrom: date('publishedFrom'),
    publishedTo: date('publishedTo'),
    traits: TRAIT_KEYS.filter(key => traits.has(key)),
    topicHint: (params.get('topicHint') ?? '').trim().slice(0, 120),
    observation: oneOf(['', ...OBSERVATION_KEYS], params.get('observation'), ''),
    minLikes: minLikes !== null && /^\d+$/.test(minLikes) ? Number(minLikes) : null,
    relevance: oneOf(['', ...RELEVANCE_KEYS], params.get('relevance'), ''),
    focus: oneOf(['', ...FOCUS_KEYS], params.get('focus'), ''),
    viral: params.get('viral') === 'true',
    highLikes: params.get('highLikes') === 'true',
    firstSeenFrom: date('firstSeenFrom'),
    minMatchedWorks: minMatchedWorks !== null && /^\d+$/.test(minMatchedWorks) ? Math.min(Number(minMatchedWorks), 1_000) : null,
    page: /^\d+$/.test(page) ? Math.min(Number(page), 1_000_000) : 0,
    pageSize: PAGE_SIZES.find(item => item === size) ?? 50,
  };
}

function setIf(params: URLSearchParams, name: string, value: string | number | boolean | null | undefined) {
  if (value === null || value === undefined || value === '' || value === false) return;
  params.set(name, String(value));
}

/** 页面地址：只写与默认值不同的参数，地址保持干净。 */
export function toPageSearch(domain: string | null, query: CreatorQuery): URLSearchParams {
  const params = new URLSearchParams();
  setIf(params, 'domain', domain);
  if (query.usageRole !== DEFAULT_QUERY.usageRole) params.set('usageRole', query.usageRole);
  if (query.searchMode !== DEFAULT_QUERY.searchMode) params.set('searchMode', query.searchMode);
  setIf(params, 'query', query.query);
  if (query.sort !== DEFAULT_QUERY.sort) params.set('sort', query.sort);
  setIf(params, 'publishedFrom', query.publishedFrom);
  setIf(params, 'publishedTo', query.publishedTo);
  setIf(params, 'traits', query.traits.join(','));
  setIf(params, 'topicHint', query.topicHint);
  setIf(params, 'observation', query.observation);
  setIf(params, 'minLikes', query.minLikes);
  setIf(params, 'relevance', query.relevance);
  setIf(params, 'focus', query.focus);
  setIf(params, 'viral', query.viral);
  setIf(params, 'highLikes', query.highLikes);
  setIf(params, 'firstSeenFrom', query.firstSeenFrom);
  setIf(params, 'minMatchedWorks', query.minMatchedWorks);
  if (query.page > 0) params.set('page', String(query.page));
  if (query.pageSize !== DEFAULT_QUERY.pageSize) params.set('pageSize', String(query.pageSize));
  return params;
}

/** 接口参数：范围与分页总是显式给出，其余只在有值时给。 */
export function toApiParams(domain: string, query: CreatorQuery): URLSearchParams {
  const params = new URLSearchParams({
    domain,
    usageRole: query.usageRole,
    platform: query.platform,
    searchMode: query.searchMode,
    sort: query.sort,
  });
  setIf(params, 'query', query.query);
  setIf(params, 'publishedFrom', query.publishedFrom);
  setIf(params, 'publishedTo', query.publishedTo);
  setIf(params, 'traits', query.traits.join(','));
  setIf(params, 'topicHint', query.topicHint);
  setIf(params, 'observation', query.observation);
  setIf(params, 'minLikes', query.minLikes);
  setIf(params, 'relevance', query.relevance);
  setIf(params, 'focus', query.focus);
  setIf(params, 'viral', query.viral);
  setIf(params, 'highLikes', query.highLikes);
  setIf(params, 'firstSeenFrom', query.firstSeenFrom);
  setIf(params, 'minMatchedWorks', query.minMatchedWorks);
  params.set('page', String(query.page));
  params.set('pageSize', String(query.pageSize));
  return params;
}

// ───────────────────────── 条件：额外条件、条件芯片、快捷视图 ─────────────────────────

/** 「额外条件」：不含领域、材料用途、平台和发表日期这四项范围条件。 */
const EXTRA_DEFAULTS = {
  query: '',
  traits: [] as string[],
  topicHint: '',
  observation: '',
  minLikes: null,
  relevance: '',
  focus: '',
  viral: false,
  highLikes: false,
  firstSeenFrom: '',
  minMatchedWorks: null,
};

export function hasExtraConditions(query: CreatorQuery): boolean {
  return (
    query.query !== '' || query.traits.length > 0 || query.topicHint !== '' || query.observation !== '' ||
    query.minLikes !== null || query.relevance !== '' || query.focus !== '' || query.viral || query.highLikes ||
    query.firstSeenFrom !== '' || query.minMatchedWorks !== null
  );
}

/** 「重置」只清列表条件（搜索、筛选、排序）和页码；领域、材料用途、平台和发表日期保持不动。 */
export function clearExtras(query: CreatorQuery): CreatorQuery {
  return { ...query, ...EXTRA_DEFAULTS, traits: [], sort: 'recent', page: 0 };
}

export type ChipKey = 'query' | 'traits' | 'topicHint' | 'observation' | 'minLikes' | 'relevance' | 'focus' | 'viral' | 'highLikes' | 'firstSeenFrom' | 'minMatchedWorks';

export interface ConditionChip {
  key: ChipKey;
  /** traits 逐项移除时标识具体哪一项。 */
  value?: string;
  text: string;
}

export const LABELS: Record<string, string> = {
  personal_experience: '亲历自述',
  professional_output: '知识与方法讲解',
  explicit_promotion: '推广表达',
  institution_or_brand: '机构／品牌自述',
  vertical_tendency: '本领域持续创作',
  multi_topic: '多主题分享',
  unknown: '尚不能判定',
  related: '已判相关',
  unrelated: '已判无关',
};

export const OBSERVATION_LABELS: Record<string, string> = {
  outside: '未加入本领域',
  inside: '已加入本领域',
  monitoring: '巡查中',
  paused: '已暂停',
  other_domains_only: '仅其他领域有目标',
  dismissed: '已忽略',
};

export function formatCount(value: number | null | undefined): string {
  return value === null || value === undefined || !Number.isFinite(value) ? '—' : value.toLocaleString('zh-CN');
}

export function conditionChips(query: CreatorQuery): ConditionChip[] {
  const chips: ConditionChip[] = [];
  if (query.query) chips.push({ key: 'query', text: `${query.searchMode === 'work' ? '作品' : '作者'}：${query.query}` });
  for (const trait of query.traits) chips.push({ key: 'traits', value: trait, text: LABELS[trait] ?? trait });
  if (query.topicHint) chips.push({ key: 'topicHint', text: `方向：${query.topicHint}` });
  if (query.observation) chips.push({ key: 'observation', text: OBSERVATION_LABELS[query.observation] ?? query.observation });
  if (query.minLikes !== null) chips.push({ key: 'minLikes', text: `最低点赞：${formatCount(query.minLikes)}` });
  if (query.relevance) chips.push({ key: 'relevance', text: LABELS[query.relevance] ?? query.relevance });
  if (query.focus) chips.push({ key: 'focus', text: LABELS[query.focus] ?? query.focus });
  if (query.viral) chips.push({ key: 'viral', text: '有相关爆款' });
  if (query.highLikes) chips.push({ key: 'highLikes', text: '有已知点赞的作品' });
  if (query.firstSeenFrom) chips.push({ key: 'firstSeenFrom', text: `首次发现：${query.firstSeenFrom} 起` });
  if (query.minMatchedWorks !== null) chips.push({ key: 'minMatchedWorks', text: `至少 ${formatCount(query.minMatchedWorks)} 篇命中作品` });
  return chips;
}

/** 单独移除一项条件；回到第一页。 */
export function removeCondition(query: CreatorQuery, chip: ConditionChip): CreatorQuery {
  switch (chip.key) {
    case 'traits':
      return { ...query, traits: query.traits.filter(trait => trait !== chip.value), page: 0 };
    case 'viral':
    case 'highLikes':
      return { ...query, [chip.key]: false, page: 0 };
    case 'minLikes':
      return { ...query, minLikes: null, page: 0 };
    case 'minMatchedWorks':
      return { ...query, minMatchedWorks: null, page: 0 };
    default:
      return { ...query, [chip.key]: '', page: 0 };
  }
}

export type QuickKey = 'all' | 'new' | 'outside' | 'high' | 'inside' | 'vertical' | 'personal';

/** 参与「视图是否选中」的字段：页码与排序不参与，翻页或换排序不会让视图失去选中态。 */
function conditionSignature(query: CreatorQuery): string {
  return [
    query.usageRole, query.publishedFrom, query.publishedTo, query.searchMode, query.query, query.traits.join(','),
    query.topicHint, query.observation, query.minLikes, query.relevance, query.focus, query.viral, query.highLikes,
    query.firstSeenFrom, query.minMatchedWorks,
  ].join('\u0001');
}

/** 批量选择的适用范围：同一批条件内翻页、换排序都保留选择；条件一变就作废，不导出范围外的作者。 */
export function selectionScope(domain: string, query: CreatorQuery): string {
  return `${domain}\u0001${conditionSignature(query)}`;
}

/**
 * 快捷视图：先清掉所有额外条件再套用，所以点击后的结果与统计数字一致（统计只跟范围变化）。
 * 「最近新见」= 北京时间最近 7 个自然日内首次发现，按首次发现排序。
 * 「高赞线索」= 未加入观察，且同一作者至少有 N 篇作品点赞不少于 M；口径由服务器给出，页面不另编标准。
 */
export function applyQuick(query: CreatorQuery, key: QuickKey, leadRule: LeadRule | null, recentFrom: string): CreatorQuery {
  const next = clearExtras(query);
  if (key === 'new') {
    next.firstSeenFrom = recentFrom;
    next.sort = 'first_seen';
  }
  if (key === 'outside' || key === 'high') next.observation = 'outside';
  if (key === 'inside') next.observation = 'inside';
  if (key === 'high' && leadRule !== null) {
    next.minLikes = leadRule.minLikes;
    next.minMatchedWorks = leadRule.minWorks;
    next.sort = 'high_likes';
  }
  if (key === 'personal') next.traits = ['personal_experience'];
  if (key === 'vertical') next.focus = 'vertical_tendency';
  return next;
}

/** 选中态：当前条件与套用该视图后的条件完全一致；没有高赞线索口径时该视图不可选中。 */
export function quickPressed(
  query: CreatorQuery,
  leadRule: LeadRule | null,
  recentFrom: string,
): { all: boolean; recent: boolean; outside: boolean; high: boolean; inside: boolean } {
  const pressed = (key: QuickKey) => {
    if (key === 'high' && leadRule === null) return false;
    return conditionSignature(applyQuick(query, key, leadRule, recentFrom)) === conditionSignature(query);
  };
  return { all: pressed('all'), recent: pressed('new'), outside: pressed('outside'), high: pressed('high'), inside: pressed('inside') };
}

/** 作品层条件：命中作品数、最高赞的含义会随之变成「当前命中」。 */
export function hasWorkFilter(query: CreatorQuery): boolean {
  return (
    query.traits.length > 0 || query.topicHint !== '' || query.relevance !== '' || query.minLikes !== null ||
    query.viral || query.highLikes || (query.query !== '' && query.searchMode === 'work')
  );
}

export function highMode(query: CreatorQuery): boolean {
  return query.sort === 'high_likes' || query.highLikes || query.viral;
}

// ───────────────────────── 日期 ─────────────────────────

/** 以北京时间的今天为终点（含今天）的 N 个自然日；结束日不含，所以是明天。 */
export function presetDates(days: number, now: Date): { from: string; to: string } {
  const formatter = new Intl.DateTimeFormat('en-CA', { timeZone: 'Asia/Shanghai', year: 'numeric', month: '2-digit', day: '2-digit' });
  const today = formatter.format(now);
  const start = new Date(`${today}T00:00:00+08:00`);
  const end = new Date(start);
  end.setUTCDate(end.getUTCDate() + 1);
  start.setUTCDate(start.getUTCDate() - (days - 1));
  return { from: formatter.format(start), to: formatter.format(end) };
}

export function dateRangeError(from: string, to: string): string | null {
  return from !== '' && to !== '' && from >= to ? '结束日期必须晚于开始日期（结束日不含）。' : null;
}

// ───────────────────────── 回链到证据库 ─────────────────────────

/** 证据库 `creatorFilter` 里能带的接口范围字段（与 `CreatorScope` 一一对应，多一个都会被服务器拒绝）。 */
function scopeObject(domain: string, query: CreatorQuery): Record<string, unknown> {
  const scope: Record<string, unknown> = {
    domain,
    usageRole: query.usageRole,
    platform: query.platform,
    searchMode: query.searchMode,
    sort: query.sort,
  };
  const put = (name: string, value: unknown) => {
    if (value !== '' && value !== null && value !== false && value !== undefined) scope[name] = value;
  };
  put('publishedFrom', query.publishedFrom);
  put('publishedTo', query.publishedTo);
  put('query', query.query);
  put('traits', query.traits.join(','));
  put('topicHint', query.topicHint);
  put('relevance', query.relevance);
  put('minLikes', query.minLikes);
  put('viral', query.viral);
  put('highLikes', query.highLikes);
  return scope;
}

export interface LinkContext {
  domain: string;
  query: CreatorQuery;
  /** 回到本页时的地址（路径 + 查询串）。 */
  returnTo: string;
}

type Extra = Record<string, string | boolean | undefined>;

/**
 * 跳到证据库的精确作品集合。带 `domain`、材料用途、发表范围和当前作品条件（`creatorFilter`），
 * 而不是把昵称填进搜索框；`returnTo` 让证据库能回到这里。
 */
export function workLink(context: LinkContext, creatorKey: string | null, extra: Extra = {}, workRef?: string): string {
  const scope: Record<string, unknown> = { ...scopeObject(context.domain, context.query) };
  if (creatorKey !== null) scope.creatorKey = creatorKey;
  for (const [name, value] of Object.entries(extra)) {
    if (value === undefined) delete scope[name];
    else scope[name] = value;
  }
  if (scope.unknownAuthor) {
    for (const name of ['creatorKey', 'query', 'traits', 'topicHint', 'relevance', 'minLikes', 'viral', 'highLikes', 'sort']) delete scope[name];
    scope.searchMode = 'author';
  } else if (scope.searchMode === 'author') {
    delete scope.query;
  }
  const params = new URLSearchParams({ domain: context.domain, creatorFilter: JSON.stringify(scope), returnTo: context.returnTo });
  if (creatorKey !== null) params.set('creatorKey', creatorKey);
  if (workRef) params.set('work', workRef);
  return `/corpus/evidence?${params}`;
}

/** 数量链接：回到「范围内、某种相关性」的作品集合（忽略当前的表达/方向/点赞条件）。 */
export function scopeWorkLink(context: LinkContext, creatorKey: string, relevance: string): string {
  return workLink(context, creatorKey, {
    relevance, query: undefined, traits: undefined, topicHint: undefined, minLikes: undefined, viral: false, highLikes: false,
  });
}

/** 引用的作品可能落在列表当前的日期/表达条件之外：只为这篇作品清掉这些临时条件，保留领域、作者和用途。 */
export function evidenceLink(context: LinkContext, creatorKey: string, workRef: string): string {
  return workLink(context, creatorKey, {
    publishedFrom: undefined, publishedTo: undefined, query: undefined, traits: undefined, topicHint: undefined,
    relevance: undefined, minLikes: undefined, viral: false, highLikes: false, sort: 'recent',
  }, workRef);
}

/** 「作者身份待补」的作品集合。 */
export function unknownAuthorLink(context: LinkContext): string {
  return workLink(context, null, { unknownAuthor: true });
}
