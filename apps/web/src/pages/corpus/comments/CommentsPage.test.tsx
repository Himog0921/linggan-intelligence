import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { calls, comment, DOMAIN, installFetch, json, plan, renderPage, WORK, writes } from './testKit';

afterEach(() => vi.unstubAllGlobals());
const commentReads = () => calls.filter(call => call.url.pathname.endsWith('/comment-study/comments'));
const page = (items: unknown[], extra: Record<string, unknown> = {}) => json({ domainRef: DOMAIN, items, summary: { displayableCommentCount: items.length, eligibleCommentCount: items.length }, indexCoverage: { indexedCount: items.length, pendingCount: 0 }, page: { nextCursor: null, ...extra } });

describe('B-04 用户评论', () => {
 it('renders literal source text, keeps eligibility server-owned and moves research records out of the sidebar', async () => {
  installFetch(); renderPage(`domain=${DOMAIN}&view=comments`);
  await screen.findByRole('table');
  const side = screen.getByRole('complementary', { name: '语料导航' });
  expect(within(side).getByRole('link', { name: '用户评论' })).toHaveAttribute('aria-current', 'page');
  expect(within(side).queryByRole('link', { name: '研究记录' })).toBeNull();
  expect(screen.getByRole('button', { name: /^研究记录/ })).toHaveAttribute('aria-haspopup', 'dialog');
  expect(screen.getByRole('navigation', { name: '当前位置' })).toHaveTextContent(/评论研究.*用户评论/);
  expect(screen.queryByRole('heading', { name: '用户评论', level: 2 })).toBeNull();
  expect(screen.getAllByRole('button', { name: 'SYNTHETIC 原声 <script>不执行</script>' })).toHaveLength(2);
  expect(document.querySelector('script')).toBeNull();
  expect(document.querySelector('select')).toBeNull();
  for (const name of ['搜索范围', '研究状态', '研究标签', '排序']) expect(screen.getByRole('combobox', { name })).toBeInTheDocument();
  expect(screen.getByRole('combobox', { name: '研究状态' })).toHaveTextContent('全部状态');
  expect(screen.getByTitle('点赞数未知：采集时没有取得这条评论的点赞')).toHaveTextContent('—');
  expect(screen.getAllByRole('columnheader')).toHaveLength(9);
  expect(screen.queryByRole('checkbox', { name: '选择这条评论' })).toBeNull();
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: '选择评论' }));
  expect(screen.getAllByRole('columnheader')).toHaveLength(10);
  const boxes = screen.getAllByRole('checkbox', { name: '选择这条评论' });
  expect(boxes[0]).toBeEnabled(); expect(boxes[1]).toBeDisabled();
  await user.click(boxes[0]!);
  const bar = screen.getByRole('region', { name: '已选评论' });
  expect(bar).toHaveTextContent('已选 1 条评论');
  await within(bar).findByText(/将提交/);
  await waitFor(() => expect(bar).toHaveTextContent('将提交 1 条 · 预计调用 1 次'));
  expect(writes()).toEqual([]);
 });

 it('exits selection by clearing it and announcing what was cleared', async () => {
  installFetch(); renderPage(`domain=${DOMAIN}&view=comments`);
  await screen.findByRole('table');
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: '选择评论' }));
  await user.click(screen.getAllByRole('checkbox', { name: '选择这条评论' })[0]!);
  await user.click(screen.getByRole('button', { name: '退出选择' }));
  expect(screen.queryByRole('region', { name: '已选评论' })).toBeNull();
  expect(screen.getAllByText('已退出选择，清空了 1 条已选评论').length).toBeGreaterThan(0);
  await user.click(screen.getByRole('button', { name: '选择评论' }));
  expect(screen.getAllByRole('checkbox', { name: '选择这条评论' })[0]).not.toBeChecked();
 });

 it('freezes the first 300 filtered rows, previews them and submits only after explicit confirmation', async () => {
  const planHash = 'f'.repeat(64);
  const filteredPages: string[] = [];
  installFetch(url => {
   if (url.pathname.endsWith('/comments') && url.searchParams.get('limit') === '100') {
    expect(url.searchParams.get('q')).toBe('样本');
    const cursor = url.searchParams.get('cursor') || 'first';
    filteredPages.push(cursor);
    const offset = cursor === 'first' ? 0 : Number(cursor);
    return json({ domainRef: DOMAIN, items: Array.from({ length: 100 }, (_, i) => comment(`batch-${offset + i}`, offset + i !== 100)),
     summary: { displayableCommentCount: 400, eligibleCommentCount: 399 }, indexCoverage: { indexedCount: 400, pendingCount: 0 }, page: { nextCursor: String(offset + 100) } });
   }
   if (url.pathname.endsWith('/v2/preview')) return json(plan({ planHash, selectedCount: 299, researchableCount: 280, unitCount: 14, expectedModelCalls: 14, alreadyResearchedCount: 19 }));
   if (url.pathname.endsWith('/v2/runs') && url.searchParams.size === 0) return json({ batchRef: 'batch-new', unitCount: 14, targetCount: 280 });
   return undefined;
  });
  const { router } = renderPage(`domain=${DOMAIN}&view=comments&q=${encodeURIComponent('样本')}`);
  await screen.findByRole('table');
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: '选择评论' }));
  await user.click(screen.getByRole('button', { name: '按当前筛选生成研究清单' }));
  await screen.findByText(/已检查当前筛选结果的前 300 条，选入 299 条。/);
  expect(filteredPages).toEqual(['first', '100', '200']);
  await user.click(screen.getByRole('button', { name: /预览研究计划/ }));
  const dialog = await screen.findByRole('dialog', { name: '本次研究预览' });
  await within(dialog).findByText('按所属作品分组');
  expect(dialog.querySelector('strong')).toHaveTextContent('将提交 280 条评论 · 预计调用 14 次');
  expect(within(dialog).getByText('不会提交 19 条')).toBeInTheDocument();
  expect(within(dialog).getByText('已有研究结果，本次跳过')).toBeInTheDocument();
  const previews = calls.filter(call => call.url.pathname.endsWith('/v2/preview'));
  expect(previews.at(-1)?.body?.selections).toHaveLength(299);
  expect(previews.at(-1)?.body?.selections).not.toContainEqual({ workRef: WORK, sourceRef: 'source-batch-100', observationRole: 'primary' });
  expect(writes()).toEqual([]);
  await user.click(within(dialog).getByRole('button', { name: '提交 280 条研究' }));
  await waitFor(() => expect(new URLSearchParams(router.state.location.search).get('runRef')).toBe('batch-new'));
  expect(await screen.findByRole('dialog', { name: '研究记录' })).toBeInTheDocument();
  const starts = writes();
  expect(starts).toHaveLength(1);
  expect(starts[0]?.body).toMatchObject({ expectedPlanHash: planHash, selections: previews.at(-1)?.body?.selections });
  expect(screen.queryByRole('region', { name: '已选评论' })).toBeNull();
 });

 it('does not expose a partial selection when a later directory page fails', async () => {
  installFetch(url => {
   if (!url.pathname.endsWith('/comments') || url.searchParams.get('limit') !== '100') return undefined;
   if (url.searchParams.has('cursor')) return json({ code: 'read_unavailable' }, 503);
   return json({ domainRef: DOMAIN, items: [comment()], summary: {}, indexCoverage: {}, page: { nextCursor: 'second' } });
  });
  renderPage(`domain=${DOMAIN}&view=comments`);
  await screen.findByRole('table');
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: '选择评论' }));
  await user.click(screen.getByRole('button', { name: '按当前筛选生成研究清单' }));
  expect(await screen.findByText('未能生成评论清单，请重试。')).toBeVisible();
  expect(screen.queryByRole('region', { name: '已选评论' })).toBeNull();
  expect(calls.every(call => call.method === 'GET')).toBe(true);
 });

 it('applies dropdown filters at once, keeps the search draft local until submitted, and applies the popover on 完成', async () => {
  installFetch(); const { router } = renderPage(`domain=${DOMAIN}&view=comments`); await screen.findByRole('table');
  const user = userEvent.setup();
  await user.type(screen.getByRole('searchbox', { name: '字面搜索' }), '两字');
  expect(commentReads()).toHaveLength(1);
  await user.click(screen.getByRole('combobox', { name: '研究标签' })); await user.click(screen.getByRole('option', { name: '需求' }));
  await waitFor(() => expect(commentReads().at(-1)?.url.searchParams.get('signalKind')).toBe('need'));
  expect(commentReads().at(-1)?.url.searchParams.get('q')).toBe('');
  await user.click(screen.getByRole('button', { name: '搜索' }));
  await waitFor(() => expect(commentReads().at(-1)?.url.searchParams.get('q')).toBe('两字'));
  await user.click(screen.getByRole('button', { name: /^筛选/ }));
  const pop = screen.getByRole('dialog', { name: '更多筛选' });
  await user.type(within(pop).getByRole('spinbutton', { name: '最低点赞' }), '100');
  await user.click(within(pop).getByRole('combobox', { name: '改帖标记' })); await user.click(screen.getByRole('option', { name: '已标记' }));
  expect(commentReads().at(-1)?.url.searchParams.get('minLikes')).toBeNull();
  await user.click(within(pop).getByRole('button', { name: '完成' }));
  await waitFor(() => expect(commentReads().at(-1)?.url.searchParams.get('minLikes')).toBe('100'));
  const call = commentReads().at(-1)!;
  expect(call.url.searchParams.get('postUsage')).toBe('used'); expect(call.url.searchParams.get('domain')).toBe(DOMAIN);
  expect(router.state.location.search).toContain('signalKind=need'); expect(router.state.location.search).toContain('minLikes=100');
  expect(screen.getByLabelText('2 项生效')).toHaveTextContent('2');
 });

 it('shows the promotion label in the comment directory', async () => {
  installFetch(url => url.pathname.endsWith('/comments') ? page([{ ...comment(), signalKinds: ['promotion'] }]) : undefined);
  renderPage(`domain=${DOMAIN}&view=comments`);
  const table = await screen.findByRole('table');
  expect(within(table).getByText('引流话术')).toBeVisible();
  await userEvent.click(screen.getByRole('combobox', { name: '研究标签' }));
  await userEvent.click(screen.getByRole('option', { name: '引流话术' }));
  await waitFor(() => expect(commentReads().at(-1)?.url.searchParams.get('signalKind')).toBe('promotion'));
 });

 it('pages by offset when the server reports totalCount and falls back to cursors when it does not', async () => {
  installFetch(url => {
   if (!url.pathname.endsWith('/comments')) return undefined;
   const offset = Number(url.searchParams.get('offset') || 0);
   return page(Array.from({ length: 10 }, (_, i) => comment(`n${offset + i}`)), { totalCount: 24, nextCursor: 'next-cursor', ...(url.searchParams.has('offset') ? { offset } : {}) });
  });
  const { router } = renderPage(`domain=${DOMAIN}&view=comments`); await screen.findByRole('table');
  expect(screen.getByText('第 1–10 条 · 共 24 条')).toBeInTheDocument();
  expect(commentReads()[0]?.url.searchParams.get('limit')).toBe('10');
  expect(commentReads()[0]?.url.searchParams.has('offset')).toBe(false);
  await userEvent.click(screen.getByRole('button', { name: '第 3 页' }));
  await waitFor(() => expect(commentReads().at(-1)?.url.searchParams.get('offset')).toBe('20'));
  expect(await screen.findByText('第 21–24 条 · 共 24 条')).toBeInTheDocument();
  expect(router.state.location.search).toContain('page=3');
  await userEvent.click(screen.getByRole('combobox', { name: '每页条数' })); await userEvent.click(screen.getByRole('option', { name: '20 条/页' }));
  await waitFor(() => expect(commentReads().at(-1)?.url.searchParams.get('limit')).toBe('20'));
  expect(commentReads().at(-1)?.url.searchParams.has('offset')).toBe(false);
 });

 it('keeps cursor paging when totalCount is absent', async () => {
  installFetch(url => url.pathname.endsWith('/comments') ? page([comment(url.searchParams.get('cursor') ? 'second' : 'first')], { nextCursor: url.searchParams.get('cursor') ? null : 'cursor-2' }) : undefined);
  renderPage(`domain=${DOMAIN}&view=comments`); await screen.findByRole('table');
  expect(screen.getByText('第 1 页')).toBeInTheDocument();
  expect(screen.queryByRole('spinbutton', { name: '跳转到页码' })).toBeNull();
  await userEvent.click(screen.getByRole('button', { name: '下一页' }));
  await waitFor(() => expect(commentReads().at(-1)?.url.searchParams.get('cursor')).toBe('cursor-2'));
  expect(commentReads().every(call => !call.url.searchParams.has('offset'))).toBe(true);
  expect(await screen.findByText('第 2 页')).toBeInTheDocument();
 });

 it('keeps comment-directory failures local', async () => {
  installFetch(url => url.pathname.endsWith('/comments') ? json({ error: 'unavailable' }, 503) : undefined);
  renderPage(`domain=${DOMAIN}&view=comments`);
  await screen.findByText(/评论目录读取失败/);
  expect(screen.getByRole('combobox', { name: '研究状态' })).toBeInTheDocument();
  expect(calls.every(call => call.method === 'GET')).toBe(true);
 });

 it('retains an uncertain post-usage request identity and never updates optimistically', async () => {
  let attempts = 0; installFetch((url, options) => {
   if (url.pathname.endsWith('/comments/post-usage')) { attempts++; if (attempts === 1) return Promise.reject(new TypeError('lost receipt')); const body = JSON.parse(String(options.body)); return json({ contract: 'comment-post-usage.v1', requestRef: body.requestRef, commentKey: body, outcome: 'replayed', postUsage: { state: 'used', markedAt: null } }); }
   if (url.pathname.endsWith('/comments') && attempts > 1) return page([{ ...comment(), postUsage: { state: 'used', markedAt: null } }]);
   return undefined;
  }); renderPage(`domain=${DOMAIN}&view=comments`); await screen.findByRole('table');
  await userEvent.click(screen.getAllByRole('button', { name: '添加改帖标记' })[0]!);
  expect(await screen.findByText(/保存未获确认/)).toHaveAttribute('role', 'status');
  expect(screen.getAllByRole('button', { name: '添加改帖标记' })[0]).toHaveAttribute('aria-pressed', 'false');
  await userEvent.click(screen.getAllByRole('button', { name: '添加改帖标记' })[0]!);
  expect(await screen.findByRole('button', { name: '取消改帖标记' })).toHaveTextContent('已标');
  expect(screen.getByText('已确认原请求回执，没有重复记录。')).toBeInTheDocument();
  const posted = writes(); expect(posted).toHaveLength(2); expect(posted[0]!.body?.requestRef).toBe(posted[1]!.body?.requestRef); expect(posted[0]!.body?.workRef).toBe(WORK);
 });
});
