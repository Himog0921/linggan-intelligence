import { describe, expect, it } from 'vitest';
import {
  DEFAULT_QUERY,
  applyQuick,
  clearExtras,
  conditionChips,
  dateRangeError,
  evidenceLink,
  hasExtraConditions,
  hasWorkFilter,
  parseQuery,
  presetDates,
  quickPressed,
  removeCondition,
  scopeWorkLink,
  selectionScope,
  toApiParams,
  toPageSearch,
  unknownAuthorLink,
  workLink,
  type CreatorQuery,
} from './creatorsQuery';

const D = '00000000-0000-4000-8000-000000000001';
const KEY = 'xhs:61';
const q = (patch: Partial<CreatorQuery> = {}): CreatorQuery => ({ ...DEFAULT_QUERY, ...patch });
const parse = (search: string) => parseQuery(new URLSearchParams(search));

describe('地址参数 ⇄ 页面状态', () => {
  it('没有参数就是默认值：主研究、小红书、作者搜索、最近收录、第 0 页、每页 50', () => {
    expect(parse('')).toEqual(DEFAULT_QUERY);
  });

  it('完整往返：写出的地址再读回来是同一个状态，地址只含与默认值不同的参数', () => {
    const state = q({
      usageRole: 'reference', searchMode: 'work', query: '开始', sort: 'high_likes', publishedFrom: '2026-09-16', publishedTo: '2026-10-16',
      traits: ['personal_experience', 'explicit_promotion'], topicHint: '晚间练习', observation: 'outside', minLikes: 100, relevance: 'related',
      focus: 'vertical_tendency', viral: true, highLikes: true, firstSeenFrom: '2026-09-01', minMatchedWorks: 2, page: 3, pageSize: 25,
    });
    const search = toPageSearch(D, state);
    expect(parse(search.toString())).toEqual(state);
    expect(toPageSearch(D, DEFAULT_QUERY).toString()).toBe(`domain=${D}`);
    expect([...toPageSearch(D, q({ query: '开始' })).keys()]).toEqual(['domain', 'query']);
  });

  it('page 与接口一样从 0 开始；0 不写进地址，也不丢', () => {
    expect(toPageSearch(D, q({ page: 0 })).has('page')).toBe(false);
    expect(parse('page=0').page).toBe(0);
    expect(toApiParams(D, q({ page: 0 })).get('page')).toBe('0');
    expect(parse('page=2').page).toBe(2);
  });

  it('手改的无效值按默认处理，不把页面带坏（不会变成接口 400）', () => {
    const bad = parse('usageRole=x&searchMode=y&sort=zzz&publishedFrom=2026-1-1&publishedTo=hello&traits=a,personal_experience&observation=nope&minLikes=-3&relevance=maybe&focus=q&viral=1&page=-1&pageSize=7&platform=douyin&firstSeenFrom=2026-1-1&minMatchedWorks=-3');
    expect(bad).toEqual({ ...DEFAULT_QUERY, traits: ['personal_experience'] });
    expect(parse('page=99999999999').page).toBe(1_000_000);
    expect(parse('minMatchedWorks=999999').minMatchedWorks).toBe(1_000);
    expect(parse(`query=${'x'.repeat(300)}`).query).toHaveLength(200);
  });

  it('表达的顺序固定，重复值只算一次', () => {
    expect(parse('traits=explicit_promotion,personal_experience,personal_experience').traits).toEqual(['personal_experience', 'explicit_promotion']);
  });

  it('接口参数：范围与分页总是显式给出，其余只在有值时给；false 不写', () => {
    const params = toApiParams(D, q({ query: '开始', viral: true }));
    expect(Object.fromEntries(params)).toEqual({
      domain: D, usageRole: 'primary', platform: 'xhs', searchMode: 'author', sort: 'recent', query: '开始', viral: 'true', page: '0', pageSize: '50',
    });
    expect(toApiParams(D, DEFAULT_QUERY).has('viral')).toBe(false);
  });
});

describe('额外条件、条件芯片与「重置」', () => {
  const busy = q({
    usageRole: 'reference', publishedFrom: '2026-09-16', publishedTo: '2026-10-16', query: '开始', traits: ['personal_experience', 'professional_output'],
    topicHint: '晚间练习', observation: 'outside', minLikes: 100, relevance: 'related', focus: 'vertical_tendency', viral: true, highLikes: true,
    firstSeenFrom: '2026-09-01', minMatchedWorks: 2, sort: 'viral_works', page: 4, pageSize: 100,
  });

  it('「重置」只清列表条件与排序、页码；领域范围（用途、日期、平台）和每页数量保持不动（#384 发现 3）', () => {
    const reset = clearExtras(busy);
    expect(reset).toEqual({ ...DEFAULT_QUERY, usageRole: 'reference', publishedFrom: '2026-09-16', publishedTo: '2026-10-16', pageSize: 100 });
    expect(hasExtraConditions(reset)).toBe(false);
    expect(hasExtraConditions(busy)).toBe(true);
  });

  it('每个已选条件一个芯片，表达逐项；移除某一项只动这一项并回到第一页', () => {
    const chips = conditionChips(busy);
    expect(chips.map(chip => chip.text)).toEqual([
      '作者：开始', '亲历自述', '知识与方法讲解', '方向：晚间练习', '未加入本领域', '最低点赞：100', '已判相关', '本领域持续创作',
      '有相关爆款', '有已知点赞的作品', '首次发现：2026-09-01 起', '至少 2 篇命中作品',
    ]);
    const chip = (key: string) => chips.find(candidate => candidate.key === key) as (typeof chips)[number];
    const withoutOne = removeCondition(busy, chips[1] as (typeof chips)[number]);
    expect(withoutOne.traits).toEqual(['professional_output']);
    expect(withoutOne.page).toBe(0);
    expect(withoutOne.observation).toBe('outside');
    expect(removeCondition(busy, chip('minLikes')).minLikes).toBeNull();
    expect(removeCondition(busy, chip('viral')).viral).toBe(false);
    expect(removeCondition(busy, chip('firstSeenFrom')).firstSeenFrom).toBe('');
    expect(removeCondition(busy, chip('minMatchedWorks')).minMatchedWorks).toBeNull();
    expect(removeCondition(busy, chips[0] as (typeof chips)[number]).query).toBe('');
  });

  it('搜索芯片按搜索范围写「作者」或「作品」', () => {
    expect(conditionChips(q({ query: 'a', searchMode: 'work' }))[0]?.text).toBe('作品：a');
  });

  it('作品层条件：命中数与最高赞的含义随之变成「当前命中」', () => {
    expect(hasWorkFilter(q())).toBe(false);
    expect(hasWorkFilter(q({ query: 'a' }))).toBe(false);
    expect(hasWorkFilter(q({ query: 'a', searchMode: 'work' }))).toBe(true);
    expect(hasWorkFilter(q({ relevance: 'related' }))).toBe(true);
    expect(hasWorkFilter(q({ focus: 'unknown' }))).toBe(false);
  });
});

describe('快捷视图', () => {
  const LEAD = { minLikes: 500, minWorks: 2 };
  const RECENT = '2026-10-09';

  it('每次先清额外条件再套用，所以点击后的结果与统计数字一致；范围不动', () => {
    const base = q({ usageRole: 'reference', publishedFrom: '2026-09-16', publishedTo: '2026-10-16', query: '旧词', traits: ['explicit_promotion'], page: 3 });
    expect(applyQuick(base, 'all', LEAD, RECENT)).toEqual({ ...DEFAULT_QUERY, usageRole: 'reference', publishedFrom: '2026-09-16', publishedTo: '2026-10-16' });
    expect(applyQuick(base, 'outside', LEAD, RECENT).observation).toBe('outside');
    expect(applyQuick(base, 'inside', LEAD, RECENT).observation).toBe('inside');
    expect(applyQuick(base, 'personal', LEAD, RECENT).traits).toEqual(['personal_experience']);
    expect(applyQuick(base, 'vertical', LEAD, RECENT).focus).toBe('vertical_tendency');
    expect(applyQuick(base, 'outside', LEAD, RECENT).query).toBe('');
  });

  it('「最近新见」= 北京时间最近 7 天首次发现，并按首次发现排序', () => {
    expect(applyQuick(q({ observation: 'outside' }), 'new', LEAD, RECENT)).toMatchObject({ firstSeenFrom: RECENT, sort: 'first_seen', observation: '' });
  });

  it('「高赞线索」用服务器给的口径：未加入观察 + 至少 N 篇达到 M 赞；没有口径时不编默认值', () => {
    expect(applyQuick(q(), 'high', LEAD, RECENT)).toMatchObject({
      observation: 'outside', minLikes: 500, minMatchedWorks: 2, sort: 'high_likes', viral: false, highLikes: false,
    });
    expect(applyQuick(q(), 'high', null, RECENT)).toMatchObject({ observation: 'outside', minLikes: null, minMatchedWorks: null });
  });

  it('选中态：条件（不看页码与排序）与套用视图后完全一致才算选中', () => {
    expect(quickPressed(q(), LEAD, RECENT)).toEqual({ all: true, recent: false, outside: false, high: false, inside: false });
    expect(quickPressed(q({ observation: 'outside', page: 3, sort: 'related_works' }), LEAD, RECENT)).toEqual({ all: false, recent: false, outside: true, high: false, inside: false });
    expect(quickPressed(q({ firstSeenFrom: RECENT }), LEAD, RECENT)).toEqual({ all: false, recent: true, outside: false, high: false, inside: false });
    expect(quickPressed(q({ observation: 'outside', minLikes: 500, minMatchedWorks: 2, sort: 'high_likes' }), LEAD, RECENT)).toEqual({ all: false, recent: false, outside: false, high: true, inside: false });
    expect(quickPressed(q({ observation: 'inside' }), LEAD, RECENT)).toMatchObject({ inside: true });
    expect(quickPressed(q({ query: 'a' }), LEAD, RECENT).all).toBe(false);
  });

  it('没有高赞线索口径时，即使条件吻合也不把「高赞线索」标成选中', () => {
    expect(quickPressed(q({ observation: 'outside', minLikes: 500, minMatchedWorks: 2 }), null, RECENT).high).toBe(false);
  });
});

describe('批量选择范围', () => {
  it('翻页与换排序不清空选择；条件或领域一变就换一个范围', () => {
    const base = q({ page: 2, sort: 'high_likes' });
    expect(selectionScope(D, base)).toBe(selectionScope(D, q({ page: 0, sort: 'recent' })));
    expect(selectionScope(D, q())).not.toBe(selectionScope(D, q({ observation: 'outside' })));
    expect(selectionScope(D, q())).not.toBe(selectionScope('00000000-0000-4000-8000-000000000002', q()));
  });
});

describe('发表日期', () => {
  it('近 N 天 = 以北京时间今天为终点的 N 个自然日，结束日不含（所以是明天）', () => {
    const now = new Date('2026-10-15T17:30:00Z'); // 北京时间 10-16 01:30
    expect(presetDates(30, now)).toEqual({ from: '2026-09-17', to: '2026-10-17' });
    expect(presetDates(1, now)).toEqual({ from: '2026-10-16', to: '2026-10-17' });
    expect(presetDates(90, new Date('2026-03-01T00:00:00Z'))).toEqual({ from: '2025-12-02', to: '2026-03-02' });
  });

  it('结束日必须晚于开始日', () => {
    expect(dateRangeError('2026-10-16', '2026-10-16')).toMatch(/结束日期必须晚于开始日期/);
    expect(dateRangeError('2026-10-17', '2026-10-16')).not.toBeNull();
    expect(dateRangeError('2026-10-15', '2026-10-16')).toBeNull();
    expect(dateRangeError('', '2026-10-16')).toBeNull();
  });
});

describe('回链证据库：创作者的精确作品集合，而不是把昵称填进搜索框', () => {
  const context = (query: CreatorQuery) => ({ domain: D, query, returnTo: `/corpus/creators?domain=${D}&page=2` });
  const read = (href: string) => {
    const url = new URL(href, 'http://x');
    return { path: url.pathname, params: url.searchParams, filter: JSON.parse(url.searchParams.get('creatorFilter') ?? 'null') as Record<string, unknown> };
  };

  it('带领域、创作者身份、材料用途、发表范围与返回地址；不含分页、观察关系和作者倾向', () => {
    const { path, params, filter } = read(workLink(context(q({ usageRole: 'reference', publishedFrom: '2026-09-16', publishedTo: '2026-10-16', observation: 'outside', focus: 'unknown', page: 2 })), KEY));
    expect(path).toBe('/corpus/evidence');
    expect(params.get('domain')).toBe(D);
    expect(params.get('creatorKey')).toBe(KEY);
    expect(params.get('returnTo')).toBe(`/corpus/creators?domain=${D}&page=2`);
    expect(filter).toEqual({
      domain: D, usageRole: 'reference', platform: 'xhs', searchMode: 'author', sort: 'recent', publishedFrom: '2026-09-16', publishedTo: '2026-10-16', creatorKey: KEY,
    });
  });

  it('作者搜索词是作者层条件，不带去证据库；作品搜索词作为同一篇作品的条件带上', () => {
    expect(read(workLink(context(q({ query: '昵称' })), KEY)).filter.query).toBeUndefined();
    expect(read(workLink(context(q({ query: '正文词', searchMode: 'work' })), KEY)).filter).toMatchObject({ query: '正文词', searchMode: 'work' });
  });

  it('数量链接：回到范围内某种相关性的作品，忽略当前的表达、方向、点赞条件', () => {
    const { filter } = read(scopeWorkLink(context(q({ traits: ['personal_experience'], topicHint: 'x', minLikes: 5, viral: true, highLikes: true })), KEY, 'unknown'));
    expect(filter).toMatchObject({ relevance: 'unknown', viral: false, highLikes: false, creatorKey: KEY });
    expect(filter.traits).toBeUndefined();
    expect(filter.topicHint).toBeUndefined();
    expect(filter.minLikes).toBeUndefined();
  });

  it('引用的作品落在当前日期/表达条件之外时：只为这篇作品清掉临时条件，保留领域、作者和用途', () => {
    const { params, filter } = read(evidenceLink(context(q({ usageRole: 'reference', publishedFrom: '2026-09-16', publishedTo: '2026-10-16', traits: ['personal_experience'], relevance: 'related' })), KEY, '10000000-0000-4000-8000-000000000001'));
    expect(params.get('work')).toBe('10000000-0000-4000-8000-000000000001');
    expect(filter).toMatchObject({ domain: D, usageRole: 'reference', creatorKey: KEY, sort: 'recent' });
    for (const gone of ['publishedFrom', 'publishedTo', 'traits', 'relevance', 'query', 'topicHint', 'minLikes']) expect(filter[gone], gone).toBeUndefined();
  });

  it('作者身份待补的作品集合：不带创作者、搜索、表达、排序，搜索范围固定为作者', () => {
    const { params, filter } = read(unknownAuthorLink(context(q({ query: '词', searchMode: 'work', traits: ['personal_experience'], sort: 'high_likes', publishedFrom: '2026-09-16' }))));
    expect(params.has('creatorKey')).toBe(false);
    expect(filter).toMatchObject({ domain: D, unknownAuthor: true, searchMode: 'author', publishedFrom: '2026-09-16' });
    for (const gone of ['creatorKey', 'query', 'traits', 'sort', 'relevance']) expect(filter[gone], gone).toBeUndefined();
  });
});
