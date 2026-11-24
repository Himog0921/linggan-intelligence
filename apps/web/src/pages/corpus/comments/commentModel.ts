import type { CommentItem } from './commentDtos';
import { label, signalKindLabel, targetStateLabel } from './labels';

export const PAGE_SIZES = ['10', '20', '50'] as const;
export type PageSize = typeof PAGE_SIZES[number];
export interface CommentFilters {
  q: string; searchScope: 'comment' | 'work'; studyState: string; signalKind: string | null; voiceRole: string; sort: string; minLikes: string | null; postUsage: string;
  workRef: string | null; workLabel: string; cursor: string | null; history: (string | null)[]; workChoices: { workRef: string; displayTitle: string | null }[];
  /** 1-based page for numbered paging (offset); cursor paging keeps `cursor` + `history`. */
  page: number; size: PageSize;
  /** Show the comments that support this problem (from the problem's evidence list). */
  supportOf: string | null;
}
const valueIn = (value: string | null, choices: string[], fallback: string) => value != null && choices.includes(value) ? value : fallback;
const positive = (value: string | null) => value && /^[1-9]\d{0,9}$/.test(value) ? value : null;
export function commentFilters(params: URLSearchParams, saved?: CommentFilters | null): CommentFilters {
  const cursor = params.get('cursor');
  return {
    q: params.get('q') || '', searchScope: params.get('searchScope') === 'work' ? 'work' : 'comment', workRef: params.get('workRef'),
    studyState: valueIn(params.get('state'), ['all', 'never_studied', 'in_progress', 'studied', 'needs_context', 'failed'], 'all'),
    signalKind: params.get('signalKind') && Object.hasOwn(signalKindLabel, params.get('signalKind')!) ? params.get('signalKind') : null,
    voiceRole: valueIn(params.get('voiceRole'), ['reader_and_unknown', 'reader', 'unknown', 'creator', 'all'], 'reader_and_unknown'),
    sort: valueIn(params.get('sort'), ['received_desc', 'like_desc'], 'like_desc'), minLikes: positive(params.get('minLikes')),
    postUsage: valueIn(params.get('postUsage'), ['all', 'unused', 'used'], 'all'), cursor, history: saved?.cursor === cursor ? saved.history : [],
    workLabel: saved?.workLabel ?? '', workChoices: saved?.workChoices ?? [],
    page: Number(positive(params.get('page')) ?? 1), size: valueIn(params.get('size'), [...PAGE_SIZES], '10') as PageSize,
    supportOf: params.get('supportOf'),
  };
}

export type StatusTone = 'good' | 'wait' | 'bad' | 'running' | '';
/** Table status (B-04 statusInfo) from the server-owned latest/effective study; the full wording stays in the title. */
export function commentStatusInfo(item: CommentItem): { label: string; tone: StatusTone } {
  const latest = item.latestStudy?.state, hasResult = Boolean(item.effectiveStudy) && !['source_changed', 'source_unavailable'].includes(item.effectiveState ?? '');
  if (item.effectiveState === 'source_changed' || item.effectiveState === 'source_unavailable') return { label: item.effectiveState === 'source_changed' ? '原声已变化' : '来源不可用', tone: 'wait' };
  if (latest === 'queued' || latest === 'running') return { label: hasResult ? '研究中 · 有结果' : '研究中', tone: 'running' };
  if (latest === 'failed') return { label: hasResult ? '最近失败 · 有结果' : '最近失败', tone: 'bad' };
  if (latest === 'needs_context') return { label: '需要语境', tone: 'wait' };
  if (latest === 'no_signal') return { label: '已研究 · 无信号', tone: 'good' };
  if (latest === 'succeeded' || hasResult) return { label: '已有结果', tone: 'good' };
  if (latest === 'excluded') return { label: '来源受限', tone: '' };
  if (!item.studyEligibility.eligible) return { label: '不可研究', tone: '' };
  return { label: '尚未研究', tone: '' };
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
