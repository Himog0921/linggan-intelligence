import { readFileSync } from 'node:fs';
import path from 'node:path';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { createMemoryRouter, RouterProvider } from 'react-router';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { OpportunityStatusContext } from '../../app/opportunityStatusContext';
import { OpportunityPage } from './OpportunityPage';

const DOMAIN = '00000000-0000-4000-8000-000000000001';
const PROBLEM = 'f0312a02-48ed-7114-02aa-1822e831200f';
const original = JSON.parse(readFileSync(path.resolve(process.cwd(), '../../crates/intelligence/tests/opportunity_metrics_postgres/p1-card.snapshot.json'), 'utf8'));
const reply = (value: unknown, status = 200) => new Response(JSON.stringify(value), { status, headers: { 'Content-Type': 'application/json' } });

function fixture() {
  const card = structuredClone(original);
  card.problem.statement = '合成的用户问题';
  card.demand.groups[0].protocolVersion = 1;
  card.demand.groups[0].current.ratePer1k = 12;
  card.channel.groups[0].protocolVersion = 1;
  card.demand.sampleVoices[0].text = null;
  card.demand.sampleVoices[0].tombstone = true;
  return card;
}

function setup(search: string, custom?: (url: URL) => Response | Promise<Response> | undefined) {
  const card = fixture();
  const calls: URL[] = [];
  vi.stubGlobal('fetch', vi.fn(async (input: RequestInfo | URL) => {
    const url = new URL(String(input), 'http://localhost'); calls.push(url);
    const override = custom?.(url); if (override) return override;
    if (url.pathname === '/api/local/corpus/shell') return reply({ domains: [{ domainRef: DOMAIN, name: '合成领域', status: 'active', targetCount: 2, sampleCount: 3 }], selectedDomain: DOMAIN, collectionState: '观察中' });
    if (url.pathname.endsWith('/opportunities')) return reply({ domainRef: DOMAIN, items: [card], weeklyComparisons: [{ problemId: PROBLEM, groups: [{ protocolVersion: 1, currentPeriodStart: '2026-11-09', previousPeriodStart: '2026-11-02', currentRatePer1k: 12, previousRatePer1k: 10, status: 'rising', reason: null }] }], page: { nextCursor: null, hasMore: false, asOf: card.asOf } });
    if (url.pathname.endsWith('/card')) return reply(card);
    if (url.pathname.endsWith('/evidence')) return reply({ kind: url.searchParams.get('kind'), cardAsOf: card.asOf, items: card.demand.sampleVoices, excluded: {}, page: { nextCursor: null, hasMore: false, asOf: card.asOf } });
    throw new Error(`unexpected ${url.pathname}`);
  }));
  const client = new QueryClient({ defaultOptions: { queries: { retry: false, refetchOnWindowFocus: false } } });
  const router = createMemoryRouter([{ path: '/opportunity', element: <OpportunityPage /> }], { initialEntries: [`/opportunity?${search}`] });
  const view = render(<QueryClientProvider client={client}><OpportunityStatusContext.Provider value={{ state: 'enabled', retry: () => {} }}><RouterProvider router={router} /></OpportunityStatusContext.Provider></QueryClientProvider>);
  return { ...view, client, calls, router };
}

afterEach(() => vi.unstubAllGlobals());

describe('机会卡页面', () => {
  it('未指定领域时选择实际可用的 ADHD 领域并保留其他参数', async () => {
    const { router, calls } = setup(`problem=${PROBLEM}`, url => {
      if (url.pathname !== '/api/local/corpus/shell') return undefined;
      return reply({ domains: [{ domainRef: DOMAIN, name: 'ADHD', status: 'active', targetCount: 2, sampleCount: 3 }], selectedDomain: url.searchParams.get('domain'), collectionState: '观察中' });
    });
    await screen.findByRole('heading', { name: '合成的用户问题' });
    expect(router.state.location.search).toContain(`domain=${DOMAIN}`);
    expect(router.state.location.search).toContain(`problem=${PROBLEM}`);
    expect(calls.some(url => url.pathname.endsWith('/card') && url.pathname.includes(DOMAIN))).toBe(true);
  });
  it('保留明确指定的其他领域', async () => {
    const other = '00000000-0000-4000-8000-000000000002';
    const { router, calls } = setup(`domain=${other}`, url => {
      if (url.pathname === '/api/local/corpus/shell') return reply({ domains: [{ domainRef: DOMAIN, name: 'ADHD', status: 'active', targetCount: 2, sampleCount: 3 }, { domainRef: other, name: '其他领域', status: 'active', targetCount: 0, sampleCount: 0 }], selectedDomain: other, collectionState: '观察中' });
      if (url.pathname.endsWith('/opportunities')) return reply({ domainRef: other, items: [], weeklyComparisons: [], page: { nextCursor: null, hasMore: false, asOf: fixture().asOf } });
      return undefined;
    });
    expect(await screen.findByText('当前领域尚无可读取的机会卡。')).toBeVisible();
    expect(router.state.location.search).toBe(`?domain=${other}`);
    expect(calls.some(url => url.pathname.includes(`/domains/${other}/opportunities`))).toBe(true);
  });
  it('没有唯一的 ADHD 领域时要求选择，不猜测领域', async () => {
    const { router } = setup('', url => url.pathname === '/api/local/corpus/shell' ? reply({ domains: [{ domainRef: DOMAIN, name: '其他领域', status: 'active', targetCount: 0, sampleCount: 0 }], selectedDomain: null, collectionState: '状态未知' }) : undefined);
    expect(await screen.findByText('请先在页头选择观察领域。')).toBeVisible();
    expect(new URLSearchParams(router.state.location.search).has('domain')).toBe(false);
  });
  it('显示服务端返回的读模型过期提示', async () => {
    setup(`domain=${DOMAIN}&problem=${PROBLEM}`, url => {
      if (!url.pathname.endsWith('/card')) return undefined;
      const card = fixture(); card.flags.push('stale_read_model'); return reply(card);
    });
    expect(await screen.findByText('读模型可能过期')).toBeVisible();
  });
  it('开关关闭时直接访问说明未启用', () => {
    render(<OpportunityStatusContext.Provider value={{ state: 'disabled', retry: () => {} }}><OpportunityPage /></OpportunityStatusContext.Provider>);
    expect(screen.getByRole('heading', { name: '机会卡未启用' })).toBeVisible();
  });
  it('按服务端周对比展示列表，三个真实入口保留领域', async () => {
    const { calls } = setup(`domain=${DOMAIN}`);
    const table = await screen.findByRole('table', { name: '本周观察到上升的问题' });
    expect(within(table).getByRole('link', { name: '合成的用户问题' })).toHaveAttribute('href', `/opportunity?domain=${DOMAIN}&problem=${PROBLEM}`);
    expect(table).toHaveTextContent('采集协议 1'); expect(table).toHaveTextContent('上周 10');
    expect(within(table).queryByText('读模型可能过期')).not.toBeInTheDocument();
    expect(screen.getByRole('link', { name: /查看用户问题/ })).toHaveAttribute('href', `/corpus/comments?domain=${DOMAIN}&view=problems`);
    expect(screen.getByRole('link', { name: /查看作品依据/ })).toHaveAttribute('href', `/corpus/evidence?domain=${DOMAIN}`);
    expect(screen.getByRole('link', { name: /查看创作者/ })).toHaveAttribute('href', `/corpus/creators?domain=${DOMAIN}`);
    expect(screen.getByText(/提及人数反映讨论热度/)).toBeVisible();
    expect(calls.find(url => url.pathname.endsWith('/opportunities'))?.searchParams.get('includeWeeklyComparison')).toBe('true');
  });
  it('没有上升项时直接展示已有问题和不可比较原因', async () => {
    setup(`domain=${DOMAIN}`, url => {
      if (!url.pathname.endsWith('/opportunities')) return undefined;
      const card = fixture();
      return reply({ domainRef: DOMAIN, items: [card], weeklyComparisons: [{ problemId: PROBLEM, groups: [{ protocolVersion: 1, currentPeriodStart: '2026-11-09', previousPeriodStart: '2026-11-02', currentRatePer1k: null, previousRatePer1k: null, status: 'incomparable', reason: 'current_unavailable' }] }], page: { nextCursor: null, hasMore: false, asOf: card.asOf } });
    });
    expect(await screen.findByText('本页没有观察到上升的问题。')).toBeVisible();
    expect(screen.getByText('本页 1 个问题；同一问题按采集协议分别展示。')).toBeVisible();
    const table = screen.getByRole('table', { name: '本页其他观察与不可比较项' });
    expect(within(table).getByRole('link', { name: '合成的用户问题' })).toBeVisible();
    expect(table).toHaveTextContent('不可比较：本周提及率未知');
  });
  it('作者列表只预览前五项，展开与收起不改变组内顺序', async () => {
    setup(`domain=${DOMAIN}&problem=${PROBLEM}`, url => {
      if (!url.pathname.endsWith('/card')) return undefined;
      const card = fixture();
      const template = card.channel.groups[0].ranked[0];
      card.channel.groups[0].ranked = Array.from({ length: 7 }, (_, i) => ({ ...template, creatorId: `ranked-${i}`, name: `排序作者 ${i + 1}` }));
      card.channel.groups[0].insufficient = Array.from({ length: 7 }, (_, i) => ({ ...template, creatorId: `small-${i}`, name: `样本不足作者 ${i + 1}` }));
      return reply(card);
    });
    await screen.findByRole('heading', { name: '合成的用户问题' });
    expect(screen.getByText('排序作者 5')).toBeVisible();
    expect(screen.queryByText('排序作者 6')).not.toBeInTheDocument();
    expect(screen.getByText('样本不足作者 5')).toBeVisible();
    expect(screen.queryByText('样本不足作者 6')).not.toBeInTheDocument();
    const expand = screen.getAllByRole('button', { name: '查看全部 7 位作者' });
    if (!expand[0] || !expand[1]) throw new Error('missing creator group controls');
    await userEvent.click(expand[0]);
    expect(screen.getByText('排序作者 7')).toBeVisible();
    expect(screen.queryByText('样本不足作者 6')).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: '收起作者' }));
    expect(screen.queryByText('排序作者 7')).not.toBeInTheDocument();
    await userEvent.click(expand[1]);
    expect(screen.getByText('样本不足作者 7')).toBeVisible();
  });
  it('首页直接显示服务端返回的卡片过期提示', async () => {
    setup(`domain=${DOMAIN}`, url => {
      if (!url.pathname.endsWith('/opportunities')) return undefined;
      const card = fixture(); card.flags.push('stale_read_model');
      return reply({ domainRef: DOMAIN, items: [card], weeklyComparisons: [{ problemId: PROBLEM, groups: [{ protocolVersion: 1, currentPeriodStart: '2026-11-09', previousPeriodStart: '2026-11-02', currentRatePer1k: 12, previousRatePer1k: 10, status: 'rising', reason: null }] }], page: { nextCursor: null, hasMore: false, asOf: card.asOf } });
    });
    const table = await screen.findByRole('table', { name: '本周观察到上升的问题' });
    expect(within(table).getByText('读模型可能过期')).toBeVisible();
  });
  it('详情保留墓碑、未知作者和供给未评估，并按领域读取依据', async () => {
    const { calls } = setup(`domain=${DOMAIN}&problem=${PROBLEM}`);
    await screen.findByRole('heading', { name: '合成的用户问题' });
    expect(screen.getByText('来源已撤回：原文不再展示，保留引用记录')).toBeVisible();
    expect(screen.getByText('暂不判断')).toBeVisible();
    expect(screen.getAllByText('姓名未知').length).toBeGreaterThan(0);
    expect(screen.getAllByText('观察状态未知').length).toBeGreaterThan(0);
    expect(screen.getAllByText('样本不足，暂不排序').length).toBeGreaterThan(0);
    expect(screen.getByText('周期 2026-11-09 提及人数')).toBeVisible();
    expect(document.body).toHaveTextContent('提及 4 人 · 有效评论 60 条');
    expect(screen.getAllByRole('img', { name: /提及率每千/ }).length).toBeGreaterThan(0);
    await userEvent.click(screen.getByRole('button', { name: '评论依据' }));
    expect(await screen.findByText(/依据读取于/)).toBeVisible();
    expect(calls.some(url => url.pathname === `/api/local/opportunity/domains/${DOMAIN}/problems/${PROBLEM}/evidence`)).toBe(true);
  });
  it('来源刷新失败时隐藏旧正文，避免继续展示撤回来源', async () => {
    let failed = false;
    const { client } = setup(`domain=${DOMAIN}&problem=${PROBLEM}`, url => url.pathname.endsWith('/card') && failed ? reply({ code: 'opportunity_card_source_unavailable' }, 503) : undefined);
    await screen.findByText('SYNTHETIC 原声 c-work-0-2');
    failed = true;
    await act(async () => { await client.invalidateQueries({ queryKey: ['opportunity', 'card', DOMAIN, PROBLEM] }); });
    expect(await screen.findByRole('alert')).toHaveTextContent('部分来源已不可用');
    await waitFor(() => expect(screen.queryByText('SYNTHETIC 原声 c-work-0-2')).not.toBeInTheDocument());
  });
  it('卡片重读尚未完成时不继续显示旧原声', async () => {
    let slow = false; let finish: (value: Response) => void = () => {};
    const pending = new Promise<Response>(resolve => { finish = resolve; });
    const { client } = setup(`domain=${DOMAIN}&problem=${PROBLEM}`, url => url.pathname.endsWith('/card') && slow ? pending : undefined);
    await screen.findByText('SYNTHETIC 原声 c-work-0-2');
    slow = true; let refresh: Promise<void> | undefined;
    await act(async () => { refresh = client.invalidateQueries({ queryKey: ['opportunity', 'card', DOMAIN, PROBLEM] }); });
    await waitFor(() => expect(screen.queryByText('SYNTHETIC 原声 c-work-0-2')).not.toBeInTheDocument());
    expect(screen.getByRole('status')).toHaveTextContent('正在读取机会卡');
    await act(async () => { finish(reply(fixture())); await refresh; });
    expect(await screen.findByText('SYNTHETIC 原声 c-work-0-2')).toBeVisible();
  });
  it('评论依据重读尚未完成时不继续显示旧原声', async () => {
    let slow = false; let finish: (value: Response) => void = () => {};
    const pending = new Promise<Response>(resolve => { finish = resolve; });
    const { client } = setup(`domain=${DOMAIN}&problem=${PROBLEM}`, url => url.pathname.endsWith('/evidence') && slow ? pending : undefined);
    await screen.findByRole('heading', { name: '合成的用户问题' });
    await userEvent.click(screen.getByRole('button', { name: '评论依据' }));
    const panel = screen.getByRole('heading', { name: '判断与依据' }).closest('section');
    if (!panel) throw new Error('evidence panel missing');
    expect(await within(panel).findByText('SYNTHETIC 原声 c-work-0-2')).toBeVisible();
    slow = true; let refresh: Promise<void> | undefined;
    await act(async () => { refresh = client.invalidateQueries({ queryKey: ['opportunity', 'evidence', DOMAIN, PROBLEM] }); });
    await waitFor(() => expect(within(panel).queryByText('SYNTHETIC 原声 c-work-0-2')).not.toBeInTheDocument());
    expect(screen.queryByText('SYNTHETIC 原声 c-work-0-2')).not.toBeInTheDocument();
    expect(within(panel).getByRole('status')).toHaveTextContent('正在读取依据');
    const card = fixture();
    await act(async () => { finish(reply({ kind: 'comments', cardAsOf: card.asOf, items: card.demand.sampleVoices, excluded: {}, page: { nextCursor: null, hasMore: false, asOf: card.asOf } })); await refresh; });
    expect(await within(panel).findByText('SYNTHETIC 原声 c-work-0-2')).toBeVisible();
  });
  it('首次读取依据尚未返回时隐藏卡片原声', async () => {
    let finish: (value: Response) => void = () => {};
    const pending = new Promise<Response>(resolve => { finish = resolve; });
    setup(`domain=${DOMAIN}&problem=${PROBLEM}`, url => url.pathname.endsWith('/evidence') ? pending : undefined);
    await screen.findByText('SYNTHETIC 原声 c-work-0-2');
    await userEvent.click(screen.getByRole('button', { name: '评论依据' }));
    expect(screen.queryByText('SYNTHETIC 原声 c-work-0-2')).not.toBeInTheDocument();
    expect(screen.getByText('正在核对来源，原声暂不展示。')).toBeVisible();
    await act(async () => { finish(reply({ code: 'opportunity_card_source_unavailable' }, 503)); });
    expect(await screen.findByRole('alert')).toHaveTextContent('依据未能确认，已隐藏原声');
  });
  it('依据读取失败时也隐藏卡片原声，成功重读卡片后恢复', async () => {
    let failure = true;
    setup(`domain=${DOMAIN}&problem=${PROBLEM}`, url => url.pathname.endsWith('/evidence') && failure ? reply({ code: 'opportunity_card_source_unavailable' }, 503) : undefined);
    await screen.findByText('SYNTHETIC 原声 c-work-0-2');
    await userEvent.click(screen.getByRole('button', { name: '评论依据' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('依据未能确认，已隐藏原声');
    expect(screen.queryByText('SYNTHETIC 原声 c-work-0-2')).not.toBeInTheDocument();
    failure = false;
    await userEvent.click(screen.getByRole('button', { name: '重新读取卡片' }));
    expect(await screen.findByText('SYNTHETIC 原声 c-work-0-2')).toBeVisible();
  });
  it('卡片版本发生变化时不混合展示不同版本的依据', async () => {
    setup(`domain=${DOMAIN}&problem=${PROBLEM}`, url => url.pathname.endsWith('/evidence') ? reply({ kind: 'comments', cardAsOf: '2026-11-11T00:00:00Z', items: [], excluded: {}, page: { nextCursor: null, hasMore: false, asOf: '2026-11-11T00:00:00Z' } }) : undefined);
    await screen.findByRole('heading', { name: '合成的用户问题' });
    await userEvent.click(screen.getByRole('button', { name: '评论依据' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('卡片已更新');
    expect(screen.queryByText('SYNTHETIC 原声 c-work-0-2')).not.toBeInTheDocument();
  });
  it('卡片和依据版本交错时可重新读取并恢复', async () => {
    let reads = 0; let slowCard = false; let finishCard: (value: Response) => void = () => {}; const card = fixture();
    const pending = new Promise<Response>(resolve => { finishCard = resolve; });
    setup(`domain=${DOMAIN}&problem=${PROBLEM}`, url => {
      if (url.pathname.endsWith('/card') && slowCard) return pending;
      if (!url.pathname.endsWith('/evidence')) return undefined;
      reads += 1;
      const asOf = reads === 1 ? '2026-11-11T00:00:00Z' : card.asOf;
      return reply({ kind: 'comments', cardAsOf: asOf, items: [], excluded: {}, page: { nextCursor: null, hasMore: false, asOf } });
    });
    await screen.findByRole('heading', { name: '合成的用户问题' });
    await userEvent.click(screen.getByRole('button', { name: '评论依据' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('卡片已更新');
    slowCard = true;
    await userEvent.click(screen.getByRole('button', { name: '重新读取卡片' }));
    expect(screen.queryByText('SYNTHETIC 原声 c-work-0-2')).not.toBeInTheDocument();
    await act(async () => { finishCard(reply(card)); });
    await screen.findByRole('heading', { name: '合成的用户问题' });
    await userEvent.click(screen.getByRole('button', { name: '评论依据' }));
    expect(await screen.findByText(/依据读取于/)).toBeVisible();
    expect(reads).toBe(2);
  });
});
