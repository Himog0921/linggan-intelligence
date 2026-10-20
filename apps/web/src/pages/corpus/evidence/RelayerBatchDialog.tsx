import { ActionButton } from '../../../shared/ActionButton';
import { useEffect, useId, useRef, useState } from 'react';
import { ApiError } from '../../../shared/api';
import { ModalDialog } from '../../../corpus/ModalDialog';
import { saveBatch, type Batch, type BatchItem } from './batchStore';
import { fetchRelayerPlan, relayerDossier } from './dossierApi';
import styles from './Dialogs.module.css';

interface RelayerBatchDialogProps {
  /** 打开者已把这份批量登记为断点（saveBatch）；对话框只在它之上逐篇推进。 */
  initial: Batch;
  onClose: () => void;
  /** 断点变化：「继续上次整理」按钮据此刷新。 */
  onChange: () => void;
}

type Running = 'idle' | 'preview' | 'execute';

/** 服务器明确拒绝是「未完成」；没有得到答复（或 2xx 却看不懂）是「未确认」——写入可能已经发生。 */
function failureOf(error: unknown): 'answered' | 'unconfirmed' {
  return error instanceof ApiError && error.failure.kind === 'http' ? 'answered' : 'unconfirmed';
}

function itemText(item: BatchItem, running: Running): string {
  const head = `${item.work.slice(0, 8)} · `;
  if (item.state === 'done' && item.result) return `${head}更新 ${item.result.updated} 个布局，已是当前版本 ${item.result.alreadyCurrent} 个，跳过 ${item.result.skipped} 个`;
  if (item.state === 'failed') {
    return `${head}${item.failure === 'unconfirmed' ? '本篇结果未确认，可能已经生效；请重新预览确认后再试' : '本篇处理失败，可重新预览后重试'}`;
  }
  if (item.plan) return `${head}${item.plan.pendingLayouts} 个待分层，${item.plan.alreadyCurrent} 个已是当前版本，${item.plan.needsOcr} 个需先识字`;
  return `${head}${running === 'preview' ? '正在读取处理范围…' : '范围尚未读取'}`;
}

/**
 * 批量整理：对固定的（至多 200 篇）作品逐篇重复单篇的「重新分层」，没有后台调度。
 * 先逐篇读取各自的计划（含各自的修订），再逐篇写入；暂停只发生在篇间，Esc 视为暂停；已完成的结果在数据库里保留，
 * 刷新后可「继续上次整理」。任何一篇失败都丢弃它的旧计划，必须重新预览——这本来就是「先核对再重试」。
 */
export function RelayerBatchDialog({ initial, onClose, onChange }: RelayerBatchDialogProps) {
  const titleId = useId();
  const [batch, setBatch] = useState(initial);
  const [running, setRunning] = useState<Running>('idle');
  const working = useRef(initial);
  const paused = useRef(false);
  const alive = useRef(true);
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
      paused.current = true;
    };
  }, []);

  function update(index: number, patch: Partial<BatchItem>) {
    const next: Batch = { ...working.current, items: working.current.items.map((item, position) => (position === index ? { ...item, ...patch } : item)) };
    working.current = next;
    saveBatch(next);
    onChange();
    if (alive.current) setBatch(next);
  }

  async function run(kind: Exclude<Running, 'idle'>) {
    if (running !== 'idle') return;
    paused.current = false;
    setRunning(kind);
    try {
      for (let index = 0; index < working.current.items.length; index += 1) {
        const item = working.current.items[index];
        if (!item || paused.current || !alive.current) break;
        if (item.state === 'done') continue;
        if (kind === 'execute' && !item.plan) continue;
        try {
          if (kind === 'preview') {
            update(index, { state: 'ready', plan: await fetchRelayerPlan(item.work, working.current.domain), failure: null });
          } else if (item.plan) {
            const receipt = await relayerDossier(item.work, working.current.domain, item.plan.expectedRevision);
            update(index, { state: 'done', result: { updated: receipt.updated, alreadyCurrent: receipt.alreadyCurrent, skipped: receipt.skipped }, failure: null });
          }
        } catch (error) {
          update(index, { state: 'failed', plan: null, failure: failureOf(error) });
        }
      }
    } finally {
      if (alive.current) setRunning('idle');
    }
  }

  const done = batch.items.filter(item => item.state === 'done').length;
  const failed = batch.items.filter(item => item.state === 'failed').length;
  const close = () => {
    paused.current = true;
    onClose();
  };
  return (
    <ModalDialog labelledBy={titleId} className={styles.dialog} onClose={close}>
      <div className={styles.form}>
        <header className={styles.head}>
          <div>
            <small>批量 · 研究底稿</small>
            <h2 id={titleId}>重新整理选定作品的图片文字</h2>
          </div>
        </header>
        <div className={styles.body}>
          <p role="status">{`${batch.items.length} 篇固定作品 · 已处理 ${done} 篇 · 失败 ${failed} 篇 · 尚未处理 ${batch.items.length - done - failed} 篇`}</p>
          <p className={styles.copy}>仅使用已经取得的 OCR 行。缺少识别结果的图片不会自动识字或补采。关闭或暂停后，已完成结果保留；刷新页面后可继续未完成范围。</p>
          <ul className={styles.list}>
            {batch.items.map(item => (
              <li key={item.work}>{itemText(item, running)}</li>
            ))}
          </ul>
        </div>
        <footer className={styles.foot}>
          <button type="button" disabled={running !== 'idle'} onClick={() => void run('preview')}>
            预览 / 刷新范围
          </button>
          <ActionButton variant="primary" disabled={running !== 'idle' || !batch.items.some(item => item.state !== 'done' && item.plan)} onClick={() => void run('execute')}>
            开始 / 继续整理
          </ActionButton>
          <button type="button" disabled={running === 'idle'} onClick={() => (paused.current = true)}>
            处理完当前篇后暂停
          </button>
          <span className={styles.spacer} />
          <button type="button" onClick={close}>
            关闭
          </button>
        </footer>
      </div>
    </ModalDialog>
  );
}
