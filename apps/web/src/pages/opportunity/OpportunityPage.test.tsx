import { readFileSync } from 'node:fs';
import path from 'node:path';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, render, screen, within } from '@testing-library/react';
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

function setup(search: string, custom?: (url: URL) => Response | undefined) {
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
  return { ...view, client, calls };
}

afterEach(() => vi.unstubAllGlobals());

describe('机会卡页面', () => {
  it('按服务端周对比展示列表，三个真实入口保留领域', async () => {
    const { calls } = setup(`domain=${DOMAIN}`);
    const table = await screen.findByRole('table', { name: '本周观察到上升的问题' });
    expect(within(table).getByRole('link', { name: '合成的用户问题' })).toHaveAttribute('href', `/opportunity?domain=${DOMAIN}&problem=${PROBLEM}`);
    expect(table).toHaveTextContent('采集协议 1'); expect(table).toHaveTextContent('上周 10');
    expect(screen.getByRole('link', { name: /查看用户问题/ })).toHaveAttribute('href', `/corpus/comments?domain=${DOMAIN}&view=problems`);
    expect(screen.getByRole('link', { name: /查看作品依据/ })).toHaveAttribute('href', `/corpus/evidence?domain=${DOMAIN}`);
    expect(screen.getByRole('link', { name: /查看创作者/ })).toHaveAttribute('href', `/corpus/creators?domain=${DOMAIN}`);
    expect(screen.getByText(/提及人数反映讨论热度/)).toBeVisible();
    expect(calls.find(url => url.pathname.endsWith('/opportunities'))?.searchParams.get('includeWeeklyComparison')).toBe('true');
  });
  it('详情保留墓碑、未知作者和供给未评估，并按领域读取依据', async () => {
    const { calls } = setup(`domain=${DOMAIN}&problem=${PROBLEM}`);
    await screen.findByRole('heading', { name: '合成的用户问题' });
    expect(screen.getByText('来源已撤回：原文不再展示，保留引用记录')).toBeVisible();
    expect(screen.getByText('暂不判断')).toBeVisible();
    expect(screen.getAllByText('姓名未知').length).toBeGreaterThan(0);
    expect(screen.getAllByText('观察状态未知').length).toBeGreaterThan(0);
    expect(screen.getAllByText('样本不足，暂不排序').length).toBeGreaterThan(0);
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
    expect(screen.queryByText('SYNTHETIC 原声 c-work-0-2')).not.toBeInTheDocument();
  });
  it('卡片版本发生变化时不混合展示不同版本的依据', async () => {
    setup(`domain=${DOMAIN}&problem=${PROBLEM}`, url => url.pathname.endsWith('/evidence') ? reply({ kind: 'comments', cardAsOf: '2026-11-11T00:00:00Z', items: [], excluded: {}, page: { nextCursor: null, hasMore: false, asOf: '2026-11-11T00:00:00Z' } }) : undefined);
    await screen.findByRole('heading', { name: '合成的用户问题' });
    await userEvent.click(screen.getByRole('button', { name: '评论依据' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('卡片已更新');
  });
});
