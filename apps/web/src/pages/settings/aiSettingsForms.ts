import type { ModelLimits } from './aiSettingsModel';
export function numeric(form: FormData, key: string): number {
  const value = String(form.get(key) ?? '');
  if (!/^\d+$/.test(value) || !Number.isSafeInteger(Number(value))) throw new Error('请输入范围内的整数。');
  return Number(value);
}

export function readLimits(form: FormData): ModelLimits {
  return { inputTokens: numeric(form, 'inputTokens'), outputTokens: numeric(form, 'outputTokens'), timeoutSeconds: numeric(form, 'timeoutSeconds'), attempts: numeric(form, 'attempts'), thinkingLevel: String(form.get('thinkingLevel')) };
}
