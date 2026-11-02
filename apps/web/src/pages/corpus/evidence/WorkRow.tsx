import type { MouseEvent } from 'react';
import { CoverPreview } from './Media';
import { StateLine } from './Primitives';
import { compactMoment, laneSummary, materialSummary, metricReading, publishedCopy, stateMeta, workTitle, ENGAGEMENT_KINDS } from './evidencePresent';
import type { WorkItem } from './evidenceTypes';
import { AuthorFact, Engagement, MaterialRail, QuoteBlock } from './WorkParts';
import { MaterialTiles } from './MaterialTiles';
import { useTitleTooltip } from './TitleTooltip';
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

/** 研读信息列只有 108–126px 宽，少数状态原句较长，关键结论会被省略号截掉。
 *  这里按状态码给研读列专用的短名；完整原句仍留在 title 与读屏文本里，其他页面的词表不变。 */
const RESEARCH_STATE_LABELS: Readonly<Record<string, string>> = {
  DETAIL_READ_BLOCKED: '详情未读成',
  PAGE_UNAVAILABLE: '平台页不存在',
  INPUT_BLOCKED: '输入不可执行',
  EXPIRED_WITHOUT_RECEIPT: '租约结束无回执',
  COMPLETED_WITHOUT_RECEIPT: '任务结束无回执',
};

/** The 36px hit area floats over the object; it never participates in the reading-row height. */
export function CheckBox({ item, checked, onCheck }: { item: WorkItem; checked: boolean; onCheck: (ref: string, checked: boolean) => void }) {
  return <label className={styles.check} data-evidence-check="" onClick={stop}>
    <input type="checkbox" checked={checked} aria-label={`勾选作品：${workTitle(item)}`}
      onChange={event => onCheck(item.ref, event.currentTarget.checked)} />
  </label>;
}

function SelectTitle({ item, selected, tabStop }: Pick<RowProps, 'item' | 'selected' | 'tabStop'>) {
  const title = workTitle(item);
  const { anchorRef, onEnter, onLeave, onFocus, onBlur, onPointerDown, tooltip } = useTitleTooltip<HTMLButtonElement>(title, 'width');
  return <>
    <button ref={anchorRef} type="button" className={styles.titleButton} data-select-target=""
      aria-current={selected ? 'true' : undefined} tabIndex={tabStop ? 0 : -1}
      onMouseEnter={onEnter} onMouseLeave={onLeave} onFocus={onFocus} onBlur={onBlur} onPointerDown={onPointerDown}>{title}</button>
    {tooltip}
  </>;
}

/** V2.2: 78×104 cover, 106px row including its edges. Title / quote / author / metrics are four fixed tracks.
 *  右侧是一列安静的事实：材料现在怎样、何时发布、何时观察；细圈线只强调状态词。 */
export function ResearchRow({ item, selected, tabStop, canCheck, checked, onSelect, onCheck }: RowProps) {
  const detail = laneSummary(item.lanes, 'detail')?.state ?? 'UNKNOWN';
  const [stateLabel] = stateMeta(detail);
  const shortState = RESEARCH_STATE_LABELS[detail] ?? stateLabel;
  const observed = compactMoment(item.lastObservedAt);
  // <time datetime> 要 ISO 形式；库里的观察时刻是「YYYY-MM-DD HH:MM」，这里只换分隔符，不改显示。
  const observedIso = item.lastObservedAt ? item.lastObservedAt.replace(' ', 'T') : undefined;
  const published = publishedCopy(item.display, { compact: true, withFieldName: false });
  // 来源时间在字段名里已经说过一次，值里只留时间本身。
  const fromSource = published.startsWith('来源时间：');
  return <li className={styles.row} data-selected={selected} data-checked={canCheck && checked}
    data-selecting={canCheck} data-work-ref={item.ref} onClick={() => onSelect(item)}>
    <article className={styles.article}>
      <CoverPreview title={workTitle(item)} url={item.media.coverUrl} state={item.media.coverState} />
      <div className={styles.identity}>
        <h2 className={styles.title}><SelectTitle item={item} selected={selected} tabStop={tabStop} /></h2>
        <QuoteBlock fragment={item.fragment} />
        <div className={styles.meta}><AuthorFact item={item} />
          {/* 窄屏整列隐藏时，发布时间回到作者行，不随侧栏一起消失。 */}
          <span className={styles.metaDate} data-meta-date="">{publishedCopy(item.display, { compact: true })}</span>
        </div>
        <Engagement engagement={item.display.engagement} />
      </div>
      <div className={styles.side}>
        <dl className={styles.info}>
          <div className={styles.infoRow}>
            <dt>状态</dt>
            <dd className={styles.stateCell} title={stateLabel}>
              <span className={styles.annotatedState}>
                <svg viewBox="0 0 100 28" preserveAspectRatio="none" aria-hidden="true" focusable="false"><path d="M12 3C4 5 1 17 8 22c13 6 70 5 84-1 9-5 5-17-7-18C65 0 30 1 17 4" /></svg>
                <StateLine state={detail} label={shortState} ariaHidden />
              </span>
              <span className="v7-sr-only">{stateLabel}</span>
            </dd>
          </div>
          <div className={styles.infoRow}>
            <dt>材料</dt>
            <dd className={styles.materialsCell}><MaterialTiles summary={materialSummary(item)} /></dd>
          </div>
          <div className={styles.infoRow}>
            <dt>{fromSource ? '来源' : '发布'}</dt>
            <dd title={published}>{fromSource ? published.slice('来源时间：'.length) : published}</dd>
          </div>
          <div className={styles.infoRow}>
            <dt>观察</dt>
            <dd><time dateTime={observedIso} title={item.lastObservedAt ? `最近观察 ${item.lastObservedAt}` : undefined}>
              {observed ?? '未知'}
            </time></dd>
          </div>
        </dl>
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
