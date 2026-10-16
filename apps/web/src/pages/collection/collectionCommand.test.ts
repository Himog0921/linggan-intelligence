import { afterEach, expect, it, vi } from 'vitest';
import { ApiError } from '../../shared/api';
import { executeCollectionCommand, readCollectionCommandReceipt } from './collectionCommand';
function json(value: unknown, status = 200) { return new Response(JSON.stringify(value), { status, headers: { 'Content-Type': 'application/json' } }); }
afterEach(() => vi.unstubAllGlobals());
it('同身份并发点击只发一次，失联后只读回执，待确认不是成功', async () => {
  const id = crypto.randomUUID(); const action = 'domain-save';
  const fetch = vi.fn().mockResolvedValueOnce(json({ epoch: crypto.randomUUID() })).mockRejectedValueOnce(new TypeError('offline')).mockResolvedValueOnce(json({ requestRef: id, action, outcome: 'pending', code: null, location: null }, 202));
  vi.stubGlobal('fetch', fetch);
  const first = executeCollectionCommand(action, { name: '合成领域' }, id); const second = executeCollectionCommand(action, { name: '合成领域' }, id);
  expect((await Promise.allSettled([first, second])).every(r => r.status === 'rejected')).toBe(true);
  await expect(executeCollectionCommand(action, { name: '合成领域' }, id)).rejects.toBeInstanceOf(ApiError);
  expect(fetch.mock.calls.filter(c => c[1].method === 'POST')).toHaveLength(1);
});
it('明确拒绝的查询回执可以结清；响应身份不匹配不能冒认成功', async () => {
  const id = crypto.randomUUID(); const action = 'domain-save';
  const rejected = { requestRef: id, action, outcome: 'rejected', code: 'invalid_name', location: null };
  vi.stubGlobal('fetch', vi.fn().mockResolvedValueOnce(json(rejected, 409)).mockResolvedValueOnce(json({ ...rejected, requestRef: crypto.randomUUID(), outcome: 'applied' })));
  await expect(readCollectionCommandReceipt(action, id)).resolves.toEqual(rejected);
  await expect(readCollectionCommandReceipt(action, id)).rejects.toBeInstanceOf(ApiError);
});
it('读取发送上下文失败可确认未提交，之后可以重新尝试，不锁死为未知', async () => {
  const id = crypto.randomUUID(); const action = 'domain-save';
  const fetch = vi.fn().mockRejectedValueOnce(new TypeError('offline')).mockResolvedValueOnce(json({ epoch: crypto.randomUUID() })).mockResolvedValueOnce(json({ requestRef: id, action, outcome: 'applied', code: null, location: null }));
  vi.stubGlobal('fetch', fetch);
  await expect(executeCollectionCommand(action, {}, id)).rejects.toMatchObject({ failure: { code: 'command_not_sent' } });
  await expect(executeCollectionCommand(action, {}, id)).resolves.toMatchObject({ outcome: 'applied' });
  expect(fetch.mock.calls.filter(c => c[1].method === 'POST')).toHaveLength(1);
});
