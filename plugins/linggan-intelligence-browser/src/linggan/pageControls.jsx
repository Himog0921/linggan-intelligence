import { renderButtonGroup, unmountButtonGroup } from '../content/components/ButtonGroup.jsx';
import { showToast } from '../content/components/Toast.jsx';

const PENDING_ACTION_NOTICE = '该按钮的 Linggan 接收合同尚未接通：没有访问平台、没有下载媒体，也没有写入 Linggan。';

function xhsPageType() {
  const href = window.location.href;
  const pathname = window.location.pathname;
  if (/\/explore\/[a-z0-9]+/i.test(pathname) || /\/discovery\/item\/[a-z0-9]+/i.test(pathname)) return 'note';
  if (/\/user\/profile\/[a-z0-9]+\/[a-z0-9]+/i.test(pathname) && /xsec_token=/i.test(href)) return 'note';
  if (/\/search_result/.test(pathname) || href.includes('keyword=')) return 'search';
  if (/\/user\/profile\/[a-z0-9]+/i.test(pathname)) return 'profile';
  return 'unknown';
}

function douyinPageType() {
  const pathname = window.location.pathname;
  const params = new URLSearchParams(window.location.search);
  if (/^\/(?:video|note)\/[A-Za-z0-9_-]+/.test(pathname) || params.get('modal_id')) return 'detail';
  if (/^\/search/.test(pathname)) return 'search';
  if (/^\/(?:user|@)\/?/.test(pathname)) return 'profile';
  return 'unknown';
}

function removeControls(selector) {
  document.querySelectorAll(selector).forEach((element) => {
    unmountButtonGroup(element);
    element.remove();
  });
}

function injectXhsControls() {
  removeControls('.lgboom-btn-group');
  const page = xhsPageType();
  if (page === 'note') {
    const container = document.createElement('div');
    container.className = 'lgboom-btn-group';
    Object.assign(container.style, {
      position: 'fixed', top: '88px', right: '20px', zIndex: '2147483640', margin: '0',
      flexDirection: 'column', alignItems: 'stretch', minWidth: '180px',
    });
    renderButtonGroup(container, {
      platform: 'xhs', brandVariant: 'banner', floatingKey: 'xhs.note-detail',
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
    return;
  }

  if (page === 'search') {
    const header = document.querySelector('.feeds-container');
    if (!header?.parentElement) return;
    const container = document.createElement('div');
    container.className = 'lgboom-btn-group';
    renderButtonGroup(container, {
      platform: 'xhs',
      buttons: [
        { text: '批量笔记', action: 'batchNotes', style: 'primary', data: { mode: 'search' } },
        { text: '批量评论', action: 'batchComments', style: 'secondary', data: { mode: 'search' } },
      ],
      containerStyle: { padding: '8px 14px', gap: '10px', marginBottom: '10px', boxShadow: '4px 4px 0 #121212' },
      brandStyle: { marginRight: '8px', padding: '0' },
      buttonStyle: { padding: '7px 16px', borderRadius: '8px', boxShadow: '1px 1px 0 #121212', whiteSpace: 'nowrap' },
    });
    header.parentElement.insertBefore(container, header);
    return;
  }

  if (page === 'profile') {
    const narrow = window.innerWidth <= 1200;
    const container = document.createElement('div');
    container.className = 'lgboom-btn-group';
    Object.assign(container.style, {
      position: 'fixed', top: narrow ? '86px' : '96px', right: narrow ? '12px' : '18px', zIndex: '2147483640', margin: '0',
    });
    renderButtonGroup(container, {
      platform: 'xhs', compact: true, floatingKey: 'xhs.profile',
      buttons: [
        { text: '采集博主', action: 'collectAuthor', style: 'primary' },
        { text: '批量笔记', action: 'batchNotes', style: 'secondary', data: { mode: 'profile' } },
        { text: '批量评论', action: 'batchComments', style: 'secondary', data: { mode: 'profile' } },
      ],
      buttonStyle: { padding: '7px 12px', fontSize: '13px', borderRadius: '8px', boxShadow: '1px 1px 0 #121212', whiteSpace: 'nowrap' },
    });
    document.body.appendChild(container);
  }
}

function injectDouyinControls() {
  removeControls('.lgboom-dy-btn-group');
  const page = douyinPageType();
  const buttonsByPage = {
    detail: [
      { text: '采集视频', icon: 'collect', action: 'dy_collectVideo', style: 'primary' },
      { text: '下载视频', icon: 'download', action: 'dy_downloadVideo', style: 'secondary' },
      { text: '采集评论', icon: 'comment', action: 'dy_collectComments', style: 'secondary' },
      { text: '评论图片', icon: 'image', action: 'dy_collectCommentImages', style: 'secondary' },
    ],
    search: [
      { text: '批量视频', icon: 'video', action: 'dy_batchVideos', style: 'primary', data: { mode: 'search' } },
      { text: '批量评论', icon: 'comment', action: 'dy_batchComments', style: 'secondary', data: { mode: 'search' } },
    ],
    profile: [
      { text: '采集博主', icon: 'author', action: 'dy_collectAuthor', style: 'primary' },
      { text: '批量视频', icon: 'video', action: 'dy_batchVideos', style: 'secondary', data: { mode: 'profile' } },
      { text: '批量评论', icon: 'comment', action: 'dy_batchComments', style: 'secondary', data: { mode: 'profile' } },
    ],
  };
  if (!buttonsByPage[page]) return;
  const container = document.createElement('div');
  container.className = 'lgboom-dy-btn-group';
  Object.assign(container.style, {
    position: 'fixed', top: page === 'detail' ? '100px' : '80px', right: '20px', zIndex: '2147483640',
    flexDirection: page === 'detail' ? 'column' : undefined,
    alignItems: page === 'detail' ? 'stretch' : undefined,
    minWidth: page === 'detail' ? '160px' : undefined,
    boxShadow: page === 'detail' ? undefined : '4px 4px 0 #121212',
  });
  renderButtonGroup(container, {
    platform: 'douyin', compact: page !== 'detail', brandVariant: page === 'detail' ? 'banner' : 'logo',
    floatingKey: page === 'detail' ? 'douyin.video-detail' : '', buttons: buttonsByPage[page],
    containerStyle: page === 'detail' ? { flexDirection: 'column', alignItems: 'stretch', boxShadow: 'none' } : {},
    brandStyle: page === 'detail' ? { marginBottom: '10px', marginRight: '0', justifyContent: 'center', width: '100%' } : {},
    buttonStyle: page === 'detail' ? { width: '100%' } : {},
  });
  document.body.appendChild(container);
}

export function injectLingganPendingPageControls(platform = '') {
  if (platform === 'douyin') injectDouyinControls();
  if (platform === 'xhs') injectXhsControls();
}

export function registerLingganPendingActionGuard() {
  document.addEventListener('click', (event) => {
    const button = event.target.closest('.lgboom-btn, .lgboom-dy-btn, .lgboom-task-btn, .lgboom-dy-task-btn');
    if (!button) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    const label = String(button.textContent || '').trim() || '此操作';
    showToast(`${label}：${PENDING_ACTION_NOTICE}`, 'warning');
  }, true);
}
