import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ModelSettingsPage } from './ModelSettingsPage';

type Handler = (body: unknown) => { status: number; body: unknown } | 'network';

interface Call {
  path: string;
  method: string;
  body: unknown;
}

const settings = (overrides: Record<string, unknown> = {}) => ({
  workspaceRef: 'w',
  secretStorage: 'SYNTHETIC_PREVIEW_ONLY',
  worker: { lastSeenAt: '2026-10-15 14:30:00+08', recent: true, state: 'idle', lastError: null },
  connections: [
    {
      connectionRef: 'c1',
      versionRef: 'v1',
      name: '本机合成连接',
      api: 'openai-completions',
      baseUrl: 'http://127.0.0.1:9/v1',
      localEndpoint: true,
      enabled: true,
      revision: 1,
      test: null,
      credentialStored: true,
    },
  ],
  models: [
    {
      modelRef: 'm1',
      modelId: 'model-qualified',
      connectionVersionRef: 'v1',
      connectionName: '本机合成连接',
      currentVersion: true,
      semanticQualified: true,
      modelCallable: true,
      testState: 'succeeded',
      enabled: true,
      test: null,
    },
    {
      modelRef: 'm2',
      modelId: 'model-untested',
      connectionVersionRef: 'v1',
      connectionName: '本机合成连接',
      currentVersion: true,
      semanticQualified: false,
      modelCallable: false,
      testState: null,
      enabled: true,
      test: null,
    },
  ],
  config: null,
  invocations: [
    {
      invocationRef: 'i1',
      operation: 'probe',
      state: 'failed',
      failureCode: 'provider_timeout',
      inputTokens: null,
      outputTokens: null,
      chargedTokens: 1024,
      reservedTokens: 1024,
      elapsedMs: null,
      createdAt: '2026-10-15 06:00:00+00',
      modelId: 'model-qualified',
      result: null,
    },
  ],
  ...overrides,
});

let calls: Call[];
let handlers: Record<string, Handler[]>;

function route(method: string, path: string, ...responses: Handler[]) {
  handlers[`${method} ${path}`] = responses;
}

beforeEach(() => {
  calls = [];
  handlers = {};
  vi.stubGlobal(
    'fetch',
    vi.fn(async (input: string, init?: RequestInit) => {
      const method = init?.method ?? 'GET';
      const body = typeof init?.body === 'string' ? JSON.parse(init.body) : undefined;
      calls.push({ path: input, method, body });
      const queue = handlers[`${method} ${input}`];
      const handler = queue && queue.length > 1 ? queue.shift() : queue?.[0];
      if (!handler) throw new Error(`no handler for ${method} ${input}`);
      const result = handler(body);
      if (result === 'network') throw new TypeError('Failed to fetch');
      return new Response(JSON.stringify(result.body), { status: result.status });
    }),
  );
});

afterEach(() => {
  vi.unstubAllGlobals();
});

function renderPage() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <ModelSettingsPage />
    </QueryClientProvider>,
  );
}

const ok = (body: unknown) => () => ({ status: 200, body });
const posts = (path: string) => calls.filter(call => call.method === 'POST' && call.path === path);

describe('模型设置页', () => {
  it('展示连接、模型状态、默认配置提示、执行器与调用记录', async () => {
    route('GET', '/api/local/model-settings', ok(settings()));
    renderPage();
    expect(await screen.findByText('本机合成连接')).toBeInTheDocument();
    expect(screen.getByText(/OpenAI 兼容 Chat Completions · http:\/\/127\.0\.0\.1:9\/v1/)).toBeInTheDocument();
    const connections = screen.getAllByRole('table')[0] as HTMLElement;
    expect(within(connections).getByText('可用于评论研究 V1', { exact: false })).toBeInTheDocument();
    expect(within(connections).getByText('需要完成模型测试', { exact: false })).toBeInTheDocument();
    expect(screen.getByText('尚未保存默认研究模型。先添加并测试连接。')).toBeInTheDocument();
    expect(screen.getByText('执行器最近活动：2026-10-15 14:30。')).toBeInTheDocument();
    const ledger = screen.getAllByRole('table')[1] as HTMLElement;
    expect(within(ledger).getByText('2026-10-15 14:00')).toBeInTheDocument();
    expect(within(ledger).getByText('能力测试 · model-qualified')).toBeInTheDocument();
    expect(within(ledger).getByText('provider_timeout')).toBeInTheDocument();
    expect(within(ledger).getByText('输入 未知 · 输出 未知 · 计入 1024 Token')).toBeInTheDocument();
    // 未通过测试的模型不能被选为默认研究模型。
    const select = screen.getByRole('combobox', { name: '研究模型' });
    expect(within(select).getByRole('option', { name: /model-untested/ })).toBeDisabled();
    expect(within(select).getByRole('option', { name: /model-qualified/ })).toBeEnabled();
    expect(document.title).toContain('模型与 AI');
  });

  it('空状态如实说明', async () => {
    route('GET', '/api/local/model-settings', ok(settings({ connections: [], models: [], invocations: [], worker: { lastSeenAt: null, recent: false, lastError: 'adapter_missing' } })));
    renderPage();
    expect(await screen.findByText('还没有供应商连接。')).toBeInTheDocument();
    expect(screen.getByText('还没有调用记录。')).toBeInTheDocument();
    expect(screen.getByText('执行器未在最近 90 秒报告活动。上次错误：adapter_missing')).toBeInTheDocument();
  });

  it('读取失败时说明原因并保留错误码', async () => {
    route('GET', '/api/local/model-settings', () => ({ status: 503, body: { error: 'model_database_unavailable' } }));
    renderPage();
    const alert = await screen.findByRole('alert');
    expect(alert).toHaveTextContent('读取失败');
    expect(alert).toHaveTextContent('模型设置数据库暂不可用。');
    expect(alert).toHaveTextContent('model_database_unavailable');
  });

  it('停用连接：带基础修订号提交，回执后才显示结果', async () => {
    route('GET', '/api/local/model-settings', ok(settings()));
    route('POST', '/api/local/model-settings/connections/state', ok({ enabled: false, revision: 2 }));
    const user = userEvent.setup();
    renderPage();
    await user.click(await screen.findByRole('button', { name: '停用' }));
    expect(await screen.findByText('连接已停用。')).toBeInTheDocument();
    expect(posts('/api/local/model-settings/connections/state')[0]?.body).toEqual({ connectionRef: 'c1', expectedRevision: 1, enabled: false });
  });

  it('测试结果未确认后重试，沿用同一个请求编号，不重复计费', async () => {
    route('GET', '/api/local/model-settings', ok(settings()));
    route(
      'POST',
      '/api/local/model-settings/probes',
      () => 'network',
      () => ({ status: 200, body: { invocationRef: 'x', state: 'succeeded', replayed: true, result: { semanticQualified: true } } }),
    );
    const user = userEvent.setup();
    renderPage();
    const testButtons = await screen.findAllByRole('button', { name: '测试' });
    await user.click(testButtons[0] as HTMLElement);
    expect(await screen.findByText(/结果未确认/)).toBeInTheDocument();
    await user.click(screen.getAllByRole('button', { name: '测试' })[0] as HTMLElement);
    expect(await screen.findByText('模型 V1 JSON 测试通过。')).toBeInTheDocument();
    const probes = posts('/api/local/model-settings/probes');
    expect(probes).toHaveLength(2);
    const [first, second] = probes.map(call => call.body as { invocationRef: string });
    expect(second?.invocationRef).toBe(first?.invocationRef);
  });

  it('保存默认配置：只允许已通过测试的模型，回执后提示没有启动研究', async () => {
    route('GET', '/api/local/model-settings', ok(settings()));
    route('POST', '/api/local/model-settings/config', ok({ configRef: 'new', state: 'SAVED_NO_ANALYSIS_STARTED' }));
    const user = userEvent.setup();
    renderPage();
    await user.click(await screen.findByRole('button', { name: '保存默认配置' }));
    expect(posts('/api/local/model-settings/config')).toHaveLength(0);
    await user.selectOptions(screen.getByRole('combobox', { name: '研究模型' }), 'm1');
    await user.click(screen.getByRole('button', { name: '保存默认配置' }));
    expect(await screen.findByText('默认研究模型已保存；没有启动评论研究。')).toBeInTheDocument();
    expect(posts('/api/local/model-settings/config')[0]?.body).toMatchObject({
      expectedConfigRef: null,
      modelRef: 'm1',
      inputTokenLimit: 16000,
      outputTokenLimit: 2000,
      timeoutSeconds: 30,
      maxAttempts: 2,
    });
  });

  it('添加连接并测试：连接、模型、测试依次提交，测试结论显示在对话框里', async () => {
    route('GET', '/api/local/model-settings', ok(settings({ connections: [], models: [] })));
    route('POST', '/api/local/model-settings/connections', body => ({
      status: 200,
      body: { connectionRef: (body as { connectionRef: string }).connectionRef, versionRef: (body as { versionRef: string }).versionRef, revision: 1, state: 'SAVED_UNTESTED' },
    }));
    route('POST', '/api/local/model-settings/models', body => ({ status: 200, body: { modelRef: (body as { modelRef: string }).modelRef, state: 'UNTESTED' } }));
    route('POST', '/api/local/model-settings/probes', ok({ ok: true, modelCallable: true, semanticQualified: false }));
    const user = userEvent.setup();
    renderPage();
    await user.click(await screen.findByRole('button', { name: '添加连接' }));
    const dialog = await screen.findByRole('dialog', { name: '添加供应商连接' });
    await user.type(within(dialog).getByLabelText('连接名称'), '合成连接');
    await user.type(within(dialog).getByLabelText('API 地址'), 'http://127.0.0.1:9/v1');
    await user.type(within(dialog).getByLabelText('API Key'), 'SYNTHETIC-NOT-A-CREDENTIAL');
    await user.type(within(dialog).getByLabelText('模型 ID'), 'synthetic-model');
    await user.click(within(dialog).getByLabelText('本机回环服务（允许 HTTP）'));
    await user.click(within(dialog).getByRole('button', { name: '保存并测试' }));
    expect(await within(dialog).findByText('模型已返回，但 V1 JSON 测试未通过。')).toBeInTheDocument();
    const [connection] = posts('/api/local/model-settings/connections');
    const [model] = posts('/api/local/model-settings/models');
    const [probe] = posts('/api/local/model-settings/probes');
    expect(connection?.body).toMatchObject({ expectedRevision: 0, name: '合成连接', localEndpoint: true, api: 'openai-completions' });
    expect(model?.body).toMatchObject({ connectionVersionRef: (connection?.body as { versionRef: string }).versionRef, modelId: 'synthetic-model' });
    expect(probe?.body).toMatchObject({ operation: 'probe', modelRef: (model?.body as { modelRef: string }).modelRef });
    // 输入没变时再点「保存并测试」，只重新测试，不再建新版本。
    await user.click(within(dialog).getByRole('button', { name: '保存并测试' }));
    await waitFor(() => expect(posts('/api/local/model-settings/probes')).toHaveLength(2));
    expect(posts('/api/local/model-settings/connections')).toHaveLength(1);
  });

  it('只保存连接：关闭对话框，提示尚未发送测试请求', async () => {
    route('GET', '/api/local/model-settings', ok(settings()));
    route('POST', '/api/local/model-settings/connections', body => ({
      status: 200,
      body: { connectionRef: 'c1', versionRef: (body as { versionRef: string }).versionRef, revision: 2, state: 'SAVED_UNTESTED' },
    }));
    route('POST', '/api/local/model-settings/models', body => ({ status: 200, body: { modelRef: (body as { modelRef: string }).modelRef, state: 'UNTESTED' } }));
    const user = userEvent.setup();
    renderPage();
    await user.click(await screen.findByRole('button', { name: '编辑' }));
    const dialog = await screen.findByRole('dialog', { name: '编辑供应商连接' });
    expect(within(dialog).getByLabelText('API Key（留空沿用）')).toHaveValue('');
    await user.click(within(dialog).getByRole('button', { name: '保存' }));
    expect(await screen.findByText('供应商连接已保存；尚未发送测试请求。')).toBeInTheDocument();
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    const [connection] = posts('/api/local/model-settings/connections');
    expect(connection?.body).toMatchObject({ connectionRef: 'c1', expectedRevision: 1, apiKey: '' });
    // 编辑会产生新的连接版本，模型条目按新版本重新登记，不沿用旧版本的模型编号。
    const [model] = posts('/api/local/model-settings/models');
    expect((model?.body as { modelRef: string }).modelRef).not.toBe('m1');
    expect(posts('/api/local/model-settings/probes')).toHaveLength(0);
  });

  it('对话框里的服务端拒绝显示中文原因与错误码', async () => {
    route('GET', '/api/local/model-settings', ok(settings()));
    route('POST', '/api/local/model-settings/connections', () => ({ status: 409, body: { error: 'model_revision_conflict' } }));
    const user = userEvent.setup();
    renderPage();
    await user.click(await screen.findByRole('button', { name: '编辑' }));
    const dialog = await screen.findByRole('dialog', { name: '编辑供应商连接' });
    await user.click(within(dialog).getByRole('button', { name: '保存' }));
    expect(await within(dialog).findByText('设置已变化，请刷新后重试。')).toBeInTheDocument();
    expect(within(dialog).getByText('model_revision_conflict')).toBeInTheDocument();
  });
});

describe('不在列表里的接口协议', () => {
  it('编辑时如实显示原协议，保存时不悄悄改成第一项', async () => {
    const base = settings();
    const wemm = { ...base.connections[0], connectionRef: 'c-wemm', versionRef: 'v-wemm', name: '本机 WeMM Runtime', api: 'local-wemm-runtime', baseUrl: 'local://wemm-embedding-2b' };
    route('GET', '/api/local/model-settings', ok(settings({ connections: [wemm], models: [] })));
    route('POST', '/api/local/model-settings/connections', () => ({ status: 400, body: { error: 'invalid_model_command' } }));
    const user = userEvent.setup();
    renderPage();
    await user.click(await screen.findByRole('button', { name: '编辑' }));
    const dialog = await screen.findByRole('dialog', { name: '编辑供应商连接' });
    expect(within(dialog).getByLabelText('接口协议')).toHaveValue('local-wemm-runtime');
    await user.type(within(dialog).getByLabelText('模型 ID'), 'wemm');
    await user.click(within(dialog).getByRole('button', { name: '保存' }));
    expect(await within(dialog).findByText('输入不符合模型设置要求。')).toBeInTheDocument();
    expect(posts('/api/local/model-settings/connections')[0]?.body).toMatchObject({ api: 'local-wemm-runtime' });
  });
});

describe('默认配置数字输入', () => {
  it('清空数字框不会提交，也不会把非数字写进请求', async () => {
    route('GET', '/api/local/model-settings', ok(settings()));
    route('POST', '/api/local/model-settings/config', ok({ configRef: 'new', state: 'SAVED_NO_ANALYSIS_STARTED' }));
    const user = userEvent.setup();
    renderPage();
    await user.selectOptions(await screen.findByRole('combobox', { name: '研究模型' }), 'm1');
    const timeout = screen.getByLabelText('单次等待（秒）');
    await user.clear(timeout);
    await user.click(screen.getByRole('button', { name: '保存默认配置' }));
    expect(posts('/api/local/model-settings/config')).toHaveLength(0);
    await user.type(timeout, '45');
    await user.click(screen.getByRole('button', { name: '保存默认配置' }));
    expect(await screen.findByText('默认研究模型已保存；没有启动评论研究。')).toBeInTheDocument();
    expect(posts('/api/local/model-settings/config')[0]?.body).toMatchObject({ timeoutSeconds: 45, inputTokenLimit: 16000 });
  });
});
