import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '../../../shared/api';
import * as api from './assetsApi';
import { ExperienceLibrary } from './ExperienceLibrary';
import { AssetsLibrary } from './AssetsLibrary';
import type { Experience } from './assetsModel';

vi.mock('./assetsApi', () => ({ listAccounts: vi.fn(), listExperiences: vi.fn(), createExperience: vi.fn(), editExperience: vi.fn(), confirmExperience: vi.fn(), withdrawExperience: vi.fn(), experienceRequestStatus: vi.fn(), cancelExperienceRequest: vi.fn(), assetsErrorText: () => '结果未确认，请重试' }));
const account = '00000000-0000-4000-8000-000000000001';
const id = '00000000-0000-4000-8000-000000000002';
const item: Experience = { id, accountRef: account, code: 'JL-0001', currentPerson: true, state: 'awaiting_subject', revision: 1,
  title: '合成经历', body: '合成经历正文', tags: [], origin: 'oral', sendToModel: null, confirmedAt: null, confirmedSubjectRole: null };
function show() { return render(<QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}><ExperienceLibrary domainRef={account}/></QueryClientProvider>); }
beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(api.listAccounts).mockResolvedValue([{ accountRef: account, accountName: '合成账号', platform: 'xhs', status: 'active', operatorName: null,
    identity: { accountRef: account, revision: 1, orgRelation: 'parent', bioDisclosesRelation: null, bioCheckedAt: null, bioCheckedBy: null,
      hasRealPerson: true, personRef: id, backingPersonRole: '妈妈', canProvideExperience: null, savedAt: 't', savedBy: 'local_machine' } }]);
  vi.mocked(api.listExperiences).mockResolvedValue([item]);
});
async function selectItem() {
  await screen.findByRole('option', { name: '合成账号' });
  fireEvent.change(await screen.findByLabelText('账号'), { target: { value: account } });
  fireEvent.click(await screen.findByRole('button', { name: /合成经历/ }));
}
describe('经历库', () => {
  it('shows saved draft references without implying publication', async () => {
    vi.mocked(api.listExperiences).mockResolvedValue([{ ...item, usedCount: 2 }]);
    show(); await selectItem();
    expect(screen.getByText('2 篇已保存稿件引用 · 不代表已发布')).toBeInTheDocument();
  });
  it('updates the open detail count after references change elsewhere', async () => {
    vi.mocked(api.listExperiences).mockResolvedValue([{ ...item, usedCount: 0 }]);
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(<QueryClientProvider client={client}><ExperienceLibrary domainRef={account}/></QueryClientProvider>);
    await selectItem();
    expect(screen.getByText('0 篇已保存稿件引用 · 不代表已发布')).toBeInTheDocument();
    act(() => client.setQueryData(['creation-assets', 'experiences', account], [{ ...item, usedCount: 1 }]));
    expect(await screen.findByText('1 篇已保存稿件引用 · 不代表已发布')).toBeInTheDocument();
  });
  it('requires saving a changed version before recording confirmation', async () => {
    show(); await selectItem();
    fireEvent.change(screen.getByLabelText('经历正文'), { target: { value: '另一段合成内容' } });
    fireEvent.click(screen.getByLabelText('本人已确认这版内容并同意用于稿件'));
    expect(screen.getByRole('button', { name: '登记本人确认' })).toBeDisabled();
    expect(screen.getByText(/请先保存/)).toBeInTheDocument();
    expect(api.confirmExperience).not.toHaveBeenCalled();
  });
  it('immediately removes withdrawn text even if the list reload fails', async () => {
    vi.mocked(api.withdrawExperience).mockResolvedValue({ replayed: false, experience: { ...item, state: 'withdrawn', body: null, title: null, tags: null, origin: null } });
    vi.mocked(api.listExperiences).mockResolvedValueOnce([item]).mockRejectedValue(new Error('network'));
    show(); await selectItem();
    fireEvent.click(screen.getByLabelText('本人已要求撤回这段经历'));
    fireEvent.click(screen.getByRole('button', { name: '撤回经历' }));
    expect(await screen.findByText('本人已撤回，经历内容已移除。')).toBeInTheDocument();
    expect(screen.queryByDisplayValue('合成经历正文')).not.toBeInTheDocument();
  });
  it('freezes an unanswered request and retries with the same request ID', async () => {
    vi.mocked(api.createExperience).mockRejectedValueOnce(new ApiError({ kind: 'network' })).mockResolvedValue({ experience: item, replayed: true });
    show(); await screen.findByRole('option', { name: '合成账号' }); fireEvent.change(screen.getByLabelText('账号'), { target: { value: account } });
    const form = screen.getByRole('heading', { name: '采集一段经历' }).closest('form')!;
    fireEvent.change(within(form).getByLabelText('标题'), { target: { value: '合成经历' } });
    fireEvent.change(within(form).getByLabelText('经历正文'), { target: { value: '合成经历正文' } });
    fireEvent.click(within(form).getByRole('button', { name: '保存经历' }));
    const retry = await within(form).findByRole('button', { name: '核对并重试原请求' });
    expect(within(form).getByLabelText('经历正文')).toBeDisabled();
    expect(screen.getByLabelText('账号')).toBeDisabled();
    fireEvent.click(retry);
    await waitFor(() => expect(api.createExperience).toHaveBeenCalledTimes(2));
    expect(vi.mocked(api.createExperience).mock.calls[0]?.[0].requestId).toBe(vi.mocked(api.createExperience).mock.calls[1]?.[0].requestId);
  });
  it('keeps an unknown request available while other asset tabs are locked', async () => {
    vi.mocked(api.createExperience).mockRejectedValueOnce(new ApiError({ kind: 'network' })).mockResolvedValue({ experience: item, replayed: true });
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(<QueryClientProvider client={client}><AssetsLibrary domainRef={account}/></QueryClientProvider>);
    fireEvent.click(screen.getByRole('button', { name: '经历库' }));
    await screen.findByRole('option', { name: '合成账号' });
    fireEvent.change(screen.getByLabelText('账号'), { target: { value: account } });
    fireEvent.change(screen.getByLabelText('标题'), { target: { value: '合成经历' } });
    fireEvent.change(screen.getByLabelText('经历正文'), { target: { value: '合成经历正文' } });
    fireEvent.click(screen.getByRole('button', { name: '保存经历' }));
    const retry = await screen.findByRole('button', { name: '核对并重试原请求' });
    expect(screen.getByRole('button', { name: '账号身份' })).toBeDisabled();
    client.setQueryData(['creation-assets', 'experiences', account], [{ ...item, revision: 2 }]);
    expect(retry).toBeInTheDocument();
    fireEvent.click(retry);
    await waitFor(() => expect(api.createExperience).toHaveBeenCalledTimes(2));
    expect(vi.mocked(api.createExperience).mock.calls[0]?.[0].requestId).toBe(vi.mocked(api.createExperience).mock.calls[1]?.[0].requestId);
  });
  it('hides an open experience when another page withdraws it', async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(<QueryClientProvider client={client}><ExperienceLibrary domainRef={account}/></QueryClientProvider>);
    await selectItem();
    expect(screen.getByDisplayValue('合成经历正文')).toBeInTheDocument();
    act(() => client.setQueryData(['creation-assets', 'experiences', account], [{ ...item, state: 'withdrawn', title: null, body: null, tags: null, origin: null }]));
    await waitFor(() => expect(screen.queryByDisplayValue('合成经历正文')).not.toBeInTheDocument());
    expect(screen.getByText('本人已撤回，经历内容已移除。')).toBeInTheDocument();
  });
  it('reconciles by request ID without resending erased text after withdrawal', async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    vi.mocked(api.editExperience).mockRejectedValueOnce(new ApiError({ kind: 'network' }));
    vi.mocked(api.experienceRequestStatus).mockResolvedValue({ kind: 'edit', experienceRef: id, revision: 2, state: 'awaiting_subject' });
    render(<QueryClientProvider client={client}><ExperienceLibrary domainRef={account}/></QueryClientProvider>);
    await selectItem();
    fireEvent.change(screen.getByLabelText('经历正文'), { target: { value: '合成的新内容' } });
    fireEvent.click(screen.getByRole('button', { name: '保存经历' }));
    await screen.findByRole('button', { name: '核对并重试原请求' });
    const requestId = vi.mocked(api.editExperience).mock.calls[0]![1].requestId;
    act(() => client.setQueryData(['creation-assets', 'experiences', account], [{ ...item, state: 'withdrawn', title: null, body: null, tags: null, origin: null }]));
    await screen.findByText('本人已撤回，经历内容已移除。');
    fireEvent.click(screen.getByRole('button', { name: '核对并重试原请求' }));
    await waitFor(() => expect(api.experienceRequestStatus).toHaveBeenCalledWith(requestId));
    expect(api.editExperience).toHaveBeenCalledTimes(1);
  });
  it('can safely abandon an unrecorded request after withdrawal', async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    vi.mocked(api.editExperience).mockRejectedValueOnce(new ApiError({ kind: 'network' }));
    vi.mocked(api.experienceRequestStatus).mockResolvedValue(null);
    vi.mocked(api.cancelExperienceRequest).mockResolvedValue('cancelled');
    render(<QueryClientProvider client={client}><ExperienceLibrary domainRef={account}/></QueryClientProvider>);
    await selectItem();
    fireEvent.change(screen.getByLabelText('经历正文'), { target: { value: '合成的新内容' } });
    fireEvent.click(screen.getByRole('button', { name: '保存经历' }));
    await screen.findByRole('button', { name: '核对并重试原请求' });
    const requestId = vi.mocked(api.editExperience).mock.calls[0]![1].requestId;
    act(() => client.setQueryData(['creation-assets', 'experiences', account], [{ ...item, state: 'withdrawn', title: null, body: null, tags: null, origin: null }]));
    await screen.findByText('本人已撤回，经历内容已移除。');
    fireEvent.click(screen.getByRole('button', { name: '核对并重试原请求' }));
    fireEvent.click(await screen.findByRole('button', { name: '放弃并停止原请求' }));
    await waitFor(() => expect(api.cancelExperienceRequest).toHaveBeenCalledWith(requestId));
    await waitFor(() => expect(screen.getByLabelText('账号')).toBeEnabled());
  });
  it('does not let a late edit response reveal a withdrawn experience again', async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    let finish!: (value: { experience: Experience; replayed: boolean }) => void;
    vi.mocked(api.editExperience).mockImplementation(() => new Promise(resolve => { finish = resolve; }));
    vi.mocked(api.listExperiences).mockResolvedValueOnce([item]).mockRejectedValue(new Error('offline'));
    render(<QueryClientProvider client={client}><ExperienceLibrary domainRef={account}/></QueryClientProvider>);
    await selectItem();
    fireEvent.change(screen.getByLabelText('经历正文'), { target: { value: '合成的新内容' } });
    fireEvent.click(screen.getByRole('button', { name: '保存经历' }));
    act(() => client.setQueryData(['creation-assets', 'experiences', account], [{ ...item, state: 'withdrawn', title: null, body: null, tags: null, origin: null }]));
    await screen.findByText('本人已撤回，经历内容已移除。');
    await act(async () => finish({ experience: { ...item, body: '合成的新内容', revision: 2 }, replayed: false }));
    expect(screen.queryByDisplayValue('合成的新内容')).not.toBeInTheDocument();
    expect(screen.getByText('本人已撤回，经历内容已移除。')).toBeInTheDocument();
  });
  it('asks before discarding unsaved collection when changing asset type', async () => {
    vi.spyOn(window, 'confirm').mockReturnValue(false);
    render(<QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}><AssetsLibrary domainRef={account}/></QueryClientProvider>);
    fireEvent.click(screen.getByRole('button', { name: '经历库' }));
    await screen.findByRole('option', { name: '合成账号' });
    fireEvent.change(screen.getByLabelText('账号'), { target: { value: account } });
    fireEvent.change(screen.getByLabelText('经历正文'), { target: { value: '未保存的合成内容' } });
    fireEvent.click(screen.getByRole('button', { name: '账号身份' }));
    expect(window.confirm).toHaveBeenCalled();
    expect(screen.getByLabelText('经历正文')).toHaveValue('未保存的合成内容');
    vi.restoreAllMocks();
  });
});
