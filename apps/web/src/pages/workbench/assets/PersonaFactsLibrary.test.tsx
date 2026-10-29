import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '../../../shared/api';
import * as api from './assetsApi';
import { PersonaFactsLibrary } from './PersonaFactsLibrary';
import type { PersonaFact } from './assetsModel';

vi.mock('./assetsApi', () => ({ listAccounts: vi.fn(), listPersonaFacts: vi.fn(), personaHistory: vi.fn(), putPersonaFact: vi.fn(), confirmPersonaFact: vi.fn(), withdrawPersonaFact: vi.fn(), personaRequestStatus: vi.fn(), cancelPersonaRequest: vi.fn(), assetsErrorText: () => '结果未确认，请重试' }));
const account = '00000000-0000-4000-8000-000000000001';
const person = '00000000-0000-4000-8000-000000000002';
const id = '00000000-0000-4000-8000-000000000003';
const fact: PersonaFact = { id, accountRef: account, key: '年级', version: 1, currentRevision: 1, currentPerson: true,
  state: 'active', text: '三年级', source: 'oral', status: 'new', confirmationMethod: null };
function show(client = new QueryClient({ defaultOptions: { queries: { retry: false } } })) {
  return render(<QueryClientProvider client={client}><PersonaFactsLibrary domainRef={account}/></QueryClientProvider>);
}
beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(api.listAccounts).mockResolvedValue([{ accountRef: account, accountName: '合成账号', platform: 'xhs', status: 'active', operatorName: null,
    identity: { accountRef: account, revision: 1, orgRelation: 'parent', bioDisclosesRelation: null, bioCheckedAt: null, bioCheckedBy: null,
      hasRealPerson: true, personRef: person, backingPersonRole: '合成角色', canProvideExperience: null, savedAt: 't', savedBy: 'local_machine' } }]);
  vi.mocked(api.listPersonaFacts).mockResolvedValue([fact]);
  vi.mocked(api.personaHistory).mockResolvedValue([]);
});
async function selectFact() {
  await screen.findByRole('option', { name: '合成账号' });
  fireEvent.change(screen.getByLabelText('账号'), { target: { value: account } });
  fireEvent.click(await screen.findByRole('button', { name: /年级/ }));
}
describe('人设事实', () => {
  it('requires an explicit choice for conflicting facts and records the decision', async () => {
    vi.mocked(api.putPersonaFact).mockResolvedValue({ fact: { ...fact, version: 2 }, resultVersion: 2, replayed: false });
    show(); await selectFact();
    fireEvent.change(screen.getByLabelText('事实内容'), { target: { value: '四年级' } });
    expect(screen.getByRole('button', { name: '保存事实' })).toBeDisabled();
    fireEvent.click(screen.getByLabelText('保留原来的'));
    fireEvent.click(screen.getByRole('button', { name: '保存事实' }));
    await waitFor(() => expect(api.putPersonaFact).toHaveBeenCalledWith(expect.objectContaining({ expectedVersion: 1, text: '四年级', conflictDecision: 'keep_old', expectedPersonRef: person })));
  });
  it('does not record confirmation for unsaved changes', async () => {
    show(); await selectFact();
    fireEvent.change(screen.getByLabelText('事实内容'), { target: { value: '四年级' } });
    fireEvent.click(screen.getByLabelText('本人已确认当前事实'));
    expect(screen.getByRole('button', { name: '登记本人确认' })).toBeDisabled();
  });
  it('hides the former person’s fact immediately when cache refresh detects replacement', async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    show(client); await selectFact();
    expect(screen.getByDisplayValue('三年级')).toBeInTheDocument();
    act(() => client.setQueryData(['creation-assets','persona-facts',account], [{ ...fact, currentPerson: false, key: null, text: null, source: null, status: null }]));
    expect(await screen.findByText('这是上一位真人的事实，不能用于当前真人。')).toBeInTheDocument();
    expect(screen.queryByDisplayValue('三年级')).not.toBeInTheDocument();
  });
  it('hides an open fact while another window withdraws it, then displays the redacted state', async () => {
    const channels: Array<{ onmessage: ((event: MessageEvent) => void) | null }> = [];
    class Channel {
      onmessage: ((event: MessageEvent) => void) | null = null;
      constructor() { channels.push(this); }
      close() {} postMessage() {}
    }
    vi.stubGlobal('BroadcastChannel', Channel);
    try {
      show(); await selectFact();
      let finish!: (items: PersonaFact[]) => void;
      vi.mocked(api.listPersonaFacts).mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }));
      act(() => channels.forEach(channel => channel.onmessage?.(new MessageEvent('message', { data: 'persona-changed' }))));
      expect(screen.queryByDisplayValue('三年级')).not.toBeVisible();
      await act(async () => finish([{ ...fact, state: 'withdrawn', key: null, text: null, source: null, status: null }]));
      expect(await screen.findByText('本人已撤回，事实内容已移除。')).toBeVisible();
      expect(screen.queryByDisplayValue('三年级')).not.toBeInTheDocument();
    } finally { vi.unstubAllGlobals(); }
  });
  it('keeps an open form covered after a failed recheck and permits retry without a refresh', async () => {
    const channels: Array<{ onmessage: ((event: MessageEvent) => void) | null }> = [];
    class Channel { onmessage: ((event: MessageEvent) => void) | null = null; constructor() { channels.push(this); } close() {} postMessage() {} }
    vi.stubGlobal('BroadcastChannel', Channel);
    try {
      show(); await selectFact();
      fireEvent.change(screen.getByLabelText('事实内容'), { target: { value: '未保存的合成内容' } });
      vi.mocked(api.listPersonaFacts).mockRejectedValueOnce(new Error('offline'));
      act(() => channels.forEach(channel => channel.onmessage?.(new MessageEvent('message', { data: 'identity-changed' }))));
      const retry = await screen.findByRole('button', { name: '重新核对' });
      expect(screen.getByDisplayValue('未保存的合成内容')).not.toBeVisible();
      fireEvent.click(retry);
      expect(await screen.findByDisplayValue('未保存的合成内容')).toBeVisible();
    } finally { vi.unstubAllGlobals(); }
  });
  it('clears an unsaved new fact when the account changes to a different person', async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    show(client); await screen.findByRole('option', { name: '合成账号' });
    fireEvent.change(screen.getByLabelText('账号'), { target: { value: account } });
    fireEvent.change(screen.getByLabelText('事实内容'), { target: { value: '前真人的合成草稿' } });
    const rows = await api.listAccounts(account);
    act(() => client.setQueryData(['creation-assets','accounts',account], rows.map(row => ({ ...row, identity: { ...row.identity!, personRef: id } }))));
    expect(await screen.findByText('账号真人已变化，原真人的未保存事实已清除。')).toBeVisible();
    expect(screen.queryByDisplayValue('前真人的合成草稿')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '补充事实' }));
    expect(screen.getByLabelText('事实内容')).toHaveValue('');
  });
  it('temporarily hides and preserves a draft when the person is unknown and then restored', async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    show(client); await screen.findByRole('option', { name: '合成账号' });
    fireEvent.change(screen.getByLabelText('账号'), { target: { value: account } });
    fireEvent.change(screen.getByLabelText('事实内容'), { target: { value: '待确认的合成草稿' } });
    const rows = await api.listAccounts(account);
    act(() => client.setQueryData(['creation-assets','accounts',account], rows.map(row => ({ ...row, identity: { ...row.identity!, hasRealPerson: null, personRef: null } }))));
    expect(await screen.findByText('账号真人暂未确认，草稿暂时隐藏；确认仍是同一位真人后可继续编辑。')).toBeVisible();
    expect(screen.queryByDisplayValue('待确认的合成草稿')).not.toBeInTheDocument();
    act(() => client.setQueryData(['creation-assets','accounts',account], rows));
    expect(await screen.findByDisplayValue('待确认的合成草稿')).toBeVisible();
  });
  it('does not discard fact edits when the account refresh finishes before the fact refresh', async () => {
    const channels: Array<{ onmessage: ((event: MessageEvent) => void) | null }> = [];
    class Channel { onmessage: ((event: MessageEvent) => void) | null = null; constructor() { channels.push(this); } close() {} postMessage() {} }
    vi.stubGlobal('BroadcastChannel', Channel);
    try {
      const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
      show(client); await selectFact();
      fireEvent.change(screen.getByLabelText('事实内容'), { target: { value: '待保存的合成事实' } });
      const rows = await api.listAccounts(account);
      act(() => { client.setQueryData(['creation-assets','accounts',account], rows.map(row => ({ ...row, identity: { ...row.identity!, hasRealPerson: null, personRef: null } })));
        client.setQueryData(['creation-assets','persona-facts',account], [{ ...fact, currentPerson: false, key: null, text: null, source: null, status: null }]); });
      await screen.findByText('账号真人暂未确认，草稿暂时隐藏；确认仍是同一位真人后可继续编辑。');
      let finishAccounts!: (items: typeof rows) => void; let finishFacts!: (items: PersonaFact[]) => void;
      vi.mocked(api.listAccounts).mockImplementationOnce(() => new Promise(resolve => { finishAccounts = resolve; }));
      vi.mocked(api.listPersonaFacts).mockImplementationOnce(() => new Promise(resolve => { finishFacts = resolve; }));
      act(() => channels.forEach(channel => channel.onmessage?.(new MessageEvent('message', { data: 'identity-changed' }))));
      await act(async () => finishAccounts(rows));
      await act(async () => finishFacts([fact]));
      expect(await screen.findByDisplayValue('待保存的合成事实')).toBeVisible();
    } finally { vi.unstubAllGlobals(); }
  });
  it('opens an already masked fact in a paused state rather than an editable form', async () => {
    vi.mocked(api.listAccounts).mockImplementationOnce(async () => [{ accountRef: account, accountName: '合成账号', platform: 'xhs', status: 'active', operatorName: null,
      identity: { accountRef: account, revision: 2, orgRelation: 'unknown', bioDisclosesRelation: null, bioCheckedAt: null, bioCheckedBy: null, hasRealPerson: null, personRef: null, backingPersonRole: null, canProvideExperience: null, savedAt: 't', savedBy: 'local_machine' } }]);
    vi.mocked(api.listPersonaFacts).mockResolvedValue([{ ...fact, currentPerson: false, key: null, text: null, source: null, status: null }]);
    show(); await screen.findByRole('option', { name: '合成账号' });
    fireEvent.change(screen.getByLabelText('账号'), { target: { value: account } });
    fireEvent.click(await screen.findByRole('button', { name: /历史事实/ }));
    expect(await screen.findByText('账号真人暂未确认，草稿暂时隐藏；确认仍是同一位真人后可继续编辑。')).toBeVisible();
    expect(screen.queryByLabelText('事实内容')).not.toBeInTheDocument();
  });
  it('keeps the request identity until a paused confirmation can reload its committed result', async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    vi.mocked(api.confirmPersonaFact).mockRejectedValueOnce(new ApiError({ kind: 'network' })).mockResolvedValueOnce({ fact: { ...fact, version: 2, currentRevision: 2, status: 'confirmed' }, resultVersion: 2, replayed: true });
    vi.mocked(api.personaRequestStatus).mockResolvedValueOnce(null).mockResolvedValueOnce({ kind: 'confirm', factRef: id, version: 2 });
    show(client); await selectFact();
    fireEvent.click(screen.getByLabelText('本人已确认当前事实'));
    fireEvent.click(screen.getByRole('button', { name: '登记本人确认' }));
    await screen.findByRole('button', { name: '核对并重试原请求' });
    const rows = await api.listAccounts(account);
    act(() => { client.setQueryData(['creation-assets','accounts',account], rows.map(row => ({ ...row, identity: { ...row.identity!, hasRealPerson: null, personRef: null } })));
      client.setQueryData(['creation-assets','persona-facts',account], [{ ...fact, currentPerson: false, key: null, text: null, source: null, status: null }]); });
    fireEvent.click(await screen.findByRole('button', { name: '核对原请求' }));
    expect(await screen.findByRole('button', { name: '放弃并停止原请求' })).toBeVisible();
    fireEvent.click(screen.getByRole('button', { name: '核对原请求' }));
    await screen.findByText('原请求已登记；确认真人后，请继续核对原请求以读取最新结果。');
    expect(screen.queryByRole('button', { name: '放弃并停止原请求' })).not.toBeInTheDocument();
    vi.mocked(api.listPersonaFacts).mockResolvedValue([{ ...fact, version: 2, currentRevision: 2, status: 'confirmed' }]);
    act(() => { client.setQueryData(['creation-assets','accounts',account], rows); client.setQueryData(['creation-assets','persona-facts',account], [{ ...fact, version: 2, currentRevision: 2, status: 'confirmed' }]); });
    fireEvent.click(await screen.findByRole('button', { name: '核对并重试原请求' }));
    await waitFor(() => expect(api.confirmPersonaFact).toHaveBeenCalledTimes(2));
    expect(vi.mocked(api.confirmPersonaFact).mock.calls[0]![1].requestId).toBe(vi.mocked(api.confirmPersonaFact).mock.calls[1]![1].requestId);
    expect(await screen.findByText('本人已确认 · 修改后需要再次确认。')).toBeVisible();
  });
  it('keeps a paused request locked if cancel discovers it has already committed', async () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    vi.mocked(api.confirmPersonaFact).mockRejectedValueOnce(new ApiError({ kind: 'network' }));
    vi.mocked(api.personaRequestStatus).mockResolvedValue(null);
    vi.mocked(api.cancelPersonaRequest).mockResolvedValue('committed');
    show(client); await selectFact();
    fireEvent.click(screen.getByLabelText('本人已确认当前事实'));
    fireEvent.click(screen.getByRole('button', { name: '登记本人确认' }));
    await screen.findByRole('button', { name: '核对并重试原请求' });
    const rows = await api.listAccounts(account);
    act(() => { client.setQueryData(['creation-assets','accounts',account], rows.map(row => ({ ...row, identity: { ...row.identity!, hasRealPerson: null, personRef: null } })));
      client.setQueryData(['creation-assets','persona-facts',account], [{ ...fact, currentPerson: false, key: null, text: null, source: null, status: null }]); });
    fireEvent.click(await screen.findByRole('button', { name: '核对原请求' }));
    fireEvent.click(await screen.findByRole('button', { name: '放弃并停止原请求' }));
    await screen.findByText('原请求已登记；确认真人后，请继续核对原请求以读取最新结果。');
    expect(screen.getByLabelText('账号')).toBeDisabled();
    expect(screen.getByRole('button', { name: '核对原请求' })).toBeVisible();
    expect(screen.queryByRole('button', { name: '放弃并停止原请求' })).not.toBeInTheDocument();
  });
  it('keeps an uncertain save locked and retries its original request identity', async () => {
    vi.mocked(api.putPersonaFact).mockRejectedValueOnce(new ApiError({ kind: 'network' })).mockResolvedValueOnce({ fact, resultVersion: 1, replayed: true });
    show(); await screen.findByRole('option', { name: '合成账号' });
    fireEvent.change(screen.getByLabelText('账号'), { target: { value: account } });
    fireEvent.change(screen.getByLabelText('事实名称'), { target: { value: '年级' } });
    fireEvent.change(screen.getByLabelText('事实内容'), { target: { value: '三年级' } });
    fireEvent.click(screen.getByRole('button', { name: '保存事实' }));
    const retry = await screen.findByRole('button', { name: '核对并重试原请求' });
    expect(screen.getByLabelText('账号')).toBeDisabled();
    fireEvent.click(retry);
    await waitFor(() => expect(api.putPersonaFact).toHaveBeenCalledTimes(2));
    expect(vi.mocked(api.putPersonaFact).mock.calls[0]![0].requestId).toBe(vi.mocked(api.putPersonaFact).mock.calls[1]![0].requestId);
  });
});
