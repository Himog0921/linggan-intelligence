import { afterEach, describe, expect, it, vi } from 'vitest';
import { fetchCommentPage, fetchWorkDetail, fetchWorkList } from './evidenceApi';
import { DOMAIN, commentsJson, detailJson, listItemJson, listJson, workRef } from './evidenceFixtures';
import { DEFAULT_QUERY } from './evidenceQuery';

afterEach(() => {
  vi.unstubAllGlobals();
});

function respond(body: unknown, status = 200) {
  const fetchMock = vi.fn<typeof fetch>(async () => new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } }));
  vi.stubGlobal('fetch', fetchMock);
  return fetchMock;
}

describe('证据库读取', () => {
  it('列表：只发同源 GET，带领域、筛选、续读游标；缓存策略 no-store', async () => {
    const mock = respond(listJson([listItemJson(1)]));
    const list = await fetchWorkList(DOMAIN, { ...DEFAULT_QUERY, q: '词', lane: 'comments' }, 'cur', undefined);
    expect(list.items).toHaveLength(1);
    const [url, init] = mock.mock.calls[0] ?? [];
    expect(url).toBe(`/api/local/work-resources?q=${encodeURIComponent('词')}&lane=comments&cursor=cur&domain=${DOMAIN}`);
    expect(init).toMatchObject({ method: 'GET', cache: 'no-store', credentials: 'same-origin' });
    expect(String(url).startsWith('/')).toBe(true);
  });

  it('详情与评论通道：地址必须是本页接口；续页只加 cursor', async () => {
    const detail = respond(detailJson(1));
    await fetchWorkDetail(`/api/local/work-resources/${workRef(1)}?domain=${DOMAIN}`);
    expect(detail.mock.calls[0]?.[0]).toBe(`/api/local/work-resources/${workRef(1)}?domain=${DOMAIN}`);
    const comments = respond(commentsJson(20, { cursor: 'c2', total: 48 }));
    const page = await fetchCommentPage(`/api/local/work-resources/${workRef(1)}/comments?domain=${DOMAIN}`, 'c1');
    expect(page.nextCursor).toBe('c2');
    expect(comments.mock.calls[0]?.[0]).toBe(`/api/local/work-resources/${workRef(1)}/comments?domain=${DOMAIN}&cursor=c1`);
    await expect(fetchCommentPage('https://evil.example/api/local/work-resources/x/comments', null)).rejects.toThrow('评论通道地址不是本页接口');
  });

  it('服务器答复了错误：保留状态与错误码；纯文本的 400 按「有答复、无错误码」处理', async () => {
    respond({ operation: 'local_read', outcome: 'unavailable', code: 'invalid_material_query_cursor_or_sort' }, 400);
    await expect(fetchWorkList(DOMAIN, DEFAULT_QUERY, null)).rejects.toMatchObject({ failure: { kind: 'http', status: 400, code: 'invalid_material_query_cursor_or_sort' } });
    vi.stubGlobal('fetch', vi.fn(async () => new Response('unknown field `creatorKye`', { status: 400, headers: { 'Content-Type': 'text/plain' } })));
    await expect(fetchWorkList(DOMAIN, DEFAULT_QUERY, null)).rejects.toMatchObject({ failure: { kind: 'http', status: 400, code: null } });
  });

  it('网络失败与看不懂的 2xx 分开：network／invalid_response', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => {
      throw new TypeError('Failed to fetch');
    }));
    await expect(fetchWorkList(DOMAIN, DEFAULT_QUERY, null)).rejects.toMatchObject({ failure: { kind: 'network' } });
    respond({ items: 'not a list' });
    await expect(fetchWorkList(DOMAIN, DEFAULT_QUERY, null)).rejects.toMatchObject({ failure: { kind: 'invalid_response' } });
  });
});
