import { bool, decode, list, obj, optNum, optObj, optStr, str, strList, type Json } from '../../../shared/decode';
import type { CommentKey } from './studyApi';

export interface StudyAttempt { requestRef: string; state: string; createdAt: string | null }
export interface PostUsage { state: string; markedAt: string | null }
export interface CommentItem {
  sourceRef: string; commentKey: CommentKey; commentText: string | null; researchText: string | null; sourceState: string | null;
  observationRole: string; authorDisplayName: string | null; observedAt: string | null; likeCount: number | null; likeCountAsOf: string | null;
  cleanState: string | null; cleanReasons: string[]; signalKinds: string[]; workTitle: string | null; workTitleSource: string | null;
  studyEligibility: { eligible: boolean; reasons: string[] }; latestStudy: StudyAttempt | null; effectiveStudy: StudyAttempt | null; effectiveState: string | null; postUsage: PostUsage | null;
}
export interface CommentDetail {
  commentKey: CommentKey; comment: CommentItem | null; source: { sourceRef: string | null; sourceState: string | null; displayState: string | null };
  work: { displayTitle: string | null; displayTitleSource: string | null } | null;
  parentContext: { commentText: string | null; commentKey: CommentKey | null } | null;
}
export const parseCommentKey = (r: Json): CommentKey => ({ workRef: str(r, 'workRef'), commentExternalId: str(r, 'commentExternalId') });
function parseAttempt(r: Json): StudyAttempt { return { requestRef: str(r, 'requestRef'), state: str(r, 'state'), createdAt: optStr(r, 'createdAt') }; }
export function parseComment(row: Json): CommentItem {
  const eligibility = obj(row, 'studyEligibility'), latest = optObj(row, 'latestStudy'), effective = optObj(row, 'effectiveStudy'), usage = optObj(row, 'postUsage');
  return { sourceRef: str(row, 'sourceRef'), commentKey: parseCommentKey(obj(row, 'commentKey')), commentText: optStr(row, 'commentText'), researchText: optStr(row, 'researchText'), sourceState: optStr(row, 'sourceState'), observationRole: str(row, 'observationRole'), authorDisplayName: optStr(row, 'authorDisplayName'), observedAt: optStr(row, 'observedAt'), likeCount: optNum(row, 'likeCount'), likeCountAsOf: optStr(row, 'likeCountAsOf'), cleanState: optStr(row, 'cleanState'), cleanReasons: strList(row, 'cleanReasons'), signalKinds: strList(row, 'signalKinds'), workTitle: optStr(row, 'workTitle'), workTitleSource: optStr(row, 'workTitleSource'), studyEligibility: { eligible: bool(eligibility, 'eligible'), reasons: strList(eligibility, 'reasons') }, latestStudy: latest ? parseAttempt(latest) : null, effectiveStudy: effective ? parseAttempt(effective) : null, effectiveState: optStr(row, 'effectiveState'), postUsage: usage ? { state: str(usage, 'state'), markedAt: optStr(usage, 'markedAt') } : null };
}
export function parseComments(row: Json) {
  return decode(row, r => { const summary = obj(r, 'summary'), coverage = obj(r, 'indexCoverage'); return { items: list(r, 'items', parseComment), summary: { displayableCommentCount: optNum(summary, 'displayableCommentCount'), eligibleCommentCount: optNum(summary, 'eligibleCommentCount') }, indexCoverage: { indexedCount: optNum(coverage, 'indexedCount'), pendingCount: optNum(coverage, 'pendingCount') }, page: { nextCursor: optStr(obj(r, 'page'), 'nextCursor') } }; });
}
export function parseCommentDetail(row: Json): CommentDetail {
  return decode(row, r => { const comment = optObj(r, 'comment'), source = obj(r, 'source'), work = optObj(r, 'work'), parent = optObj(r, 'parentContext'); return { commentKey: parseCommentKey(obj(r, 'commentKey')), comment: comment ? parseComment(comment) : null, source: { sourceRef: optStr(source, 'sourceRef'), sourceState: optStr(source, 'sourceState'), displayState: optStr(source, 'displayState') }, work: work ? { displayTitle: optStr(work, 'displayTitle'), displayTitleSource: optStr(work, 'displayTitleSource') } : null, parentContext: parent ? { commentText: optStr(parent, 'commentText'), commentKey: optObj(parent, 'commentKey') ? parseCommentKey(obj(parent, 'commentKey')) : null } : null }; });
}
export function parsePostUsageReceipt(row: Json) {
  return decode(row, r => ({ contract: str(r, 'contract'), requestRef: str(r, 'requestRef'), commentKey: parseCommentKey(obj(r, 'commentKey')), outcome: str(r, 'outcome'), postUsage: { state: str(obj(r, 'postUsage'), 'state'), markedAt: optStr(obj(r, 'postUsage'), 'markedAt') } }));
}
