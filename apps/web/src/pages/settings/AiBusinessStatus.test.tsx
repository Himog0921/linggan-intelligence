import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { AiBusinessStatus } from './AiBusinessStatus';
import { AiInvocationHistory } from './AiInvocationHistory';
import { parseAiSettings } from './aiSettingsModel';
import { settingsFixture } from './aiSettingsFixture.test-support';

afterEach(() => vi.unstubAllGlobals());
function renderWithQuery(child: React.ReactNode) {
  return render(<QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>{child}</QueryClientProvider>);
}
function response(body: unknown, status = 200) { return new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } }); }
describe('frozen business AI uses the same control', () => {
  it('reads only the foundation and preserves an explicit unknown when it cannot be read', async () => {
    const fetch = vi.fn(async (path: string) => { expect(path).toBe('/api/local/ai-settings'); return response({ error: 'ai_database_unavailable' }, 503); }); vi.stubGlobal('fetch', fetch);
    renderWithQuery(<AiBusinessStatus taskKey="comment_study" />);
    expect(await screen.findByText('开关读取失败，当前值待核对。')).toBeInTheDocument();
    expect(screen.getByText(/已停用，接入时间另定/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '暂停此项 AI' })).toBeDisabled();
    expect(screen.getByRole('link', { name: '去 AI 设置' })).toHaveAttribute('href', '/settings/models?section=automation&task=comment_study');
    expect(fetch).toHaveBeenCalledTimes(1); expect(fetch.mock.calls[0]?.[0]).toBe('/api/local/ai-settings');
  });
  it('pauses by task CAS and reports a confirmed receipt; no legacy API or queue is used', async () => {
    const base = settingsFixture(); let enabled = true; const posts: Record<string, unknown>[] = [];
    const fetch = vi.fn(async (path: string, init?: RequestInit) => {
      if (init?.method === 'POST') { const body = JSON.parse(String(init.body)); posts.push(body); enabled = false; return response({ scopeKey: 'task:legacy_creator_discovery', revision: 5, enabled: false, commandRef: body.commandRef }); }
      return response({ ...base, tasks: base.tasks.map(t => ({ ...t, taskKey: 'legacy_creator_discovery', label: '旧创作者分析', lifecycle: 'retired', configuration: { ...t.configuration, taskKey: 'legacy_creator_discovery' } })), controls: [{ ...base.controls[0], taskKey: 'legacy_creator_discovery', scope: 'task:legacy_creator_discovery', lifecycle: 'retired', enabled, revision: 4, commandRef: 'old-command' }] });
    }); vi.stubGlobal('fetch', fetch);
    renderWithQuery(<AiBusinessStatus taskKey="legacy_creator_discovery" />);
    const user = userEvent.setup(); await waitFor(() => expect(screen.getByRole('button', { name: '暂停此项 AI' })).toBeEnabled()); await user.click(screen.getByRole('button', { name: '暂停此项 AI' }));
    expect(await screen.findByText('统一工作开关已关闭，未创建新任务。')).toBeInTheDocument();
    expect(posts).toHaveLength(1); expect(posts[0]).toMatchObject({ scope: { kind: 'task', task: 'legacy_creator_discovery' }, expectedRevision: 4, enabled: false });
    await waitFor(() => expect(screen.getByRole('button', { name: '工作开关已关闭' })).toBeDisabled());
    expect(fetch.mock.calls.every(([path]) => path === '/api/local/ai-settings' || path === '/api/local/ai-settings/control')).toBe(true);
    expect(screen.queryByRole('button', { name: /启用|重试/ })).not.toBeInTheDocument();
  });
});
describe('historical invocation ledger', () => {
  it('keeps unknown, known zero and large counts visible and follows the server cursor', async () => {
    const id = '10000000-0000-4000-8000-000000000001'; const calls: string[] = [];
    const item = { invocationRef: id, taskKey: 'model_probe', taskConfigRef: null, modelConfigRef: null, batchRef: null, roundNo: null, operation: 'probe', state: 'failed', connectionVersionRef: 'version-1', modelId: null, connectionName: '历史连接', usageState: 'unknown_reserved', dispatchState: 'legacy_unknown', budgetDay: '2026-10-01', inputTokens: null, outputTokens: '0', measuredTokens: null, cachedInputTokens: null, cacheWriteTokens: null, reasoningTokens: null, quotaHeldTokens: '9007199254740993', legacyChargedTokens: '9007199254740993', quotePicoUsd: null, failureCode: null, createdAt: '2026-10-01T00:00:00Z', finishedAt: '2026-10-01T00:01:00Z' };
    vi.stubGlobal('fetch', vi.fn(async (path: string) => { calls.push(path); return response({ contract: 'ai-invocation-history.v1', pageSize: 50, items: path.includes('before=') ? [] : [item], hasMore: !path.includes('before='), nextCursor: path.includes('before=') ? null : id }); }));
    renderWithQuery(<AiInvocationHistory data={parseAiSettings(settingsFixture())} taskKey="" />);
    expect(await screen.findByText('用量未知，仍占额度')).toBeInTheDocument();
    expect(screen.getByText('9,007,199,254,740,993', { exact: true })).toBeInTheDocument();
    expect(screen.getAllByText('—', { exact: false }).length).toBeGreaterThan(0);
    const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: '更早一页' }));
    expect(await screen.findByText('本页没有调用记录，不据此推断其他工作或历史没有消耗。')).toBeInTheDocument();
    expect(calls[1]).toContain(`before=${id}`); expect(screen.getByRole('button', { name: '较新一页' })).toBeEnabled();
  });
});
