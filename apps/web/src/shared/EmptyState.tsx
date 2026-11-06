import type { ReactNode } from 'react';
import styles from './EmptyState.module.css';

export type EmptyKind = 'first_use' | 'no_match' | 'no_material' | 'unavailable_feature';

interface Props {
  /** 缺的是哪一类：首次使用、无命中、无可用材料、功能未接入。 */
  kind?: EmptyKind;
  /** 缺少什么；这一块的主句。 */
  title: string;
  /** 原因与边界：为什么空、这个空不证明什么。 */
  children?: ReactNode;
  /** 已经真实可用的下一步；没有就不放。 */
  action?: ReactNode;
}

/** LIDS C42 候选：空态。读取失败与加载中不归它表达。 */
export function EmptyState({ kind = 'no_match', title, children, action }: Props) {
  return (
    <section className={styles.empty} data-lids-component="C42" data-empty-kind={kind}>
      <strong>{title}</strong>
      {children === undefined ? null : <p>{children}</p>}
      {action === undefined ? null : <div className={styles.action}>{action}</div>}
    </section>
  );
}
