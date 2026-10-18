import { bool, decode, list, obj, optNum, optObj, optStr, str, strList, type Json } from '../../../shared/decode';
import type { CommentKey, StudyPage } from './studyApi';

export interface StudyAttempt { runRef: string; targetRef: string; state: string; createdAt: string | null }
export interface PostUsage { state: string; markedAt: string | null }
export interface CommentItem {
  commentKey: CommentKey; commentText: string | null; researchText: string | null; sourceState: string | null;
  observationRole: string; authorDisplayName: string | null; observedAt: string | null; likeCount: number | null; likeCountAsOf: string | null;
  cleanState: string | null; cleanReasons: string[]; signalKinds: string[]; workTitle: string | null; workTitleSource: string | null;
  studyEligibility: { eligible: boolean; reasons: string[] }; latestStudy: StudyAttempt | null; effectiveStudy: StudyAttempt | null; effectiveState: string | null; postUsage: PostUsage | null;
}
export interface CommentHistory { runRef: string | null; targetRef: string | null; state: string | null; createdAt: string | null; signalCount: number | null; method: { recordingState: string | null; methodName: string | null; policyRef: string | null } | null }
export interface HistoryPage { items: CommentHistory[]; totalCount: number | null; page: StudyPage }
export interface CommentRelatedSignal { signalRef: string; targetRef: string | null; runRef: string | null; kind: string; proposition: string | null; evidence: string | null; resolvedProblemRef: string | null }
export interface RelatedPage { commentKey: CommentKey; sourceState: string | null; signals: CommentRelatedSignal[]; page: StudyPage }
export interface CommentDetail {
  commentKey: CommentKey; comment: CommentItem | null; source: { sourceState: string | null; displayState: string | null };
  work: { displayTitle: string | null; displayTitleSource: string | null } | null;
  parentContext: { commentText: string | null; commentKey: CommentKey | null } | null;
  studyHistory: HistoryPage | null;
}
export const parseCommentKey = (r: Json): CommentKey => ({ workRef: str(r, 'workRef'), commentExternalId: str(r, 'commentExternalId') });
function parseAttempt(r: Json): StudyAttempt { return { runRef: str(r, 'runRef'), targetRef: str(r, 'targetRef'), state: str(r, 'state'), createdAt: optStr(r, 'createdAt') }; }
export function parseComment(row: Json): CommentItem {
  const eligibility = obj(row, 'studyEligibility'), latest = optObj(row, 'latestStudy'), effective = optObj(row, 'effectiveStudy'), usage = optObj(row, 'postUsage');
  return { commentKey: parseCommentKey(obj(row, 'commentKey')), commentText: optStr(row, 'commentText'), researchText: optStr(row, 'researchText'), sourceState: optStr(row, 'sourceState'), observationRole: str(row, 'observationRole'), authorDisplayName: optStr(row, 'authorDisplayName'), observedAt: optStr(row, 'observedAt'), likeCount: optNum(row, 'likeCount'), likeCountAsOf: optStr(row, 'likeCountAsOf'), cleanState: optStr(row, 'cleanState'), cleanReasons: strList(row, 'cleanReasons'), signalKinds: strList(row, 'signalKinds'), workTitle: optStr(row, 'workTitle'), workTitleSource: optStr(row, 'workTitleSource'), studyEligibility: { eligible: bool(eligibility, 'eligible'), reasons: strList(eligibility, 'reasons') }, latestStudy: latest ? parseAttempt(latest) : null, effectiveStudy: effective ? parseAttempt(effective) : null, effectiveState: optStr(row, 'effectiveState'), postUsage: usage ? { state: str(usage, 'state'), markedAt: optStr(usage, 'markedAt') } : null };
}
export function parseComments(row: Json) {
  return decode(row, r => { const summary = obj(r, 'summary'), coverage = obj(r, 'indexCoverage'); return { items: list(r, 'items', parseComment), summary: { displayableCommentCount: optNum(summary, 'displayableCommentCount'), eligibleCommentCount: optNum(summary, 'eligibleCommentCount') }, indexCoverage: { indexedCount: optNum(coverage, 'indexedCount'), pendingCount: optNum(coverage, 'pendingCount') }, page: { nextCursor: optStr(obj(r, 'page'), 'nextCursor') } }; });
}
export function parseHistory(row: Json): HistoryPage {
  return { items: list(row, 'items', h => { const method = optObj(h, 'method'); return { runRef: optStr(h, 'runRef'), targetRef: optStr(h, 'targetRef'), state: optStr(h, 'state'), createdAt: optStr(h, 'createdAt'), signalCount: optNum(h, 'signalCount'), method: method ? { recordingState: optStr(method, 'recordingState'), methodName: optStr(method, 'methodName'), policyRef: optStr(method, 'policyRef') } : null }; }), totalCount: optNum(row, 'totalCount'), page: { nextCursor: optStr(obj(row, 'page'), 'nextCursor') } };
}
export function parseCommentDetail(row: Json): CommentDetail {
  return decode(row, r => { const comment = optObj(r, 'comment'), source = obj(r, 'source'), work = optObj(r, 'work'), parent = optObj(r, 'parentContext'), history = optObj(r, 'studyHistory'); return { commentKey: parseCommentKey(obj(r, 'commentKey')), comment: comment ? parseComment(comment) : null, source: { sourceState: optStr(source, 'sourceState'), displayState: optStr(source, 'displayState') }, work: work ? { displayTitle: optStr(work, 'displayTitle'), displayTitleSource: optStr(work, 'displayTitleSource') } : null, parentContext: parent ? { commentText: optStr(parent, 'commentText'), commentKey: optObj(parent, 'commentKey') ? parseCommentKey(obj(parent, 'commentKey')) : null } : null, studyHistory: history ? parseHistory(history) : null }; });
}
export function parseRelated(row: Json): RelatedPage {
  return decode(row, r => ({ commentKey: parseCommentKey(obj(r, 'commentKey')), sourceState: optStr(r, 'sourceState'), signals: list(r, 'signals', s => ({ signalRef: str(s, 'signalRef'), targetRef: optStr(s, 'targetRef'), runRef: optStr(s, 'runRef'), kind: str(s, 'kind'), proposition: optStr(s, 'proposition'), evidence: optStr(s, 'evidence'), resolvedProblemRef: optStr(s, 'resolvedProblemRef') })), page: { nextCursor: optStr(obj(r, 'page'), 'nextCursor') } }));
}
export function parsePostUsageReceipt(row: Json) {
  return decode(row, r => ({ contract: str(r, 'contract'), requestRef: str(r, 'requestRef'), commentKey: parseCommentKey(obj(r, 'commentKey')), outcome: str(r, 'outcome'), postUsage: { state: str(obj(r, 'postUsage'), 'state'), markedAt: optStr(obj(r, 'postUsage'), 'markedAt') } }));
}
