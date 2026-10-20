import { useQueryClient } from '@tanstack/react-query';
import { useRef, useState } from 'react';
import { ApiError, isRecord } from '../../shared/api';
import { RequestIdentities } from '../../shared/requestIdentity';
import { AI_SETTINGS_KEY } from './aiSettingsApi';

export type Notice = { tone: 'success' | 'error' | 'unknown'; text: string; code?: string };
const ERRORS: Record<string, string> = {
  ai_revision_conflict: '设置已被修改。当前草稿已保留，请核对最新设置后重新填写并保存。',
  ai_task_not_connected: '这项工作尚未接入执行器，可以保存配置，暂时不能运行。',
  ai_configuration_incomplete: '配置尚不完整，请补齐模型、能力、做法和额度。',
  ai_skill_trial_required: '该候选还没有合格的试跑比较，暂时不能启用。',
  ai_admission_denied: '运行准入未通过，请核对开关、当前地址的材料许可、额度和做法。',
};
export function actionError(error: unknown): Notice {
  if (!(error instanceof ApiError)) return { tone: 'error', text: error instanceof Error ? error.message : '操作未完成。' };
  const failure = error.failure;
  if (failure.kind === 'network' || failure.kind === 'invalid_response') return { tone: 'unknown', text: '结果未确认，操作可能已经生效。同一份输入重试会沿用请求编号，请先核对回执。' };
  const reasons = isRecord(error.responseBody) && Array.isArray(error.responseBody.reasons)
    ? error.responseBody.reasons.filter((v): v is string => typeof v === 'string').join('、') : '';
  return { tone: 'error', text: `${ERRORS[failure.code ?? ''] ?? (failure.status === 409 ? '当前状态不允许该操作，草稿已保留，请核对最新设置。' : '操作被拒绝，请核对输入及所需配置。')}${reasons ? ` 原因：${reasons}` : ''}`, ...(failure.code ? { code: failure.code } : {}) };
}
export function useAiAction() {
  const client = useQueryClient();
  const [ids] = useState(() => new RequestIdentities());
  const lock = useRef(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [notice, setNotice] = useState<Notice | null>(null);
  async function run<T>(action: string, payload: object, perform: (commandRef: string) => Promise<T>, message: string | ((result: T) => string)): Promise<T | undefined> {
    if (lock.current) return undefined;
    lock.current = true; setBusy(action); setNotice(null);
    try {
      const result = await perform(ids.keyFor(action, JSON.stringify(payload)));
      ids.settle(action);
      setNotice({ tone: 'success', text: typeof message === 'function' ? message(result) : message });
      await client.invalidateQueries({ queryKey: AI_SETTINGS_KEY });
      return result;
    } catch (error) {
      if (error instanceof ApiError && error.answered) ids.settle(action);
      setNotice(actionError(error));
      return undefined;
    } finally { lock.current = false; setBusy(null); }
  }
  return { busy, notice, run, setNotice };
}
export type AiAction = ReturnType<typeof useAiAction>;
