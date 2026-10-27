/**
 * 创作者页的文字与派生：服务器给出的判断值、状态码如何用中文说给人听。
 * 这里只翻译与归类，不重算任何业务结论（资格、相关性、观察关系都以服务器为准）。
 */
import { CREATOR_AI_RETIREMENT_NOTICE } from '../../settings/aiSettingsModel';
import { humanMoment } from '../../../shared/formatting';
import { LABELS } from './creatorsQuery';
import type { Analysis, CreatorItem, Fragment, Judgement, Observation, Work } from './creatorsTypes';
import type { WorkResources } from './useWorkResources';

export { formatCount } from './creatorsQuery';

const UNKNOWN: Judgement = { value: 'unknown', reason: null, evidenceFragmentIds: [] };

/** 判断值的中文。`yes/no` 是作品表达的有无：「未见」只说本次材料没看到，不说现实里没有。 */
export const VALUE_LABELS: Record<string, string> = {
  ...LABELS,
  yes: '有依据',
  no: '本次材料未见',
};

export function valueLabel(value: string | null | undefined): string {
  return VALUE_LABELS[value ?? 'unknown'] ?? VALUE_LABELS.unknown ?? '尚不能判定';
}

/** 作品与当前领域的关系。「待判断」与筛选、计数里的说法一致：它不是「无关」。 */
export function relevanceLabel(value: string | null | undefined): string {
  if (value === 'related') return '已判相关';
  if (value === 'unrelated') return '已判无关';
  return '待判断';
}

/** 判断项的名字。作品表达（亲历自述等）属于作品，作者倾向属于作者，两者分开命名。 */
export const FIELD_NAMES: Record<string, string> = {
  relevance: '与当前领域的关系',
  focus: '作者在本领域的内容倾向',
  institution_or_brand: '机构／品牌自述',
  personal_experience: '亲历自述',
  professional_output: '知识与方法讲解',
  explicit_promotion: '推广表达',
};

/**
 * 每一项判断的依据门槛（#384 发现：门槛与可并存关系要能直接看到）。
 * 与后台校验一致：持续创作要求至少两篇独立相关作品，再加被引用的公开简介或至少两篇主页发现作品。
 */
export const CRITERIA: Record<string, string> = {
  personal_experience: '具体的本人或家庭经历、行动、过程或感受；泛建议、第一人称广告与转载不足以支持，也不认证经历真实性。',
  professional_output: '有概念、知识或方法讲解；不认证职业资质。',
  explicit_promotion: '材料明确介绍产品、服务、课程或购买入口；本次未见不代表没有商业行为。',
  institution_or_brand: '公开简介明确自述机构或品牌身份；不凭昵称、头像或粉丝数推断。',
  relevance: '结合当前领域说明、研究目标与这篇作品的可读片段判断。',
  focus:
    '持续创作倾向须至少两篇独立相关作品，加上被引用的公开简介或至少两篇主页发现作品；多主题分享须有较宽的来源材料；仅靠有限的领域样本，不足以判断整个账号。',
};

/** 两个层次分开说：作品表达属于作品，作者倾向属于作者；它们互不推出，可以同时成立。 */
export const COEXISTENCE =
  '亲历自述、知识与方法讲解、推广表达是「作品」层面的表达，可以同时出现；「本领域持续创作」是「作者」层面的倾向，与前者并存、互不推出。不同领域共用这套证据门槛，相关性依当前领域的说明与研究目标判断；切换领域不会开启分析。';

export const MONITORING_LABELS: Record<string, string> = {
  monitoring: '巡查中',
  paused: '观察已暂停',
  dismissed: '已忽略',
  not_enabled: '已加入 · 巡查未开启',
};

/** 本领域内的观察关系，一句话。关系没读到是「暂不可用」，不是「未加入」。 */
export function observedText(observation: Observation): string {
  if (observation.lookup === 'unavailable' || observation.inCurrentDomain === null) return '观察关系暂不可用';
  if (observation.inCurrentDomain) {
    const state = MONITORING_LABELS[observation.monitoringState ?? ''] ?? '观察状态待核对';
    return observation.usageRole === 'reference' ? `${state} · 参照` : state;
  }
  if (observation.lifecycleState === 'paused' || observation.lifecycleState === 'dismissed') {
    return `${MONITORING_LABELS[observation.lifecycleState]} · 未加入本领域`;
  }
  return observation.existsInOtherDomains ? '仅其他领域已观察' : '未加入观察';
}

export function preservedLifecycle(observation: Observation): string | null {
  return observation.lifecycleState === 'paused' || observation.lifecycleState === 'dismissed'
    ? (MONITORING_LABELS[observation.lifecycleState] ?? null)
    : null;
}

// ───────────────────────── 分析状态 ─────────────────────────

/** 分析失败或暂停的原因：服务器给的是受控代码，这里只翻译；认不得的代码不猜，引导看处理信息。 */
const FAILURE_REASONS: Record<string, string> = {
  provider_unavailable: '模型服务暂不可用',
  model_provider_unavailable: '模型服务暂不可用',
  budget_exhausted: '本日额度已用尽',
  model_budget_exhausted: '本日额度已用尽',
  invalid_result: '结果未通过校验',
  model_invalid_output: '结果未通过校验',
  creator_analysis_invalid_output: '结果未通过校验',
  source_unavailable: '材料暂不可用',
  model_source_unavailable: '材料暂不可用',
  creator_source_changed: '历史执行时材料已变化',
  model_secret_unavailable: '模型密钥不可用',
  model_not_qualified: '模型尚未通过用途验证',
  model_disabled: '模型配置未启用',
  lease_expired: '上次执行超时',
};

export function failureReason(code: string | null): string {
  return (code && FAILURE_REASONS[code]) || '历史失败原因未识别，可查看处理信息';
}

/** 旧分析已退役；保留历史执行记录和结果，不将历史 running/queued 当作当前执行。 */
export type AnalysisKind = 'failed' | 'paused' | 'insufficient' | 'done' | 'off';

export function analysisStatus(
  analysis: Analysis,
  value: string | null | undefined,
): { kind: AnalysisKind; text: string } {
  if (analysis.origin === 'creator_research') {
    switch (analysis.state) {
      case 'pending': return { kind: 'off', text: '研究已排队，等待执行' };
      case 'running': return { kind: 'off', text: '正在研究' };
      case 'failed': return { kind: 'failed', text: '研究未完成，请检查运行记录' };
      case 'skipped': return { kind: 'insufficient', text: '材料当前不可用，未执行研究' };
      case 'cancelled': return { kind: 'off', text: '研究已取消' };
    }
  }
  switch (analysis.state) {
    case 'running':
      return { kind: 'off', text: '历史分析中记录 · 已停用，不再执行' };
    case 'queued':
      return { kind: 'off', text: '历史排队记录 · 已停用，不再执行' };
    case 'failed':
      return { kind: 'failed', text: `历史分析失败 · ${failureReason(analysis.lastErrorCode)}` };
    case 'paused':
      return { kind: 'paused', text: `历史分析暂停 · ${failureReason(analysis.lastErrorCode)}` };
  }
  if (analysis.resultAt !== null || analysis.resultAtDisplay !== null) {
    return value === 'unknown' || !value
      ? { kind: 'insufficient', text: '已分析，依据不足' }
      : { kind: 'done', text: '已有分析结果' };
  }
  return { kind: 'off', text: `尚无分析结果 · ${CREATOR_AI_RETIREMENT_NOTICE}` };
}

/** 实际取样篇数：服务器记录了才显示，没记录就明说没记录，绝不显示成 0 或候选数。 */
export function sampleText(analysis: Analysis): string {
  const used = analysis.sampleWorkCount;
  if (used === null || !Number.isInteger(used)) return '本次实际取样篇数未记录。';
  const available = analysis.availableWorkCount;
  return `本次实际使用 ${used} 篇作品摘要${available !== null && Number.isInteger(available) ? `；分析当时已有 ${available} 篇` : ''}。`;
}

export function sampleBasisText(analysis: Analysis): string {
  switch (analysis.sampleBasis) {
    case 'profile_discovery_sample':
      return '样本包含主页发现的作品';
    case 'limited_domain_sample':
      return '样本仅为本领域已有作品';
    default:
      return '材料范围尚未记录';
  }
}

// ───────────────────────── 判断与依据 ─────────────────────────

/** 作品层/身份层某一项当前生效的判断：人工调整优先，其次自动结果，都没有就是「尚不能判定」。 */
export function effectiveJudgement(analysis: Analysis, field: string): Judgement {
  const manual = analysis.manual[field];
  if (manual) return manual.judgement;
  if (field === 'relevance') return analysis.relevance ?? UNKNOWN;
  if (field === 'focus') return analysis.focus ?? UNKNOWN;
  if (field === 'institution_or_brand') return analysis.institution ?? UNKNOWN;
  return analysis.traits[field] ?? UNKNOWN;
}

/** 作者倾向：列表项自带的 `focus` 已经是服务器按「人工优先、自动其次」算好的有效值。 */
export function authorFocus(item: CreatorItem): Judgement {
  return item.authorAnalysis.manual.focus?.judgement ?? item.focus;
}

/** 判断引用的片段：人工调整引用人工选择的片段，否则引用自动结果里的片段。 */
export function evidenceFor(analysis: Analysis, field: string, judgement: Judgement): Fragment[] {
  const manual = analysis.manual[field];
  const ids = new Set(manual ? manual.supportFragmentIds : judgement.evidenceFragmentIds);
  return analysis.fragments.filter(fragment => ids.has(fragment.fragmentId));
}

export const FIELD_SOURCE: Record<string, string> = {
  biography: '公开简介',
  title: '标题',
  body: '正文',
  ocr: '图文识别',
  transcript: '转写',
};

export function fieldSource(field: string): string {
  return FIELD_SOURCE[field] ?? '材料片段';
}

export function acquisitionText(work: Work): string {
  if (work.acquisitionKind === 'profile_discovery') return '主页发现作品';
  if (work.acquisitionKind === 'limited_domain_sample') return '本领域已有作品';
  return '已有作品材料';
}

/** 自动依据的更新时间 / 人工调整的时间；一律 `YYYY-MM-DD HH:MM`，没记录就明说。 */
export function provenanceText(analysis: Analysis, field: string): string {
  const manual = analysis.manual[field];
  if (manual) return manual.at ? `人工调整 · ${humanMoment(manual.at)}` : '人工调整';
  const at = analysis.resultAt ?? analysis.resultAtDisplay;
  return at ? `自动依据更新于 ${humanMoment(at)}` : '自动依据更新时间未记录';
}

export function authorName(item: CreatorItem): string {
  return item.displayName ?? '作者名称未取得';
}

/** 作品标题：作品资源给出的展示标题（含封面识别补位）优先，其次作品自带标题，都没有就明说。 */
export function workTitle(work: Work, resources: WorkResources): string {
  return resources.byRef.get(work.workRef)?.title || work.title || '标题未取得';
}

/** 观察目标在采集页的地址。 */
export function targetHref(domain: string, targetRef: string): string {
  return `/collection/targets?${new URLSearchParams({ domain, drawer: targetRef })}`;
}
