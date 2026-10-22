import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { calls, DOMAIN, installFetch, json, renderPage, WORK, wordCloud } from './testKit';
import { parseWordCloud } from './wordCloudDtos';

afterEach(() => vi.unstubAllGlobals());

describe('V2.3 scoped language observation', () => {
  it('renders only server counts and links a phrase to stable comment evidence', async () => {
    installFetch();
    const { router } = renderPage();
    const region = await screen.findByRole('region', { name: '领域高频表达' });
    expect(await within(region).findByRole('button', { name: '作业拖拉' })).toHaveAttribute('title', expect.stringContaining('3 条不同合格评论'));
    expect(within(region).getByRole('complementary', { name: '高频短语 Top10' })).toHaveTextContent('作业拖拉');
    await userEvent.click(within(region).getByRole('button', { name: '作业拖拉' }));
    await userEvent.click(within(region).getByRole('button', { name: '查看原声 ↗' }));
    const detail = await screen.findByRole('dialog', { name: '词语对应的真实评论' });
    await userEvent.click(within(detail).getByRole('button', { name: /查看评论 1 及父语境/ }));
    await waitFor(() => expect(router.state.location.search).toContain(`commentWorkRef=${WORK}`));
    expect(calls.filter(call => call.url.pathname.endsWith('/word-cloud')).every(call => call.method === 'GET' && call.url.searchParams.get('domain') === DOMAIN)).toBe(true);
  });
  it('keeps time windows and category filters linked without guessing missing categories', async () => {
    installFetch();
    renderPage();
    await screen.findByRole('region', { name: '领域高频表达' });
    await userEvent.click(screen.getByRole('button', { name: '近 7 天' }));
    await waitFor(() => expect(calls.some(call => call.url.pathname.endsWith('/word-cloud') && call.url.searchParams.get('days') === '7')).toBe(true));
    await userEvent.click(screen.getByRole('button', { name: '情绪体验' }));
    const region = screen.getByRole('region', { name: '领域高频表达' });
    expect(within(region).queryByRole('button', { name: '作业拖拉' })).not.toBeInTheDocument();
    expect(within(region).getByRole('button', { name: '情绪失控' })).toBeInTheDocument();
  });
  it('shows a real read failure instead of synthetic frequency values', async () => {
    installFetch(url => url.pathname.endsWith('/word-cloud') ? json({ error: { code: 'catalog_unavailable', message: 'unavailable' } }, 503) : undefined);
    renderPage();
    const region = await screen.findByRole('region', { name: '领域高频表达' });
    await within(region).findByText(/词频暂时无法读取/);
    expect(within(region).queryByRole('button', { name: '作业拖拉' })).not.toBeInTheDocument();
    expect(within(region).getByRole('button', { name: '重试读取' })).toBeInTheDocument();
  });
  it('keeps empty source coverage separate from a claim that the market is silent', async () => {
    installFetch(url => url.pathname.endsWith('/word-cloud') ? json({ ...wordCloud(), eligibleCommentCount: 0, processedCommentCount: 0, items: [] }) : undefined);
    renderPage();
    expect(await screen.findByText(/不代表用户没有表达/)).toBeInTheDocument();
  });
  it('rejects mismatched domain and invalid numeric counts before rendering', () => {
    expect(() => parseWordCloud({ ...wordCloud(), eligibleCommentCount: 1, processedCommentCount: 3 })).toThrow();
    expect(() => parseWordCloud({ ...wordCloud(), items: [{ text: 'X', count: 4, workCount: 1, category: 'scenario', examples: [] }] })).toThrow();
  });
});
