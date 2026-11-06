import { useState } from 'react';
import { formatNumber, momentText } from './evidencePresent';
import type { Inspector, WorkItem } from './evidenceTypes';
import {
  OBSERVED_METRICS,
  TIMELINE_WINDOW,
  deltaCopy,
  elapsedCopy,
  observationLaneCopy,
  observationRows,
  type MetricCell,
} from './observationRecords';
import { EmptyState } from '../../../shared/EmptyState';
import styles from './Records.module.css';

function Cell({ cell, label }: { cell: MetricCell; label: string }) {
  const delta = deltaCopy(cell.delta);
  return (
    <td data-label={label}>
      <span className={styles.value}>{cell.value === null ? '—' : formatNumber(cell.value)}</span>
      {delta ? (
        <span className={styles.delta} data-trend={cell.delta === 0 ? 'flat' : cell.delta! > 0 ? 'up' : 'down'}>
          {delta}
        </span>
      ) : null}
    </td>
  );
}

/**
 * 「观察记录」的正文（Mog 2026-10-23、2026-11-02）：最近一次观察时间用轻元数据，下面是每次读到的
 * 点赞／评论／收藏／分享以及和上一次相比的变化。没读到的数字写「—」，不当 0；同一来源连续读数相同合成一行。
 * 标准宽度用两层记录（时间／来源在上，四项数字在下），加宽后用紧凑表格；两种排布读同一份事实。
 */
export function ObservationHistory({ item, inspector, now: fixedNow }: { item: WorkItem; inspector: Inspector; now?: number }) {
  // 「距今几天」以打开这篇作品的时刻为准，重新渲染不会让它跳动；测试可以传入固定时刻。
  const [openedAt] = useState(() => fixedNow ?? Date.now());
  const now = fixedNow ?? openedAt;
  const rows = observationRows(inspector.timeline);
  const total = inspector.timeline.length;
  const latest = inspector.timeline.at(-1)?.observedAt ?? item.lastObservedAt;
  const elapsed = elapsedCopy(latest, now);
  return (
    <div className={styles.records} data-observation-records>
      <p className={styles.recent}>
        <span className={styles.recentLabel}>最近</span>
        <span className={styles.separator} aria-hidden="true">·</span>
        <time className={styles.recentTime} dateTime={latest ?? undefined}>{momentText(latest, '时间未知')}</time>
        {elapsed ? (
          <>
            <span className={styles.separator} aria-hidden="true">·</span>
            <span>{elapsed}</span>
          </>
        ) : null}
      </p>
      {rows.length > 0 ? (
        <div className={styles.tableWrap}>
          <table className={styles.observations}>
            <caption className="v7-sr-only">每次观察读到的数字，最近一次在最上面</caption>
            <thead>
              <tr>
                <th scope="col">时间 · 来源</th>
                {OBSERVED_METRICS.map(([key, label]) => (
                  <th key={key} scope="col">
                    {label}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {rows.map((row, index) => (
                <tr key={`${row.last ?? 'unknown'}:${index}`}>
                  <th scope="row">
                    <span className={styles.when}>{momentText(row.last, '时间未知')}</span>
                    <span className={styles.how}>{observationLaneCopy(row.lane)}</span>
                    {row.count > 1 ? <span className={styles.how}>{`连续 ${row.count} 次相同 · 最早 ${momentText(row.first, '时间未知')}`}</span> : null}
                  </th>
                  {OBSERVED_METRICS.map(([key, label]) => (
                    <Cell key={key} cell={row.cells[key]} label={label} />
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <EmptyState kind="first_use" title="暂无互动观察记录" />
      )}
      {total >= TIMELINE_WINDOW ? <p className={styles.windowNote}>{`至少 ${TIMELINE_WINDOW} 次，仅列最近 ${TIMELINE_WINDOW} 次。`}</p> : null}
    </div>
  );
}
