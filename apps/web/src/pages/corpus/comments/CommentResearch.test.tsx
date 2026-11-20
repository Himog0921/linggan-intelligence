import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, expect, it, vi } from 'vitest';
import { CommentResearch } from './CommentResearch';
import { CommentResearchSelection } from './CommentResearchSelection';

const nav = { view: vi.fn(), research: vi.fn(), comment: vi.fn(), run: vi.fn(), problem: vi.fn() };
const plan = { domainRef: 'domain', knowledgeCutoff: '2026-10-27T00:00:00Z', planHash: 'f'.repeat(64), taskConfigRef: 'config', taskRevision: 3,
  selectedCount: 2, researchableCount: 1, workCount: 1, unitCount: 1, expectedModelCalls: 1,
  waitingL1: { targetCount: 1, workRefs: ['missing'] }, alreadyResearchedCount: 0, alreadyQueuedCount: 0,
  dispatchUnresolvedCount: 0, excludedCounts: {} };
function reply(value: unknown) { return new Response(JSON.stringify(value), { status: 200, headers: { 'content-type': 'application/json' } }); }
afterEach(() => vi.unstubAllGlobals());

it('一次预览所选评论并复用 L1 任务，仅按明确的研究操作提交评论批次', async () => {
  const fetch = vi.fn(async (path: string) => {
    if (path.endsWith('/v2/preview')) return reply(plan);
    if (path.includes('content-understanding/preview')) return reply({ ...plan, workRef: 'missing', inputHash: 'l1-hash', canStart: true, reasons: [] });
    if (path.endsWith('/content-understanding')) return reply({ requestRef: 'l1', reused: false });
    return reply({ batchRef: 'batch', unitCount: 1, targetCount: 1 });
  });
  vi.stubGlobal('fetch', fetch);
  const items = ['work', 'missing'].map(workRef => ({ sourceRef: workRef + '-source', commentKey: { workRef, commentExternalId: 'id' }, observationRole: 'primary' }));
  render(<CommentResearchSelection domain="domain" selected={items} nav={nav} />);
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: '预览所选评论' }));
  expect(await screen.findByText('L1 未就绪 1 条，涉及 1 个作品。')).toBeVisible();
  await user.click(screen.getByRole('button', { name: '为缺 L1 的作品提交内容理解' }));
  expect(await screen.findByText(/1 个作品已收到 L1 提交回执/)).toBeVisible();
  expect(fetch.mock.calls.some(([path]) => path.endsWith('/v2/runs'))).toBe(false);
  await user.click(screen.getByRole('button', { name: '开始所选评论研究' }));
  expect(await screen.findByText(/已提交 1 条评论，共 1 个研究单元/)).toBeVisible();
  await user.click(screen.getByRole('button', { name: '查看研究记录' }));
  expect(nav.run).toHaveBeenCalledWith('batch');
});

it('评论详情区分运行与当前结果，保留原声，并在来源不可用时隐藏旧结果', async () => {
  let restricted = false;
  vi.stubGlobal('fetch', vi.fn(async (path: string) => path.includes('/comment-assignments?') ? reply({ domainRef: 'domain', items: [] }) : reply({
    batchRef: 'batch', state: 'succeeded', failureCode: null,
    result: restricted ? null : { requestRef: 'request', outcome: 'signals', reason: null, workContext: { summary: '合成 L1 摘要' },
      signals: [{ kind: 'need', proposition: '需要外部提醒', quote: '需要提醒', problem: { problemRef: 'problem', label: '每日提醒需求', state: 'candidate', authorCount: 1 } }] },
  })));
  render(<QueryClientProvider client={new QueryClient()}><CommentResearch domain="domain" work="work" source="source" role="primary" nav={nav} /></QueryClientProvider>);
  expect(await screen.findByText(/需要外部提醒/)).toBeVisible();
  expect(screen.getByText('需要提醒').tagName).toBe('BLOCKQUOTE');
  expect(screen.getByText('合成 L1 摘要')).toBeVisible();
  expect(screen.getByRole('button', { name: '每日提醒需求 · 候选 · 1 位作者' })).toBeVisible();
  restricted = true;
  await userEvent.setup().click(screen.getByRole('button', { name: '刷新研究状态' }));
  await screen.findByText(/最近一次执行/);
  await import('@testing-library/react').then(({ waitFor }) => waitFor(() => expect(screen.queryByText('需要提醒')).toBeNull()));
});
