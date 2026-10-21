import type { MouseEvent } from 'react';
import { CoverPreview } from './Media';
import { StateLine } from './Primitives';
import { compactMoment, laneSummary, materialSummary, metricReading, publishedCopy, workTitle, ENGAGEMENT_KINDS } from './evidencePresent';
import type { WorkItem } from './evidenceTypes';
import { AuthorFact, Engagement, MaterialRail, QuoteBlock } from './WorkParts';
import styles from './WorkList.module.css';

export interface RowProps {
  item: WorkItem;
  selected: boolean;
  tabStop: boolean;
  canCheck: boolean;
  checked: boolean;
  onSelect: (item: WorkItem) => void;
  onCheck: (ref: string, checked: boolean) => void;
}

function stop(event: MouseEvent) { event.stopPropagation(); }

/** The 36px hit area floats over the object; it never participates in the reading-row height. */
export function CheckBox({ item, checked, onCheck }: { item: WorkItem; checked: boolean; onCheck: (ref: string, checked: boolean) => void }) {
  return <label className={styles.check} data-evidence-check="" onClick={stop}>
    <input type="checkbox" checked={checked} aria-label={`勾选作品：${workTitle(item)}`}
      onChange={event => onCheck(item.ref, event.currentTarget.checked)} />
  </label>;
}

function SelectTitle({ item, selected, tabStop }: Pick<RowProps, 'item' | 'selected' | 'tabStop'>) {
  const title = workTitle(item);
  return <button type="button" className={styles.titleButton} data-select-target=""
    aria-current={selected ? 'true' : undefined} tabIndex={tabStop ? 0 : -1} title={title}>{title}</button>;
}

/** V2.2: 78×104 cover, 106px row including its edges. Title / quote / author / metrics are four fixed tracks. */
export function ResearchRow({ item, selected, tabStop, canCheck, checked, onSelect, onCheck }: RowProps) {
  const detail = laneSummary(item.lanes, 'detail')?.state ?? 'UNKNOWN';
  const observed = compactMoment(item.lastObservedAt);
  return <li className={styles.row} data-selected={selected} data-checked={canCheck && checked}
    data-selecting={canCheck} data-work-ref={item.ref} onClick={() => onSelect(item)}>
    <article className={styles.article}>
      <CoverPreview title={workTitle(item)} url={item.media.coverUrl} state={item.media.coverState} />
      <div className={styles.identity}>
        <h2 className={styles.title}><SelectTitle item={item} selected={selected} tabStop={tabStop} /></h2>
        <QuoteBlock fragment={item.fragment} />
        <div className={styles.meta}><AuthorFact item={item} /><span className={styles.time}>{publishedCopy(item.display, { compact: true })}</span></div>
        <Engagement engagement={item.display.engagement} />
      </div>
      <div className={styles.side}>
        <MaterialRail summary={materialSummary(item)} />
        <StateLine state={detail} />
        <span className={styles.observed} title={item.lastObservedAt ? `最近观察 ${item.lastObservedAt}` : undefined}>
          {`最近观察 ${observed ?? '未知'}`}
        </span>
      </div>
      {canCheck ? <CheckBox item={item} checked={checked} onCheck={onCheck} /> : null}
    </article>
  </li>;
}

/** One real table column per metric: header icons and values have the same column widths. */
export function TableRow({ item, selected, tabStop, canCheck, checked, onSelect, onCheck }: RowProps) {
  const detail = laneSummary(item.lanes, 'detail')?.state ?? 'UNKNOWN';
  const observed = compactMoment(item.lastObservedAt);
  return <tr className={styles.tableRow} data-selected={selected} data-checked={canCheck && checked}
    data-work-ref={item.ref} onClick={() => onSelect(item)}>
    {canCheck ? <td className={styles.checkCell}><CheckBox item={item} checked={checked} onCheck={onCheck} /></td> : null}
    <td className={styles.workCell}><SelectTitle item={item} selected={selected} tabStop={tabStop} /></td>
    <td><AuthorFact item={item} /></td>
    <td><div className={styles.materialCell}><MaterialRail summary={materialSummary(item)} /><StateLine state={detail} /></div></td>
    {ENGAGEMENT_KINDS.map(([kind,label]) =>
      <td key={kind} className={styles.metricCell} title={`${label}：${metricReading(item.display.engagement[kind])}`}>
        <span className="v7-sr-only">{label}：</span><span>{metricReading(item.display.engagement[kind])}</span>
      </td>)}
    <td className={styles.cell}>{publishedCopy(item.display, { compact: true, withFieldName: false })}</td>
    <td className={styles.cell} title={item.lastObservedAt ?? undefined}>{observed ?? '未知'}</td>
  </tr>;
}
