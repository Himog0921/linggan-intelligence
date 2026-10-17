import { FactGrid, SourceNote, StateLine } from './Primitives';
import { admissionCopy } from './evidenceFeedback';
import { LANE_LABELS, momentText } from './evidencePresent';
import type { ReobservationOperation as Operation, ReobservationTask } from './reobservationApi';
import styles from './Reobservation.module.css';

/** 任务的通道名：详情通道在通用词表里叫「详情」，采集任务里叫 content_detail。 */
const CAPABILITY_LABELS: Readonly<Record<string, string>> = { content_detail: '详情', comments: LANE_LABELS.comments ?? '评论', replies: LANE_LABELS.replies ?? '回复' };

function commentLimitText(limit: ReobservationTask['commentLimit']): string {
  if (limit === 'not_requested') return '本详情 lane 不单独请求评论';
  return limit === null ? '当前未知' : String(limit);
}

function Task({ task }: { task: ReobservationTask }) {
  const rows = [
    ['评论上限', commentLimitText(task.commentLimit)],
    ['媒体', task.acquireMedia === 'not_requested' ? '未请求' : (task.acquireMedia ?? '当前未知')],
    ['认领时间', task.claimedAt ? momentText(task.claimedAt) : '尚未认领'],
  ] as const;
  return (
    <article className={styles.task}>
      <div className={styles.taskHead}>
        <strong>{(task.capability ? CAPABILITY_LABELS[task.capability] : undefined) ?? task.capability ?? '通道当前未知'}</strong>
        <StateLine state={task.state} />
      </div>
      <FactGrid rows={rows} />
      <details className={styles.technical}>
        <summary>技术详情</summary>
        <FactGrid
          rows={[
            ['任务', task.taskId ?? '当前未知'],
            ['尝试', task.attemptId ?? '尚未开始'],
            ['包 / 回执', task.packageRef || task.receiptRef ? `${task.packageRef ?? '包未知'} / ${task.receiptRef ?? '回执未见'}` : '尚未形成'],
          ]}
        />
      </details>
    </article>
  );
}

/**
 * 本次复观测的执行状态。准入结果写中文，准入说明是服务器原文；编号类字段（决定、请求、工单、租约）
 * 下沉到「技术详情」折叠区（F06）。没有任务时分两种说法，都明说这不是平台已读取或执行成功。
 */
export function OperationBlock({ operation }: { operation: Operation }) {
  const state = operation.leaseState ?? operation.execution ?? 'NOT_STARTED';
  const queued = operation.execution === 'QUEUED';
  return (
    <div className={styles.operation}>
      <div className={styles.operationHead}>
        <strong>本次复观测执行状态</strong>
        <StateLine state={state} />
      </div>
      <FactGrid
        rows={[
          ['准入结果', admissionCopy(operation.admission), operation.admission],
          ['准入说明', operation.admissionReason ?? '状态读取未返回新的准入说明'],
          ['媒体策略', operation.mediaState === 'NOT_REQUESTED' ? '未请求新媒体，复用既有资产' : '当前未知'],
        ]}
      />
      <details className={styles.technical}>
        <summary>技术详情</summary>
        <FactGrid
          rows={[
            ['准入决定', operation.decisionRef ?? '当前未知'],
            ['请求', operation.requestRef ?? '当前未知'],
            ['工单', operation.workOrderRef ?? '尚未形成'],
            ['租约', operation.leaseRef ?? '尚未形成'],
            ['到期时间', momentText(operation.expiresAt)],
            ['媒体策略原码', operation.mediaReason ?? 'NOT REQUESTED'],
          ]}
        />
      </details>
      {operation.tasks.length === 0 ? (
        <SourceNote>
          {queued
            ? '工单已进入立即队列，尚未被合格工位认领；这不是平台已读取、执行或采集成功。'
            : '准入已形成记录，但当前没有可读取的执行 lane；这不是平台已读取或执行成功。'}
        </SourceNote>
      ) : (
        <div className={styles.tasks}>
          {operation.tasks.map((task, index) => (
            <Task key={task.taskId ?? index} task={task} />
          ))}
        </div>
      )}
    </div>
  );
}
