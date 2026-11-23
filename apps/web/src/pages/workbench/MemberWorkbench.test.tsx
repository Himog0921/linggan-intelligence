import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, within } from '@testing-library/react';
import { createMemoryRouter, RouterProvider } from 'react-router';
import { expect, it, vi } from 'vitest';
import * as operations from '../ops/api';
import { ApiError } from '../../shared/api';
import { MemberDraft, MemberWorkbench } from './MemberWorkbench';
import { newDocument, type SaveReceipt } from './workbenchModel';
import * as creation from './memberWorkbenchApi';

vi.mock('../ops/api', async original => ({
  ...(await original<typeof import('../ops/api')>()),
  createNativePublication: vi.fn(),
}));
vi.mock('./memberWorkbenchApi', async original => ({
  ...(await original<typeof import('./memberWorkbenchApi')>()),
  save: vi.fn(),
  create: vi.fn(),
  readDomains: vi.fn(),
  readWorkspace: vi.fn(),
  readDraft: vi.fn(),
}));

const draft: creation.Draft = {
  workId: 'draft', domainRef: 'domain', revisionId: 'saved-revision',
  editable: false, ownership: 'mine', restricted: false, publications: [],
  document: { title: '已有结构化稿', accountRef: 'account', tags: [], paragraphs: [] },
};
const account = { ref: 'account', name: '责任账号', platform: 'xhs' as const, canPublish: true };

function show(ownership: creation.Draft['ownership'], canPublish: boolean) {
  const router = createMemoryRouter([{
    path: '/',
    element: <MemberDraft draft={{ ...draft, ownership }} accounts={[{ ...account, canPublish }]} refresh={vi.fn()} />,
  }]);
  return render(<QueryClientProvider client={new QueryClient()}><RouterProvider router={router} /></QueryClientProvider>);
}

it('registers an existing structured draft using its saved revision without saving new content', async () => {
  vi.mocked(operations.createNativePublication).mockImplementation(async command => ({
    requestRef: command.requestId, outcome: 'accepted', code: 'accepted', recordRef: 'publication',
    resultingVersion: 1, replayed: false,
  }));
  show('mine', true);
  expect(screen.queryByRole('button', { name: '保存稿件' })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '登记发布' }));
  const dialog = within(await screen.findByRole('dialog', { name: '新增原生发布' }));
  fireEvent.change(dialog.getByLabelText('发布链接'), { target: { value: 'https://example.test/post' } });
  fireEvent.change(dialog.getByLabelText('发布日期'), { target: { value: '2026-11-22' } });
  fireEvent.click(dialog.getByRole('button', { name: '登记发布' }));
  expect(await screen.findByRole('link', { name: '查看发布记录' })).toHaveAttribute('href', '/ops/publications?domain=domain&publication_ref=publication');
  expect(creation.save).not.toHaveBeenCalled();
  expect(operations.createNativePublication).toHaveBeenCalledWith(expect.objectContaining({
    accountRef: 'account', creation: { workId: 'draft', expectedRevision: 'saved-revision' },
  }));
});

it.each([['team', true], ['mine', false]] as const)(
  'withholds registration for ownership %s and account access %s',
  (ownership, canPublish) => {
    show(ownership, canPublish);
    expect(screen.queryByRole('button', { name: '登记发布' })).not.toBeInTheDocument();
  },
);


it.each([
  { kind: 'network' },
  { kind: 'http', status: 503, code: null },
] as const)('retains a creation request when leaving would lose an unconfirmed $kind result', async failure => {
  vi.mocked(creation.readDomains).mockResolvedValue([{ domainRef: draft.domainRef, name: '合成领域' }]);
  vi.mocked(creation.readWorkspace).mockResolvedValue({ works: [], ideas: [], accounts: [] });
  vi.mocked(creation.readDraft).mockResolvedValue(draft);
  let rejectCreation: (error: ApiError) => void = () => {};
  const pending = new Promise<SaveReceipt>((_resolve, reject) => { rejectCreation = reject; });
  vi.mocked(creation.create).mockClear().mockReturnValueOnce(pending).mockImplementationOnce(async command => ({
    workId: draft.workId, requestId: command.requestId, outcome: 'accepted', replayed: true,
    revision: { id: draft.revisionId, document: newDocument('新稿标题'), number: 1, savedAt: '2026-11-22', referenceFormat: 'current' },
  }));
  const router = createMemoryRouter([
    { path: '/ops/creation', element: <MemberWorkbench memberRef="writer" /> },
    { path: '/ops/publications', element: <h1>发布记录</h1> },
  ], { initialEntries: ['/ops/creation?domain=domain&section=works'] });
  render(<QueryClientProvider client={new QueryClient()}><RouterProvider router={router} /></QueryClientProvider>);

  const title = await screen.findByRole('textbox', { name: '稿件标题' });
  fireEvent.change(title, { target: { value: '新稿标题' } });
  fireEvent.click(screen.getByRole('button', { name: '新建稿件' }));
  const dataNavigation = screen.getByRole('link', { name: '数据' });
  fireEvent.click(dataNavigation);
  await screen.findByText('新建稿件尚未确认，请留在当前页面。');
  expect(router.state.location.pathname).toBe('/ops/creation');
  expect(window.dispatchEvent(new Event('beforeunload', { cancelable: true }))).toBe(false);

  rejectCreation(new ApiError(failure));
  const retry = await screen.findByRole('button', { name: '沿用原请求重试' });
  expect(title).toBeDisabled();
  fireEvent.click(screen.getByRole('button', { name: '留在我的创作' }));
  fireEvent.click(dataNavigation);
  await screen.findByText('新建稿件尚未确认，请留在当前页面。');
  expect(router.state.location.search).toBe('?domain=domain&section=works');
  fireEvent.click(retry);

  await screen.findByRole('heading', { name: '已有结构化稿' });
  expect(router.state.location.search).toContain('work=draft');
  expect(creation.create).toHaveBeenCalledTimes(2);
  expect(vi.mocked(creation.create).mock.calls[1]?.[0]).toEqual(vi.mocked(creation.create).mock.calls[0]?.[0]);
  expect(window.dispatchEvent(new Event('beforeunload', { cancelable: true }))).toBe(true);
  fireEvent.click(screen.getByRole('link', { name: '数据' }));
  await screen.findByRole('heading', { name: '发布记录' });
});
