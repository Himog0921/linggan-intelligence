import { afterEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '../../../shared/api';
import { mediaRef, operationJson, taskJson, workRef } from './evidenceFixtures';
import { fetchReobservationStatus, isTerminalTask, parseOperation, parseReobservationReceipt, parseReobservationStatus, requestReobservation } from './reobservationApi';

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('复观测回执解码', () => {
  it('请求回执：准入、工单、执行状态；请求时没有租约和状态地址', () => {
    const receipt = parseReobservationReceipt({ operation: operationJson(), statusUrl: null });
    expect(receipt.statusPath).toBeNull();
    expect(receipt.operation).toMatchObject({ admission: 'ADMITTED', execution: 'QUEUED', leaseRef: null, workOrderRef: mediaRef(904), mediaState: 'NOT_REQUESTED', mediaReason: 'EXISTING_ASSETS_REUSED', tasks: [] });
  });

  it('状态地址只在同源且在作品资源根下时保留', () => {
    const own = `/api/local/work-resources/${workRef(1)}/reobserve/${mediaRef(7)}?domain=d`;
    expect(parseReobservationReceipt({ operation: operationJson(), statusUrl: own }).statusPath).toBe(own);
    expect(parseReobservationReceipt({ operation: operationJson(), statusUrl: 'https://evil.example/x' }).statusPath).toBeNull();
  });

  it('任务行：评论上限可以是数字或 not_requested；媒体、认领、回执引用', () => {
    const operation = parseOperation(operationJson({ tasks: [taskJson('RUNNING', { commentLimit: 30, acquireMedia: 'not_requested', claimedAt: '2026-10-16 09:00', attemptId: mediaRef(1), packageRef: mediaRef(2) })] }));
    expect(operation.tasks[0]).toMatchObject({ state: 'RUNNING', commentLimit: 30, acquireMedia: 'not_requested', claimedAt: '2026-10-16 09:00', attemptId: mediaRef(1), packageRef: mediaRef(2), receiptRef: null });
  });

  it('缺 operation 报无法解析；状态响应用 leaseState', () => {
    expect(() => parseReobservationReceipt({})).toThrow(ApiError);
    const status = parseReobservationStatus({ operation: { leaseState: 'ACTIVE', expiresAt: '2026-10-16T10:00:00Z', tasks: [taskJson('CLAIMED')], media: { state: 'NOT_REQUESTED', reason: 'EXISTING_ASSETS_REUSED' } } });
    expect(status).toMatchObject({ leaseState: 'ACTIVE', admission: null, workOrderRef: null });
  });

  it('终态：已接纳回执／租约结束未见回执／任务结束未见回执／输入不可执行', () => {
    for (const state of ['ACCEPTED', 'EXPIRED_WITHOUT_RECEIPT', 'COMPLETED_WITHOUT_RECEIPT', 'INPUT_BLOCKED']) expect(isTerminalTask({ state }), state).toBe(true);
    for (const state of ['QUEUED', 'CLAIMED', 'RUNNING', 'PAGE_UNAVAILABLE', 'DETAIL_READ_BLOCKED', 'NOT_STARTED']) expect(isTerminalTask({ state }), state).toBe(false);
  });
});

describe('复观测请求', () => {
  it('POST 不带请求体；状态读取是 GET', async () => {
    const fetchMock = vi.fn(async (_url: string, init?: RequestInit) => new Response(JSON.stringify(init?.method === 'POST' ? { operation: operationJson(), statusUrl: null } : { operation: { leaseState: 'ACTIVE', tasks: [] } }), { status: 200, headers: { 'Content-Type': 'application/json' } }));
    vi.stubGlobal('fetch', fetchMock);
    const path = `/api/local/work-resources/${workRef(1)}/reobserve?domain=d`;
    await requestReobservation(path);
    const [, init] = fetchMock.mock.calls[0] ?? [];
    expect(init).toMatchObject({ method: 'POST', body: null });
    await fetchReobservationStatus(`${path}`);
    expect(fetchMock.mock.calls[1]?.[1]).toMatchObject({ method: 'GET' });
  });

  it('网络失败是 network（未确认），不重试', async () => {
    const fetchMock = vi.fn(async () => {
      throw new TypeError('Failed to fetch');
    });
    vi.stubGlobal('fetch', fetchMock);
    await expect(requestReobservation('/api/local/work-resources/x/reobserve?domain=d')).rejects.toMatchObject({ failure: { kind: 'network' } });
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });
});
