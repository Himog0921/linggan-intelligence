import '../extensionPublicPath.js';
import '../content.css';
import { initThemeManager } from '../themes/themeManager.js';
import { injectLingganPendingPageControls, registerLingganPendingActionGuard } from '../linggan/pageControls.jsx';
import { createLingganPendingResult, unavailableLingganStats } from '../linggan/adapter.js';
import { LINGGAN_RUNTIME_ACTION } from '../linggan/runtimeActions.js';
import { buildCurrentVisibleXhsDiscoveryPackage } from '../linggan/xhsVisibleSearchReader.js';
import { showToast } from './components/Toast.jsx';

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
  console.info('[Linggan Intelligence Browser] 页面控制已加载；仅 XHS 当前搜索页 Discovery 已接通。');
}

function coverageText(coverage = {}) {
  const partial = coverage.stoppedReason && coverage.stoppedReason !== 'quota_reached'
    ? '本次为部分当前页面结果；'
    : '';
  return `${partial}当前面：发现 ${coverage.discoveredCards ?? '?'}，提交 ${coverage.emittedCards ?? '?'}，字段不足/重复 ${coverage.failedCards ?? '?'}，未读取 ${coverage.notAttemptedCards ?? '?'}`;
}

function deliveryMessage(message = {}) {
  const coverage = coverageText(message.localCoverage);
  if (message.deliveryState === 'delivering') return `正在向 Linggan 本机提交。${coverage}`;
  if (message.deliveryState === 'acknowledged' || message.deliveryState === 'acknowledged_replay') {
    return `${message.deliveryState === 'acknowledged_replay' ? 'Linggan 已确认同一份历史回传，未重复入库。' : 'Linggan 已接纳本次发现卡片。'} ${coverage}`;
  }
  if (message.deliveryState === 'conflict') return `Linggan 拒绝这次冲突回传（${message.code || 'conflict'}）；请刷新页面后重新发起一次新的发现。`;
  if (message.deliveryState === 'rejected') return `Linggan 未接纳本次回传（${message.code || 'rejected'}）；当前页面材料保留在本机待检查，不会显示在 Evidence Library。`;
  return `Linggan 暂时无法接收，材料仍安全保留在本机队列等待重试。${coverage}`;
}

async function submitCurrentVisibleXhsSearch() {
  const result = buildCurrentVisibleXhsDiscoveryPackage();
  if (!result.ok) {
    showToast(result.message, 'warning');
    return;
  }
  try {
    const response = await chrome.runtime.sendMessage({
      action: LINGGAN_RUNTIME_ACTION.SUBMIT_DISCOVERY_PACKAGE,
      discoveryPackage: result.package,
      localCoverage: result.localCoverage,
    });
    if (!response?.success) {
      showToast(response?.message || '本机回传未能排队；当前页面材料没有提交。', 'warning');
      return;
    }
    showToast(`已本机排队，等待 Linggan 接收。${coverageText(result.localCoverage)}`, 'info');
  } catch {
    showToast('插件后台没有回应；当前页面材料没有确认提交，请刷新页面后重试。', 'warning');
  }
}

chrome.runtime.onMessage.addListener((message = {}, _sender, sendResponse) => {
  const action = String(message.action || '').trim();
  Promise.resolve().then(() => {
    if (action === LINGGAN_RUNTIME_ACTION.GET_PAGE_CONTEXT) return { success: true, context: pageContext() };
    if (action === LINGGAN_RUNTIME_ACTION.DISCOVERY_DELIVERY_UPDATE) {
      showToast(deliveryMessage(message), message.deliveryState === 'rejected' || message.deliveryState === 'conflict' ? 'warning' : 'info');
      return { success: true };
    }
    if (action === LINGGAN_RUNTIME_ACTION.GET_STATS) {
      return unavailableLingganStats();
    }
    return createLingganPendingResult(action || 'platform_collection');
  }).then(sendResponse).catch((error) => {
    sendResponse({ success: false, code: 'linggan_adapter_error', message: String(error?.message || error) });
  });
  return true;
});

window.addEventListener('linggan:discover-current-xhs-search', () => {
  void submitCurrentVisibleXhsSearch();
});

if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', init, { once: true });
} else {
  void init();
}
