import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter, Route, Routes, useLocation } from 'react-router';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { OpsPage } from './OpsPage';
vi.mock('./OverviewPage', () => ({ OverviewPage: () => <p>概览内容</p> }));
const asOf = { exportedAt: '2026-10-19T02:00:00Z', exportedAtBeijing: '2026-10-19 10:00', batchRef: 'batch' };
const publication = {
  ref: 'pub', source: 'imported', businessDate: '2026-10-18', dateState: 'as_recorded', platform: 'xhs', title: '测试发布标题', body: '完整正文\n保留第二段',
  url: 'https://example.com/post', account: { name: '合成账号', ledgerState: 'in_ledger' }, publisher: '甲',
  views: { state: 'pending', value: null, overdue: false, hit: false }, likes: null, collects: 0, comments: null,
  coverClickRate: null, systemCheck: 'not_checked', sync: { requestRef: null, decision: null, state: 'not_requested' }, duplicates: [], cover: { available: true },
};
let failPublications = false;
let noAccounts = false;
let nativeOnly = false;
let persistedSync = false;
const requests: URL[] = [];
const requestBodies: unknown[] = [];
beforeEach(() => {
  requests.length = 0; requestBodies.length = 0; failPublications = false; noAccounts = false; nativeOnly = false; persistedSync = false;
  vi.stubGlobal('fetch', vi.fn(async (path: string, options?: RequestInit) => {
    const url = new URL(path, 'http://localhost'); requests.push(url);
    if (options?.body) requestBodies.push(JSON.parse(String(options.body)));
    const endpoint = url.pathname.split('/').at(-1);
    if (endpoint === 'publication-syncs') return new Response(JSON.stringify({
      requestRef: 'sync-request', selectedCount: 1, replayed: false, items: [{
        publicationRef: 'pub', decision: 'accepted', code: 'accepted', acquisitionRequestRef: 'request', workOrderRef: 'work-order',
        execution: { state: 'queued', taskId: null, attemptId: null, packageRef: null, receiptRef: null, receiptEffect: null, materialRef: null },
      }],
    }), { status: 201, headers: { 'Content-Type': 'application/json' } });
    if (url.pathname.includes('/publication-syncs/')) return new Response(JSON.stringify({
      requestRef: 'sync-request', selectedCount: 1, replayed: false, items: [{
        publicationRef: 'pub', decision: 'accepted', code: 'accepted', acquisitionRequestRef: 'request', workOrderRef: 'work-order',
        execution: { state: 'material_projected', taskId: 'task', attemptId: 'attempt', packageRef: 'package', receiptRef: 'receipt', receiptEffect: 'COMPLETED_LIVE_STEP', materialRef: 'material' },
      }],
    }), { status: 200, headers: { 'Content-Type': 'application/json' } });
    const result = endpoint === 'businesses' ? [{ domainRef: 'd1', name: '合成业务', accountCount: 1 }] : endpoint === 'summary' ? {
      asOf: nativeOnly ? null : asOf, nativeDataPresent: nativeOnly, month: '2026-10', activeAccounts: 1, ledgerAccounts: 1, monthPublished: 1,
      effectivePublication: { state: 'not_connected' }, monthFollowersGained: { value: null, scope: 'all_businesses' }, monthHits: null,
    } : endpoint === 'accounts' ? { asOf: noAccounts ? null : asOf, total: 0, active: 0, operators: [], items: [] }
    : { asOf, page: Number(url.searchParams.get('page') || 1), pageSize: 50, total: 100, totalPages: 2, operators: ['甲'], items: [persistedSync ? { ...publication, sync: { requestRef: 'sync-request', decision: 'accepted', state: 'queued' } } : publication] };
    return new Response(JSON.stringify(failPublications && endpoint === 'publications' ? { error: 'unavailable' } : result), {
      status: failPublications && endpoint === 'publications' ? 503 : 200, headers: { 'Content-Type': 'application/json' },
    });
  }));
});
afterEach(() => vi.unstubAllGlobals());
function Location() { const location = useLocation(); return <output aria-label="当前位置">{location.pathname}{location.search}</output>; }
function show(path = '/ops/publications?domain=d1') {
  return render(<QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
    <MemoryRouter initialEntries={[path]}><Routes><Route path="/ops/:page" element={<OpsPage/>}/></Routes><Location/></MemoryRouter>
  </QueryClientProvider>);
}
it('uses two secondary destinations and data tabs in Z02, preserving selected business', async () => {
  show();
  await screen.findByRole('button', { name: '查看发布记录：测试发布标题' });
  const side = screen.getByLabelText('运营导航');
  expect(within(side).getAllByRole('link').map(link => link.textContent)).toEqual(['数据', '创作']);
  expect(within(side).getByRole('link', { name: '创作' })).toHaveAttribute('href', '/ops/creation?domain=d1');
  const nav = screen.getByRole('navigation', { name: '数据页面' });
  expect(nav.closest('.v7-context-main')).not.toBeNull();
  expect(within(nav).getAllByRole('link').map(link => link.textContent)).toEqual(['概览','发布记录','账号管理','每日数据']);
  expect(screen.queryByText('导入导出')).not.toBeInTheDocument();
  expect(within(nav).getByRole('link', { name: '账号管理' })).toHaveAttribute('href', '/ops/accounts?domain=d1');
});
it('retires the IO screen and redirects its old deep link without executing export/import', async () => {
  show('/ops/io?domain=d1');
  await screen.findByText('概览内容');
  expect(screen.getByLabelText('当前位置')).toHaveTextContent('/ops/overview?domain=d1');
  expect(requests.every(url => !/export|import/.test(url.pathname))).toBe(true);
});
it('commits FIND manually, protects IME and keeps filters while resetting pagination', async () => {
  show('/ops/publications?domain=d1&platform=xhs&operator=甲&page=2');
  const input = await screen.findByRole('searchbox', { name: '搜索账号' });
  fireEvent.compositionStart(input); fireEvent.change(input, { target: { value: '中文账号' } });
  fireEvent.submit(screen.getByRole('search', { name: '搜索账号' }));
  expect(screen.getByLabelText('当前位置')).not.toHaveTextContent('account=');
  fireEvent.compositionEnd(input);
  fireEvent.submit(screen.getByRole('search', { name: '搜索账号' }));
  await waitFor(() => expect(new URLSearchParams(screen.getByLabelText('当前位置').textContent?.split('?')[1]).get('account')).toBe('中文账号'));
  expect(screen.getByLabelText('当前位置')).not.toHaveTextContent('page=');
  expect(screen.getByLabelText('当前位置')).toHaveTextContent('platform=xhs');
  fireEvent.click(screen.getByRole('button', { name: '清空搜索' }));
  expect(screen.getByLabelText('当前位置')).not.toHaveTextContent('account=');
});
it('opens full publication content, preserves unknown vs zero, and restores focus after nested media failure', async () => {
  show();
  const opener = await screen.findByRole('button', { name: '查看发布记录：测试发布标题' }); opener.focus(); fireEvent.click(opener);
  const detail = screen.getByRole('dialog', { name: '发布记录详情' });
  expect(within(detail).getByText(/保留第二段/)).toBeInTheDocument();
  expect(within(detail).getByText('待补')).toBeInTheDocument();
  const facts = within(detail).getByText('点赞').parentElement;
  expect(facts).toHaveTextContent('—');
  fireEvent.click(within(detail).getByRole('button', { name: '查看封面大图' }));
  const media = screen.getByRole('dialog', { name: '封面大图' });
  fireEvent.error(within(media).getByRole('img', { name: '封面大图' }));
  expect(within(media).getByText('封面暂时无法读取，原记录仍保留。')).toBeInTheDocument();
  fireEvent.click(within(media).getByRole('button', { name: '关闭' }));
  expect(screen.getByRole('dialog', { name: '发布记录详情' })).toBeInTheDocument();
  fireEvent.click(within(detail).getByRole('button', { name: '关闭详情' })); expect(opener).toHaveFocus();
});
it('keeps scope picker in the breadcrumb and clears filters when switching business', async () => {
  show('/ops/publications?domain=d1&platform=xhs&page=2');
  const picker = await screen.findByLabelText('切换业务');
  await waitFor(() => expect(picker).not.toHaveAttribute('aria-disabled'));
  expect(picker.closest('.v7-context-crumb')).not.toBeNull();
  fireEvent.click(picker); fireEvent.click(screen.getByRole('button', { name: '全部业务' }));
  expect(screen.getByLabelText('当前位置')).toHaveTextContent('/ops/publications?domain=all');
  expect(screen.getByLabelText('当前位置')).not.toHaveTextContent('platform=');
});
it('does not turn an unavailable publication query into an empty successful list', async () => {
  failPublications = true; show();
  await screen.findByText(/发布记录.*读取失败，可以重试/);
  expect(screen.queryByText('没有发布记录。')).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '重试' }));
  await waitFor(() => expect(requests.filter(url => url.pathname.endsWith('/publications')).length).toBe(2));
});
it('retains no-import as a distinct state from an empty imported account list', async () => {
  noAccounts = true; show('/ops/accounts?domain=d1');
  await screen.findByText('还没有迁入数据。');
  expect(screen.queryByText('当前筛选下没有账号。')).not.toBeInTheDocument();
});
it('does not label native-only Operations facts as an absent import', async () => {
  nativeOnly = true; show();
  await screen.findByRole('button', { name: '查看发布记录：测试发布标题' });
  expect(screen.getByText('原生台账数据（历史尚未迁入）')).toBeInTheDocument();
});

it('submits only explicitly selected publications and refreshes shared execution facts', async () => {
  show();
  await screen.findByRole('button', { name: '查看发布记录：测试发布标题' });
  fireEvent.click(screen.getByRole('button', { name: '同步数据' }));
  const dialog = screen.getByRole('dialog', { name: '同步已登记发布' });
  expect(within(dialog).getByText('只为选中的精确发布申请采集；不会自动发现、不会改写台账字段。')).toBeInTheDocument();
  expect(within(dialog).getByRole('button', { name: '提交' })).toBeDisabled();
  fireEvent.click(within(dialog).getByRole('checkbox'));
  fireEvent.click(within(dialog).getByRole('button', { name: '提交 1 条' }));
  await within(dialog).findByText('等待分派');
  expect(requestBodies).toContainEqual(expect.objectContaining({ publicationRefs: ['pub'] }));
  fireEvent.click(within(dialog).getByRole('button', { name: '刷新执行状态' }));
  await within(dialog).findByText('已关联材料（字段尚未采用）');
  expect(requests.some(url => url.pathname === '/api/local/ops/publication-syncs/sync-request')).toBe(true);
});

it('keeps a persisted sync status on the row and can reopen its request after returning', async () => {
  persistedSync = true; show();
  await screen.findByRole('button', { name: '等待分派' });
  fireEvent.click(screen.getByRole('button', { name: '等待分派' }));
  const dialog = screen.getByRole('dialog', { name: '同步已登记发布' });
  await within(dialog).findByText('已关联材料（字段尚未采用）');
  expect(requests.some(url => url.pathname === '/api/local/ops/publication-syncs/sync-request')).toBe(true);
});

it('protects unapplied dates and restores the draft after cancelling a close', async () => {
  show(); await screen.findByRole('button', { name: '查看发布记录：测试发布标题' });
  fireEvent.click(screen.getByRole('button', { name: '日期范围' }));
  const dialog = screen.getByRole('dialog', { name: '日期范围' });
  fireEvent.change(within(dialog).getByLabelText('开始日期'), { target: { value: '2026-10-01' } });
  fireEvent.click(within(dialog).getByRole('button', { name: '取消' }));
  const protection = screen.getByRole('dialog', { name: '放弃未应用的修改？' });
  fireEvent.click(within(protection).getByRole('button', { name: '继续编辑' }));
  expect(within(dialog).getByLabelText('开始日期')).toHaveValue('2026-10-01');
  fireEvent.click(within(dialog).getByRole('button', { name: '取消' }));
  fireEvent.click(within(screen.getByRole('dialog', { name: '放弃未应用的修改？' })).getByRole('button', { name: '放弃修改' }));
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  expect(screen.getByLabelText('当前位置')).not.toHaveTextContent('from=');
});
