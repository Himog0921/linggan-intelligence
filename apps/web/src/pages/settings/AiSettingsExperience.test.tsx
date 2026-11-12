import { EgressForm } from './AiModelSection';
import { parseAiSettings } from './aiSettingsModel';
import { useAiAction } from './useAiAction';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { createMemoryRouter, RouterProvider } from 'react-router';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ModelSettingsPage } from './ModelSettingsPage';
import { businessSettingsFixture, settingsFixture } from './aiSettingsFixture.test-support';

type Reply = { status: number; body: unknown };
type Call = { path: string; method: string; body: Record<string, unknown> };
let calls: Call[];
let respond: (call: Call) => Reply;
const ok = (body: unknown): Reply => ({ status: 200, body });
function mount(url = '/settings/models') {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const router = createMemoryRouter([{ path: '*', element: <ModelSettingsPage /> }], { initialEntries: [url] });
  return { ...render(<QueryClientProvider client={client}><RouterProvider router={router} /></QueryClientProvider>), router };
}
beforeEach(() => {
  calls = [];
  respond = call => call.method === 'GET' && call.path === '/api/local/ai-settings'
    ? ok(businessSettingsFixture()) : { status: 503, body: { error: 'unavailable' } };
  vi.stubGlobal('fetch', vi.fn(async (path: string, init?: RequestInit) => {
    const call = { path, method: init?.method ?? 'GET', body: typeof init?.body === 'string' ? JSON.parse(init.body) : {} };
    calls.push(call);
    const response = respond(call);
    return new Response(JSON.stringify(response.body), { status: response.status, headers: { 'Content-Type': 'application/json' } });
  }));
});
afterEach(() => vi.unstubAllGlobals());

describe('AI 设置正式四视图', () => {
  it('默认展示工作能力，并将四个视图放进共享左栏，不读即写', async () => {
    const user = userEvent.setup();
    mount();
    expect(await screen.findByRole('heading', { name: '工作能力' })).toBeInTheDocument();
    expect(document.querySelector('h1#ai-settings-page-title.v7-sr-only')).toBeInTheDocument();
    expect(document.querySelectorAll('.v7-global-header')).toHaveLength(1);
    const navigation = screen.getByRole('complementary', { name: '设置导航' });
    const disclosure = within(navigation).getByRole('button', { name: '收起AI 设置子页面' });
    expect(disclosure).toHaveAttribute('aria-expanded', 'true');
    const group = document.getElementById(disclosure.getAttribute('aria-controls')!)!;
    expect(within(group).getAllByRole('link').map(link => link.textContent)).toEqual(['AI 能力', '模型连接', '运行与用量', '高级配置']);
    expect(screen.getByRole('link', { name: 'AI 能力' })).toHaveAttribute('aria-current', 'page');
    expect(screen.queryByText('SYSTEM / AI OPERATIONS')).not.toBeInTheDocument();
    expect(screen.queryByText('管理研究能力、模型连接与运行结果，让每一项 AI 工作有据可查。')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '刷新设置' })).not.toBeInTheDocument();
    await user.click(screen.getByRole('link', { name: '模型连接' }));
    expect(await screen.findByRole('heading', { name: '模型连接', level: 2 })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: '模型连接' })).toHaveAttribute('aria-current', 'page');
    await user.click(screen.getByRole('link', { name: 'AI 能力' }));
    expect(await screen.findByRole('heading', { name: '工作能力', level: 2 })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: /评论研究/ }));
    expect(await screen.findByRole('dialog', { name: '评论研究' })).toBeInTheDocument();
    expect(screen.getByText(/目前不能执行新工作/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '关闭详情' }));
    expect(screen.queryByRole('dialog', { name: '评论研究' })).not.toBeInTheDocument();
    expect(calls.filter(call => call.method === 'POST')).toHaveLength(0);
  });

  it('关键词、状态和视图切换不改变实际开关或配置', async () => {
    const user = userEvent.setup();
    mount();
    await screen.findByRole('heading', { name: '工作能力' });
    const search = screen.getByRole('searchbox', { name: '搜索 AI 工作' });
    await user.type(search, '评论');
    expect(screen.getByRole('button', { name: /评论研究/ })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /旧创作者分析/ })).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '列表视图' }));
    expect(screen.getByRole('button', { name: /评论研究/ })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '已接入' }));
    expect(screen.getByText(/没有符合条件的工作/)).toBeInTheDocument();
    expect(calls.filter(call => call.method === 'POST')).toHaveLength(0);
  });

  it('模型连接从真实连接与登记模型读取，内置 WeMM 为只读专用运行时', async () => {
    const data = settingsFixture();
    data.connections.push({ ...data.connections[0]!, connectionRef: 'local-conn', name: '本机 WeMM Runtime',
      api: 'local-wemm-runtime', baseUrl: 'local://wemm-embedding-2b', localEndpoint: true, endpointRef: 'local-endpoint' });
    data.models.push({ ...data.models[0]!, identityRef: 'local-model', connectionRef: 'local-conn',
      endpointRef: 'local-endpoint', modelId: 'Tencent/WeMM-Embedding-2B' });
    respond = call => call.method === 'GET' && call.path === '/api/local/ai-settings' ? ok(data) : { status: 503, body: {} };
    const user = userEvent.setup();
    mount('/settings/models?view=models');
    await screen.findByRole('heading', { name: '模型连接', level: 2 });
    expect(screen.getByText('隔离合成连接', { selector: 'b' })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: /本机 WeMM Runtime/ }));
    expect(screen.getByText(/本机 WeMM 不使用通用 API 密钥/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '技术配置' }));
    expect(screen.getByText(/不允许通过普通 HTTP 模型表单修改/)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '保存连接' })).not.toBeInTheDocument();
    expect(calls.filter(call => call.method === 'POST')).toHaveLength(0);
  });

  it('连接向导有明确保存边界，保存不触发探测、许可或派发', async () => {
    const user = userEvent.setup();
    respond = call => {
      if (call.method === 'GET') return ok(settingsFixture());
      if (call.path === '/api/local/model-settings/connections') {
        return ok({ connectionRef: call.body.connectionRef, versionRef: call.body.versionRef, revision: 1 });
      }
      return { status: 503, body: { error: 'unexpected_post' } };
    };
    mount('/settings/models?view=models');
    await screen.findByRole('heading', { name: '模型连接', level: 2 });
    await user.click(screen.getByRole('button', { name: '接入模型' }));
    expect(screen.getByRole('dialog', { name: '接入模型' })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: /外部语言模型服务/ }));
    await user.type(screen.getByRole('textbox', { name: '连接名称' }), '我的模型服务');
    await user.type(screen.getByRole('textbox', { name: '服务地址' }), 'https://example.com/v1');
    await user.type(screen.getByLabelText('API 密钥'), 'synthetic-test-key');
    await user.click(screen.getByRole('button', { name: '保存连接并继续' }));
    expect((await screen.findAllByText(/连接已保存，尚未测试/)).length).toBeGreaterThan(0);
    await waitFor(() => expect(screen.getByLabelText('模型 ID')).toBeInTheDocument());
    expect(calls.filter(call => call.method === 'POST')).toHaveLength(1);
    expect(calls.find(call => call.method === 'POST')).toMatchObject({
      path: '/api/local/model-settings/connections',
      body: { expectedRevision: 0, name: '我的模型服务', baseUrl: 'https://example.com/v1' },
    });
  });

  it('运行页只显示真实响应；未知 token、未报价和失败详情均可回链', async () => {
    const data = settingsFixture();
    const usage = { taskKey: 'model_probe', budgetDay: '2026-10-20', calls: '1', runningCalls: '0', failedCalls: '1',
      measuredCalls: '1', unknownCalls: '0', notDispatchedCalls: '0', measuredTokens: '16', quotaHeldTokens: '20',
      unknownHeldTokens: '0', knownQuotePicoUsd: null, unpricedMeasuredCalls: '1', cacheReadUnknownCalls: '0', reasoningUnknownCalls: '0' };
    const enriched = { ...data, execution: { ...data.execution, dailyUsage: [usage] } };
    const invocation = { invocationRef: 'invocation-1', taskKey: 'model_probe', taskConfigRef: 'config-1',
      modelConfigRef: null, batchRef: null, roundNo: null, operation: 'probe', state: 'failed',
      connectionVersionRef: 'version-1', modelId: 'synthetic-model', connectionName: '隔离合成连接', usageState: 'measured',
      dispatchState: 'sent', budgetDay: '2026-10-20', inputTokens: '11', outputTokens: '5',
      measuredTokens: '16', cachedInputTokens: null, cacheWriteTokens: null, reasoningTokens: null,
      quotaHeldTokens: '20', legacyChargedTokens: null, quotePicoUsd: null, failureCode: 'model_call_failed',
      createdAt: '2026-10-20T03:00:00Z', finishedAt: '2026-10-20T03:00:01Z' };
    respond = call => call.path === '/api/local/ai-settings' ? ok(enriched)
      : call.path.startsWith('/api/local/ai-settings/invocations') ? ok({
        contract: 'ai-invocation-history.v1', pageSize: 50, hasMore: false, nextCursor: null, items: [invocation],
      }) : { status: 503, body: {} };
    const user = userEvent.setup();
    mount('/settings/models?view=runs');
    expect(await screen.findByRole('heading', { name: '运行与用量', level: 2 })).toBeInTheDocument();
    expect(await screen.findByText('synthetic-model')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '查看' }));
    expect(screen.getByRole('dialog', { name: '调用与版本详情' })).toBeInTheDocument();
    expect(screen.getByText('model_call_failed')).toBeInTheDocument();
    expect(screen.getByText('未知（未定价）')).toBeInTheDocument();
    expect(calls.filter(call => call.method === 'POST')).toHaveLength(0);
  });
  it('OCR 模型复用现有连接，保存不启用运行、不发图', async () => {
    const base = settingsFixture();
    const capability = { capabilityRef: 'cap-vision', source: 'manual', sourceVersion: 'synthetic-only', probeInvocationRef: null, contextTokens: 8192, maxOutputTokens: 2048, maxTimeoutSeconds: 60, thinkingLevels: ['off'], compat: {}, thinkingLevelMap: { off: null }, outputMode: 'json_object', strictSchema: false };
    const model = { ...base.models[0]!, capability };
    const config = { ...base.tasks[0]!.configuration, taskKey: 'ocr_recognition', executionContract: 'ocr-recognition.v1',
      model: { modelConfigRef: 'model-config', modelRef: 'old-model-ref', identityRef: 'identity-1', modelId: 'synthetic-model',
        connectionRef: 'connection-1', connectionVersionRef: 'version-1', capabilityRef: 'cap-vision',
        limits: { inputTokens: 2048, outputTokens: 512, timeoutSeconds: 30, attempts: 1, thinkingLevel: 'off' } } };
    const data = { ...base, models: [model, { ...model, identityRef: 'identity-2', selectionModelRef: 'second-model', modelId: 'synthetic-other-vision' }],
      tasks: [{ ...base.tasks[0]!, taskKey: 'ocr_recognition', label: '图片文字识别', handlerContract: 'ocr-recognition.v1', configuration: config }],
      controls: base.controls.map(control => control.taskKey ? { ...control, scope: 'task:ocr_recognition', taskKey: 'ocr_recognition', label: '图片文字识别' } : control) };
    respond = call => call.method === 'GET' ? ok(data) : call.path === '/api/local/ai-settings/task-configs'
      ? ok({ taskKey: 'ocr_recognition', taskConfigRef: 'next-config', modelConfigRef: 'next-model', revision: 1, applied: true })
      : { status: 503, body: {} };
    const user = userEvent.setup(); mount();
    await user.click(await screen.findByRole('button', { name: /图片文字识别/ }));
    expect(screen.getByRole('heading', { name: 'OCR 模型' })).toBeInTheDocument();
    await user.selectOptions(screen.getByRole('combobox', { name: '选择 OCR 模型' }), 'identity-2');
    for (const [label, value] of [['输入 token 上限', '2048'], ['输出 token 上限', '512'], ['单次等待秒数', '30']]) {
      await user.clear(screen.getByRole('spinbutton', { name: label })); await user.type(screen.getByRole('spinbutton', { name: label }), value!);
    }
    expect(screen.getByRole('combobox', { name: '思考开关' })).toHaveValue('off');
    expect(screen.getByRole('combobox', { name: '思考等级' })).toBeDisabled();
    await user.selectOptions(screen.getByRole('combobox', { name: '每日额度' }), 'unlimited');
    expect(screen.getByRole('spinbutton', { name: '最多尝试次数' })).toHaveValue(1);
    await user.click(screen.getByRole('button', { name: '保存配置' }));
    await waitFor(() => expect(calls.filter(call => call.method === 'POST')).toHaveLength(1));
    expect(calls.find(call => call.method === 'POST')).toMatchObject({ path: '/api/local/ai-settings/task-configs', body: { taskKey: 'ocr_recognition', budget: { mode: 'unlimited' }, model: { modelRef: 'second-model', limits: { attempts: 1, thinkingLevel: 'off' } } } });
  });

  it('材料范围沿用底稿六类字段，视频画面可显式选入后保存', async () => {
    const base = settingsFixture();
    const capability = { capabilityRef: 'cap-text', source: 'manual', sourceVersion: 'synthetic-only', probeInvocationRef: null, contextTokens: 8192, maxOutputTokens: 2048, maxTimeoutSeconds: 60, thinkingLevels: ['off'], compat: {}, thinkingLevelMap: { off: null }, outputMode: 'json_object', strictSchema: false };
    const config = { ...base.tasks[0]!.configuration, taskKey: 'note_interpretation', materialColumns: ['title', 'body', 'tags', 'ocr', 'transcript'],
      model: { modelConfigRef: 'model-config', modelRef: 'old-model-ref', identityRef: 'identity-1', modelId: 'synthetic-model',
        connectionRef: 'connection-1', connectionVersionRef: 'version-1', capabilityRef: 'cap-text',
        limits: { inputTokens: 2048, outputTokens: 512, timeoutSeconds: 30, attempts: 1, thinkingLevel: 'off' } } };
    const data = { ...base, models: [{ ...base.models[0]!, capability }],
      tasks: [{ ...base.tasks[0]!, taskKey: 'note_interpretation', label: '笔记解读', configuration: config }] };
    respond = call => call.method === 'GET' ? ok(data) : call.path === '/api/local/ai-settings/task-configs'
      ? ok({ taskKey: 'note_interpretation', taskConfigRef: 'next-config', modelConfigRef: 'model-config', revision: 1, applied: true })
      : { status: 503, body: {} };
    const user = userEvent.setup(); mount();
    await user.click(await screen.findByRole('button', { name: /笔记解读/ }));
    await user.click(screen.getByRole('link', { name: '查看工作配置' }));
    expect(screen.queryByRole('checkbox', { name: '封面文字' })).not.toBeInTheDocument();
    expect(screen.getByRole('checkbox', { name: '视频画面文字' })).not.toBeChecked();
    expect(calls.filter(call => call.method === 'POST')).toHaveLength(0);
    await user.click(screen.getByRole('checkbox', { name: '视频画面文字' }));
    await user.click(screen.getByRole('button', { name: '保存配置' }));
    await screen.findByText(/配置已保存，用于后续批次/);
    const writes = calls.filter(call => call.method === 'POST');
    expect(writes).toHaveLength(1);
    expect(writes[0]).toMatchObject({ path: '/api/local/ai-settings/task-configs', body: { taskKey: 'note_interpretation', expectedRevision: 0, materialColumns: ['title', 'body', 'tags', 'ocr', 'transcript', 'frame_text'] } });
  });

  it('单篇分析可补齐 DeepSeek 思考能力，再开启思考、选择高并读回', async () => {
    const base = settingsFixture();
    base.connections[0]!.name = 'DeepSeek';
    const modelId = 'deepseek-v4-flash';
    const preset = { provider: 'deepseek', modelId, name: 'DeepSeek V4 Flash', api: 'openai-completions',
      sourceVersion: 'synthetic-catalog', supported: true, reasons: [],
      limits: { contextTokens: 8192, maxOutputTokens: 2048, maxTimeoutSeconds: null, thinkingLevels: ['off', 'high', 'max'] },
      compat: { thinkingFormat: 'deepseek' }, thinkingLevelMap: { minimal: null, high: 'high', max: 'max' },
      outputMode: 'prompt_only', strictSchema: false, prices: { input: null, output: null, cachedRead: null, cachedWrite: null },
      operatorFields: ['maxTimeoutSeconds'], unknownFields: [] };
    let capability = { ...preset.limits, capabilityRef: 'cap-off', source: 'manual', sourceVersion: 'existing',
      probeInvocationRef: null, maxTimeoutSeconds: 60, thinkingLevels: ['off'], compat: preset.compat,
      thinkingLevelMap: preset.thinkingLevelMap, outputMode: preset.outputMode, strictSchema: false };
    const model = { ...base.models[0]!, identityRef: 'deepseek-flash', modelId };
    let configuration = { ...base.tasks[0]!.configuration, taskKey: 'note_interpretation',
      model: { modelConfigRef: 'model-config', modelRef: 'old-model-ref', identityRef: model.identityRef, modelId,
        connectionRef: model.connectionRef, connectionVersionRef: 'version-1', capabilityRef: 'cap-off',
        limits: { inputTokens: 2048, outputTokens: 512, timeoutSeconds: 30, attempts: 1, thinkingLevel: 'off' } } };
    let revision = 0;
    respond = call => {
      if (call.path.startsWith('/api/local/ai-settings/catalog?')) {
        return ok({ contract: 'ai-pi-catalog.v1', sdkVersion: 'synthetic', providers: ['deepseek'], total: 1, offset: 0, limit: 100, hasMore: false, presets: [preset] });
      }
      if (call.path === '/api/local/ai-settings/capabilities') {
        capability = { ...capability, capabilityRef: 'cap-reasoning', thinkingLevels: preset.limits.thinkingLevels };
        return ok({ revisionRef: capability.capabilityRef });
      }
      if (call.path === '/api/local/ai-settings/task-configs') {
        const thinkingLevel = (call.body.model as { limits: { thinkingLevel: string } }).limits.thinkingLevel;
        configuration = { ...configuration, taskConfigRef: 'next-config',
          model: { ...configuration.model, capabilityRef: capability.capabilityRef, limits: { ...configuration.model.limits, thinkingLevel } } };
        return ok({ taskKey: 'note_interpretation', taskConfigRef: 'next-config', modelConfigRef: 'model-config', revision: ++revision, applied: true });
      }
      return call.path === '/api/local/ai-settings' ? ok({ ...base, models: [...base.models, { ...model, capability }],
        tasks: [{ ...base.tasks[0]!, taskKey: 'note_interpretation', label: '单篇分析', configuration, revision, skillAllowed: true, requiresSkill: true }] }) : { status: 503, body: {} };
    };
    const user = userEvent.setup();
    const page = mount('/settings/models?view=advanced&area=work&task=note_interpretation');
    expect(await screen.findByRole('combobox', { name: '思考开关' })).toHaveValue('off');
    expect(screen.queryByRole('option', { name: '开启' })).not.toBeInTheDocument();
    await user.click(screen.getByRole('link', { name: '编辑模型能力声明' }));
    expect(screen.getByRole('button', { name: '技术配置' })).toHaveAttribute('aria-current', 'page');
    await user.click(screen.getByRole('button', { name: '连接概览' }));
    await act(() => page.router.navigate(-1));
    expect(screen.getByRole('button', { name: '技术配置' })).toHaveAttribute('aria-current', 'page');
    await user.click(screen.getByRole('button', { name: '读取本机离线模型目录' }));
    await screen.findByRole('option', { name: 'deepseek · DeepSeek V4 Flash' });
    expect(calls.some(call => new URLSearchParams(call.path.split('?')[1]).get('modelId') === modelId)).toBe(true);
    await user.selectOptions(screen.getByRole('combobox', { name: '目录参考' }), '0');
    await user.clear(screen.getByLabelText('最长等待秒数'));
    await user.type(screen.getByLabelText('最长等待秒数'), '90');
    await user.click(screen.getByRole('button', { name: '保存能力声明' }));
    await screen.findByText(/能力声明已保存/);
    expect(calls.find(call => call.method === 'POST')).toMatchObject({ path: '/api/local/ai-settings/capabilities', body: {
      identityRef: model.identityRef, expectedCapabilityRef: 'cap-off', source: 'pi_catalog',
      limits: { maxTimeoutSeconds: 90, thinkingLevels: preset.limits.thinkingLevels }, thinkingLevelMap: preset.thinkingLevelMap,
    } });

    page.unmount();
    const reopened = mount('/settings/models?view=advanced&area=work&task=note_interpretation');
    const toggle = await screen.findByRole('combobox', { name: '思考开关' });
    expect(toggle).toHaveValue('off');
    await user.selectOptions(toggle, 'on');
    expect(screen.getByRole('combobox', { name: '思考等级' })).toBeEnabled();
    await user.selectOptions(screen.getByRole('combobox', { name: '思考等级' }), screen.getByRole('option', { name: '高' }));
    await user.click(screen.getByRole('button', { name: '保存配置' }));
    await screen.findByText(/配置已保存，用于后续批次/);
    const writes = calls.filter(call => call.method === 'POST');
    expect(writes.map(call => call.path)).toEqual(['/api/local/ai-settings/capabilities', '/api/local/ai-settings/task-configs']);
    expect(writes[1]?.body).toMatchObject({ taskKey: 'note_interpretation', skillVersionRef: null, model: { capabilityRef: 'cap-reasoning', limits: { thinkingLevel: 'high' } } });
    reopened.unmount();
    mount('/settings/models?view=advanced&area=work&task=note_interpretation');
    expect(await screen.findByRole('combobox', { name: '思考开关' })).toHaveValue('on');
    expect(screen.getByRole('combobox', { name: '思考等级' })).toHaveValue('high');
  });

});


it('作品内容对照的正式材料许可可由设置表单提交，保存不派发任务',async()=>{
  const raw=settingsFixture();
  const configuration={...raw.configurations[0]!,taskKey:'work_comparison',taskConfigRef:'comparison-config',executionContract:'work-content-comparison.v1'};
  raw.tasks.push({...raw.tasks[0]!,taskKey:'work_comparison',label:'作品内容对照',configuration});
  raw.configurations.push(configuration);
  const data=parseAiSettings(raw),connection=data.connections[0]!;
  function Form(){return <EgressForm data={data} connection={connection} action={useAiAction()}/>;}
  respond=call=>call.method==='POST'?ok({eventRef:'00000000-0000-4000-8000-000000000009',allowed:true}):ok(raw);
  const client=new QueryClient({defaultOptions:{queries:{retry:false}}});
  render(<QueryClientProvider client={client}><Form/></QueryClientProvider>);
  const user=userEvent.setup();
  await user.selectOptions(screen.getByLabelText('工作与材料'),'work_comparison:note_material');
  await user.type(screen.getByLabelText('外发许可原因'),'合成材料授权证明');
  await user.click(screen.getByRole('button',{name:'批准这项材料外发'}));
  await waitFor(()=>expect(calls.filter(c=>c.method==='POST')).toHaveLength(1));
  expect(calls.find(c=>c.method==='POST')?.body).toMatchObject({taskKey:'work_comparison',materialClass:'note_material',allowed:true});
  expect(calls.some(c=>/batches|content-comparison/.test(c.path))).toBe(false);
});

it('当前图片识别可单独配置原图许可，选择或保存原因不会发图', async () => {
  const raw = settingsFixture();
  const configuration = { ...raw.configurations[0]!, taskKey: 'ocr_recognition', taskConfigRef: 'ocr-config', executionContract: 'ocr-recognition.v1' };
  raw.tasks.push({ ...raw.tasks[0]!, taskKey: 'ocr_recognition', label: '图片文字识别', configuration });
  raw.configurations.push(configuration);
  const data = parseAiSettings(raw), connection = data.connections[0]!;
  function Form() { return <EgressForm data={data} connection={connection} action={useAiAction()} />; }
  respond = call => call.method === 'POST' ? ok({ eventRef: '00000000-0000-4000-8000-000000000009', allowed: true }) : ok(raw);
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(<QueryClientProvider client={client}><Form /></QueryClientProvider>);
  const user = userEvent.setup();
  await user.selectOptions(screen.getByLabelText('工作与材料'), 'ocr_recognition:manual_ocr_image');
  await user.type(screen.getByLabelText('外发许可原因'), '仅配置本篇手动图片识别的材料许可');
  expect(calls.filter(call => call.method === 'POST')).toHaveLength(0);
  expect(screen.getByRole('option', { name: '图片文字识别 · 本篇手动识别原图' })).toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: '批准这项材料外发' }));
  await waitFor(() => expect(calls.filter(call => call.method === 'POST')).toHaveLength(1));
  expect(calls.find(call => call.method === 'POST')?.body).toMatchObject({ taskKey: 'ocr_recognition', materialClass: 'manual_ocr_image', allowed: true });
  expect(calls.some(call => /batches|ocr-recognition/.test(call.path))).toBe(false);
});
it('groups old topic tasks as history without changing their executable lifecycle', async () => {
  const seed = businessSettingsFixture();
  const keys = ['topic_research', 'topic_research_extract_review', 'topic_research_resolve', 'topic_research_resolve_review'];
  const labels = ['主题归并', '旧讨论审核', '旧归属判断', '旧归属审核'];
  const data = { ...seed, tasks: keys.map((taskKey, index) => ({ ...seed.tasks[0]!, taskKey, label: labels[index]!, lifecycle: 'active', configuration: { ...seed.tasks[0]!.configuration, taskKey } })) };
  respond = () => ok(data);
  mount();
  await screen.findByRole('heading', { name: '工作能力' });
  await userEvent.click(screen.getByRole('button', { name: '已接入' }));
  expect(screen.getByRole('button', { name: /主题归并/ })).toBeVisible();
  expect(screen.queryByRole('button', { name: /旧讨论审核/ })).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: '历史' }));
  for (const label of labels.slice(1)) expect(screen.getByRole('button', { name: new RegExp(label) })).toBeVisible();
  expect(screen.queryByRole('button', { name: /主题归并/ })).not.toBeInTheDocument();
  expect(data.tasks.every(task => task.lifecycle === 'active')).toBe(true);
  expect(calls.filter(call => call.method === 'POST')).toHaveLength(0);
});
