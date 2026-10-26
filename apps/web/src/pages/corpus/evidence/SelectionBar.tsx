import { ActionButton } from '../../../shared/ActionButton';
import styles from './Controls.module.css';

/** 单次批量动作上限：保留与现有采集任务交接的 200 篇合同。 */
export const SELECTION_LIMIT = 200;
interface SelectionBarProps {
  count: number;
  visibleCount: number;
  visibleSelected: number;
  hasDomain: boolean;
  onClear: () => void;
  onDeepCollect: () => void;
}
/** 只有明确进入选择模式时出现；选择列表与详情阅读互不干扰。 */
export function SelectionBar({ count, visibleCount, visibleSelected, hasDomain, onClear, onDeepCollect }: SelectionBarProps) {
  const usable = hasDomain && count > 0 && count <= SELECTION_LIMIT;
  return <div className={styles.selection} role="group" aria-label="批量操作">
    <span className={styles.selectedCount} role="status" aria-live="polite">
      已选 <b>{count}</b> 篇 <small>· 当前筛选 {visibleSelected}/{visibleCount}</small>
    </span>
    <span aria-hidden="true" className={styles.selectionDivider} />
    <ActionButton variant="quiet" disabled={count === 0} onClick={onClear}>清空已选</ActionButton>
    <ActionButton variant="quiet" disabled={!usable} onClick={onDeepCollect}>评论深采</ActionButton>
    {count > SELECTION_LIMIT ? <span className={styles.limit}>{`最多 ${SELECTION_LIMIT} 篇，请减少勾选`}</span> : null}
  </div>;
}
