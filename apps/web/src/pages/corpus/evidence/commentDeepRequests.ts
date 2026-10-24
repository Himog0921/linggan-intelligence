import type { RunRequest } from '../../collection/tasks/runRequests';
import { object, uuid } from '../../collection/tasks/tasksApi';

/** 「指定条数」的范围，与后端 `MAX_COMMENT_TARGET` 一致。 */
export const MIN_COMMENT_TARGET = 1;
export const MAX_COMMENT_TARGET = 1000;

/** 与采集页「语料已选作品」同一份请求：只含这一篇；不给条数就是尽量采全，给了就是采到这么多条（含回复）就停。 */
export function commentDeepIntent(domain: string, work: string, target: number | null) {
  const scope = { domainRef: domain, source: { kind: 'selected_works', workRefs: [work] }, ranking: null, workLimit: 1 };
  return target === null ? { ...scope, commentMode: 'all_public' } : { ...scope, commentMode: 'up_to', commentTarget: target };
}

export type CommentDeepIntent = ReturnType<typeof commentDeepIntent>;

/** 输入框里的条数：只认 1–1000 的整数，其余是 null（按钮不可用并就地说明）。 */
export function parseCommentTarget(raw: string): number | null {
  const value = raw.trim();
  if (!/^\d+$/.test(value)) return null;
  const target = Number(value);
  return target >= MIN_COMMENT_TARGET && target <= MAX_COMMENT_TARGET ? target : null;
}

type PendingRequest = { request: RunRequest; intent: CommentDeepIntent; runRef: string | null };
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
      const frozen = object(JSON.parse(String(request.fingerprint)));
      const target = frozen.commentMode === 'up_to' && typeof frozen.commentTarget === 'number' ? parseCommentTarget(String(frozen.commentTarget)) : null;
      if (frozen.commentMode === 'up_to' && target === null) return undefined;
      const intent = commentDeepIntent(uuid(domain), uuid(work), target);
      const fingerprint = JSON.stringify(intent);
      if (request.fingerprint !== fingerprint || request.sent !== true) return undefined;
      return { intent, request: { identity: uuid(request.identity), fingerprint, sent: true }, runRef: value.runRef === null ? null : uuid(value.runRef) };
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
