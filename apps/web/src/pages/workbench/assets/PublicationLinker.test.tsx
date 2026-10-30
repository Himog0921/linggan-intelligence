import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useState } from 'react';
import { PublicationLinker, type PublicationPending } from './PublicationLinker';
import * as api from './assetsApi';
import { newDocument } from '../workbenchModel';
vi.mock('./assetsApi', async original => ({ ...(await original<typeof import('./assetsApi')>()), publicationCandidates: vi.fn(), linkPublication: vi.fn(), publicationLinks: vi.fn(), unlinkPublication: vi.fn() }));
const work = '00000000-0000-4000-8000-000000000001';
const revision = '00000000-0000-4000-8000-000000000002';
const publication = '00000000-0000-4000-8000-000000000003';
function show() {
  const saveCurrent = vi.fn().mockResolvedValue({ id: revision, number: 1, savedAt: '2026-10-29', document: newDocument() });
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  function Harness() { const [pending, setPending] = useState<PublicationPending | null>(null); return <PublicationLinker workId={work} domainRef={work} saveCurrent={saveCurrent} disabled={false} pending={pending} onPending={setPending} />; }
  render(<QueryClientProvider client={client}><Harness /></QueryClientProvider>);
  return saveCurrent;
}
beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(api.publicationCandidates).mockResolvedValue([{ publicationRef: publication, title: '合成发布', publishedOn: '2026-10-29', url: 'https://example.test/note' }]);
  vi.mocked(api.publicationLinks).mockResolvedValue([]);
  vi.mocked(api.linkPublication).mockImplementation(async (_work, cmd) => ({ linkRef: work, revisionId: cmd.expectedRevision, publicationRef: cmd.publicationRef, replayed: false }));
});
describe('publication link', () => {
  it('saves first and links only the record the operator chooses', async () => {
    const save = show(); fireEvent.click(screen.getByRole('button', { name: '关联发布记录' }));
    const select = await screen.findByLabelText('发布记录');
    expect(save).toHaveBeenCalled();
    expect(api.linkPublication).not.toHaveBeenCalled();
    fireEvent.change(select, { target: { value: publication } });
    fireEvent.click(screen.getByRole('button', { name: '确认关联' }));
    await screen.findByText('已关联运营发布记录。');
    expect(api.linkPublication).toHaveBeenCalledWith(work, expect.objectContaining({ expectedRevision: revision, publicationRef: publication }));
  });
  it('reuses the request identity after an uncertain response', async () => {
    vi.mocked(api.linkPublication).mockRejectedValueOnce(new Error('network'));
    show(); fireEvent.click(screen.getByRole('button', { name: '关联发布记录' }));
    fireEvent.change(await screen.findByLabelText('发布记录'), { target: { value: publication } });
    fireEvent.click(screen.getByRole('button', { name: '确认关联' }));
    await screen.findByText('关联结果尚未确认。请沿用原请求重试。');
    fireEvent.click(screen.getByRole('button', { name: '重试关联' }));
    await waitFor(() => expect(api.linkPublication).toHaveBeenCalledTimes(2));
    expect(vi.mocked(api.linkPublication).mock.calls[0]?.[1]).toEqual(vi.mocked(api.linkPublication).mock.calls[1]?.[1]);
  });
  it('allows an explicit correction without deleting the Operations record', async () => {
    vi.mocked(api.publicationLinks).mockResolvedValue([{ linkRef: publication, revisionId: revision, publicationRef: publication, title: '合成发布', publishedOn: null, available: true }]);
    vi.mocked(api.unlinkPublication).mockResolvedValue({ linkRef: publication, replayed: false });
    vi.spyOn(window, 'confirm').mockReturnValueOnce(true);
    show(); fireEvent.click(screen.getByRole('button', { name: '关联发布记录' }));
    fireEvent.click(await screen.findByRole('button', { name: '撤销关联' }));
    await screen.findByText('已撤销关联，运营发布记录仍保留。');
    expect(api.unlinkPublication).toHaveBeenCalledWith(work, publication, expect.any(String));
    vi.restoreAllMocks();
  });
  it('can retry an uncertain unlink even when the record has disappeared from the active list', async () => {
    vi.mocked(api.publicationLinks).mockResolvedValueOnce([{ linkRef: publication, revisionId: revision, publicationRef: publication, title: '合成发布', publishedOn: null, available: true }]).mockResolvedValue([]);
    vi.mocked(api.unlinkPublication).mockRejectedValueOnce(new Error('network')).mockResolvedValueOnce({ linkRef: publication, replayed: true });
    vi.spyOn(window, 'confirm').mockReturnValueOnce(true);
    show(); fireEvent.click(screen.getByRole('button', { name: '关联发布记录' }));
    fireEvent.click(await screen.findByRole('button', { name: '撤销关联' }));
    await screen.findByText('撤销结果尚未确认，请沿用原请求重试。');
    fireEvent.click(screen.getByRole('button', { name: '关闭' }));
    fireEvent.click(screen.getByRole('button', { name: '关联发布记录' }));
    fireEvent.click(await screen.findByRole('button', { name: '沿用原请求重试撤销' }));
    await screen.findByText('已撤销关联，运营发布记录仍保留。');
    expect(vi.mocked(api.unlinkPublication).mock.calls[0]).toEqual(vi.mocked(api.unlinkPublication).mock.calls[1]);
    vi.restoreAllMocks();
  });
});
it('keeps the original request accessible when editing becomes locked', async () => {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const pending: PublicationPending = { kind: 'link', command: { requestId: 'synthetic-request', expectedRevision: revision, publicationRef: publication } };
  render(<QueryClientProvider client={client}><PublicationLinker workId={work} domainRef={work} saveCurrent={vi.fn()} disabled pending={pending} onPending={() => {}} /></QueryClientProvider>);
  fireEvent.click(screen.getByRole('button', { name: '关联发布记录' }));
  expect(await screen.findByRole('button', { name: '重试关联' })).toBeEnabled();
});
