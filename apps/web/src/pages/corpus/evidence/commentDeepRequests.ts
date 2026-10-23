import type { RunRequest } from '../../collection/tasks/runRequests';

/**
 * 这一页里每篇作品最近一次评论深采请求（只在内存里，离开页面即丢弃）。结果未确认时，下一次点击沿用原请求身份
 * 去核对回执，不会重复建立；服务器已明确答复后才换新身份。按作品与领域分开，切换作品回来仍能接着核对。
 */
export const commentDeepRequests = new Map<string, { request: RunRequest; runRef: string | null }>();

/** 测试用：清空这一页记住的请求。 */
export function forgetCommentDeepRequests(): void {
  commentDeepRequests.clear();
}

/** 与采集页「语料已选作品」同一份请求：只含这一篇、尽量采全（含回复）。 */
export function commentDeepIntent(domain: string, work: string) {
  return { domainRef: domain, source: { kind: 'selected_works', workRefs: [work] }, ranking: null, workLimit: 1, commentMode: 'all_public' };
}
