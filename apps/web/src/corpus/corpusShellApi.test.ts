import { afterEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '../shared/api';
import { domainMeta, fetchCorpusShell, parseCorpusShell } from './corpusShellApi';

const D = '00000000-0000-4000-8000-000000000001';

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('语料外壳接口', () => {
  it('解析领域选择器需要的字段，作品数未知保持 null 而不是 0', () => {
    const shell = parseCorpusShell({
      domains: [
        { domainRef: D, name: '领域一', status: 'active', targetCount: 3, sampleCount: 12 },
        { domainRef: 'x', name: '领域二', status: 'paused', targetCount: 0, sampleCount: null },
      ],
      selectedDomain: D,
      collectionState: '观察中',
      ignoredFutureField: true,
    });
    expect(shell.selectedDomain).toBe(D);
    expect(shell.collectionState).toBe('观察中');
    expect(shell.domains[1]?.sampleCount).toBeNull();
  });

  it('必需字段缺失就报「返回内容无法解析」，不拿残缺数据渲染', () => {
    for (const bad of [
      null,
      [],
      { domains: [], selectedDomain: null },
      { domains: [{ name: 'x' }], selectedDomain: null, collectionState: '状态未知' },
      { domains: [], selectedDomain: 3, collectionState: '状态未知' },
    ]) {
      let failure: unknown;
      try {
        parseCorpusShell(bad);
      } catch (error) {
        failure = error;
      }
      expect(failure).toBeInstanceOf(ApiError);
      expect((failure as ApiError).failure).toEqual({ kind: 'invalid_response', status: 200 });
    }
  });

  it('带上领域参数（已编码）；没有选定领域时不带', async () => {
    const fetchMock = vi.fn(async () => new Response(JSON.stringify({ domains: [], selectedDomain: null, collectionState: '状态未知' }), { headers: { 'Content-Type': 'application/json' } }));
    vi.stubGlobal('fetch', fetchMock);
    await fetchCorpusShell(D);
    await fetchCorpusShell(null);
    expect(fetchMock.mock.calls.map(call => (call as unknown as [string])[0])).toEqual([`/api/local/corpus/shell?domain=${D}`, '/api/local/corpus/shell']);
  });

  it('领域说明与 shell.rs 的选择器一致：暂停写「已暂停」，作品数未知写「未知」', () => {
    expect(domainMeta({ domainRef: D, name: 'a', status: 'paused', targetCount: 9, sampleCount: 9 })).toBe('已暂停');
    expect(domainMeta({ domainRef: D, name: 'a', status: 'active', targetCount: 3, sampleCount: 12 })).toBe('3 个目标 · 12 篇作品');
    expect(domainMeta({ domainRef: D, name: 'a', status: 'active', targetCount: 0, sampleCount: null })).toBe('0 个目标 · 未知 篇作品');
  });
});
