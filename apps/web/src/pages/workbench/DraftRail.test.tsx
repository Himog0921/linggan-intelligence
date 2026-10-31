import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { describe, expect, it, vi } from 'vitest';
import { DraftRail } from './DraftRail';
import { checkSavedDrafts } from './draftChecks';
import type { WorkSummary } from './workbenchModel';

vi.mock('./draftChecks', () => ({ checkSavedDrafts: vi.fn() }));
const base: WorkSummary = { workId: 'a', domainRef: 'domain', title: '第一篇', revisionId: 'r1', savedAt: '2026-10-29T00:00:00Z' };
describe('draft rail', () => {
  it('groups checked saved revisions without confusing copies with publication', async () => {
    const onOpen = vi.fn();
    vi.mocked(checkSavedDrafts).mockResolvedValue({
      a: { revisionId: 'r1', must: 1, warn: 2, notRun: 0 },
      b: { revisionId: 'r1', must: 0, warn: 1, notRun: 0 },
      c: { revisionId: 'old', must: 0, warn: 0, notRun: 0 },
    });
    render(<QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}><DraftRail currentId="a" status="ready" onOpen={onOpen} drafts={[
      { ...base, copiedRevisionId: null },
      { ...base, workId: 'b', title: '第二篇', copiedRevisionId: 'r1' },
      { ...base, workId: 'c', title: '第三篇', revisionId: 'r2', copiedRevisionId: 'r1' },
      { ...base, workId: 'd', title: '第四篇' },
      { ...base, workId: 'e', title: '第五篇', publishedRevisionId: 'old' },
      { ...base, workId: 'f', title: '第六篇', publicationUnavailable: true },
    ]} /></QueryClientProvider>);
    expect(await within(await screen.findByRole('region', { name: '必须先改' })).findByText('第一篇')).toBeVisible();
    expect(within(screen.getByRole('region', { name: '已复制' })).getByText(/已复制不代表已发布/)).toBeVisible();
    expect(within(screen.getByRole('region', { name: '状态未核实' })).getByText('第三篇')).toBeVisible();
    expect(within(screen.getByRole('region', { name: '状态未核实' })).getByText('第四篇')).toBeVisible();
    expect(within(screen.getByRole('region', { name: '已登记发布' })).getByText(/当前稿件已修改/)).toBeVisible();
    expect(within(screen.getByRole('region', { name: '状态未核实' })).getByText('关联的发布记录当前不可用')).toBeVisible();
    fireEvent.click(screen.getByRole('button', { name: /第二篇/ }));
    expect(onOpen).toHaveBeenCalledWith('b');
  });
  it('rechecks only the batch containing a changed saved revision', async () => {
    vi.mocked(checkSavedDrafts).mockReset().mockImplementation(async drafts => Object.fromEntries(drafts.map(d => [d.workId, { revisionId: d.revisionId, must: 0, warn: 0, notRun: 0 }])));
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    const items = Array.from({ length: 26 }, (_, index) => ({ ...base, workId: String(index).padStart(2, '0'), title: `稿件 ${index}` }));
    const renderRail = (items: WorkSummary[]) => <QueryClientProvider client={client}><DraftRail currentId="a" status="ready" onOpen={() => {}} drafts={items} /></QueryClientProvider>;
    const page = render(renderRail(items));
    await waitFor(() => expect(checkSavedDrafts).toHaveBeenCalledTimes(2));
    await screen.findByRole('region', { name: '已检查' });
    page.rerender(renderRail(items.map((item, index) => index === 0 ? { ...item, revisionId: 'r2' } : item)));
    await waitFor(() => expect(checkSavedDrafts).toHaveBeenCalledTimes(3));
    expect(within(screen.getByRole('region', { name: '已检查' })).getByText('稿件 25')).toBeVisible();
    expect(vi.mocked(checkSavedDrafts).mock.calls[2]?.[0]).toHaveLength(25);
  });
  it('does not present a result started before the window regained focus', async () => {
    let resolveOld!: (value: Awaited<ReturnType<typeof checkSavedDrafts>>) => void;
    vi.mocked(checkSavedDrafts).mockReset()
      .mockImplementationOnce(() => new Promise(resolve => { resolveOld = resolve; }))
      .mockResolvedValueOnce({ a: { revisionId: 'r1', must: 1, warn: 0, notRun: 0 } });
    const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(<QueryClientProvider client={client}><DraftRail currentId="a" status="ready" onOpen={() => {}} drafts={[base]} /></QueryClientProvider>);
    await waitFor(() => expect(checkSavedDrafts).toHaveBeenCalledTimes(1));
    fireEvent.focus(window);
    await waitFor(() => expect(checkSavedDrafts).toHaveBeenCalledTimes(2));
    await screen.findByRole('region', { name: '必须先改' });
    await act(async () => resolveOld({ a: { revisionId: 'r1', must: 0, warn: 0, notRun: 0 } }));
    expect(screen.queryByRole('region', { name: '已检查' })).not.toBeInTheDocument();
  });
});
