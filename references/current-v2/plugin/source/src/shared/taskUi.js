import { TASK_STATE } from './constants.js';

const TASK_STATE_VALUES = new Set(Object.values(TASK_STATE));

export const DEFAULT_TOKENS = {
  ink: '#121212',
  surface: '#ffdd57',
  card: '#fffdf7',
  line: '#121212',
  muted: '#4f4f4f',
  running: '#d8f4ff',
  paused: '#fff0a8',
  stopping: '#ffd2a8',
  done: '#d4f5d3',
  error: '#ffd4d4',
  neutral: '#fff6cf',
};

export function normalizeTaskState(taskState, fallback = TASK_STATE.RUNNING) {
  const normalizedState = String(taskState || '').trim();
  return TASK_STATE_VALUES.has(normalizedState) ? normalizedState : fallback;
}

export function resolveTaskState({
  taskState = '',
  status = '',
  fallback = TASK_STATE.RUNNING,
} = {}) {
  const normalizedTaskState = normalizeTaskState(taskState, '');
  if (normalizedTaskState) return normalizedTaskState;
  return normalizeTaskState(status, fallback);
}

export function isPausedTaskState(taskState) {
  return normalizeTaskState(taskState, '') === TASK_STATE.PAUSED;
}

export function isTerminalTaskState(taskState) {
  const normalizedState = normalizeTaskState(taskState, '');
  return normalizedState === TASK_STATE.DONE
    || normalizedState === TASK_STATE.ERROR
    || normalizedState === TASK_STATE.IDLE;
}

export function describeTaskProgress({ current = 0, total = 0 } = {}) {
  const normalizedCurrent = Number.isFinite(Number(current)) ? Math.max(0, Math.floor(Number(current))) : 0;
  const normalizedTotal = Number.isFinite(Number(total)) ? Math.max(0, Math.floor(Number(total))) : 0;

  if (normalizedTotal > 0) {
    return `${normalizedCurrent}/${normalizedTotal}`;
  }
  if (normalizedCurrent > 0) {
    return `已处理 ${normalizedCurrent}`;
  }
  return '进行中';
}

function hasProgressSummaryText(text = '') {
  return /已处理\s*\d+|\d+\s*\/\s*\d+|成功\s*\d+\s*\/\s*\d+|评论\s*\d+\s*条/.test(String(text || ''));
}

export function inferTaskStage({ taskState = '', message = '', current = 0, total = 0 } = {}) {
  const state = normalizeTaskState(taskState);
  const text = String(message || '').trim();

  if (state === TASK_STATE.PAUSED) {
    const progressHint = current > 0 ? `（${describeTaskProgress({ current, total })}）` : '';
    return { label: '已暂停', tone: 'paused', helper: `任务已暂停${progressHint}，可继续或停止。` };
  }
  if (state === TASK_STATE.IDLE) {
    const progressHint = current > 0 ? `（${describeTaskProgress({ current, total })}）` : '';
    return { label: '已停止', tone: 'paused', helper: `任务已停止${progressHint}，可查看当前结果。` };
  }
  if (state === TASK_STATE.STOPPING) {
    return { label: '停止中', tone: 'stopping', helper: '正在完成当前步骤并安全收尾。' };
  }
  if (state === TASK_STATE.DONE) {
    return { label: '已完成', tone: 'done', helper: '任务已经完成，可以查看结果。' };
  }
  if (state === TASK_STATE.ERROR) {
    return { label: '失败', tone: 'error', helper: '任务执行失败，请查看提示后重试。' };
  }
  if (/采集|处理|展开|拉取|写入/.test(text)) {
    return { label: '进行中', tone: 'running', helper: '任务正在执行当前步骤。' };
  }
  if (/打包|下载/.test(text)) {
    return { label: '下载中', tone: 'running', helper: '正在下载或打包本轮结果。' };
  }
  if (/扫描|发现|滚动|定位/.test(text)) {
    return { label: '扫描中', tone: 'running', helper: '正在扫描目标内容，请稍候。' };
  }
  if (/启动|准备/.test(text) || (!text && current === 0 && total > 0)) {
    return { label: '准备中', tone: 'neutral', helper: '任务已发起，正在建立执行上下文。' };
  }
  return { label: '进行中', tone: 'running', helper: '任务正在执行中。' };
}

export function describeTaskDetail({ taskState = '', message = '', current = 0, total = 0 } = {}) {
  const state = normalizeTaskState(taskState);
  const text = String(message || '').trim();
  const progress = current > 0 ? describeTaskProgress({ current, total }) : '';
  const stage = inferTaskStage({ taskState: state, message: text, current, total });

  if ((state === TASK_STATE.PAUSED || state === TASK_STATE.IDLE) && text) {
    if (!progress || hasProgressSummaryText(text)) return text;
    return `${text}（${progress}）`;
  }

  if (text) return text;
  return stage.helper;
}

export function shouldDelayTaskbarHide({ taskState = '', badgeText = '' } = {}) {
  if (isTerminalTaskState(taskState)) return true;
  return /已完成|已停止|失败/.test(String(badgeText || ''));
}

export function getBadgeColor(tone = 'neutral', theme = 'default') {
  if (theme === 'ac-ui') {
    switch (tone) {
      case 'running': return '#C0E8F9';
      case 'paused': return '#F6E27F';
      case 'stopping': return '#F6E27F';
      case 'done': return '#ABEDC6';
      case 'error': return '#FFBCB5';
      default: return '#f2f2f2';
    }
  }
  switch (tone) {
    case 'running': return DEFAULT_TOKENS.running;
    case 'paused': return DEFAULT_TOKENS.paused;
    case 'stopping': return DEFAULT_TOKENS.stopping;
    case 'done': return DEFAULT_TOKENS.done;
    case 'error': return DEFAULT_TOKENS.error;
    default: return DEFAULT_TOKENS.neutral;
  }
}

