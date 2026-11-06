import type { ReactNode } from 'react';
import styles from './InlineNotice.module.css';

export type NoticeTone = 'info' | 'warning' | 'error';

interface Props {
  /** info 说明与用法提示；warning 有界范围或条件限制；error 局部读取/来源不可用。 */
  tone?: NoticeTone;
  /** 原因、影响与下一步；关键限制要写在这句话里，不能只靠颜色。 */
  children: ReactNode;
  /** 已经可用的下一步；没有就不放。 */
  action?: ReactNode;
}

/** LIDS C43 候选：局部提示与限制。整组读取失败归 C41，动作结果归 C44。 */
export function InlineNotice({ tone = 'info', children, action }: Props) {
  return (
    <div
      className={styles.notice}
      data-lids-component="C43"
      data-tone={tone}
      role={tone === 'error' ? 'alert' : 'status'}
    >
      <p className={styles.text}>{children}</p>
      {action === undefined ? null : <div className={styles.action}>{action}</div>}
    </div>
  );
}
