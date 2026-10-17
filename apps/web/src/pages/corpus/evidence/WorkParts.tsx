import { Fragment } from 'react';
import { AuthorAvatar } from './Media';
import {
  ENGAGEMENT_KINDS,
  effectiveAuthor,
  fragmentSourceLabel,
  metricReading,
  quoteParts,
  segmentReading,
  segmentTitle,
  type MaterialSummary,
} from './evidencePresent';
import type { Engagement as EngagementFacts, Fragment as QuoteFragment, WorkItem } from './evidenceTypes';
import styles from './WorkParts.module.css';

/** 原声块：行内与检查器同一块文字——命中按字符偏移高亮；没有原声写明确的空态，不拿标题或摘要顶替。 */
export function QuoteBlock({ fragment, wrap = false }: { fragment: QuoteFragment | null; wrap?: boolean }) {
  if (fragment === null) {
    return (
      <div className={styles.quote} data-absent="true">
        <span className={styles.quoteSource}>无原声</span>
        <span>尚未取得可引用的原文、评论或图片文字</span>
      </div>
    );
  }
  const parts = quoteParts(fragment);
  return (
    <div className={styles.quote} data-wrap={wrap}>
      <span className={styles.quoteSource}>{fragmentSourceLabel(fragment.sourceKind)}</span>
      <q>
        {parts === null ? (
          fragment.text
        ) : (
          <Fragment>
            {parts.before}
            <mark>{parts.match}</mark>
            {parts.after}
          </Fragment>
        )}
      </q>
    </div>
  );
}

/* LIDS 图标：24 栅格、实心、currentColor。字符图形各平台渲染不同、无法控制粗细，所以不用；
 * 每个图标都有中文名，旁边的数字单独读没有意义。 */
const ICON_PATHS: Record<(typeof ENGAGEMENT_KINDS)[number][0], string> = {
  like: 'M12 20.5C12 20.5 3.5 15.4 3.5 9.9A4.9 4.9 0 0 1 12 6.6a4.9 4.9 0 0 1 8.5 3.3c0 5.5-8.5 10.6-8.5 10.6z',
  comment: 'M4 3.5h16a1.5 1.5 0 0 1 1.5 1.5v9a1.5 1.5 0 0 1-1.5 1.5H9.2L4 20.2V3.5z',
  collect: 'M6.5 2.5h11a1 1 0 0 1 1 1v18l-6.5-4.8-6.5 4.8v-18a1 1 0 0 1 1-1z',
  share: 'M21.6 2.6 2.9 9.9a.6.6 0 0 0 .05 1.13l4.6 1.5 1.6 4.9a.6.6 0 0 0 1.06.18l2.2-2.9 4.4 3.2a.6.6 0 0 0 .94-.33l4-14.2a.6.6 0 0 0-.15-.78z',
};

export function MetricIcon({ kind, label }: { kind: keyof typeof ICON_PATHS; label: string }) {
  return (
    <svg viewBox="0 0 24 24" width="16" height="16" fill="currentColor" role="img" aria-label={label}>
      <path d={ICON_PATHS[kind]} />
    </svg>
  );
}

/** 互动行。表格里列头已有图标，行内只留数字（读屏靠隐藏文字读出名称）。 */
export function Engagement({ engagement, numbersOnly = false }: { engagement: EngagementFacts; numbersOnly?: boolean }) {
  return (
    <div className={styles.engagement}>
      {ENGAGEMENT_KINDS.map(([kind, label]) => (
        <span key={kind}>
          {numbersOnly ? <span className="v7-sr-only">{label}</span> : <MetricIcon kind={kind} label={label} />}
          <b>{metricReading(engagement[kind])}</b>
        </span>
      ))}
    </div>
  );
}

/** 材料完整度导轨。颜色是状态唯一编码，所以读屏拿到的是逐维度的文字说明。 */
export function MaterialRail({ summary }: { summary: MaterialSummary }) {
  const label = `材料完整度：${summary.segments.map(segment => `${segment.label} ${segmentReading(segment)}`).join('；')}`;
  return (
    <div className={styles.rail} role="img" aria-label={label}>
      {summary.segments.map(segment => (
        <i key={segment.key} className={styles.segment} data-fill={segment.fill} title={segmentTitle(segment)} />
      ))}
    </div>
  );
}

/** 检查器里的图例：把每个维度的名字和它的填充状态对应起来，颜色只作补充。 */
export function MaterialLegend({ summary }: { summary: MaterialSummary }) {
  return (
    <ul className={styles.legend} aria-hidden="true">
      {summary.segments.map(segment => (
        <li key={segment.key} data-fill={segment.fill} title={segmentTitle(segment)}>
          {segment.label}
        </li>
      ))}
    </ul>
  );
}

export function AuthorFact({ item }: { item: Pick<WorkItem, 'display' | 'context' | 'media'> }) {
  const name = effectiveAuthor(item);
  return (
    <span className={styles.authorFact}>
      <AuthorAvatar author={name} url={item.media.avatarUrl} state={item.media.avatarState} />
      <strong>{name}</strong>
    </span>
  );
}
