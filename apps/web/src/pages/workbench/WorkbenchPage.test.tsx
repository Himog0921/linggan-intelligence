import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { createMemoryRouter, Link, RouterProvider, useNavigate } from 'react-router';
import { useState, type ReactNode } from 'react';
import { webcrypto } from 'node:crypto';
function TestRouter({ initialEntries, children }: { initialEntries: string[]; children: ReactNode }) {
  const [router] = useState(() => createMemoryRouter([{ path: '*', element: children }], { initialEntries }));
  return <RouterProvider router={router} />;
}
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '../../shared/api';
import { WorkbenchPage } from './WorkbenchPage';
import * as api from './workbenchApi';
import { newDocument, type AiStatus, type SaveReceipt, type WorkEnvelope } from './workbenchModel';
Object.defineProperty(globalThis.crypto, 'subtle', { value: webcrypto.subtle, configurable: true });
vi.mock('./workbenchApi');
vi.mock('./MaterialBrowser', async () => {
  const actual = await vi.importActual<typeof import('./MaterialBrowser')>('./MaterialBrowser');
  return { MaterialBrowser: (props: Parameters<typeof actual.MaterialBrowser>[0]) => <>
    <actual.MaterialBrowser {...props}/>
    <button onClick={() => props.onCollect({citation: {asOf:'2026-10-29',blockRef:'body',sourceKind:'comment',sourceRef:'00000000-0000-4000-8000-000000000002',workRef:'00000000-0000-4000-8000-000000000002',field:'comment_body',sourceVersion:'v1',start:0,end:2,textHash:'a'.repeat(64),location:null},selectedText:'合成',textHash:'a'.repeat(64)}, 'inbox')}>测试收集未保存来源</button>
  </> };
});
vi.mock('./ImagePages', () => ({
  ImagePages: () => <div>图页组件</div>
}));
// The check panel and the asset library have their own tests; these tests are about the editor.
let checkPanelProps: { saveCurrent: () => Promise<{ number: number }>; pendingConfirmation: string | null; setPendingConfirmation: (receipt: string | null) => void } | null = null;
vi.mock('./assets/CheckPanel', () => ({
  CheckPanel: (props: NonNullable<typeof checkPanelProps>) => {
    checkPanelProps = props;
    return <div>检查面板 <span>{props.pendingConfirmation}</span><button onClick={() => props.setPendingConfirmation('待确认回执')}>测试待确认回执</button></div>;
  }
}));
vi.mock('./assets/AssetsLibrary', () => ({
  AssetsLibrary: ({ domainRef, onDirtyChange }: { domainRef: string; onDirtyChange: (dirty: boolean) => void }) => <div>资产库页 {domainRef}<button onClick={() => onDirtyChange(true)}>测试资产未保存</button><Link to={`/ops/creation?domain=${domainRef}&work=${work}&view=body`}>测试来源链接</Link></div>
}));
vi.mock('./assets/PublicationLinker', () => ({
  PublicationLinker: ({ pending, onPending }: { pending: { kind: string; command?: { requestId: string } } | null; onPending: (value: unknown) => void }) => <div>
    <span data-testid="pending-publication">{pending?.command?.requestId ?? ''}</span>
    <button onClick={() => onPending({ kind: 'link', command: { requestId: 'synthetic-request', expectedRevision: revision, publicationRef: work } })}>测试待确认发布关联</button>
  </div>,
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
<TestRouter initialEntries={[`/ops/creation?domain=${domain}&work=${work}&view=${view}`]}>
<WorkbenchPage />
</TestRouter>
</QueryClientProvider>);
}
beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(api.readiness).mockReset().mockResolvedValue({ configured: false, transportCallable: null, writingQualified: null, configRef: null, methodVersion: 'content-workbench-writing.v1' });
  vi.mocked(api.domains).mockResolvedValue([{
    domainRef: domain,
    name: '合成领域'
  }]);
  vi.mocked(api.listWorks).mockResolvedValue([]);
  vi.mocked(api.listIdeaBoxes).mockResolvedValue([]);
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
  it('keeps editing commands and the save state available in the compact writing menu', async () => {
    show();
    await screen.findByLabelText('正文第 1 段');
    const summary = screen.getByText('写稿工具 · 已保存');
    const menu = summary.closest('details');
    expect(menu).not.toHaveAttribute('open');
    fireEvent.click(summary);
    expect(menu).toHaveAttribute('open');
    expect(within(menu!).getByRole('button', { name: '保存' })).toBeInTheDocument();
    expect(within(menu!).getByRole('button', { name: '历史' })).toBeInTheDocument();
    expect(within(menu!).getByRole('button', { name: '按材料起草' })).toBeInTheDocument();
  });
  it('offers withdrawal recovery even when the current draft no longer has an experience slot', async () => {
    vi.mocked(api.readWork).mockResolvedValue({ ...envelope(), experienceResetRequired: true });
    show();
    expect(await screen.findByRole('button', { name: '撤回后重新开始编辑' })).toBeInTheDocument();
    expect(screen.queryByRole('textbox', { name: '正文第 1 段' })).not.toBeInTheDocument();
  });
  it('hides an open editor on return after another page withdraws an experience', async () => {
    show();
    const body = await screen.findByLabelText('正文第 1 段');
    fireEvent.change(body, { target: { value: '尚未保存的合成文字' } });
    vi.mocked(api.readWork).mockResolvedValue({ ...envelope(), experienceResetRequired: true });
    fireEvent.focus(window);
    expect(await screen.findByRole('button', { name: '撤回后重新开始编辑' })).toBeInTheDocument();
    expect(screen.queryByRole('textbox', { name: '正文第 1 段' })).not.toBeInTheDocument();
    expect(api.resetWithdrawn).not.toHaveBeenCalled();
  });
  it('clears unfinished candidate and idea inputs when a withdrawal invalidates the editor', async () => {
    show();
    await screen.findByLabelText('正文第 1 段');
    fireEvent.click(screen.getByText('添加笔记标题候选'));
    fireEvent.change(screen.getByLabelText('新标题候选'), { target: { value: '待撤回的合成输入' } });
    fireEvent.click(screen.getByText('补一个选题框'));
    fireEvent.change(screen.getByLabelText('新选题框名称'), { target: { value: '待撤回的合成选题' } });
    vi.mocked(api.readWork).mockResolvedValue({ ...envelope(), experienceResetRequired: true });
    fireEvent.focus(window);
    await screen.findByRole('button', { name: '撤回后重新开始编辑' });
    await waitFor(() => {
      const event = new Event('beforeunload', { cancelable: true });
      window.dispatchEvent(event);
      expect(event.defaultPrevented).toBe(false);
    });
  });
  it('keeps the editor covered when rechecking fails and allows retry', async () => {
    show();
    await screen.findByLabelText('正文第 1 段');
    vi.mocked(api.readWork).mockRejectedValueOnce(new Error('offline'));
    fireEvent.focus(window);
    expect(await screen.findByText(/暂时无法核验稿件来源/)).toBeInTheDocument();
    expect(screen.queryByRole('textbox', { name: '正文第 1 段' })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '重新核验' }));
    expect(await screen.findByLabelText('正文第 1 段')).toBeVisible();
  });
  it('covers an old editor even when another page already completed the withdrawal reset', async () => {
    show();
    await screen.findByLabelText('正文第 1 段');
    const latest = envelope();
    latest.revision.document.experienceEpoch = '00000000-0000-4000-8000-000000000099';
    vi.mocked(api.readWork).mockResolvedValue(latest);
    fireEvent.focus(window);
    expect(await screen.findByRole('button', { name: '撤回后重新开始编辑' })).toBeInTheDocument();
    expect(screen.queryByRole('textbox', { name: '正文第 1 段' })).not.toBeInTheDocument();
  });
  it('waits for a pending recheck before allowing a withdrawal reset', async () => {
    let finish!: (value: WorkEnvelope) => void;
    const response = new Promise<WorkEnvelope>(resolve => { finish = resolve; });
    const withdrawn = { ...envelope(), experienceResetRequired: true };
    vi.mocked(api.readWork).mockResolvedValueOnce(withdrawn).mockImplementationOnce(() => response);
    show();
    const button = await screen.findByRole('button', { name: '撤回后重新开始编辑' });
    expect(button).toBeDisabled();
    await act(async () => finish(withdrawn));
    await waitFor(() => expect(button).toBeEnabled());
  });
  it('rechecks a cached draft when switching away and back after a withdrawal', async () => {
    const other = '00000000-0000-4000-8000-000000000098';
    let withdrawn = false;
    vi.mocked(api.readWork).mockImplementation(async id => id === other
      ? { ...envelope(), workId: other }
      : { ...envelope(), experienceResetRequired: withdrawn });
    function Navigation() {
      const navigate = useNavigate();
      return <><button onClick={() => navigate(`/ops/creation?domain=${domain}&work=${other}&view=body`)}>打开合成稿 B</button>
        <button onClick={() => navigate(`/ops/creation?domain=${domain}&work=${work}&view=body`)}>返回合成稿 A</button></>;
    }
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(<QueryClientProvider client={client}><TestRouter initialEntries={[`/ops/creation?domain=${domain}&work=${work}&view=body`]}><Navigation /><WorkbenchPage /></TestRouter></QueryClientProvider>);
    await screen.findByLabelText('正文第 1 段');
    fireEvent.click(screen.getByRole('button', { name: '打开合成稿 B' }));
    await waitFor(() => expect(api.readWork).toHaveBeenCalledWith(other, expect.anything()));
    withdrawn = true;
    fireEvent.click(screen.getByRole('button', { name: '返回合成稿 A' }));
    expect(await screen.findByRole('button', { name: '撤回后重新开始编辑' })).toBeInTheDocument();
    expect(screen.queryByRole('textbox', { name: '正文第 1 段' })).not.toBeInTheDocument();
  });
  it('closes the selection dialog when rechecking finds a withdrawal', async () => {
    show();
    const body = await screen.findByLabelText('正文第 1 段') as HTMLTextAreaElement;
    body.setSelectionRange(0, 2);
    fireEvent.select(body);
    fireEvent.click(screen.getByRole('button', { name: '改写选段' }));
    expect(await screen.findByRole('dialog', { name: '改写选段' })).toBeInTheDocument();
    vi.mocked(api.readWork).mockResolvedValue({ ...envelope(), experienceResetRequired: true });
    fireEvent.focus(window);
    await screen.findByRole('button', { name: '撤回后重新开始编辑' });
    expect(screen.queryByRole('dialog', { name: '改写选段' })).not.toBeInTheDocument();
  });
  it('rechecks an open draft after a withdrawal notification from another page', async () => {
    const channels: Array<{ onmessage: ((event: { data: string }) => void) | null }> = [];
    class SyntheticChannel {
      onmessage: ((event: { data: string }) => void) | null = null;
      constructor() { channels.push(this); }
      close() {}
    }
    vi.stubGlobal('BroadcastChannel', SyntheticChannel);
    try {
      show();
      await screen.findByLabelText('正文第 1 段');
      vi.mocked(api.readWork).mockResolvedValue({ ...envelope(), experienceResetRequired: true });
      act(() => channels.forEach(channel => channel.onmessage?.({ data: 'withdrawn' })));
      expect(await screen.findByRole('button', { name: '撤回后重新开始编辑' })).toBeInTheDocument();
      expect(screen.queryByRole('textbox', { name: '正文第 1 段' })).not.toBeInTheDocument();
    } finally { vi.unstubAllGlobals(); }
  });
  it('covers the editor immediately when withdrawal arrives during an in-flight save', async () => {
    const channels: Array<{ onmessage: ((event: { data: string }) => void) | null }> = [];
    class SyntheticChannel {
      onmessage: ((event: { data: string }) => void) | null = null;
      constructor() { channels.push(this); }
      close() {}
    }
    vi.stubGlobal('BroadcastChannel', SyntheticChannel);
    let finish!: () => void;
    vi.mocked(api.save).mockImplementationOnce((_id, request) => new Promise<SaveReceipt>(resolve => {
      finish = () => resolve({ requestId: request.requestId, workId: work, outcome: 'accepted', replayed: false,
        revision: { ...envelope().revision, document: request.document } });
    }));
    try {
      show();
      fireEvent.change(await screen.findByLabelText('正文第 1 段'), { target: { value: '合成编辑' } });
      fireEvent.click(screen.getByRole('button', { name: '保存' }));
      await waitFor(() => expect(api.save).toHaveBeenCalled());
      vi.mocked(api.readWork).mockResolvedValue({ ...envelope(), experienceResetRequired: true });
      act(() => channels.forEach(channel => channel.onmessage?.({ data: 'withdrawn' })));
      expect(screen.queryByRole('textbox', { name: '正文第 1 段' })).not.toBeInTheDocument();
      await act(async () => finish());
      expect(await screen.findByRole('button', { name: '撤回后重新开始编辑' })).toBeInTheDocument();
    } finally { vi.unstubAllGlobals(); }
  });
  it('does not allow the comparison editor to fill an unresolved experience gap', async () => {
    show(); await screen.findByLabelText('正文第 1 段');
    fireEvent.click(screen.getByRole('button', { name: '留一个经历位' }));
    expect(screen.getByLabelText('正文第 2 段')).toHaveAttribute('readonly');
    expect(screen.getByRole('button', { name: '改稿对照' })).toBeDisabled();
  });
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
  it('does not offer to keep stale editor text after an experience reset', async () => {
    const next = envelope().revision;
    next.id = crypto.randomUUID();
    next.number = 2;
    next.document = { ...next.document, experienceEpoch: crypto.randomUUID(), paragraphs: [{ id: paragraph, text: '服务器安全版本' }] };
    const conflict: SaveReceipt = { requestId: '', workId: work, outcome: 'stale', replayed: false, revision: next };
    vi.mocked(api.save).mockImplementationOnce(async (_id, request) => {
      conflict.requestId = request.requestId;
      throw new ApiError({ kind: 'http', status: 409, code: 'creation_stale' });
    });
    vi.mocked(api.conflictReceipt).mockReturnValue(conflict);
    show();
    fireEvent.change(await screen.findByLabelText('正文第 1 段'), { target: { value: '合成的旧编辑' } });
    fireEvent.click(screen.getByRole('button', { name: '保存' }));
    const dialog = await screen.findByRole('dialog');
    expect(within(dialog).queryByText('合成的旧编辑')).not.toBeInTheDocument();
    expect(within(dialog).queryByRole('button', { name: '保留本页编辑，另存新版本' })).not.toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole('button', { name: '丢弃本页编辑，使用服务器版本' }));
    expect(screen.getByLabelText('正文第 1 段')).toHaveValue('服务器安全版本');
  });
  it('asks before discarding an editor after withdrawal and resumes from the server copy', async () => {
    vi.mocked(api.save).mockRejectedValueOnce(new ApiError({ kind: 'http', status: 409, code: 'creation_experience_reset_required' }));
    vi.mocked(api.resetWithdrawn).mockImplementation(async (_id, command) => ({
      requestId: command.requestId, workId: work, outcome: 'accepted', replayed: false,
      revision: { ...envelope().revision, id: crypto.randomUUID(), number: 2,
        document: { ...envelope().revision.document, experienceEpoch: crypto.randomUUID(), paragraphs: [{ id: paragraph, text: '服务器保留正文' }] } },
    }));
    show();
    fireEvent.change(await screen.findByLabelText('正文第 1 段'), { target: { value: '合成的未保存编辑' } });
    fireEvent.click(screen.getByRole('button', { name: '保存' }));
    fireEvent.click(await screen.findByRole('button', { name: '撤回后重新开始编辑' }));
    const dialog = await screen.findByRole('dialog', { name: '撤回后重新开始编辑' });
    expect(within(dialog).getByText(/尚未保存的编辑/)).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole('button', { name: '丢弃本页编辑并继续' }));
    await waitFor(() => expect(screen.getByLabelText('正文第 1 段')).toHaveValue('服务器保留正文'));
    expect(api.resetWithdrawn).toHaveBeenCalledTimes(1);
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
  it('keeps an uncertain publication request and blocks navigation until resolved', async () => {
    show(); await screen.findByLabelText('正文第 1 段');
    fireEvent.click(screen.getByRole('button', { name: '测试待确认发布关联' }));
    const leave = new Event('beforeunload', { cancelable: true });
    window.dispatchEvent(leave);
    expect(leave.defaultPrevented).toBe(true);
    fireEvent.click(screen.getByRole('button', { name: '制作封面 →' }));
    expect(await screen.findByText(/操作结果尚未确认/)).toBeInTheDocument();
    expect(screen.getByTestId('pending-publication')).toHaveTextContent('synthetic-request');
  });
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
      name: '全部稿件'
    }));
    fireEvent.click(await screen.findByRole('button', { name: /真实选题.*继续创作/s }));
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
    }].map(r => ({ id: r.id, number: r.number, savedAt: r.savedAt, referenceFormat: 'current' as const })));
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
    }].map(r => ({ id: r.id, number: r.number, savedAt: r.savedAt, referenceFormat: 'current' as const })));
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
<TestRouter initialEntries={[`/ops/creation?domain=${domain}`]}>
<WorkbenchPage />
</TestRouter>
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
  expect(screen.getByLabelText('创作领域')).toHaveAttribute('aria-disabled', 'true');
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
    estimatedInputTokens: 20,
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
    name: '已核对不含真实姓名、学校、密码和点赞数，确认输入及额度'
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
    estimatedInputTokens: 20,
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
      state: 'unknown_reserved',
      inputTokens: null,
      outputTokens: null,
      chargedTokens: 200,
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
    name: '已核对不含真实姓名、学校、密码和点赞数，确认输入及额度'
  }));
  fireEvent.click(screen.getByRole('button', {
    name: '执行一次'
  }));
  expect(await screen.findByText('这次返回的建议暂时无法使用，正文没有改变。可调整要求后重新准备，也可以继续手写。')).toBeVisible();
  expect(screen.getByLabelText('正文第 1 段')).toHaveValue('甲😀\n原始正文');
  expect(screen.getByText(/"failureCode": "writing_output_invalid"/)).not.toBeVisible();
});

it('shows an unreserved queued request and lets the user cancel it without showing zero consumption', async () => {
  const actual=await vi.importActual<typeof import('./workbenchApi')>('./workbenchApi');
  vi.mocked(api.aiStateText).mockImplementation(actual.aiStateText);
  vi.mocked(api.readiness).mockResolvedValue({configured:true,transportCallable:true,writingQualified:true,configRef:domain,methodVersion:'content-workbench-writing.v1'});
  vi.mocked(api.preview).mockResolvedValue({previewRef:paragraph,inputRevision:revision,inputHash:'hash',action:'draft',methodVersion:'content-workbench-writing.v1',
    configRef:domain,modelRef:domain,connectionVersionRef:domain,sourceCount:0,inputCharacterCount:20,estimatedInputTokens:20,reservedTokens:200,maxCalls:1,outboundInput:{},confirmationRequired:true});
  const queued:AiStatus={requestId:paragraph,workRef:work,invocationRef:null,state:'queued',inputRevision:revision,action:'draft',methodVersion:'content-workbench-writing.v1',
    suggestion:null,sourceQualification:'readable',failureCode:null,staleBase:false,usage:{state:'not_reserved',inputTokens:null,outputTokens:null,chargedTokens:null,costUsd:null}};
  vi.mocked(api.execute).mockResolvedValue(queued);
  vi.mocked(api.cancel).mockResolvedValue({...queued,state:'cancelled'});
  show();await screen.findByLabelText('正文第 1 段');
  fireEvent.click(screen.getByRole('button',{name:'按材料起草'}));
  const prepare=await screen.findByRole('button',{name:'准备并核对输入'});await waitFor(()=>expect(prepare).toBeEnabled());fireEvent.click(prepare);
  fireEvent.click(await screen.findByRole('checkbox',{name:'已核对不含真实姓名、学校、密码和点赞数，确认输入及额度'}));
  fireEvent.click(screen.getByRole('button',{name:'执行一次'}));
  expect(await screen.findByText('等待处理')).toBeVisible();fireEvent.click(screen.getByText('调用记录'));
  expect(screen.getByText('尚未派发，未建立调用账。')).toBeVisible();
  expect(screen.queryByText(/输入 0 \/ 输出 0/)).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('button',{name:'取消这次生成'}));
  expect(await screen.findByText('已取消')).toBeVisible();expect(api.cancel).toHaveBeenCalledExactlyOnceWith(paragraph);
  expect(api.execute).toHaveBeenCalledTimes(1);
});

it('retains right hand author edits when a lost AI adoption response is reconciled by the original receipt', async () => {
  vi.mocked(api.readiness).mockResolvedValue({configured:true,transportCallable:true,writingQualified:true,configRef:domain,methodVersion:'writing.v1'});
  vi.mocked(api.preview).mockResolvedValue({previewRef:paragraph,inputRevision:revision,inputHash:'hash',action:'draft',methodVersion:'writing.v1',configRef:domain,modelRef:domain,connectionVersionRef:domain,sourceCount:0,inputCharacterCount:20,estimatedInputTokens:20,reservedTokens:200,maxCalls:1,outboundInput:{},confirmationRequired:true});
  vi.mocked(api.execute).mockImplementation(async request=>({requestId:request.requestId,workRef:work,invocationRef:paragraph,state:'completed',inputRevision:revision,action:'draft',methodVersion:'writing.v1',suggestion:{title:null,paragraphs:[{paragraphId:paragraph,text:'模型建议保留来源'}],pages:[]},sourceQualification:'readable',failureCode:null,staleBase:false,usage:{state:'measured',inputTokens:20,outputTokens:20,chargedTokens:40,costUsd:null}}));
  vi.mocked(api.adopt).mockRejectedValueOnce(new ApiError({kind:'network'}));
  vi.mocked(api.lookupWorkRequest).mockImplementation(async requestId=>({requestId,workId:work,outcome:'accepted',replayed:true,revision:{...envelope().revision,id:crypto.randomUUID(),number:2,document:{...envelope().revision.document,paragraphs:[{id:paragraph,text:'模型建议保留来源'}]}}}));
  show();await screen.findByLabelText('正文第 1 段');fireEvent.click(screen.getByRole('button',{name:'按材料起草'}));
  const prepare=await screen.findByRole('button',{name:'准备并核对输入'});await waitFor(()=>expect(prepare).toBeEnabled());fireEvent.click(prepare);
  fireEvent.click(await screen.findByRole('checkbox',{name:'已核对不含真实姓名、学校、密码和点赞数，确认输入及额度'}));fireEvent.click(screen.getByRole('button',{name:'执行一次'}));
  fireEvent.change(await screen.findByLabelText('编辑改稿 第 1 段'),{target:{value:'作者在对照右侧写的新句子😀'}});
  fireEvent.click(screen.getByRole('button',{name:'采用这一段'}));
  fireEvent.click(await screen.findByRole('button',{name:'沿用原请求重试'}));
  expect(await screen.findByLabelText('正文第 1 段')).toHaveValue('作者在对照右侧写的新句子😀');
  expect(api.adopt).toHaveBeenCalledTimes(1);expect(api.lookupWorkRequest).toHaveBeenCalledTimes(1);
});

it('retains the second edited paragraph after adopting the first and returning to the manuscript', async()=>{
 const initial=envelope();const second=crypto.randomUUID();initial.revision.document.paragraphs=[{id:paragraph,text:'原稿一'},{id:second,text:'原稿二'}];
 vi.mocked(api.readWork).mockResolvedValue(initial);
 show();await screen.findByLabelText('正文第 1 段');fireEvent.click(screen.getByRole('button',{name:'改稿对照'}));
 fireEvent.change(screen.getByLabelText('编辑改稿 第 1 段'),{target:{value:'采用第一段😀'}});fireEvent.change(screen.getByLabelText('编辑改稿 第 2 段'),{target:{value:'第二段尚未采用的编辑'}});
 fireEvent.click(screen.getAllByRole('button',{name:'采用这一段'})[0]!);
 expect(screen.getByLabelText('编辑改稿 第 2 段')).toHaveValue('第二段尚未采用的编辑');
 expect(screen.getAllByRole('button',{name:'采用这一段'})[1]).toBeEnabled();
 fireEvent.click(screen.getByRole('button',{name:'返回正文'}));expect(screen.getByLabelText('正文第 1 段')).toHaveValue('采用第一段😀');expect(screen.getByLabelText('正文第 2 段')).toHaveValue('原稿二');
 fireEvent.click(screen.getByRole('button',{name:'改稿对照'}));expect(screen.getByLabelText('编辑改稿 第 2 段')).toHaveValue('第二段尚未采用的编辑');
 fireEvent.click(screen.getAllByRole('button',{name:'采用这一段'})[1]!);fireEvent.click(screen.getByRole('button',{name:'返回正文'}));expect(screen.getByLabelText('正文第 2 段')).toHaveValue('第二段尚未采用的编辑');
});

it('creates an inner page in the placement dialog and flushes its stable identity before one body-and-page placement',async()=>{
 const initial=envelope();const excerpt=crypto.randomUUID();initial.revision.document.excerpts=[{id:excerpt,citation:{asOf:'2026-10-23T08:00:00Z',blockRef:'block',sourceKind:'note',sourceRef:domain,workRef:domain,field:'body',sourceVersion:'source1',start:0,end:4,textHash:'hash',location:null},selectedText:'可用原文',textHash:'hash'}];initial.revision.document.canvas=[{id:paragraph,excerptId:excerpt,expression:'自己的表达',expressionRevision:1,group:'opening',note:''}];
 vi.mocked(api.readWork).mockResolvedValue(initial);const order:string[]=[];let stored=initial.revision.document;const savedId=crypto.randomUUID();
 vi.mocked(api.save).mockImplementation(async(_id,request)=>{order.push('save');stored=request.document;return {requestId:request.requestId,workId:work,outcome:'accepted',replayed:false,revision:{id:savedId,number:2,savedAt:'now',document:stored}};});
 vi.mocked(api.place).mockImplementation(async(_id,request)=>{order.push('place');return {requestId:request.requestId,workId:work,outcome:'accepted',replayed:false,revision:{id:crypto.randomUUID(),number:3,savedAt:'now',document:stored}};});
 show('canvas');fireEvent.click(await screen.findByRole('button',{name:'放入正文 / 指定图页 →'}));fireEvent.click(screen.getByRole('button',{name:'新建内页'}));
 const target=screen.getByLabelText('放入图页位置') as HTMLSelectElement;const pageId=target.value;expect(pageId).not.toBe('');expect(screen.getByRole('dialog',{name:'放入作品'})).toBeInTheDocument();
 fireEvent.click(screen.getByRole('button',{name:'确认放入'}));await waitFor(()=>expect(api.place).toHaveBeenCalledTimes(1));
 expect(order).toEqual(['save','place']);expect(stored.pages.find(page=>page.id===pageId)?.kind).toBe('opinion');expect(stored.excerpts[0]?.citation.sourceVersion).toBe('source1');
 const request=vi.mocked(api.place).mock.calls[0]![1];expect(request.expectedRevision).toBe(savedId);expect(request.targets).toHaveLength(2);expect(request.targets.some(target=>target.kind==='pageBlock'&&target.pageId===pageId)).toBe(true);expect(request.targets.some(target=>target.kind==='paragraph')).toBe(true);
});

it('retains both AI edits after partial adoption and keeps the unadopted draft available after returning and reopening',async()=>{
 const initial=envelope(),second=crypto.randomUUID();initial.revision.document.paragraphs=[{id:paragraph,text:'原一'},{id:second,text:'原二'}];vi.mocked(api.readWork).mockResolvedValue(initial);
 vi.mocked(api.readiness).mockResolvedValue({configured:true,transportCallable:true,writingQualified:true,configRef:domain,methodVersion:'writing.v1'});
 vi.mocked(api.preview).mockResolvedValue({previewRef:paragraph,inputRevision:revision,inputHash:'hash',action:'draft',methodVersion:'writing.v1',configRef:domain,modelRef:domain,connectionVersionRef:domain,sourceCount:0,inputCharacterCount:20,estimatedInputTokens:20,reservedTokens:200,maxCalls:1,outboundInput:{},confirmationRequired:true});
 vi.mocked(api.execute).mockImplementation(async request=>({requestId:request.requestId,workRef:work,invocationRef:paragraph,state:'completed',inputRevision:revision,action:'draft',methodVersion:'writing.v1',suggestion:{title:null,paragraphs:[{paragraphId:paragraph,text:'建议一'},{paragraphId:second,text:'建议二'}],pages:[]},sourceQualification:'readable',failureCode:null,staleBase:false,usage:{state:'measured',inputTokens:20,outputTokens:20,chargedTokens:40,costUsd:null}}));
 vi.mocked(api.adopt).mockImplementation(async(_id,request)=>({requestId:request.requestId,workId:work,outcome:'accepted',replayed:false,revision:{...initial.revision,id:crypto.randomUUID(),number:2,document:{...initial.revision.document,paragraphs:[{id:paragraph,text:'建议一'},{id:second,text:'原二'}]}}}));
 vi.mocked(api.aiStatus).mockImplementation(async requestId=>({requestId,workRef:work,invocationRef:paragraph,state:'completed',inputRevision:revision,action:'draft',methodVersion:'writing.v1',suggestion:{title:null,paragraphs:[{paragraphId:paragraph,text:'建议一'},{paragraphId:second,text:'建议二'}],pages:[]},sourceQualification:'readable',failureCode:null,staleBase:true,usage:{state:'measured',inputTokens:20,outputTokens:20,chargedTokens:40,costUsd:null}}));
 show();await screen.findByLabelText('正文第 1 段');fireEvent.click(screen.getByRole('button',{name:'按材料起草'}));const prepare=await screen.findByRole('button',{name:'准备并核对输入'});await waitFor(()=>expect(prepare).toBeEnabled());fireEvent.click(prepare);fireEvent.click(await screen.findByRole('checkbox',{name:'已核对不含真实姓名、学校、密码和点赞数，确认输入及额度'}));fireEvent.click(screen.getByRole('button',{name:'执行一次'}));
 fireEvent.change(await screen.findByLabelText('编辑改稿 第 1 段'),{target:{value:'作者采用第一段'}});fireEvent.change(screen.getByLabelText('编辑改稿 第 2 段'),{target:{value:'作者未采用第二段😀'}});fireEvent.click(screen.getAllByRole('button',{name:'采用这一段'})[0]!);
 await screen.findByRole('button',{name:'复制未采用改稿'});expect(screen.getByLabelText('编辑改稿 第 2 段')).toHaveValue('作者未采用第二段😀');expect(screen.getAllByRole('button',{name:'采用这一段'})[1]).toBeDisabled();
 fireEvent.click(screen.getByRole('button',{name:'复制未采用改稿'}));expect(await screen.findByLabelText('未采用改稿，可手动复制')).toHaveValue('作者未采用第二段😀');
 fireEvent.click(screen.getByRole('button',{name:'返回正文'}));expect(screen.getByLabelText('正文第 1 段')).toHaveValue('作者采用第一段');fireEvent.click(screen.getByRole('button',{name:'按材料起草'}));fireEvent.click(await screen.findByRole('button',{name:'打开改稿对照'}));expect(screen.getByLabelText('编辑改稿 第 2 段')).toHaveValue('作者未采用第二段😀');expect(api.execute).toHaveBeenCalledTimes(1);expect(api.adopt).toHaveBeenCalledTimes(1);
});

it.each([
  { writingQualified: true, transportCallable: true, writing: '创作输出格式已验证', connection: '模型可连接' },
  { writingQualified: false, transportCallable: true, writing: '创作输出格式未通过验证', connection: '模型可连接' },
  { writingQualified: null, transportCallable: true, writing: '创作输出格式尚未验证', connection: '模型可连接' },
  { writingQualified: null, transportCallable: false, writing: '创作输出格式尚未验证', connection: '模型连接测试尚未通过' },
  { writingQualified: null, transportCallable: null, writing: '创作输出格式尚未验证', connection: '连接情况尚未确认' },
])('distinguishes resolved writing and transport qualification: $connection / $writing', async ({ writingQualified, transportCallable, writing, connection }) => {
  vi.mocked(api.readiness).mockResolvedValue({ configured: true, transportCallable, writingQualified, configRef: domain, methodVersion: 'content-workbench-writing.v1' });
  show(); await screen.findByLabelText('正文第 1 段'); fireEvent.click(screen.getByRole('button', { name: '按材料起草' }));
  await waitFor(() => expect(screen.getByRole('button', { name: '准备并核对输入' })).toBeEnabled());
  expect(screen.getByText(`${connection} · ${writing}`)).toBeInTheDocument();
  expect(screen.queryByText(/创作能力已验证/)).not.toBeInTheDocument(); expect(api.execute).not.toHaveBeenCalled(); expect(api.preview).not.toHaveBeenCalled();
});

it('clears cached AI configuration and ignores an older dialog read after reopening', async () => {
  type Readiness = Awaited<ReturnType<typeof api.readiness>>;
  let resolveOlder!: (value: Readiness) => void;
  let resolveLatest!: (value: Readiness) => void;
  const ready: Readiness = { configured: true, transportCallable: true, writingQualified: true, configRef: domain, methodVersion: 'content-workbench-writing.v1' };
  vi.mocked(api.readiness).mockResolvedValueOnce(ready)
    .mockImplementationOnce(() => new Promise(resolve => { resolveOlder = resolve; }))
    .mockImplementationOnce(() => new Promise(resolve => { resolveLatest = resolve; }));
  vi.mocked(api.preview).mockRejectedValue(new Error('SYNTHETIC stop after explicit preview'));
  show(); await screen.findByLabelText('正文第 1 段');
  fireEvent.click(screen.getByRole('button', { name: '按材料起草' }));
  await waitFor(() => expect(screen.getByRole('button', { name: '准备并核对输入' })).toBeEnabled());
  fireEvent.click(within(screen.getByRole('dialog')).getByRole('button', { name: '关闭' }));
  fireEvent.click(screen.getByRole('button', { name: '按材料起草' }));
  expect(screen.getByText(/正在读取模型配置/)).toBeInTheDocument();
  expect(screen.queryByText(/创作输出格式/)).not.toBeInTheDocument();
  expect(screen.getByRole('button', { name: '准备并核对输入' })).toBeDisabled();
  fireEvent.click(within(screen.getByRole('dialog')).getByRole('button', { name: '关闭' }));
  fireEvent.click(screen.getByRole('button', { name: '按材料起草' }));
  const latestConfig = crypto.randomUUID();
  await act(async () => { resolveLatest({ ...ready, configRef: latestConfig }); });
  await waitFor(() => expect(screen.getByRole('button', { name: '准备并核对输入' })).toBeEnabled());
  await act(async () => { resolveOlder({ configured: false, transportCallable: false, writingQualified: false, configRef: null, methodVersion: ready.methodVersion }); });
  await waitFor(() => expect(screen.queryByText(/尚未配置创作模型/)).not.toBeInTheDocument());
  fireEvent.click(screen.getByRole('button', { name: '准备并核对输入' }));
  await waitFor(() => expect(api.preview).toHaveBeenCalledTimes(1));
  expect(vi.mocked(api.preview).mock.calls[0]?.[0].configRef).toBe(latestConfig);
  expect(api.readiness).toHaveBeenCalledTimes(3); expect(api.execute).not.toHaveBeenCalled();
});

it('ignores a closed dialog configuration failure after a newer read has succeeded', async () => {
  type Readiness = Awaited<ReturnType<typeof api.readiness>>;
  let rejectOlder!: (reason: Error) => void;
  vi.mocked(api.readiness).mockImplementationOnce(() => new Promise<Readiness>((_resolve, reject) => { rejectOlder = reject; }))
    .mockResolvedValueOnce({ configured: true, transportCallable: true, writingQualified: true, configRef: domain, methodVersion: 'content-workbench-writing.v1' });
  show(); await screen.findByLabelText('正文第 1 段');
  fireEvent.click(screen.getByRole('button', { name: '按材料起草' }));
  fireEvent.click(within(screen.getByRole('dialog')).getByRole('button', { name: '关闭' }));
  fireEvent.click(screen.getByRole('button', { name: '按材料起草' }));
  await waitFor(() => expect(screen.getByRole('button', { name: '准备并核对输入' })).toBeEnabled());
  await act(async () => { rejectOlder(new Error('SYNTHETIC old configuration failure')); });
  expect(screen.queryByText('结果未确认')).not.toBeInTheDocument();
  expect(screen.queryByText(/模型配置读取失败/)).not.toBeInTheDocument();
  expect(screen.getByText('模型可连接 · 创作输出格式已验证')).toBeInTheDocument();
  expect(api.preview).not.toHaveBeenCalled(); expect(api.execute).not.toHaveBeenCalled();
});

it('reports a current configuration read failure without asserting output qualification', async () => {
  vi.mocked(api.readiness).mockRejectedValueOnce(new Error('SYNTHETIC current configuration failure'));
  show(); await screen.findByLabelText('正文第 1 段'); fireEvent.click(screen.getByRole('button', { name: '按材料起草' }));
  expect(await screen.findByText('模型配置读取失败，请关闭后重新打开。')).toBeInTheDocument();
  expect(screen.queryByText(/正在读取模型配置/)).not.toBeInTheDocument();
  expect(screen.queryByText(/创作输出格式/)).not.toBeInTheDocument();
  expect(screen.getByRole('button', { name: '准备并核对输入' })).toBeDisabled();
  expect(api.preview).not.toHaveBeenCalled(); expect(api.execute).not.toHaveBeenCalled();
});


describe('retained recovery copies preserve manuscript tags', () => {
  it.each([false, true])('restores retained tags through a subsequent save (unsaved tag draft: %s)', async (unsaved) => {
    vi.mocked(api.save).mockReset();
    const initial = envelope();
    initial.revision.document.tags = ['本页标签'];
    const historical = { ...initial.revision, id: crypto.randomUUID(), number: 0, document: { ...initial.revision.document, tags: ['历史标签'], paragraphs: [{ id: paragraph, text: '历史正文' }] } };
    const restored = { ...historical, id: crypto.randomUUID(), number: 2 };
    vi.mocked(api.readWork).mockResolvedValue(initial);
    vi.mocked(api.history).mockResolvedValue([historical].map(r => ({ id: r.id, number: r.number, savedAt: r.savedAt, referenceFormat: 'current' as const })));
    vi.mocked(api.restore).mockImplementation(async (_id, request) => ({ requestId: request.requestId, workId: work, outcome: 'accepted', replayed: false, revision: restored }));
    vi.mocked(api.save).mockImplementation(async (_id, request) => ({ requestId: request.requestId, workId: work, outcome: 'accepted', replayed: false, revision: { ...restored, id: crypto.randomUUID(), number: 3, document: request.document } }));
    const view = show();
    await screen.findByLabelText('确认发布标签');
    // Keep this recovery sequence before the 900ms autosave deadline, regardless of CPU load.
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
    try {
      if (unsaved) fireEvent.change(screen.getByLabelText('确认发布标签'), { target: { value: '未保存标签 共同标签 未保存标签' } });
      await act(async () => { fireEvent.click(screen.getByRole('button', { name: '历史' })); });
      await act(async () => { fireEvent.click(screen.getByRole('button', { name: '恢复为新版本' })); });
      expect(screen.getByLabelText('确认发布标签')).toHaveValue('历史标签');
      await act(async () => { fireEvent.click(screen.getByRole('button', { name: '历史' })); });
      fireEvent.click(screen.getByRole('button', { name: '恢复到编辑区' }));
      const expected = unsaved ? ['未保存标签', '共同标签'] : ['本页标签'];
      expect(api.save).not.toHaveBeenCalled();
      await act(async () => { fireEvent.click(screen.getByRole('button', { name: '保存' })); });
      expect(api.save).toHaveBeenCalledTimes(1);
      expect(vi.mocked(api.save).mock.calls[0]?.[1].document.tags).toEqual(expected);
      expect(screen.getByLabelText('确认发布标签')).toHaveValue(expected.join(' '));
      await act(async () => { await vi.advanceTimersByTimeAsync(900); });
      expect(api.save).toHaveBeenCalledTimes(1);
    } finally {
      view.unmount();
      vi.useRealTimers();
    }
  });
});

it('uses the standard shell and Z02 creation tabs, keeps empty scope explicit, and never loads works for all', async () => {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(<QueryClientProvider client={client}><TestRouter initialEntries={['/ops/creation?domain=all']}><WorkbenchPage/></TestRouter></QueryClientProvider>);
  await screen.findByText('当前领域不可用于创作');
  expect(screen.getByLabelText('创作领域').closest('.v7-context-crumb')).not.toBeNull();
  expect(screen.getByRole('navigation', { name: '创作页面' }).closest('.v7-context-main')).not.toBeNull();
  expect(screen.getByRole('navigation', { name: '一级导航' })).toBeInTheDocument();
  expect(api.listWorks).not.toHaveBeenCalled();
  fireEvent.click(screen.getByLabelText('创作领域'));
  fireEvent.click(screen.getByRole('button', { name: '合成领域' }));
  await screen.findByLabelText('新选题标题');
  expect(api.listWorks).toHaveBeenCalledWith(domain, expect.any(AbortSignal));
});

it('keeps the current editor and unsaved text mounted while entering and leaving immersion', async () => {
  show();
  const body = await screen.findByLabelText('正文第 1 段');
  fireEvent.change(body, { target: { value: '切换沉浸仍保留的未保存正文' } });
  fireEvent.click(screen.getByRole('button', { name: '收起导航' }));
  expect(screen.getByLabelText('正文第 1 段')).toBe(body);
  expect(body).toHaveValue('切换沉浸仍保留的未保存正文');
  fireEvent.click(screen.getByRole('button', { name: '退出沉浸' }));
  expect(screen.getByLabelText('正文第 1 段')).toBe(body);
  expect(body).toHaveValue('切换沉浸仍保留的未保存正文');
});


it('keeps every editor step reachable from the writing workflow menu', async () => {
  vi.mocked(api.prepareMaterials).mockResolvedValue({ contract: 'content-workbench-materials.v1', asOf: 'now', methodVersion: 'test', query: '', terms: [], state: 'insufficient', gaps: [], documents: [], nextCursor: null, searchScope: 'test' });
  show(); await screen.findByLabelText('正文第 1 段');
  fireEvent.click(screen.getByText(/^创作步骤/, { selector: 'summary' }));
  const nav = screen.getByRole('navigation', { name: '创作步骤' });
  for (const name of ['选题方案','材料浏览','创作画布','正文','封面','内页','预览与导出']) {
    expect(within(nav).getByRole('button', { name })).toBeInTheDocument();
  }
  fireEvent.click(within(nav).getByRole('button', { name: '材料浏览' }));
  await screen.findByText('创作步骤 · 材料浏览', { selector: 'summary' });
  fireEvent.click(screen.getByText(/^创作步骤/, { selector: 'summary' }));
  const materialNav = await screen.findByRole('navigation', { name: '创作步骤' });
  await waitFor(() => expect(within(materialNav).getByRole('button', { name: '材料浏览' })).toHaveAttribute('aria-current', 'step'));
  fireEvent.click(within(materialNav).getByRole('button', { name: '正文' }));
  await screen.findByLabelText('正文第 1 段');
});

it('keeps unfinished candidate and idea inputs across editor steps and warns before leaving', async () => {
  show(); await screen.findByLabelText('正文第 1 段');
  fireEvent.click(screen.getByText('添加笔记标题候选'));
  fireEvent.change(screen.getByLabelText('新标题候选'), { target: { value: '尚未添加的候选' } });
  fireEvent.click(screen.getByText('补一个选题框'));
  fireEvent.change(screen.getByLabelText('新选题框名称'), { target: { value: '尚未建立的选题' } });
  const leaving = new Event('beforeunload', { cancelable: true });
  window.dispatchEvent(leaving);
  expect(leaving.defaultPrevented).toBe(true);
  fireEvent.click(screen.getByText(/^创作步骤/, { selector: 'summary' }));
  fireEvent.click(within(screen.getByRole('navigation', { name: '创作步骤' })).getByRole('button', { name: '选题方案' }));
  fireEvent.click(screen.getByText(/^创作步骤/, { selector: 'summary' }));
  fireEvent.click(within(await screen.findByRole('navigation', { name: '创作步骤' })).getByRole('button', { name: '正文' }));
  fireEvent.click(await screen.findByText('添加笔记标题候选'));
  expect(screen.getByLabelText('新标题候选')).toHaveValue('尚未添加的候选');
  fireEvent.click(screen.getByText('补一个选题框'));
  expect(screen.getByLabelText('新选题框名称')).toHaveValue('尚未建立的选题');
});

it('opens the asset library from its own tab for the chosen domain and leaves it again', async () => {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  render(<QueryClientProvider client={client}>
    <TestRouter initialEntries={[`/ops/creation?domain=${domain}`]}><WorkbenchPage /></TestRouter>
  </QueryClientProvider>);
  const tab = await screen.findByRole('button', { name: '资产库' });
  expect(tab).not.toHaveAttribute('aria-current');
  fireEvent.click(tab);
  expect(await screen.findByText(`资产库页 ${domain}`)).toBeInTheDocument();
  expect(screen.getByRole('button', { name: '资产库' })).toHaveAttribute('aria-current', 'page');
  fireEvent.click(screen.getByRole('button', { name: '写稿' }));
  await waitFor(() => expect(screen.queryByText(`资产库页 ${domain}`)).not.toBeInTheDocument());
  expect(screen.getByRole('button', { name: '写稿' })).toHaveAttribute('aria-current', 'page');
});

it('lets the copy wait for a save already in flight, then saves what was typed meanwhile', async () => {
  show();
  const body = await screen.findByLabelText('正文第 1 段');
  const receipt = (r: Parameters<typeof api.save>[1], number: number): SaveReceipt => ({
    requestId: r.requestId, workId: work, outcome: 'accepted', replayed: false,
    revision: { id: crypto.randomUUID(), number, savedAt: 'now', document: r.document }
  });
  let finishFirst!: () => void;
  vi.mocked(api.save)
    .mockImplementationOnce((_id, r) => new Promise<SaveReceipt>(resolve => { finishFirst = () => resolve(receipt(r, 2)); }))
    .mockImplementationOnce(async (_id, r) => receipt(r, 3));
  fireEvent.change(body, { target: { value: '第一次输入' } });
  fireEvent.click(screen.getByRole('button', { name: '保存' }));
  await waitFor(() => expect(api.save).toHaveBeenCalledTimes(1));
  fireEvent.change(screen.getByLabelText('正文第 1 段'), { target: { value: '第二次输入' } });
  const saving = checkPanelProps!.saveCurrent();
  await new Promise(resolve => setTimeout(resolve, 150));
  expect(api.save).toHaveBeenCalledTimes(1);
  await act(async () => { finishFirst(); });
  expect((await saving).number).toBe(3);
  expect(api.save).toHaveBeenCalledTimes(2);
  expect(vi.mocked(api.save).mock.calls[1]?.[1].document.paragraphs[0]?.text).toBe('第二次输入');
});

it('keeps the unconfirmed copy receipt while moving to the cover and back', async () => {
  show();
  await screen.findByLabelText('正文第 1 段');
  fireEvent.click(screen.getByRole('button', { name: '测试待确认回执' }));
  expect(screen.getByText('待确认回执')).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '制作封面 →' }));
  expect(screen.queryByText('待确认回执')).not.toBeInTheDocument();
  fireEvent.click(screen.getByText(/^创作步骤/, { selector: 'summary' }));
  fireEvent.click(screen.getByRole('button', { name: '正文' }));
  expect(await screen.findByText('待确认回执')).toBeInTheDocument();
});

it('protects an uncertain template save from navigation when multiple draft editors are mounted', async () => {
  const other = '00000000-0000-4000-8000-000000000008';
  vi.mocked(api.readWork).mockImplementation(async id => id === other ? { ...envelope(), workId: other } : envelope());
  const templateApi = await import('./assets/structuralTemplatesApi');
  const create = vi.spyOn(templateApi, 'createTemplate').mockRejectedValue(new Error('offline'));
  function Navigation() {
    const navigate = useNavigate();
    return <><button onClick={() => navigate(`/ops/creation?domain=${domain}&work=${other}&view=body`)}>打开 B</button><button onClick={() => navigate(`/ops/creation?domain=${domain}&work=${work}&view=body`)}>打开 A</button><button onClick={() => navigate('/elsewhere')}>离开创作</button></>;
  }
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(<QueryClientProvider client={client}><TestRouter initialEntries={[`/ops/creation?domain=${domain}&work=${work}&view=body`]}><Navigation/><WorkbenchPage/></TestRouter></QueryClientProvider>);
  await screen.findByLabelText('正文第 1 段'); fireEvent.click(screen.getByRole('button', { name: '打开 B' }));
  await waitFor(() => expect(api.readWork).toHaveBeenCalledWith(other, expect.anything()));
  fireEvent.click(screen.getByRole('button', { name: '打开 A' }));
  fireEvent.click(await screen.findByRole('button', { name: '存为样板' }));
  fireEvent.click(screen.getByRole('button', { name: '＋ 开头' }));
  fireEvent.click(screen.getByRole('button', { name: '保存待审核' }));
  await screen.findByText('保存结果尚未确认，请沿用原请求重试。');
  fireEvent.click(screen.getByRole('button', { name: '离开创作', hidden: true }));
  expect(await screen.findByText(/样板保存结果未确认，请先留在此稿/)).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '返回稿件' }));
  fireEvent.click(screen.getByRole('button', { name: '重试保存' }));
  await waitFor(() => expect(create).toHaveBeenCalledTimes(2));
  expect(create.mock.calls[0]).toEqual(create.mock.calls[1]);
  create.mockRestore();
});

it('protects an uncertain publication link from leaving the workbench', async () => {
  function Navigation() { const navigate = useNavigate(); return <button onClick={() => navigate('/elsewhere')}>离开创作</button>; }
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(<QueryClientProvider client={client}><TestRouter initialEntries={[`/ops/creation?domain=${domain}&work=${work}&view=body`]}><Navigation/><WorkbenchPage/></TestRouter></QueryClientProvider>);
  fireEvent.click(await screen.findByRole('button', { name: '测试待确认发布关联' }));
  fireEvent.click(screen.getByRole('button', { name: '离开创作' }));
  expect(await screen.findByText(/操作结果尚未确认/)).toBeInTheDocument();
  expect(screen.getByTestId('pending-publication')).toHaveTextContent('synthetic-request');
});

it('keeps the original template retry available while withdrawal hides the editor', async () => {
  const templateApi = await import('./assets/structuralTemplatesApi');
  const create = vi.spyOn(templateApi, 'createTemplate').mockRejectedValue(new Error('offline'));
  show(); await screen.findByLabelText('正文第 1 段');
  fireEvent.click(screen.getByRole('button', { name: '存为样板' }));
  fireEvent.click(screen.getByRole('button', { name: '＋ 开头' }));
  fireEvent.click(screen.getByRole('button', { name: '保存待审核' }));
  await screen.findByText('保存结果尚未确认，请沿用原请求重试。');
  fireEvent.click(screen.getByRole('button', { name: '关闭' }));
  vi.mocked(api.readWork).mockResolvedValue({ ...envelope(), experienceResetRequired: true });
  fireEvent.focus(window);
  await screen.findByRole('button', { name: '撤回后重新开始编辑' });
  expect(screen.queryByRole('textbox', { name: '正文第 1 段' })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '存为样板' }));
  fireEvent.click(screen.getByRole('button', { name: '重试保存' }));
  await waitFor(() => expect(create).toHaveBeenCalledTimes(2));
  expect(create.mock.calls[0]).toEqual(create.mock.calls[1]);
  create.mockRestore();
});

it('covers stale AI-derived text when its source becomes unavailable and allows a clean reread', async () => {
  const initial = envelope();
  initial.revision.document.derivedSources = [{ target: { kind: 'paragraph', paragraphId: paragraph }, citation: {
    asOf: '2026-10-29', blockRef: 'body', sourceKind: 'comment', sourceRef: work, workRef: work,
    field: 'comment_body', sourceVersion: 'v1', start: 0, end: 1, textHash: 'a'.repeat(64), location: null,
  } }];
  vi.mocked(api.readWork).mockResolvedValue(initial);
  show(); await screen.findByLabelText('正文第 1 段');
  const unavailable = structuredClone(initial);
  unavailable.revision.document.paragraphs[0]!.text = '[来源当前不可使用]';
  vi.mocked(api.readWork).mockResolvedValue(unavailable);
  fireEvent.focus(window);
  expect(await screen.findByText(/参考来源的状态已变化/)).toBeVisible();
  expect(screen.queryByRole('textbox', { name: '正文第 1 段' })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '丢弃本页编辑并重新读取' }));
  const resumed = await screen.findByRole('textbox',{name:'正文第 1 段'}); expect(resumed).toBeVisible(); expect(resumed).toHaveValue('[来源当前不可使用]');
});

it('rechecks newly collected unsaved source text before revealing the editor again', async () => {
  vi.mocked(api.qualifyCitation).mockResolvedValue(false);
  show('materials');
  const collect = await screen.findByRole('button', {name:'测试收集未保存来源'});
  fireEvent.click(collect);
  fireEvent.focus(window);
  expect(await screen.findByText(/参考来源的状态已变化/)).toBeVisible();
  expect(api.qualifyCitation).toHaveBeenCalledWith(domain, expect.objectContaining({sourceKind:'comment',textHash:'a'.repeat(64)}));
  expect(screen.queryByRole('button', {name:'测试收集未保存来源'})).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', {name:'丢弃本页编辑并重新读取'}));
  expect(await screen.findByRole('button', {name:'测试收集未保存来源'})).toBeVisible();
});

it('hides and clears a stale version conflict when its source becomes unavailable', async () => {
  const initial = envelope();
  initial.revision.document.derivedSources = [{target:{kind:'paragraph',paragraphId:paragraph},citation:{asOf:'2026-10-29',blockRef:'body',sourceKind:'comment',sourceRef:work,workRef:work,field:'comment_body',sourceVersion:'v1',start:0,end:1,textHash:'a'.repeat(64),location:null}}];
  const conflict: SaveReceipt = {requestId:'',workId:work,outcome:'stale',replayed:false,revision:{...initial.revision,id:crypto.randomUUID(),number:2}};
  vi.mocked(api.readWork).mockResolvedValue(initial);
  vi.mocked(api.save).mockImplementationOnce(async (_id, request) => { conflict.requestId=request.requestId; throw new ApiError({kind:'http',status:409,code:'creation_stale'}); });
  vi.mocked(api.conflictReceipt).mockReturnValue(conflict);
  show(); const body=await screen.findByLabelText('正文第 1 段');
  fireEvent.change(body,{target:{value:'本页含来源的编辑'}}); fireEvent.click(screen.getByRole('button',{name:'保存'}));
  expect(await screen.findByRole('dialog',{name:'服务器已有新版本'})).toBeVisible();
  const restricted=structuredClone(initial);restricted.revision.document.paragraphs[0]!.text='[来源当前不可使用]';
  vi.mocked(api.readWork).mockResolvedValue(restricted); fireEvent.focus(window);
  expect(await screen.findByText(/参考来源的状态已变化/)).toBeVisible();
  expect(screen.queryByRole('dialog',{name:'服务器已有新版本'})).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('button',{name:'丢弃本页编辑并重新读取'}));
  expect(await screen.findByLabelText('正文第 1 段')).toHaveValue('[来源当前不可使用]');
  expect(screen.queryByRole('dialog',{name:'服务器已有新版本'})).not.toBeInTheDocument();
});

it('resumes editing when withdrawn experience and invalidated reference are detected together', async () => {
  const initial=envelope();
  initial.revision.document.derivedSources=[{target:{kind:'paragraph',paragraphId:paragraph},citation:{asOf:'2026-10-29',blockRef:'body',sourceKind:'comment',sourceRef:work,workRef:work,field:'comment_body',sourceVersion:'v1',start:0,end:1,textHash:'a'.repeat(64),location:null}}];
  vi.mocked(api.readWork).mockResolvedValue(initial);
  show(); await screen.findByLabelText('正文第 1 段');
  const restricted=structuredClone(initial);restricted.experienceResetRequired=true;restricted.revision.document.paragraphs[0]!.text='[来源当前不可使用]';
  vi.mocked(api.readWork).mockResolvedValue(restricted);
  vi.mocked(api.resetWithdrawn).mockImplementation(async (_id, command) => ({requestId:command.requestId,workId:work,outcome:'accepted',replayed:false,revision:{...initial.revision,id:crypto.randomUUID(),number:2,document:{...restricted.revision.document,experienceEpoch:crypto.randomUUID()}}}));
  fireEvent.focus(window);
  fireEvent.click(await screen.findByRole('button',{name:'撤回后重新开始编辑'}));
  fireEvent.click(screen.getByRole('button',{name:'丢弃本页编辑并继续'}));
  const resumed = await screen.findByRole('textbox',{name:'正文第 1 段'}); expect(resumed).toBeVisible(); expect(resumed).toHaveValue('[来源当前不可使用]');
});

it('generates a source-checked title batch and adopts a selected cover via a saved receipt', async () => {
  const initial=envelope();
  vi.mocked(api.readWork).mockResolvedValue(initial);
  vi.mocked(api.titleReadiness).mockResolvedValue({configured:true,transportCallable:true,writingQualified:null,configRef:domain,methodVersion:'creation-title.v1',deniedReasons:[]});
  vi.mocked(api.titlePreview).mockResolvedValue({previewRef:paragraph,inputRevision:revision,inputHash:'a'.repeat(64),action:'draft',methodVersion:'creation-title.v1',configRef:domain,modelRef:domain,connectionVersionRef:domain,sourceCount:0,inputCharacterCount:10,estimatedInputTokens:10,reservedTokens:100,maxCalls:1,outboundInput:{paragraphs:[{text:'核对的合成正文'}]},outboundSystem:'标题生成指令',confirmationRequired:true});
  vi.mocked(api.titleExecute).mockImplementation(async command=>({requestId:command.requestId,workRef:work,invocationRef:domain,state:'succeeded',inputRevision:revision,action:'title_ideas',methodVersion:'creation-title.v1',suggestion:{coverTitles:['从第一步开始'],noteTitles:['孩子启动困难时先做这一步']},titleWarnings:{cover:[false],note:[false]},sourceQualification:'readable',failureCode:null,staleBase:false,usage:{state:'measured',inputTokens:10,outputTokens:10,chargedTokens:20,costUsd:null}}));
  vi.mocked(api.titleStatus).mockImplementation(async id=>({...await vi.mocked(api.titleExecute).mock.results[0]!.value,requestId:id}));
  vi.mocked(api.adoptTitle).mockImplementation(async (_id,command)=>({requestId:command.requestId,workId:work,outcome:'accepted',replayed:false,revision:{...initial.revision,id:crypto.randomUUID(),number:2,document:{...initial.revision.document,pages:[{id:crypto.randomUUID(),kind:'cover',templateVersion:'creation-v3-type',title:'从第一步开始',subtitle:'',blocks:[],assetId:null,crop:{x:0,y:0,width:1,height:1},style:{palette:'cream',fontFamily:'system-cjk',width:1080,height:1440,alignment:'start'}}]}}}));
  show();await screen.findByLabelText('正文第 1 段');fireEvent.click(screen.getByRole('button',{name:'换一批'}));
  expect(await screen.findByRole('dialog',{name:'AI 会看到的内容'})).toHaveTextContent('核对的合成正文'); expect(api.titleExecute).not.toHaveBeenCalled(); fireEvent.click(screen.getByLabelText('我已核对，发送内容不含真实姓名、学校、密码和点赞数')); fireEvent.click(screen.getByRole('button',{name:'确认并生成'}));
  fireEvent.click(await screen.findByRole('button',{name:'从第一步开始'}));
  expect(await screen.findByLabelText('封面大字')).toHaveValue('从第一步开始');
  expect(screen.getByRole('button',{name:'孩子启动困难时先做这一步'})).toBeEnabled();
  expect(api.titlePreview).toHaveBeenCalledTimes(1);expect(api.titleExecute).toHaveBeenCalledTimes(1);expect(api.adoptTitle).toHaveBeenCalledTimes(1);
});

it('keeps the first title request identity after a 503 and explicitly retries it', async () => {
  vi.mocked(api.titleReadiness).mockResolvedValue({configured:true,transportCallable:true,writingQualified:null,configRef:domain,methodVersion:'creation-title.v1',deniedReasons:[]});
  vi.mocked(api.titlePreview).mockResolvedValue({previewRef:paragraph,inputRevision:revision,inputHash:'a'.repeat(64),action:'draft',methodVersion:'creation-title.v1',configRef:domain,modelRef:domain,connectionVersionRef:domain,sourceCount:0,inputCharacterCount:10,estimatedInputTokens:10,reservedTokens:100,maxCalls:1,outboundInput:{paragraphs:[{text:'核对的合成正文'}]},outboundSystem:'标题生成指令',confirmationRequired:true});
  vi.mocked(api.titleExecute).mockRejectedValueOnce(new ApiError({kind:'http',status:503,code:'creation_ai_unavailable'})).mockImplementationOnce(async command=>({requestId:command.requestId,workRef:work,invocationRef:domain,state:'succeeded',inputRevision:revision,action:'title_ideas',methodVersion:'creation-title.v1',suggestion:{coverTitles:['从第一步开始'],noteTitles:['先做一件小事']},titleWarnings:{cover:[false],note:[false]},sourceQualification:'readable',failureCode:null,staleBase:false,usage:{state:'measured',inputTokens:10,outputTokens:10,chargedTokens:20,costUsd:null}}));
  show();await screen.findByLabelText('正文第 1 段');fireEvent.click(screen.getByRole('button',{name:'换一批'}));
  await screen.findByRole('dialog',{name:'AI 会看到的内容'}); fireEvent.click(screen.getByLabelText('我已核对，发送内容不含真实姓名、学校、密码和点赞数')); fireEvent.click(screen.getByRole('button',{name:'确认并生成'}));
  fireEvent.click(await screen.findByRole('button',{name:'沿用原请求重试'}));
  await screen.findByRole('button',{name:'从第一步开始'});
  expect(vi.mocked(api.titleExecute).mock.calls[0]![0]).toEqual(vi.mocked(api.titleExecute).mock.calls[1]![0]);
  expect(api.titlePreview).toHaveBeenCalledTimes(1);
});

it('blocks leaving during a slow title preview before any model request', async () => {
  function Navigation() { const navigate=useNavigate(); return <button onClick={()=>navigate('/elsewhere')}>离开创作</button>; }
  let resolve!: (value: Awaited<ReturnType<typeof api.titlePreview>>) => void;
  vi.mocked(api.titleReadiness).mockResolvedValue({configured:true,transportCallable:true,writingQualified:null,configRef:domain,methodVersion:'creation-title.v1',deniedReasons:[]});
  vi.mocked(api.titlePreview).mockImplementationOnce(()=>new Promise(done=>{resolve=done;}));
  const client=new QueryClient({defaultOptions:{queries:{retry:false}}});
  render(<QueryClientProvider client={client}><TestRouter initialEntries={[`/ops/creation?domain=${domain}&work=${work}&view=body`]}><Navigation/><WorkbenchPage/></TestRouter></QueryClientProvider>);
  await screen.findByLabelText('正文第 1 段');fireEvent.click(screen.getByRole('button',{name:'换一批'}));
  await waitFor(()=>expect(api.titlePreview).toHaveBeenCalledTimes(1));fireEvent.click(screen.getByRole('button',{name:'离开创作'}));
  expect(await screen.findByText(/标题请求尚未完成/)).toBeVisible();expect(api.titleExecute).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button',{name:'返回标题'}));
  await act(async()=>resolve({previewRef:paragraph,inputRevision:revision,inputHash:'a'.repeat(64),action:'draft',methodVersion:'creation-title.v1',configRef:domain,modelRef:domain,connectionVersionRef:domain,sourceCount:0,inputCharacterCount:10,estimatedInputTokens:10,reservedTokens:100,maxCalls:1,outboundInput:{},confirmationRequired:true}));
});

it('discards a title preview that returns after its source was withdrawn and reloaded', async () => {
  const initial=envelope();
  initial.revision.document.derivedSources=[{target:{kind:'paragraph',paragraphId:paragraph},citation:{asOf:'2026-10-29',blockRef:'body',sourceKind:'comment',sourceRef:work,workRef:work,field:'comment_body',sourceVersion:'v1',start:0,end:1,textHash:'a'.repeat(64),location:null}}];
  vi.mocked(api.readWork).mockResolvedValue(initial);
  vi.mocked(api.titleReadiness).mockResolvedValue({configured:true,transportCallable:true,writingQualified:null,configRef:domain,methodVersion:'creation-title.v1',deniedReasons:[]});
  let resolve!: (value: Awaited<ReturnType<typeof api.titlePreview>>) => void;
  vi.mocked(api.titlePreview).mockImplementationOnce(()=>new Promise(done=>{resolve=done;}));
  show();await screen.findByLabelText('正文第 1 段');fireEvent.click(screen.getByRole('button',{name:'换一批'}));
  await waitFor(()=>expect(api.titlePreview).toHaveBeenCalledTimes(1));
  const restricted=structuredClone(initial);restricted.revision.document.paragraphs[0]!.text='[来源当前不可使用]';
  vi.mocked(api.readWork).mockResolvedValue(restricted);fireEvent.focus(window);
  await screen.findByText(/参考来源的状态已变化/);fireEvent.click(screen.getByRole('button',{name:'丢弃本页编辑并重新读取'}));
  await screen.findByLabelText('正文第 1 段');
  await act(async()=>resolve({previewRef:paragraph,inputRevision:revision,inputHash:'a'.repeat(64),action:'draft',methodVersion:'creation-title.v1',configRef:domain,modelRef:domain,connectionVersionRef:domain,sourceCount:1,inputCharacterCount:10,estimatedInputTokens:10,reservedTokens:100,maxCalls:1,outboundInput:{text:'已撤回的合成文字'},outboundSystem:'标题生成指令',confirmationRequired:true}));
  expect(screen.queryByRole('dialog',{name:'AI 会看到的内容'})).not.toBeInTheDocument();expect(screen.queryByText(/已撤回的合成文字/)).not.toBeInTheDocument();expect(api.titleExecute).not.toHaveBeenCalled();
});

it('protects unsaved assets when a source link changes the route', async () => {
  const client=new QueryClient({defaultOptions:{queries:{retry:false}}});
  render(<QueryClientProvider client={client}><TestRouter initialEntries={[`/ops/creation?domain=${domain}&section=assets`]}><WorkbenchPage/></TestRouter></QueryClientProvider>);
  await screen.findByText(`资产库页 ${domain}`);fireEvent.click(screen.getByRole('button',{name:'测试资产未保存'}));fireEvent.click(screen.getByRole('button',{name:'资产库'}));expect(screen.queryByText(/当前内容有未保存编辑/)).not.toBeInTheDocument();fireEvent.click(screen.getByRole('link',{name:'测试来源链接'}));
  expect(await screen.findByText(/当前内容有未保存编辑/)).toBeVisible();fireEvent.click(screen.getByRole('button',{name:'继续编辑'}));
  expect(screen.getByText(`资产库页 ${domain}`)).toBeVisible();fireEvent.click(screen.getByRole('link',{name:'测试来源链接'}));
  fireEvent.click(await screen.findByRole('button',{name:'丢弃编辑并离开'}));await screen.findByLabelText('正文第 1 段');
});

it('keeps navigation blocked for an uncertain title adoption even after title status refresh', async () => {
  function Navigation(){const navigate=useNavigate();return <button onClick={()=>navigate('/elsewhere')}>离开创作</button>;}
  vi.mocked(api.titleReadiness).mockResolvedValue({configured:true,transportCallable:true,writingQualified:null,configRef:domain,methodVersion:'creation-title.v1',deniedReasons:[]});
  vi.mocked(api.titlePreview).mockResolvedValue({previewRef:paragraph,inputRevision:revision,inputHash:'a'.repeat(64),action:'draft',methodVersion:'creation-title.v1',configRef:domain,modelRef:domain,connectionVersionRef:domain,sourceCount:0,inputCharacterCount:10,estimatedInputTokens:10,reservedTokens:100,maxCalls:1,outboundInput:{},confirmationRequired:true});
  const titleStatus=(id:string)=>({requestId:id,workRef:work,invocationRef:domain,state:'succeeded',inputRevision:revision,action:'title_ideas' as const,methodVersion:'creation-title.v1',suggestion:{coverTitles:['从第一步开始'],noteTitles:['先做一件小事']},titleWarnings:{cover:[false],note:[false]},sourceQualification:'readable',failureCode:null,staleBase:false,usage:{state:'measured',inputTokens:10,outputTokens:10,chargedTokens:20,costUsd:null}});
  vi.mocked(api.titleExecute).mockImplementation(async command=>titleStatus(command.requestId));vi.mocked(api.titleStatus).mockImplementation(async id=>titleStatus(id));
  vi.mocked(api.lookupWorkRequest).mockReset().mockResolvedValue(null);
  vi.mocked(api.adoptTitle).mockRejectedValueOnce(new ApiError({kind:'network'})).mockRejectedValueOnce(new ApiError({kind:'http',status:403,code:'creation_ai_source_unavailable'}));
  const client=new QueryClient({defaultOptions:{queries:{retry:false}}});
  render(<QueryClientProvider client={client}><TestRouter initialEntries={[`/ops/creation?domain=${domain}&work=${work}&view=body`]}><Navigation/><WorkbenchPage/></TestRouter></QueryClientProvider>);
  await screen.findByLabelText('正文第 1 段');fireEvent.click(screen.getByRole('button',{name:'换一批'}));await screen.findByRole('dialog',{name:'AI 会看到的内容'});fireEvent.click(screen.getByLabelText('我已核对，发送内容不含真实姓名、学校、密码和点赞数'));fireEvent.click(screen.getByRole('button',{name:'确认并生成'}));fireEvent.click(await screen.findByRole('button',{name:'从第一步开始'}));
  await screen.findByRole('button',{name:'沿用原请求重试'});fireEvent.focus(window);
  await waitFor(()=>expect(api.titleStatus).toHaveBeenCalledTimes(2));fireEvent.click(screen.getByRole('button',{name:'离开创作'}));
  expect(await screen.findByText(/标题请求尚未完成/)).toBeVisible();fireEvent.click(screen.getByRole('button',{name:'返回标题'}));
  fireEvent.click(screen.getByRole('button',{name:'沿用原请求重试'}));
  await waitFor(()=>expect(api.adoptTitle).toHaveBeenCalledTimes(2));
  await waitFor(()=>expect(screen.queryByRole('button',{name:'沿用原请求重试'})).not.toBeInTheDocument());
  fireEvent.click(screen.getByRole('button',{name:'离开创作'}));
  await waitFor(()=>expect(screen.queryByText(/标题请求尚未完成/)).not.toBeInTheDocument());
});

it('keeps the original writing request after a 503 and retries the same identity', async () => {
  vi.mocked(api.readiness).mockResolvedValue({configured:true,transportCallable:true,writingQualified:true,configRef:domain,methodVersion:'writing.v1'});
  vi.mocked(api.preview).mockResolvedValue({previewRef:paragraph,inputRevision:revision,inputHash:'hash',action:'draft',methodVersion:'writing.v1',configRef:domain,modelRef:domain,connectionVersionRef:domain,sourceCount:0,inputCharacterCount:1,estimatedInputTokens:1,reservedTokens:200,maxCalls:1,outboundInput:{},confirmationRequired:true});
  const status=(id:string):AiStatus=>({requestId:id,workRef:work,invocationRef:null,state:'queued',inputRevision:revision,action:'draft',methodVersion:'writing.v1',suggestion:null,sourceQualification:'readable',failureCode:null,staleBase:false,usage:{state:'not_reserved',inputTokens:null,outputTokens:null,chargedTokens:null,costUsd:null}});
  vi.mocked(api.execute).mockRejectedValueOnce(new ApiError({kind:'http',status:503,code:'creation_ai_unavailable'})).mockImplementationOnce(async cmd=>status(cmd.requestId));
  vi.mocked(api.aiStatus).mockRejectedValueOnce(new ApiError({kind:'http',status:404,code:'creation_ai_not_found'}));
  show();await screen.findByLabelText('正文第 1 段');fireEvent.click(screen.getByRole('button',{name:'按材料起草'}));
  const prepare=await screen.findByRole('button',{name:'准备并核对输入'});await waitFor(()=>expect(prepare).toBeEnabled());fireEvent.click(prepare);
  fireEvent.click(await screen.findByRole('checkbox',{name:'已核对不含真实姓名、学校、密码和点赞数，确认输入及额度'}));fireEvent.click(screen.getByRole('button',{name:'执行一次'}));
  fireEvent.click(await screen.findByRole('button',{name:'核实原请求并重试'}));
  await waitFor(()=>expect(api.execute).toHaveBeenCalledTimes(2));expect(vi.mocked(api.execute).mock.calls[0]).toEqual(vi.mocked(api.execute).mock.calls[1]);
});

it('does not reveal a late writing preview after experience withdrawal and reset', async () => {
  const initial=envelope();initial.revision.document.experienceSlots=[{paragraphId:paragraph,experienceRef:work,experienceRevision:1}];
  vi.mocked(api.readWork).mockResolvedValue(initial);vi.mocked(api.readiness).mockResolvedValue({configured:true,transportCallable:true,writingQualified:true,configRef:domain,methodVersion:'writing.v1'});
  let resolve!: (value: Awaited<ReturnType<typeof api.preview>>) => void;vi.mocked(api.preview).mockImplementationOnce(()=>new Promise(done=>{resolve=done;}));
  show();const body=await screen.findByLabelText('正文第 1 段') as HTMLTextAreaElement;body.setSelectionRange(0, 3);fireEvent.select(body);fireEvent.click(screen.getByRole('button',{name:'改写选段'}));const prepare=await screen.findByRole('button',{name:'准备并核对输入'});await waitFor(()=>expect(prepare).toBeEnabled());fireEvent.click(prepare);
  await waitFor(()=>expect(api.preview).toHaveBeenCalledTimes(1));
  const restricted=structuredClone(initial);restricted.experienceResetRequired=true;restricted.revision.document.paragraphs[0]!.text='[本人已撤回这段经历]';vi.mocked(api.readWork).mockResolvedValue(restricted);
  vi.mocked(api.resetWithdrawn).mockImplementation(async (_id,command)=>({requestId:command.requestId,workId:work,outcome:'accepted',replayed:false,revision:{...initial.revision,id:crypto.randomUUID(),number:2,document:{...restricted.revision.document,experienceEpoch:crypto.randomUUID(),experienceSlots:[]}}}));
  fireEvent.focus(window);fireEvent.click(await screen.findByRole('button',{name:'撤回后重新开始编辑'}));fireEvent.click(screen.getByRole('button',{name:'丢弃本页编辑并继续'}));await screen.findByLabelText('正文第 1 段');
  await act(async()=>resolve({previewRef:paragraph,inputRevision:revision,inputHash:'hash',action:'draft',methodVersion:'writing.v1',configRef:domain,modelRef:domain,connectionVersionRef:domain,sourceCount:0,inputCharacterCount:1,estimatedInputTokens:1,reservedTokens:200,maxCalls:1,outboundInput:{text:'撤回的合成私密文字'},confirmationRequired:true}));
  expect(screen.queryByText(/撤回的合成私密文字/)).not.toBeInTheDocument();expect(api.execute).not.toHaveBeenCalled();
});

it('keeps an uncertain writing request but hides a late result after experience withdrawal', async () => {
  const initial=envelope();initial.revision.document.experienceSlots=[{paragraphId:paragraph,experienceRef:work,experienceRevision:1}];
  vi.mocked(api.readWork).mockResolvedValue(initial);vi.mocked(api.readiness).mockResolvedValue({configured:true,transportCallable:true,writingQualified:true,configRef:domain,methodVersion:'writing.v1'});
  vi.mocked(api.preview).mockResolvedValue({previewRef:paragraph,inputRevision:revision,inputHash:'hash',action:'draft',methodVersion:'writing.v1',configRef:domain,modelRef:domain,connectionVersionRef:domain,sourceCount:0,inputCharacterCount:1,estimatedInputTokens:1,reservedTokens:200,maxCalls:1,outboundInput:{},confirmationRequired:true});
  let resolve!: (value: AiStatus) => void;vi.mocked(api.execute).mockImplementationOnce(()=>new Promise(done=>{resolve=done;}));
  const status=(id:string,available:boolean):AiStatus=>({requestId:id,workRef:work,invocationRef:domain,state:'succeeded',inputRevision:revision,action:'draft',methodVersion:'writing.v1',suggestion:available?{title:null,paragraphs:[{paragraphId:paragraph,text:'已撤回经历的模型结果'}],pages:[]}:null,sourceQualification:available?'readable':'unavailable',failureCode:null,staleBase:false,usage:{state:'measured',inputTokens:1,outputTokens:1,chargedTokens:2,costUsd:null}});
  vi.mocked(api.aiStatus).mockImplementation(async id=>status(id,false));
  show();const body=await screen.findByLabelText('正文第 1 段') as HTMLTextAreaElement;body.setSelectionRange(0, 3);fireEvent.select(body);fireEvent.click(screen.getByRole('button',{name:'改写选段'}));const prepare=await screen.findByRole('button',{name:'准备并核对输入'});await waitFor(()=>expect(prepare).toBeEnabled());fireEvent.click(prepare);
  fireEvent.click(await screen.findByRole('checkbox',{name:'已核对不含真实姓名、学校、密码和点赞数，确认输入及额度'}));fireEvent.click(screen.getByRole('button',{name:'执行一次'}));await waitFor(()=>expect(api.execute).toHaveBeenCalledTimes(1));
  const restricted=structuredClone(initial);restricted.experienceResetRequired=true;restricted.revision.document.paragraphs[0]!.text='[本人已撤回这段经历]';vi.mocked(api.readWork).mockResolvedValue(restricted);
  vi.mocked(api.resetWithdrawn).mockImplementation(async (_id,command)=>({requestId:command.requestId,workId:work,outcome:'accepted',replayed:false,revision:{...initial.revision,id:crypto.randomUUID(),number:2,document:{...restricted.revision.document,experienceEpoch:crypto.randomUUID(),experienceSlots:[]}}}));
  fireEvent.focus(window);fireEvent.click(await screen.findByRole('button',{name:'撤回后重新开始编辑'}));fireEvent.click(screen.getByRole('button',{name:'丢弃本页编辑并继续'}));await screen.findByLabelText('正文第 1 段');
  const requestId=vi.mocked(api.execute).mock.calls[0]![0].requestId;await act(async()=>resolve(status(requestId,true)));
  expect(screen.queryByText(/已撤回经历的模型结果/)).not.toBeInTheDocument();fireEvent.click(await screen.findByRole('button',{name:'核实上次生成'}));
  await waitFor(()=>expect(api.aiStatus).toHaveBeenCalledWith(requestId));expect(api.execute).toHaveBeenCalledTimes(1);expect(screen.queryByText(/已撤回经历的模型结果/)).not.toBeInTheDocument();
});


it('keeps creation under operations and exposes one set of three creation destinations', async () => {
  show(); await screen.findByLabelText('正文第 1 段');
  const primary = screen.getByRole('navigation', { name: '一级导航' });
  expect(within(primary).getAllByRole('link').map(link => link.getAttribute('href'))).toEqual(['/topics', '/corpus', '/collection', '/ops']);
  expect(within(primary).getByRole('link', { name: /运营/ })).toHaveAttribute('aria-current', 'page');
  const sidebar = screen.getByRole('complementary', { name: '运营导航' });
  expect(within(sidebar).getByRole('link', { name: '创作' })).toHaveAttribute('aria-current', 'page');
  expect(within(sidebar).getByRole('link', { name: '数据' })).toHaveAttribute('href', `/ops/overview?domain=${domain}`);
  const pages = screen.getByRole('navigation', { name: '创作页面' });
  expect(within(pages).getAllByRole('button').map(button => button.textContent)).toEqual(['写稿', '材料桌', '资产库']);
  expect(within(pages).getByRole('button', { name: '写稿' })).toHaveAttribute('aria-current', 'page');
  expect(screen.getByText(/^创作步骤/, { selector: 'summary' }).closest('details')).not.toHaveAttribute('open');
  fireEvent.click(screen.getByRole('button', { name: '全部稿件' }));
  const content = await screen.findByRole('group', { name: '写稿内容' });
  expect(within(content).getByRole('button', { name: '我的稿件' })).toHaveAttribute('aria-pressed', 'true');
  fireEvent.click(within(content).getByRole('button', { name: '今日选题' }));
  expect(within(screen.getByRole('group', { name: '写稿内容' })).getByRole('button', { name: '今日选题' })).toHaveAttribute('aria-pressed', 'true');
});
