import type { ReactNode } from 'react';
import { FeedbackLine, StateLine } from './Primitives';
import { stateMeta } from './evidencePresent';
import { isTerminalTask, type ReobservationOperation } from './reobservationApi';
import type { Reobservation } from './useReobservation';
import styles from './Records.module.css';

interface ReobservationStatusProps {
  domain: string;
  state: Reobservation;
  /** 重新读取作品详情；来源轨迹里出现工单引用即说明已入队。 */
  onRecheck: () => void;
}

const LIVE_TASK_STATES = new Set(['RUNNING', 'ACTIVE', 'CLAIMED', 'LEASED']);
const QUEUED_TASK_STATES = new Set(['QUEUED', 'NOT_STARTED', 'NOT_REQUESTED']);
const PROBLEM_TASK_STATES = new Set(['FAILED', 'RISK_CONTROL', 'PAGE_UNAVAILABLE', 'DETAIL_READ_BLOCKED', 'INPUT_BLOCKED']);

/** 把一次复观测的已知事实收成一条短状态；只沿用已有状态短语，不额外推断完成。 */
function operationReading(operation: ReobservationOperation): { state: string; label: string } | null {
  if (operation.admission === 'REUSE') return null;
  const tasks = operation.tasks;
  if (tasks.length > 0) {
    if (tasks.some(task => LIVE_TASK_STATES.has(task.state))) return { state: 'RUNNING', label: '正在采集' };
    if (tasks.some(task => QUEUED_TASK_STATES.has(task.state))) return { state: 'QUEUED', label: '等待执行' };
    const problem = tasks.find(task => PROBLEM_TASK_STATES.has(task.state));
    if (problem) return { state: problem.state, label: stateMeta(problem.state)[0] };
    if (tasks.every(task => task.state === 'ACCEPTED')) return { state: 'ACCEPTED', label: stateMeta('ACCEPTED')[0] };
    const last = tasks.find(task => !isTerminalTask(task)) ?? tasks[0];
    return last ? { state: last.state, label: stateMeta(last.state)[0] } : null;
  }
  const state = operation.leaseState ?? operation.execution;
  if (state === null) return null;
  if (LIVE_TASK_STATES.has(state)) return { state: 'RUNNING', label: '正在采集' };
  if (QUEUED_TASK_STATES.has(state)) return { state: 'QUEUED', label: '等待执行' };
  return { state, label: stateMeta(state)[0] };
}

function LightLine({ state, label, children }: { state: string; label: string; children?: ReactNode }) {
  return (
    <div className={styles.statusLine} role="status" aria-live="polite">
      <StateLine state={state} label={label} />
      {children ? <>
        <span aria-hidden="true">·</span>
        {children}
      </> : null}
    </div>
  );
}

/**
 * 复观测的持续状态与就地入口：提交回执进轻通知，成功不在这里重复；错误与未确认留在动作旁，
 * 保留核对和查看采集任务入口，不渲染默认技术表格。未确认不会因为通知消失而解除。
 */
export function ReobservationStatus({ domain, state, onRecheck }: ReobservationStatusProps) {
  const tasksUrl = `/collection/tasks?domain=${encodeURIComponent(domain)}`;
  const feedback = state.feedback;
  const unknown = feedback?.tone === 'unknown';
  const failed = feedback !== null && (feedback.tone === 'error' || feedback.tone === 'refused');
  const reading = state.operation && !failed ? operationReading(state.operation) : null;
  if (!feedback && !state.readError && !state.pollError && !state.operation) return null;
  return (
    <>
      {unknown ? (
        <LightLine state="UNKNOWN" label="提交结果未确认">
          <button type="button" className={styles.statusAction} onClick={onRecheck}>重新核对</button>
          <a className={styles.statusAction} href={tasksUrl}>查看采集任务</a>
        </LightLine>
      ) : null}
      {failed ? (
        <FeedbackLine feedback={feedback}>
          <button type="button" className={styles.statusAction} onClick={onRecheck}>重新核对</button>
          <a className={styles.statusAction} href={tasksUrl}>查看采集任务</a>
        </FeedbackLine>
      ) : null}
      {state.pollError ? (
        <LightLine state="UNKNOWN" label="状态读取失败">
          <button type="button" className={styles.statusAction} onClick={state.retryStatus}>重试</button>
        </LightLine>
      ) : null}
      {state.readError ? (
        <LightLine state="UNKNOWN" label="复观测状态已更新，作品事实未重新读取">
          <button type="button" className={styles.statusAction} onClick={onRecheck}>重新读取</button>
        </LightLine>
      ) : null}
      {reading ? (
        <LightLine state={reading.state} label={state.pollError ? `上次状态：${reading.label}` : reading.label}>
          <a className={styles.statusAction} href={tasksUrl}>查看采集任务</a>
        </LightLine>
      ) : null}
    </>
  );
}
