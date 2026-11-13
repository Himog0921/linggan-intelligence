import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { TopicResearchFlow, type ResearchSubmission } from './TopicResearchFlow';
vi.mock('./TopicAutomation', () => ({ TopicAutomation: ({ onEnabledChange }: { onEnabledChange: (enabled: boolean) => void }) => <button onClick={() => onEnabledChange(true)}>启用自动归并测试</button> }));
const response = (pending: number, ref = 'run') => ({ runs: [{ runRef: ref, executionMode: 'single_pass', pendingStageCount: pending, failedStageCount: 0, windowCount: 1, stages: [] }] });
const json = (value: unknown) => new Response(JSON.stringify(value), { headers: { 'Content-Type': 'application/json' } });
const works = [{ workRef: 'work', title: '作品' }];
beforeEach(() => { vi.useFakeTimers(); });
afterEach(() => { cleanup(); vi.useRealTimers(); vi.unstubAllGlobals(); });
async function settle() { await act(async () => { await vi.advanceTimersByTimeAsync(0); }); }
it('polls running work, refreshes the map once at completion, then stops polling', async () => {
  const done = vi.fn();
  const fetch = vi.fn().mockResolvedValueOnce(json(response(1))).mockImplementation(async () => json(response(0)));
  vi.stubGlobal('fetch', fetch);
  render(<MemoryRouter><TopicResearchFlow domain="one" works={works} onCompleted={done} /></MemoryRouter>);
  await settle();
  expect(screen.getByText(/研究进行中/)).toBeInTheDocument();
  await act(async () => { await vi.advanceTimersByTimeAsync(3000); });
  expect(done).toHaveBeenCalledTimes(1);
  expect(screen.getByText(/研究完成/)).toBeInTheDocument();
  await act(async () => { await vi.advanceTimersByTimeAsync(9000); });
  expect(fetch).toHaveBeenCalledTimes(2);
});
it('reads progress immediately after submitting and cancelling', async () => {
  let submitted = false;
  const fetch = vi.fn(async (_path: string, options: RequestInit) => {
    if (options.method === 'POST') { submitted = true; return json({ runRef: 'run' }); }
    return json(submitted ? response(1) : { runs: [] });
  });
  vi.stubGlobal('fetch', fetch);
  render(<MemoryRouter><TopicResearchFlow domain="one" works={works} /></MemoryRouter>);
  await settle();
  fireEvent.click(screen.getByRole('checkbox'));
  fireEvent.click(screen.getByRole('button', { name: '开始研究' }));
  await settle();
  expect(fetch).toHaveBeenCalledTimes(3);
  expect(screen.getByText(/研究进行中/)).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '取消研究' }));
  await settle();
  expect(fetch).toHaveBeenCalledTimes(5);
});
it('resets selection on domain change and ignores a late previous-domain response', async () => {
  let finish!: (value: Response) => void;
  const fetch = vi.fn().mockImplementationOnce(() => new Promise<Response>(resolve => { finish = resolve; })).mockResolvedValue(json({ runs: [] }));
  vi.stubGlobal('fetch', fetch);
  const page = render(<MemoryRouter><TopicResearchFlow domain="one" works={works} /></MemoryRouter>);
  await settle();
  fireEvent.click(screen.getByRole('checkbox'));
  page.rerender(<MemoryRouter><TopicResearchFlow domain="two" works={works} /></MemoryRouter>);
  await settle();
  expect(screen.getByRole('checkbox')).not.toBeChecked();
  await act(async () => { finish(json(response(1, 'old'))); });
  expect(screen.queryByText(/研究进行中/)).not.toBeInTheDocument();
  await act(async () => { await vi.advanceTimersByTimeAsync(6000); });
  expect(fetch).toHaveBeenCalledTimes(2);
});
it('refreshes the map when a newly submitted run has already completed at the first read', async () => {
  const done = vi.fn();
  let submitted = false;
  vi.stubGlobal('fetch', vi.fn(async (_path: string, options: RequestInit) => {
    if (options.method === 'POST') { submitted = true; return json({ runRef: 'run' }); }
    return json(submitted ? response(0) : { runs: [] });
  }));
  render(<MemoryRouter><TopicResearchFlow domain="one" works={works} onCompleted={done} /></MemoryRouter>);
  await settle();
  fireEvent.click(screen.getByRole('checkbox'));
  fireEvent.click(screen.getByRole('button', { name: '开始研究' }));
  await settle();
  expect(done).toHaveBeenCalledTimes(1);
  await act(async () => { await vi.advanceTimersByTimeAsync(6000); });
  expect(done).toHaveBeenCalledTimes(1);
});
it('refreshes once after reopening a run that completed while closed, without a refresh loop', async () => {
  const done = vi.fn();
  let pending = 1;
  const fetch = vi.fn(async () => json(response(pending)));
  vi.stubGlobal('fetch', fetch);
  const page = render(<MemoryRouter><TopicResearchFlow domain="one" works={works} onCompleted={done} /></MemoryRouter>);
  await settle();
  expect(done).not.toHaveBeenCalled();
  page.unmount();
  pending = 0;
  const reopened = render(<MemoryRouter><TopicResearchFlow domain="one" works={works} onCompleted={done} /></MemoryRouter>);
  await settle();
  expect(done).toHaveBeenCalledTimes(1);
  reopened.rerender(<MemoryRouter><TopicResearchFlow domain="one" works={[...works]} onCompleted={() => done()} /></MemoryRouter>);
  fireEvent.click(screen.getByRole('button', { name: '刷新进度' }));
  await settle();
  await act(async () => { await vi.advanceTimersByTimeAsync(9000); });
  expect(done).toHaveBeenCalledTimes(1);
  expect(fetch).toHaveBeenCalledTimes(3);
});
it('reuses the frozen request after closing during a network-unknown submission', async () => {
  let current: ResearchSubmission | null = null;
  const requestState = { get: () => current, set: (value: ResearchSubmission | null) => { current = value; } };
  let reject!: (error: Error) => void;
  const posts: string[] = [];
  vi.stubGlobal('fetch', vi.fn(async (_path: string, options: RequestInit) => {
    if (options.method === 'POST') {
      posts.push(String(options.body));
      if (posts.length === 1) return new Promise<Response>((_resolve, fail) => { reject = fail; });
      return json({ runRef: 'run' });
    }
    return json({ runs: [] });
  }));
  const page = render(<MemoryRouter><TopicResearchFlow domain="one" works={works} requestState={requestState} /></MemoryRouter>);
  await settle();
  fireEvent.click(screen.getByRole('checkbox'));
  fireEvent.click(screen.getByRole('button', { name: '开始研究' }));
  page.unmount();
  await act(async () => { reject(new TypeError('network')); });
  render(<MemoryRouter><TopicResearchFlow domain="one" works={works} requestState={requestState} /></MemoryRouter>);
  await settle();
  expect(screen.getByRole('checkbox')).toBeChecked();
  expect(screen.getByRole('checkbox')).toBeDisabled();
  fireEvent.click(screen.getByRole('button', { name: '重试同一请求' }));
  await settle();
  expect(posts).toHaveLength(2);
  expect(posts[1]).toEqual(posts[0]);
  expect(requestState.get()).toBeNull();
});
it('follows an in-flight submission after reopening without issuing another POST', async () => {
  let current: ResearchSubmission | null = null;
  const requestState = { get: () => current, set: (value: ResearchSubmission | null) => { current = value; } };
  let finish!: (value: Response) => void;
  let posts = 0;
  vi.stubGlobal('fetch', vi.fn(async (_path: string, options: RequestInit) => {
    if (options.method === 'POST') {
      posts++;
      return new Promise<Response>(resolve => { finish = resolve; });
    }
    return json({ runs: [] });
  }));
  const page = render(<MemoryRouter><TopicResearchFlow domain="one" works={works} requestState={requestState} /></MemoryRouter>);
  await settle();
  fireEvent.click(screen.getByRole('checkbox'));
  fireEvent.click(screen.getByRole('button', { name: '开始研究' }));
  page.unmount();
  render(<MemoryRouter><TopicResearchFlow domain="one" works={works} requestState={requestState} /></MemoryRouter>);
  await settle();
  expect(screen.getByRole('button', { name: '重试同一请求' })).toBeDisabled();
  await act(async () => { finish(json({ runRef: 'run' })); });
  expect(posts).toBe(1);
  expect(requestState.get()).toBeNull();
  expect(screen.getByRole('button', { name: '开始研究' })).toBeEnabled();
});

it('keeps selections across search and pages and submits only those selections', async () => {
  const posts: { workRefs: string[] }[] = [];
  vi.stubGlobal('fetch', vi.fn(async (_path: string, options: RequestInit) => {
    if (options.method === 'POST') { posts.push(JSON.parse(String(options.body))); return json({ runRef: 'run' }); }
    return json({ runs: [] });
  }));
  const many = Array.from({ length: 41 }, (_, i) => ({ workRef: `work-${i}`, title: `作品${i}` }));
  render(<MemoryRouter><TopicResearchFlow domain="one" works={many} /></MemoryRouter>);
  await settle();
  expect(screen.getAllByRole('checkbox')).toHaveLength(20);
  fireEvent.click(screen.getByRole('checkbox', { name: '作品0' }));
  fireEvent.click(screen.getByRole('button', { name: '下一页' }));
  fireEvent.click(screen.getByRole('checkbox', { name: '作品20' }));
  fireEvent.change(screen.getByRole('searchbox'), { target: { value: '作品40' } });
  fireEvent.click(screen.getByRole('checkbox', { name: '作品40' }));
  fireEvent.click(screen.getByRole('button', { name: '开始研究' }));
  await settle();
  expect(posts[0]?.workRefs).toEqual(['work-0', 'work-20', 'work-40']);
});

it('discovers automatically queued runs after an initially empty response', async () => {
  const done = vi.fn();
  const fetch = vi.fn().mockResolvedValueOnce(json({ runs: [] }))
    .mockResolvedValueOnce(json(response(1))).mockResolvedValue(json(response(0)));
  vi.stubGlobal('fetch', fetch);
  render(<MemoryRouter><TopicResearchFlow domain="one" works={works} onCompleted={done} /></MemoryRouter>);
  await settle();
  fireEvent.click(screen.getByRole('button', { name: '启用自动归并测试' }));
  await act(async () => { await vi.advanceTimersByTimeAsync(5000); });
  expect(screen.getByText(/研究进行中/)).toBeInTheDocument();
  await act(async () => { await vi.advanceTimersByTimeAsync(3000); });
  expect(done).toHaveBeenCalledTimes(1);
});

it('refreshes for a new automatic run completed between two reads only once', async () => {
  const done = vi.fn();
  vi.stubGlobal('fetch', vi.fn().mockResolvedValueOnce(json({ runs: [] })).mockResolvedValue(json(response(0, 'fast-auto'))));
  render(<MemoryRouter><TopicResearchFlow domain="one" works={works} onCompleted={done} /></MemoryRouter>);
  await settle();
  fireEvent.click(screen.getByRole('button', { name: '启用自动归并测试' }));
  await act(async () => { await vi.advanceTimersByTimeAsync(5000); });
  expect(done).toHaveBeenCalledTimes(1);
  await act(async () => { await vi.advanceTimersByTimeAsync(10000); });
  expect(done).toHaveBeenCalledTimes(1);
});

it('uses the explicit execution mode while preserving legacy phase labels', async () => {
  const single = { ...response(0).runs[0]!, stages: [{ windowRef: 'w1', stageNumber: 0, state: 'succeeded' }] };
  const legacy = { ...response(0, 'legacy').runs[0]!, executionMode: 'legacy_four_stage', stages: [{ windowRef: 'w2', stageNumber: 0, state: 'succeeded' }, { windowRef: 'w2', stageNumber: 3, state: 'succeeded' }] };
  vi.stubGlobal('fetch', vi.fn(async () => json({ runs: [single, legacy] })));
  render(<MemoryRouter><TopicResearchFlow domain="one" works={works} /></MemoryRouter>);
  await settle();
  expect(screen.getByText(/主题归并：已完成/)).toBeVisible();
  expect(screen.getByText(/讨论提炼：已通过/)).toBeVisible();
  expect(screen.getByText(/归属核验：已通过/)).toBeVisible();
});
