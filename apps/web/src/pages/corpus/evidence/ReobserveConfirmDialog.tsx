import { ActionButton } from '../../../shared/ActionButton';
import { useId } from 'react';
import { ModalDialog } from '../../../corpus/ModalDialog';
import styles from './Dialogs.module.css';

interface ReobserveConfirmDialogProps {
  /** 上一次提交没有得到答复：可能已经入队，再提交可能再形成一张工单。 */
  afterUnknown: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * 立即复观测的确认（DR-3）。保留真实后果与不可撤销，去掉栏目眉题和「会／不会」长清单；
 * 「取消」排在提交之前并持有初始焦点，原生模态对话框的焦点约束不变。未确认之后再次提交时
 * 写明可能重复执行，建议先核对；不改成自动重试或伪幂等。
 */
export function ReobserveConfirmDialog({ afterUnknown, onConfirm, onCancel }: ReobserveConfirmDialogProps) {
  const titleId = useId();
  return (
    <ModalDialog labelledBy={titleId} className={`${styles.dialog} ${styles.confirmDialog}`} onClose={onCancel}>
      <div className={styles.form}>
        <header className={styles.head}>
          <h2 id={titleId}>确认复观测</h2>
        </header>
        <div className={styles.body}>
          <p>重新访问平台，读取这篇作品的详情、评论（最多 30 条）与回复。提交后不能撤销。</p>
          <p className={styles.muted}>提交后等待执行，不下载新媒体。</p>
          {afterUnknown ? (
            <p className={styles.notice}>上次提交结果未确认，再次提交可能重复执行。建议先核对。</p>
          ) : null}
        </div>
        <footer className={styles.foot}>
          <span className={styles.spacer} />
          <button type="button" data-initial-focus onClick={onCancel}>
            取消
          </button>
          <ActionButton variant="ink" onClick={onConfirm}>
            {afterUnknown ? '仍然提交' : '提交复观测'}
          </ActionButton>
        </footer>
      </div>
    </ModalDialog>
  );
}
