import type { RunRequest } from '../../collection/tasks/runRequests';
import { object, uuid } from '../../collection/tasks/tasksApi';

/** 与采集页「语料已选作品」同一份请求：只含这一篇、尽量采全（含回复）。 */
export function commentDeepIntent(domain: string, work: string) {
  return { domainRef: domain, source: { kind: 'selected_works', workRefs: [work] }, ranking: null, workLimit: 1, commentMode: 'all_public' };
}

type PendingRequest = { request: RunRequest; runRef: string | null };
const PREFIX = 'linggan.evidence.comment-deep.v1:';

/** 只保存请求身份和冻结意图，不保存原文。刷新后仍核对同一次写入。 */
export const commentDeepRequests = {
  get(key: string): PendingRequest | undefined {
    try {
      const raw = sessionStorage.getItem(PREFIX + key);
      if (raw === null) return undefined;
      const value = object(JSON.parse(raw));
      const request = object(value.request);
      const [domain, work] = key.split(':');
      const fingerprint = JSON.stringify(commentDeepIntent(uuid(domain), uuid(work)));
      if (request.fingerprint !== fingerprint || request.sent !== true) return undefined;
      return { request: { identity: uuid(request.identity), fingerprint, sent: true }, runRef: value.runRef === null ? null : uuid(value.runRef) };
    } catch { return undefined; }
  },
  set(key: string, value: PendingRequest): void {
    // 在任何网络调用前落盘。保守记录 sent，刷新只能核对；存储失败会阻止 POST。
    sessionStorage.setItem(PREFIX + key, JSON.stringify({ request: { identity: value.request.identity, fingerprint: value.request.fingerprint, sent: true }, runRef: value.runRef }));
  },
  delete(key: string): void { sessionStorage.removeItem(PREFIX + key); },
};

/** 测试用：清空当前会话保存的请求。 */
export function forgetCommentDeepRequests(): void {
  for (const key of Object.keys(sessionStorage)) if (key.startsWith(PREFIX)) sessionStorage.removeItem(key);
}
