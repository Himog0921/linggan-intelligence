import { isRecord, requestJson } from '../../../shared/api';
function record(v: unknown) {
  if (!isRecord(v)) throw new Error('历史任务格式不符');
  return v;
}
function text(v: unknown): string {
  if (typeof v !== 'string') throw new Error('历史任务文字格式不符');
  return v;
}
function number(v: unknown): number {
  if (typeof v !== 'number' || !Number.isFinite(v)) throw new Error('历史任务计数格式不符');
  return v;
}
function optional<T>(v: unknown, decode: (value: unknown) => T): T | null {
  return v === null ? null : decode(v);
}
function array<T>(v: unknown, decode: (value: unknown) => T): T[] {
  if (!Array.isArray(v)) throw new Error('历史任务列表格式不符');
  return v.map(decode);
}
function task(value: unknown) {
  const t = record(value);
  return {
    taskId: text(t.taskId),
    target: text(t.target),
    source: text(t.source),
    platform: text(t.platform),
    pageType: text(t.pageType),
    capabilities: text(t.capabilities),
    domains: text(t.domains),
    created: text(t.created),
    sequence: optional(t.sequence, number),
    state: text(t.state),
    stateClass: text(t.stateClass),
    stateNote: text(t.stateNote),
    attemptId: optional(t.attemptId, text),
    attemptStarted: optional(t.attemptStarted, text),
    packageRef: optional(t.packageRef, text),
    packageKind: optional(t.packageKind, text),
    packageAccepted: optional(t.packageAccepted, text),
    receiptRef: optional(t.receiptRef, text),
    receiptReceived: optional(t.receiptReceived, text),
    receiptLabel: text(t.receiptLabel),
    admission: text(t.admission),
    effect: text(t.effect),
    effectCode: text(t.effectCode),
    failure: optional(t.failure, text),
    failureAt: optional(t.failureAt, text),
  };
}
function delivery(value: unknown) {
  const s = record(value);
  return {
    sessionRef: text(s.sessionRef),
    target: text(s.target),
    content: text(s.content),
    platform: text(s.platform),
    label: text(s.label),
    slug: text(s.slug),
    note: text(s.note),
    className: text(s.class),
    prepared: number(s.prepared),
    delivered: number(s.delivered),
    observed: text(s.observed),
    closed: optional(s.closed, text),
    closedReason: optional(s.closedReason, text),
    reasonCode: text(s.reasonCode),
  };
}
function frozen(value: unknown) {
  const w = record(value);
  return {
    workRef: text(w.workRef),
    taskId: optional(w.taskId, text),
    target: text(w.target),
    lane: text(w.lane),
    created: text(w.created),
    leaseRef: optional(w.leaseRef, text),
    leaseLabel: text(w.leaseLabel),
    taskState: optional(w.taskState, text),
    resources: array(w.resources, (value) => {
      const r = record(value);
      return { label: text(r.label), ref: optional(r.ref, text), state: text(r.state) };
    }),
  };
}
export function decodeLegacyTaskDiagnostics(value: unknown) {
  const r = record(value);
  return {
    limit: number(r.limit),
    active: number(r.active),
    expired: number(r.expired),
    awaiting: optional(r.awaiting, number),
    unverified: optional(r.unverified, number),
    tasks: array(r.tasks, task),
    delivery: optional(r.delivery, (v) => array(v, delivery)),
    frozen: optional(r.frozen, (v) => array(v, frozen)),
  };
}
export type LegacyTask = ReturnType<typeof task>;
export type LegacyTaskDiagnosticsData = ReturnType<typeof decodeLegacyTaskDiagnostics>;
export async function readLegacyTaskDiagnostics(signal: AbortSignal) {
  return decodeLegacyTaskDiagnostics(
    await requestJson('/api/collection/workspace/task-diagnostics', { signal }),
  );
}
