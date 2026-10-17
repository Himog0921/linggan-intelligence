import { useEffect, useRef, useState } from 'react';
import { reobservationFailure, reobservationReceipt, type Feedback } from './evidenceFeedback';
import { fetchReobservationStatus, isTerminalTask, mergeOperation, requestReobservation, type ReobservationOperation } from './reobservationApi';

/** 有状态地址时每 2.5 秒读一次，全部任务进入终态即停。 */
export const POLL_INTERVAL_MS = 2500;

export interface Reobservation {
  /** 提交请求进行中。 */
  sending: boolean;
  operation: ReobservationOperation | null;
  /** 最近一次提交的结局；换作品或卸载即清空（本次操作结果不跨作品保留）。 */
  feedback: Feedback | null;
  /** 终态后重读作品详情失败。 */
  readError: unknown;
  /** 状态读取失败（保留上一次已知状态）。 */
  pollError: unknown;
  /** 已有工单或租约引用：不能再提交。 */
  queued: boolean;
  /** 只由用户的明确确认调用；从不自动重试。 */
  submit: () => void;
}

interface Options {
  /** 详情给出并已校验同源的复观测入口。 */
  actionPath: string | null;
  /** 全部任务进入终态：重读作品当前事实。返回 false 表示重读失败。 */
  onTerminal: () => Promise<boolean>;
}

/**
 * 立即复观测的动作与状态跟踪。
 *
 * 写操作只由点击（确认后）触发，不由 Effect 触发，所以 StrictMode 的重复挂载不会重复提交。没有请求编号：
 * 同一范围若已有未被认领的工单则合并，一旦被认领再点会新建一张——所以未确认之后不自动重发。
 * 当前后端在创建请求时从不返回租约（statusUrl 恒为 null），轮询分支在真实链路上走不到；这里保留并用合成回执测试。
 */
export function useReobservation({ actionPath, onTerminal }: Options): Reobservation {
  const [sending, setSending] = useState(false);
  const [operation, setOperation] = useState<ReobservationOperation | null>(null);
  const [statusPath, setStatusPath] = useState<string | null>(null);
  const [feedback, setFeedback] = useState<Feedback | null>(null);
  const [readError, setReadError] = useState<unknown>(null);
  const [pollError, setPollError] = useState<unknown>(null);
  const inFlight = useRef(false);
  const controller = useRef<AbortController | null>(null);
  const terminalHandled = useRef<ReobservationOperation | null>(null);
  const latest = useRef({ onTerminal });
  useEffect(() => {
    latest.current = { onTerminal };
  });

  useEffect(() => {
    return () => controller.current?.abort();
  }, []);

  const queued = operation !== null && (operation.workOrderRef !== null || operation.leaseRef !== null);
  const tasks = operation?.tasks ?? [];
  const settled = tasks.length > 0 && tasks.every(isTerminalTask);

  // 终态后重读一次详情；重读失败要如实说，不把旧快照写成新事实。
  useEffect(() => {
    if (!operation || !settled || terminalHandled.current === operation) return;
    terminalHandled.current = operation;
    void latest.current.onTerminal().then(refreshed => {
      if (!refreshed) setReadError('MATERIAL_DETAIL_REFRESH_UNAVAILABLE');
    });
  }, [operation, settled]);

  // 轮询：有状态地址、有任务、还没全部终态。
  useEffect(() => {
    if (statusPath === null || operation === null || tasks.length === 0 || settled) return;
    const abort = new AbortController();
    const timer = window.setTimeout(() => {
      fetchReobservationStatus(statusPath, abort.signal).then(
        status => {
          if (abort.signal.aborted) return;
          setPollError(null);
          setOperation(previous => (previous === null ? status : mergeOperation(previous, status)));
        },
        (error: unknown) => {
          if (abort.signal.aborted) return;
          setPollError(error);
        },
      );
    }, POLL_INTERVAL_MS);
    return () => {
      window.clearTimeout(timer);
      abort.abort();
    };
  }, [statusPath, operation, tasks.length, settled]);

  function submit() {
    if (actionPath === null || queued || inFlight.current) return;
    inFlight.current = true;
    controller.current?.abort();
    const abort = new AbortController();
    controller.current = abort;
    setSending(true);
    setFeedback(null);
    setReadError(null);
    setPollError(null);
    requestReobservation(actionPath, abort.signal).then(
      receipt => {
        if (abort.signal.aborted) return;
        setOperation(receipt.operation);
        setStatusPath(receipt.statusPath);
        setFeedback(reobservationReceipt(receipt.operation));
      },
      (error: unknown) => {
        // 换作品或卸载时取消：服务器可能已经处理，所以不显示「已取消」。
        if (abort.signal.aborted) return;
        setFeedback(reobservationFailure(error));
      },
    ).finally(() => {
      inFlight.current = false;
      if (!abort.signal.aborted) setSending(false);
    });
  }

  return { sending, operation, feedback, readError, pollError, queued, submit };
}
