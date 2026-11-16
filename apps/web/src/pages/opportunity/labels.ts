import { ApiError } from '../../shared/api';
import type { Tier } from './opportunityApi';

const tierLabels: Record<Tier, string> = { candidate: '候选', established: '成立', significant: '显著' };
const supplyLabels: Record<string, string> = { not_evaluated: '暂不判断', awaiting_understanding: '等待作品理解', insufficient_coverage: '供给侧覆盖不足', covered: '已有好内容', suspected_gap: '疑似供给缺口' };
export const flagLabels: Record<string, string> = {
  insufficient_sample: '样本不足', not_calibrated: '重要性未校准', no_baseline: '基线不足',
  collection_volume_shift: '采集量变化', stale_read_model: '读模型可能过期', period_anchor_missing: '部分评论缺少周期锚点',
  author_unknown_share_high: '作品作者未知占比较高', engagement_reference_missing: '互动参照不可用',
  collection_below_target: '采集量低于目标', collection_protocol_changed: '采集协议已变化',
};
const reasonLabels: Record<string, string> = {
  protocol_unknown: '采集协议未知，无法确认两期可比', card_not_current_week: '卡片尚未更新到本周',
  current_unavailable: '本周提及率未知', previous_unavailable: '上周提及率未知',
};
export const tierLabel = (tier: Tier) => tierLabels[tier];
export const supplyLabel = (state: string) => supplyLabels[state] ?? state;
export const protocolLabel = (version: number | null) => version === null ? '未知协议' : `采集协议 ${version}`;
export const comparisonReason = (reason: string | null) => reason ? reasonLabels[reason] ?? reason : '数据不足，无法比较';
export const formatValue = (value: number | null) => value === null ? '—' : new Intl.NumberFormat('zh-CN', { maximumFractionDigits: 3 }).format(value);

export function readableError(error: unknown): string {
  if (error instanceof ApiError && error.failure.kind === 'http') {
    const code = error.failure.code;
    if (code === 'opportunity_page_changed' || code === 'invalid_cursor') return '数据已变化或服务已重启，请从第一页重新读取。';
    if (code === 'opportunity_card_sources_busy') return '来源正在更新，请稍后重试。';
    if (code === 'opportunity_card_source_unavailable') return '部分来源已不可用，请重新读取。';
    if (code === 'opportunity_period_snapshot_unavailable') return '周期索引尚未准备好，请稍后重试。';
    if (code === 'opportunity_card_disabled') return '机会卡功能未开启。';
    if (error.failure.status === 404) return '未找到这张机会卡。';
  }
  return '读取未完成，请稍后重试。';
}
