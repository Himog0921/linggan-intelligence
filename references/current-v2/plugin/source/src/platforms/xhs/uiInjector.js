import { detectPageType } from './pageDetector.js';
import { PAGE_TYPE, COMMENT_DEPTH_MODE } from '../../shared/constants.js';
import { renderButtonGroup, unmountButtonGroup } from '../../content/components/ButtonGroup.jsx';
import { renderTaskControlBar, unmountTaskControlBar } from '../../content/components/TaskControlBar.jsx';
import { showToast } from '../../content/components/Toast.jsx';
import { showCommentLimitDialog } from '../../content/components/CommentLimitDialog.jsx';
import { showMediaDownloadDialog } from '../../content/components/MediaDownloadDialog.jsx';
import { showBatchSettingsDialog } from '../../content/components/BatchSettingsDialog.jsx';
import { shouldDelayTaskbarHide } from '../../shared/taskUi.js';

/**
 * 根据页面类型注入对应的操作按钮（小红书）
 * 使用 isInjecting 锁防止 MutationObserver 触发无限循环
 */
export function injectUI() {
  if (window.__lgboom_injecting) return;
  window.__lgboom_injecting = true;

  try {
    const page = detectPageType();

    // 清除已有的注入按钮（先卸载 React root 再移除 DOM）
    document.querySelectorAll('.lgboom-btn-group').forEach((el) => {
      unmountButtonGroup(el);
      el.remove();
    });

    switch (page.type) {
      case PAGE_TYPE.NOTE_DETAIL:
        injectNoteDetailButtons();
        break;
      case PAGE_TYPE.SEARCH:
        injectBatchButtons('search');
        break;
      case PAGE_TYPE.PROFILE:
        injectProfileButtons();
        break;
    }
  } finally {
    setTimeout(() => { window.__lgboom_injecting = false; }, 100);
  }
}

function injectNoteDetailButtons() {
  const container = document.createElement('div');
  container.className = 'lgboom-btn-group';
  Object.assign(container.style, {
    position: 'fixed',
    top: '88px',
    right: '20px',
    zIndex: '2147483640',
    margin: '0',
    flexDirection: 'column',
    alignItems: 'stretch',
    minWidth: '180px',
  });

  renderButtonGroup(container, {
    platform: 'xhs',
    brandVariant: 'banner',
    floatingKey: 'xhs.note-detail',
    buttons: [
      { text: '采集当前笔记', action: 'collectNote', style: 'primary' },
      { text: '采集当前评论', action: 'collectComment', style: 'secondary' },
      { text: '采集评论图片', action: 'collectCommentImages', style: 'secondary' },
    ],
    containerStyle: { flexDirection: 'column', alignItems: 'stretch', boxShadow: 'none' },
    brandStyle: { marginRight: '0', marginBottom: '10px', justifyContent: 'center', width: '100%' },
    buttonStyle: { width: '100%' },
  });

  document.body.appendChild(container);
}

function injectBatchButtons(mode) {
  const header = document.querySelector('.feeds-container');
  if (!header) return;

  const container = document.createElement('div');
  container.className = 'lgboom-btn-group';

  renderButtonGroup(container, {
    platform: 'xhs',
    buttons: [
      { text: '批量笔记', action: 'batchNotes', style: 'primary', data: { mode } },
      { text: '批量评论', action: 'batchComments', style: 'secondary', data: { mode } },
    ],
    containerStyle: {
      padding: '8px 14px',
      gap: '10px',
      marginBottom: '10px',
      boxShadow: '4px 4px 0 #121212',
    },
    brandStyle: {
      marginRight: '8px',
      padding: '0',
    },
    buttonStyle: {
      padding: '7px 16px',
      borderRadius: '8px',
      boxShadow: '1px 1px 0 #121212',
      whiteSpace: 'nowrap',
    },
  });

  header.parentElement.insertBefore(container, header);
}

function injectProfileButtons() {
  const isNarrow = window.innerWidth <= 1200;
  const container = document.createElement('div');
  container.className = 'lgboom-btn-group';
  Object.assign(container.style, {
    position: 'fixed',
    top: isNarrow ? '86px' : '96px',
    right: isNarrow ? '12px' : '18px',
    zIndex: '2147483640',
    margin: '0',
  });

  renderButtonGroup(container, {
    platform: 'xhs',
    compact: true,
    floatingKey: 'xhs.profile',
    buttons: [
      { text: '采集博主', action: 'collectAuthor', style: 'primary' },
      { text: '批量笔记', action: 'batchNotes', style: 'secondary', data: { mode: 'profile' } },
      { text: '批量评论', action: 'batchComments', style: 'secondary', data: { mode: 'profile' } },
    ],
    buttonStyle: { padding: '7px 12px', fontSize: '13px', borderRadius: '8px', boxShadow: '1px 1px 0 #121212', whiteSpace: 'nowrap' },
  });

  document.body.appendChild(container);
}

export function toggleStopButton(show) {
  const stopBtn = document.querySelector('.lgboom-btn[data-action="stopBatch"]');
  if (stopBtn) stopBtn.style.display = show ? 'inline-block' : 'none';
  const pauseBtn = document.querySelector('.lgboom-btn[data-action="pauseBatch"]');
  if (pauseBtn) pauseBtn.style.display = show ? 'inline-block' : 'none';
  const resumeBtn = document.querySelector('.lgboom-btn[data-action="resumeBatch"]');
  if (resumeBtn) resumeBtn.style.display = 'none';
}

export function togglePauseResumeButtons(isPaused) {
  const pauseBtn = document.querySelector('.lgboom-btn[data-action="pauseBatch"]');
  const resumeBtn = document.querySelector('.lgboom-btn[data-action="resumeBatch"]');
  if (pauseBtn) pauseBtn.style.display = isPaused ? 'none' : 'inline-block';
  if (resumeBtn) resumeBtn.style.display = isPaused ? 'inline-block' : 'none';
}

function resolveTaskLabel(taskType = '') {
  if (taskType === 'singleComments') return '当前评论';
  if (taskType === 'batchComments') return '批量评论';
  if (taskType === 'commentImages') return '评论图片区';
  return '批量笔记';
}

export function ensureTaskControlBar() {
  let bar = document.querySelector('.lgboom-taskbar');
  if (bar) return bar;

  bar = document.createElement('div');
  bar.className = 'lgboom-taskbar';
  Object.assign(bar.style, {
    position: 'fixed',
    right: '20px',
    bottom: '24px',
    zIndex: '2147483646',
    width: '320px',
    minHeight: '182px',
    padding: '12px',
    display: 'none',
    boxSizing: 'border-box',
  });

  document.body.appendChild(bar);
  renderTaskControlBar(bar, {
    title: '小红书任务控制台',
    theme: 'default',
    platform: 'xhs',
    visible: false,
  });
  return bar;
}

export function updateTaskControlBar(state = {}) {
  const bar = ensureTaskControlBar();

  if (!state.visible) {
    bar.style.display = 'none';
    return;
  }

  bar.style.display = 'block';
  bar.dataset.taskState = String(state.taskState || 'running');
  const total = state.total || 0;
  const current = state.current || 0;
  const taskLabel = resolveTaskLabel(state.taskType);
  const message = state.message || '';

  renderTaskControlBar(bar, {
    title: '小红书任务控制台',
    taskType: taskLabel,
    taskState: state.taskState || 'running',
    current,
    total,
    message,
    theme: 'default',
    platform: 'xhs',
  });

  const isStopping = state.taskState === 'stopping';
  const isDone = state.taskState === 'done' || state.taskState === 'error' || state.taskState === 'idle';

  const pauseBtn = bar.querySelector('.lgboom-btn[data-action="pauseBatch"]');
  const resumeBtn = bar.querySelector('.lgboom-btn[data-action="resumeBatch"]');
  const stopBtn = bar.querySelector('.lgboom-btn[data-action="stopBatch"]');

  if (isStopping) {
    if (pauseBtn) pauseBtn.style.display = 'none';
    if (resumeBtn) resumeBtn.style.display = 'none';
    if (stopBtn) { stopBtn.textContent = '停止中...'; stopBtn.disabled = true; }
  } else if (state.taskState === 'paused') {
    if (pauseBtn) pauseBtn.style.display = 'none';
    if (resumeBtn) resumeBtn.style.display = 'inline-block';
    if (stopBtn) { stopBtn.textContent = '停止'; stopBtn.disabled = false; }
  } else {
    if (pauseBtn) pauseBtn.style.display = isDone ? 'none' : 'inline-block';
    if (resumeBtn) resumeBtn.style.display = 'none';
    if (stopBtn) { stopBtn.textContent = '停止'; stopBtn.disabled = isDone; }
  }
  if (pauseBtn) pauseBtn.disabled = isDone || isStopping;
  if (resumeBtn) resumeBtn.disabled = isDone || isStopping;
}

export function hideTaskControlBar({ immediate = false } = {}) {
  const bar = document.querySelector('.lgboom-taskbar');
  if (!bar) return;
  const badge = bar.querySelector('span');
  const shouldDelay = shouldDelayTaskbarHide({
    taskState: bar.dataset.taskState,
    badgeText: badge?.textContent || '',
  });
  if (!immediate && shouldDelay && bar.style.display !== 'none') {
    setTimeout(() => { bar.style.display = 'none'; }, 3000);
  } else {
    bar.style.display = 'none';
  }
}

export { showCommentLimitDialog };
export { showMediaDownloadDialog };
export { showBatchSettingsDialog };
export { showToast };
