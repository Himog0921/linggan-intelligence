import { ApiError, isRecord } from '../../shared/api';

export const CREATOR_RESEARCH_GUIDANCE = '可在作品与作者详情发起研究';

type Decoder<T> = (value: unknown) => T;
type Fields = Record<string, Decoder<unknown>>;
type Decoded<S extends Fields> = { [K in keyof S]: ReturnType<S[K]> };
function fail(): never { throw new ApiError({ kind: 'invalid_response', status: 200 }); }
export const text: Decoder<string> = value => typeof value === 'string' ? value : fail();
export const bool: Decoder<boolean> = value => typeof value === 'boolean' ? value : fail();
export const integer: Decoder<number> = value => typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 ? value : fail();
export const decimal: Decoder<string> = value => typeof value === 'string' && /^(0|[1-9]\d*)$/.test(value) ? value : fail();
export function nullable<T>(decode: Decoder<T>): Decoder<T | null> { return value => value === null ? null : decode(value); }
export function optional<T>(decode: Decoder<T>): Decoder<T | undefined> { return value => value === undefined ? undefined : decode(value); }
export function list<T>(decode: Decoder<T>): Decoder<T[]> { return value => Array.isArray(value) ? value.map(decode) : fail(); }
export function object<S extends Fields>(fields: S): Decoder<Decoded<S>> {
  return value => {
    if (!isRecord(value) || Object.keys(value).some(key => !Object.hasOwn(fields, key))) return fail();
    // Every property is decoded before this generic mapped object crosses the boundary.
    return Object.fromEntries(Object.entries(fields).map(([key, decode]) => [key, decode(value[key])])) as Decoded<S>;
  };
}
function record<T>(decode: Decoder<T>): Decoder<Record<string, T>> {
  return value => isRecord(value) && !Object.keys(value).some(key => ['__proto__', 'prototype', 'constructor'].includes(key))
    ? Object.fromEntries(Object.entries(value).map(([key, entry]) => [key, decode(entry)])) : fail();
}
export type Json = null | boolean | number | string | Json[] | { [key: string]: Json };
export const json: Decoder<Json> = value => {
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return value;
  if (typeof value === 'number') return Number.isFinite(value) ? value : fail();
  if (Array.isArray(value)) return value.map(json);
  return record(json)(value);
};
export type Budget = { mode: 'unconfigured' | 'unlimited' } | { mode: 'capped'; tokens: string };
const budget: Decoder<Budget> = value => {
  if (!isRecord(value)) return fail();
  if (value.mode === 'capped') {
    const row = object({ mode: text, tokens: decimal })(value);
    return { mode: 'capped', tokens: row.tokens };
  }
  const row = object({ mode: text })(value);
  return row.mode === 'unconfigured' || row.mode === 'unlimited' ? { mode: row.mode } : fail();
};
export const limits = object({ inputTokens: integer, outputTokens: integer, timeoutSeconds: integer, attempts: integer, thinkingLevel: text });
const modelSelection = object({ modelConfigRef: text, modelRef: text, identityRef: nullable(text), modelId: text,
  connectionRef: text, connectionVersionRef: text, capabilityRef: nullable(text), limits });
const configuration = object({ contract: text, taskConfigRef: text, taskKey: text, executionContract: nullable(text),
  skillVersionRef: nullable(text), materialColumns: list(text), budget, origin: text, createdAt: text, model: nullable(modelSelection) });
const connection = object({ connectionRef: text, enabled: bool, revision: integer, versionRef: text, name: text,
  api: text, baseUrl: text, localEndpoint: bool, endpointRef: text });
const probe = object({ invocationRef: text, state: text, modelCallable: nullable(bool), finishedAt: nullable(text), failureCode: nullable(text) });
const capability = object({ capabilityRef: text, source: text, sourceVersion: text, probeInvocationRef: nullable(text),
  contextTokens: integer, maxOutputTokens: integer, maxTimeoutSeconds: integer, thinkingLevels: list(text), compat: record(json),
  thinkingLevelMap: record(nullable(text)), outputMode: text, strictSchema: bool });
const price = object({ priceRef: text, currency: text, source: text, sourceVersion: text,
  input: nullable(decimal), output: nullable(decimal), cachedRead: nullable(decimal), cachedWrite: nullable(decimal) });
const model = object({ identityRef: text, connectionRef: text, modelId: text, endpointRef: text, modelRef: nullable(text),
  selectionModelRef: nullable(text), connectionVersionRef: text, probe: nullable(probe), capability: nullable(capability), price: nullable(price) });
const control = object({ scope: text, taskKey: nullable(text), enabled: bool, revision: integer, actor: text, reason: text,
  commandRef: nullable(text), updatedAt: text, label: nullable(text), lifecycle: nullable(text), automaticBrake: nullable(bool) });
const egress = object({ eventRef: text, endpointRef: text, taskKey: text, materialClass: text, allowed: bool,
  actor: text, reason: text, createdAt: text });
const version = object({ versionRef: text, versionNo: integer, description: text, packageSha256: text,
  sourceSha256: text, disableModelInvocation: bool, createdAt: text, fileCount: integer });
const skill = object({ skillRef: text, name: text, versions: list(version) });
const task = object({ taskKey: text, label: text, lifecycle: text, requiresSkill: bool, skillAllowed: bool,
  handlerContract: nullable(text), revision: integer, configuration, bindingRevision: integer,
  activeSkillVersionRef: nullable(text), approvalSource: nullable(text), approvalReason: nullable(text) });
const trial = object({ comparisonRef: text, taskKey: text, baselineBatchRef: nullable(text), candidateBatchRef: text,
  baselineState: nullable(text), candidateState: text, candidateConfigRef: text, itemCount: integer, inputsSha256: text, createdAt: text });
const batch = object({ batchRef: text, taskKey: text, taskConfigRef: text, kind: text, createdAt: text,
  itemCount: integer, state: text, revision: integer, cancelRequested: bool, stopReason: nullable(text),
  lastRoundNo: integer, finishedAt: nullable(text), pending: integer, running: integer });
const instance = object({ instanceRef: text, generation: decimal, processId: integer, sourceRevision: text, state: text,
  observedState: text, concurrency: integer, startedAt: text, heartbeatAt: text, drainRequestedAt: nullable(text), drainedAt: nullable(text), inflight: integer });
const service: Decoder<ReturnType<typeof instance> | { state: string; observedState: string }> = value => {
  if (isRecord(value) && value.state === 'not_started') return object({ state: text, observedState: text })(value);
  return instance(value);
};
const usage = object({ taskKey: text, budgetDay: text, calls: decimal, runningCalls: decimal, failedCalls: decimal,
  measuredCalls: decimal, unknownCalls: decimal, notDispatchedCalls: decimal, measuredTokens: nullable(decimal),
  quotaHeldTokens: nullable(decimal), unknownHeldTokens: nullable(decimal), knownQuotePicoUsd: nullable(decimal),
  unpricedMeasuredCalls: decimal, cacheReadUnknownCalls: decimal, reasoningUnknownCalls: decimal });
const failure = object({ invocationRef: text, taskKey: text, batchRef: nullable(text), taskConfigRef: nullable(text),
  connectionVersionRef: text, state: text, failureCode: nullable(text), description: nullable(text), retryable: nullable(bool),
  countsAsFailure: nullable(bool), usageState: text, dispatchState: text, createdAt: text, finishedAt: nullable(text) });
const execution = object({ service, dailyUsage: list(usage), failures: list(failure), lateUsage: object({ receiptCount: decimal,
  knownInputTokens: nullable(decimal), knownOutputTokens: nullable(decimal), quotaReconciled: bool }) });
const resource = object({ contract: text, maxContextTokens: integer, maxOutputTokens: integer, maxTimeoutSeconds: integer,
  maxAttempts: integer, defaultConcurrency: integer, maxConcurrency: integer, roundSize: integer, maxBatchItems: integer,
  adapterDeadlineAllowanceSeconds: integer, receiptFinalizationAllowanceSeconds: integer, maxSkillFiles: integer,
  maxSkillFileBytes: integer, maxSkillTotalBytes: integer, maxRequestBytes: integer, maxResponseBytes: integer,
  maxTextBytes: integer, maxStreamBytes: integer, baseStreamBytes: integer, streamBytesPerOutputToken: integer,
  maxResponseSchemaBytes: integer, legacyInputTokens: integer, legacyOutputTokens: integer, legacyTimeoutSeconds: integer,
  legacyRequestBytes: integer, legacyTopicRequestBytes: integer, legacyCommentSchemaBytes: integer });
const apiSpec = object({ defaults: record(json), booleans: list(text), enums: record(list(text)), templates: list(text),
  numbers: record(object({ min: (v: unknown) => typeof v === 'number' && Number.isSafeInteger(v) ? v : fail(), max: integer })),
  outputModes: list(text), minBudgetThinkingTokens: optional(integer), minThinkingAnswerTokens: optional(integer) });
const protocol = object({ contract: text, wireVersion: text, sdkVersion: text, thinkingLevels: list(text), apis: record(apiSpec),
  legacyTopicContracts: list(text), editableConnectionApis: list(text) });
const settings = object({ contract: text, synthetic: bool, connections: list(connection), models: list(model),
  controls: list(control), egress: list(egress), skills: list(skill), tasks: list(task), configurations: list(configuration),
  trials: list(trial), execution, batches: list(batch), resources: resource, protocol });
export function parseAiSettings(value: unknown) {
  const data = settings(value);
  if (data.contract !== 'ai-settings.v1' || data.resources.contract !== 'ai-resource-limits.v1'
    || data.protocol.contract !== 'ai-adapter-protocol.v1') return fail();
  return data;
}
export type AiSettings = ReturnType<typeof parseAiSettings>;
export type Connection = ReturnType<typeof connection>;
export type AiModel = ReturnType<typeof model>;
export type Capability = ReturnType<typeof capability>;
export type Task = ReturnType<typeof task>;
export type Configuration = ReturnType<typeof configuration>;
export type Control = ReturnType<typeof control>;
export type ModelLimits = ReturnType<typeof limits>;
export type Skill = ReturnType<typeof skill>;
export const parseSamples = object({ state: text, samples: list(object({ previewRef: text, workRef: text, revisionRef: text,
  title: nullable(text), action: text, createdAt: text })), examined: nullable(integer), limit: integer });
const comparisonSide = object({ batchRef: text, taskConfigRef: text, state: text, itemState: text, attemptCount: integer,
  output: json, failureCode: nullable(text), executionContract: nullable(text), skillVersionRef: nullable(text), invocationRef: nullable(text) });
export const parseComparison = object({ comparisonRef: text, taskKey: text, position: integer, itemCount: integer, inputRef: record(json),
  inputsSha256: text, baseline: nullable(comparisonSide), candidate: comparisonSide, qualifiesForActivation: bool, sourceQualification: text });
export type Comparison = ReturnType<typeof parseComparison>;
export const TASK_STATE: Record<string, string> = { active: '已接入', pending: '尚未接入', frozen: '已停用，接入时间另定', retired: '已退役', legacy: '仅保留历史' };
export const MATERIAL: Record<string, string> = { manual_ocr_image: '本篇手动识别原图', synthetic_probe: '合成测试内容', model_catalog: '模型目录', note_material: '研究底稿', comment_material: '选中的评论和必要的父评论', public_author_bio: '公开作者简介', selected_writing_material: '本篇选中写作材料' };
export const THINKING: Record<string, string> = { off: '关闭思考', minimal: '最低', low: '轻量', medium: '标准', high: '深入', xhigh: '更深入', max: '最高' };
export const OUTPUT: Record<string, string> = { prompt_only: '按提示返回', json_object: 'JSON 对象', json_schema: '结构约束' };
export const BATCH_STATE: Record<string, string> = { open: '待领取或执行中', completed: '本批已结束', cancelled: '已取消', pending: '待领取', running: '执行中', succeeded: '成功', failed: '失败', skipped: '已跳过' };
export function count(value: string | number | null): string { return value === null ? '—' : typeof value === 'number' ? value.toLocaleString('zh-CN') : BigInt(value).toLocaleString('zh-CN'); }
export function budgetText(value: Budget): string { return value.mode === 'capped' ? `每日 ${count(value.tokens)} token` : value.mode === 'unlimited' ? '当前不设每日上限' : '额度待配置'; }
export function taskLabel(data: AiSettings, key: string): string { return data.tasks.find(task => task.taskKey === key)?.label ?? '未识别的工作'; }
export const BRAKE_REASON: Record<string, string> = { failure_ratio: '本轮失败比例严格超过 30%', no_usable_output: '连续两轮实际尝试但没有可用产出' };
export function controlStatus(control: Control): string {
  if (control.enabled) return '开关已启用';
  if (control.commandRef === null && control.revision === 0) return '初始未启用';
  if (control.actor === 'system' && control.reason.startsWith('automatic_brake:')) return '系统自动暂停';
  return control.actor === 'system' ? '系统关闭' : '人工暂停';
}
export function controlReason(control: Control): string {
  if (control.actor === 'system' && control.commandRef === null && control.revision === 0 && control.reason === 'not_enabled') return '尚无人启用';
  return control.actor === 'system' && control.reason.startsWith('automatic_brake:') ? BRAKE_REASON[control.reason.slice(16)] ?? control.reason : control.reason;
}
