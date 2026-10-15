import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter } from 'react-router';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '../../shared/api';
import { WorkbenchPage } from './WorkbenchPage';
import * as api from './workbenchApi';
import { newDocument, type SaveReceipt, type WorkEnvelope } from './workbenchModel';
vi.mock('./workbenchApi');
vi.mock('./ImagePages', () => ({
  ImagePages: () => <div>图页组件</div>
}));
const domain = '00000000-0000-4000-8000-000000000001',
  work = '00000000-0000-4000-8000-000000000002',
  revision = '00000000-0000-4000-8000-000000000003',
  paragraph = '00000000-0000-4000-8000-000000000004';
const envelope = (): WorkEnvelope => ({
  workId: work,
  domainRef: domain,
  savedBy: 'local_machine',
  revision: {
    id: revision,
    number: 1,
    savedAt: '2026-10-15T01:00:00Z',
    document: {
      ...newDocument('真实选题'),
      paragraphs: [{
        id: paragraph,
        text: '甲😀\n原始正文'
      }]
    }
  },
  sourceStatuses: [],
  exports: []
});
function show(view = 'body') {
  const client = new QueryClient({
    defaultOptions: {
      queries: {
        retry: false
      },
      mutations: {
        retry: false
      }
    }
  });
  return render(<QueryClientProvider client={client}>
<MemoryRouter initialEntries={[`/ops/creation?domain=${domain}&work=${work}&view=${view}`]}>
<WorkbenchPage />
</MemoryRouter>
</QueryClientProvider>);
}
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(api.domains).mockResolvedValue([{
    domainRef: domain,
    name: '合成领域'
  }]);
  vi.mocked(api.listWorks).mockResolvedValue([]);
  vi.mocked(api.accounts).mockResolvedValue([]);
  vi.mocked(api.readWork).mockResolvedValue(envelope());
  vi.mocked(api.sources).mockResolvedValue({
    contract: 'content-workbench-sources.v1',
    asOf: 'now',
    items: [],
    distribution: [],
    total: 0,
    totalBasis: 'source_directory',
    returnedSourceCount: 0,
    returnedFragmentCount: 0,
    nextCursor: null,
    searchScope: 'admitted_native_title_body_history_directory',
    scope: 'collected_material_only'
  });
  vi.mocked(api.errorText).mockReturnValue('结果未确认');
  vi.mocked(api.conflictReceipt).mockReturnValue(null);
});
describe('React workbench author control', () => {
  it('does not invoke AI when mounted or while typing; composition blocks save until completed', async () => {
    show();
    const body = await screen.findByLabelText('正文第 1 段');
    fireEvent.compositionStart(body);
    fireEvent.change(body, {
      target: {
        value: '中文输入中😀'
      }
    });
    fireEvent.click(screen.getByRole('button', {
      name: '保存'
    }));
    expect(api.save).not.toHaveBeenCalled();
    expect(api.execute).not.toHaveBeenCalled();
    expect(api.preview).not.toHaveBeenCalled();
    fireEvent.compositionEnd(body);
    vi.mocked(api.save).mockImplementation(async (_id, r) => ({
      requestId: r.requestId,
      workId: work,
      outcome: 'accepted',
      replayed: false,
      revision: {
        id: crypto.randomUUID(),
        number: 2,
        savedAt: 'now',
        document: r.document
      }
    }));
    fireEvent.click(screen.getByRole('button', {
      name: '保存'
    }));
    await waitFor(() => expect(api.save).toHaveBeenCalledTimes(1));
    expect(vi.mocked(api.save).mock.calls[0]?.[1].document.paragraphs[0]?.text).toBe('中文输入中😀');
  });
  it('keeps later local typing and resolves an uncertain save by request receipt without a second command', async () => {
    vi.mocked(api.save).mockRejectedValueOnce(new ApiError({
      kind: 'network'
    }));
    vi.mocked(api.lookupWorkRequest).mockImplementation(async id => ({
      requestId: id,
      workId: work,
      outcome: 'accepted',
      replayed: true,
      revision: {
        id: crypto.randomUUID(),
        number: 2,
        savedAt: 'now',
        document: {
          ...envelope().revision.document,
          paragraphs: [{
            id: paragraph,
            text: '第一次编辑'
          }]
        }
      }
    }));
    show();
    const body = await screen.findByLabelText('正文第 1 段');
    fireEvent.change(body, {
      target: {
        value: '第一次编辑'
      }
    });
    fireEvent.click(screen.getByRole('button', {
      name: '保存'
    }));
    await screen.findByRole('button', {
      name: '沿用原请求重试'
    });
    fireEvent.change(body, {
      target: {
        value: '第二次继续编辑😀'
      }
    });
    fireEvent.click(screen.getByRole('button', {
      name: '沿用原请求重试'
    }));
    await waitFor(() => expect(api.lookupWorkRequest).toHaveBeenCalledTimes(1));
    expect(api.save).toHaveBeenCalledTimes(1);
    expect(body).toHaveValue('第二次继续编辑😀');
    expect(vi.mocked(api.lookupWorkRequest).mock.calls[0]?.[0]).toBe(vi.mocked(api.save).mock.calls[0]?.[1].requestId);
  });
  it('compares conflict versions without losing local text and restores historical versions by a new command', async () => {
    const server = envelope();
    server.revision = {
      ...server.revision,
      id: crypto.randomUUID(),
      number: 2,
      document: {
        ...server.revision.document,
        title: '另一标签的稿子'
      }
    };
    const c: SaveReceipt = {
      requestId: crypto.randomUUID(),
      workId: work,
      outcome: 'stale',
      replayed: false,
      revision: server.revision
    };
    vi.mocked(api.save).mockImplementationOnce(async (_id, request) => {
      c.requestId = request.requestId;
      throw new ApiError({
        kind: 'http',
        status: 409,
        code: 'creation_stale'
      });
    });
    vi.mocked(api.conflictReceipt).mockReturnValue(c);
    show();
    const body = await screen.findByLabelText('正文第 1 段');
    fireEvent.change(body, {
      target: {
        value: '本页新稿'
      }
    });
    fireEvent.click(screen.getByRole('button', {
      name: '保存'
    }));
    const dialog = await screen.findByRole('dialog');
    expect(within(dialog).getByText('另一标签的稿子')).toBeInTheDocument();
    expect(body).toHaveValue('本页新稿');
    fireEvent.click(within(dialog).getByRole('button', {
      name: '保留本页编辑，另存新版本'
    }));
    expect(body).toHaveValue('本页新稿');
  });
  it('keeps stable paragraph identities when Enter splits a Chinese/emoji paragraph', async () => {
    show();
    const body = await screen.findByLabelText('正文第 1 段');
    fireEvent.change(body, {
      target: {
        value: '甲😀乙'
      }
    });
    (body as HTMLTextAreaElement).setSelectionRange(3, 3);
    fireEvent.keyDown(body, {
      key: 'Enter',
      code: 'Enter'
    });
    expect(screen.getByLabelText('正文第 1 段')).toHaveValue('甲😀');
    expect(screen.getByLabelText('正文第 2 段')).toHaveValue('乙');
  });
});
describe('pending command and version protection', () => {
  it('keeps tags and body edits in memory across SPA library navigation without retrying an unknown write', async () => {
    vi.mocked(api.save).mockRejectedValueOnce(new ApiError({
      kind: 'network'
    }));
    vi.mocked(api.listWorks).mockResolvedValue([{
      workId: work,
      domainRef: domain,
      title: '真实选题',
      revisionId: revision,
      savedAt: 'now'
    }]);
    show();
    const body = await screen.findByLabelText('正文第 1 段');
    fireEvent.change(body, {
      target: {
        value: '未确认正文😀'
      }
    });
    fireEvent.click(screen.getByRole('button', {
      name: '保存'
    }));
    await screen.findByRole('button', {
      name: '沿用原请求重试'
    });
    fireEvent.change(screen.getByLabelText('确认发布标签'), {
      target: {
        value: '新标签 保留空格'
      }
    });
    fireEvent.click(screen.getByRole('button', {
      name: '我的创作'
    }));
    fireEvent.click((await screen.findByText('真实选题')).closest('button')!);
    fireEvent.click(screen.getByRole('button', {
      name: '正文'
    }));
    expect(await screen.findByLabelText('正文第 1 段')).toHaveValue('未确认正文😀');
    expect(screen.getByLabelText('确认发布标签')).toHaveValue('新标签 保留空格');
    expect(screen.getByRole('button', {
      name: '沿用原请求重试'
    })).toBeInTheDocument();
    await new Promise(resolve => setTimeout(resolve, 1000));
    expect(api.save).toHaveBeenCalledTimes(1);
  });
  it('locks replacement edits after an uncertain placement and rejects a different history command', async () => {
    const initial = envelope();
    initial.revision.document.canvas = [{
      id: paragraph,
      excerptId: null,
      group: 'inbox',
      note: '',
      expression: '自己的表达',
      expressionRevision: 1
    }];
    vi.mocked(api.readWork).mockResolvedValue(initial);
    vi.mocked(api.place).mockRejectedValue(new ApiError({
      kind: 'network'
    }));
    vi.mocked(api.history).mockResolvedValue([initial.revision, {
      ...initial.revision,
      id: crypto.randomUUID(),
      number: 0
    }]);
    show('canvas');
    fireEvent.click(await screen.findByRole('button', {
      name: '放入正文 / 指定图页 →'
    }));
    fireEvent.change(screen.getByLabelText('放入正文位置'), {
      target: {
        value: paragraph
      }
    });
    fireEvent.click(screen.getByRole('button', {
      name: '确认放入'
    }));
    await screen.findByRole('button', {
      name: '沿用原请求重试'
    });
    expect(screen.getByLabelText(`我的表达 ${paragraph}`)).toBeDisabled();
    fireEvent.click(screen.getByRole('button', {
      name: '历史'
    }));
    const restoreButtons = await screen.findAllByRole('button', {
      name: '恢复为新版本'
    });
    fireEvent.click(restoreButtons.find(b => !b.hasAttribute('disabled'))!);
    expect(api.restore).not.toHaveBeenCalled();
    expect(api.place).toHaveBeenCalledTimes(1);
  });
  it('syncs restored tags instead of autosaving old tags over the restored revision', async () => {
    const initial = envelope();
    initial.revision.document.tags = ['旧标签'];
    const restored = {
      ...initial.revision,
      id: crypto.randomUUID(),
      number: 2,
      document: {
        ...initial.revision.document,
        tags: ['恢复标签']
      }
    };
    vi.mocked(api.readWork).mockResolvedValue(initial);
    vi.mocked(api.history).mockResolvedValue([{
      ...restored,
      id: crypto.randomUUID(),
      number: 0
    }]);
    vi.mocked(api.restore).mockImplementation(async (_id, r) => ({
      requestId: r.requestId,
      workId: work,
      outcome: 'accepted',
      replayed: false,
      revision: restored
    }));
    show();
    await screen.findByLabelText('正文第 1 段');
    fireEvent.click(screen.getByRole('button', {
      name: '历史'
    }));
    fireEvent.click(await screen.findByRole('button', {
      name: '恢复为新版本'
    }));
    await waitFor(() => expect(screen.getByLabelText('确认发布标签')).toHaveValue('恢复标签'));
    expect(api.save).not.toHaveBeenCalled();
  });
});
it('keeps an uncertain create frozen and locks domain until original receipt is reconciled', async () => {
  vi.mocked(api.create).mockRejectedValueOnce(new ApiError({
    kind: 'network'
  }));
  const client = new QueryClient({
    defaultOptions: {
      queries: {
        retry: false
      }
    }
  });
  render(<QueryClientProvider client={client}>
<MemoryRouter initialEntries={[`/ops/creation?domain=${domain}`]}>
<WorkbenchPage />
</MemoryRouter>
</QueryClientProvider>);
  fireEvent.change(await screen.findByLabelText('新选题标题'), {
    target: {
      value: '从手动真实选题开始'
    }
  });
  fireEvent.click(screen.getByRole('button', {
    name: '建立选题'
  }));
  await screen.findByRole('button', {
    name: '核实原请求并重试'
  });
  expect(screen.getByLabelText('创作领域')).toBeDisabled();
  expect(screen.getByLabelText('新选题标题')).toBeDisabled();
  const frozen = vi.mocked(api.create).mock.calls[0]![0];
  vi.mocked(api.lookupWorkRequest).mockResolvedValue({
    requestId: frozen.requestId,
    workId: work,
    outcome: 'accepted',
    replayed: true,
    revision: envelope().revision
  });
  fireEvent.click(screen.getByRole('button', {
    name: '核实原请求并重试'
  }));
  await waitFor(() => expect(api.lookupWorkRequest).toHaveBeenCalledWith(frozen.requestId));
  expect(api.create).toHaveBeenCalledTimes(1);
});
it('saves a new personal canvas card at expression revision one', async () => {
  vi.mocked(api.save).mockImplementation(async (_id, r) => ({
    requestId: r.requestId,
    workId: work,
    outcome: 'accepted',
    replayed: false,
    revision: {
      id: crypto.randomUUID(),
      number: 2,
      savedAt: 'now',
      document: r.document
    }
  }));
  show('canvas');
  fireEvent.click(await screen.findByRole('button', {
    name: '＋ 写一张自己的想法'
  }));
  fireEvent.click(screen.getByRole('button', {
    name: '保存'
  }));
  await waitFor(() => expect(api.save).toHaveBeenCalledTimes(1));
  expect(vi.mocked(api.save).mock.calls[0]?.[1].document.canvas[0]?.expressionRevision).toBe(1);
});
it('retains the original AI input and action while an execution result is unknown', async () => {
  vi.mocked(api.readiness).mockResolvedValue({
    configured: true,
    transportCallable: true,
    writingQualified: true,
    configRef: domain,
    methodVersion: 'content-workbench-writing.v1'
  });
  vi.mocked(api.preview).mockResolvedValue({
    previewRef: paragraph,
    inputRevision: revision,
    inputHash: 'hash',
    action: 'draft',
    methodVersion: 'content-workbench-writing.v1',
    configRef: domain,
    modelRef: domain,
    connectionVersionRef: domain,
    sourceCount: 0,
    inputCharacterCount: 20,
    estimatedInputTokensUpperBound: 20,
    reservedTokens: 200,
    maxCalls: 1,
    outboundInput: {
      authorInstruction: '原始指令'
    },
    confirmationRequired: true
  });
  vi.mocked(api.execute).mockRejectedValueOnce(new ApiError({
    kind: 'network'
  }));
  show();
  await screen.findByLabelText('正文第 1 段');
  fireEvent.click(screen.getByRole('button', {
    name: '按材料起草'
  }));
  fireEvent.change(await screen.findByLabelText('改写指令'), {
    target: {
      value: '原始指令'
    }
  });
  const prepare = screen.getByRole('button', {
    name: '准备并核对输入'
  });
  await waitFor(() => expect(prepare).toBeEnabled());
  fireEvent.click(prepare);
  fireEvent.click(await screen.findByRole('checkbox', {
    name: '确认这次输入和额度，执行一次'
  }));
  fireEvent.click(screen.getByRole('button', {
    name: '执行一次'
  }));
  await screen.findByRole('button', {
    name: '核实原请求并重试'
  });
  fireEvent.click(screen.getByRole('button', {
    name: '关闭'
  }));
  fireEvent.click(screen.getByRole('button', {
    name: '图组文案'
  }));
  expect(screen.getByRole('heading', {
    name: '按所选材料起草'
  })).toBeInTheDocument();
  expect(screen.getByLabelText('改写指令')).toHaveValue('原始指令');
  expect(screen.getByLabelText('改写指令')).toBeDisabled();
  expect(screen.getByRole('button', {
    name: '准备并核对输入'
  })).toBeDisabled();
  expect(api.preview).toHaveBeenCalledTimes(1);
  expect(api.execute).toHaveBeenCalledTimes(1);
});
it('reports saved without POST when undo returns to the exact confirmed server document', async () => {
  show();
  const body = await screen.findByLabelText('正文第 1 段');
  fireEvent.change(body, {
    target: {
      value: '暂时的修改😀'
    }
  });
  expect(screen.getByLabelText('保存状态：未保存')).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', {
    name: '撤销'
  }));
  expect(body).toHaveValue('甲😀\n原始正文');
  expect(screen.getByLabelText('保存状态：已保存')).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', {
    name: '保存'
  }));
  await waitFor(() => expect(screen.getByLabelText('保存状态：已保存')).toBeInTheDocument());
  expect(api.save).not.toHaveBeenCalled();
});
it('opens version comparison when the lost save response is reconciled as a stale receipt', async () => {
  const actual = await vi.importActual<typeof import('./workbenchApi')>('./workbenchApi');
  vi.mocked(api.conflictReceipt).mockImplementation(actual.conflictReceipt);
  vi.mocked(api.save).mockRejectedValueOnce(new ApiError({
    kind: 'network'
  }));
  vi.mocked(api.lookupWorkRequest).mockImplementationOnce(async requestId => {
    const current = envelope();
    current.revision.document.title = '服务器较新的稿子';
    current.revision.number = 2;
    throw new ApiError({
      kind: 'http',
      status: 409,
      code: 'creation_stale'
    }, {
      error: {
        code: 'creation_stale'
      },
      receipt: {
        requestId,
        workId: work,
        outcome: 'stale',
        replayed: true,
        revision: current.revision
      }
    });
  });
  show();
  const body = await screen.findByLabelText('正文第 1 段');
  fireEvent.change(body, {
    target: {
      value: '本页未确认的新正文😀'
    }
  });
  fireEvent.click(screen.getByRole('button', {
    name: '保存'
  }));
  fireEvent.click(await screen.findByRole('button', {
    name: '沿用原请求重试'
  }));
  const dialog = await screen.findByRole('dialog', {
    name: '服务器已有新版本'
  });
  expect(within(dialog).getByText('服务器较新的稿子')).toBeInTheDocument();
  expect(within(dialog).getByText('本页未确认的新正文😀')).toBeInTheDocument();
  expect(body).toHaveValue('本页未确认的新正文😀');
  expect(screen.queryByRole('button', {
    name: '沿用原请求重试'
  })).not.toBeInTheDocument();
  expect(api.save).toHaveBeenCalledTimes(1);
});
it('shows AI failure guidance while retaining the technical reason only in optional diagnostics', async () => {
  const actual = await vi.importActual<typeof import('./workbenchApi')>('./workbenchApi');
  vi.mocked(api.aiFailureText).mockImplementation(actual.aiFailureText);
  vi.mocked(api.aiStateText).mockImplementation(actual.aiStateText);
  vi.mocked(api.readiness).mockResolvedValue({
    configured: true,
    transportCallable: true,
    writingQualified: true,
    configRef: domain,
    methodVersion: 'content-workbench-writing.v1'
  });
  vi.mocked(api.preview).mockResolvedValue({
    previewRef: paragraph,
    inputRevision: revision,
    inputHash: 'hash',
    action: 'draft',
    methodVersion: 'content-workbench-writing.v1',
    configRef: domain,
    modelRef: domain,
    connectionVersionRef: domain,
    sourceCount: 0,
    inputCharacterCount: 20,
    estimatedInputTokensUpperBound: 20,
    reservedTokens: 200,
    maxCalls: 1,
    outboundInput: {},
    confirmationRequired: true
  });
  vi.mocked(api.execute).mockImplementation(async request => ({
    requestId: request.requestId,
    workRef: work,
    invocationRef: paragraph,
    state: 'failed',
    inputRevision: revision,
    action: 'draft',
    methodVersion: 'content-workbench-writing.v1',
    suggestion: null,
    sourceQualification: 'readable',
    failureCode: 'writing_output_invalid',
    staleBase: false,
    usage: {
      inputTokens: null,
      outputTokens: null,
      chargedTokens: 0,
      costUsd: null
    }
  }));
  show();
  await screen.findByLabelText('正文第 1 段');
  fireEvent.click(screen.getByRole('button', {
    name: '按材料起草'
  }));
  const prepare = await screen.findByRole('button', {
    name: '准备并核对输入'
  });
  await waitFor(() => expect(prepare).toBeEnabled());
  fireEvent.click(prepare);
  fireEvent.click(await screen.findByRole('checkbox', {
    name: '确认这次输入和额度，执行一次'
  }));
  fireEvent.click(screen.getByRole('button', {
    name: '执行一次'
  }));
  expect(await screen.findByText('这次返回的建议暂时无法使用，正文没有改变。可调整要求后重新准备，也可以继续手写。')).toBeVisible();
  expect(screen.getByLabelText('正文第 1 段')).toHaveValue('甲😀\n原始正文');
  expect(screen.getByText(/"failureCode": "writing_output_invalid"/)).not.toBeVisible();
});
