import { ApiError } from '../../../shared/api';

/** 动作回执的三种语气：已完成（服务器确认）、未完成（服务器明确拒绝）、未确认（没有得到答复）。 */
export interface Feedback {
  tone: 'success' | 'error' | 'unknown';
  text: string;
  /** 合同里的错误码，作为技术旁注显示；没有时为 null。 */
  code: string | null;
}

/** 服务端错误码的中文说明。错误码本身作为技术旁注一并显示，便于排查。 */
const ERROR_TEXT: Record<string, string> = {
  policy_revision_conflict: '设置已被更新（可能正是刚才那次保存已经生效），请关闭后重新打开设置再确认。',
  active_primary_domain_conflict: '其他领域已有主研究关系，本领域只能按参照用途加入；也可以先到观察目标管理中调整原关系。',
  creator_resource_not_found: '材料或创作者已不可读取，请刷新。',
  creator_read_unavailable: '读取暂不可用，请稍后重试。',
  read_model_not_connected: '本机数据库没有连接，页面没有回退到其他数据。',
  domain_read_unavailable: '领域范围当前不可读，页面没有把未知结果显示成空列表。',
  domain_paused_or_missing: '当前领域已暂停或不存在，不能修改。',
  observation_lookup_unavailable: '观察关系暂时无法读取。',
  viral_threshold_unset: '当前未设爆款标准，可以改看高赞线索。',
  invalid_creator_scope: '筛选条件无效，请检查日期和数值。',
  invalid_publication_range: '开始日期须早于结束日期（结束日不含）。',
  invalid_publication_date: '请填写有效日期。',
  invalid_like_threshold: '点赞标准须为正整数。',
  invalid_daily_token_limit: '每日词元上限须在 1,024 到 10,000,000 之间。',
  analysis_config_required: '启用分析前请选择模型配置。',
  daily_token_limit_required: '启用分析前请填写每日词元上限。',
  analysis_config_not_found: '所选模型配置已不存在，请重新选择。',
  reason_too_long: '调整原因最多 300 字。',
  invalid_support_fragment: '请选择当前仍可读的支持材料；材料可能已变化，请刷新后重新选择。',
  too_many_support_fragments: '一次最多选择 40 个支持片段。',
  creator_work_mismatch: '作品与创作者不匹配，请刷新后重试。',
  unsupported_creator_identity: '这个平台账号暂不支持登记为观察目标。',
  analysis_disabled: '本领域未开启自动分析，不能重试。',
  local_comment_study_origin_rejected: '这个页面只能在本机访问。',
};

export function errorText(code: string | null): string | null {
  return code ? (ERROR_TEXT[code] ?? null) : null;
}

interface ErrorOptions {
  /** 「未确认」时补充：这个动作重试为什么安全（或需要先核对什么）。 */
  retryNote?: string;
}

export function feedbackForError(error: unknown, options: ErrorOptions = {}): Feedback {
  if (error instanceof ApiError) {
    const failure = error.failure;
    if (failure.kind === 'network') {
      return {
        tone: 'unknown',
        text: `请求没有得到答复，结果未确认，可能已经生效。请先刷新查看，再决定是否重试。${options.retryNote ?? ''}`,
        code: null,
      };
    }
    if (failure.kind === 'invalid_response') {
      return { tone: 'error', text: '服务返回的内容无法识别，页面没有采用它。请刷新后重试。', code: null };
    }
    return {
      tone: 'error',
      text: errorText(failure.code) ?? '请求没有完成，请保留当前输入后重试。',
      code: failure.code,
    };
  }
  return { tone: 'error', text: '请求没有完成，请保留当前输入后重试。', code: null };
}
