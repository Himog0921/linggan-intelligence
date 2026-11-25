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
import { chooseOption, selectedText } from '../../test/selectField';
Object.defineProperty(globalThis.crypto, 'subtle', { value: webcrypto.subtle, configurable: true });
vi.mock('./workbenchApi');
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
const deskEnvelope = (): WorkEnvelope => {
  const result = envelope();
  result.revision.document = {
    ...result.revision.document,
    excerpts: [{ id: 'excerpt-1', citation: { asOf: '2026-10-29', blockRef: 'body', sourceKind: 'note', sourceRef: work, workRef: work, field: 'body', sourceVersion: 'v1', start: 0, end: 6, textHash: 'a'.repeat(64), location: null }, selectedText: '一段原笔记', textHash: 'a'.repeat(64) }],
    canvas: [{ id: 'block-1', excerptId: 'excerpt-1', group: 'inbox', note: '', expression: '', expressionRevision: 0 }],
  };
  return result;
};
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
  vi.mocked(api.deskPositions).mockResolvedValue([]);
  vi.mocked(api.deskCardPositions).mockResolvedValue([]);
  vi.mocked(api.saveDeskPosition).mockResolvedValue({ workId: work, x: 0, y: 0, width: null, height: null });
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
  it('keeps editing commands and the save state available in the writing tools menu', async () => {
    show();
    await screen.findByLabelText('正文第 1 段');
    const summary = screen.getByLabelText('写稿工具，已保存');
    const menu = summary.closest('details');
    expect(menu).not.toHaveAttribute('open');
    fireEvent.click(summary);
    expect(menu).toHaveAttribute('open');
    expect(within(menu!).getByRole('button', { name: '保存' })).toBeInTheDocument();
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
    fireEvent.compositionEnd(body, { data: '中文输入中😀' });
    await waitFor(() => expect(screen.getByRole('button', { name: '保存' })).toBeEnabled());
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
  it('undoes and redoes an entire committed Mandarin composition as one edit', async () => {
    show();
    const body = await screen.findByLabelText('正文第 1 段');
    fireEvent.click(screen.getByLabelText('写稿工具，已保存'));
    const undo = screen.getByRole('button', { name: '撤销' });
    const redo = screen.getByRole('button', { name: '重做' });
    fireEvent.compositionStart(body);
    fireEvent.change(body, { target: { value: 'zh' } });
    fireEvent.change(body, { target: { value: 'zhongwen' } });
    expect(body).toHaveValue('zhongwen');
    expect(undo).toBeDisabled();
    fireEvent.blur(body);
    fireEvent.compositionEnd(body, { data: '中文' });
    // Chromium may emit compositionend before the final input updates the field.
    fireEvent.change(body, { target: { value: '中文' } });
    await waitFor(() => expect(undo).toBeEnabled());
    fireEvent.click(undo);
    expect(body).toHaveValue('甲😀\n原始正文');
    expect(undo).toBeDisabled();
    fireEvent.click(redo);
    expect(body).toHaveValue('中文');
    expect(redo).toBeDisabled();
  });

  it('settles a committed composition before another starts in the same paragraph', async () => {
    show();
    const body = await screen.findByLabelText('正文第 1 段');
    fireEvent.click(screen.getByLabelText('写稿工具，已保存'));
    const undo = screen.getByRole('button', { name: '撤销' });
    fireEvent.compositionStart(body);
    fireEvent.change(body, { target: { value: '中文' } });
    fireEvent.compositionEnd(body, { data: '中文' });
    fireEvent.compositionStart(body);
    fireEvent.change(body, { target: { value: '中文pin' } });
    expect(undo).toBeDisabled();
    fireEvent.change(body, { target: { value: '中文拼' } });
    fireEvent.compositionEnd(body, { data: '拼' });
    await waitFor(() => expect(undo).toBeEnabled());
    fireEvent.click(undo);
    expect(body).toHaveValue('中文');
    fireEvent.click(undo);
    expect(body).toHaveValue('甲😀\n原始正文');
  });

  it('keeps the previous undo and redo after composition is cancelled', async () => {
    show();
    const body = await screen.findByLabelText('正文第 1 段');
    fireEvent.click(screen.getByLabelText('写稿工具，已保存'));
    const undo = screen.getByRole('button', { name: '撤销' });
    const redo = screen.getByRole('button', { name: '重做' });
    fireEvent.change(body, { target: { value: '上次有效编辑' } });
    fireEvent.compositionStart(body);
    fireEvent.change(body, { target: { value: 'wei xuan ci' } });
    fireEvent.compositionEnd(body, { data: '' });
    expect(body).toHaveValue('上次有效编辑');
    fireEvent.click(undo);
    expect(body).toHaveValue('甲😀\n原始正文');

    fireEvent.compositionStart(body);
    fireEvent.change(body, { target: { value: 'wei xuan ci' } });
    expect(redo).toBeDisabled();
    fireEvent.compositionEnd(body, { data: '' });
    fireEvent.click(redo);
    expect(body).toHaveValue('上次有效编辑');
  });

  it('keeps the editor mounted when a writing candidate arrives during composition', async () => {
    vi.mocked(api.readiness).mockResolvedValue({
      configured: true, transportCallable: true, writingQualified: true, configRef: domain, methodVersion: 'writing.v1'
    });
    vi.mocked(api.preview).mockResolvedValue({
      previewRef: paragraph, inputRevision: revision, inputHash: 'hash', action: 'draft', methodVersion: 'writing.v1',
      configRef: domain, modelRef: domain, connectionVersionRef: domain, sourceCount: 0, inputCharacterCount: 20,
      estimatedInputTokens: 20, reservedTokens: 200, maxCalls: 1, outboundInput: {}, confirmationRequired: true
    });
    let finish!: (status: AiStatus) => void;
    vi.mocked(api.execute).mockReturnValue(new Promise(resolve => { finish = resolve; }));
    show();
    const body = await screen.findByLabelText('正文第 1 段');
    fireEvent.click(screen.getByRole('button', { name: '按材料起草' }));
    const prepare = await screen.findByRole('button', { name: '准备并核对输入' });
    await waitFor(() => expect(prepare).toBeEnabled());
    fireEvent.click(prepare);
    fireEvent.click(await screen.findByRole('checkbox', { name: '已核对不含真实姓名、学校、密码和点赞数，确认输入及额度' }));
    fireEvent.click(screen.getByRole('button', { name: '执行一次' }));
    fireEvent.compositionStart(body);
    fireEvent.change(body, { target: { value: 'zhongwen' } });
    await act(async () => finish({
      requestId: paragraph, workRef: work, invocationRef: paragraph, state: 'completed', inputRevision: revision,
      action: 'draft', methodVersion: 'writing.v1', sourceQualification: 'readable', failureCode: null, staleBase: false,
      suggestion: { title: null, paragraphs: [{ paragraphId: paragraph, text: '合成建议' }], pages: [] },
      usage: { state: 'measured', inputTokens: 20, outputTokens: 20, chargedTokens: 40, costUsd: null }
    }));
    expect(screen.getByLabelText('正文第 1 段')).toBe(body);
    expect(body).toHaveValue('zhongwen');
    const compare = screen.getByRole('button', { name: '打开改稿对照' });
    expect(compare).toBeDisabled();
    fireEvent.change(body, { target: { value: '中文' } });
    fireEvent.compositionEnd(body, { data: '中文' });
    expect(body).toHaveValue('中文');
    await waitFor(() => expect(compare).toBeEnabled());
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
      name: /选题与材料/
    }));
    const frame = await screen.findByLabelText('选题框：真实选题');
    fireEvent.click(within(frame).getByRole('button', { name: '去稿件' }));
    expect(await screen.findByLabelText('正文第 1 段')).toHaveValue('未确认正文😀');
    expect(screen.getByLabelText('确认发布标签')).toHaveValue('新标签 保留空格');
    expect(screen.getByRole('button', {
      name: '沿用原请求重试'
    })).toBeInTheDocument();
    await new Promise(resolve => setTimeout(resolve, 1000));
    expect(api.save).toHaveBeenCalledTimes(1);
  });
  it('locks replacement edits after an uncertain placement', async () => {
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
    show('canvas');
    fireEvent.click(await screen.findByRole('button', {
      name: '放入正文 / 指定图页 →'
    }));
    chooseOption('放入正文位置', /^第 1 段 · /);
    fireEvent.click(screen.getByRole('button', {
      name: '确认放入'
    }));
    await screen.findByRole('button', {
      name: '沿用原请求重试'
    });
    expect(screen.getByLabelText(`我的表达 ${paragraph}`)).toBeDisabled();
    expect(api.place).toHaveBeenCalledTimes(1);
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
  expect(api.preview).toHaveBeenCalledWith(expect.objectContaining({ action: 'draft', draftContextExcerptIds: [] }));
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
  expect(screen.getByLabelText('写稿工具，未保存')).toBeInTheDocument();
  fireEvent.click(screen.getByLabelText('写稿工具，未保存'));
  fireEvent.click(screen.getByRole('button', {
    name: '撤销'
  }));
  expect(body).toHaveValue('甲😀\n原始正文');
  expect(screen.getByLabelText('写稿工具，已保存')).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', {
    name: '保存'
  }));
  await waitFor(() => expect(screen.getByLabelText('写稿工具，已保存')).toBeInTheDocument());
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
  vi.mocked(api.lookupWorkRequest).mockImplementation(async requestId=>({requestId,workId:work,outcome:'accepted',replayed:true,revision:{...envelope().revision,id:crypto.randomUUID(),number:2,document:{...envelope().revision.document,paragraphs:[{id:paragraph,text:'作者在对照右侧写的新句子😀'}]}}}));
  show();await screen.findByLabelText('正文第 1 段');fireEvent.click(screen.getByRole('button',{name:'按材料起草'}));
  const prepare=await screen.findByRole('button',{name:'准备并核对输入'});await waitFor(()=>expect(prepare).toBeEnabled());fireEvent.click(prepare);
  fireEvent.click(await screen.findByRole('checkbox',{name:'已核对不含真实姓名、学校、密码和点赞数，确认输入及额度'}));fireEvent.click(screen.getByRole('button',{name:'执行一次'}));
  fireEvent.change(await screen.findByLabelText('编辑改稿 第 1 段'),{target:{value:'作者在对照右侧写的新句子😀'}});
  fireEvent.click(screen.getByRole('button',{name:'采用这一段'}));
  fireEvent.click(await screen.findByRole('button',{name:'沿用原请求重试'}));
  expect(await screen.findByLabelText('正文第 1 段')).toHaveValue('作者在对照右侧写的新句子😀');
  expect(api.adopt).toHaveBeenCalledWith(expect.any(String), expect.objectContaining({draftEdits:[{index:0,text:'作者在对照右侧写的新句子😀'}]}));
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
 expect(selectedText('放入图页位置')).not.toBe('选择内页');expect(screen.getByRole('dialog',{name:'放入作品'})).toBeInTheDocument();
 fireEvent.click(screen.getByRole('button',{name:'确认放入'}));await waitFor(()=>expect(api.place).toHaveBeenCalledTimes(1));
 expect(order).toEqual(['save','place']);expect(stored.excerpts[0]?.citation.sourceVersion).toBe('source1');
 const request=vi.mocked(api.place).mock.calls[0]![1];const pageId=request.targets.flatMap(target=>target.kind==='pageBlock'?[target.pageId]:[])[0]!;expect(stored.pages.find(page=>page.id===pageId)?.kind).toBe('opinion');expect(request.expectedRevision).toBe(savedId);expect(request.targets).toHaveLength(2);expect(request.targets.some(target=>target.kind==='pageBlock'&&target.pageId===pageId)).toBe(true);expect(request.targets.some(target=>target.kind==='paragraph')).toBe(true);
});

it('retains both AI edits after partial adoption and keeps the unadopted draft available after returning and reopening',async()=>{
 const initial=envelope(),second=crypto.randomUUID();initial.revision.document.paragraphs=[{id:paragraph,text:'原一'},{id:second,text:'原二'}];vi.mocked(api.readWork).mockResolvedValue(initial);
 vi.mocked(api.readiness).mockResolvedValue({configured:true,transportCallable:true,writingQualified:true,configRef:domain,methodVersion:'writing.v1'});
 vi.mocked(api.preview).mockResolvedValue({previewRef:paragraph,inputRevision:revision,inputHash:'hash',action:'draft',methodVersion:'writing.v1',configRef:domain,modelRef:domain,connectionVersionRef:domain,sourceCount:0,inputCharacterCount:20,estimatedInputTokens:20,reservedTokens:200,maxCalls:1,outboundInput:{},confirmationRequired:true});
 vi.mocked(api.execute).mockImplementation(async request=>({requestId:request.requestId,workRef:work,invocationRef:paragraph,state:'completed',inputRevision:revision,action:'draft',methodVersion:'writing.v1',suggestion:{title:null,paragraphs:[{paragraphId:paragraph,text:'建议一'},{paragraphId:second,text:'建议二'}],pages:[]},sourceQualification:'readable',failureCode:null,staleBase:false,usage:{state:'measured',inputTokens:20,outputTokens:20,chargedTokens:40,costUsd:null}}));
 vi.mocked(api.adopt).mockImplementation(async(_id,request)=>({requestId:request.requestId,workId:work,outcome:'accepted',replayed:false,revision:{...initial.revision,id:crypto.randomUUID(),number:2,document:{...initial.revision.document,paragraphs:[{id:paragraph,text:request.draftEdits?.[0]?.text ?? '建议一'},{id:second,text:'原二'}]}}}));
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


it('uses the standard creation shell with a single breadcrumb, keeps empty scope explicit, and never loads works for all', async () => {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(<QueryClientProvider client={client}><TestRouter initialEntries={['/ops/creation?domain=all']}><WorkbenchPage/></TestRouter></QueryClientProvider>);
  await screen.findByText('当前领域不可用于创作');
  expect(screen.getByLabelText('创作领域').closest('.v7-context-crumb')).not.toBeNull();
  expect(screen.queryByRole('navigation', { name: '创作页面' })).not.toBeInTheDocument();
  expect(screen.getByRole('navigation', { name: '一级导航' })).toBeInTheDocument();
  expect(api.listWorks).not.toHaveBeenCalled();
  fireEvent.click(screen.getByLabelText('创作领域'));
  fireEvent.click(screen.getByRole('button', { name: '合成领域' }));
  await screen.findByLabelText('新选题标题');
  expect(api.listWorks).toHaveBeenCalledWith(domain, expect.any(AbortSignal));
});

it('reaches the desk from the draft rail and every other editor step from the step menu', async () => {
  vi.mocked(api.listWorks).mockResolvedValue([{ workId: work, domainRef: domain, title: '真实选题', revisionId: revision, savedAt: '2026-10-15T01:00:00Z' }]);
  vi.mocked(api.prepareMaterials).mockResolvedValue({ contract: 'content-workbench-materials.v1', asOf: 'now', methodVersion: 'test', query: '', terms: [], state: 'insufficient', gaps: [], documents: [], nextCursor: null, searchScope: 'test' });
  show(); await screen.findByLabelText('正文第 1 段');
  fireEvent.click(screen.getByRole('button', { name: /选题与材料/ }));
  const frame = await screen.findByLabelText('选题框：真实选题');
  fireEvent.click(within(frame).getByRole('button', { name: '去稿件' }));
  await screen.findByLabelText('正文第 1 段');
  fireEvent.click(screen.getByRole('button', { name: '制作封面 →' }));
  fireEvent.click(await screen.findByText(/^创作步骤/, { selector: 'summary' }));
  const nav = screen.getByRole('navigation', { name: '创作步骤' });
  for (const name of ['选题与材料','创作画布','正文','封面','内页','预览与导出']) {
    expect(within(nav).getByRole('button', { name })).toBeInTheDocument();
  }
});

it('moves a desk frame and saves where it was put', async () => {
  vi.mocked(api.listWorks).mockResolvedValue([{ workId: work, domainRef: domain, title: '真实选题', revisionId: revision, savedAt: '2026-10-15T01:00:00Z' }]);
  show('desk');
  const frame = await screen.findByLabelText('选题框：真实选题');
  const head = frame.querySelector('.frame-head') as HTMLElement;
  fireEvent.pointerDown(head, { button: 0, clientX: 300, clientY: 200, pointerId: 1 });
  fireEvent.pointerMove(head, { clientX: 340, clientY: 260, pointerId: 1 });
  const moved = frame.style.left;
  expect(moved).not.toBe('0px');
  fireEvent.pointerUp(head, { clientX: 340, clientY: 260, pointerId: 1 });
  await waitFor(() => expect(api.saveDeskPosition).toHaveBeenCalledTimes(1));
  const request = vi.mocked(api.saveDeskPosition).mock.calls[0]?.[0];
  expect(request).toEqual({ domainRef: domain, workId: work, x: parseInt(moved, 10), y: parseInt(frame.style.top, 10), width: null, height: null });
});

it('lets a material card be dragged freely and saves where it landed', async () => {
  vi.mocked(api.listWorks).mockResolvedValue([{ workId: work, domainRef: domain, title: '真实选题', revisionId: revision, savedAt: '2026-10-15T01:00:00Z' }]);
  vi.mocked(api.readWork).mockResolvedValue(deskEnvelope());
  show('desk');
  const card = await screen.findByRole('group', { name: /^原笔记/ });
  const before = card.style.left;
  fireEvent.pointerDown(card, { button: 0, clientX: 100, clientY: 100, pointerId: 2 });
  fireEvent.pointerMove(card, { clientX: 260, clientY: 180, pointerId: 2 });
  expect(card.style.left).not.toBe(before);
  expect(card.style.top).not.toBe('96px');
  fireEvent.pointerUp(card, { clientX: 260, clientY: 180, pointerId: 2 });
  await waitFor(() => expect(api.saveDeskCardPosition).toHaveBeenCalledTimes(1));
  expect(vi.mocked(api.saveDeskCardPosition).mock.calls[0]?.[0]).toEqual({
    domainRef: domain, workId: work, blockId: 'block-1',
    x: parseInt(card.style.left, 10), y: parseInt(card.style.top, 10),
  });
});

it('grows a frame when its corner is dragged and saves the size', async () => {
  vi.mocked(api.listWorks).mockResolvedValue([{ workId: work, domainRef: domain, title: '真实选题', revisionId: revision, savedAt: '2026-10-15T01:00:00Z' }]);
  show('desk');
  const frame = await screen.findByLabelText('选题框：真实选题');
  const grip = frame.querySelector('.frame-resize') as HTMLElement;
  fireEvent.pointerDown(grip, { button: 0, clientX: 0, clientY: 0, pointerId: 3 });
  fireEvent.pointerMove(grip, { clientX: 160, clientY: 120, pointerId: 3 });
  fireEvent.pointerUp(grip, { clientX: 160, clientY: 120, pointerId: 3 });
  await waitFor(() => expect(api.saveDeskPosition).toHaveBeenCalledTimes(1));
  const request = vi.mocked(api.saveDeskPosition).mock.calls[0]?.[0];
  expect(request?.width).toBe(parseInt(frame.style.width, 10));
  expect(request?.height).toBe(parseInt(frame.style.height, 10));
  expect(request?.width ?? 0).toBeGreaterThan(720);
  expect(request?.height ?? 0).toBeGreaterThan(300);
});

it('puts each desk frame back where it was last saved', async () => {
  vi.mocked(api.listWorks).mockResolvedValue([{ workId: work, domainRef: domain, title: '真实选题', revisionId: revision, savedAt: '2026-10-15T01:00:00Z' }]);
  vi.mocked(api.deskPositions).mockResolvedValue([{ workId: work, x: 40, y: 80, width: null, height: null }]);
  show('desk');
  const frame = await screen.findByLabelText('选题框：真实选题');
  await waitFor(() => expect(frame.style.left).toBe('40px'));
  expect(frame.style.top).toBe('80px');
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
  fireEvent.click(screen.getByRole('button', { name: '制作封面 →' }));
  fireEvent.click(await screen.findByText(/^创作步骤/, { selector: 'summary' }));
  fireEvent.click(within(screen.getByRole('navigation', { name: '创作步骤' })).getByRole('button', { name: '正文' }));
  fireEvent.click(await screen.findByText('添加笔记标题候选'));
  expect(screen.getByLabelText('新标题候选')).toHaveValue('尚未添加的候选');
  fireEvent.click(screen.getByText('补一个选题框'));
  expect(screen.getByLabelText('新选题框名称')).toHaveValue('尚未建立的选题');
});

it('opens the asset library as a modal for the chosen domain and closes it again', async () => {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  render(<QueryClientProvider client={client}>
    <TestRouter initialEntries={[`/ops/creation?domain=${domain}`]}><WorkbenchPage /></TestRouter>
  </QueryClientProvider>);
  const entry = await screen.findByRole('button', { name: '资产库' });
  await waitFor(() => expect(entry).toBeEnabled());
  fireEvent.click(entry);
  expect(await screen.findByText(`资产库页 ${domain}`)).toBeInTheDocument();
  expect(screen.getByRole('dialog')).toBeInTheDocument();
  const app = screen.getByRole('dialog').closest('.v7-app') ?? document.body;
  fireEvent.mouseDown(screen.getByRole('dialog').parentElement!);
  fireEvent.mouseUp(app);
  await waitFor(() => expect(screen.queryByText(`资产库页 ${domain}`)).not.toBeInTheDocument());
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
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(<QueryClientProvider client={client}><TestRouter initialEntries={[`/ops/creation?domain=${domain}&work=${work}&view=body`]}><WorkbenchPage/></TestRouter></QueryClientProvider>);
  fireEvent.click(await screen.findByRole('button', { name: '测试待确认发布关联' }));
  fireEvent.click(within(screen.getByRole('complementary', { name: '运营导航' })).getByRole('link', { name: '今日选题' }));
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

it('changes a material role from the desk and removes the card', async () => {
  vi.mocked(api.listWorks).mockResolvedValue([{ workId: work, domainRef: domain, title: '真实选题', revisionId: revision, savedAt: '2026-10-15T01:00:00Z' }]);
  vi.mocked(api.readWork).mockResolvedValue(deskEnvelope());
  show('desk');
  const card = await screen.findByRole('group', { name: /^原笔记：一段原笔记/ });
  fireEvent.click(within(card).getByRole('button', { name: '修改材料用途' }));
  const picks = await screen.findByRole('group', { name: '材料用途' });
  fireEvent.click(within(picks).getByRole('button', { name: '例子' }));
  await waitFor(() => expect(screen.getByRole('button', { name: '修改材料用途' })).toHaveTextContent('例子'));
  fireEvent.click(screen.getByRole('button', { name: /^移出桌面：一段原笔记/ }));
  await waitFor(() => expect(screen.queryByRole('group', { name: /^原笔记：一段原笔记/ })).not.toBeInTheDocument());
});

it('offers every use a material can have, in the sample\'s words, and keeps the one chosen', async () => {
  vi.mocked(api.listWorks).mockResolvedValue([{ workId: work, domainRef: domain, title: '真实选题', revisionId: revision, savedAt: '2026-10-15T01:00:00Z' }]);
  vi.mocked(api.readWork).mockResolvedValue(deskEnvelope());
  show('desk');
  const card = await screen.findByRole('group', { name: /^原笔记：一段原笔记/ });
  fireEvent.click(within(card).getByRole('button', { name: '修改材料用途' }));
  const picks = await screen.findByRole('group', { name: '材料用途' });
  expect(within(picks).getAllByRole('button').map(button => button.textContent)).toEqual(['待分', '开头', '共鸣', '经历', '观点', '例子', '结尾', '标题参考', '只做参考', '反例']);
  fireEvent.click(within(picks).getByRole('button', { name: '共鸣' }));
  await waitFor(() => expect(screen.getByRole('button', { name: '修改材料用途' })).toHaveTextContent('共鸣'));
  expect(within(screen.getByRole('group', { name: '材料用途' })).getByRole('button', { name: '共鸣' })).toHaveAttribute('aria-pressed', 'true');
  expect(within(screen.getByRole('group', { name: '材料用途' })).getByRole('button', { name: '待分' })).toHaveAttribute('aria-pressed', 'false');
  expect(screen.getByRole('button', { name: '修改材料用途' })).not.toHaveClass('soft');
  for (const grey of ['反例', '只做参考', '待分']) {
    fireEvent.click(within(screen.getByRole('group', { name: '材料用途' })).getByRole('button', { name: grey }));
    await waitFor(() => expect(screen.getByRole('button', { name: '修改材料用途' })).toHaveTextContent(grey));
    expect(screen.getByRole('button', { name: '修改材料用途' })).toHaveClass('soft');
  }
});

it('writes material memo from the desk and shows every material in frame settings', async () => {
  vi.mocked(api.listWorks).mockResolvedValue([{ workId: work, domainRef: domain, title: '真实选题', revisionId: revision, savedAt: '2026-10-15T01:00:00Z' }]);
  vi.mocked(api.readWork).mockResolvedValue(deskEnvelope());
  show('desk');
  fireEvent.click(await screen.findByRole('group', { name: /^原笔记：一段原笔记/ }));
  fireEvent.change(await screen.findByPlaceholderText('这张材料准备怎么用？'), { target: { value: '用来当开头' } });
  expect(await screen.findByRole('button', { name: '编辑材料思路' })).toHaveTextContent('用来当开头');
  fireEvent.click(screen.getByRole('button', { name: '设置' }));
  expect(await screen.findByLabelText('材料 1 思路')).toHaveValue('用来当开头');
  fireEvent.click(screen.getByRole('button', { name: '添加便签' }));
  expect(await screen.findByRole('group', { name: /^便签：/ })).toBeInTheDocument();
});

const ideaRef = '00000000-0000-4000-8000-0000000000a1', accountA = '00000000-0000-4000-8000-0000000000b1', accountB = '00000000-0000-4000-8000-0000000000b2';
const ideaDesk = (over: Partial<WorkEnvelope['revision']['document']> = {}) => {
  const result = deskEnvelope();
  result.revision.document = { ...result.revision.document, ideaRef, ideaName: '放手也是爱', accountRef: accountA, ...over };
  return result;
};
const ideaBox = (anchorWorkId = work) => ({ ideaRef, anchorWorkId, anchorRevisionId: revision, savedAt: '2026-10-15T01:00:00Z', name: '放手也是爱', intent: '' });
function ideaDeskReady(anchorWorkId = work, document = ideaDesk()) {
  vi.mocked(api.listWorks).mockResolvedValue([{ workId: work, domainRef: domain, title: '真实选题', revisionId: revision, savedAt: '2026-10-15T01:00:00Z', ideaRef }]);
  vi.mocked(api.readWork).mockResolvedValue(document);
  vi.mocked(api.listIdeaBoxes).mockResolvedValue([ideaBox(anchorWorkId)]);
  vi.mocked(api.accounts).mockResolvedValue([
    { ref: accountA, name: '甲号', platform: 'xhs', identity: null },
    { ref: accountB, name: '乙号', platform: 'xhs', identity: null },
  ]);
}

it('opens a draft for each chosen account from the idea’s own frame settings and says what happened', async () => {
  ideaDeskReady();
  vi.mocked(api.openIdeaDrafts).mockImplementation(async (_idea, body) => ({
    requestId: body.requestId, ideaRef, skippedExcerpts: 0, replayed: false,
    drafts: body.accountRefs.map(accountRef => ({ workId: '00000000-0000-4000-8000-0000000000c1', accountRef, created: true, revisionId: revision })),
  }));
  show('desk');
  fireEvent.click(await screen.findByRole('button', { name: '设置' }));
  expect(await screen.findByRole('button', { name: '甲号 · 已开稿' })).toBeDisabled();
  expect(screen.getByRole('button', { name: '用这组材料开稿' })).toBeDisabled();
  fireEvent.click(screen.getByRole('button', { name: '乙号' }));
  const listCalls = vi.mocked(api.listWorks).mock.calls.length;
  fireEvent.click(screen.getByRole('button', { name: '用这组材料开稿' }));
  await waitFor(() => expect(api.openIdeaDrafts).toHaveBeenCalledWith(ideaRef, { requestId: expect.any(String), accountRefs: [accountB] }));
  expect(await screen.findByText('已开稿 1 篇：乙号')).toBeInTheDocument();
  // The new drafts show up on the desk: the list of drafts is read again.
  await waitFor(() => expect(vi.mocked(api.listWorks).mock.calls.length).toBeGreaterThan(listCalls));
  expect(screen.getByRole('button', { name: '乙号' })).toBeInTheDocument();
});

it('keeps the request identity when the answer was lost, and takes a new one once the server has answered', async () => {
  ideaDeskReady();
  vi.mocked(api.openIdeaDrafts).mockRejectedValueOnce(new ApiError({ kind: 'network' }, null)).mockImplementation(async (_idea, body) => ({
    requestId: body.requestId, ideaRef, skippedExcerpts: 0, replayed: true,
    drafts: [{ workId: '00000000-0000-4000-8000-0000000000c1', accountRef: accountB, created: true, revisionId: revision }],
  }));
  show('desk');
  fireEvent.click(await screen.findByRole('button', { name: '设置' }));
  fireEvent.click(await screen.findByRole('button', { name: '乙号' }));
  fireEvent.click(screen.getByRole('button', { name: '用这组材料开稿' }));
  const listCalls = vi.mocked(api.listWorks).mock.calls.length;
  await waitFor(() => expect(api.openIdeaDrafts).toHaveBeenCalledTimes(1));
  expect(await screen.findByText('未确认')).toBeInTheDocument();
  // The drafts may exist even though the answer was lost, so the list is read again.
  await waitFor(() => expect(vi.mocked(api.listWorks).mock.calls.length).toBeGreaterThan(listCalls));
  fireEvent.click(await screen.findByRole('button', { name: '用这组材料开稿' }));
  await waitFor(() => expect(api.openIdeaDrafts).toHaveBeenCalledTimes(2));
  const [first, second] = vi.mocked(api.openIdeaDrafts).mock.calls;
  expect(second?.[1].requestId).toBe(first?.[1].requestId);
  expect(await screen.findByText(/这些稿是上一次同一个请求已经开好的/)).toBeInTheDocument();
});

it('waits until every draft of the idea has been read before saying which accounts are taken, and asks in one account order', async () => {
  const accountC = '00000000-0000-4000-8000-0000000000b3', sibling = '00000000-0000-4000-8000-0000000000e1';
  ideaDeskReady();
  vi.mocked(api.accounts).mockResolvedValue([
    { ref: accountA, name: '甲号', platform: 'xhs', identity: null },
    { ref: accountB, name: '乙号', platform: 'xhs', identity: null },
    { ref: accountC, name: '丙号', platform: 'xhs', identity: null },
  ]);
  vi.mocked(api.listWorks).mockResolvedValue([
    { workId: work, domainRef: domain, title: '真实选题', revisionId: revision, savedAt: '2026-10-15T01:00:00Z', ideaRef },
    { workId: sibling, domainRef: domain, title: '同一选题的另一篇', revisionId: revision, savedAt: '2026-10-15T01:00:00Z', ideaRef },
  ]);
  let release: (value: WorkEnvelope) => void = () => undefined;
  vi.mocked(api.readWork).mockImplementation(id => id === sibling ? new Promise(resolve => { release = resolve; }) : Promise.resolve(ideaDesk()));
  vi.mocked(api.openIdeaDrafts).mockImplementation(async (_idea, body) => ({
    requestId: body.requestId, ideaRef, skippedExcerpts: 0, replayed: false,
    drafts: body.accountRefs.map(accountRef => ({ workId: '00000000-0000-4000-8000-0000000000c1', accountRef, created: true, revisionId: revision })),
  }));
  show('desk');
  fireEvent.click((await screen.findAllByRole('button', { name: '设置' }))[0]!);
  expect(await screen.findByText(/正在读取这个选题下的稿件/)).toBeInTheDocument();
  expect(screen.queryByRole('button', { name: '乙号' })).toBeNull();
  const other = ideaDesk({ accountRef: accountB });
  other.workId = sibling;
  release(other);
  fireEvent.click(await screen.findByRole('button', { name: '丙号' }));
  expect(screen.getByRole('button', { name: '乙号 · 已开稿' })).toBeDisabled();
  fireEvent.click(screen.getByRole('button', { name: '用这组材料开稿' }));
  await waitFor(() => expect(api.openIdeaDrafts).toHaveBeenCalledWith(ideaRef, { requestId: expect.any(String), accountRefs: [accountC] }));
});

it('does not offer to open drafts from a draft that is not the idea’s own', async () => {
  ideaDeskReady('00000000-0000-4000-8000-0000000000d1');
  show('desk');
  fireEvent.click(await screen.findByRole('button', { name: '设置' }));
  expect(await screen.findByRole('button', { name: '切到选题框原稿' })).toBeInTheDocument();
  expect(screen.queryByRole('button', { name: '用这组材料开稿' })).toBeNull();
});

it('renames a frame title and writes a sticky note directly on the desk', async () => {
  vi.mocked(api.listWorks).mockResolvedValue([{ workId: work, domainRef: domain, title: '真实选题', revisionId: revision, savedAt: '2026-10-15T01:00:00Z' }]);
  show('desk');
  const title = await screen.findByRole('textbox', { name: '选题框标题' });
  title.textContent = '改过的选题名';
  fireEvent.blur(title);
  expect(await screen.findByRole('textbox', { name: '选题框标题' })).toHaveTextContent('改过的选题名');
  fireEvent.click(screen.getByRole('button', { name: '添加便签' }));
  const note = await screen.findByRole('textbox', { name: '便签内容' });
  note.textContent = '现场改的一句话';
  fireEvent.blur(note);
  expect(await screen.findByRole('group', { name: /^便签：现场改的一句话/ })).toBeInTheDocument();
});

it('rechecks newly collected unsaved source text before revealing the editor again', async () => {
  vi.mocked(api.qualifyCitation).mockResolvedValue(false);
  vi.mocked(api.listWorks).mockResolvedValue([{ workId: work, domainRef: domain, title: '合成选题', revisionId: revision, savedAt: '2026-10-15T01:00:00Z' }]);
  vi.mocked(api.sources).mockResolvedValue({
    contract: 'content-workbench-sources.v1', asOf: '2026-10-29', distribution: [], total: 1, totalBasis: 'source_directory',
    returnedSourceCount: 1, returnedFragmentCount: 1, nextCursor: null, searchScope: 'admitted_native_title_body_history_directory', scope: 'collected_material_only',
    items: [{ asOf: '2026-10-29', fragmentId: 'fragment-1', blockRef: 'body', speaker: 'other', structure: null, issues: [], sourceKind: 'comment', sourceRef: work, workRef: work, field: 'comment_body', sourceVersion: 'v1', title: '合成评论', text: '合成', bodyState: 'ok', authorLabel: '合成', qualification: 'readable', location: null }],
  });
  vi.mocked(api.anchor).mockResolvedValue({
    citation: { asOf: '2026-10-29', blockRef: 'body', sourceKind: 'comment', sourceRef: work, workRef: work, field: 'comment_body', sourceVersion: 'v1', start: 0, end: 2, textHash: 'a'.repeat(64), location: null },
    selectedText: '合成',
    textHash: 'a'.repeat(64),
  });
  show('desk');
  fireEvent.click(await screen.findByRole('button', { name: '找材料' }));
  fireEvent.click(await screen.findByRole('button', { name: '放进框' }));
  fireEvent.focus(window);
  expect(await screen.findByText(/参考来源的状态已变化/)).toBeVisible();
  expect(api.qualifyCitation).toHaveBeenCalledWith(domain, expect.objectContaining({ sourceKind: 'comment', textHash: 'a'.repeat(64) }));
  fireEvent.click(screen.getByRole('button', { name: '丢弃本页编辑并重新读取' }));
  await waitFor(() => expect(screen.queryByText(/参考来源的状态已变化/)).not.toBeInTheDocument());
  expect(screen.getByRole('button', { name: '放进框' })).toBeVisible();
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


it('keeps creation under operations with one breadcrumb and the desk entry in the draft rail', async () => {
  show(); await screen.findByLabelText('正文第 1 段');
  const primary = screen.getByRole('navigation', { name: '一级导航' });
  expect(within(primary).getAllByRole('link').map(link => link.getAttribute('href'))).toEqual(['/topics', '/corpus', '/collection', '/ops']);
  expect(within(primary).getByRole('link', { name: /运营/ })).toHaveAttribute('aria-current', 'page');
  const sidebar = screen.getByRole('complementary', { name: '运营导航' });
  expect(within(sidebar).getByRole('link', { name: '我的创作' })).toHaveAttribute('aria-current', 'page');
  expect(within(sidebar).getByRole('button', { name: '收起创作子页面' })).toHaveAttribute('aria-expanded', 'true');
  expect(screen.queryByRole('group', { name: '写稿内容' })).not.toBeInTheDocument();
  expect(within(sidebar).getByRole('link', { name: '数据' })).toHaveAttribute('href', `/ops/overview?domain=${domain}`);
  expect(screen.queryByRole('navigation', { name: '创作页面' })).not.toBeInTheDocument();
  const crumbs = screen.getByRole('navigation', { name: '当前位置' });
  expect(crumbs).toHaveTextContent(/运营.*创作\/我的创作/);
  const rail = screen.getByRole('complementary', { name: '稿件列表' });
  expect(within(rail).getByRole('button', { name: /选题与材料/ })).toBeInTheDocument();
});

it('resolves handed-off materials into the existing shelf and validates before adding to the canvas', async () => {
  const initial = envelope();
  const selection = { documentId: `note:${work}`, sourceKind: 'note' as const, sourceRef: work, workRef: work, sourceVersion: 'v1', addedBy: 'manual' as const, matchReasons: [] };
  initial.revision.document.materials = [selection];
  vi.mocked(api.readWork).mockResolvedValue(initial);
  vi.mocked(api.listWorks).mockResolvedValue([{ workId: work, domainRef: domain, title: '真实选题', revisionId: revision, savedAt: '2026-10-15T01:00:00Z' }]);
  vi.mocked(api.resolveMaterials).mockResolvedValue({
    contract: 'content-workbench-materials.v1', asOf: '2026-10-29', methodVersion: 'test', query: '', terms: [], state: 'ready', gaps: [], nextCursor: null, searchScope: 'test',
    documents: [{ ...selection, title: '交接来源', authorLabel: '合成', qualification: 'readable', context: null, fragments: [{ asOf: '2026-10-29', fragmentId: 'fragment-1', blockRef: 'body', speaker: 'other', structure: null, issues: [], sourceKind: 'note', sourceRef: work, workRef: work, field: 'body', sourceVersion: 'v1', title: '交接来源', text: '合成原文', bodyState: 'ok', authorLabel: '合成', qualification: 'readable', location: null }] }],
  });
  vi.mocked(api.anchor).mockResolvedValue({ citation: { asOf: '2026-10-29', blockRef: 'body', sourceKind: 'note', sourceRef: work, workRef: work, field: 'body', sourceVersion: 'v1', start: 0, end: 4, textHash: 'a'.repeat(64), location: null }, selectedText: '合成原文', textHash: 'a'.repeat(64) });
  show('desk');
  fireEvent.click(await screen.findByRole('button', { name: '找材料' }));
  expect(await screen.findByText('交接来源')).toBeVisible();
  expect(api.resolveMaterials).toHaveBeenCalledWith({ domainRef: domain, selections: [selection] }, expect.any(AbortSignal));
  expect(api.sources).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: '放进框' }));
  expect(await screen.findByRole('group', { name: /^原笔记：合成原文/ })).toBeVisible();
  expect(api.anchor).toHaveBeenCalledWith(expect.objectContaining({ sourceRef: work, blockRef: 'body', start: 0, end: 4 }));
  fireEvent.click(screen.getByRole('button', { name: '全部材料' }));
  await waitFor(() => expect(api.sources).toHaveBeenCalled());
});

it('marks the current frame as selected (not as a drag target) and puts the cursor in the search field when the shelf opens', async () => {
  vi.mocked(api.listWorks).mockResolvedValue([{ workId: work, domainRef: domain, title: '合成选题', revisionId: revision, savedAt: '2026-10-15T01:00:00Z' }]);
  show('desk');
  fireEvent.click(await screen.findByRole('button', { name: '找材料' }));
  const frame = document.querySelector('.desk .frame');
  expect(frame).toHaveClass('sel');
  expect(frame).not.toHaveClass('hot');
  expect(screen.getByLabelText('搜索材料')).toHaveFocus();
});

it('offers our own publications as a material type and asks the sources for exactly that kind', async () => {
  vi.mocked(api.listWorks).mockResolvedValue([{ workId: work, domainRef: domain, title: '合成选题', revisionId: revision, savedAt: '2026-10-15T01:00:00Z' }]);
  show('desk');
  fireEvent.click(await screen.findByRole('button', { name: '找材料' }));
  const types = within(await screen.findByRole('group', { name: '材料类型' }));
  expect(types.getAllByRole('button').map(button => button.textContent)).toEqual(['全部', '原笔记', '评论', '我方发布', '口播', '图片文字']);
  fireEvent.click(screen.getByRole('button', { name: '全部材料' }));
  fireEvent.click(types.getByRole('button', { name: '我方发布' }));
  await waitFor(() => expect(api.sources).toHaveBeenCalledWith(expect.objectContaining({ domainRef: domain, kind: 'ops_publication' }), expect.any(AbortSignal)));
  expect(screen.getByPlaceholderText('搜原笔记、评论、我方发布')).toBeVisible();
});

// --- The desk canvas: wheel, frames that never overlap, and leaving the desk ---

const otherWork = '00000000-0000-4000-8000-000000000012';
const twoWorks = () => vi.mocked(api.listWorks).mockResolvedValue([
  { workId: work, domainRef: domain, title: '第一个选题', revisionId: revision, savedAt: '2026-10-15T01:00:00Z' },
  { workId: otherWork, domainRef: domain, title: '第二个选题', revisionId: revision, savedAt: '2026-10-15T01:00:00Z' },
]);
const place = (workId: string, x: number, y: number, width: number | null = null, height: number | null = null) => ({ workId, x, y, width, height });
const boxOf = (frame: Element) => { const style = (frame as HTMLElement).style; return { x: parseInt(style.left, 10), y: parseInt(style.top, 10), w: parseInt(style.width, 10), h: parseInt(style.height, 10) }; };
const clear = (a: { x: number; y: number; w: number; h: number }, b: { x: number; y: number; w: number; h: number }, gap = 24) => a.x + a.w + gap <= b.x || b.x + b.w + gap <= a.x || a.y + a.h + gap <= b.y || b.y + b.h + gap <= a.y;
const frames = () => Array.from(document.querySelectorAll('.desk .frame'));
const zoomText = () => screen.getByRole('status', { name: '画布缩放' }).textContent;
const worldAt = () => (document.querySelector('.desk .world') as HTMLElement).style.transform;

it('moves the canvas with the wheel and zooms it only with Ctrl/⌘, as on the demo desk', async () => {
  vi.mocked(api.listWorks).mockResolvedValue([{ workId: work, domainRef: domain, title: '真实选题', revisionId: revision, savedAt: '2026-10-15T01:00:00Z' }]);
  show('desk');
  await screen.findByLabelText('选题框：真实选题');
  const viewport = document.querySelector('.desk .viewport') as HTMLElement;
  const before = worldAt();
  // The wheel over the bare canvas pans it (and is the canvas's, so the page does not scroll as well).
  expect(fireEvent.wheel(viewport, { deltaY: 120 })).toBe(false);
  expect(zoomText()).toBe('100%');
  expect(worldAt()).not.toBe(before);
  // Ctrl/⌘ (also what a trackpad pinch sends) zooms.
  expect(fireEvent.wheel(viewport, { deltaY: 30, ctrlKey: true })).toBe(false);
  expect(parseInt(zoomText() ?? '', 10)).toBeLessThan(100);
  expect(fireEvent.wheel(viewport, { deltaY: -30, metaKey: true })).toBe(false);
  expect(parseInt(zoomText() ?? '', 10)).toBeGreaterThan(70);
});

it('lets the wheel scroll the settings panel instead of moving or zooming the canvas behind it', async () => {
  vi.mocked(api.listWorks).mockResolvedValue([{ workId: work, domainRef: domain, title: '真实选题', revisionId: revision, savedAt: '2026-10-15T01:00:00Z' }]);
  show('desk');
  await screen.findByLabelText('选题框：真实选题');
  fireEvent.click(screen.getByRole('button', { name: '设置' }));
  const panel = await screen.findByRole('region', { name: '选题框设置' });
  const world = worldAt();
  // Not cancelled: the browser scrolls the panel. Ctrl/⌘ makes no difference over a panel.
  expect(fireEvent.wheel(panel, { deltaY: 120 })).toBe(true);
  expect(fireEvent.wheel(panel, { deltaY: 120, ctrlKey: true })).toBe(true);
  expect(zoomText()).toBe('100%');
  expect(worldAt()).toBe(world);
  expect(screen.getByRole('region', { name: '选题框设置' })).toBeInTheDocument();
  // The tool bar is not the canvas either.
  expect(fireEvent.wheel(screen.getByRole('status', { name: '画布缩放' }), { deltaY: 120, ctrlKey: true })).toBe(true);
  expect(zoomText()).toBe('100%');
  // The bare canvas still moves, and takes the panel (anchored to the old place) away.
  expect(fireEvent.wheel(document.querySelector('.desk .viewport') as HTMLElement, { deltaY: 60 })).toBe(false);
  expect(screen.queryByRole('region', { name: '选题框设置' })).toBeNull();
});

it('leaves the desk through a frame’s own 去稿件; the breadcrumb has no second way back to the same place', async () => {
  vi.mocked(api.listWorks).mockResolvedValue([{ workId: work, domainRef: domain, title: '真实选题', revisionId: revision, savedAt: '2026-10-15T01:00:00Z' }]);
  show('desk');
  const frame = await screen.findByLabelText('选题框：真实选题');
  expect(screen.getByText('选题组织 · 材料编排 · 多账号开稿')).toBeInTheDocument();
  expect(screen.queryByRole('button', { name: /返回写稿/ })).toBeNull();
  // Focus mode has its own 去稿件 and still no 返回写稿.
  fireEvent.click(within(frame).getByRole('button', { name: '专注' }));
  expect(screen.queryByRole('button', { name: /返回写稿/ })).toBeNull();
  expect(screen.getAllByRole('button', { name: '去稿件' })).toHaveLength(2);
  fireEvent.click(within(frame).getByRole('button', { name: '去稿件' }));
  expect(await screen.findByLabelText('正文第 1 段')).toBeInTheDocument();
});

it('settles two frames that were saved on top of each other so that none overlaps', async () => {
  twoWorks();
  vi.mocked(api.deskPositions).mockResolvedValue([place(work, 0, 0), place(otherWork, 100, 50)]);
  show('desk');
  await waitFor(() => expect(frames()).toHaveLength(2));
  await waitFor(() => expect(clear(boxOf(frames()[0]!), boxOf(frames()[1]!))).toBe(true));
  // The upper-left frame stays where it was put.
  expect(boxOf(frames()[0]!)).toMatchObject({ x: 0, y: 0 });
});

it('puts a new frame beside a stretched one, not on top of it', async () => {
  twoWorks();
  // The first frame was stretched far wider than the default column; the second has no position yet.
  vi.mocked(api.deskPositions).mockResolvedValue([place(work, 0, 0, 1450, 760)]);
  show('desk');
  await waitFor(() => expect(frames()).toHaveLength(2));
  await waitFor(() => expect(boxOf(frames()[0]!).w).toBe(1450));
  await waitFor(() => expect(clear(boxOf(frames()[0]!), boxOf(frames()[1]!), 48)).toBe(true));
});

it('settles a frame dropped on another at the nearest clear place and saves that place', async () => {
  twoWorks();
  vi.mocked(api.deskPositions).mockResolvedValue([place(work, 0, 0), place(otherWork, 1000, 0)]);
  show('desk');
  await waitFor(() => expect(frames()).toHaveLength(2));
  await waitFor(() => expect(boxOf(frames()[1]!).x).toBe(1000));
  const head = frames()[1]!.querySelector('.frame-head') as HTMLElement;
  fireEvent.pointerDown(head, { button: 0, clientX: 1100, clientY: 20, pointerId: 1 });
  // Held over the first frame: it follows the pointer exactly while it is held.
  fireEvent.pointerMove(head, { clientX: 200, clientY: 70, pointerId: 1 });
  expect(boxOf(frames()[1]!)).toMatchObject({ x: 100, y: 50 });
  fireEvent.pointerUp(head, { clientX: 200, clientY: 70, pointerId: 1 });
  await waitFor(() => expect(api.saveDeskPosition).toHaveBeenCalledTimes(1));
  const saved = vi.mocked(api.saveDeskPosition).mock.calls[0]![0];
  const landed = boxOf(frames()[1]!);
  expect(saved).toMatchObject({ workId: otherWork, x: landed.x, y: landed.y });
  expect(clear(landed, boxOf(frames()[0]!))).toBe(true);
});

it('stops a frame short of its neighbour when its corner is dragged into it', async () => {
  twoWorks();
  vi.mocked(api.deskPositions).mockResolvedValue([place(work, 0, 0), place(otherWork, 800, 0)]);
  show('desk');
  await waitFor(() => expect(frames()).toHaveLength(2));
  await waitFor(() => expect(boxOf(frames()[1]!).x).toBe(800));
  const grip = frames()[0]!.querySelector('.frame-resize') as HTMLElement;
  fireEvent.pointerDown(grip, { button: 0, clientX: 0, clientY: 0, pointerId: 3 });
  fireEvent.pointerMove(grip, { clientX: 400, clientY: 0, pointerId: 3 });
  fireEvent.pointerUp(grip, { clientX: 400, clientY: 0, pointerId: 3 });
  await waitFor(() => expect(api.saveDeskPosition).toHaveBeenCalledTimes(1));
  // 800 (the neighbour) − 24 (clearance): as wide as it can get.
  expect(vi.mocked(api.saveDeskPosition).mock.calls[0]![0]).toMatchObject({ workId: work, width: 776 });
  expect(clear(boxOf(frames()[0]!), boxOf(frames()[1]!))).toBe(true);
});

it('gives a draft that has no account one in the writing view, finding it by name', async () => {
  vi.mocked(api.accounts).mockResolvedValue([
    { ref: '00000000-0000-4000-8000-0000000000a1', name: '裴裴妈妈', platform: 'xhs', identity: null },
    { ref: '00000000-0000-4000-8000-0000000000a2', name: '小满老师', platform: 'xhs', identity: null },
  ]);
  show();
  await screen.findByLabelText('正文第 1 段');
  fireEvent.click(await screen.findByRole('button', { name: '写作账号：未选择账号' }));
  fireEvent.change(screen.getByLabelText('搜索账号'), { target: { value: '小满' } });
  expect(screen.queryByRole('button', { name: '裴裴妈妈' })).toBeNull();
  fireEvent.click(screen.getByRole('button', { name: '小满老师' }));
  expect(await screen.findByRole('button', { name: '写作账号：小满老师' })).toBeInTheDocument();
  // The choice is an edit of the draft: it is saved like any other.
  expect(screen.getByLabelText('写稿工具，未保存')).toBeInTheDocument();
});

it('puts away the open settings panel on Escape, but lets an open menu inside it take the first Escape', async () => {
  vi.mocked(api.listWorks).mockResolvedValue([{ workId: work, domainRef: domain, title: '真实选题', revisionId: revision, savedAt: '2026-10-15T01:00:00Z' }]);
  vi.mocked(api.accounts).mockResolvedValue([{ ref: '00000000-0000-4000-8000-0000000000a1', name: '甲号', platform: 'xhs', identity: null }]);
  show('desk');
  await screen.findByLabelText('选题框：真实选题');
  fireEvent.click(screen.getByRole('button', { name: '设置' }));
  const trigger = await screen.findByRole('combobox', { name: '给哪个账号写' });
  // An open menu closes first; the panel stays.
  fireEvent.click(trigger);
  expect(screen.getByRole('listbox', { name: '给哪个账号写' })).toBeInTheDocument();
  fireEvent.keyDown(trigger, { key: 'Escape' });
  expect(screen.queryByRole('listbox', { name: '给哪个账号写' })).toBeNull();
  expect(screen.getByRole('region', { name: '选题框设置' })).toBeInTheDocument();
  // The next Escape closes the panel itself.
  fireEvent.keyDown(trigger, { key: 'Escape' });
  expect(screen.queryByRole('region', { name: '选题框设置' })).toBeNull();
});

it('opens a stretched frame zoomed out far enough to show its whole width, and a desk that has no width yet at full size', async () => {
  vi.mocked(api.listWorks).mockResolvedValue([{ workId: work, domainRef: domain, title: '真实选题', revisionId: revision, savedAt: '2026-10-15T01:00:00Z' }]);
  vi.mocked(api.deskPositions).mockResolvedValue([place(work, 0, 0, 2000, 400)]);
  // jsdom lays nothing out: the desk is not shown yet, so it opens at 100% rather than at the smallest zoom.
  show('desk');
  await screen.findByLabelText('选题框：真实选题');
  await waitFor(() => expect(parseInt((document.querySelector('.desk .frame') as HTMLElement).style.width, 10)).toBe(2000));
  expect(zoomText()).toBe('100%');
});
