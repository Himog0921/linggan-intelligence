export const PHASES = ['原声提取', '问题归并', '归并核验'];
export const STATES: Record<string, string> = { needs_support: '等待更多原声', pending: '等待', running: '执行中', succeeded: '已完成', failed: '失败', skipped: '未执行', cancelled: '已取消' };
export const REASONS: Record<string, string> = { ai_dispatch_unknown: '发送结果未知，请先核对调用记录', ai_source_unavailable: '来源当前不可读取', ai_output_invalid: '结果未通过校验', comment_publication_withheld: '材料、许可或控制状态已变化', ai_input_limit_before_dispatch: '完整材料超过模型输入额度，未发送', ai_provider_failed: '模型调用失败' };

import type { ResearchRun } from './understandingApi';
import type { ObservationPoint } from './overviewDtos';
import { shortRef } from './format';

export type RunStatus = 'running' | 'completed' | 'partial' | 'failed' | 'queued' | 'cancelled' | 'unknown';
export const STATUS_LABEL: Record<RunStatus, string> = { running: '进行中', completed: '已完成', partial: '部分失败', failed: '失败', queued: '等待执行', cancelled: '已取消', unknown: '状态待核对' };
export const STATUS_CLASS: Record<RunStatus, string> = { running: 'is-running', completed: 'is-completed', partial: 'is-partial', failed: 'is-failed', queued: 'is-queued', cancelled: 'is-queued', unknown: 'is-queued' };
export const UNIT_STATUS: Record<string, RunStatus> = { pending: 'queued', queued: 'queued', running: 'running', succeeded: 'completed', failed: 'failed', cancelled: 'cancelled', skipped: 'cancelled' };

/** With unit counts (CONTRACT §1) the status is derived from units; older replies fall back to the batch control state. */
export function runStatus(run: ResearchRun): RunStatus {
  if (run.unitCounts) {
    const { completed, failed, pending, cancelled } = run.unitCounts;
    return pending > 0 ? 'running' : failed > 0 ? (completed > 0 ? 'partial' : 'failed') : cancelled > 0 && completed === 0 ? 'cancelled' : 'completed';
  }
  return run.state === 'open' ? 'running' : run.state === 'completed' ? 'completed' : run.state === 'cancelled' ? 'cancelled' : 'unknown';
}
export function recordsActivity(runs: ResearchRun[]) {
  return { active: runs.filter(run => runStatus(run) === 'running').length, failed: runs.filter(run => (run.unitCounts?.failed ?? 0) > 0).length };
}

export type SeriesRange = 'fit' | 7 | 28 | 56;
export const RANGES: readonly [SeriesRange, string][] = [['fit', '有数据以来'], [7, '近 7 天'], [28, '近 28 天'], [56, '近 56 天']];

/** 「有数据以来」：从第一天有评论或信号的日期前两天开始，至少 7 天。没有任何数据时取最近 7 天。 */
export function rangePoints(points: ObservationPoint[], range: SeriesRange): ObservationPoint[] {
  if (range !== 'fit') return points.slice(-range);
  const first = points.findIndex(p => (p.newObservedCommentCount ?? 0) > 0 || (p.acceptedSignalCount ?? 0) > 0);
  if (first < 0) return points.slice(-7);
  return points.slice(Math.max(0, Math.min(first - 2, points.length - 7)));
}


export const EMBLEMS = ['phoenix', 'dragon', 'tortoise', 'tiger'] as const;
export type Emblem = typeof EMBLEMS[number];

/** Stable 32-bit FNV-1a hash so the same problem always gets the same emblem and banner number. */
function hash(value: string) {
  let h = 0x811c9dc5;
  for (let i = 0; i < value.length; i++) { h ^= value.charCodeAt(i); h = Math.imul(h, 0x01000193) >>> 0; }
  return h;
}
export const emblemFor = (problemRef: string): Emblem => EMBLEMS[hash(problemRef) % EMBLEMS.length]!;
/** Two decorative digits for the banner; not an identifier. */
export const bannerCode = (problemRef: string) => String(hash(problemRef + ':code') % 100).padStart(2, '0');


export const archiveId = (problemRef: string) => shortRef(problemRef);
