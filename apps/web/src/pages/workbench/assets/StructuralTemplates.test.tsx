import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { beforeEach, expect, it, vi } from 'vitest';
import { useState } from 'react';
import { createMemoryRouter, Link, RouterProvider } from 'react-router';
import { SaveStructuralTemplate } from './SaveStructuralTemplate';
import { StructuralTemplateLibrary } from './StructuralTemplateLibrary';
import * as api from './structuralTemplatesApi';
import { ApiError } from '../../../shared/api';
import { newDocument } from '../workbenchModel';
import { chooseOption } from '../../../test/selectField';
vi.mock('./structuralTemplatesApi', async original => ({ ...(await original<typeof import('./structuralTemplatesApi')>()), createTemplate: vi.fn(), listTemplates: vi.fn(), reviewTemplate: vi.fn(), templateHistory: vi.fn(), reviseTemplate: vi.fn() }));
const work = '00000000-0000-4000-8000-000000000001';
const revision = '00000000-0000-4000-8000-000000000002';
const template: api.Template = { code: 'YB-0001', revision: 1, sourceWorkId: work, sourceRevisionId: revision, sourceAvailable: true, status: 'review', steps: ['opening', 'reflection'], createdAt: '2026-10-29' };
const wrap = (child: React.ReactNode) => { const router = createMemoryRouter([{ path: '/', element: <><Link to='/other'>离开</Link>{child}</> }, { path: '/other', element: <p>其他页面</p> }]); return render(<QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}><RouterProvider router={router}/></QueryClientProvider>); };
beforeEach(() => { vi.resetAllMocks(); vi.mocked(api.templateHistory).mockResolvedValue([template]); });
it('saves only ordered structural labels for the saved revision and retries the same request', async () => {
  const saveCurrent = vi.fn().mockResolvedValue({ id: revision, number: 1, savedAt: '2026-10-29', document: newDocument() });
  vi.mocked(api.createTemplate).mockRejectedValueOnce(new Error('network')).mockResolvedValueOnce({ template, replayed: true });
  function Harness() { const [pending, setPending] = useState<api.CreateCommand | null>(null); return <SaveStructuralTemplate workId={work} domainRef={work} saveCurrent={saveCurrent} disabled={false} pending={pending} onPending={setPending} blocked={false} onResetBlock={() => {}} />; }
  wrap(<Harness />); fireEvent.click(screen.getByRole('button', { name: '存为样板' }));
  fireEvent.click(screen.getByRole('button', { name: '＋ 开头' })); fireEvent.click(screen.getByRole('button', { name: '＋ 观点' }));
  fireEvent.change(screen.getByRole('textbox', { name: '样板名称' }), { target: { value: ' 先有场景 ' } });
  fireEvent.change(screen.getByRole('textbox', { name: '样板标签' }), { target: { value: '观察，示例' } });
  fireEvent.click(screen.getByRole('button', { name: '保存待审核' })); await screen.findByText('保存结果尚未确认，请沿用原请求重试。');
  fireEvent.click(screen.getByRole('button', { name: '重试保存' })); await screen.findByText('已保存 YB-0001，等待审核。');
  expect(saveCurrent).toHaveBeenCalledTimes(1);
  expect(vi.mocked(api.createTemplate).mock.calls[0]).toEqual(vi.mocked(api.createTemplate).mock.calls[1]);
  expect(vi.mocked(api.createTemplate).mock.calls[0]?.[0]).toEqual(expect.objectContaining({ workId: work, expectedRevision: revision, steps: ['opening', 'reflection'], labels: { name: '先有场景', tags: ['观察','示例'] } }));
});
it('shows a withdrawn source as unavailable and disables approval', async () => {
  vi.mocked(api.listTemplates).mockResolvedValue([{ ...template, sourceAvailable: false }]);
  wrap(<StructuralTemplateLibrary domainRef={work} onPendingChange={() => {}} />);
  fireEvent.click(await screen.findByRole('button', { name: /YB-0001/ }));
  expect(screen.getByRole('button', { name: '审核通过' })).toBeDisabled();
  await waitFor(() => expect(api.reviewTemplate).not.toHaveBeenCalled());
});
it('retries an uncertain review with the same request', async () => {
  vi.mocked(api.listTemplates).mockResolvedValue([template]);
  vi.mocked(api.reviewTemplate).mockRejectedValueOnce(new Error('network')).mockResolvedValueOnce({ template: { ...template, status: 'ok' }, replayed: true });
  wrap(<StructuralTemplateLibrary domainRef={work} onPendingChange={() => {}} />);
  fireEvent.click(await screen.findByRole('button', { name: /YB-0001/ }));
  fireEvent.click(screen.getByRole('button', { name: '审核通过' }));
  await screen.findByText('结果尚未确认，请沿用原请求重试。');
  fireEvent.click(screen.getByRole('button', { name: '重试上次操作' }));
  await waitFor(() => expect(api.reviewTemplate).toHaveBeenCalledTimes(2));
  expect(vi.mocked(api.reviewTemplate).mock.calls[0]).toEqual(vi.mocked(api.reviewTemplate).mock.calls[1]);
});
it('keeps retry accessible when editing is locked after an uncertain save', () => {
  const pending: api.CreateCommand = { requestId: 'request', workId: work, expectedRevision: revision, steps: ['opening'] };
  wrap(<SaveStructuralTemplate workId={work} domainRef={work} saveCurrent={vi.fn()} disabled pending={pending} onPending={() => {}} blocked={false} onResetBlock={() => {}} />);
  fireEvent.click(screen.getByRole('button', { name: '存为样板' }));
  expect(screen.getByRole('button', { name: '重试保存' })).toBeEnabled();
});
it('filters samples and retries a version edit with the same request', async () => {
  vi.mocked(api.listTemplates).mockResolvedValue([template, { ...template, code: 'YB-0002', status: 'ok' }]);
  vi.mocked(api.reviseTemplate).mockRejectedValueOnce(new Error('network')).mockResolvedValueOnce({ template: { ...template, revision: 2 }, replayed: true });
  wrap(<StructuralTemplateLibrary domainRef={work} onPendingChange={() => {}} />);
  chooseOption('样板状态', '待审核');
  fireEvent.click(await screen.findByRole('button', { name: /YB-0001/ }));
  expect(screen.queryByRole('button', { name: /YB-0002/ })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '编辑结构' }));
  fireEvent.click(screen.getByRole('button', { name: '＋ 行动' }));
  fireEvent.click(screen.getByRole('button', { name: '保存新版本' }));
  await screen.findByText('结果尚未确认，请沿用原请求重试。');
  fireEvent.click(screen.getByRole('button', { name: '重试上次操作' }));
  await screen.findByText('新版本已保存，等待审核。');
  expect(vi.mocked(api.reviseTemplate).mock.calls[0]).toEqual(vi.mocked(api.reviseTemplate).mock.calls[1]);
  expect(vi.mocked(api.reviseTemplate).mock.calls[0]?.[1].steps).toEqual(['opening', 'reflection', 'action']);
  fireEvent.click(screen.getByText('版本历史'));
  expect(await screen.findByText(/第 1 版 · 待审核 · 开头 → 观点/)).toBeInTheDocument();
});
it('keeps the same version command after a 503 and can retry it', async () => {
  vi.mocked(api.listTemplates).mockResolvedValue([template]);
  vi.mocked(api.reviseTemplate).mockRejectedValueOnce(new ApiError({kind:'http',status:503,code:'creation_assets_unavailable'})).mockResolvedValueOnce({template:{...template,revision:2},replayed:true});
  wrap(<StructuralTemplateLibrary domainRef={work} onPendingChange={() => {}} />);
  fireEvent.click(await screen.findByRole('button',{name:/YB-0001/}));fireEvent.click(screen.getByRole('button',{name:'编辑结构'}));
  fireEvent.click(screen.getByRole('button',{name:'保存新版本'}));await screen.findByText('结果尚未确认，请沿用原请求重试。');
  fireEvent.click(screen.getByRole('button',{name:'重试上次操作'}));await screen.findByText('新版本已保存，等待审核。');
  expect(vi.mocked(api.reviseTemplate).mock.calls[0]).toEqual(vi.mocked(api.reviseTemplate).mock.calls[1]);
});
it('retains the edit after a version conflict and reads the new base before saving', async () => {
  vi.mocked(api.listTemplates).mockResolvedValueOnce([template]).mockResolvedValue([{...template,revision:2}]);
  vi.mocked(api.templateHistory).mockResolvedValueOnce([template]).mockResolvedValue([
    { ...template, revision: 3, labels: { name: '最新样板名称', tags: [] } }, template,
  ]);
  vi.mocked(api.reviseTemplate).mockRejectedValueOnce(new ApiError({kind:'http',status:409,code:'creation_assets_stale'})).mockResolvedValueOnce({template:{...template,revision:4},replayed:false});
  wrap(<StructuralTemplateLibrary domainRef={work} onPendingChange={() => {}} />);
  fireEvent.click(await screen.findByRole('button',{name:/YB-0001/}));fireEvent.click(screen.getByRole('button',{name:'编辑结构'}));fireEvent.click(screen.getByRole('button',{name:'＋ 行动'}));
  fireEvent.click(screen.getByRole('button',{name:'保存新版本'}));await screen.findByText(/样板已有新版本/);
  expect(screen.getByRole('button',{name:'保存新版本'})).toBeDisabled();fireEvent.click(screen.getByRole('button',{name:'读取最新版本'}));
  await screen.findByText('已读取最新版本，请核对后保存。');expect(screen.getByText('新版本：开头 → 观点 → 行动')).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button',{name:'保存新版本'}));await screen.findByText('新版本已保存，等待审核。');
  expect(screen.getByRole('heading', { name: '最新样板名称' })).toBeVisible();
  expect(vi.mocked(api.reviseTemplate).mock.calls[1]?.[1].expectedRevision).toBe(3);
});
it('protects the edited structure when selecting another sample or leaving the asset view', async () => {
  vi.mocked(api.listTemplates).mockResolvedValue([template,{...template,code:'YB-0002'}]);
  const dirty = vi.fn(); const confirm = vi.spyOn(window,'confirm').mockReturnValue(false);
  wrap(<StructuralTemplateLibrary domainRef={work} onPendingChange={() => {}} onDirtyChange={dirty} />);
  fireEvent.click(await screen.findByRole('button',{name:/YB-0001/}));fireEvent.click(screen.getByRole('button',{name:'编辑结构'}));fireEvent.click(screen.getByRole('button',{name:'＋ 行动'}));
  expect(dirty).toHaveBeenLastCalledWith(true);fireEvent.click(screen.getByRole('button',{name:/YB-0002/}));
  expect(confirm).toHaveBeenCalledTimes(1);expect(screen.getByText('新版本：开头 → 观点 → 行动')).toBeVisible();
  fireEvent.click(screen.getByRole('button',{name:'取消编辑'}));expect(dirty).toHaveBeenLastCalledWith(false);confirm.mockRestore();
});
it('keeps a conflicted edit when the refreshed sample was disabled elsewhere', async () => {
  vi.mocked(api.listTemplates).mockResolvedValueOnce([template]).mockResolvedValue([{...template,revision:2,status:'off'}]);
  vi.mocked(api.reviseTemplate).mockRejectedValueOnce(new ApiError({kind:'http',status:409,code:'creation_assets_stale'}));
  wrap(<StructuralTemplateLibrary domainRef={work} onPendingChange={() => {}} />);
  fireEvent.click(await screen.findByRole('button',{name:/YB-0001/}));fireEvent.click(screen.getByRole('button',{name:'编辑结构'}));fireEvent.click(screen.getByRole('button',{name:'＋ 行动'}));
  fireEvent.click(screen.getByRole('button',{name:'保存新版本'}));fireEvent.click(await screen.findByRole('button',{name:'读取最新版本'}));
  await screen.findByText('已读取最新版本，请核对后保存。');expect(screen.getByRole('button',{name:'恢复待审核'})).toBeDisabled();expect(screen.getByText('新版本：开头 → 观点 → 行动')).toBeVisible();
});

it('keeps the editing base when a search finds a newer version', async () => {
  const newer = { ...template, revision: 2, labels: { name: '另一窗口修改', tags: [] } };
  vi.mocked(api.listTemplates).mockResolvedValueOnce([template]).mockResolvedValue([newer]);
  vi.mocked(api.reviseTemplate).mockRejectedValueOnce(new ApiError({kind:'http',status:409,code:'creation_assets_stale'}));
  wrap(<StructuralTemplateLibrary domainRef={work} onPendingChange={() => {}} />);
  fireEvent.click(await screen.findByRole('button', { name: /YB-0001/ }));
  fireEvent.click(screen.getByRole('button', { name: '编辑结构' }));
  fireEvent.change(screen.getByRole('textbox', { name: '样板名称' }), { target: { value: '本地编辑' } });
  fireEvent.change(screen.getByRole('textbox', { name: '搜索样板' }), { target: { value: 'YB-0001' } });
  fireEvent.click(screen.getByRole('button', { name: '搜索' }));
  await screen.findByRole('heading', { name: '另一窗口修改' });
  fireEvent.click(screen.getByRole('button', { name: '保存新版本' }));
  await screen.findByText(/样板已有新版本/);
  expect(vi.mocked(api.reviseTemplate).mock.calls[0]?.[1]).toMatchObject({
    expectedRevision: 1,
    labels: { name: '本地编辑', tags: [] },
  });
});

it('retains unsaved name and tags when a search excludes the selection and history fails', async () => {
  vi.mocked(api.listTemplates).mockResolvedValueOnce([template]).mockResolvedValue([]);
  vi.mocked(api.templateHistory).mockRejectedValue(new Error('network'));
  wrap(<StructuralTemplateLibrary domainRef={work} onPendingChange={() => {}} />);
  fireEvent.click(await screen.findByRole('button', { name: /YB-0001/ }));
  fireEvent.click(screen.getByRole('button', { name: '编辑结构' }));
  fireEvent.change(screen.getByRole('textbox', { name: '样板标签' }), { target: { value: '未保存标签' } });
  fireEvent.change(screen.getByRole('textbox', { name: '搜索样板' }), { target: { value: '不存在的样板' } });
  fireEvent.click(screen.getByRole('button', { name: '搜索' }));
  await screen.findByRole('button', { name: '重新读取样板' });
  expect(screen.getByRole('textbox', { name: '样板标签' })).toHaveValue('未保存标签');
  expect(screen.getByRole('button', { name: '保存新版本' })).toBeDisabled();
});
