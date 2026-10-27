import { ApiError, requestJson } from '../../shared/api';
import { bool, decimal, integer, json, list, nullable, object, parseAiSettings, parseComparison, parseSamples, text } from './aiSettingsModel';

const ROOT = '/api/local/ai-settings';
export const AI_SETTINGS_KEY = ['ai-settings'] as const;
export async function fetchAiSettings(signal?: AbortSignal) { return parseAiSettings(await requestJson(ROOT, { signal })); }
export async function fetchSamples(signal?: AbortSignal) { return parseSamples(await requestJson(`${ROOT}/writing-samples`, { signal })); }
export async function fetchComparison(reference: string, position = 0, signal?: AbortSignal) {
  return parseComparison(await requestJson(`${ROOT}/trials/${encodeURIComponent(reference)}?position=${position}`, { signal }));
}
const receipts = {
  control: object({ scopeKey: text, revision: integer, enabled: bool, commandRef: text }),
  egress: object({ eventRef: text, allowed: bool }),
  capabilities: object({ revisionRef: text }), prices: object({ revisionRef: text }),
  'task-configs': object({ taskKey: text, taskConfigRef: text, modelConfigRef: nullable(text), revision: integer, applied: bool }),
  'skills/import': object({ skillRef: text, versionRef: text, versionNo: integer, packageSha256: text, reused: bool }),
  'skills/activate': object({ versionRef: text, taskConfigRef: text, taskRevision: integer, bindingRevision: integer, approvalSource: text }),
  'skills/switch': object({ versionRef: text, taskConfigRef: text, taskRevision: integer, bindingRevision: integer, approvalSource: text }),
  'prompts/save': object({ versionRef: text, versionNo: integer }),
  'prompts/activate': object({ versionRef: text, taskConfigRef: text, taskRevision: integer }),
  trials: object({ comparisonRef: text, baselineBatchRef: nullable(text), candidateBatchRef: text, inputsSha256: text, accepted: bool }),
  batches: object({ batchRef: text, inputsSha256: text, itemCount: integer, accepted: bool }),
  'batches/cancel': object({ batchRef: text, revision: integer, state: text, cancelRequested: bool }),
};
export async function command<K extends keyof typeof receipts>(route: K, payload: object): Promise<ReturnType<typeof receipts[K]>> {
  const value = await requestJson(`${ROOT}/${route}`, { method: 'POST', body: payload });
  // Each route owns its closed decoder; the generic return preserves that route's shape.
  return receipts[route](value) as ReturnType<typeof receipts[K]>;
}
const catalog = object({ contract: text, sdkVersion: text, providers: list(text), total: integer, offset: integer,
  limit: integer, hasMore: bool, presets: list(object({ provider: text, modelId: text, name: text, api: text,
    sourceVersion: text, supported: bool, reasons: list(text), limits: object({ contextTokens: integer,
      maxOutputTokens: integer, maxTimeoutSeconds: nullable(integer), thinkingLevels: list(text) }),
    compat: json, thinkingLevelMap: json, outputMode: text, strictSchema: bool,
    prices: object({ input: nullable(decimal), output: nullable(decimal), cachedRead: nullable(decimal), cachedWrite: nullable(decimal) }),
    operatorFields: list(text), unknownFields: list(text) })) });
export async function fetchCatalog(api: string, modelId: string, signal?: AbortSignal) {
  const query = new URLSearchParams({ api, modelId, limit: '100' });
  return catalog(await requestJson(`${ROOT}/catalog?${query}`, { signal }));
}
export const parseSkill = object({ versionRef: text, skillRef: text, name: text, versionNo: integer,
  description: text, body: text, frontmatter: json, disableModelInvocation: bool,
  loaderVersion: text, packageSha256: text, sourceSha256: text,
  files: list(object({ path: text, sha256: text, bytes: integer, content: nullable(text), contentBase64: nullable(text), kind: text })) });
export async function fetchSkill(reference: string, signal?: AbortSignal) {
  return parseSkill(await requestJson(`${ROOT}/skills/${encodeURIComponent(reference)}`, { signal }));
}
const prompts = object({ taskKey: text, revision: integer, activeVersionRef: nullable(text), versions: list(object({
  versionRef: text, versionNo: integer, systemText: text, instructionText: text, createdAt: text,
})) });
export async function fetchPrompts(task: string, signal?: AbortSignal) {
  return prompts(await requestJson(`${ROOT}/prompts/${encodeURIComponent(task)}`, { signal }));
}
const batchRead = object({ batch: object({ batchRef: text, taskKey: text, taskConfigRef: text, kind: text,
  itemCount: integer, createdAt: text, state: text, revision: integer, cancelRequested: bool, stopReason: nullable(text), lastRoundNo: integer, finishedAt: nullable(text) }),
  items: list(object({ itemRef: text, position: integer, state: text, attemptCount: integer, usable: nullable(bool), invocationRef: nullable(text), failureCode: nullable(text), waitReason: nullable(text), nextAttemptAt: nullable(text), leaseExpiresAt: nullable(text), finishedAt: nullable(text) })),
  rounds: list(object({ roundRef: text, roundNo: integer, attempted: integer, failed: integer, usable: integer, consecutiveWithoutUsable: integer, brakeReason: nullable(text) })), fromPosition: integer, pageSize: integer });
export async function fetchBatch(reference: string, fromPosition: number, signal?: AbortSignal) {
  return batchRead(await requestJson(`${ROOT}/batches/${encodeURIComponent(reference)}?fromPosition=${fromPosition}`, { signal }));
}
const history = object({ contract: text, pageSize: integer, hasMore: bool, nextCursor: nullable(text), items: list(object({
  invocationRef: text, taskKey: text, taskConfigRef: nullable(text), modelConfigRef: nullable(text), batchRef: nullable(text), roundNo: nullable(integer),
  operation: text, state: text, connectionVersionRef: text, modelId: nullable(text), connectionName: text,
  usageState: text, dispatchState: text, budgetDay: text, inputTokens: nullable(decimal), outputTokens: nullable(decimal),
  measuredTokens: nullable(decimal), cachedInputTokens: nullable(decimal), cacheWriteTokens: nullable(decimal),
  reasoningTokens: nullable(decimal), quotaHeldTokens: decimal, legacyChargedTokens: nullable(decimal),
  quotePicoUsd: nullable(decimal), failureCode: nullable(text), createdAt: text, finishedAt: nullable(text) })) });
export async function fetchHistory(taskKey: string, before: string, signal?: AbortSignal) {
  const query = new URLSearchParams(); if (taskKey) query.set('taskKey', taskKey); if (before) query.set('before', before);
  const page = history(await requestJson(`${ROOT}/invocations?${query}`, { signal }));
  if (page.contract !== 'ai-invocation-history.v1' || page.pageSize !== 50 || page.items.length > 50
    || (page.hasMore && (!page.nextCursor || page.nextCursor !== page.items.at(-1)?.invocationRef))) {
    throw new ApiError({ kind: 'invalid_response', status: 200 });
  }
  return page;
}
