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
  /** 有领域才有勾选框。 */
  canCheck: boolean;
  /** `user`：点击或键盘确认（新增历史、展开检查器）；`auto`：方向键移动（替换历史）。 */
  onSelect: (item: WorkItem, source: 'user' | 'auto') => void;
  onCheck: (ref: string, checked: boolean) => void;
  ref?: Ref<HTMLDivElement>;
}

const MOVES = ['ArrowDown', 'ArrowUp', 'Home', 'End'];

/**
 * 作品列表的三种排版（研读／表格／封面）。语义是真列表或真表格：每行一个可聚焦的选择控件（漫游焦点，
 * 选中行是 tab 停靠点），方向键 ↑↓ Home End 在行间移动并选中；鼠标点整行都选中。勾选框是独立控件，不触发行选择。
 */
export function WorkList({ items, layout, selectedRef, checked, canCheck, onSelect, onCheck, ref }: WorkListProps) {
  const stopRef = items.some(item => item.ref === selectedRef) ? selectedRef : (items[0]?.ref ?? null);
  const rowProps = (item: WorkItem): RowProps => ({
    item,
    selected: item.ref === selectedRef,
    tabStop: item.ref === stopRef,
    canCheck,
    checked: checked.has(item.ref),
    onSelect: picked => onSelect(picked, 'user'),
    onCheck,
  });

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    const target = event.target as HTMLElement;
    if (!target.matches('[data-select-target]') || !MOVES.includes(event.key)) return;
    event.preventDefault();
    const targets = [...event.currentTarget.querySelectorAll<HTMLElement>('[data-select-target]')];
    const index = targets.indexOf(target);
    if (targets.length === 0) return;
    let next = index;
    if (event.key === 'Home') next = 0;
    if (event.key === 'End') next = targets.length - 1;
    if (event.key === 'ArrowDown') next = (index + 1) % targets.length;
    if (event.key === 'ArrowUp') next = (index - 1 + targets.length) % targets.length;
    const element = targets[next];
    const ref = element?.closest<HTMLElement>('[data-work-ref]')?.dataset.workRef;
    const item = items.find(candidate => candidate.ref === ref);
    if (item && element) {
      onSelect(item, 'auto');
      element.focus();
    }
  }

  return (
    <div ref={ref} className={styles.list} data-layout={layout} onKeyDown={onKeyDown}>
      {layout === 'table' ? (
        <table className={styles.table}>
          <caption className="v7-sr-only">作品材料集合</caption>
          <thead>
            <tr>
              <th scope="col">作品</th>
              <th scope="col">作者</th>
              <th scope="col">材料</th>
              <th scope="col">
                <span className="v7-sr-only">互动：</span>
                <span className={styles.metricHead}>
                  {ENGAGEMENT_KINDS.map(([kind, label]) => (
                    <MetricIcon key={kind} kind={kind} label={label} />
                  ))}
                </span>
              </th>
              <th scope="col">发布时间</th>
              <th scope="col">最近观察</th>
            </tr>
          </thead>
          <tbody>
            {items.map(item => (
              <TableRow key={item.ref} {...rowProps(item)} />
            ))}
          </tbody>
        </table>
      ) : (
        <ul className={styles.items} aria-label="作品材料集合">
          {items.map((item, index) =>
            layout === 'cover' ? <CoverCard key={item.ref} {...rowProps(item)} ordinal={index + 1} /> : <ResearchRow key={item.ref} {...rowProps(item)} />,
          )}
        </ul>
      )}
    </div>
  );
}
