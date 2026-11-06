import type { ReactNode } from 'react';
import styles from './LoadingState.module.css';

export type LoadPhase = 'initial' | 'refreshing' | 'stale_error' | 'ready';

interface Props {
  /** initial 只有状态行；refreshing / stale_error 保留上次结果；ready 只渲染内容。 */
  phase: LoadPhase;
  /** 读的是什么：「领域配置」「巡检记录」。 */
  subject: string;
  /** stale_error 的原因；省略时用「{subject}当前读不到。」 */
  error?: ReactNode;
  /** 接在原因后面的保留说明；省略时用「以下保留上次读取结果。」 */
  retained?: ReactNode;
  /** 读取失败时的重新读取动作。 */
  action?: ReactNode;
  /** 旧结果是否仍在页面上；省略时以是否给了 children 为准。 */
  retainsOld?: boolean;
  /** 上次读取结果；给不给决定刷新与读取失败时保留不保留旧内容。 */
  children?: ReactNode;
}

/** LIDS C41 候选：读取与刷新状态。首次读取不渲染旧内容，刷新不遮住可读旧值。 */
export function LoadingState({ phase, subject, error, retained, action, retainsOld, children }: Props) {
  if (phase === 'initial') {
    return (
      <p className={styles.status} role="status" aria-busy="true" data-lids-component="C41" data-load-phase="initial">
        {`正在读取${subject}…`}
      </p>
    );
  }
  // 三个阶段用同一个兄弟槽位放提示，children 始终在第二位：
  // 刷新途中提示出现/消失若改变 children 的位置，React 会卸载重建整块内容，
  // 页面里未提交的输入和展开的面板会一起丢。
  const keepsOld = retainsOld ?? children !== undefined;
  const notice =
    phase === 'refreshing' ? (
      <p className={styles.status} role="status" aria-busy="true" data-lids-component="C41" data-load-phase="refreshing">
        {`正在刷新${subject}，以下是上次读取结果。`}
      </p>
    ) : phase === 'stale_error' ? (
      <div className={styles.notice} role="alert" data-lids-component="C41" data-load-phase="stale_error">
        <p>
          {error ?? `${subject}当前读不到。`}
          {keepsOld ? retained ?? '以下保留上次读取结果。' : null}
        </p>
        {action === undefined ? null : action}
      </div>
    ) : null;
  return (
    <>
      {notice}
      {children}
    </>
  );
}
