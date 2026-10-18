/** Display labels from the existing Rust comment-study surface; no eligibility decisions. */
export const workTitleSourceLabel: Record<string, string | null> = { platform_title: '平台标题', cover_ocr: '封面 OCR', unknown: '标题未记录' };
export const contextSourceLabel: Record<string, string | null> = { native_title: '作品标题', description: '作品描述', body: '正文', asr_text: '视频转写', frame_ocr_text: '视频画面文字', image_ocr_text: '图片文字' };
export const cleanStateLabel: Record<string, string | null> = { direct: '可分析', context: '需上下文', dropped: '无语义已剔除', anomaly: '异常文本', low_information: '低信息' };
export const studyModeLabel: Record<string, string | null> = { new_only:'只研究新评论', input_changed:'只重研输入已变化的评论', retry_failed:'重试失败评论', reanalyse:'重新分析所选评论' };
export const selectionExclusionLabels: Record<string, string | null> = {
  sourceRestricted:'来源受限', bodyUnavailable:'正文不可用', indexPending:'清洗索引未完成',
  textNotResearchable:'文本不具研究条件', workAuthorUnknown:'作品作者身份未知',
  commentAuthorUnknown:'评论作者身份未知', creatorVoice:'作品作者本人',
  inProgress:'已有运行处理中', notSelectedByMode:'不符合本次研究模式', budgetNotSelected:'评论上限截断'
};
export const targetStateLabel: Record<string, string | null> = {
  ready: '准备就绪', needs_context: '等待语境', excluded: '来源受限，未处理', queued: '排队中',
  running: '处理中', succeeded: '已产出结果', no_signal: '已处理 · 无信号', failed: '处理失败'
};
export const observationRoleLabel: Record<string, string | null> = { primary: '本领域', reference: '参考领域' };
export const eligibilityLabel: Record<string, string | null> = {
  eligible: '具备归并资格', deferred_context: '语境不足，暂缓', not_user_problem: '不构成用户问题',
  not_applicable: '不适用（非问题/需求类信号）'
};
export const resolutionLabel: Record<string, string | null> = {
  pending: '待归并判断', assigned: '已归入用户问题', deferred_context: '语境不足，继续等待',
  deferred_ambiguous: '归并边界尚未确定', deferred_novel: '新表达，尚未建档',
  retrieval_incomplete: '候选目录未查全，当前不能判定是否为新问题',
  budget_stopped: '归并预算已到上限，当前未完成判断',
  not_user_problem: '判定不构成用户问题', protocol_rejected: '模型输出不合规，已拒绝', failed: '归并判断失败'
};
export const pairStateLabel: Record<string, string | null> = {
  pending: '正在比较首个合格候选', approved: '已共同建立用户问题', rejected: '未共同建立用户问题',
  failed: '比较未完成，请查看原因'
};
export const pairDecisionLabel: Record<string, string | null> = {
  approved: '两条独立证据支持同一用户问题，已建立问题',
  not_same_problem: '关键维度不同，当前不是同一用户问题',
  ambiguous: '当前证据不足以可靠判断是否为同一问题',
  insufficient_independent_evidence: '独立证据条件不足，未建立问题',
  contract_rejected_json_schema: '模型输出结构不符合约定，未接纳',
  contract_rejected_contract: '模型输出合同版本不符合约定，未接纳',
  contract_rejected_candidate_set_mismatch: '模型返回的比较对象不符合约定，未接纳',
  contract_rejected_invalid_verdict: '模型返回的维度判断无效，未接纳',
  contract_rejected_invalid_problem_definition: '模型给出的问题定义不完整，未接纳',
  contract_rejected_pair_signal_mismatch: '模型返回的研究信号不对应当前配对，未接纳',
  attempts_exhausted: '请求尝试次数已用尽，未收到可接纳结果',
  budget_exhausted: '运行预算已用尽，比较未完成',
  input_limit_exceeded: '请求超过模型输入上限，未发送'
};
export const signalKindLabel: Record<string, string | null> = {
  problem: '问题', need: '需求', belief: '观念', emotion: '情绪', experience: '经历',
  solution: '解决方案', quote: '引述', context: '语境', question: '疑问'
};
export const problemStateLabel: Record<string, string | null> = { active: '生效中', retired: '已停用' };
export const problemSupportLabel: Record<string, string | null> = {supported:'有不同作者支持',support_insufficient:'当前支持不足',definition_stale:'定义依据已失效'};
export const contextStateLabel: Record<string, string | null> = { ready: '语境完整', partial: '语境部分（有截断）', missing: '缺少语境' };
export const dependencyStateLabel: Record<string, string | null> = { self_contained: '不依赖父评论', parent_available: '父评论已冻结', parent_required_missing: '判定缺少父评论' };
export const sourceStateLabel: Record<string, string | null> = { known: null, restricted: '来源已被限制，原文不再显示', unknown: '原文未知（来源未采集到正文）' };
export const runStateLabel: Record<string, string | null> = {
  queued: '排队中', running: '处理中', completed: '已完成',
  completed_with_failures: '已结束 · 有目标未完成', cancelled: '已停止'
};
export const requestStageLabel: Record<string, string | null> = { semantic: '评论语义提取', resolution: '问题归并', pair: '独立证据配对' };
export const modelRequestStateLabel: Record<string, string | null> = { running: '处理中', succeeded: '已完成', failed: '处理失败' };
export const dispatchStateLabel: Record<string, string | null> = { enabled: '允许继续派发', paused: '已暂停派发', stopped: '已停止派发' };
export const dispatchReasonLabel: Record<string, string | null> = {
  user_paused: '用户暂停', user_stopped: '用户停止', budget_exhausted: '预算已用尽',
  legacy_unrecorded: '历史方法未记录', upgrade_guard: '升级保护', method_unavailable: '方法不可用'
};
export const label = (map: Record<string, string | null>, value: string | null | undefined) => value == null ? null : (map[value] ?? "未知状态");
