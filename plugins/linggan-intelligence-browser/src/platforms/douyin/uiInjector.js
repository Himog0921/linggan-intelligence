import { detectDouyinPageType, detectDouyinSearchBatchContext, getDouyinSearchTabType, DY_PAGE_TYPE } from './pageDetector.js';
import { detectDouyinSecurityChallenge } from './securityChallenge.js';
import { renderButtonGroup, unmountButtonGroup } from '../../content/components/ButtonGroup.jsx';
import { renderTaskControlBar, unmountTaskControlBar } from '../../content/components/TaskControlBar.jsx';
import { showDouyinToast } from '../../content/components/Toast.jsx';
import { showActionDialog } from '../../content/components/ActionDialog.jsx';
import { getCurrentTheme } from '../../themes/themeManager.js';
import { shouldDelayTaskbarHide } from '../../shared/taskUi.js';

// 注入锁（防止 MutationObserver 引发无限循环）
const INJECT_LOCK_KEY = '__lgboom_dy_injecting';

/**
 * 在抖音页面注入采集按钮
 * 根据页面类型显示不同按钮组合
 */
export function injectDouyinUI() {
  if (window[INJECT_LOCK_KEY]) return;
  window[INJECT_LOCK_KEY] = true;

  try {
    const page = detectDouyinPageType();

    // 清除已有的注入按钮（先卸载 React root 再移除 DOM）
    cleanupDouyinInjectedUI({ includeTaskbar: false });

    if (detectDouyinSecurityChallenge({ root: document, href: window.location.href })) {
      return;
    }

    switch (page.type) {
      case DY_PAGE_TYPE.VIDEO_DETAIL:
      case DY_PAGE_TYPE.NOTE_DETAIL:
        injectVideoDetailButtons(page);
        break;
      case DY_PAGE_TYPE.SEARCH:
        injectSearchButtons();
        break;
      case DY_PAGE_TYPE.PROFILE:
        injectProfileButtons();
        break;
    }
  } finally {
    setTimeout(() => { window[INJECT_LOCK_KEY] = false; }, 150);
  }
}

export function cleanupDouyinInjectedUI({ includeTaskbar = true } = {}) {
  document.querySelectorAll('.lgboom-dy-btn-group').forEach((el) => {
    unmountButtonGroup(el);
    el.remove();
  });

  document.querySelectorAll('.lgboom-dy-progress, .lgboom-dy-work-indicator').forEach((el) => {
    el.remove();
  });

  if (includeTaskbar) {
    const bar = document.querySelector('.lgboom-dy-taskbar');
    if (bar) {
      unmountTaskControlBar(bar);
      bar.remove();
    }
  }
}

/**
 * 视频/笔记详情页按钮
 */
function injectVideoDetailButtons(page) {
  const container = document.createElement('div');
  container.className = 'lgboom-dy-btn-group';
  Object.assign(container.style, {
    position: 'fixed',
    top: '100px',
    right: '20px',
    zIndex: '2147483640',
    flexDirection: 'column',
    alignItems: 'stretch',
    minWidth: '160px',
  });

  const buttons = [
    { text: '采集视频', icon: 'collect', action: 'dy_collectVideo', style: 'primary' },
    { text: '下载视频', icon: 'download', action: 'dy_downloadVideo', style: 'secondary' },
    { text: '采集评论', icon: 'comment', action: 'dy_collectComments', style: 'secondary' },
    { text: '评论图片', icon: 'image', action: 'dy_collectCommentImages', style: 'secondary' },
  ];

  const filteredButtons = page.type === DY_PAGE_TYPE.NOTE_DETAIL
    ? buttons.filter((b) => b.action !== 'dy_downloadVideo')
    : buttons;

  renderButtonGroup(container, {
    platform: 'douyin',
    brandVariant: 'banner',
    floatingKey: page.type === DY_PAGE_TYPE.NOTE_DETAIL ? 'douyin.note-detail' : 'douyin.video-detail',
    buttons: filteredButtons,
    containerStyle: { flexDirection: 'column', alignItems: 'stretch', boxShadow: 'none' },
    brandStyle: { marginBottom: '10px', marginRight: '0', justifyContent: 'center', width: '100%' },
    buttonStyle: { width: '100%' },
  });

  document.body.appendChild(container);
}

/**
 * 搜索结果页按钮
 */
function injectSearchButtons() {
  const tabType = getDouyinSearchTabType();
  const isVideoTab = tabType === 'video';
  const isGeneralTab = tabType === 'general';

  if (!isVideoTab && !isGeneralTab) return;

  const container = document.createElement('div');
  container.className = 'lgboom-dy-btn-group';
  Object.assign(container.style, {
    position: 'fixed',
    top: '80px',
    right: '20px',
    zIndex: '2147483640',
    boxShadow: '4px 4px 0 #121212',
  });

  renderButtonGroup(container, {
    platform: 'douyin',
    compact: true,
    floatingKey: 'douyin.search',
    buttons: [
      { text: '批量视频', icon: 'video', action: 'dy_batchVideos', style: 'primary', data: { mode: 'search' } },
      { text: '批量评论', icon: 'comment', action: 'dy_batchComments', style: 'secondary', data: { mode: 'search' } },
    ],
  });

  document.body.appendChild(container);
}

/**
 * 博主主页按钮
 */
function injectProfileButtons() {
  const container = document.createElement('div');
  container.className = 'lgboom-dy-btn-group';
  Object.assign(container.style, {
    position: 'fixed',
    top: '80px',
    right: '20px',
    zIndex: '2147483640',
    boxShadow: '4px 4px 0 #121212',
  });

  renderButtonGroup(container, {
    platform: 'douyin',
    compact: true,
    floatingKey: 'douyin.profile',
    buttons: [
      { text: '采集博主', icon: 'author', action: 'dy_collectAuthor', style: 'primary' },
      { text: '批量视频', icon: 'video', action: 'dy_batchVideos', style: 'secondary', data: { mode: 'profile' } },
      { text: '批量评论', icon: 'comment', action: 'dy_batchComments', style: 'secondary', data: { mode: 'profile' } },
    ],
  });

  document.body.appendChild(container);
}

export { showDouyinToast };

export function hideDouyinProgressBar() {
  const bar = document.querySelector('.lgboom-dy-progress');
  if (bar) bar.remove();
}

export function ensureDouyinTaskControlBar() {
  let bar = document.querySelector('.lgboom-dy-taskbar');
  if (bar) return bar;

  bar = document.createElement('div');
  bar.className = 'lgboom-dy-taskbar';
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
    title: '抖音任务控制台',
    theme: getCurrentTheme(),
    platform: 'douyin',
    visible: false,
  });
  return bar;
}

export function updateDouyinTaskControlBar(state = {}) {
  const bar = ensureDouyinTaskControlBar();

  if (!state.visible) {
    bar.style.display = 'none';
    return;
  }

  bar.style.display = 'block';
  bar.dataset.taskState = String(state.taskState || 'running');
  const total = Number(state.total || 0);
  const current = Number(state.current || 0);
  const taskLabel =
    state.taskType === 'batchComments' ? '批量评论' :
    state.taskType === 'batchVideos' ? '批量视频' :
    state.taskType === 'singleComments' ? '当前评论' :
    state.taskType === 'commentImageDownload' ? '评论图片区' :
    '批量任务';

  renderTaskControlBar(bar, {
    title: '抖音任务控制台',
    taskType: taskLabel,
    taskState: state.taskState || 'running',
    current,
    total,
    message: state.message || '正在处理中...',
    theme: getCurrentTheme(),
    platform: 'douyin',
  });

  const isStopping = state.taskState === 'stopping';
  const isPaused = state.taskState === 'paused';
  const isDone = ['done', 'error', 'idle'].includes(state.taskState);

  const pauseBtn = bar.querySelector('.lgboom-dy-task-btn[data-action="dy_pauseBatch"]');
  const resumeBtn = bar.querySelector('.lgboom-dy-task-btn[data-action="dy_resumeBatch"]');
  const stopBtn = bar.querySelector('.lgboom-dy-task-btn[data-action="dy_stopBatch"]');

  if (pauseBtn) {
    pauseBtn.style.display = (isPaused || isStopping || isDone) ? 'none' : 'inline-block';
    pauseBtn.disabled = isDone || isStopping;
  }
  if (resumeBtn) {
    resumeBtn.style.display = isPaused ? 'inline-block' : 'none';
    resumeBtn.disabled = isDone || isStopping;
  }
  if (stopBtn) {
    stopBtn.textContent = isStopping ? '停止中...' : '停止';
    stopBtn.disabled = isDone || isStopping;
  }
}

export function hideDouyinTaskControlBar({ immediate = false } = {}) {
  const bar = document.querySelector('.lgboom-dy-taskbar');
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

export { showActionDialog as showDouyinActionDialog };
