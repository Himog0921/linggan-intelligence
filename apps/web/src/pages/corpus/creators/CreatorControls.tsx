import { CREATOR_AI_RETIREMENT_NOTICE } from '../../settings/aiSettingsModel';
import type { Ref } from 'react';
import { ActionButton } from '../../../shared/ActionButton';
import { CreatorSelect } from './CreatorSelect';
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
  refreshing: boolean;
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
      <div className={styles.workTools}>
      <div className={styles.toolbar} data-region="Z04-T">
        <SearchField
          handle={props.searchHandle}
          mode={query.searchMode}
          value={query.query}
          onCommit={props.onSearchCommit}
          onModeChange={props.onSearchMode}
        />
        <div className={styles.queryActions}>
        <ActionButton onClick={props.onOpenFilter} disabled={data === null}>
          筛选 <span className={styles.count}>{chips.length}</span>
        </ActionButton>
        <CreatorSelect label="排序" value={query.sort} onChange={props.onSort}
          options={[
            { value: 'recent', label: '最近收录' }, { value: 'related_works', label: '命中相关作品数' },
            { value: 'high_likes', label: '命中最高点赞' }, { value: 'viral_works', label: '命中相关爆款数', disabled: !hasRule },
          ]} />
        <ActionButton onClick={props.onRefresh} busy={props.refreshing} disabled={data === null}>
          {props.refreshing ? '读取中…' : '刷新结果'}
        </ActionButton>
        </div>
      </div>
      <div className={styles.viewBar} data-region="Z04-V">
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
          <span className={styles.analysisDot}>
            {CREATOR_AI_RETIREMENT_NOTICE}
          </span>
          <span aria-hidden="true">
            <span className={styles.analysisSep}>{' · '}</span>设置
          </span>
        </button>
      </div>
      </div>
      <div className={styles.resultBar} data-region="Z04-R">
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
