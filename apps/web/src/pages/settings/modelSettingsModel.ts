import { ApiError } from '../../shared/api';
import type { ModelEntry, ProbeOutcome } from './modelSettingsApi';

/** 只有已启用、能调用且通过 V1 JSON 测试的模型，才能被选为默认研究模型。 */
export function isCallable(model: ModelEntry | undefined): boolean {
  return Boolean(model?.enabled && model.modelCallable && model.semanticQualified);
}

export function modelStateLabel(model: ModelEntry): string {
  if (!model.enabled) return '连接已停用';
  if (isCallable(model)) return '可用于评论研究 V1';
  if (model.modelCallable) return '模型已响应，但 V1 JSON 测试未通过';
  return '需要完成模型测试';
}

export const CONFIG_DEFAULTS = {
  inputTokenLimit: 16000,
  outputTokenLimit: 2000,
  timeoutSeconds: 30,
  maxAttempts: 2,
} as const;

export const CONFIG_LIMITS = {
  inputTokenLimit: { min: 1024, max: 32768 },
  outputTokenLimit: { min: 128, max: 8192 },
  timeoutSeconds: { min: 1, max: 60 },
  maxAttempts: { min: 1, max: 3 },
} as const;

export const CONNECTION_APIS: readonly { value: string; label: string }[] = [
  { value: 'openai-completions', label: 'OpenAI 兼容 Chat Completions' },
  { value: 'openai-responses', label: 'OpenAI Responses' },
  { value: 'anthropic-messages', label: 'Anthropic Messages' },
];

/** 服务端错误码的中文说明。错误码本身作为技术旁注一并显示，便于排查（页面规格「受控 failure code」）。 */
const ERROR_TEXT: Record<string, string> = {
  invalid_model_command: '输入不符合模型设置要求。',
  model_revision_conflict: '设置已变化，请刷新后重试。',
  model_disabled: '连接未启用。',
  model_not_qualified: '请先完成模型能力测试。',
  model_database_unavailable: '模型设置数据库暂不可用。',
  model_not_found: '这条设置记录不存在，请刷新后重试。',
  model_secret_unavailable: '本机无法保存或读取 API Key。',
  model_adapter_unavailable: '模型调用组件暂不可用。',
  provider_timeout: '模型服务响应超时。',
  model_input_limit: '输入超过了允许的上限。',
  model_invalid_output: '模型返回的内容不符合要求。',
  model_budget_exhausted: '调用额度已用完。',
  model_schema_missing: '模型设置所需的数据表尚未建立。',
  model_source_unavailable: '模型设置的来源数据暂不可用。',
};

export interface Feedback {
  tone: 'success' | 'error' | 'unknown';
  text: string;
  /** 合同里的错误码，作为技术旁注显示；没有时为 null。 */
  code: string | null;
}

export function feedbackForError(error: unknown): Feedback {
  if (error instanceof ApiError) {
    const failure = error.failure;
    if (failure.kind === 'network') {
      return {
        tone: 'unknown',
        text: '请求没有得到答复，结果未确认，可能已经生效。请先刷新查看，再决定是否重试；重试会沿用同一个请求编号，不会重复生效。',
        code: null,
      };
    }
    if (failure.kind === 'invalid_response') {
      return { tone: 'error', text: '服务返回的内容无法识别，页面没有采用它。请刷新后重试。', code: null };
    }
    const known = failure.code ? ERROR_TEXT[failure.code] : undefined;
    return {
      tone: 'error',
      text: known ?? '请求没有完成，请保留当前输入后重试。',
      code: failure.code,
    };
  }
  return { tone: 'error', text: '请求没有完成，请保留当前输入后重试。', code: null };
}

export function feedbackForProbe(outcome: ProbeOutcome): Feedback {
  switch (outcome.kind) {
    case 'qualified':
      return { tone: 'success', text: '模型 V1 JSON 测试通过。', code: null };
    case 'callable_not_qualified':
      return { tone: 'error', text: '模型已返回，但 V1 JSON 测试未通过。', code: null };
    case 'failed':
      return {
        tone: 'error',
        text: outcome.failureCode ? '模型测试未通过。' : '模型测试未通过，服务没有给出原因。',
        code: outcome.failureCode,
      };
    case 'still_running':
      return {
        tone: 'unknown',
        text: '上一次测试仍在进行，结果尚未确认。请稍后刷新查看，不会重复发起测试。',
        code: null,
      };
  }
}
