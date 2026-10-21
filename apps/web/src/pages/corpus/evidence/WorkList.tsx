import type { KeyboardEvent, Ref } from 'react';
import type { LayoutKey } from './evidenceQuery';
import type { WorkItem } from './evidenceTypes';
import { CoverCard } from './CoverCard';
import { ENGAGEMENT_KINDS } from './evidencePresent';
import { ResearchRow, TableRow, type RowProps } from './WorkRow';
import { MetricIcon } from './WorkParts';
import styles from './WorkList.module.css';

export interface WorkListProps {
  items: readonly WorkItem[];
  layout: LayoutKey;
  selectedRef: string | null;
  checked: ReadonlySet<string>;
  /** True only in the explicit bulk-selection mode. */
  canCheck: boolean;
  onSelect: (item: WorkItem, source: 'user' | 'auto') => void;
  onCheck: (ref: string, checked: boolean) => void;
  ref?: Ref<HTMLDivElement>;
}
const MOVES = ['ArrowDown', 'ArrowUp', 'Home', 'End'];

export function WorkList({ items, layout, selectedRef, checked, canCheck, onSelect, onCheck, ref }: WorkListProps) {
  const stopRef = items.some(item => item.ref === selectedRef) ? selectedRef : items[0]?.ref ?? null;
  const rowProps = (item: WorkItem): RowProps => ({
    item, selected: item.ref === selectedRef, tabStop: item.ref === stopRef,
    canCheck, checked: checked.has(item.ref), onSelect: picked => onSelect(picked,'user'), onCheck,
  });
  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    const target = event.target as HTMLElement;
    if (!target.matches('[data-select-target]') || !MOVES.includes(event.key)) return;
    event.preventDefault();
    const targets = [...event.currentTarget.querySelectorAll<HTMLElement>('[data-select-target]')];
    const index = targets.indexOf(target);
    let next = index;
    if (event.key === 'Home') next = 0;
    if (event.key === 'End') next = targets.length-1;
    if (event.key === 'ArrowDown') next = (index+1) % targets.length;
    if (event.key === 'ArrowUp') next = (index-1+targets.length) % targets.length;
    const element = targets[next];
    const item = items.find(candidate=>candidate.ref===element?.closest<HTMLElement>('[data-work-ref]')?.dataset.workRef);
    if (item && element) { onSelect(item,'auto'); element.focus(); }
  }
  return <div ref={ref} className={styles.list} data-layout={layout} data-selecting={canCheck} onKeyDown={onKeyDown}>
    {layout === 'table' ? (
      <table className={styles.table}>
        <caption className="v7-sr-only">作品材料集合</caption>
        <colgroup>
          {canCheck ? <col className={styles.selectColumn} /> : null}
          <col className={styles.workColumn} />
          <col className={styles.authorColumn} />
          <col className={styles.materialColumn} />
          {ENGAGEMENT_KINDS.map(([kind]) => <col key={kind} className={styles.metricColumn} />)}
          <col className={styles.dateColumn} />
          <col className={styles.observedColumn} />
        </colgroup>
        <thead><tr>
          {canCheck ? <th scope="col" className={styles.selectHeader}><span className="v7-sr-only">批量勾选</span></th> : null}
          <th scope="col">作品</th><th scope="col">作者</th><th scope="col">材料</th>
          {ENGAGEMENT_KINDS.map(([kind,label]) =>
            <th key={kind} className={styles.metricHeadCell} scope="col"><MetricIcon kind={kind} label={label} /></th>)}
          <th scope="col">发布时间</th><th scope="col">最近观察</th>
        </tr></thead>
        <tbody>{items.map(item=><TableRow key={item.ref} {...rowProps(item)} />)}</tbody>
      </table>
    ) : <ul className={styles.items} aria-label="作品材料集合">
      {items.map((item,index)=>layout==='cover'
        ? <CoverCard key={item.ref} {...rowProps(item)} ordinal={index+1} />
        : <ResearchRow key={item.ref} {...rowProps(item)} />)}
    </ul>}
  </div>;
}
