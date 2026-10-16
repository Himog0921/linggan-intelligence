import type { Ref } from 'react';
import { SearchField, type SearchFieldHandle } from './SearchField';
import { conditionChips, formatCount, quickPressed, type ConditionChip, type CreatorQuery, type QuickKey, type SortKey } from './creatorsQuery';
import type { CreatorList } from './creatorsTypes';
import styles from './CreatorsPage.module.css';

interface CreatorControlsProps {
  query: CreatorQuery;
  /** 当前范围的读数；还没读到（读取中、失败、未选领域）时为 null。 */
  data: CreatorList | null;
  /** 结果条左侧的一句话：读取中 / 请选择领域 / 读取失败 / 「N 位创作者」。 */
  resultText: string;
  searchHandle: Ref<SearchFieldHandle>;
  onSearchCommit: (text: string) => void;
  onSearchMode: (mode: 'author' | 'work', text: string) => void;
  onOpenFilter: () => void;
  onSort: (sort: SortKey) => void;
  onRefresh: () => void;
  onQuick: (key: QuickKey) => void;
  onOpenPolicy: () => void;
  onOpenScope: () => void;
  onRemoveChip: (chip: ConditionChip) => void;
  onClearChips: () => void;
}

function rangeText(query: CreatorQuery): string {
  if (!query.publishedFrom && !query.publishedTo) return '全部日期';
  return `${query.publishedFrom || '不限'} 至 ${query.publishedTo || '不限'}（不含）`;
}

/**
 * 表格上方的全部控件：搜索与排序、快捷视图、分析状态、结果条、条件芯片。
 * 范围统计（快捷视图上的数字）与筛选结果（结果条的「N 位创作者」）分开显示：前者只跟领域、用途、
 * 平台和发表日期变化，后者随搜索与筛选变化。
 */
export function CreatorControls(props: CreatorControlsProps) {
  const { query, data, resultText } = props;
  const chips = conditionChips(query);
  const pressed = quickPressed(query);
  const stats = data?.stats ?? null;
  const hasRule = data !== null && data.policy.likeThreshold !== null;
  const outsideKnown = stats !== null && stats.outside !== null;
  const scope = `${query.usageRole === 'reference' ? '参照' : '主研究'} · ${rangeText(query)}`;
  return (
    <>
      <div className={styles.toolbar}>
        <SearchField
          handle={props.searchHandle}
          mode={query.searchMode}
          value={query.query}
          onCommit={props.onSearchCommit}
          onModeChange={props.onSearchMode}
        />
        <button type="button" onClick={props.onOpenFilter}>
          筛选 <span className={styles.count}>{chips.length}</span>
        </button>
        <label className="v7-sr-only" htmlFor="creator-sort">
          排序
        </label>
        <select id="creator-sort" value={query.sort} onChange={event => props.onSort(event.target.value as SortKey)}>
          <option value="recent">最近收录</option>
          <option value="related_works">命中相关作品数</option>
          <option value="high_likes">命中最高点赞</option>
          <option value="viral_works" disabled={!hasRule}>
            命中相关爆款数
          </option>
        </select>
        <button type="button" className={styles.primary} onClick={props.onRefresh}>
          刷新结果
        </button>
      </div>
      <div className={styles.viewBar}>
        <nav className={styles.quick} aria-label="创作者视图">
          <button type="button" aria-pressed={pressed.all} onClick={() => props.onQuick('all')}>
            全部创作者{stats ? ` ${formatCount(stats.authors)}` : ''}
          </button>
          <button type="button" aria-pressed={pressed.outside} disabled={!outsideKnown} onClick={() => props.onQuick('outside')}>
            未加入观察{stats ? ` ${formatCount(stats.outside)}` : ''}
          </button>
          <button type="button" aria-pressed={pressed.high} disabled={!outsideKnown} onClick={() => props.onQuick('high')}>
            {hasRule ? '未观察的爆款作者' : '未观察的高赞作者'}
            {stats && hasRule ? ` ${formatCount(stats.outsideViral)}` : ''}
          </button>
        </nav>
        <button type="button" className={`${styles.textButton} ${styles.analysisButton}`} aria-label="发现设置" onClick={props.onOpenPolicy} disabled={data === null}>
          <span className={`${styles.analysisDot} ${data?.policy.analysisEnabled ? styles.analysisOn : ''}`}>
            {data === null ? '分析状态待读取' : data.policy.analysisEnabled ? '自动分析已开启' : '自动分析未开启'}
          </span>
          <span aria-hidden="true">
            <span className={styles.analysisSep}>{' · '}</span>设置
          </span>
        </button>
      </div>
      <div className={styles.resultBar}>
        <strong className={styles.resultCount} aria-live="polite">
          {resultText}
        </strong>
        <div className={styles.scopeTools}>
          <span className={styles.scopeSummary} title={scope}>
            {scope}
          </span>
          <button type="button" className={styles.textButton} onClick={props.onOpenScope} disabled={data === null}>
            材料范围
          </button>
        </div>
      </div>
      {chips.length > 0 ? (
        <div className={styles.conditions} role="group" aria-label="当前筛选条件">
          {chips.map(chip => (
            <button
              key={`${chip.key}:${chip.value ?? ''}`}
              type="button"
              title={chip.text}
              aria-label={`移除条件：${chip.text}`}
              onClick={() => props.onRemoveChip(chip)}
            >
              {chip.text} ×
            </button>
          ))}
          <button type="button" onClick={props.onClearChips}>
            清除筛选
          </button>
        </div>
      ) : null}
    </>
  );
}
