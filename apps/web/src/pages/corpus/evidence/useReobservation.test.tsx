import { act, renderHook, waitFor } from '@testing-library/react';
import { StrictMode, type ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DOMAIN, operationJson, taskJson, workRef } from './evidenceFixtures';
import { createFetchMock, ok, type FetchKit } from './testKit';
import { POLL_INTERVAL_MS, useReobservation } from './useReobservation';

const ACTION = `/api/local/work-resources/${workRef(1)}/reobserve`;
const STATUS = `${ACTION}/lease-1`;
let kit: FetchKit;

beforeEach(() => {
  kit = createFetchMock();
  kit.install();
});

afterEach(() => {
  vi.useRealTimers();
});

function setup(onTerminal: () => Promise<boolean> = () => Promise.resolve(true)) {
  return renderHook(() => useReobservation({ actionPath: `${ACTION}?domain=${DOMAIN}`, onTerminal }));
}

const withLease = (state: string) => ({
  operation: operationJson({ leaseRef: 'lease-1', execution: 'QUEUED', tasks: [taskJson(state)] }),
  statusUrl: `${STATUS}?domain=${DOMAIN}`,
});

describe('立即复观测：提交', () => {
  it('只由 submit 触发，不由 Effect 触发：StrictMode 的重复挂载不会提交', () => {
    const wrapper = ({ children }: { children: ReactNode }) => <StrictMode>{children}</StrictMode>;
    renderHook(() => useReobservation({ actionPath: ACTION, onTerminal: () => Promise.resolve(true) }), { wrapper });
    expect(kit.calls).toHaveLength(0);
  });

  it('提交一次发一个无请求体的 POST；回执之后才显示结局；已有工单后不能再提交', async () => {
    kit.route('POST', ACTION, ok({ operation: operationJson(), statusUrl: null }));
    const { result } = setup();
    act(() => result.current.submit());
    expect(result.current.sending).toBe(true);
    await waitFor(() => expect(result.current.feedback?.label).toBe('已提交'));
    expect(result.current.sending).toBe(false);
    expect(kit.of('POST', ACTION)).toHaveLength(1);
    expect(kit.of('POST', ACTION)[0]?.body).toBeUndefined();
    expect(result.current.queued).toBe(true);
    act(() => result.current.submit());
    expect(kit.of('POST', ACTION)).toHaveLength(1);
  });

  it('请求进行中重复点击只显示进行中，不再发第二个请求', async () => {
    kit.route('POST', ACTION, ok({ operation: operationJson(), statusUrl: null }));
    const { result } = setup();
    act(() => {
      result.current.submit();
      result.current.submit();
    });
    await waitFor(() => expect(result.current.feedback).not.toBeNull());
    expect(kit.of('POST', ACTION)).toHaveLength(1);
  });

  it('200 却被拒：未受理，不是已排队，也不锁住按钮（没有工单）', async () => {
    kit.route('POST', ACTION, ok({ operation: operationJson({ admission: 'REFUSED', workOrderRef: null, execution: 'NOT_STARTED' }), statusUrl: null }));
    const { result } = setup();
    act(() => result.current.submit());
    await waitFor(() => expect(result.current.feedback?.label).toBe('未受理'));
    expect(result.current.queued).toBe(false);
  });

  it('没有得到答复：未确认；页面不自动重发，也不编造工单', async () => {
    kit.route('POST', ACTION, () => 'network');
    const { result } = setup();
    act(() => result.current.submit());
    await waitFor(() => expect(result.current.feedback?.label).toBe('未确认'));
    expect(result.current.operation).toBeNull();
    expect(result.current.queued).toBe(false);
    await new Promise(resolve => setTimeout(resolve, 30));
    expect(kit.of('POST', ACTION)).toHaveLength(1);
  });

  it('没有入口地址时 submit 什么也不做', () => {
    const { result } = renderHook(() => useReobservation({ actionPath: null, onTerminal: () => Promise.resolve(true) }));
    act(() => result.current.submit());
    expect(kit.calls).toHaveLength(0);
  });
});

describe('立即复观测：轮询与终态（合成回执；当前后端请求时不返回租约）', () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
  });

  it('不为没有租约的回执编造轮询', async () => {
    kit.route('POST', ACTION, ok({ operation: operationJson(), statusUrl: null }));
    const { result } = setup();
    act(() => result.current.submit());
    await waitFor(() => expect(result.current.operation).not.toBeNull());
    await act(async () => {
      await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS * 4);
    });
    expect(kit.calls.filter(call => call.method === 'GET')).toHaveLength(0);
  });

  it('有状态地址时每 2.5 秒读一次，合并任务状态，终态后重读详情一次并停止', async () => {
    const onTerminal = vi.fn(() => Promise.resolve(true));
    kit.route('POST', ACTION, ok(withLease('QUEUED')));
    kit.route('GET', STATUS, ok({ operation: { leaseState: 'ACTIVE', tasks: [taskJson('RUNNING')] } }), ok({ operation: { leaseState: 'ACTIVE', tasks: [taskJson('ACCEPTED')] } }));
    const { result } = setup(onTerminal);
    act(() => result.current.submit());
    await waitFor(() => expect(result.current.operation?.tasks[0]?.state).toBe('QUEUED'));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS);
    });
    await waitFor(() => expect(result.current.operation?.tasks[0]?.state).toBe('RUNNING'));
    expect(onTerminal).not.toHaveBeenCalled();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS);
    });
    await waitFor(() => expect(result.current.operation?.tasks[0]?.state).toBe('ACCEPTED'));
    await waitFor(() => expect(onTerminal).toHaveBeenCalledTimes(1));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS * 3);
    });
    expect(kit.of('GET', STATUS)).toHaveLength(2);
    expect(onTerminal).toHaveBeenCalledTimes(1);
    expect(result.current.operation?.admission).toBe('ADMITTED');
  });

  it('INPUT_BLOCKED 是终态：不再对一张已结束的租约每 2.5 秒问一次；终态后重读一次', async () => {
    const onTerminal = vi.fn(() => Promise.resolve(true));
    kit.route('POST', ACTION, ok(withLease('INPUT_BLOCKED')));
    const { result } = setup(onTerminal);
    act(() => result.current.submit());
    await waitFor(() => expect(onTerminal).toHaveBeenCalledTimes(1));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS * 3);
    });
    expect(kit.calls.filter(call => call.method === 'GET')).toHaveLength(0);
    expect(result.current.queued).toBe(true);
  });

  it('终态后重读详情失败：如实记录，不把旧快照写成新事实', async () => {
    kit.route('POST', ACTION, ok(withLease('COMPLETED_WITHOUT_RECEIPT')));
    const { result } = setup(() => Promise.resolve(false));
    act(() => result.current.submit());
    await waitFor(() => expect(result.current.readError).not.toBeNull());
    expect(result.current.operation?.tasks[0]?.state).toBe('COMPLETED_WITHOUT_RECEIPT');
  });

  it('状态读取失败：保留上一次已知状态并记录错误，不写成完成', async () => {
    kit.route('POST', ACTION, ok(withLease('CLAIMED')));
    kit.route('GET', STATUS, () => ({ status: 503, body: { code: 'reobservation_status_unavailable' } }));
    const { result } = setup();
    act(() => result.current.submit());
    await waitFor(() => expect(result.current.operation).not.toBeNull());
    await act(async () => {
      await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS);
    });
    await waitFor(() => expect(result.current.pollError).not.toBeNull());
    expect(result.current.operation?.tasks[0]?.state).toBe('CLAIMED');
  });

  it('换作品或卸载：取消定时器与在途请求，不再发状态请求', async () => {
    kit.route('POST', ACTION, ok(withLease('QUEUED')));
    kit.route('GET', STATUS, ok({ operation: { leaseState: 'ACTIVE', tasks: [taskJson('RUNNING')] } }));
    const { result, unmount } = setup();
    act(() => result.current.submit());
    await waitFor(() => expect(result.current.operation).not.toBeNull());
    unmount();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(POLL_INTERVAL_MS * 3);
    });
    expect(kit.of('GET', STATUS)).toHaveLength(0);
  });

  it('提交请求在途时卸载：迟到的回执不再写入，也不显示「已取消」', async () => {
    let release: () => void = () => undefined;
    const gate = new Promise<void>(resolve => {
      release = resolve;
    });
    kit.route('POST', ACTION, async () => {
      await gate;
      return { status: 200, body: { operation: operationJson(), statusUrl: null } };
    });
    const { result, unmount } = setup();
    act(() => result.current.submit());
    unmount();
    release();
    await act(async () => {
      await Promise.resolve();
    });
    expect(result.current.feedback).toBeNull();
  });
});
