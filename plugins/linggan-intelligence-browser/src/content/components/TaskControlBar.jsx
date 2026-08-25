import React from 'react';
import { createRoot } from 'react-dom/client';
import {
  inferTaskStage,
  getBadgeColor,
  DEFAULT_TOKENS,
  describeTaskProgress,
  describeTaskDetail,
  normalizeTaskState,
  isPausedTaskState,
  isTerminalTaskState,
} from '../../shared/taskUi.js';
import { TASK_STATE } from '../../shared/constants.js';

function getShellStyle(theme = 'default') {
  if (theme === 'ac-ui') {
    return {
      position: 'fixed', right: '20px', bottom: '24px', zIndex: '2147483646',
      width: '320px', minHeight: '182px', padding: '12px', display: 'block',
      boxSizing: 'border-box', background: '#fff', border: '1px solid #ddd',
      borderRadius: '14px', boxShadow: '0 4px 20px rgba(0,0,0,0.08)', color: '#121212',
      fontFamily: "'Segoe UI','PingFang SC','Microsoft YaHei',sans-serif",
    };
  }
  return {
    position: 'fixed', right: '20px', bottom: '24px', zIndex: '2147483646',
    width: '320px', minHeight: '182px', padding: '12px', display: 'block',
    boxSizing: 'border-box', background: DEFAULT_TOKENS.surface,
    border: `3px solid ${DEFAULT_TOKENS.line}`, borderRadius: '14px',
    boxShadow: `6px 6px 0 ${DEFAULT_TOKENS.line}`, color: DEFAULT_TOKENS.ink,
    fontFamily: "'Arial Black','Segoe UI',sans-serif",
  };
}

export default function TaskControlBar({
  title = '任务控制台',
  taskType = '',
  taskState = TASK_STATE.RUNNING,
  current = 0,
  total = 0,
  message = '',
  theme = 'default',
  platform = 'xhs',
}) {
  const normalizedTaskState = normalizeTaskState(taskState);
  const stage = inferTaskStage({ taskState: normalizedTaskState, message, current, total });
  const isAc = theme === 'ac-ui';
  const lineColor = isAc ? '#ddd' : DEFAULT_TOKENS.line;
  const neutralBg = isAc ? '#f2f2f2' : DEFAULT_TOKENS.neutral;
  const muted = isAc ? '#888' : DEFAULT_TOKENS.muted;
  const fontWeight = isAc ? '700' : '900';
  const btnRadius = isAc ? '5px' : '8px';
  const btnBorder = isAc ? `1px solid ${lineColor}` : `2px solid ${lineColor}`;
  const btnShadow = isAc ? '0 2px 4px rgba(0,0,0,0.06)' : `2px 2px 0 ${lineColor}`;
  const btnHoverShadow = isAc ? '0 4px 8px rgba(0,0,0,0.08)' : `3px 3px 0 ${lineColor}`;
  const progressTrackBg = isAc ? '#e8e8e8' : '#fff';
  const progressTrackBorder = isAc ? '1px solid #ddd' : `2px solid ${lineColor}`;
  const progressFillBg = isAc ? 'linear-gradient(90deg,#66C3FF 0%,#ABEDC6 50%,#F5CB5C 100%)' : 'linear-gradient(90deg,#3bb8d8 0%,#7dd87a 50%,#ffdd57 100%)';

  const pct = total > 0 ? Math.max(0, Math.min(100, Math.round((current / total) * 100))) : 0;
  const isStopping = normalizedTaskState === TASK_STATE.STOPPING;
  const isPaused = isPausedTaskState(normalizedTaskState);
  const isDone = isTerminalTaskState(normalizedTaskState);
  const isStopDisabled = isStopping || isDone;

  const isDy = platform === 'douyin';
  const pauseAction = isDy ? 'dy_pauseBatch' : 'pauseBatch';
  const resumeAction = isDy ? 'dy_resumeBatch' : 'resumeBatch';
  const stopAction = isDy ? 'dy_stopBatch' : 'stopBatch';

  const pauseClass = isDy ? 'lgboom-dy-task-btn' : 'lgboom-task-btn lgboom-task-btn-pause lgboom-btn';
  const resumeClass = isDy ? 'lgboom-dy-task-btn' : 'lgboom-task-btn lgboom-task-btn-resume lgboom-btn';
  const stopClass = isDy ? 'lgboom-dy-task-btn' : 'lgboom-task-btn lgboom-task-btn-stop lgboom-btn';

  const btnBase = {
    flex: 1,
    height: '44px',
    border: btnBorder,
    padding: '8px 10px',
    borderRadius: btnRadius,
    fontWeight,
    cursor: 'pointer',
    boxShadow: btnShadow,
    transition: 'transform 0.12s ease,box-shadow 0.12s ease,background 0.12s ease',
    fontFamily: isAc ? "'Segoe UI','PingFang SC','Microsoft YaHei',sans-serif" : "'Arial Black','Segoe UI',sans-serif",
  };

  return (
    <div style={getShellStyle(theme)}>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '10px', marginBottom: '8px' }}>
        <div style={{ fontSize: '13px', fontWeight }}>{title}</div>
        <span style={{
          display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
          minWidth: '64px', height: '28px', padding: '0 10px', borderRadius: '999px',
          border: btnBorder, background: getBadgeColor(stage.tone, theme),
          boxShadow: isAc ? 'none' : '1px 1px 0 ' + lineColor,
          fontSize: '11px', fontWeight, lineHeight: '1',
        }}>{stage.label}</span>
      </div>
      <div style={{ fontSize: '12px', fontWeight, lineHeight: '1.25', marginBottom: '6px', minHeight: '16px', maxHeight: '16px', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
        {taskType ? `${taskType} · ${describeTaskProgress({ current, total })}` : '空闲中'}
      </div>
      <div style={{ fontSize: '12px', fontWeight: isAc ? '600' : '700', lineHeight: '1.4', marginBottom: '10px', minHeight: '34px', maxHeight: '34px', overflow: 'hidden', textOverflow: 'ellipsis', display: '-webkit-box', WebkitLineClamp: 2, WebkitBoxOrient: 'vertical', color: muted }}>
        {describeTaskDetail({ taskState: normalizedTaskState, message, current, total })}
      </div>
      <div style={{ background: progressTrackBg, border: progressTrackBorder, borderRadius: '999px', height: '12px', overflow: 'hidden', marginBottom: '10px', boxShadow: 'inset 1px 1px 2px rgba(0,0,0,0.08)' }}>
        <div style={{ background: progressFillBg, height: '100%', width: `${pct}%`, transition: 'width 0.3s cubic-bezier(0.4, 0, 0.2, 1)', boxShadow: '1px 0 4px rgba(59,184,216,0.3)' }} />
      </div>
      <div style={{ display: 'flex', gap: '8px' }}>
        {!isStopping && !isPaused && !isDone && (
          <button
            className={pauseClass}
            data-action={pauseAction}
            style={{ ...btnBase, background: '#fff' }}
            onMouseEnter={(e) => { e.currentTarget.style.transform = 'translate(-1px,-1px)'; e.currentTarget.style.boxShadow = btnHoverShadow; }}
            onMouseLeave={(e) => { e.currentTarget.style.transform = 'translate(0,0)'; e.currentTarget.style.boxShadow = btnShadow; }}
          >暂停</button>
        )}
        {isPaused && !isStopping && (
          <button
            className={resumeClass}
            data-action={resumeAction}
            style={{ ...btnBase, background: isAc ? '#ABEDC6' : '#7dd87a' }}
            onMouseEnter={(e) => { e.currentTarget.style.transform = 'translate(-1px,-1px)'; e.currentTarget.style.boxShadow = btnHoverShadow; }}
            onMouseLeave={(e) => { e.currentTarget.style.transform = 'translate(0,0)'; e.currentTarget.style.boxShadow = btnShadow; }}
          >继续</button>
        )}
        <button
          className={stopClass}
          data-action={stopAction}
          disabled={isStopDisabled}
          style={{
            ...btnBase,
            background: isAc ? '#FFBCB5' : '#e03e3e',
            cursor: isStopDisabled ? 'not-allowed' : 'pointer',
            opacity: isStopDisabled ? 0.7 : 1,
          }}
          onMouseEnter={(e) => { if (!isStopDisabled) { e.currentTarget.style.transform = 'translate(-1px,-1px)'; e.currentTarget.style.boxShadow = btnHoverShadow; } }}
          onMouseLeave={(e) => { e.currentTarget.style.transform = 'translate(0,0)'; e.currentTarget.style.boxShadow = btnShadow; }}
        >{isStopping ? '停止中...' : '停止'}</button>
      </div>
    </div>
  );
}

const barRoots = new Map();

export function renderTaskControlBar(container, props) {
  let root = barRoots.get(container);
  if (!root) {
    root = createRoot(container);
    barRoots.set(container, root);
  }
  root.render(<TaskControlBar {...props} />);
}

export function unmountTaskControlBar(container) {
  const root = barRoots.get(container);
  if (root) {
    root.unmount();
    barRoots.delete(container);
  }
}
