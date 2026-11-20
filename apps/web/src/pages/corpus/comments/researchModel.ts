export const PHASES = ['原声提取', '问题归并', '归并核验'];
export const STATES: Record<string, string> = { needs_support: '等待更多原声', pending: '等待', running: '执行中', succeeded: '已完成', failed: '失败', skipped: '未执行', cancelled: '已取消' };
export const REASONS: Record<string, string> = { ai_dispatch_unknown: '发送结果未知，请先核对调用记录', ai_source_unavailable: '来源当前不可读取', ai_output_invalid: '结果未通过校验', comment_publication_withheld: '材料、许可或控制状态已变化', ai_input_limit_before_dispatch: '完整材料超过模型输入额度，未发送', ai_provider_failed: '模型调用失败' };
