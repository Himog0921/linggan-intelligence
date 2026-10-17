import styles from './Controls.module.css';

/** 单次批量动作的作品上限（评论深采、整理图片文字都按它判断）。 */
export const SELECTION_LIMIT = 200;

interface SelectionBarProps {
  count: number;
  hasDomain: boolean;
  /** 刷新前留下了未完成的批量整理：显示「继续上次整理」。 */
  hasPendingBatch: boolean;
  onSelectPage: () => void;
  onDeepCollect: () => void;
  onOpenBatch: () => void;
  onResumeBatch: () => void;
}

/**
 * 勾选与批量动作。勾选集合独立于检查器的选中；「评论深采」把集合交给采集页（真实动作与回执由采集页负责），
 * 「整理图片文字」在页内弹窗里逐篇重新分层。两者都要有领域，且勾选 1–200 篇；超过 200 篇就地说明，而不是只禁用。
 */
export function SelectionBar({ count, hasDomain, hasPendingBatch, onSelectPage, onDeepCollect, onOpenBatch, onResumeBatch }: SelectionBarProps) {
  const usable = hasDomain && count > 0 && count <= SELECTION_LIMIT;
  return (
    <div className={styles.selection} role="group" aria-label="批量操作">
      <button type="button" disabled={!hasDomain} onClick={onSelectPage}>
        勾选本页
      </button>
      <span role="status" aria-live="polite">{`已勾选 ${count} 篇作品`}</span>
      {count > SELECTION_LIMIT ? <span className={styles.limit}>{`最多 ${SELECTION_LIMIT} 篇，请减少勾选`}</span> : null}
      <button type="button" disabled={!usable} onClick={onDeepCollect}>
        评论深采
      </button>
      <button type="button" disabled={!usable} onClick={onOpenBatch}>
        整理图片文字
      </button>
      {hasPendingBatch ? (
        <button type="button" className={styles.resume} onClick={onResumeBatch}>
          继续上次整理
        </button>
      ) : null}
    </div>
  );
}
