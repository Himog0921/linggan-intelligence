import type { MouseEvent } from 'react';
import { CoverPreview } from './Media';
import { StateLine } from './Primitives';
import {
  compactMoment,
  laneSummary,
  materialSummary,
  publishedCopy,
  workTitle,
} from './evidencePresent';
import type { WorkItem } from './evidenceTypes';
import { AuthorFact, Engagement, MaterialRail, QuoteBlock } from './WorkParts';
import styles from './WorkList.module.css';

export interface RowProps {
  item: WorkItem;
  selected: boolean;
  /** 每行只有一个可聚焦的选择控件（漫游焦点）：选中行的是 0，其余是 -1。 */
  tabStop: boolean;
  /** 有领域才有勾选框。 */
  canCheck: boolean;
  checked: boolean;
  onSelect: (item: WorkItem) => void;
  onCheck: (ref: string, checked: boolean) => void;
}

function stop(event: MouseEvent) {
  event.stopPropagation();
}

/** 批量操作的勾选框：独立控件，点勾选框不触发行选择（F07）。 */
export function CheckBox({ item, checked, onCheck }: { item: WorkItem; checked: boolean; onCheck: (ref: string, checked: boolean) => void }) {
  return (
    <label className={styles.check} onClick={stop}>
      <input type="checkbox" checked={checked} aria-label="勾选作品用于批量操作" onChange={event => onCheck(item.ref, event.currentTarget.checked)} />
    </label>
  );
}

function SelectTitle({ item, selected, tabStop }: Pick<RowProps, 'item' | 'selected' | 'tabStop'>) {
  return (
    <button type="button" className={styles.titleButton} data-select-target="" aria-current={selected ? 'true' : undefined} tabIndex={tabStop ? 0 : -1}>
      {workTitle(item)}
    </button>
  );
}

/** 研读排版：预览、身份与原声、材料完整度。 */
export function ResearchRow({ item, selected, tabStop, canCheck, checked, onSelect, onCheck }: RowProps) {
  const detail = laneSummary(item.lanes, 'detail')?.state ?? 'UNKNOWN';
  const observed = compactMoment(item.lastObservedAt);
  return (
    <li className={styles.row} data-selected={selected} data-platform={item.platform.toLowerCase()} data-work-ref={item.ref} onClick={() => onSelect(item)}>
      <article className={styles.article}>
        <CoverPreview title={workTitle(item)} url={item.media.coverUrl} state={item.media.coverState} />
        <div className={styles.identity}>
          <div className={styles.eyebrow}>
            <span>{item.platform.toUpperCase() || '平台未知'}</span>
            {item.display.titleSource === 'cover_ocr' ? <span className={styles.tag}>封面 OCR</span> : null}
            {item.role === 'reference' ? <span className={styles.tag}>参照</span> : null}
          </div>
          <h2 className={styles.title}>
            <SelectTitle item={item} selected={selected} tabStop={tabStop} />
          </h2>
          <div className={styles.meta}>
            <AuthorFact item={item} />
            <span className={styles.time}>{publishedCopy(item.display, { compact: true })}</span>
          </div>
          <QuoteBlock fragment={item.fragment} />
          <Engagement engagement={item.display.engagement} />
        </div>
        <div className={styles.side}>
          <MaterialRail summary={materialSummary(item)} />
          <StateLine state={detail} />
          <span className={styles.observed} title={item.lastObservedAt ? `最近观察 ${item.lastObservedAt}` : undefined}>{`最近观察 ${observed ?? '未知'}`}</span>
        </div>
        {canCheck ? <CheckBox item={item} checked={checked} onCheck={onCheck} /> : null}
      </article>
    </li>
  );
}

/** 表格排版：同一份数据按列排开；列头在 <thead> 里，行内只有数字。 */
export function TableRow({ item, selected, tabStop, canCheck, checked, onSelect, onCheck }: RowProps) {
  const detail = laneSummary(item.lanes, 'detail')?.state ?? 'UNKNOWN';
  const observed = compactMoment(item.lastObservedAt);
  return (
    <tr className={styles.tableRow} data-selected={selected} data-platform={item.platform.toLowerCase()} data-work-ref={item.ref} onClick={() => onSelect(item)}>
      <td className={styles.workCell}>
        <SelectTitle item={item} selected={selected} tabStop={tabStop} />
        {canCheck ? <CheckBox item={item} checked={checked} onCheck={onCheck} /> : null}
      </td>
      <td>
        <AuthorFact item={item} />
      </td>
      <td>
        <div className={styles.materialCell}>
          <MaterialRail summary={materialSummary(item)} />
          <StateLine state={detail} />
        </div>
      </td>
      <td>
        <Engagement engagement={item.display.engagement} numbersOnly />
      </td>
      <td className={styles.cell}>{publishedCopy(item.display, { compact: true, withFieldName: false })}</td>
      <td className={styles.cell} title={item.lastObservedAt ?? undefined}>
        {observed ?? '未知'}
      </td>
    </tr>
  );
}
