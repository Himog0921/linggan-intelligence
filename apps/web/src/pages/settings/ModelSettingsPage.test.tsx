import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ModelSettingsPage } from './ModelSettingsPage';
import { settingsFixture } from './aiSettingsFixture.test-support';
import { AI_SETTINGS_KEY } from './aiSettingsApi';
import { parseAiSettings } from './aiSettingsModel';

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
function renderPage(entry = '/settings/models?section=models', client = new QueryClient({ defaultOptions: { queries: { retry: false } } })) {
  return render(<MemoryRouter initialEntries={[entry]}><QueryClientProvider client={client}><ModelSettingsPage /></QueryClientProvider></MemoryRouter>);
}
const posts = () => calls.filter(call => call.method === 'POST');
describe('AI 设置四区', () => {
  it('图片识别提示词可编辑、保存、启用和回退，保存不运行模型', async () => {
    const seed = settingsFixture();
    const config = { ...seed.configurations[0]!, taskKey: 'ocr_recognition', taskConfigRef: 'ocr-config' };
    const settings = { ...seed, tasks: [{ ...seed.tasks[0]!, taskKey: 'ocr_recognition', label: '图片文字识别', skillAllowed: true, configuration: config }], configurations: [config] };
    const first = { versionRef: 'prompt-v1', versionNo: 1, systemText: '原系统提示词', instructionText: '原识别指令', createdAt: '2026-10-27T00:00:00Z' };
    const second = { ...first, versionRef: 'prompt-v2', versionNo: 2, systemText: '更新后的系统提示词' };
    let versions = [first], active = 'prompt-v1', revision = 0;
    handler = call => {
      if (call.path.endsWith('/prompts/ocr_recognition')) return ok({ taskKey: 'ocr_recognition', revision, activeVersionRef: active, versions });
      if (call.path.endsWith('/prompts/save')) { versions = [second, first]; return ok({ versionRef: second.versionRef, versionNo: 2 }); }
      if (call.path.endsWith('/prompts/activate')) { active = String(call.body.versionRef); revision += 1; return ok({ versionRef: active, taskConfigRef: `config-${revision}`, taskRevision: revision }); }
      if (call.path.includes('/writing-samples')) return ok({ state: 'connected', samples: [], examined: 0, limit: 50 });
      return ok(settings);
    };
    renderPage('/settings/models?section=skills&task=ocr_recognition');
    const system = await screen.findByLabelText('系统提示词');
    fireEvent.change(system, { target: { value: '更新后的系统提示词' } });
    await userEvent.click(screen.getByRole('button', { name: '保存提示词新版本' }));
    expect(await screen.findByText('提示词 v2 已保存，尚未启用。')).toBeInTheDocument();
    await waitFor(() => expect(screen.getByLabelText('提示词版本')).toHaveValue('prompt-v2'));
    expect(posts()[0]).toMatchObject({ path: '/api/local/ai-settings/prompts/save', body: { taskKey: 'ocr_recognition', expectedTaskRevision: 0, systemText: '更新后的系统提示词', instructionText: '原识别指令' } });
    await userEvent.click(screen.getByRole('button', { name: '启用此提示词版本' }));
    await waitFor(() => expect(active).toBe('prompt-v2'));
    await waitFor(() => expect(screen.getByRole('button', { name: '启用此提示词版本' })).toBeDisabled());
    fireEvent.change(screen.getByLabelText('提示词版本'), { target: { value: 'prompt-v1' } });
    fireEvent.change(screen.getByLabelText('系统提示词'), { target: { value: '尚未保存的修改' } });
    expect(screen.getByRole('button', { name: '启用此提示词版本' })).toBeDisabled();
    fireEvent.change(screen.getByLabelText('系统提示词'), { target: { value: '原系统提示词' } });
    await userEvent.click(screen.getByRole('button', { name: '启用此提示词版本' }));
    await waitFor(() => expect(active).toBe('prompt-v1'));
    expect(posts().map(call => call.path)).toEqual(['/api/local/ai-settings/prompts/save', '/api/local/ai-settings/prompts/activate', '/api/local/ai-settings/prompts/activate']);
  });
  it('直接创建做法，不启动任务或模型', async () => {
    handler = call => call.method === 'GET' ? ok(settingsFixture()) : ok({ skillRef: 'skill', versionRef: 'v1', versionNo: 1, packageSha256: 'a'.repeat(64), reused: false });
    renderPage('/settings/models?section=skills');
    const name = await screen.findByLabelText('做法名称');
    fireEvent.change(name, { target: { value: 'new-method' } });
    fireEvent.change(screen.getByLabelText('SKILL.md 内容'), { target: { value: '---\nname: new-method\ndescription: A method\n---\nInstructions' } });
    await userEvent.click(screen.getByRole('button', { name: '创建做法' }));
    expect(await screen.findByText('已创建做法 v1；尚未启用或运行。')).toBeInTheDocument();
    expect(posts()).toHaveLength(1);
    expect(posts()[0]).toMatchObject({ path: '/api/local/ai-settings/skills/import', body: { expectedVersionRef: null, package: { directory: 'new-method', files: [{ path: 'SKILL.md', encoding: 'utf8' }] } } });
  });
  it('直接编辑做法并保存新版本，保留二进制引用和版本冲突草稿', async () => {
    const seed = settingsFixture();
    const version = { versionRef: 'v1', versionNo: 1, description: 'Synthetic skill', packageSha256: 'a'.repeat(64), sourceSha256: 'b'.repeat(64), disableModelInvocation: false, createdAt: '2026-10-27T00:00:00Z', fileCount: 2 };
    const data = { ...seed, skills: [{ skillRef: 'skill', name: 'example-skill', versions: [version] }] };
    const snapshot = { versionRef: 'v1', versionNo: 1, description: 'Synthetic skill', packageSha256: 'a'.repeat(64), sourceSha256: 'b'.repeat(64), disableModelInvocation: false, skillRef: 'skill', name: 'example-skill', body: 'original', frontmatter: '---', loaderVersion: 'test', files: [
      { path: 'SKILL.md', sha256: 'c'.repeat(64), bytes: 8, content: 'original', contentBase64: null, kind: 'utf8' },
      { path: 'assets/example.bin', sha256: 'd'.repeat(64), bytes: 3, content: null, contentBase64: '/wD+', kind: 'binary' },
    ] };
    let conflict = true;
    handler = call => call.method === 'POST' ? conflict ? { status: 409, body: { error: 'ai_revision_conflict' } } : ok({ skillRef: 'skill', versionRef: 'v2', versionNo: 2, packageSha256: 'a'.repeat(64), reused: false })
      : call.path === '/api/local/ai-settings/skills/v1' ? ok(snapshot) : ok(data);
    renderPage('/settings/models?section=skills&version=v1');
    const editor = await screen.findByLabelText('SKILL.md');
    fireEvent.change(editor, { target: { value: 'updated method' } });
    await userEvent.click(screen.getByRole('button', { name: '保存新版本' }));
    expect(await screen.findByText(/草稿已保留/)).toBeInTheDocument();
    expect(editor).toHaveValue('updated method');
    expect(posts()[0]?.body).toMatchObject({ expectedVersionRef: 'v1', package: { directory: 'example-skill', files: [
      { path: 'SKILL.md', encoding: 'utf8', content: 'updated method' },
      { path: 'assets/example.bin', encoding: 'base64', content: '/wD+' },
    ] } });
    conflict = false;
    await userEvent.click(screen.getByRole('button', { name: '保存新版本' }));
    expect(await screen.findByText('已保存做法 v2；尚未启用或运行。')).toBeInTheDocument();
    expect(posts()).toHaveLength(2);
  });
  it('后台刷新出现新版本时，编辑草稿仍使用打开时的版本作为并发基线', async () => {
    const seed = settingsFixture();
    const version = { versionRef: 'v1', versionNo: 1, description: 'Synthetic skill', packageSha256: 'a'.repeat(64), sourceSha256: 'b'.repeat(64), disableModelInvocation: false, createdAt: '2026-10-27T00:00:00Z', fileCount: 1 };
    const data = { ...seed, skills: [{ skillRef: 'skill', name: 'example-skill', versions: [version] }] };
    const snapshot = { versionRef: 'v1', versionNo: 1, description: 'Synthetic skill', packageSha256: 'a'.repeat(64), sourceSha256: 'b'.repeat(64), disableModelInvocation: false, skillRef: 'skill', name: 'example-skill', body: 'original', frontmatter: '---', loaderVersion: 'test', files: [
      { path: 'SKILL.md', sha256: 'c'.repeat(64), bytes: 8, content: 'original', contentBase64: null, kind: 'utf8' },
    ] };
    handler = call => call.method === 'POST' ? { status: 409, body: { error: 'ai_revision_conflict' } }
      : call.path === '/api/local/ai-settings/skills/v1' ? ok(snapshot) : ok(data);
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    renderPage('/settings/models?section=skills&version=v1', client);
    const editor = await screen.findByLabelText('SKILL.md');
    fireEvent.change(editor, { target: { value: 'my draft' } });
    const concurrent = { ...data, skills: [{ ...data.skills[0]!, versions: [{ ...version, versionRef: 'v2', versionNo: 2 }, version] }] };
    await act(async () => { client.setQueryData(AI_SETTINGS_KEY, parseAiSettings(concurrent)); });
    expect(await screen.findByRole('link', { name: /example-skill · v2/ })).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: '保存新版本' }));
    expect(await screen.findByText(/草稿已保留/)).toBeInTheDocument();
    expect(posts()[0]?.body.expectedVersionRef).toBe('v1');
    expect(editor).toHaveValue('my draft');
  });
  it('无需试跑即可直接切换版本，切换不创建模型任务', async () => {
    const seed = settingsFixture();
    const candidate = { ...seed.configurations[0]!, taskKey: 'writing_legacy', taskConfigRef: 'config-1', skillVersionRef: 'version-1' };
    const version = { versionRef: 'version-1', versionNo: 1, description: 'Synthetic', packageSha256: 'a'.repeat(64), sourceSha256: 'b'.repeat(64), disableModelInvocation: false, createdAt: candidate.createdAt, fileCount: 1 };
    const data = { ...seed, configurations: [candidate], skills: [{ skillRef: 'skill-1', name: 'example', versions: [version] }],
      tasks: [{ ...seed.tasks[0]!, taskKey: 'writing_legacy', label: '写作', lifecycle: 'active', skillAllowed: true, configuration: candidate }] };
    handler = call => call.method === 'POST' ? ok({ versionRef: 'version-1', taskConfigRef: 'new-config', taskRevision: 1, bindingRevision: 1, approvalSource: 'manual' })
      : call.path.includes('/writing-samples') ? ok({ state: 'connected', samples: [], examined: 0, limit: 50 }) : ok(data);
    renderPage('/settings/models?section=skills&task=writing_legacy');
    const reason = await screen.findByLabelText('启用理由');
    await userEvent.type(reason, '切换做法');
    await userEvent.click(screen.getByRole('button', { name: '启用候选做法' }));
    expect(await screen.findByText(/固定做法版本已启用/)).toBeInTheDocument();
    expect(posts()).toHaveLength(1);
    expect(posts()[0]).toMatchObject({ path: '/api/local/ai-settings/skills/activate', body: { candidateConfigRef: 'config-1', trialComparisonRef: null, reason: '切换做法' } });
    expect(posts()[0]?.body).not.toHaveProperty('firstVersionException');
  });
  it('做法版本可在设置中直接切换，不要求先重配模型', async () => {
    const seed = settingsFixture();
    const config = { ...seed.configurations[0]!, taskKey: 'writing_legacy', skillVersionRef: 'version-1' };
    const version = { versionRef: 'version-1', versionNo: 1, description: 'Synthetic', packageSha256: 'a'.repeat(64), sourceSha256: 'b'.repeat(64), disableModelInvocation: false, createdAt: config.createdAt, fileCount: 1 };
    const data = { ...seed, configurations: [config], skills: [{ skillRef: 'skill', name: 'example', versions: [{ ...version, versionRef: 'version-2', versionNo: 2 }, version] }],
      tasks: [{ ...seed.tasks[0]!, taskKey: 'writing_legacy', label: '写作', lifecycle: 'active', skillAllowed: true, activeSkillVersionRef: 'version-1', configuration: config }] };
    handler = call => call.method === 'POST' ? ok({ versionRef: 'version-2', taskConfigRef: 'new-config', taskRevision: 1, bindingRevision: 1, approvalSource: 'manual' })
      : call.path.includes('/prompts/') ? ok({ taskKey: 'writing_legacy', revision: 0, activeVersionRef: null, versions: [] })
      : call.path.includes('/writing-samples') ? ok({ state: 'connected', samples: [], examined: 0, limit: 50 }) : ok(data);
    renderPage('/settings/models?section=skills&task=writing_legacy');
    const select = await screen.findByLabelText('切换做法版本');
    fireEvent.change(select, { target: { value: 'version-2' } });
    await userEvent.click(screen.getByRole('button', { name: '切换做法版本' }));
    expect(await screen.findByText('做法已切换，当前模型、提示词和额度保持不变。')).toBeInTheDocument();
    expect(posts()).toHaveLength(1);
    expect(posts()[0]).toMatchObject({ path: '/api/local/ai-settings/skills/switch', body: { taskKey: 'writing_legacy', versionRef: 'version-2', expectedTaskRevision: 0, expectedBindingRevision: 0 } });
  });
  it.each(['comment_study','comment_problem_resolution','comment_problem_pair_review','topic_research','topic_research_extract_review','topic_research_resolve','topic_research_resolve_review','creator_research','creator_account_research'])('研究阶段 %s 可编辑提示词与切换做法', async taskKey => {
    const seed = settingsFixture();
    const config = { ...seed.configurations[0]!, taskKey, skillVersionRef: null, executionContract: null };
    const version = { versionRef: 'version-1', versionNo: 1, description: 'Synthetic', packageSha256: 'a'.repeat(64), sourceSha256: 'b'.repeat(64), disableModelInvocation: false, createdAt: config.createdAt, fileCount: 1 };
    const data = { ...seed, configurations: [config], skills: [{ skillRef: 'skill', name: 'example', versions: [version] }],
      tasks: [{ ...seed.tasks[0]!, taskKey, label: '研究', lifecycle: 'active', skillAllowed: true, handlerContract: null, configuration: config }] };
    handler = call => call.method === 'POST' ? ok({ versionRef: 'version-1', taskConfigRef: 'new-config', taskRevision: 1, bindingRevision: 1, approvalSource: 'manual' })
      : call.path.endsWith(`/prompts/${taskKey}`) ? ok({ taskKey, revision: 0, activeVersionRef: 'prompt-v1', versions: [{ versionRef: 'prompt-v1', versionNo: 1, systemText: '系统', instructionText: '任务', createdAt: config.createdAt }] }) : ok(data);
    renderPage(`/settings/models?section=skills&task=${taskKey}`);
    expect(await screen.findByLabelText('系统提示词')).toHaveValue('系统');
    expect(screen.queryByText(/此工作尚未接入执行/)).not.toBeInTheDocument();
    fireEvent.change(screen.getByLabelText('切换做法版本'), { target: { value: 'version-1' } });
    await userEvent.click(screen.getByRole('button', { name: '切换做法版本' }));
    expect(await screen.findByText('做法已切换，当前模型、提示词和额度保持不变。')).toBeInTheDocument();
    expect(posts()).toHaveLength(1);
    expect(posts()[0]).toMatchObject({ path: '/api/local/ai-settings/skills/switch', body: { taskKey, versionRef: 'version-1' } });
  });
  it('原入口呈现四区；导航、样本读取与刷新不产生写操作', async () => {
    const user = userEvent.setup(); renderPage();
    expect(document.querySelector('h1#ai-settings-page-title.v7-sr-only')).toBeInTheDocument();
    expect(await screen.findByLabelText('连接名称')).toHaveValue('隔离合成连接');
    expect(screen.getByRole('navigation', { name: '旧版 AI 设置分区' })).toBeInTheDocument();
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
