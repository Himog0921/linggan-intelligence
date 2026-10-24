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
import { Section, SourceNote } from './Primitives';
import styles from './Records.module.css';

function Cell({ cell }: { cell: MetricCell }) {
  const delta = deltaCopy(cell.delta);
  return (
    <td>
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
 * 「观察记录」（Mog 2026-10-23）：这篇笔记被系统看到过几次、最近一次是多久以前，每次读到的点赞／评论／收藏／分享
 * 以及和上一次相比的变化。没读到的数字写「—」，不当 0；同一来源连续读数相同合成一行。
 */
export function ObservationHistory({ item, inspector, now: fixedNow }: { item: WorkItem; inspector: Inspector; now?: number }) {
  // 「距今几天」以打开这篇作品的时刻为准，重新渲染不会让它跳动；测试可以传入固定时刻。
  const [openedAt] = useState(() => fixedNow ?? Date.now());
  const now = fixedNow ?? openedAt;
  const rows = observationRows(inspector.timeline);
  const total = inspector.timeline.length;
  const latest = inspector.timeline.at(-1)?.observedAt ?? item.lastObservedAt;
  const elapsed = elapsedCopy(latest, now);
  const latestCopy = latest ? `${momentText(latest)}${elapsed ? `（${elapsed}）` : ''}` : '时间未知';
  return (
    <Section title="观察记录">
      <p className={styles.lead}>
        {total > 0 ? `这篇笔记被观察过 ${total >= TIMELINE_WINDOW ? `至少 ${total}` : total} 次，最近一次 ${latestCopy}。` : `还没有读到点赞、评论等数字。最近一次观察 ${latestCopy}。`}
      </p>
      {rows.length > 0 ? (
        <div className={styles.tableWrap}>
          <table className={styles.observations}>
            <caption className="v7-sr-only">每次观察读到的数字，最近一次在最上面</caption>
            <thead>
              <tr>
                <th scope="col">时间 · 怎么看到的</th>
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
                    {row.count > 1 ? <span className={styles.how}>{`连续 ${row.count} 次相同，最早 ${momentText(row.first, '时间未知')}`}</span> : null}
                  </th>
                  {OBSERVED_METRICS.map(([key]) => (
                    <Cell key={key} cell={row.cells[key]} />
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : null}
      {total >= TIMELINE_WINDOW ? <SourceNote>{`这里只列最近 ${TIMELINE_WINDOW} 次观察。`}</SourceNote> : null}
    </Section>
  );
}
