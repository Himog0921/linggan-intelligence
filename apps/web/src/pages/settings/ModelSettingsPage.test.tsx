import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ModelSettingsPage } from './ModelSettingsPage';
import { settingsFixture } from './aiSettingsFixture.test-support';

type Reply = { status: number; body: unknown } | 'network';
type Call = { path: string; method: string; body: Record<string, unknown> };
let calls: Call[];
let handler: (call: Call) => Reply | Promise<Reply>;
const ok = (body: unknown): Reply => ({ status: 200, body });
beforeEach(() => {
  calls = []; handler = call => call.method === 'GET' && call.path === '/api/local/ai-settings' ? ok(settingsFixture()) : { status: 503, body: { error: 'fixture_route_unavailable' } };
  vi.stubGlobal('fetch', vi.fn(async (path: string, init?: RequestInit) => {
    const call = { path, method: init?.method ?? 'GET', body: typeof init?.body === 'string' ? JSON.parse(init.body) : {} };
    calls.push(call); const reply = await handler(call);
    if (reply === 'network') throw new TypeError('fixture lost response');
    return new Response(JSON.stringify(reply.body), { status: reply.status, headers: { 'Content-Type': 'application/json' } });
  }));
});
afterEach(() => vi.unstubAllGlobals());
function renderPage(entry = '/settings/models') {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(<MemoryRouter initialEntries={[entry]}><QueryClientProvider client={client}><ModelSettingsPage /></QueryClientProvider></MemoryRouter>);
}
const posts = () => calls.filter(call => call.method === 'POST');
describe('AI 设置四区', () => {
  it('原入口呈现四区；导航、样本读取与刷新不产生写操作', async () => {
    const user = userEvent.setup(); renderPage();
    expect(await screen.findByRole('heading', { name: 'AI 设置', level: 1 })).toBeInTheDocument();
    expect(await screen.findByLabelText('连接名称')).toHaveValue('隔离合成连接');
    expect(screen.getByText('当前密钥尚未测试')).toBeInTheDocument();
    for (const name of ['自动化与额度', '做法', '运行状况', '模型']) {
      await user.click(screen.getByRole('link', { name }));
      expect(await screen.findByRole('heading', { name, level: 2 })).toBeInTheDocument();
    }
    expect(posts()).toHaveLength(0);
    expect(calls.every(call => call.path.startsWith('/api/local/ai-settings'))).toBe(true);
  });
  it('连接保存带最新 CAS 修订，明确回执后成功；不测试、不入队', async () => {
    const user = userEvent.setup(); const data = settingsFixture();
    handler = call => call.method === 'GET' ? ok(data) : ok({ connectionRef: 'connection-1', versionRef: call.body.versionRef, revision: 4, state: 'SAVED_UNTESTED' });
    renderPage(); const name = await screen.findByLabelText('连接名称'); await user.clear(name); await user.type(name, '保存不调用');
    await user.click(screen.getByRole('button', { name: '保存连接' }));
    expect(await screen.findByText('连接已保存，未进行测试或创建任务。')).toBeInTheDocument();
    expect(posts()).toHaveLength(1); expect(posts()[0]?.path).toBe('/api/local/model-settings/connections');
    expect(posts()[0]?.body).toMatchObject({ expectedRevision: 3, name: '保存不调用', apiKey: '' });
  });
  it('CAS 拒绝保留草稿，未显示成功', async () => {
    const user = userEvent.setup(); handler = call => call.method === 'GET' ? ok(settingsFixture()) : { status: 409, body: { error: 'model_revision_conflict' } };
    renderPage(); const name = await screen.findByLabelText('连接名称'); await user.clear(name); await user.type(name, '保留我的草稿');
    await user.click(screen.getByRole('button', { name: '保存连接' }));
    expect(await screen.findByText(/草稿已保留/)).toBeInTheDocument(); expect(name).toHaveValue('保留我的草稿');
    expect(screen.queryByText('已收到回执')).not.toBeInTheDocument(); expect(posts()).toHaveLength(1);
  });
  it('保存能力并刷新版本时，另一张价格表单的草稿仍保留', async () => {
    const user = userEvent.setup(); const fixture = settingsFixture(); let capabilityRef = 'cap-1';
    handler = call => call.method === 'GET' ? ok({ ...fixture, models: fixture.models.map(model => ({ ...model, capability: { capabilityRef, source: 'manual', sourceVersion: 'synthetic-cap-v1', probeInvocationRef: null, contextTokens: 4096, maxOutputTokens: 512, maxTimeoutSeconds: 30, thinkingLevels: ['off'], compat: {}, thinkingLevelMap: {}, outputMode: 'prompt_only', strictSchema: false } })) }) : (capabilityRef = 'cap-2', ok({ revisionRef: capabilityRef }));
    renderPage(); await user.type(await screen.findByLabelText('输入价格'), '123');
    await user.type(screen.getByLabelText('价格依据及版本'), '价格草稿');
    await user.click(screen.getByRole('button', { name: '保存能力声明' }));
    expect(await screen.findByText(/能力声明已保存/)).toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole('button', { name: '保存能力声明' })).toBeEnabled());
    expect(screen.getByLabelText('输入价格')).toHaveValue('123'); expect(screen.getByLabelText('价格依据及版本')).toHaveValue('价格草稿');
    expect(posts()).toHaveLength(1); expect(posts()[0]?.path).toBe('/api/local/ai-settings/capabilities');
  });
  it.each(['network', 'malformed'] as const)('未确认回执 %s 的重试沿用原请求编号', async kind => {
    const user = userEvent.setup(); let attempt = 0;
    handler = call => call.method === 'GET' ? ok(settingsFixture()) : ++attempt === 1 ? kind === 'network' ? 'network' : ok({ unexpected: true }) : ok({ connectionRef: 'connection-1', versionRef: call.body.versionRef, revision: 4, state: 'SAVED_UNTESTED' });
    renderPage(); await screen.findByLabelText('连接名称');
    await user.click(screen.getByRole('button', { name: '保存连接' }));
    expect(await screen.findByText('结果未确认', { exact: true })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '保存连接' }));
    expect(await screen.findByText('已收到回执')).toBeInTheDocument();
    expect(posts()).toHaveLength(2); expect(posts()[1]?.body.versionRef).toBe(posts()[0]?.body.versionRef);
  });
  it('测试任务只保存额度，不开启开关、入队或调用；大整数额度按字符串发送', async () => {
    const user = userEvent.setup(); handler = call => call.method === 'GET' ? ok(settingsFixture()) : ok({ taskKey: 'model_probe', taskConfigRef: 'config-2', modelConfigRef: null, revision: 1, applied: true });
    renderPage('/settings/models?section=automation&task=model_probe');
    await user.selectOptions(await screen.findByLabelText('每日额度'), 'capped');
    await user.type(screen.getByLabelText('每日 token 上限'), '9007199254740993');
    await user.click(screen.getByRole('button', { name: '保存配置' }));
    expect(await screen.findByText(/配置已保存，用于后续批次/)).toBeInTheDocument();
    expect(posts()).toHaveLength(1); expect(posts()[0]?.path).toBe('/api/local/ai-settings/task-configs');
    expect(posts()[0]?.body).toMatchObject({ expectedRevision: 0, model: null, skillVersionRef: null, budget: { mode: 'capped', tokens: '9007199254740993' } });
  });
  it('启用只有在填写影响原因后显式提交，页面不推断运行正常', async () => {
    const user = userEvent.setup(); handler = call => call.method === 'GET' ? ok(settingsFixture()) : ok({ scopeKey: 'global', revision: 1, enabled: true, commandRef: call.body.commandRef });
    renderPage('/settings/models?section=automation');
    const button = await screen.findByRole('button', { name: '启用全局 AI' }); expect(button).toBeDisabled();
    await user.type(screen.getByLabelText('全局 AI操作原因'), '合成环境测试'); await user.click(button);
    expect(await screen.findByText(/全局 AI已启用，实际执行仍需/)).toBeInTheDocument();
    expect(posts()[0]?.body).toMatchObject({ scope: { kind: 'global' }, enabled: true, expectedRevision: 0, reason: '合成环境测试' });
  });
  it('未知与已知零值、超安全整数用量保真；未启动不会显示运行正常', async () => {
    const data = settingsFixture();
    const usage = { taskKey: 'model_probe', budgetDay: '2026-10-20', calls: '9007199254740993', runningCalls: '0', failedCalls: '0', measuredCalls: '0', unknownCalls: '1', notDispatchedCalls: '0', measuredTokens: null, quotaHeldTokens: '9007199254740993', unknownHeldTokens: '9007199254740993', knownQuotePicoUsd: '0', unpricedMeasuredCalls: '1', cacheReadUnknownCalls: '1', reasoningUnknownCalls: '1' };
    handler = call => call.method === 'GET' ? ok({ ...data, execution: { ...data.execution, dailyUsage: [usage] } }) : ok({});
    renderPage('/settings/models?section=health');
    expect(await screen.findByText(/独立 AI 执行器 · 尚未启动/)).toBeInTheDocument();
    expect(screen.getAllByText('9,007,199,254,740,993', { exact: false }).length).toBeGreaterThan(0);
    expect(screen.getByText('$0 / 1', { exact: false })).toBeInTheDocument();
    expect(screen.getAllByText('—').length).toBeGreaterThan(0); expect(posts()).toHaveLength(0);
  });
  it('返回缺字段时拒绝渲染表单', async () => {
    handler = () => ok({ ...settingsFixture(), controls: undefined }); renderPage();
    expect(await screen.findByRole('alert')).toBeInTheDocument();
    expect(screen.queryByLabelText('连接名称')).not.toBeInTheDocument();
    await waitFor(() => expect(posts()).toHaveLength(0));
  });
  it.each(['unavailable', 'unrecognized_future_state'])('比较的历史完成事实保留；当前来源 %s 即使携带产出也不显示', async sourceQualification => {
    let completed = false;
    const pendingRead: { response?: Promise<Reply> } = {};
    const seed = settingsFixture(), current = { ...seed.configurations[0]!, taskKey: 'writing_legacy', taskConfigRef: 'c1', skillVersionRef: 'v1' };
    const candidate = { ...current, taskConfigRef: 'c2', skillVersionRef: 'v2' };
    const versions = [1, 2].map(versionNo => ({ versionRef: `v${versionNo}`, versionNo, description: 'Synthetic fixed version', packageSha256: 'a'.repeat(64), sourceSha256: 'b'.repeat(64), disableModelInvocation: false, createdAt: current.createdAt, fileCount: 1 }));
    const data = { ...seed, skills: [{ skillRef: 'skill', name: 'synthetic-method', versions }], configurations: [candidate, current],
      tasks: [{ ...seed.tasks[0]!, taskKey: 'writing_legacy', label: '既有写作建议', skillAllowed: true, requiresSkill: true, configuration: current, activeSkillVersionRef: 'v1' }],
      controls: seed.controls.map(control => control.taskKey ? { ...control, taskKey: 'writing_legacy', scope: 'task:writing_legacy' } : control),
      trials: [{ comparisonRef: 'comparison', taskKey: 'writing_legacy', baselineBatchRef: 'old-batch', candidateBatchRef: 'new-batch', baselineState: 'completed', candidateState: 'completed', candidateConfigRef: 'c2', itemCount: 1, inputsSha256: 'hash', createdAt: current.createdAt }] };
    const side = { batchRef: 'batch', taskConfigRef: 'c2', state: 'completed', itemState: 'succeeded', attemptCount: 1, output: { title: null, paragraphs: [{ paragraphId: null, text: 'SYNTHETIC 不应展示的旧产出' }], pages: [] }, failureCode: null, executionContract: 'synthetic', skillVersionRef: 'v2', invocationRef: 'invocation' };
    const comparisonReply = (qualification: string): Reply => ok({ comparisonRef: 'comparison', taskKey: 'writing_legacy', position: 0, itemCount: 1, inputRef: { kind: 'writing_preview', previewRef: 'preview' }, inputsSha256: 'hash', baseline: side, candidate: side, qualifiesForActivation: true, sourceQualification: qualification });
    handler = call => call.path.includes('/trials/comparison') ? pendingRead.response ?? comparisonReply('readable')
      : call.path.includes('/writing-samples') ? ok({ state: 'connected', samples: [], examined: 0, limit: 50 }) : ok({ ...data, trials: data.trials.map(trial => ({ ...trial, candidateState: completed ? 'completed' : 'open' })) });
    renderPage('/settings/models?section=skills&task=writing_legacy');
    await userEvent.selectOptions(await screen.findByLabelText('查看试跑比较'), 'comparison');
    expect(await screen.findByText(/固定试跑已完成，具备版本比较回执/)).toBeInTheDocument();
    expect(screen.getAllByText('SYNTHETIC 不应展示的旧产出')).toHaveLength(2);
    expect(screen.getByRole('option', { name: /待领取或执行中/ })).toBeInTheDocument();
    let releaseRead!: (reply: Reply) => void;
    pendingRead.response = new Promise<Reply>(resolve => { releaseRead = resolve; });
    completed = true;
    await userEvent.click(screen.getByRole('button', { name: '刷新比较回执' }));
    expect(await screen.findByText('正在重核本次材料资格，产出暂不展示…')).toBeInTheDocument();
    expect(screen.queryByText('SYNTHETIC 不应展示的旧产出')).not.toBeInTheDocument();
    releaseRead(comparisonReply(sourceQualification));
    await screen.findByText(/固定试跑已完成，具备版本比较回执/);
    await waitFor(() => expect(screen.queryByRole('option', { name: /待领取或执行中/ })).not.toBeInTheDocument());
    expect(screen.getByLabelText('查看试跑比较')).toHaveValue('comparison');
    expect(screen.queryByText('SYNTHETIC 不应展示的旧产出')).not.toBeInTheDocument();
    expect(posts()).toHaveLength(0);
  });

});
