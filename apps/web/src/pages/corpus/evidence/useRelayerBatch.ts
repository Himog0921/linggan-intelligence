import { useState } from 'react';
import { createBatch, hasPendingBatch, loadBatch, saveBatch, type Batch } from './batchStore';

/**
 * 批量整理弹窗的开合与断点。打开时先把固定范围登记为断点（只有作品引用和各篇状态，没有内容），
 * 弹窗在它之上逐篇推进；刷新后「继续上次整理」读回断点。
 */
export function useRelayerBatch() {
  const [batch, setBatch] = useState<Batch | null>(null);
  const [pending, setPending] = useState(hasPendingBatch);
  const refresh = () => setPending(hasPendingBatch());

  /** 勾选 1–200 篇作品时才会打开；空或超限什么也不做。 */
  function open(works: readonly string[], domain: string) {
    const created = createBatch(works, domain);
    if (created === null) return;
    saveBatch(created);
    setBatch(created);
    refresh();
  }

  function resume() {
    const stored = loadBatch();
    if (stored !== null) setBatch(stored);
    refresh();
  }

  function close() {
    setBatch(null);
    refresh();
  }

  return { batch, pending, open, resume, close, refresh };
}
