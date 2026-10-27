import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, expect, it, vi } from 'vitest';
import { CreatorWorkResearch } from './CreatorWorkResearch';

const preview = { domainRef: 'domain', workRef: 'work', knowledgeCutoff: '2026-10-27T00:00:00Z', inputFingerprint: 'f'.repeat(64), taskConfigRef: 'config', taskRevision: 3,
  coverage: { imageInventory: { expectedPages: null, acquiredPages: 4, processedPages: 0 } } };
function reply(body: unknown) { return new Response(JSON.stringify(body), { status: 200, headers: { 'content-type': 'application/json' } }); }
afterEach(() => vi.unstubAllGlobals());
it('先检查覆盖，再提交并刷新单篇研究结果', async () => {
  const calls: { path: string; body: unknown }[] = [];
  vi.stubGlobal('fetch', vi.fn(async (path: string, init?: RequestInit) => {
    calls.push({ path, body: init?.body ? JSON.parse(String(init.body)) : null });
    if (path.includes('/preview?')) return reply(preview);
    if (init?.method === 'POST') return reply({ requestRef: 'request', batchRef: 'batch' });
    return reply({ state: 'succeeded', sourceAvailable: true, failureCode: null });
  }));
  const client = new QueryClient(); const invalidate = vi.spyOn(client, 'invalidateQueries');
  render(<QueryClientProvider client={client}><CreatorWorkResearch domain="domain" work="work" disabled={false} /></QueryClientProvider>);
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: '检查研究底稿' }));
  expect(await screen.findByText(/图片总数 未知 · 已取得 4 · 已识读 0/)).toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: '开始单篇研究' }));
  expect(await screen.findByText('已提交研究，结果尚未完成。')).toBeInTheDocument();
  expect(screen.getByRole('button', { name: '开始单篇研究' })).toBeDisabled();
  expect(calls[1]?.body).toMatchObject({ domainRef: 'domain', workRef: 'work', expectedFingerprint: 'f'.repeat(64), expectedTaskRevision: 3 });
  await user.click(screen.getByRole('button', { name: '刷新研究状态' }));
  expect(await screen.findByText('单篇研究已完成，判断和依据已更新。')).toBeInTheDocument();
  expect(invalidate).toHaveBeenCalledWith({ queryKey: ['creators'] });
});
it('回执未知时用同一请求身份重试，不能重新准备而重复提交', async () => {
  const commands: string[] = [];
  vi.stubGlobal('fetch', vi.fn(async (path: string, init?: RequestInit) => {
    if (path.includes('/preview?')) return reply(preview);
    const body = JSON.parse(String(init?.body)); commands.push(body.commandRef);
    if (commands.length === 1) throw new TypeError('lost response');
    return reply({ requestRef: 'request', batchRef: 'batch' });
  }));
  render(<QueryClientProvider client={new QueryClient()}><CreatorWorkResearch domain="domain" work="work" disabled={false} /></QueryClientProvider>);
  const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: '检查研究底稿' }));
  await user.click(await screen.findByRole('button', { name: '开始单篇研究' }));
  expect(await screen.findByText('请求回执未知，可使用同一请求重试。')).toBeInTheDocument();
  expect(screen.getByRole('button', { name: '检查研究底稿' })).toBeDisabled();
  await user.click(screen.getByRole('button', { name: '重试同一请求' }));
  expect(await screen.findByText('已提交研究，结果尚未完成。')).toBeInTheDocument();
  expect(commands).toHaveLength(2); expect(commands[0]).toBe(commands[1]);
});
