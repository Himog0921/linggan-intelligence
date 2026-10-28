import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, expect, it, vi } from 'vitest';
import { CommentResearch } from './CommentResearch';

const selected = { domainRef: 'domain', workRef: 'work', sourceRef: 'source', observationRole: 'primary', knowledgeCutoff: '2026-10-27T00:00:00Z', inputHash: 'f'.repeat(64), taskConfigRef: 'config', taskRevision: 3, parentState: 'parent_required_missing', coverage: { imageInventory: { expectedPages: null, acquiredPages: 4, processedPages: 0 } } };
function reply(value: unknown) { return new Response(JSON.stringify(value), { status: 200, headers: { 'content-type': 'application/json' } }); }
afterEach(() => vi.unstubAllGlobals());
it('研究选中评论，呈现缺失语境，并在来源受限时隐藏结果', async () => {
  let restricted = false;
  vi.stubGlobal('fetch', vi.fn(async (path: string, init?: RequestInit) => {
    if (path.includes('/preview?')) return reply(selected);
    if (init?.method === 'POST') return reply({ requestRef: 'request', batchRef: 'batch' });
    return reply({ requestRef: 'request', state: 'succeeded', semanticState: 'succeeded', stages: [0, 1, 2].map(stageNumber => ({ stageNumber, state: 'succeeded', failureCode: null })), target: { sourceRef: 'source', rawText: '需要提醒。' }, sourceAvailable: !restricted, failureCode: null, result: { outcome: 'signals', reason: null, signals: [{ proposition: '需要提醒', evidenceSpan: { start: 0, end: 4 } }] } });
  }));
  render(<CommentResearch domain="domain" work="work" source="source" role="primary" />);
  const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: '检查评论与研究底稿' }));
  expect(await screen.findByText(/父评论暂不可用/)).toBeInTheDocument();
  expect(screen.getByText(/图片总数 未知 · 已取得 4 · 已识读 0/)).toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: '开始研究' }));
  expect(await screen.findByText('已提交，研究尚未完成。')).toBeInTheDocument();
  await user.click(screen.getByRole('button', { name: '刷新研究状态' }));
  expect((await screen.findAllByText('需要提醒')).length).toBe(2);
  restricted = true; await user.click(screen.getByRole('button', { name: '刷新研究状态' }));
  expect(await screen.findByText('来源当前不可读取，结果不展示。')).toBeInTheDocument();
  expect(screen.queryAllByText('需要提醒')).toHaveLength(0);
});
it('重新打开详情时能读取最近结果和对应原声', async () => {
  vi.stubGlobal('fetch', vi.fn(async () => reply({ requestRef: 'request', state: 'succeeded', semanticState: 'succeeded', stages: [0, 1, 2].map(stageNumber => ({ stageNumber, state: 'succeeded', failureCode: null })), target: { sourceRef: 'source', rawText: '需要提醒。' }, sourceAvailable: true, failureCode: null, result: { outcome: 'signals', reason: null, signals: [{ proposition: '需要外部提醒', evidenceSpan: { start: 0, end: 4 } }] } })));
  render(<CommentResearch domain="domain" work="work" source="source" role="primary" />);
  const user = userEvent.setup(); await user.click(screen.getByRole('button', { name: '查看最近研究' }));
  expect(await screen.findByText('需要外部提醒')).toBeInTheDocument();
  expect(screen.getByText('需要提醒').tagName).toBe('BLOCKQUOTE');
});
