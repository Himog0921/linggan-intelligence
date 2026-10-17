import { afterEach, describe, expect, it } from 'vitest';
import { BATCH_KEY, createBatch, hasPendingBatch, loadBatch, saveBatch } from './batchStore';
import { DOMAIN, workRef } from './evidenceFixtures';

afterEach(() => sessionStorage.clear());

describe('批量整理断点（L16）', () => {
  it('新批量：去重、保序；空或超过 200 篇不成批', () => {
    expect(createBatch([workRef(1), workRef(2), workRef(1)], DOMAIN)).toEqual({
      domain: DOMAIN,
      items: [
        { work: workRef(1), state: 'pending' },
        { work: workRef(2), state: 'pending' },
      ],
    });
    expect(createBatch([], DOMAIN)).toBeNull();
    expect(createBatch(Array.from({ length: 201 }, (_, index) => workRef(index)), DOMAIN)).toBeNull();
    expect(createBatch(Array.from({ length: 200 }, (_, index) => workRef(index)), DOMAIN)?.items).toHaveLength(200);
  });

  it('存取往返；键名固定；只存引用、状态、计数与修订哈希，不含作品内容', () => {
    const batch = { domain: DOMAIN, items: [{ work: workRef(1), state: 'done' as const, result: { updated: 1, alreadyCurrent: 0, skipped: 0 } }, { work: workRef(2), state: 'pending' as const }] };
    saveBatch(batch);
    expect(BATCH_KEY).toBe('linggan-research-dossier-batch-v1');
    expect(loadBatch()).toEqual(batch);
    expect(sessionStorage.getItem(BATCH_KEY)).not.toMatch(/title|body|text|正文|标题/);
  });

  it('未完成才算有断点：全部完成就没有可继续的', () => {
    expect(hasPendingBatch()).toBe(false);
    saveBatch({ domain: DOMAIN, items: [{ work: workRef(1), state: 'done' }] });
    expect(hasPendingBatch()).toBe(false);
    saveBatch({ domain: DOMAIN, items: [{ work: workRef(1), state: 'done' }, { work: workRef(2), state: 'failed' }] });
    expect(hasPendingBatch()).toBe(true);
  });

  it('形状不对（手改、旧版本、超限、坏 JSON）当作没有，不带病继续', () => {
    for (const bad of ['{bad', 'null', '"x"', JSON.stringify({ domain: 3, items: [] }), JSON.stringify({ domain: DOMAIN, items: [] }), JSON.stringify({ domain: DOMAIN, items: [{ work: 1, state: 'done' }] }), JSON.stringify({ domain: DOMAIN, items: [{ work: 'a', state: 'weird' }] }), JSON.stringify({ domain: DOMAIN, items: Array.from({ length: 201 }, () => ({ work: 'a', state: 'pending' })) })]) {
      sessionStorage.setItem(BATCH_KEY, bad);
      expect(loadBatch(), bad).toBeNull();
    }
  });
});
