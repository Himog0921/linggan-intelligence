import { act, cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, expect, it, vi } from 'vitest';
import { CommentAssignments } from './CommentAssignments';
import { parseCommentAssignments, retryCommentAssignment } from './commentAssignmentsApi';
const response = (body: unknown, status = 200) => new Response(JSON.stringify(body), { status, headers: { 'content-type': 'application/json' } });
const signal = (assignmentState: string, status: string | null = null) => ({ signalOrdinal: 0, kind: 'need', proposition: '需要提醒', evidence: { sourceRef: 's', start: 0, end: 4, text: '原评论证据' }, assignmentState,
  assignment: status ? { status, matches: status === 'matched' ? [{ topicRef: 'topic', definitionRef: 'definition', label: '已有提醒主题', reason: '来源相符' }] : [], reason: '合成判断' } : null });
const item = (assignmentState = 'complete', status: string | null = 'matched') => ({ requestRef: 'request', windowRef: 'window', workRef: 'work', sourceRef: 's', commentExternalId: 'original-id', sourceAvailable: true, assignmentState, signals: [signal(assignmentState, status)] });
afterEach(() => { cleanup(); vi.useRealTimers(); vi.unstubAllGlobals(); });
it('reads once per scope, separates matched/unmatched/not analyzed and preserves exact comment and topic links', async () => {
  const fetch = vi.fn(async () => response({ domainRef: 'domain', items: [item(), { ...item('complete', 'uncertain'), requestRef: 'unmatched' }, { ...item('not_analyzed'), requestRef: 'new' }] }));
  vi.stubGlobal('fetch', fetch);
  render(<CommentAssignments domain="domain" work="work" />);
  expect(await screen.findByText('已匹配')).toBeVisible();
  expect(screen.getByText('未匹配')).toBeVisible();
  expect(screen.getByText('尚未分析')).toBeVisible();
  expect(fetch).toHaveBeenCalledTimes(1);
  expect(screen.getByRole('link', { name: '已有提醒主题' })).toHaveAttribute('href', '/topics?tm=1&domainRef=domain&topicRef=topic');
  const url = new URL(screen.getAllByRole('link', { name: '查看原评论' })[0]!.getAttribute('href')!, 'http://localhost');
  expect(url.searchParams.get('commentExternalId')).toBe('original-id');
  expect(url.searchParams.get('researchRequestRef')).toBe('request');
});
it('does not turn an absent result, a failure, or withdrawn content into an unmatched signal', async () => {
  let body: unknown = { domainRef: 'domain', items: [] };
  vi.stubGlobal('fetch', vi.fn(async () => response(body)));
  render(<CommentAssignments domain="domain" work="work" />);
  expect(await screen.findByText('尚无评论语义结果')).toBeVisible();
  expect(screen.queryByText('未匹配')).not.toBeInTheDocument();
  body = { domainRef: 'domain', items: [item('failed', null), { ...item(), requestRef: 'withdrawn', sourceAvailable: false }] };
  await userEvent.click(screen.getByRole('button', { name: '刷新挂靠' }));
  expect(await screen.findByText('挂靠失败')).toBeVisible();
  expect(screen.getByText('来源不可读')).toBeVisible();
  expect(screen.getAllByText('原评论证据')).toHaveLength(1);
  expect(screen.queryByRole('link', { name: '已有提醒主题' })).not.toBeInTheDocument();
});
it('rejects another scope and discards late work reads even if transport ignores abort', async () => {
  let finish!: (value: Response) => void;
  vi.stubGlobal('fetch', vi.fn((path: string) => path.includes('workRef=work&') ? new Promise<Response>(resolve => { finish = resolve; }) : Promise.resolve(response({ domainRef: 'domain', items: [] }))));
  const mounted = render(<CommentAssignments domain="domain" work="work" request="request" compact />);
  mounted.rerender(<CommentAssignments domain="domain" work="other" compact />);
  expect(await screen.findByText('尚无评论语义结果')).toBeVisible();
  finish(response({ domainRef: 'domain', items: [item()] }));
  await waitFor(() => expect(screen.queryByText('已有提醒主题')).not.toBeInTheDocument());
  expect(() => parseCommentAssignments({ domainRef: 'other', items: [] }, { domain: 'domain', work: 'work' })).toThrow();
});
it('keeps failed reads explicit and lets the same scope recover', async () => {
  let fail = true;
  vi.stubGlobal('fetch', vi.fn(async () => response(fail ? { error: 'unavailable' } : { domainRef: 'domain', items: [item()] }, fail ? 503 : 200)));
  render(<CommentAssignments domain="domain" work="work" request="request" compact />);
  expect(await screen.findByRole('alert')).toHaveTextContent('挂靠结果读取失败');
  fail = false; await userEvent.click(screen.getByRole('button', { name: '刷新挂靠' }));
  expect(await screen.findByText('已匹配')).toBeVisible();
  expect(screen.queryByText('原评论证据')).not.toBeInTheDocument();
});

it('keeps expired assignments distinct from unreadable original comments', async () => {
  vi.stubGlobal('fetch', vi.fn(async () => response({ domainRef: 'domain', items: [
    item('unavailable', null), { ...item('unavailable', null), requestRef: 'withdrawn', sourceAvailable: false },
  ] })));
  render(<CommentAssignments domain="domain" work="work" />);
  expect(await screen.findByText('归属待更新')).toBeVisible();
  expect(screen.getByText('来源不可读')).toBeVisible();
  expect(screen.getAllByText('原评论证据')).toHaveLength(1);
  expect(screen.queryByText('未匹配')).not.toBeInTheDocument();
});

it('refreshes automatic results while mounted and stops after closing', async () => {
  vi.useFakeTimers();
  const fetch = vi.fn(async () => response({ domainRef: 'domain', items: [item(fetch.mock.calls.length === 1 ? 'not_analyzed' : fetch.mock.calls.length === 3 ? 'pending' : 'complete', fetch.mock.calls.length > 3 ? 'uncertain' : 'matched')] }));
  vi.stubGlobal('fetch', fetch);
  let mounted!: ReturnType<typeof render>;
  await act(async () => { mounted = render(<CommentAssignments domain="domain" work="work" />); });
  expect(screen.getByText('尚未分析')).toBeVisible();
  await act(async () => { await vi.advanceTimersByTimeAsync(5000); });
  expect(screen.getByText('已匹配')).toBeVisible();
  await act(async () => { await vi.advanceTimersByTimeAsync(5000); });
  expect(screen.getByText('挂靠中')).toBeVisible();
  await act(async () => { await vi.advanceTimersByTimeAsync(5000); });
  expect(screen.getByText('未匹配')).toBeVisible();
  expect(fetch).toHaveBeenCalledTimes(4);
  mounted.unmount();
  await act(async () => { await vi.advanceTimersByTimeAsync(10000); });
  expect(fetch).toHaveBeenCalledTimes(4);
});
it('retries only an explicitly eligible window once and reads the new pending result', async () => {
  let state = 'failed';
  let release!: (response: Response) => void;
  const fetch = vi.fn(async (_path: string, init?: RequestInit) => {
    if (init?.method === 'POST') return new Promise<Response>(resolve => { release = resolve; });
    return response({ domainRef: 'domain', items: [{ ...item(state, null), retryable: state === 'failed', signals: [signal(state), { ...signal(state), signalOrdinal: 1 }] }] });
  });
  vi.stubGlobal('fetch', fetch);
  render(<CommentAssignments domain="domain" work="work" />);
  const retry = await screen.findByRole('button', { name: '重试挂靠' });
  expect(screen.getAllByRole('button', { name: '重试挂靠' })).toHaveLength(1);
  await userEvent.click(retry);
  expect(retry).toBeDisabled();
  const posted = fetch.mock.calls.find(([, init]) => init?.method === 'POST')!;
  expect(JSON.parse(String(posted[1]?.body))).toMatchObject({ domainRef: 'domain', windowRef: 'window' });
  state = 'pending';
  release(response({ runRef: 'retry-run', windowCount: 1, stageCount: 1 }));
  await waitFor(() => expect(screen.queryByRole('button', { name: '重试挂靠' })).not.toBeInTheDocument());
  expect(screen.getAllByText('挂靠中')).toHaveLength(2);
});
it('retains the same command after an unknown receipt and does not offer dispatch-unknown or withdrawn retries', async () => {
  const bodies: unknown[] = [];
  let eligible = true;
  let sourceAvailable = true;
  vi.stubGlobal('fetch', vi.fn(async (_path: string, init?: RequestInit) => {
    if (init?.method === 'POST') { bodies.push(JSON.parse(String(init.body))); throw new TypeError('connection closed'); }
    return response({ domainRef: 'domain', items: [{ ...item('failed', null), retryable: eligible, sourceAvailable }] });
  }));
  render(<CommentAssignments domain="domain" work="work" />);
  await userEvent.click(await screen.findByRole('button', { name: '重试挂靠' }));
  expect(await screen.findByRole('alert')).toHaveTextContent('回执未知');
  await userEvent.click(screen.getByRole('button', { name: '重试挂靠' }));
  await waitFor(() => expect(bodies).toHaveLength(2));
  expect(bodies[0]).toEqual(bodies[1]);
  eligible = false;
  await userEvent.click(screen.getByRole('button', { name: '刷新挂靠' }));
  await waitFor(() => expect(screen.queryByRole('button', { name: '重试挂靠' })).not.toBeInTheDocument());
  eligible = true; sourceAvailable = false;
  await userEvent.click(screen.getByRole('button', { name: '刷新挂靠' }));
  expect(await screen.findByText('来源不可读')).toBeVisible();
  expect(screen.queryByRole('button', { name: '重试挂靠' })).not.toBeInTheDocument();
});

it('accepts a stored two-stage retry receipt replay without sending another request', async () => {
  const fetch = vi.fn(async () => response({ runRef: 'historical-retry-run', windowCount: 1, stageCount: 2 }));
  vi.stubGlobal('fetch', fetch);
  await expect(retryCommentAssignment({ commandRef: 'same-command', domainRef: 'domain', windowRef: 'old-window' })).resolves.toBe('historical-retry-run');
  expect(fetch).toHaveBeenCalledTimes(1);
});
