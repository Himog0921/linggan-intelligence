import { afterEach, describe, expect, it, vi } from 'vitest';
import { ApiError, requestBinaryJson, requestJson } from './api';

function respond(status: number, body: string) {
  return vi.fn<typeof fetch>(async () => new Response(body, { status, headers: { 'Content-Type': 'application/json' } }));
}

afterEach(() => {
  vi.unstubAllGlobals();
});

async function rejection(promise: Promise<unknown>): Promise<ApiError> {
  try {
    await promise;
  } catch (error) {
    if (error instanceof ApiError) return error;
    throw error;
  }
  throw new Error('expected the request to fail');
}

describe('requestJson', () => {
  it('同源请求，不带缓存，写操作带 JSON 正文', async () => {
    const fetchMock = respond(200, '{"ok":true}');
    vi.stubGlobal('fetch', fetchMock);
    await expect(requestJson('/api/x', { method: 'POST', body: { a: 1 } })).resolves.toEqual({ ok: true });
    const [path, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
    expect(path).toBe('/api/x');
    expect(init.method).toBe('POST');
    expect(init.cache).toBe('no-store');
    expect(init.credentials).toBe('same-origin');
    expect(init.body).toBe('{"a":1}');
  });

  it('拒绝跨源或协议相对地址', async () => {
    await expect(requestJson('https://example.com/api')).rejects.toThrow('只允许同源相对路径');
    await expect(requestJson('//example.com/api')).rejects.toThrow('只允许同源相对路径');
    await expect(requestJson('/\\example.com/api')).rejects.toThrow('只允许同源相对路径');
  });

  it('非 2xx 是服务器的明确答复，带回合同错误码', async () => {
    vi.stubGlobal('fetch', respond(409, '{"error":"model_revision_conflict"}'));
    const error = await rejection(requestJson('/api/x'));
    expect(error.failure).toEqual({ kind: 'http', status: 409, code: 'model_revision_conflict' });
    expect(error.answered).toBe(true);
  });

  it('非 2xx 且没有错误码时，错误码为 null，不把失败变成空数据', async () => {
    vi.stubGlobal('fetch', respond(500, 'internal'));
    const error = await rejection(requestJson('/api/x'));
    expect(error.failure).toEqual({ kind: 'http', status: 500, code: null });
  });

  it('没有得到答复时是「结果未确认」', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => Promise.reject(new TypeError('Failed to fetch'))));
    const error = await rejection(requestJson('/api/x', { method: 'POST', body: {} }));
    expect(error.failure).toEqual({ kind: 'network' });
    expect(error.answered).toBe(false);
  });

  it('创作冲突保留版本回执供业务解码，读取错误保留现行 code', async () => {
    const receipt = { requestId: 'original-request', outcome: 'stale', revision: { id: 'current' } };
    vi.stubGlobal('fetch', respond(409, JSON.stringify({ error: { code: 'creation_stale' }, receipt })));
    const conflict = await rejection(requestJson('/ops/api/creation/works/one/save', { method: 'POST', body: {} }));
    expect(conflict.failure).toEqual({ kind: 'http', status: 409, code: 'creation_stale' });
    expect(conflict.responseBody).toEqual({ error: { code: 'creation_stale' }, receipt });
    vi.stubGlobal('fetch', respond(503, '{"operation":"local_read","outcome":"unavailable","code":"creation_unavailable"}'));
    expect((await rejection(requestJson('/ops/api/creation/works'))).failure)
      .toEqual({ kind: 'http', status: 503, code: 'creation_unavailable' });
  });

  it('2xx JSON 文本也必须带 JSON 内容类型', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => new Response('{"ok":true}', { headers: { 'Content-Type': 'text/html' } })));
    expect((await rejection(requestJson('/api/x'))).failure).toEqual({ kind: 'invalid_response', status: 200 });
  });

  it('2xx 但内容不是 JSON 时报无法解析', async () => {
    vi.stubGlobal('fetch', respond(200, '<html>'));
    const error = await rejection(requestJson('/api/x'));
    expect(error.failure).toEqual({ kind: 'invalid_response', status: 200 });
  });

  it('主动取消原样抛出，不当成失败', async () => {
    const controller = new AbortController();
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => {
        controller.abort();
        throw new DOMException('aborted', 'AbortError');
      }),
    );
    const error: unknown = await requestJson('/api/x', { signal: controller.signal }).catch((caught: unknown) => caught);
    expect(error).toBeInstanceOf(DOMException);
  });
});

describe('requestBinaryJson', () => {
  it('原样传送图片字节和原请求身份，PUT 不变成 JSON', async () => {
    const fetchMock = respond(200, '{"assetId":"asset"}');
    vi.stubGlobal('fetch', fetchMock);
    const bytes = new Blob(['png-bytes'], { type: 'image/png' });
    await expect(requestBinaryJson('/ops/api/creation/assets', bytes, {
      method: 'PUT', contentType: 'image/png', requestId: 'stable-request',
    })).resolves.toEqual({ assetId: 'asset' });
    expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({ method: 'PUT', cache: 'no-store', credentials: 'same-origin',
      headers: { Accept: 'application/json', 'Content-Type': 'image/png', 'x-creation-request-id': 'stable-request' }, body: bytes });
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it('二进制请求断网保持未确认且不重试', async () => {
    const fetchMock = vi.fn(async () => { throw new TypeError('offline'); });
    vi.stubGlobal('fetch', fetchMock);
    const error = await rejection(requestBinaryJson('/ops/api/creation/assets', new Blob(['bytes']), {
      method: 'POST', contentType: 'image/png', requestId: 'stable-request',
    }));
    expect(error.answered).toBe(false);
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });
});
