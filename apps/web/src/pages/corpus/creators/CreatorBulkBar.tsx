import { ActionButton } from '../../../shared/ActionButton';
import styles from './CreatorsPage.module.css';

interface CreatorBulkBarProps {
  count: number;
  onExport: () => void;
  onClear: () => void;
}

/** 已选作者的批量动作条：只在有选择时出现。导出的是选中的这些人，不是当前页。 */
export function CreatorBulkBar({ count, onExport, onClear }: CreatorBulkBarProps) {
  if (count === 0) return null;
  return (
    <div className={styles.bulkBar} role="status">
      <span>
        已选 <strong className={styles.bulkCount}>{count.toLocaleString('zh-CN')}</strong> 位创作者
      </span>
      <ActionButton variant="quiet" onClick={onExport}>
        导出名单
      </ActionButton>
      <button type="button" className={styles.textButton} onClick={onClear}>
        取消选择
      </button>
    </div>
  );
}
