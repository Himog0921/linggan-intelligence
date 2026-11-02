import { ActionButton } from '../../../shared/ActionButton';
import { useId } from 'react';
import { ModalDialog } from '../../../corpus/ModalDialog';
import styles from './Dialogs.module.css';

interface CommentDeepConfirmDialogProps {
  /** 「采指定条数」的条数；尽量采全时是 null。 */
  target: number | null;
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * 评论深采的确认（Mog 2026-10-23、2026-11-02）。保留真实后果——会让工位访问平台、到底即停、
 * 可在采集任务里暂停或停止，不声称立即得到结果；去掉栏目眉题和重复的长清单。
 * 「取消」排在提交之前并持有初始焦点；确认一次只提交一次。
 */
export function CommentDeepConfirmDialog({ target, onConfirm, onCancel }: CommentDeepConfirmDialogProps) {
  const titleId = useId();
  return (
    <ModalDialog labelledBy={titleId} className={`${styles.dialog} ${styles.confirmDialog}`} onClose={onCancel}>
      <div className={styles.form}>
        <header className={styles.head}>
          <h2 id={titleId}>确认评论采集</h2>
        </header>
        <div className={styles.body}>
          <p>
            {target === null
              ? '从评论区开头采集当前可见的评论与回复，重复内容不重复计数。'
              : `从评论区开头采集，最多新增 ${target} 条不重复评论（含回复），到底即停。`}
          </p>
          <p className={styles.muted}>缺少详情时会补采详情和媒体；可在采集任务中暂停或停止。</p>
        </div>
        <footer className={styles.foot}>
          <span className={styles.spacer} />
          <button type="button" data-initial-focus onClick={onCancel}>
            取消
          </button>
          <ActionButton variant="ink" onClick={onConfirm}>
            提交采集
          </ActionButton>
        </footer>
      </div>
    </ModalDialog>
  );
}
