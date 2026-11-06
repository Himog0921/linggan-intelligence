import type { ReactNode } from 'react';
import styles from './ActionFeedback.module.css';

/** 动作阶段：提交中、已被接纳、已完成、被拒绝、结果未确认、部分完成。 */
export type ReceiptState = 'pending' | 'accepted' | 'applied' | 'rejected' | 'unconfirmed' | 'partial';

const LABELS: Record<ReceiptState, string> = {
  pending: '进行中',
  accepted: '已接纳',
  applied: '已完成',
  rejected: '未完成',
  unconfirmed: '未确认',
  partial: '部分完成',
};

interface Props {
  /** 还没有回执时省略：保留一个空的播报区，结果出现后再播。 */
  state?: ReceiptState;
  /** 结果证据：发生了什么、依据是什么、不证明什么。 */
  children?: ReactNode;
  /** 请求编号：结果未确认时按它核对，不重复提交。 */
  requestRef?: string;
  /** 服务器错误码等技术旁注。 */
  code?: string | null;
  /** 已经可用的下一步。 */
  action?: ReactNode;
  id?: string;
}

/** LIDS C44 候选：动作回执。阶段先用文字说清，颜色只做加强。 */
export function ActionFeedback({ state, children, requestRef, code, action, id }: Props) {
  if (state === undefined) {
    return <div id={id} className={styles.empty} role="status" aria-live="polite" />;
  }
  return (
    <div
      id={id}
      className={styles.feedback}
      data-lids-component="C44"
      data-receipt-state={state}
      role="status"
      aria-live="polite"
      aria-busy={state === 'pending' || undefined}
    >
      <b className={styles.label}>{LABELS[state]}</b>
      <span className={styles.body}>
        {children === undefined ? null : <span>{children}</span>}
        {requestRef === undefined ? null : <span className={styles.ref}>请求编号 {requestRef}</span>}
        {code ? <code className={styles.code}>{code}</code> : null}
      </span>
      {action === undefined ? null : <span className={styles.action}>{action}</span>}
    </div>
  );
}
