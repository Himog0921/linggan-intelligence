import { EvidenceLabel } from './EvidenceLabel';
import { Tech } from './Primitives';
import type { PanelContext } from './ReadingPanel';
import { commentCollectionRows } from './observationRecords';
import {
  limitationCopy,
  materialSummary,
  momentText,
  purposeLabel,
  stateMeta,
  stoppedReasonMeaning,
} from './evidencePresent';
import { mediaSlotLedger, positionText } from './mediaObjects';
import type { Receipt, WorkItem } from './evidenceTypes';
import styles from './Inspector.module.css';

const PROVENANCE_GROUPS = [
  ['来源包', 'packageRefs'],
  ['任务', 'taskRefs'],
  ['尝试', 'attemptRefs'],
  ['回执', 'receiptRefs'],
  ['目标', 'targetRefs'],
  ['工单', 'workOrderRefs'],
  ['采集端', 'producers'],
] as const;

function receiptReading(receipt: Receipt | null): string {
  if (receipt === null) return '未返回';
  const total = receipt.total === null ? '总数未知' : `总数 ${receipt.total}`;
  const returned = receipt.returned === null ? '返回未知' : `返回 ${receipt.returned}`;
  const truncated = receipt.truncated === null ? '截断未知' : receipt.truncated ? '有截断' : '未截断';
  return `${total} · ${returned} · ${truncated}`;
}

function materialReading(item: Pick<WorkItem, 'media' | 'lanes'>): string {
  const summary = materialSummary(item);
  if (summary.applicable === 0) return '本作品没有适用材料通道';
  if (summary.ready === null) return `可读情况当前未知（${summary.applicable} 项）`;
  const failed = summary.segments.filter(segment => segment.applicable && segment.fill === 'failed').map(segment => segment.label);
  const partial = summary.segments.filter(segment => segment.applicable && segment.fill === 'partial').map(segment => segment.label);
  const flags = [
    summary.hasGap ? '有缺口' : null,
    failed.length > 0 ? `失败：${failed.join('、')}` : null,
    partial.length > 0 ? `部分：${partial.join('、')}` : null,
  ].filter((flag): flag is string => flag !== null);
  return `可读 ${summary.ready}/${summary.applicable}${flags.length > 0 ? ` · ${flags.join(' · ')}` : ''}`;
}

function localMaterials(context: PanelContext): string {
  const { item, inspector, listItem } = context;
  const ledger = mediaSlotLedger(inspector, (listItem ?? item).fragment);
  if (ledger.length === 0) return '暂无本地副本；这不表示作品没有媒体';
  return ledger
    .map(object => {
      const parts = [
        `${positionText(object.index, ledger.length)} ${purposeLabel(object.purpose)}`,
        stateMeta(object.bytesState)[0],
        object.derivedState ? `文字 ${stateMeta(object.derivedState)[0]}` : '文字未处理',
      ];
      return parts.join(' · ');
    })
    .join('；');
}

/**
 * 「采集详情」（Mog 2026-10-23、2026-11-02）：默认收起的轻入口，只在排查时展开。
 * 保留真实来源、观察/采集时间、材料状态、停止原因与必要来源编号；重复的五通道、作品字段、
 * 互动时间线和整页原始转储不在这里重演。没有本地媒体只说明事实，不当作故障。
 */
export function CollectionDetails({ context }: { context: PanelContext }) {
  const { item, inspector, channels, listItem } = context;
  const current = listItem ?? item;
  const history = commentCollectionRows(inspector.commentsHistory);
  const latest = history[0] ?? null;
  const comments = inspector.commentsCoverage;
  const stopped = comments?.stoppedReason ?? null;
  const provenance = inspector.provenance;
  const provenanceGroups = provenance
    ? PROVENANCE_GROUPS.filter(([, key]) => provenance[key].length > 0)
    : [];
  const provenanceReceipt = channels.provenanceReceipt ?? provenance?.receipt ?? null;
  const mediaReceipt = channels.mediaReceipt ?? inspector.mediaSlotsReceipt;
  const limitations = inspector.limitations;
  return (
    <details className={styles.collectionDetails}>
      <summary><EvidenceLabel>采集详情</EvidenceLabel></summary>
      <dl className={styles.detailFacts}>
        <dt>原文</dt>
        <dd>
          {inspector.sourceNoteUrl ? (
            <a href={inspector.sourceNoteUrl} target="_blank" rel="noopener noreferrer">
              打开原文
            </a>
          ) : '本次详情未提供原文链接'}
        </dd>
        <dt>最近观察</dt>
        <dd>{momentText(current.lastObservedAt, '暂无观察记录')}</dd>
        <dt>最近评论采集</dt>
        <dd>{momentText(latest?.at, '暂无采集记录')}</dd>
        <dt>停止原因</dt>
        <dd>{stopped ? <>{stoppedReasonMeaning(stopped)} <Tech>{stopped}</Tech></> : '本次采集未记录停止原因'}</dd>
        <dt>材料状态</dt>
        <dd>{materialReading(current)}</dd>
        <dt>本地材料</dt>
        <dd>{localMaterials(context)}</dd>
        <dt>限制</dt>
        <dd>
          {limitations.length > 0 ? (
            <>
              {limitations.map(limitationCopy).join('；')} <Tech>{limitations.join(' ')}</Tech>
            </>
          ) : '本次读取未返回限制列表'}
        </dd>
        <dt>回执</dt>
        <dd>{`来源 ${receiptReading(provenanceReceipt)}；媒体 ${receiptReading(mediaReceipt)}`}</dd>
        <dt>来源编号</dt>
        <dd className={styles.detailRefs}>
          <span>作品 <Tech>{item.ref}</Tech></span>
          {provenance === null ? (
            <span>本次详情未返回来源血缘</span>
          ) : provenanceGroups.length === 0 ? (
            <span>来源血缘不完整</span>
          ) : (
            provenanceGroups.map(([label, key]) => (
              <span key={key}>{label} {provenance[key].map(value => <Tech key={value}>{value}</Tech>)}</span>
            ))
          )}
        </dd>
      </dl>
    </details>
  );
}
