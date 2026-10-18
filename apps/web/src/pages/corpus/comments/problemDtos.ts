import { bool, decode, list, num, obj, optBool, optNum, optObj, optStr, str, type Json } from '../../../shared/decode';
import { parseVoice, type VoicePreview } from './overviewDtos';
import type { CommentKey, StudyPage } from './studyApi';

export interface ProblemCard {
  problemRef: string; revisionRef: string | null; title: string | null; definition: string | null; definitionReadable: boolean; definitionCurrent: boolean | null;
  supportCommentCount: number | null; supportAuthorCount: number | null; supportWorkCount: number | null; recentAddedSupportCommentCount: number | null;
  changeKind: string | null; updatedAt: string | null; preview: VoicePreview | null;
  stableIdentity: Partial<Record<'actor' | 'context' | 'goalOrExpectedState' | 'barrierOrUnmetNeed', string | null>> | null;
  includeCriteria: unknown[] | null; excludeCriteria: unknown[] | null;
}
export interface ProblemEvidence extends VoicePreview { commentKey: CommentKey; authorDisplayName: string | null; addedAt: string | null; signals: { kind: string; proposition: string | null }[] }
export interface ProblemDetail {
  problem: ProblemCard;
  revisionHistory: { identityVersion: number | null; createdAt: string | null; definitionReadable: boolean; title: string | null; definition: string | null }[];
  sourceDistribution: { works: { workTitle: string | null; commentCount: number | null }[] } | null;
}
export interface ProblemClue extends VoicePreview {
  signalRef: string; commentKey: CommentKey; authorDisplayName: string | null; kind: string; state: string; createdAt: string | null;
  pairOutcomes: { state: string | null; decisionReason: string | null }[];
}
function criteria(row: Json, key: string) {
  const value = row[key]; if (value == null) return null;
  if (!Array.isArray(value)) throw new Error('问题条件返回内容无法解析');
  return value as unknown[];
}
export function parseProblem(row: Json): ProblemCard {
  const preview = optObj(row, 'preview'), identity = optObj(row, 'stableIdentity');
  return { problemRef: str(row, 'problemRef'), revisionRef: optStr(row, 'revisionRef'), title: optStr(row, 'title'), definition: optStr(row, 'definition'), definitionReadable: bool(row, 'definitionReadable'), definitionCurrent: optBool(row, 'definitionCurrent'), supportCommentCount: optNum(row, 'supportCommentCount'), supportAuthorCount: optNum(row, 'supportAuthorCount'), supportWorkCount: optNum(row, 'supportWorkCount'), recentAddedSupportCommentCount: optNum(row, 'recentAddedSupportCommentCount'), changeKind: optStr(row, 'changeKind'), updatedAt: optStr(row, 'updatedAt'), preview: preview ? parseVoice(preview) : null, stableIdentity: identity ? { actor: optStr(identity, 'actor'), context: optStr(identity, 'context'), goalOrExpectedState: optStr(identity, 'goalOrExpectedState'), barrierOrUnmetNeed: optStr(identity, 'barrierOrUnmetNeed') } : null, includeCriteria: criteria(row, 'includeCriteria'), excludeCriteria: criteria(row, 'excludeCriteria') };
}
export function parseProblemCards(row: Json) {
  return decode(row, r => {
    if (str(r, 'contract') !== 'comment-study.problem-cards.v1') throw new Error('读取回执与当前范围不一致，请重新读取。');
    return { domainRef: str(r, 'domainRef'), problems: list(r, 'problems', parseProblem), totalCount: num(r, 'totalCount'), page: { nextCursor: optStr(obj(r, 'page'), 'nextCursor') } };
  });
}
export function parseProblemDetail(row: Json): ProblemDetail {
  return decode(row, r => { const distribution = optObj(r, 'sourceDistribution'); return { problem: parseProblem(obj(r, 'problem')), revisionHistory: list(r, 'revisionHistory', v => ({ identityVersion: optNum(v, 'identityVersion'), createdAt: optStr(v, 'createdAt'), definitionReadable: bool(v, 'definitionReadable'), title: optStr(v, 'title'), definition: optStr(v, 'definition') })), sourceDistribution: distribution ? { works: list(distribution, 'works', w => ({ workTitle: optStr(w, 'workTitle'), commentCount: optNum(w, 'commentCount') })) } : null }; });
}
export function parseProblemEvidence(row: Json): { problemRef: string; evidence: ProblemEvidence[]; page: StudyPage } {
  return decode(row, r => ({ problemRef: str(r, 'problemRef'), evidence: list(r, 'evidence', e => ({ ...parseVoice(e), commentKey: { workRef: str(obj(e, 'commentKey'), 'workRef'), commentExternalId: str(obj(e, 'commentKey'), 'commentExternalId') }, authorDisplayName: optStr(e, 'authorDisplayName'), addedAt: optStr(e, 'addedAt'), signals: list(e, 'signals', s => ({ kind: str(s, 'kind'), proposition: optStr(s, 'proposition') })) })), page: { nextCursor: optStr(obj(r, 'page'), 'nextCursor') } }));
}
export function parseProblemClues(row: Json): { expressions: ProblemClue[]; page: StudyPage } {
  return decode(row, r => ({ expressions: list(r, 'expressions', e => ({ ...parseVoice(e), commentKey: { workRef: str(obj(e, 'commentKey'), 'workRef'), commentExternalId: str(obj(e, 'commentKey'), 'commentExternalId') }, signalRef: str(e, 'signalRef'), authorDisplayName: optStr(e, 'authorDisplayName'), kind: str(e, 'kind'), state: str(e, 'state'), createdAt: optStr(e, 'createdAt'), pairOutcomes: list(e, 'pairOutcomes', p => ({ state: optStr(p, 'state'), decisionReason: optStr(p, 'decisionReason') })) })), page: { nextCursor: optStr(obj(r, 'page'), 'nextCursor') } }));
}
