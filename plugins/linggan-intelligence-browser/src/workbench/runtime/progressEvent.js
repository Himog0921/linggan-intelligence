import { REMOTE_EXECUTION_STAGE, REMOTE_EXECUTION_STATUS } from '../protocol/schema.js';
import { normalizeRuntimeObservability } from './taskRuntimeObservability.js';

function normalizeText(value = '') {
  return String(value || '').trim();
}

function normalizeCount(value, fallback = 0) {
  const num = Number(value);
  if (!Number.isFinite(num) || num < 0) return fallback;
  return Math.floor(num);
}

function normalizeMetrics(metrics = {}) {
  if (!metrics || typeof metrics !== 'object' || Array.isArray(metrics)) return {};
  return Object.fromEntries(
    Object.entries(metrics).map(([key, value]) => [key, normalizeCount(value, 0)]),
  );
}

function inferStageFromText(text = '') {
  if (/下载|打包/.test(text)) return REMOTE_EXECUTION_STAGE.DOWNLOADING;
  if (/写入|落库|保存/.test(text)) return REMOTE_EXECUTION_STAGE.PERSISTING;
  if (/扫描|发现|滚动|定位/.test(text)) return REMOTE_EXECUTION_STAGE.DISCOVERING;
  if (/准备|启动/.test(text)) return REMOTE_EXECUTION_STAGE.CONTEXT_CHECK;
  if (/完成|收尾/.test(text)) return REMOTE_EXECUTION_STAGE.FINALIZING;
  return REMOTE_EXECUTION_STAGE.COLLECTING;
}

function inferStatus(taskState = '', text = '') {
  const normalizedState = normalizeText(taskState);
  if (normalizedState) return normalizedState;
  if (/暂停/.test(text)) return REMOTE_EXECUTION_STATUS.PAUSED;
  if (/停止中/.test(text)) return REMOTE_EXECUTION_STATUS.STOPPING;
  if (/完成/.test(text)) return REMOTE_EXECUTION_STATUS.DONE;
  if (/失败|报错|错误/.test(text)) return REMOTE_EXECUTION_STATUS.FAILED;
  return REMOTE_EXECUTION_STATUS.RUNNING;
}

function deriveLegacyStatusText(event = {}) {
  const message = normalizeText(event.message);
  if (message) return message;

  switch (event.status) {
    case REMOTE_EXECUTION_STATUS.PAUSED:
      return '任务已暂停';
    case REMOTE_EXECUTION_STATUS.STOPPING:
      return '任务停止中';
    case REMOTE_EXECUTION_STATUS.DONE:
      return '任务已完成';
    case REMOTE_EXECUTION_STATUS.FAILED:
      return event.error?.message || '任务执行失败';
    default:
      return '任务执行中';
  }
}

export function normalizeProgressEvent(input = {}) {
  const statusText = normalizeText(input.message || input.status);
  const status = inferStatus(input.taskState, statusText);
  const stage = normalizeText(input.stage || input.phase) || inferStageFromText(statusText);

  return {
    status,
    stage,
    current: normalizeCount(input.current, 0),
    total: normalizeCount(input.total, 0),
    message: statusText,
    taskType: normalizeText(input.taskType),
    metrics: normalizeMetrics(input.metrics),
    observability: normalizeRuntimeObservability(input.observability),
    heartbeatAt: normalizeCount(input.heartbeatAt, Date.now()),
    error: input.error || null,
  };
}

export function toLegacyProgressMessage(event = {}) {
  const normalizedEvent = normalizeProgressEvent(event);
  return {
    current: normalizedEvent.current,
    total: normalizedEvent.total,
    status: deriveLegacyStatusText(normalizedEvent),
    taskState: normalizedEvent.status,
    phase: normalizedEvent.stage,
    stage: normalizedEvent.stage,
    message: normalizedEvent.message,
    metrics: normalizedEvent.metrics,
    observability: normalizedEvent.observability,
    heartbeatAt: normalizedEvent.heartbeatAt,
    error: normalizedEvent.error,
    taskType: normalizedEvent.taskType,
  };
}
