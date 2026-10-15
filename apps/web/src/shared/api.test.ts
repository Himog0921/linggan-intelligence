import { afterEach, describe, expect, it, vi } from 'vitest';
import { ApiError, requestJson } from './api';

function respond(status: number, body: string) {
  return vi.fn(async () => new Response(body, { status, headers: { 'Content-Type': 'application/json' } }));
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
