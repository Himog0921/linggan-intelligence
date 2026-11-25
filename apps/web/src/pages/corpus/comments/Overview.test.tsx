import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { calls, DOMAIN, installFetch, json, overview, renderPage, WORK } from './testKit';
afterEach(() => vi.unstubAllGlobals());

const withNewFields = () => ({ ...overview(),
  researchBreakdown: { withResult: 60, running: 5, failedWithoutResult: 5, never: 30 },
  recentSignals: [{ requestRef: 'request-a', proposition: '家长需要反复提醒才能开始', kind: 'scenario', authorDisplayName: '林林妈妈', acceptedAt: '2026-10-28T01:00:00Z', commentKey: { workRef: WORK, commentExternalId: 'c1' } }] });

describe('B-04 评论研究概览', () => {
  it('keeps source counts, the observation chart and word cloud when research was reset', async () => {
    installFetch(); renderPage(`domain=${DOMAIN}&view=overview`);
    const metrics = await screen.findByRole('group', { name: '研究累计读数与昨日活动' });
    expect(metrics).toHaveTextContent('100'); expect(metrics).toHaveTextContent('已有 200 条原始评论 · 0 条待索引');
    expect(within(metrics).getByText('昨日首次观察')).toBeInTheDocument();
    expect(screen.getByRole('region', { name: '评论观察趋势' })).toBeVisible();
    expect(screen.getByRole('button', { name: '有数据以来' })).toHaveAttribute('aria-pressed', 'true');
    await screen.findAllByText('作业拖拉', { exact: false });
    expect(screen.getByRole('heading', { name: /已有依据的用户问题/ })).toBeVisible();
    expect(screen.getByRole('heading', { name: /近期研究信号/ })).toBeVisible();
    expect(screen.getByText('当前尚无正式问题。至少两位独立作者的有效原声支持才会形成正式问题。')).toBeVisible();
    expect(calls.every(call => call.method === 'GET')).toBe(true);
  });
  it('splits 已进入研究 into four parts when researchBreakdown exists and links recent signals to the source comment', async () => {
    installFetch(url => url.pathname.endsWith('/comment-research/v2/overview') ? json(withNewFields()) : undefined);
    const { router } = renderPage(`domain=${DOMAIN}&view=overview`);
    const bar = await screen.findByRole('img', { name: /^可研究评论 100 条：有结果 60 条，研究中 5 条，最近失败 5 条，未研究 30 条$/ });
    expect(bar.children).toHaveLength(4);
    expect(screen.getByText('60 条已有结果')).toBeInTheDocument(); expect(screen.getByText('结果覆盖 60%')).toBeInTheDocument();
    expect(screen.getByText('困难场景 · 林林妈妈')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: '查看林林妈妈的原声' }));
    await waitFor(() => expect(router.state.location.search).toContain('detail=comment'));
    expect(router.state.location.search).toContain('researchRequestRef=request-a');
  });
  it('falls back to a single segment and an empty recent-signal block when the new fields are absent', async () => {
    installFetch(); renderPage(`domain=${DOMAIN}&view=overview`);
    const bar = await screen.findByRole('img', { name: /^可研究评论 100 条：已有结果 0 条，可研究 100 条$/ });
    expect(bar.children).toHaveLength(1);
    expect(screen.getByText('近期研究信号暂不可读取。')).toBeInTheDocument();
  });
  it('preserves the selected period and chart view while navigating the three views', async () => {
    installFetch(); renderPage(`domain=${DOMAIN}&view=overview`);
    await screen.findByRole('group', { name: '研究累计读数与昨日活动' });
    await userEvent.click(screen.getByRole('button', { name: '近 7 天' }));
    await userEvent.click(screen.getByRole('button', { name: '表格' }));
    expect(within(screen.getByRole('region', { name: '评论观察趋势' })).getAllByRole('row')).toHaveLength(8);
    await userEvent.click(screen.getByRole('link', { name: '用户评论' })); await screen.findByRole('searchbox', { name: '字面搜索' });
    await userEvent.click(screen.getByRole('link', { name: '概览' }));
    await waitFor(() => expect(screen.getByRole('button', { name: '近 7 天' })).toHaveAttribute('aria-pressed', 'true'));
    expect(screen.getByRole('button', { name: '表格' })).toHaveAttribute('aria-pressed', 'true');
  });
  it('shows unindexed material as pending instead of implying there are zero researchable comments', async () => {
    const value = overview(); value.indexCoverage.pendingCount = 30; value.corpusSummary.eligibleCommentCount = 0;
    installFetch(url => url.pathname.endsWith('/comment-research/v2/overview') ? json(value) : undefined); renderPage(`domain=${DOMAIN}&view=overview`);
    const metrics = await screen.findByRole('group', { name: '研究累计读数与昨日活动' });
    expect(metrics).toHaveTextContent('已有 200 条原始评论 · 30 条待索引');
    expect(metrics.querySelector('[class*="_metric-number_"] span')).toHaveTextContent('—');
    expect(metrics).toHaveTextContent('结果覆盖 —');
  });
  it('keeps word-frequency reads available when overview fails and rejects a different domain', async () => {
    installFetch(url => url.pathname.endsWith('/comment-research/v2/overview') ? json({ ...overview(), domainRef: 'wrong-domain' }) : undefined); renderPage(`domain=${DOMAIN}&view=overview`);
    await screen.findByRole('alert'); await screen.findAllByText('作业拖拉', { exact: false });
    expect(screen.queryByRole('group', { name: '研究累计读数与昨日活动' })).not.toBeInTheDocument();
    expect(screen.getByRole('link', { name: '用户评论' })).toBeVisible();
  });
  it('hides previously displayed source-derived text when a refresh fails', async () => {
    let failed = false;
    installFetch(url => url.pathname.endsWith('/comment-research/v2/overview') ? (failed ? json({ error: 'unavailable' }, 503) : json(withNewFields())) : undefined);
    const { queryClient } = renderPage(`domain=${DOMAIN}&view=overview`);
    await screen.findByText('家长需要反复提醒才能开始'); failed = true;
    await act(async () => { await queryClient.invalidateQueries({ queryKey: ['comment-research', DOMAIN, 'overview'] }); });
    await screen.findByRole('alert'); expect(screen.queryByText('家长需要反复提醒才能开始')).not.toBeInTheDocument();
  });
});
