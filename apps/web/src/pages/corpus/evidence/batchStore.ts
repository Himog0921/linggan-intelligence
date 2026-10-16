import type { RelayerPlan } from './dossierTypes';

/**
 * 批量整理的断点：作品稳定引用、各篇状态、计数与修订哈希——没有任何作品内容，只在本标签页内有效（P01）。
 * 刷新后可「继续上次整理」；已完成的结果在数据库里，不靠这里。
 */
export const BATCH_KEY = 'linggan-research-dossier-batch-v1';
export const BATCH_LIMIT = 200;

export type BatchState = 'pending' | 'ready' | 'done' | 'failed';

export interface BatchItem {
  work: string;
  state: BatchState;
  /** 预览读到的计划（含修订哈希）；失败后丢弃，必须重新预览。 */
  plan?: RelayerPlan | null;
  result?: { updated: number; alreadyCurrent: number; skipped: number } | null;
  /** 失败的性质：服务器明确拒绝（未完成），还是没有得到答复（未确认，可能已经生效）。 */
  failure?: 'answered' | 'unconfirmed' | null;
}

export interface Batch {
  domain: string;
  items: BatchItem[];
}

const STATES: readonly string[] = ['pending', 'ready', 'done', 'failed'];

function isItem(value: unknown): value is BatchItem {
  if (typeof value !== 'object' || value === null) return false;
  const item = value as Record<string, unknown>;
  return typeof item.work === 'string' && typeof item.state === 'string' && STATES.includes(item.state);
}

/** 读取断点。形状不对（被手改或来自旧版本）就当作没有，不带病继续。 */
export function loadBatch(): Batch | null {
  try {
    const value: unknown = JSON.parse(sessionStorage.getItem(BATCH_KEY) ?? 'null');
    if (typeof value !== 'object' || value === null) return null;
    const batch = value as { domain?: unknown; items?: unknown };
    if (typeof batch.domain !== 'string' || !Array.isArray(batch.items) || batch.items.length === 0 || batch.items.length > BATCH_LIMIT) return null;
    return batch.items.every(isItem) ? { domain: batch.domain, items: batch.items } : null;
  } catch {
    return null;
  }
}

export function saveBatch(batch: Batch): void {
  try {
    sessionStorage.setItem(BATCH_KEY, JSON.stringify(batch));
  } catch {
    /* 存储不可用时，内存里的范围仍可用，只是刷新后不能继续。 */
  }
}

export function hasPendingBatch(): boolean {
  return loadBatch()?.items.some(item => item.state !== 'done') ?? false;
}

/** 新批量：去重，最多 200 篇；空或超限返回 null。 */
export function createBatch(works: readonly string[], domain: string): Batch | null {
  const unique = [...new Set(works)];
  return unique.length === 0 || unique.length > BATCH_LIMIT ? null : { domain, items: unique.map(work => ({ work, state: 'pending' })) };
}
