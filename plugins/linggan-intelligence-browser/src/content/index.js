import '../extensionPublicPath.js';
import '../content.css';
import { initThemeManager } from '../themes/themeManager.js';
import { injectLingganPendingPageControls, registerLingganPendingActionGuard } from '../linggan/pageControls.jsx';
import { createLingganPendingResult } from '../linggan/adapter.js';
import { LINGGAN_RUNTIME_ACTION } from '../linggan/runtimeActions.js';

function currentPlatform() {
  const hostname = String(location.hostname || '').toLowerCase();
  if (hostname.includes('douyin.com')) return 'douyin';
  if (hostname.includes('xiaohongshu.com') || hostname.includes('xhslink.com')) return 'xhs';
  return 'unknown';
}

function pageContext() {
  return {
    platform: currentPlatform(),
    url: location.href,
    mode: 'linggan_adapter_pending',
  };
}

async function init() {
  await initThemeManager().catch(() => {});
  injectLingganPendingPageControls(currentPlatform());
  registerLingganPendingActionGuard();
  console.info('[Linggan Intelligence Browser] 页面控制已加载；采集 adapter 尚未接通。');
}

chrome.runtime.onMessage.addListener((message = {}, _sender, sendResponse) => {
  const action = String(message.action || '').trim();
  Promise.resolve().then(() => {
    if (action === LINGGAN_RUNTIME_ACTION.GET_PAGE_CONTEXT) return { success: true, context: pageContext() };
    if (action === LINGGAN_RUNTIME_ACTION.GET_STATS) {
      return { success: true, notes: 0, comments: 0, authors: 0, source: 'linggan_adapter_pending' };
    }
    return createLingganPendingResult(action || 'platform_collection');
  }).then(sendResponse).catch((error) => {
    sendResponse({ success: false, code: 'linggan_adapter_error', message: String(error?.message || error) });
  });
  return true;
});

if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', init, { once: true });
} else {
  void init();
}
