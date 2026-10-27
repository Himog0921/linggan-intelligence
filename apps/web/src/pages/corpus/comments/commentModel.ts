import type { CommentItem } from './commentDtos';
import { label, signalKindLabel, targetStateLabel } from './labels';

export interface CommentFilters { q: string; searchScope: 'comment' | 'work'; studyState: string; signalKind: string | null; voiceRole: string; sort: string; minLikes: string | null; postUsage: string; workRef: string | null; workLabel: string; cursor: string | null; history: (string | null)[]; workChoices: { workRef: string; displayTitle: string | null }[] }
const valueIn = (value: string | null, choices: string[], fallback: string) => value != null && choices.includes(value) ? value : fallback;
export function commentFilters(params: URLSearchParams, saved?: CommentFilters | null): CommentFilters {
  const cursor = params.get('cursor');
  return { q: params.get('q') || '', searchScope: params.get('searchScope') === 'work' ? 'work' : 'comment', workRef: params.get('workRef'), studyState: valueIn(params.get('state'), ['all', 'never_studied', 'in_progress', 'studied', 'needs_context', 'failed'], 'all'), signalKind: params.get('signalKind') && Object.hasOwn(signalKindLabel, params.get('signalKind')!) ? params.get('signalKind') : null, voiceRole: valueIn(params.get('voiceRole'), ['reader_and_unknown', 'reader', 'unknown', 'creator', 'all'], 'reader_and_unknown'), sort: valueIn(params.get('sort'), ['received_desc', 'like_desc'], 'like_desc'), minLikes: ['30', '100', '1000'].includes(params.get('minLikes') || '') ? params.get('minLikes') : null, postUsage: valueIn(params.get('postUsage'), ['all', 'unused', 'used'], 'all'), cursor, history: saved?.cursor === cursor ? saved.history : [], workLabel: saved?.workLabel ?? '', workChoices: saved?.workChoices ?? [] };
}
export function commentStatus(item: CommentItem) {
  if (!item.latestStudy) return '尚未研究';
  const latest = label(targetStateLabel, item.latestStudy.state) ?? item.latestStudy.state;
  if (item.effectiveState === 'source_changed') return `${latest}；原声已变化，旧结果失效`;
  if (item.effectiveState === 'source_unavailable') return `${latest}；来源不可用，旧结果失效`;
  if (item.effectiveStudy && item.latestStudy.requestRef !== item.effectiveStudy.requestRef) return `${latest}；较早完成的研究：${label(targetStateLabel, item.effectiveStudy.state) ?? item.effectiveStudy.state}`;
  return latest;
}
export function commentEligibility(item: CommentItem) {
  if (item.studyEligibility.eligible) return '可研究';
  const labels: Record<string, string> = { sourceRestricted: '来源受限', bodyUnavailable: '正文不可用', indexPending: '等待本地清洗', textNotResearchable: '纯无效文本', workAuthorUnknown: '作品作者身份未知', commentAuthorUnknown: '评论作者身份未知', creatorVoice: '作品作者声音' };
  return item.studyEligibility.reasons.map(reason => labels[reason] || reason).join('；') || '暂不可研究';
}
export const shortBeijingDate = (iso: string | null) => { const date = new Date(iso || ''); return Number.isNaN(date.getTime()) ? '' : new Intl.DateTimeFormat('zh-CN', { timeZone: 'Asia/Shanghai', year: 'numeric', month: '2-digit', day: '2-digit' }).format(date).replaceAll('/', '-'); };
