import { decode, list, obj, optBool, optNum, optObj, optStr, str, strList, type Json } from '../../../shared/decode';
import { countMap, parsePolicy, type MethodManifest } from './methodDtos';
import { parseCommentKey } from './commentDtos';
import type { CommentKey, StudyPage } from './studyApi';

const listCounts = ['requestedWorkCount', 'coveredWorkCount', 'workCount', 'targetCount', 'succeededCount', 'noSignalCount', 'needsContextCount', 'failedCount', 'pendingCount', 'cancelledCount', 'excludedCount', 'pendingResolutionCount', 'pendingPairCount', 'controlVersion'] as const;
export type StudyRun = Record<typeof listCounts[number], number | null> & { runRef: string; createdAt: string | null; finishedAt: string | null; methodName: string | null; state: string; origin: string | null; selectionContract: string | null; dispatchState: string | null; dispatchReason: string | null; recoverySourceRunRef: string | null; limits: { commentBudget: number | null; tokenLimit: number | null } | null };
const semanticCounts = ['targetCount', 'succeededTargetCount', 'noSignalTargetCount', 'needsContextTargetCount', 'failedTargetCount', 'signalCount', 'currentSignalCount'] as const;
const knowledgeCounts = ['assignedSignalCount', 'linkedProblemCount', 'pendingResolutionCount', 'pendingPairCount', 'deferredNovelCount', 'deferredAmbiguousCount', 'deferredContextCount', 'retrievalIncompleteCount', 'budgetStoppedCount', 'protocolRejectedCount', 'failedResolutionCount'] as const;
const costCounts = ['requestCount', 'dispatchedRequestCount', 'usageKnownRequestCount', 'usageUnknownRequestCount', 'totalInputTokens', 'totalOutputTokens', 'knownInputTokens', 'knownOutputTokens', 'chargedTokens'] as const;
const counts = <K extends string>(r: Json, keys: readonly K[]): Record<K, number | null> => Object.fromEntries(keys.map(key => [key, optNum(r, key)])) as Record<K, number | null>;
export interface RunDetail {
  runRef: string; createdAt: string | null; policyRef: string | null; method: { name: string | null; hash: string | null; manifest: MethodManifest | null } | null;
  scope: { requestedWorkRefs: string[] | null; selectionMode: string | null; exclusionCounts: Record<string, number | null> | null } | null;
  limits: { commentBudget: number | null; tokenLimit: number | null } | null;
  requestedWorkCount: number | null; coveredWorkCount: number | null; workCount: number | null;
  semanticSummary: Record<typeof semanticCounts[number], number | null> | null;
  knowledgeSummary: Record<typeof knowledgeCounts[number], number | null> | null;
  costSummary: (Record<typeof costCounts[number], number | null> & { recordingState: string | null }) | null;
}
export interface ModelRequest {
  invocationRef: string | null; runRef: string | null; stage: string | null; state: string | null; dispatched: boolean | null; usageKnown: boolean | null;
  inputTokens: number | null; outputTokens: number | null; chargedTokens: number | null; reservedTokens: number | null; elapsedMs: number | null; attemptOrdinal: number | null;
  failureCode: string | null; createdAt: string | null; dispatchStartedAt: string | null; modelConfigRef: string | null; sourceState: string | null; recordingState: string | null;
  modelIdentity: { modelId: string | null; modelRef: string | null; connectionVersionRef: string | null } | null; requestManifest: unknown;
}
export interface TargetAttempt {
  usageKnown: boolean | null; inputTokens: number | null; outputTokens: number | null; chargedTokens: number | null; reservedTokens: number | null;
  state: string | null; providerFailureCode: string | null; rejectionCode: string | null; retryStrategy: string | null; responseLimit: string | null; stage: string | null;
  httpStatus: string | null; receivedBytes: number | null; terminalReceived: boolean | null;
}
export interface RunTarget {
  targetRef: string; commentKey: CommentKey | null; sourceState: string | null; state: string; observationRole: string | null; commentText: string | null; researchText: string | null;
  contextState: string | null; dependencyState: string | null; signalCount: number | null; resolutionState: string | null; modelReason: string | null; exclusionReason: string | null; terminalReason: string | null; attemptCount: number | null;
  workContext: { sources: { kind: string | null; text: string | null }[] } | null; parentContext: { state: string | null; researchText: string | null } | null; latestAttempt: TargetAttempt | null;
}
export interface RunSignal { signalRef: string; targetRef: string; kind: string; sourceState: string | null; observationRole: string | null; proposition: string | null; evidence: string | null; resolutionState: string | null; eligibilityState: string | null; eligibilityReason: string | null; resolvedProblemRef: string | null; pairOutcomes: { state: string | null; decisionReason: string | null; selection: { recallRank: number | null; admissibleRank: number | null } | null }[] }
export function parseRun(r: Json): StudyRun {
  return { ...counts(r, listCounts), limits: optObj(r, 'limits') ? { commentBudget: optNum(obj(r, 'limits'), 'commentBudget'), tokenLimit: optNum(obj(r, 'limits'), 'tokenLimit') } : null, runRef: str(r, 'runRef'), createdAt: optStr(r, 'createdAt'), finishedAt: optStr(r, 'finishedAt'), methodName: optStr(r, 'methodName'), state: str(r, 'state'), origin: optStr(r, 'origin'), selectionContract: optStr(r, 'selectionContract'), dispatchState: optStr(r, 'dispatchState'), dispatchReason: optStr(r, 'dispatchReason'), recoverySourceRunRef: optStr(r, 'recoverySourceRunRef') };
}
export function parseRuns(row: Json): { runs: StudyRun[]; page: StudyPage } { return decode(row, r => ({ runs: list(r, 'runs', parseRun), page: { nextCursor: optStr(obj(r, 'page'), 'nextCursor') } })); }
export function parseRunDetail(row: Json): RunDetail {
  return decode(row, root => { const r = obj(root, 'run'), method = optObj(r, 'method'), scope = optObj(r, 'scope'), limits = optObj(r, 'limits'), semantic = optObj(r, 'semanticSummary'), knowledge = optObj(r, 'knowledgeSummary'), cost = optObj(r, 'costSummary'); return { runRef: str(r, 'runRef'), createdAt: optStr(r, 'createdAt'), policyRef: optStr(r, 'policyRef'), method: method ? { name: optStr(method, 'name'), hash: optStr(method, 'hash'), manifest: parsePolicy({ policyRef: str(r, 'runRef'), methodManifest: method.manifest }).methodManifest } : null, scope: scope ? { requestedWorkRefs: scope.requestedWorkRefs == null ? null : strList(scope, 'requestedWorkRefs'), selectionMode: optStr(scope, 'selectionMode'), exclusionCounts: optObj(scope, 'exclusionCounts') ? countMap(obj(scope, 'exclusionCounts')) : null } : null, limits: limits ? { commentBudget: optNum(limits, 'commentBudget'), tokenLimit: optNum(limits, 'tokenLimit') } : null, requestedWorkCount: optNum(r, 'requestedWorkCount'), coveredWorkCount: optNum(r, 'coveredWorkCount'), workCount: optNum(r, 'workCount'), semanticSummary: semantic ? counts(semantic, semanticCounts) : null, knowledgeSummary: knowledge ? counts(knowledge, knowledgeCounts) : null, costSummary: cost ? { ...counts(cost, costCounts), recordingState: optStr(cost, 'recordingState') } : null }; });
}
function parseRequest(r: Json): ModelRequest {
  const identity = optObj(r, 'modelIdentity');
  return { invocationRef: optStr(r, 'invocationRef'), runRef: optStr(r, 'runRef'), stage: optStr(r, 'stage'), state: optStr(r, 'state'), dispatched: optBool(r, 'dispatched'), usageKnown: optBool(r, 'usageKnown'), inputTokens: optNum(r, 'inputTokens'), outputTokens: optNum(r, 'outputTokens'), chargedTokens: optNum(r, 'chargedTokens'), reservedTokens: optNum(r, 'reservedTokens'), elapsedMs: optNum(r, 'elapsedMs'), attemptOrdinal: optNum(r, 'attemptOrdinal'), failureCode: optStr(r, 'failureCode'), createdAt: optStr(r, 'createdAt'), dispatchStartedAt: optStr(r, 'dispatchStartedAt'), modelConfigRef: optStr(r, 'modelConfigRef'), sourceState: optStr(r, 'sourceState'), recordingState: optStr(r, 'recordingState'), modelIdentity: identity ? { modelId: optStr(identity, 'modelId'), modelRef: optStr(identity, 'modelRef'), connectionVersionRef: optStr(identity, 'connectionVersionRef') } : null, requestManifest: r.requestManifest ?? null };
}
export const parseRequestDetail = (row: Json) => decode(row, root => parseRequest(obj(root, 'request')));
export function parseRequestPage(row: Json): { runRef: string; requests: ModelRequest[]; page: StudyPage } { return decode(row, r => ({ runRef: str(r, 'runRef'), requests: list(r, 'requests', parseRequest), page: { nextCursor: optStr(obj(r, 'page'), 'nextCursor') } })); }
function parseAttempt(r: Json): TargetAttempt { return { usageKnown: optBool(r, 'usageKnown'), inputTokens: optNum(r, 'inputTokens'), outputTokens: optNum(r, 'outputTokens'), chargedTokens: optNum(r, 'chargedTokens'), reservedTokens: optNum(r, 'reservedTokens'), state: optStr(r, 'state'), providerFailureCode: optStr(r, 'providerFailureCode'), rejectionCode: optStr(r, 'rejectionCode'), retryStrategy: optStr(r, 'retryStrategy'), responseLimit: optStr(r, 'responseLimit'), stage: optStr(r, 'stage'), httpStatus: typeof r.httpStatus === 'number' ? String(r.httpStatus) : optStr(r, 'httpStatus'), receivedBytes: optNum(r, 'receivedBytes'), terminalReceived: optBool(r, 'terminalReceived') }; }
function parseTarget(r: Json): RunTarget {
  const key = optObj(r, 'commentKey'), work = optObj(r, 'workContext'), parent = optObj(r, 'parentContext'), attempt = optObj(r, 'latestAttempt');
  return { targetRef: str(r, 'targetRef'), commentKey: key ? parseCommentKey(key) : null, sourceState: optStr(r, 'sourceState'), state: str(r, 'state'), observationRole: optStr(r, 'observationRole'), commentText: optStr(r, 'commentText'), researchText: optStr(r, 'researchText'), contextState: optStr(r, 'contextState'), dependencyState: optStr(r, 'dependencyState'), signalCount: optNum(r, 'signalCount'), resolutionState: optStr(r, 'resolutionState'), modelReason: optStr(r, 'modelReason'), exclusionReason: optStr(r, 'exclusionReason'), terminalReason: optStr(r, 'terminalReason'), attemptCount: optNum(r, 'attemptCount'), workContext: work ? { sources: list(work, 'sources', source => ({ kind: optStr(source, 'kind'), text: optStr(source, 'text') })) } : null, parentContext: parent ? { state: optStr(parent, 'state'), researchText: optStr(parent, 'researchText') } : null, latestAttempt: attempt ? parseAttempt(attempt) : null };
}
export function parseTargets(row: Json): { runRef: string; targets: RunTarget[]; page: StudyPage } { return decode(row, r => ({ runRef: str(r, 'runRef'), targets: list(r, 'targets', parseTarget), page: { nextCursor: optStr(obj(r, 'page'), 'nextCursor') } })); }
function parseSignal(r: Json): RunSignal {
  return { signalRef: str(r, 'signalRef'), targetRef: str(r, 'targetRef'), kind: str(r, 'kind'), sourceState: optStr(r, 'sourceState'), observationRole: optStr(r, 'observationRole'), proposition: optStr(r, 'proposition'), evidence: optStr(r, 'evidence'), resolutionState: optStr(r, 'resolutionState'), eligibilityState: optStr(r, 'eligibilityState'), eligibilityReason: optStr(r, 'eligibilityReason'), resolvedProblemRef: optStr(r, 'resolvedProblemRef'), pairOutcomes: list(r, 'pairOutcomes', pair => { const selection = optObj(pair, 'selection'); return { state: optStr(pair, 'state'), decisionReason: optStr(pair, 'decisionReason'), selection: selection ? { recallRank: optNum(selection, 'recallRank'), admissibleRank: optNum(selection, 'admissibleRank') } : null }; }) };
}
export function parseSignals(row: Json): { runRef: string; signals: RunSignal[]; page: StudyPage } { return decode(row, r => ({ runRef: str(r, 'runRef'), signals: list(r, 'signals', parseSignal), page: { nextCursor: optStr(obj(r, 'page'), 'nextCursor') } })); }
export const parseControl = (row: Json) => decode(row, root => { const data = obj(root, 'data'); return { runRef: str(data, 'runRef'), controlVersion: optNum(data, 'controlVersion'), dispatchState: str(data, 'dispatchState'), dispatchReason: optStr(data, 'dispatchReason') }; });
export const parseRecovery = (row: Json) => decode(row, root => ({ requestRef: str(root, 'requestRef'), outcome: str(root, 'outcome'), runRef: optStr(root, 'runRef'), targetCount: optNum(root, 'targetCount') }));
