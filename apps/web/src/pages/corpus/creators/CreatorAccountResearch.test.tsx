import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, expect, it, vi } from 'vitest';
import { CreatorAccountResearch } from './CreatorAccountResearch';

const preview = { domainRef: 'domain', platform: 'xhs', authorExternalId: 'author', workRefs: ['work-1', 'work-2', 'work-3'],
  knowledgeCutoff: '2026-10-27T00:00:00Z', inputFingerprint: 'f'.repeat(64), taskConfigRef: 'config', taskRevision: 3 };
function reply(body: unknown) { return new Response(JSON.stringify(body), { status: 200, headers: { 'content-type': 'application/json' } }); }
function mount() {
  return render(<QueryClientProvider client={new QueryClient()}><CreatorAccountResearch domain="domain" platform="xhs" author="author" disabled={false} /></QueryClientProvider>);
}
afterEach(() => vi.unstubAllGlobals());

it('先检查账号样本，再提交并刷新账号研究结果', async () => {
  const calls: { path: string; body: unknown }[] = [];
  vi.stubGlobal('fetch', vi.fn(async (path: string, init?: RequestInit) => {
    calls.push({ path, body: init?.body ? JSON.parse(String(init.body)) : null });
    if (path.includes('/preview?')) return reply(preview);
    if (init?.method === 'POST') return reply({ requestRef: 'request', batchRef: 'batch' });
    return reply({ state: 'succeeded', sourceAvailable: true, failureCode: null });
  }));
  mount();
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: '检查账号样本' }));
  expect(await screen.findByText(/本次固定 3 篇作品/)).toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: '开始账号研究' }));
  expect(await screen.findByText('已提交研究，结果尚未完成。')).toBeInTheDocument();
  expect(calls[1]?.body).toMatchObject({ domainRef: 'domain', platform: 'xhs', authorExternalId: 'author', expectedFingerprint: 'f'.repeat(64), expectedTaskRevision: 3 });
  await user.click(screen.getByRole('button', { name: '刷新研究状态' }));
  expect(await screen.findByText('账号研究已完成，判断和依据已更新。')).toBeInTheDocument();
});

it('回执未知后收起只隐藏面板：重新展开仍用同一请求身份重试', async () => {
  const commands: string[] = [];
  vi.stubGlobal('fetch', vi.fn(async (path: string, init?: RequestInit) => {
    if (path.includes('/preview?')) return reply(preview);
    const body = JSON.parse(String(init?.body)); commands.push(body.commandRef);
    if (commands.length === 1) throw new TypeError('lost response');
    return reply({ requestRef: 'request', batchRef: 'batch' });
  }));
  mount();
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: '检查账号样本' }));
  await user.click(await screen.findByRole('button', { name: '开始账号研究' }));
  expect(await screen.findByText('请求回执未知，可使用同一请求重试。')).toBeInTheDocument();
  expect(screen.queryByRole('button', { name: '检查账号样本' })).toBeNull();
  await user.click(screen.getByRole('button', { name: '收起' }));
  expect(screen.queryByText(/本次固定 3 篇作品/)).toBeNull();
  const reopen = screen.getByRole('button', { name: '展开账号样本研究' });
  expect(reopen).toBeEnabled();
  await user.click(reopen);
  expect(screen.getByText(/本次固定 3 篇作品/)).toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: '重试同一请求' }));
  expect(await screen.findByText('已提交研究，结果尚未完成。')).toBeInTheDocument();
  expect(commands).toHaveLength(2); expect(commands[0]).toBe(commands[1]);
});

it('提交进行中收起，回执到达后重新展开仍保留已接受的研究请求', async () => {
  let release!: () => void;
  const gate = new Promise<void>(resolve => { release = resolve; });
  vi.stubGlobal('fetch', vi.fn(async (path: string) => {
    if (path.includes('/preview?')) return reply(preview);
    await gate;
    return reply({ requestRef: 'request', batchRef: 'batch' });
  }));
  mount();
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: '检查账号样本' }));
  await user.click(await screen.findByRole('button', { name: '开始账号研究' }));
  await user.click(screen.getByRole('button', { name: '收起' }));
  expect(screen.queryByText(/本次固定 3 篇作品/)).toBeNull();
  release();
  await waitFor(() => expect(screen.getByRole('button', { name: '展开账号样本研究' })).toBeEnabled());
  await user.click(screen.getByRole('button', { name: '展开账号样本研究' }));
  expect(screen.getByText('已提交研究，结果尚未完成。')).toBeInTheDocument();
  expect(screen.getByRole('button', { name: '刷新研究状态' })).toBeInTheDocument();
});

it('明确拒绝后收起再展开，可以重新检查样本并用新指纹提交', async () => {
  const posts: { expectedFingerprint: string }[] = [];
  let previews = 0;
  vi.stubGlobal('fetch', vi.fn(async (path: string, init?: RequestInit) => {
    if (path.includes('/preview?')) {
      previews += 1;
      return reply(previews === 1 ? preview : { ...preview, workRefs: ['work-1', 'work-2'], inputFingerprint: 'b'.repeat(64) });
    }
    posts.push(JSON.parse(String(init?.body)));
    if (posts.length === 1) return new Response(JSON.stringify({ reasons: ['creator_publication_withheld'] }), { status: 409, headers: { 'content-type': 'application/json' } });
    return reply({ requestRef: 'request', batchRef: 'batch' });
  }));
  mount();
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: '检查账号样本' }));
  await user.click(await screen.findByRole('button', { name: '开始账号研究' }));
  expect(await screen.findByText('暂不能开始：材料、许可或任务状态已变化。请检查 AI 设置。')).toBeInTheDocument();
  expect(screen.getByRole('button', { name: '开始账号研究' })).toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: '收起' }));
  await user.click(screen.getByRole('button', { name: '展开账号样本研究' }));
  await user.click(screen.getByRole('button', { name: '重新检查样本' }));
  expect(await screen.findByText(/本次固定 2 篇作品/)).toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: '开始账号研究' }));
  expect(await screen.findByText('已提交研究，结果尚未完成。')).toBeInTheDocument();
  expect(previews).toBe(2);
  expect(posts).toHaveLength(2);
  expect(posts[0]?.expectedFingerprint).toBe('f'.repeat(64));
  expect(posts[1]?.expectedFingerprint).toBe('b'.repeat(64));
});

it('样本检查失败后可以重新检查，不需要关闭抽屉', async () => {
  let attempts = 0;
  vi.stubGlobal('fetch', vi.fn(async (path: string) => {
    attempts += 1;
    if (path.includes('/preview?') && attempts === 1) return new Response('{"error":"boom"}', { status: 500 });
    return reply(preview);
  }));
  mount();
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: '检查账号样本' }));
  expect(await screen.findByText('操作未完成，请检查 AI 设置和材料状态后重试。')).toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: '重新检查样本' }));
  expect(await screen.findByText(/本次固定 3 篇作品/)).toBeInTheDocument();
});
