import type { RunRequest } from '../../collection/tasks/runRequests';

/** 「指定条数」的范围，与后端 `MAX_COMMENT_TARGET` 一致。 */
export const MIN_COMMENT_TARGET = 1;
export const MAX_COMMENT_TARGET = 1000;

/** 与采集页「语料已选作品」同一份请求：只含这一篇；不给条数就是尽量采全，给了就是采到这么多条（含回复）就停。 */
export function commentDeepIntent(domain: string, work: string, target: number | null) {
  const scope = { domainRef: domain, source: { kind: 'selected_works', workRefs: [work] }, ranking: null, workLimit: 1 };
  return target === null ? { ...scope, commentMode: 'all_public' } : { ...scope, commentMode: 'up_to', commentTarget: target };
}

export type CommentDeepIntent = ReturnType<typeof commentDeepIntent>;

/**
 * 这一页里每篇作品最近一次评论深采请求（只在内存里，离开页面即丢弃）。结果未确认时，下一次点击沿用原请求身份与
 * 原请求内容去核对回执，不会重复建立；服务器已明确答复后才换新身份。按作品与领域分开，切换作品回来仍能接着核对。
 */
export const commentDeepRequests = new Map<string, { request: RunRequest; intent: CommentDeepIntent; runRef: string | null }>();

/** 测试用：清空这一页记住的请求。 */
export function forgetCommentDeepRequests(): void {
  commentDeepRequests.clear();
}

/** 输入框里的条数：只认 1–1000 的整数，其余是 null（按钮不可用并就地说明）。 */
export function parseCommentTarget(raw: string): number | null {
  const value = raw.trim();
  if (!/^\d+$/.test(value)) return null;
  const target = Number(value);
  return target >= MIN_COMMENT_TARGET && target <= MAX_COMMENT_TARGET ? target : null;
}
