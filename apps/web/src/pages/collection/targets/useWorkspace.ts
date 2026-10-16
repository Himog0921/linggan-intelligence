import { useCallback, useEffect, useRef, useState } from 'react';
import { ApiError, isRecord, requestJson } from '../../../shared/api';
import { RequestIdentities } from '../../../shared/requestIdentity';
import { executeCollectionCommand, readCollectionCommandReceipt } from '../collectionCommand';

export function useRead<T>(path: string | null, decode: (value: unknown) => T, revision = 0) {
  const [state, setState] = useState<{ path: string | null; data: T | null; error: string | null; errorCode: string | null }>({
    path: null,
    data: null,
    error: null,
    errorCode: null,
  });
  useEffect(() => {
    if (path === null) return;
    const controller = new AbortController();
    void requestJson(path, { signal: controller.signal })
      .then(decode)
      .then((data) => {
        if (!controller.signal.aborted) setState({ path, data, error: null, errorCode: null });
      })
      .catch((error: unknown) => {
        if (!controller.signal.aborted)
          setState({
            path,
            data: null,
            errorCode: error instanceof ApiError && error.failure.kind === 'http' ? error.failure.code : null,
            error:
              error instanceof ApiError && error.failure.kind === 'http' && error.failure.status === 404
                ? '未找到该观察目标'
                : '当前读取暂不可用；未知不表示没有记录。',
          });
      });
    return () => controller.abort();
  }, [path, decode, revision]);
  return state.path === path ? state : { path, data: null, error: null, errorCode: null };
}

type Fields = Record<string, string | number | boolean | string[]>;
type Feedback = {
  requestRef: string; action: string; text: string; unknown: boolean; code: string | null;
  applied: boolean; location?: string | null; scope?: string;
};
interface PendingIntent { scope: string; context?: string; requestRef: string; action: string; fields: Fields }
// Only unconfirmed command metadata/drafts live in this document's memory. No persistent
// storage, original material, account identity or credential is retained.
const unsettled = new Map<string, PendingIntent>();
function commandScope(action: string, fields: Fields) {
  const objects = ['row_target_ref', 'target_ref', 'rule_slot'].map((key) => {
    const value = fields[key];
    return [key, Array.isArray(value) ? [...value].sort() : value ?? null];
  });
  return JSON.stringify({ action, objects, createDomain: action === 'new' ? fields.domain ?? null : null });
}
export function targetCommandContext(params: URLSearchParams) {
  return JSON.stringify([params.get('domain') ?? 'all', params.get('drawer') ?? params.get('rule') ?? params.get('delete') ?? params.get('assign_domain') ?? 'list']);
}
export function pendingTargetFields(action: string, match: Fields = {}, context?: string): Fields | null {
  const found = [...unsettled.values()].find((entry) => entry.action === action
    && (context === undefined || entry.context === context)
    && Object.entries(match).every(([key, value]) => JSON.stringify(entry.fields[key]) === JSON.stringify(value)));
  return found?.fields ?? null;
}
function restoredIntent(context?: string) {
  return context === undefined ? null : [...unsettled.values()].find((entry) => entry.context === context) ?? null;
}
function unknownFeedback(intent: PendingIntent): Feedback {
  return { scope: intent.scope, requestRef: intent.requestRef, action: intent.action, code: null, applied: false, unknown: true,
    text: '提交结果未确认，可能已经生效。保留原请求编号，先核实服务器记录。' };
}
export function useCommands(onApplied: (location: string | null) => void, context?: string) {
  const identities = useRef(new RequestIdentities());
  const [feedback, setFeedback] = useState<Feedback | null>(() => {
    const restored = restoredIntent(context); return restored ? unknownFeedback(restored) : null;
  });
  const [activeContext, setActiveContext] = useState(context);
  const [busy, setBusy] = useState(false);
  const mounted = useRef(false), currentContext = useRef(context), inFlight = useRef(false);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  useEffect(() => { currentContext.current = context; }, [context]);
  if (activeContext !== context) {
    setActiveContext(context);
    const restored = restoredIntent(context); setFeedback(restored ? unknownFeedback(restored) : null);
  }
  const send = useCallback(async (intent: PendingIntent, verify: boolean) => {
    if (inFlight.current) return false;
    inFlight.current = true; setBusy(true);
    const displayContext = currentContext.current;
    const display = () => mounted.current && currentContext.current === displayContext;
    const settle = () => {
      if (unsettled.get(intent.scope)?.requestRef === intent.requestRef) unsettled.delete(intent.scope);
      identities.current.settle(intent.action);
    };
    try {
      const receipt = verify ? await readCollectionCommandReceipt(intent.action, intent.requestRef)
        : await executeCollectionCommand(intent.action,
          intent.action === 'rules' ? { ...intent.fields, idempotency_key: intent.requestRef } : intent.fields, intent.requestRef);
      settle();
      const applied = receipt.outcome === 'applied';
      if (display()) {
        setFeedback({ requestRef: intent.requestRef, action: intent.action, code: receipt.code,
          applied, unknown: false, location: receipt.location,
          text: applied ? '操作已取得服务器回执。采集进度以实际接纳结果为准。' : `操作未完成：${receipt.code ?? '服务器拒绝'}` });
        if (applied) onApplied(receipt.location);
      }
      return applied;
    } catch (error) {
      if (error instanceof ApiError && error.answered) {
        settle();
        const response = error.responseBody;
        const location = isRecord(response) && response.requestRef === intent.requestRef && response.action === intent.action
          && response.outcome === 'rejected' && typeof response.location === 'string'
          && response.location.startsWith('/collection/') && !/[\\\r\n]/.test(response.location) ? response.location : null;
        if (display()) setFeedback({ requestRef: intent.requestRef, action: intent.action,
          code: error.failure.kind === 'http' ? error.failure.code : null,
          applied: false, unknown: false, location,
          text: `操作未完成：${error.failure.kind === 'http' ? error.failure.code ?? `HTTP ${error.failure.status}` : '服务器拒绝'}` });
      } else if (display()) setFeedback(unknownFeedback(intent));
      return false;
    } finally { inFlight.current = false; if (mounted.current) setBusy(false); }
  }, [onApplied]);
  const run = useCallback(async (action: string, fields: Fields) => {
    if (inFlight.current) return false;
    const scope = commandScope(action, fields), previous = unsettled.get(scope);
    if (previous) {
      setFeedback(unknownFeedback(previous));
      return send(previous, true);
    }
    if (feedback?.unknown) {
      return feedback.scope === scope ? send({ scope, context, action, fields, requestRef: feedback.requestRef }, true) : false;
    }
    const intent = { scope, context, action, fields: { ...fields }, requestRef: identities.current.keyFor(action, JSON.stringify(fields)) };
    unsettled.set(scope, intent);
    return send(intent, false);
  }, [context, feedback, send]);
  const verify = useCallback(async () => {
    if (!feedback?.unknown || inFlight.current) return;
    const intent = [...unsettled.values()].find((entry) => entry.requestRef === feedback.requestRef && entry.action === feedback.action);
    await send(intent ?? { scope: feedback.scope ?? '', context, action: feedback.action, requestRef: feedback.requestRef, fields: {} }, true);
  }, [feedback, context, send]);
  return { run, feedback, busy: busy || feedback?.unknown === true, verify };
}
export type Commands = ReturnType<typeof useCommands>;
export function listHref(params: URLSearchParams, updates: Record<string, string | null> = {}, hash = '') {
  const next = new URLSearchParams(params);
  if (
    next.has('life_work') &&
    !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(next.get('life_work') ?? '')
  )
    next.delete('life_work');
  if (updates.rule || updates.assign_domain || ('drawer' in updates && updates.drawer !== next.get('drawer'))) {
    for (const key of [...next.keys()])
      if (
        ['drawer', 'dtab', 'wview', 'catalog_query', 'catalog_filter'].includes(key) ||
        key.startsWith('life_')
      )
        next.delete(key);
  }
  for (const [key, value] of Object.entries(updates)) {
    if (value === null) next.delete(key);
    else next.set(key, value);
  }
  return `/collection/targets${next.size ? `?${next}` : ''}${hash ? `#${hash}` : ''}`;
}
export function returnFields(params: URLSearchParams) {
  return {
    return_domain: params.get('domain') ?? 'all',
    return_filter: params.get('filter') ?? '',
    return_sort: params.get('sort') ?? '',
  };
}
