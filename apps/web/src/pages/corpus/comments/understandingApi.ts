import { ApiError, isRecord, requestJson } from '../../../shared/api';
import { bool, decode, list, num, obj, optNum, optObj, optStr, str, strList } from '../../../shared/decode';
import { REASONS } from './researchModel';
import type { SelectedComment } from './studyApi';

const root = '/api/local/comment-research/v2';
function read(path: string, domain: string, fields: Record<string, string> = {}, signal?: AbortSignal) {
  return requestJson(`${root}${path}?${new URLSearchParams({ domainRef: domain, ...fields })}`, { signal });
}
function post(path: string, body: unknown) {
  return requestJson(root + path, { method: 'POST', body });
}
const selection = (entry: SelectedComment) => ({ workRef: entry.commentKey.workRef, sourceRef: entry.sourceRef, observationRole: entry.observationRole });

export const detailReasons: Record<string, string> = {
  ...REASONS,
  source_unavailable: '来源不可用', workAuthorUnknown: '作品作者未知', creatorVoice: '创作者本人评论',
  sourceRestricted: '来源受限', bodyUnavailable: '正文不可用', textNotResearchable: '评论文本不可研究',
  comment_l1_unavailable: 'L1 尚不可用', comment_catalog_too_large: '领域需求目录超过全量处理上限',
  comment_consolidation_no_pairs: '没有可比较的需求配对', unit_output_invalid: '单元输出无效',
  unit_truncated: '单元输出被截断', target_missing: '缺少这条评论的结果', target_invalid: '评论结果无效',
  target_all_signals_invalid: '各项信号均未通过原声检查',
};
export function why(code: string) { return detailReasons[code] ?? '请查看任务设置或执行记录'; }
export function commandError(error: unknown) {
  if (error instanceof ApiError) {
    if (!error.answered) return '回执未确认。请沿用同一请求重试。';
    if (isRecord(error.responseBody) && Array.isArray(error.responseBody.reasons)) {
      return error.responseBody.reasons.filter((value): value is string => typeof value === 'string').map(why).join('、');
    }
    if (isRecord(error.responseBody) && error.responseBody.error === 'ai_revision_conflict') return '材料或配置已变化，请重新预览。';
  }
  return '操作未完成，请检查 AI 设置和材料状态。';
}
export function uncertain(error: unknown) {
  return !(error instanceof ApiError) || !error.answered;
}

export async function previewSelection(domain: string, selected: SelectedComment[], rerun: boolean) {
  return decode(await post('/preview', { domainRef: domain, selections: selected.map(selection), rerun }), row => {
    if (str(row, 'domainRef') !== domain) throw new Error('预览范围不一致');
    const waiting = obj(row, 'waitingL1');
    return {
      knowledgeCutoff: str(row, 'knowledgeCutoff'), planHash: str(row, 'planHash'),
      taskConfigRef: str(row, 'taskConfigRef'), taskRevision: num(row, 'taskRevision'),
      selected: num(row, 'selectedCount'), researchable: num(row, 'researchableCount'),
      units: num(row, 'unitCount'), works: num(row, 'workCount'), calls: num(row, 'expectedModelCalls'),
      waiting: num(waiting, 'targetCount'), missingWorks: strList(waiting, 'workRefs'),
      researched: num(row, 'alreadyResearchedCount'), active: num(row, 'alreadyQueuedCount'),
      unresolved: num(row, 'dispatchUnresolvedCount'),
      exclusions: Object.keys(obj(row, 'excludedCounts')).map(code => ({ code, count: num(obj(row, 'excludedCounts'), code) })),
    };
  });
}
export type PreviewPlan = Awaited<ReturnType<typeof previewSelection>>;
export async function startSelection(domain: string, selected: SelectedComment[], rerun: boolean, plan: PreviewPlan, commandRef: string) {
  return decode(await post('/runs', {
    domainRef: domain, commandRef, selections: selected.map(selection), rerun,
    knowledgeCutoff: plan.knowledgeCutoff, expectedPlanHash: plan.planHash,
    taskConfigRef: plan.taskConfigRef, expectedTaskRevision: plan.taskRevision,
  }), row => ({ batchRef: str(row, 'batchRef'), unitCount: num(row, 'unitCount'), targetCount: num(row, 'targetCount') }));
}
export async function readRuns(domain: string, signal?: AbortSignal) {
  return decode(await read('/runs', domain, {}, signal), row => ({
    runs: list(row, 'runs', item => ({ batchRef: str(item, 'batchRef'), createdAt: str(item, 'createdAt'),
      state: str(item, 'state'), unitCount: num(item, 'unitCount'), cancelRequested: bool(item, 'cancelRequested') })),
    consolidationTaskRevision: num(row, 'consolidationTaskRevision'),
    consolidations: list(row, 'consolidations', item => ({ batchRef: str(item, 'batchRef'), createdAt: str(item, 'createdAt'),
      state: str(item, 'state'), failureCode: optStr(item, 'failureCode'), researchBatchRef: optStr(item, 'researchBatchRef'),
      redirects: optObj(item, 'output') ? optNum(obj(item, 'output'), 'redirects') : null })),
  }));
}
export async function readRun(domain: string, batch: string, signal?: AbortSignal) {
  return decode(await read(`/runs/${encodeURIComponent(batch)}`, domain, {}, signal), row => {
    if (str(row, 'batchRef') !== batch) throw new Error('运行回执不一致');
    const usage = obj(row, 'usage');
    return {
      cancelRequested: bool(row, 'cancelRequested'),
      retrySelections: list(row, 'retrySelections', item => ({ sourceRef: str(item, 'sourceRef'),
        observationRole: str(item, 'observationRole'), commentKey: { workRef: str(item, 'workRef'), commentExternalId: str(item, 'commentExternalId') } })),
      units: list(row, 'units', item => ({ unitRef: str(item, 'unitRef'), workRef: str(item, 'workRef'), state: str(item, 'state'),
        failureCode: optStr(item, 'failureCode'), targetCount: num(item, 'targetCount') })),
      targets: list(row, 'targets', item => ({ state: str(item, 'state'), failureCode: optStr(item, 'failureCode'), count: num(item, 'count') })),
      usage: { calls: num(usage, 'modelCalls'), input: num(usage, 'inputTokens'), output: num(usage, 'outputTokens') },
    };
  });
}
export async function cancelRun(domainRef: string, batchRef: string, commandRef: string) {
  return decode(await post('/cancel', { domainRef, batchRef, commandRef }), row => {
    if (str(row, 'batchRef') !== batchRef || !bool(row, 'cancelRequested')) throw new Error('取消回执未确认');
  });
}
export const startConsolidation = (domainRef: string, expectedTaskRevision: number, commandRef: string) => post('/consolidation', { domainRef, expectedTaskRevision, commandRef });
export const revokeRedirect = (domainRef: string, redirectRef: string, commandRef: string) => post('/redirects/revoke', { domainRef, redirectRef, commandRef });

function problemSummary(row: Record<string, unknown>) {
  return { problemRef: str(row, 'problemRef'), label: str(row, 'label'), state: str(row, 'state'), authors: num(row, 'authorCount') };
}
function catalogRow(row: Record<string, unknown>) {
  return { ...problemSummary(row), comments: num(row, 'commentCount'), works: num(row, 'workCount'), lastResearchedAt: optStr(row, 'lastResearchedAt') };
}
export async function readProblems(domain: string, signal?: AbortSignal) {
  return decode(await read('/problems', domain, {}, signal), row => list(row, 'problems', catalogRow));
}
export async function readProblem(domain: string, problem: string, signal?: AbortSignal) {
  return decode(await read(`/problems/${encodeURIComponent(problem)}`, domain, {}, signal), row => ({
    ...catalogRow(row), actor: optStr(row, 'actor'), goal: str(row, 'goal'), barrier: str(row, 'barrier'),
    context: optStr(row, 'context'), redirectedFrom: optStr(row, 'redirectedFrom'),
    evidence: list(row, 'evidence', item => ({ requestRef: str(item, 'requestRef'), workRef: str(item, 'workRef'), workTitle: str(item, 'workTitle'),
      commentExternalId: str(item, 'commentExternalId'), proposition: str(item, 'proposition'), quote: str(item, 'quote') })),
    history: list(row, 'history', item => ({ redirectRef: str(item, 'redirectRef'), from: str(item, 'fromLabel'), to: str(item, 'toLabel'),
      createdAt: str(item, 'createdAt'), revokedAt: optStr(item, 'revokedAt') })),
  }));
}
export async function readLatest(domain: string, workRef: string, sourceRef: string, signal?: AbortSignal) {
  const reply = await read('/latest', domain, { workRef, sourceRef }, signal);
  if (reply === null) return null;
  return decode(reply, row => {
    const result = optObj(row, 'result'), context = result && optObj(result, 'workContext');
    return { state: str(row, 'state'), batchRef: str(row, 'batchRef'), failureCode: optStr(row, 'failureCode'),
      result: result && { requestRef: str(result, 'requestRef'), outcome: str(result, 'outcome'), reason: optStr(result, 'reason'),
        summary: context && optStr(context, 'summary'),
        signals: list(result, 'signals', item => { const problem = optObj(item, 'problem');
          return { kind: str(item, 'kind'), proposition: str(item, 'proposition'), quote: str(item, 'quote'), problem: problem && problemSummary(problem) };
        }) }
    };
  });
}

export async function planL1(domainRef: string, workRef: string) {
  const endpoint = '/api/local/content-understanding';
  return decode(await requestJson(`${endpoint}/preview?${new URLSearchParams({domainRef, workRef})}`), row => ({
    domainRef: str(row, 'domainRef'), workRef: str(row, 'workRef'), inputHash: str(row, 'inputHash'), knowledgeCutoff: str(row, 'knowledgeCutoff'),
    taskConfigRef: str(row, 'taskConfigRef'), taskRevision: num(row, 'taskRevision'),
    canStart: bool(row, 'canStart'), reasons: strList(row, 'reasons'),
  }));
}
export async function submitL1(plan: Awaited<ReturnType<typeof planL1>>, commandRef: string) {
  const { domainRef, workRef, knowledgeCutoff, taskConfigRef, taskRevision, inputHash } = plan;
  return decode(await requestJson('/api/local/content-understanding', { method: 'POST',
    body: { domainRef, workRef, commandRef, knowledgeCutoff, taskConfigRef, taskRevision, inputHash } }),
  row => ({ reused: bool(row, 'reused'), requestRef: str(row, 'requestRef') }));
}

function automationSetting(row: Record<string, unknown>) {
  return {
    domainRef: str(row, 'domainRef'), enabled: bool(row, 'enabled'), revision: num(row, 'revision'),
    minuteOfDay: num(row, 'minuteOfDay'), timeZone: str(row, 'timeZone'), selectionLimit: num(row, 'selectionLimit'),
    enabledFrom: optStr(row, 'enabledFrom'), nextRunAt: optStr(row, 'nextRunAt'),
  };
}
export type CommentAutomationSetting = ReturnType<typeof automationSetting>;
export async function readAutomation(domain: string, before: string | null, signal?: AbortSignal) {
  return decode(await read('/automation', domain, before ? { before } : {}, signal), row => {
    const setting = automationSetting(obj(row, 'setting'));
    if (setting.domainRef !== domain) throw new Error('自动化设置领域不一致');
    return {
      setting, nextBefore: optStr(row, 'nextBefore'),
      selections: list(row, 'selections', item => ({
        selectionRef: str(item, 'selectionRef'), scheduledAt: str(item, 'scheduledAt'),
        generatedAt: str(item, 'generatedAt'), targetCount: num(item, 'targetCount'),
      })),
    };
  });
}
export interface CommentAutomationChange {
  commandRef: string; domainRef: string; expectedRevision: number; enabled: boolean;
  minuteOfDay: number; timeZone: string; selectionLimit: number;
}
export async function changeAutomation(change: CommentAutomationChange) {
  return decode(await post('/automation', change), row => {
    const setting = automationSetting(row);
    if (setting.domainRef !== change.domainRef) throw new Error('自动化设置回执领域不一致');
    return setting;
  });
}
export async function readAutomationSelection(domain: string, selectionRef: string, signal?: AbortSignal): Promise<SelectedComment[]> {
  return decode(await read(`/automation/selections/${encodeURIComponent(selectionRef)}`, domain, {}, signal), row => {
    if (str(row, 'domainRef') !== domain || str(row, 'selectionRef') !== selectionRef) throw new Error('自动清单范围不一致');
    return list(row, 'selections', item => ({
      sourceRef: str(item, 'sourceRef'), observationRole: str(item, 'observationRole'),
      commentKey: { workRef: str(item, 'workRef'), commentExternalId: str(item, 'commentExternalId') },
    }));
  });
}
