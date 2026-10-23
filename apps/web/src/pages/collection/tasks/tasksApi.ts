import { ApiError, isRecord, requestJson } from '../../../shared/api';

export function object(value: unknown): Record<string, unknown> {
  if (!isRecord(value)) throw new ApiError({ kind: 'invalid_response', status: 200 });
  return value;
}
export function text(value: unknown): string {
  if (typeof value !== 'string') throw new ApiError({ kind: 'invalid_response', status: 200 });
  return value;
}
export function number(value: unknown): number {
  if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 0) throw new ApiError({ kind: 'invalid_response', status: 200 });
  return value;
}
function bool(value: unknown): boolean {
  if (typeof value !== 'boolean') throw new ApiError({ kind: 'invalid_response', status: 200 });
  return value;
}
function optional<T>(value: unknown, parse: (value: unknown) => T): T | null { return value == null ? null : parse(value); }
function array<T>(value: unknown, parse: (value: unknown) => T): T[] {
  if (!Array.isArray(value)) throw new ApiError({ kind: 'invalid_response', status: 200 });
  return value.map(parse);
}
const uuidPattern = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
export function uuid(value: unknown): string { const v = text(value); if (!uuidPattern.test(v)) throw new ApiError({ kind: 'invalid_response', status: 200 }); return v; }
export function parseRun(value: unknown) {
  const v = object(value);
  const legacy = bool(v.legacy);
  if (!legacy && (v.control_intent == null || v.comment_mode == null || v.revision == null)) throw new ApiError({kind:'invalid_response', status:200});
  return {
    run_ref: uuid(v.run_ref), domain_ref: uuid(v.domain_ref), name: text(v.name), domain_name: text(v.domain_name), source_kind: text(v.source_kind),
    legacy, ended: bool(v.ended), control_intent: optional(v.control_intent, text), phase: text(v.phase), comment_mode: optional(v.comment_mode, text), comment_target: optional(v.comment_target, number),
    created_at: text(v.created_at), updated_at: text(v.updated_at), revision: optional(v.revision, number), work_limit: number(v.work_limit),
    scope_frozen_at: optional(v.scope_frozen_at, text), discovery_reason: optional(v.discovery_reason, text), latest_delivery_at: optional(v.latest_delivery_at, text),
    available_details: optional(v.available_details, number), delivered_comments: optional(v.delivered_comments, number), new_comments: optional(v.new_comments, number), reused_details: optional(v.reused_details, number), selected_works: optional(v.selected_works, number),
    needs_attention: optional(v.needs_attention, bool), live_leases: optional(v.live_leases, number), domain_status: optional(v.domain_status, text), queued_orders: optional(v.queued_orders, number),
    received_comments: optional(v.received_comments, number), cumulative_comments: optional(v.cumulative_comments, number), completed_discussions: optional(v.completed_discussions, number), gap_works: optional(v.gap_works, number),
    stations: v.stations == null ? [] : array(v.stations, value => { const s = object(value); return { stationRef: uuid(s.stationRef), name: text(s.name) }; }),
  };
}
export type Run = ReturnType<typeof parseRun>;
export function parseRunList(value: unknown) { const v = object(value); return { items: array(v.items, parseRun), total: number(v.total), page: number(v.page), pageSize: number(v.pageSize) }; }
export function listRuns(params: URLSearchParams, signal?: AbortSignal) {
  const q = new URLSearchParams();
  if (params.get('domain') && params.get('domain') !== 'all') q.set('domainRef', params.get('domain') ?? '');
  for (const key of ['filter', 'q', 'page']) if (params.get(key)) q.set(key, params.get(key) ?? '');
  return requestJson(`/api/collection/runs?${q}`, { signal }).then(parseRunList);
}
export function readRun(id: string, domain: string, signal?: AbortSignal) { return requestJson(`/api/collection/runs/${uuid(id)}?domainRef=${encodeURIComponent(domain)}`, { signal }).then(value => { const run = parseRun(value); if (run.run_ref !== id || run.domain_ref !== domain) throw new ApiError({ kind: 'invalid_response', status: 200 }); return run; }); }
export function parseDomains(value: unknown) { const v = object(value); if (v.readable !== true) throw new ApiError({kind:'invalid_response',status:200}); return array(v.domains, value => { const v = object(value); return { domainRef: uuid(v.domainRef), name: text(v.name), status: text(v.status) }; }); }
export function readDomains(signal?: AbortSignal) { return requestJson('/api/collection/workspace/domains', { signal }).then(parseDomains); }
export function parseTargets(value: unknown) { const v = object(value); return { total: number(v.total), items: array(v.items, value => { const t = object(value); return { target_ref: uuid(t.target_ref), name: text(t.name), target_kind: text(t.target_kind) }; }) }; }
export function readTargets(domain: string, q: string, page: number, selected: string, signal?: AbortSignal) { return requestJson(`/api/collection/runs/targets?${new URLSearchParams({ domainRef: domain, q, page: String(page), selected })}`, { signal }).then(parseTargets); }
export function parseWorks(value: unknown) {
  const v = object(value); return { runRef: uuid(v.runRef), domainRef: uuid(v.domainRef), total: number(v.total), items: array(v.items, value => { const w = object(value); const checkpoint = w.checkpoint == null ? null : object(w.checkpoint); return { publicRef: uuid(w.publicRef), ordinal: optional(w.ordinal, number), title: optional(w.title, text), creator: optional(w.creator, text), gapReason: optional(w.gapReason, text), commentsComplete: checkpoint ? optional(checkpoint.commentsComplete, bool) : null }; }) };
}
export function readWorks(id: string, domain: string, page: number, signal?: AbortSignal) { return requestJson(`/api/collection/runs/${uuid(id)}/works?${new URLSearchParams({ domainRef: domain, page: String(page) })}`, { signal }).then(value => { const v = parseWorks(value); if (v.runRef !== id || v.domainRef !== domain) throw new ApiError({ kind: 'invalid_response', status: 200 }); return v; }); }
export function parseCreationReceipt(value: unknown) { const v = object(value); return { runRef: uuid(v.runRef), domainRef: uuid(v.domainRef) }; }
export function known(value: number | null): string { return value === null ? '未知' : String(value); }
export function moment(value: string): string { const date = new Date(value); return Number.isNaN(date.valueOf()) ? value : new Intl.DateTimeFormat('zh-CN', { timeZone: 'Asia/Shanghai', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', hour12: false }).format(date); }
export function runStatus(r: Run): string {
  if (r.control_intent === 'stopped') return '已停止';
  if (r.control_intent === 'paused') return '已暂停';
  if (r.phase === 'blocked') return '需处理';
  if (r.ended) return r.needs_attention ? '本轮结束，有缺口' : '本轮已结束';
  if (r.domain_status === 'paused') return '领域暂停';
  return r.live_leases !== null && r.live_leases > 0 ? '执行中' : '等待派发';
}
/** 评论范围的写法；`sample` 是两处列表原有的不同短语（台账写「总样本」、抽屉写「评论样本」）。 */
export function commentModeLabel(r: Pick<Run, 'comment_mode' | 'comment_target'>, sample: string): string {
  if (r.comment_mode === 'all_public') return '尽量采全，含回复';
  if (r.comment_mode === 'up_to') return r.comment_target === null ? '每篇采到指定条数，含回复' : `每篇采到 ${r.comment_target} 条就停，含回复`;
  return sample;
}
export function sourceLabel(kind: string): string { return ({ temporary_keyword: '临时关键词', observation_target: '观察目标', selected_works: '语料已选作品', archive: '建档', patrol: '本轮巡查', supplement: '显式补采' } as Record<string, string>)[kind] ?? '既有采集'; }
export function discoveryLabel(reason: string | null): string { return reason === null ? '等待发现结果' : ({ surface_ended: '本轮可见作品已发现完毕', maximum_quota: '已达到本次作品上限', accepted_discovery_scope: '已固定本次发现结果', person_selected_exact_set: '沿用勾选的作品集合', discovery_execution_stopped: '发现未完成，需要处理' } as Record<string, string>)[reason] ?? '发现原因见执行诊断'; }
export function errorText(error: unknown): string {
  if (error instanceof ApiError) {
    if (!error.answered) return '服务器尚未给出可核对的结果';
    const code = error.failure.kind === 'http' ? error.failure.code : null;
    return ({ domain_not_active: '本次领域尚未运行，请先恢复领域', target_outside_domain_or_stopped: '观察目标已停用或不属于本次领域，请重新选择', works_outside_domain: '所选作品不属于本次领域，请回到语料重新选择', invalid_work_limit: '作品上限须为 1–200 篇', invalid_keyword: '请填写 1–80 字的临时关键词', invalid_work_set: '所选作品集合无效，请回到语料重新选择', unsupported_ranking: '当前排序不可用，请重新选择', ranking_not_applicable: '该来源不使用搜索排序', collection_run_conflict: '任务状态或请求内容已改变，请重新读取后操作', collection_run_not_found: '当前领域下找不到这次采集', run_ended: '本轮已结束，成果保留', run_stopped: '本次采集已停止，成果保留', invalid_comment_target: '条数须为 1–1000 的整数', comment_target_not_applicable: '只有「指定条数」才填条数' } as Record<string, string>)[code ?? ''] ?? '请求未被接受，请重新核对当前范围';
  }
  return '读取或提交没有完成，请保留当前输入后重试';
}
