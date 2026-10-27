import { afterEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '../../../shared/api';
import {
  fetchCreators,
  fetchWorkResources,
  parseCreatorList,
  parseWorkResources,
  registerCreator,
  safeAssetUrl,
  saveOverride,
  savePolicy,
} from './creatorsApi';
import { DEFAULT_QUERY } from './creatorsQuery';
import { DOMAIN, createFetchMock, itemJson, listJson, ok, workJson } from './testKit';

afterEach(() => {
  vi.unstubAllGlobals();
});

function failure(run: () => unknown): ApiError {
  try {
    run();
  } catch (error) {
    if (error instanceof ApiError) return error;
    throw error;
  }
  throw new Error('expected a failure');
}

describe('列表解码（运行时校验）', () => {
  it('解码真实形状：计数、分析状态、观察关系、代表作品', () => {
    const list = parseCreatorList(listJson({ items: [itemJson(1, { authorState: 'queued', authorResult: false })], threshold: 100, analysisEnabled: true }));
    const item = list.items[0];
    expect(list.total).toBe(1);
    expect(list.domain).toMatchObject({ domainRef: DOMAIN, name: '合成领域一', status: 'active' });
    expect(list.policy).toMatchObject({ likeThreshold: 100, revision: 0 });
    expect(item).toMatchObject({ displayName: '合成作者 1', relatedWorkCount: 3, unknownRelevanceWorkCount: 2, followerCount: 2001 });
    expect(item?.authorAnalysis.state).toBe('queued');
    expect(item?.representatives).toHaveLength(2);
    expect(item?.observation).toMatchObject({ lookup: 'available', inCurrentDomain: false });
  });

  it('未知保持未知：缺失的数字是 null，不是 0；观察关系没读到是三态', () => {
    const list = parseCreatorList(listJson({ items: [itemJson(1, { followers: null, maxLikes: null, inDomain: null, works: [workJson(11, { likes: null })] })], outside: null }));
    const item = list.items[0];
    expect(item?.followerCount).toBeNull();
    expect(item?.maxRelatedLikeCount).toBeNull();
    expect(item?.representatives[0]?.likes).toBeNull();
    expect(item?.observation).toMatchObject({ lookup: 'unavailable', inCurrentDomain: null, hasOtherPrimaryDomain: null });
    expect(list.stats.outside).toBeNull();
    expect(list.stats.outsideViral).toBeNull();
    expect(list.policy.likeThreshold).toBeNull();
  });

  it('没有分析记录时服务器给 {}：所有字段都是未知，页面不崩', () => {
    const json = listJson({ items: [itemJson(1)] }) as unknown as { items: { authorAnalysis: unknown; representatives: { analysis: unknown }[] }[] };
    json.items[0]!.authorAnalysis = {};
    json.items[0]!.representatives[0]!.analysis = {};
    const item = parseCreatorList(json).items[0];
    expect(item?.authorAnalysis).toMatchObject({ state: null, resultAt: null, focus: null, sampleWorkCount: null, fragments: [] });
    expect(item?.representatives[0]?.analysis.relevance).toBeNull();
  });

  it('必需字段缺失或类型不对：报「返回内容无法解析」，不拿残缺数据渲染', () => {
    const good = listJson() as Record<string, unknown>;
    for (const mutate of [
      (value: Record<string, unknown>) => delete value.total,
      (value: Record<string, unknown>) => (value.items = 'x'),
      (value: Record<string, unknown>) => delete value.stats,
      (value: Record<string, unknown>) => delete value.domain,
      (value: Record<string, unknown>) => ((value.stats as Record<string, unknown>).authors = '27'),
    ]) {
      const bad = structuredClone(good);
      mutate(bad);
      expect(failure(() => parseCreatorList(bad)).failure).toEqual({ kind: 'invalid_response', status: 200 });
    }
    expect(failure(() => parseCreatorList(null)).failure.kind).toBe('invalid_response');
  });

  it('人工调整的有效值取 value.value；原依据已不可读时带标记', () => {
    const json = listJson({
      items: [itemJson(1, { authorManual: { focus: { value: { value: 'unknown', reason: '当前依据已变化或不可读，请复核；原人工记录仍保留', supportUnavailable: true }, supportUnavailable: true } } })],
    });
    const manual = parseCreatorList(json).items[0]?.authorAnalysis.manual.focus;
    expect(manual).toMatchObject({ judgement: { value: 'unknown' }, supportUnavailable: true });
  });

  it('请求：范围与分页显式带上，领域在其中', async () => {
    const kit = createFetchMock();
    kit.route('GET', '/api/local/creators', ok(listJson()));
    kit.install();
    await fetchCreators(DOMAIN, { ...DEFAULT_QUERY, query: '开始', page: 2 });
    expect(Object.fromEntries(kit.calls[0]!.search)).toMatchObject({ domain: DOMAIN, usageRole: 'primary', platform: 'xhs', query: '开始', page: '2', pageSize: '50' });
  });
});

describe('作品资源与媒体地址', () => {
  it('只接受同源的本机媒体路径；其他任何地址都不当图片用', () => {
    const origin = 'http://127.0.0.1:3000';
    expect(safeAssetUrl('/api/local/media/abc', origin)).toBe('/api/local/media/abc');
    expect(safeAssetUrl('/api/local/derivative/abc?v=1', origin)).toBe('/api/local/derivative/abc?v=1');
    for (const bad of ['https://evil.example/x.png', '//evil.example/api/local/media/x', '/api/local/other/x', '/media/x', null, 3, 'javascript:alert(1)']) {
      expect(safeAssetUrl(bad, origin), String(bad)).toBeNull();
    }
  });

  it('头像只在「可内联」时使用；封面地址同样校验', () => {
    const [resource, restricted] = parseWorkResources({
      items: [
        {
          identity: { publicRef: 'w1' },
          display: { title: '展示标题' },
          media: { avatar: { localAssetUrl: '/api/local/media/a', blob: { deliveryState: 'INLINE_SAFE' } }, cover: { localAssetUrl: '/api/local/derivative/c' } },
        },
        {
          identity: { publicRef: 'w2' },
          display: null,
          media: { avatar: { localAssetUrl: '/api/local/media/a', blob: { deliveryState: 'WITHDRAWN_OR_RESTRICTED' } }, cover: { localAssetUrl: 'https://evil.example/c' } },
        },
      ],
    });
    expect(resource).toEqual({ publicRef: 'w1', title: '展示标题', avatarUrl: '/api/local/media/a', coverUrl: '/api/local/derivative/c' });
    expect(restricted).toEqual({ publicRef: 'w2', title: null, avatarUrl: null, coverUrl: null });
  });

  it('批量读取最多 100 个、去重；没有要读的就不发请求', async () => {
    const kit = createFetchMock();
    kit.route('GET', '/api/local/work-resources', ok({ items: [] }));
    kit.install();
    expect(await fetchWorkResources(DOMAIN, [])).toEqual([]);
    expect(kit.calls).toHaveLength(0);
    const refs = Array.from({ length: 130 }, (_, index) => `10000000-0000-4000-8000-${String(index % 120).padStart(12, '0')}`);
    await fetchWorkResources(DOMAIN, refs);
    expect(kit.calls[0]!.search.get('publicRefs')!.split(',')).toHaveLength(100);
    expect(kit.calls[0]!.search.get('domain')).toBe(DOMAIN);
  });
});

describe('写接口：方法、路径与请求体', () => {
  it('加入观察：POST，按作者键定位，体里只有领域与用途；重试发送的是同一个请求体', async () => {
    const kit = createFetchMock();
    kit.route('POST', '/api/local/creators/xhs%3A61/observation-target', () => 'network', () => ({
      status: 200,
      body: { targetRef: 't1', created: false, relationshipAdded: false, inCurrentDomain: true, usageRole: 'primary', lifecycleState: 'active', monitoringEnabled: false },
    }));
    kit.install();
    const request = { domain: DOMAIN, creatorKey: 'xhs:61', usageRole: 'primary' as const };
    await expect(registerCreator(request)).rejects.toMatchObject({ failure: { kind: 'network' } });
    const receipt = await registerCreator(request);
    expect(receipt).toMatchObject({ created: false, relationshipAdded: false, inCurrentDomain: true });
    const posts = kit.calls.filter(call => call.method === 'POST');
    expect(posts).toHaveLength(2);
    expect(posts[0]!.body).toEqual({ domain: DOMAIN, usageRole: 'primary' });
    expect(posts[1]!.body).toEqual(posts[0]!.body);
  });

  it('发现设置：PUT，带设置版本号；只存标准时不带分析三项', async () => {
    const kit = createFetchMock();
    kit.route('PUT', '/api/local/creator-discovery-policy', ok({ revision: 4, saved: true }));
    kit.install();
    await expect(savePolicy({ domain: DOMAIN, revision: 3, likeThreshold: 100 })).resolves.toEqual({ revision: 4 });
    expect(kit.calls[0]!.body).toEqual({ domain: DOMAIN, revision: 3, likeThreshold: 100 });
    await savePolicy({ domain: DOMAIN, revision: 4, likeThreshold: null });
    expect(kit.calls[1]!.body).toEqual({ domain: DOMAIN, revision: 4, likeThreshold: null });
  });

  it('回执形状不对就是「无法解析」，不报成功', async () => {
    const kit = createFetchMock();
    kit.route('PUT', '/api/local/creator-discovery-policy', ok({ saved: false }));
    kit.route('PUT', '/api/local/creator-discovery-overrides', ok({}));
    kit.install();
    await expect(savePolicy({ domain: DOMAIN, revision: 0, likeThreshold: null })).rejects.toMatchObject({ failure: { kind: 'invalid_response' } });
    await expect(saveOverride({ domain: DOMAIN, creatorKey: 'xhs:61', workRef: null, field: 'focus', value: null, reason: null, supportFragmentIds: [] })).rejects.toMatchObject({
      failure: { kind: 'invalid_response' },
    });
  });

  it('调整判断：路径与请求体', async () => {
    const kit = createFetchMock();
    kit.route('PUT', '/api/local/creator-discovery-overrides', ok({ saved: true }));
    kit.install();
    const override = { domain: DOMAIN, creatorKey: 'xhs:61', workRef: null, field: 'focus', value: 'vertical_tendency', reason: '理由', supportFragmentIds: ['f1'] };
    await saveOverride(override);
    expect(kit.calls[0]!.body).toEqual(override);
  });
});
