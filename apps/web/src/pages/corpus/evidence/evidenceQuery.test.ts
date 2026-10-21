import { describe, expect, it } from 'vitest';
import {
  DEFAULT_QUERY,
  LANES,
  LANE_STATES,
  VIEW_FILTERS,
  activeFilterCount,
  collectionReturnHref,
  creatorsReturnHref,
  hasCreatorScope,
  parseQuery,
  scopeProblem,
  toApiParams,
  toPageSearch,
  withoutScope,
  type EvidenceQuery,
} from './evidenceQuery';
import { DOMAIN, workRef } from './evidenceFixtures';

const parse = (search: string) => parseQuery(new URLSearchParams(search));

describe('地址 → 查询状态', () => {
  it('空地址就是默认值：后退到不带参数的地址时不保留旧筛选', () => {
    expect(parse('')).toEqual(DEFAULT_QUERY);
  });

  it('合法值原样读入；无法识别的值按默认处理，不会上送', () => {
    const query = parse(`q=两分钟&lane=comments&laneState=PARTIAL&mediaKind=video&view=risk&layout=cover&tab=dossier&panel=wide&work=${workRef(3)}`);
    expect(query).toMatchObject({ q: '两分钟', lane: 'comments', laneState: 'PARTIAL', mediaKind: 'video', view: 'risk', layout: 'cover', tab: 'dossier', panel: 'wide', work: workRef(3) });
    const junk = parse('lane=bogus&laneState=nope&mediaKind=audio&view=all2&layout=grid&tab=x&panel=huge&work=not-a-uuid');
    expect(junk).toEqual(DEFAULT_QUERY);
    expect(toApiParams(DOMAIN, junk).toString()).toBe(`domain=${DOMAIN}`);
  });

  it('DR-1：旧页的「相关度」排序、近 7／30 天窗口、旧 OCR 已停用状态不再被识别，也不会上送', () => {
    const query = parse('sort=relevance&window=last_7_days&laneState=RETIRED');
    expect(query).toEqual(DEFAULT_QUERY);
    const search = toApiParams(DOMAIN, query).toString();
    expect(search).not.toMatch(/sort|window|RETIRED/);
    expect(LANE_STATES.map(([key]) => key)).not.toContain('RETIRED');
    expect(toPageSearch(DOMAIN, query).toString()).toBe(`domain=${DOMAIN}`);
  });

  it('旧详情标签 URL 迁移到研究底稿或采集记录，不丢失 deep link', () => {
    expect(parse('tab=evidence').tab).toBe('dossier');
    expect(parse('tab=materials').tab).toBe('trace');
    expect(parse('tab=trace').tab).toBe('trace');
  });
  it('筛选项表：9 个通道、14 个状态', () => {
    expect(LANES).toHaveLength(9);
    expect(LANE_STATES).toHaveLength(14);
  });

  it('work 只采用合法 UUID；panel=closed 与三档宽度，缺省是 null（取本机偏好）', () => {
    expect(parse('work=10000000-0000-4000-8000-000000000001').work).toBe(workRef(1));
    expect(parse('work=123').work).toBeNull();
    expect(parse('panel=closed').panel).toBe('closed');
    expect(parse('').panel).toBeNull();
  });

  it('创作者集合与采集运行集合：存在即成立（参数可以是空串），原样读入', () => {
    const query = parse('creatorKey=xhs%3A41&creatorFilter=%7B%7D&returnTo=%2Fcorpus%2Fcreators%3Fdomain%3Dx&collectionRunRef=r');
    expect(query).toMatchObject({ creatorKey: 'xhs:41', creatorFilter: '{}', returnTo: '/corpus/creators?domain=x', collectionRunRef: 'r' });
    expect(hasCreatorScope(query)).toBe(true);
    expect(hasCreatorScope(parse('creatorKey='))).toBe(true);
    expect(hasCreatorScope(DEFAULT_QUERY)).toBe(false);
  });
});

describe('查询状态 → 页面地址', () => {
  it('只写与默认值不同的参数；domain 总是原样写回（领域不能选一次、下一次交互即失效）', () => {
    expect(toPageSearch(DOMAIN, DEFAULT_QUERY).toString()).toBe(`domain=${DOMAIN}`);
    expect(toPageSearch(null, DEFAULT_QUERY).toString()).toBe('');
    const full: EvidenceQuery = { ...DEFAULT_QUERY, q: ' 两分钟 ', lane: 'ocr', laneState: 'QUEUED', mediaKind: 'image', view: 'cleaned', layout: 'table', work: workRef(2), tab: 'trace', panel: 'focus' };
    const search = toPageSearch(DOMAIN, full);
    expect(search.get('q')).toBe('两分钟');
    expect(search.get('view')).toBe('cleaned');
    expect(search.get('layout')).toBe('table');
    expect(search.get('tab')).toBe('trace');
    expect(search.get('panel')).toBe('focus');
    expect(parseQuery(search)).toEqual({ ...full, q: '两分钟' });
  });

  it('显式选过的「标准」宽度也写回：缺省才取本机偏好', () => {
    expect(toPageSearch(DOMAIN, { ...DEFAULT_QUERY, panel: 'normal' }).get('panel')).toBe('normal');
    expect(toPageSearch(DOMAIN, { ...DEFAULT_QUERY, panel: null }).has('panel')).toBe(false);
  });

  it('创作者集合、采集运行集合与 returnTo 原样往返', () => {
    const query: EvidenceQuery = { ...DEFAULT_QUERY, creatorKey: 'xhs:41', creatorFilter: '{"domain":"d"}', returnTo: '/corpus/creators?domain=d', collectionRunRef: workRef(9) };
    expect(parseQuery(toPageSearch(DOMAIN, query))).toEqual(query);
  });
});

describe('查询状态 → 接口参数', () => {
  it('domain 必带；returnTo 不上送（接口忽略它）；游标只在续读时给', () => {
    const query: EvidenceQuery = { ...DEFAULT_QUERY, q: '关键词', creatorKey: 'xhs:41', creatorFilter: '{}', returnTo: '/corpus/creators?x', collectionRunRef: workRef(9) };
    const params = toApiParams(DOMAIN, query, 'cursor-1');
    expect(params.get('domain')).toBe(DOMAIN);
    expect(params.get('q')).toBe('关键词');
    expect(params.get('cursor')).toBe('cursor-1');
    expect(params.get('creatorKey')).toBe('xhs:41');
    expect(params.get('collectionRunRef')).toBe(workRef(9));
    expect(params.has('returnTo')).toBe(false);
    expect(toApiParams(DOMAIN, query).has('cursor')).toBe(false);
  });

  it('系统视图展开为接口条件，并覆盖同名的手选筛选', () => {
    expect(VIEW_FILTERS.partial).toEqual({ laneState: 'PARTIAL' });
    expect(VIEW_FILTERS.risk).toEqual({ laneState: 'RISK_CONTROL' });
    expect(VIEW_FILTERS.cleaned).toEqual({ lane: 'media_bytes', laneState: 'BYTES_CLEANED' });
    expect(VIEW_FILTERS.restricted).toEqual({ restriction: 'WITHDRAWN_OR_RESTRICTED' });
    const params = toApiParams(DOMAIN, { ...DEFAULT_QUERY, lane: 'comments', laneState: 'FAILED', mediaKind: 'cover', view: 'cleaned' });
    expect(params.get('lane')).toBe('media_bytes');
    expect(params.get('laneState')).toBe('BYTES_CLEANED');
    expect(params.get('mediaKind')).toBe('cover');
    expect(toApiParams(DOMAIN, { ...DEFAULT_QUERY, view: 'restricted' }).get('restriction')).toBe('WITHDRAWN_OR_RESTRICTED');
  });

  it('筛选徽标数包含手选下拉和状态预设', () => {
    expect(activeFilterCount(DEFAULT_QUERY)).toBe(0);
    expect(activeFilterCount({ ...DEFAULT_QUERY, lane: 'ocr', mediaKind: 'video', view: 'risk' })).toBe(3);
  });
});

describe('地址里的筛选条件无效（B20）', () => {
  it('collectionRunRef 不是 UUID、creatorFilter 不是 JSON 对象：识别为地址问题，不当成读取失败', () => {
    expect(scopeProblem(DEFAULT_QUERY)).toBeNull();
    expect(scopeProblem({ ...DEFAULT_QUERY, collectionRunRef: 'abc' })).toBe('collectionRunRef');
    expect(scopeProblem({ ...DEFAULT_QUERY, collectionRunRef: workRef(1) })).toBeNull();
    expect(scopeProblem({ ...DEFAULT_QUERY, creatorFilter: '{bad' })).toBe('creatorFilter');
    expect(scopeProblem({ ...DEFAULT_QUERY, creatorFilter: '[1]' })).toBe('creatorFilter');
    expect(scopeProblem({ ...DEFAULT_QUERY, creatorFilter: '{"a":1}' })).toBeNull();
  });

  it('清除无效条件：只去掉出问题的那一项', () => {
    const query: EvidenceQuery = { ...DEFAULT_QUERY, q: '词', creatorKey: 'k', creatorFilter: '{bad', collectionRunRef: 'bad' };
    expect(withoutScope(query, 'collectionRunRef')).toMatchObject({ collectionRunRef: null, creatorFilter: '{bad' });
    expect(withoutScope(query, 'creatorFilter')).toMatchObject({ creatorKey: null, creatorFilter: null, collectionRunRef: 'bad', q: '词' });
  });
});

describe('返回链接', () => {
  it('返回创作者：returnTo 须以 /corpus/creators? 开头，否则回创作者页本身', () => {
    expect(creatorsReturnHref({ ...DEFAULT_QUERY, returnTo: '/corpus/creators?domain=d&page=2' }, 'd')).toBe('/corpus/creators?domain=d&page=2');
    expect(creatorsReturnHref({ ...DEFAULT_QUERY, returnTo: 'https://evil.example/x' }, 'd')).toBe('/corpus/creators?domain=d');
    expect(creatorsReturnHref({ ...DEFAULT_QUERY, returnTo: '//evil.example/corpus/creators?x' }, 'd')).toBe('/corpus/creators?domain=d');
    expect(creatorsReturnHref(DEFAULT_QUERY, null)).toBe('/corpus/creators?domain=');
  });

  it('返回采集任务：returnTo 须以 /collection/tasks? 开头，否则回本次采集的任务页', () => {
    const query = { ...DEFAULT_QUERY, collectionRunRef: workRef(7) };
    expect(collectionReturnHref({ ...query, returnTo: '/collection/tasks?domain=d&run=r' }, 'd')).toBe('/collection/tasks?domain=d&run=r');
    expect(collectionReturnHref({ ...query, returnTo: '/somewhere/else' }, 'd')).toBe(`/collection/tasks?domain=d&run=${workRef(7)}`);
  });
});
