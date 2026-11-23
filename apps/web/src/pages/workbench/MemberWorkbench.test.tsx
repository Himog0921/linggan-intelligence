import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, within } from '@testing-library/react';
import { createMemoryRouter, RouterProvider } from 'react-router';
import { expect, it, vi } from 'vitest';
import * as operations from '../ops/api';
import { MemberDraft } from './MemberWorkbench';
import * as creation from './memberWorkbenchApi';

vi.mock('../ops/api', async original => ({
  ...(await original<typeof import('../ops/api')>()),
  createNativePublication: vi.fn(),
}));
vi.mock('./memberWorkbenchApi', async original => ({
  ...(await original<typeof import('./memberWorkbenchApi')>()),
  save: vi.fn(),
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
