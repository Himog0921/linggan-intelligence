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
      focus: 'vertical_tendency', viral: true, highLikes: true, page: 3, pageSize: 25,
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
    const bad = parse('usageRole=x&searchMode=y&sort=zzz&publishedFrom=2026-1-1&publishedTo=hello&traits=a,personal_experience&observation=nope&minLikes=-3&relevance=maybe&focus=q&viral=1&page=-1&pageSize=7&platform=douyin');
    expect(bad).toEqual({ ...DEFAULT_QUERY, traits: ['personal_experience'] });
    expect(parse('page=99999999999').page).toBe(1_000_000);
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
    sort: 'viral_works', page: 4, pageSize: 100,
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
      '作者：开始', '亲历自述', '知识与方法讲解', '方向：晚间练习', '未加入本领域', '最低点赞：100', '已判相关', '本领域持续创作', '有相关爆款', '有高赞线索',
    ]);
    const withoutOne = removeCondition(busy, chips[1] as (typeof chips)[number]);
    expect(withoutOne.traits).toEqual(['professional_output']);
    expect(withoutOne.page).toBe(0);
    expect(withoutOne.observation).toBe('outside');
    expect(removeCondition(busy, chips[5] as (typeof chips)[number]).minLikes).toBeNull();
    expect(removeCondition(busy, chips[8] as (typeof chips)[number]).viral).toBe(false);
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
  it('每次先清额外条件再套用，所以点击后的结果与统计数字一致；范围不动', () => {
    const base = q({ usageRole: 'reference', publishedFrom: '2026-09-16', publishedTo: '2026-10-16', query: '旧词', traits: ['explicit_promotion'], page: 3 });
    expect(applyQuick(base, 'all', true)).toEqual({ ...DEFAULT_QUERY, usageRole: 'reference', publishedFrom: '2026-09-16', publishedTo: '2026-10-16' });
    expect(applyQuick(base, 'outside', true).observation).toBe('outside');
    expect(applyQuick(base, 'personal', true).traits).toEqual(['personal_experience']);
    expect(applyQuick(base, 'vertical', true).focus).toBe('vertical_tendency');
    expect(applyQuick(base, 'outside', true).query).toBe('');
  });

  it('「高赞」：配置了爆款标准用爆款并按爆款排序；没有标准就退回高赞线索，不编一个默认标准', () => {
    expect(applyQuick(q(), 'high', true)).toMatchObject({ observation: 'outside', viral: true, highLikes: false, sort: 'viral_works' });
    expect(applyQuick(q(), 'high', false)).toMatchObject({ observation: 'outside', viral: false, highLikes: true, sort: 'high_likes' });
  });

  it('选中态：全部 = 没有任何额外条件；高赞 = 未加入 + 高赞/爆款；未加入 = 其余的未加入', () => {
    expect(quickPressed(q())).toEqual({ all: true, outside: false, high: false });
    expect(quickPressed(q({ observation: 'outside' }))).toEqual({ all: false, outside: true, high: false });
    expect(quickPressed(q({ observation: 'outside', highLikes: true }))).toEqual({ all: false, outside: false, high: true });
    expect(quickPressed(q({ query: 'a' })).all).toBe(false);
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
