/**
 * 抖音平台适配器
 */
import { BATCH_CONFIG, MSG } from '../../shared/constants.js';
import {
  injectDouyinUI,
  showDouyinToast,
  hideDouyinProgressBar,
  showDouyinActionDialog,
  ensureDouyinTaskControlBar,
  updateDouyinTaskControlBar,
  hideDouyinTaskControlBar,
  cleanupDouyinInjectedUI,
} from './uiInjector.js';
import { detectDouyinPageType, DY_PAGE_TYPE, isStrictDouyinDetailPage } from './pageDetector.js';
import { collectDouyinVideo, downloadDouyinVideo } from './videoCollector.js';
import { collectDouyinAuthor } from './authorCollector.js';
import { batchCollectDouyinProfileVideos, batchCollectDouyinProfileComments } from './batchController.js';
import { collectDouyinComments, downloadDouyinCommentImages } from './commentCollector.js';
import { resolveDouyinSingleCommentUiTotal } from './commentTaskSupport.js';
import { detectDouyinSecurityChallenge } from './securityChallenge.js';
import { consumeSelectorHealthAlertMessage } from '../../shared/selectorHealth.js';
import {
  runDouyinSelectorBootstrapProbe,
  runDouyinSelectorPreflight,
} from './selectorHealth.js';
import { resolveDouyinTaskbarRenderState } from './taskbarRenderState.js';
import { createManagedTaskController } from '../../shared/managedTaskController.js';
import { assertActivePluginAuthorization } from '../../workbench/runtime/pluginAuthorization.js';

/**
 * 抖音平台适配器实现
 */
const DouyinAdapter = {
  platformId: 'douyin',
  id: 'douyin',
  platformName: '抖音',
  hostPattern: /douyin\.com/,
  _shareCaptureState: {
    collecting: false,
    lastKey: '',
    lastAt: 0,
  },
  _batchTaskState: {
    controller: null,
    taskType: '',
    current: 0,
    total: 0,
    paused: false,
    cleanupIndicator: null,
  },
  _selectorProbeTimer: null,
  _initialized: false,
  _urlObserver: null,
  _reinjectTimer: null,
  _fallbackReinjectTimer: null,
  _nativeShareClickHandler: null,
  _apiBridgeCleanup: null,

  async _ensurePluginAuthorized() {
    try {
      return await assertActivePluginAuthorization();
    } catch (error) {
      showDouyinToast(String(error?.userMessage || error?.message || '当前浏览器还没有插件授权'), 'warning');
      throw error;
    }
  },

  /**
   * 初始化：设置 SPA 路由监听 + MutationObserver
   */
  init() {
    if (this._initialized) {
      this._tryInject();
      this._scheduleSelectorBootstrapProbe(420);
      return;
    }

    // 建立主世界 API 捕获数据到 content script 的桥接
    this._bindApiBridge();

    // 注入 API 捕获器到 MAIN world（拦截 fetch/XHR 获取视频真实下载地址）
    this._injectApiCapture();

    // 首次注入
    this._tryInject();
    ensureDouyinTaskControlBar();
    this._scheduleSelectorBootstrapProbe(420);

    // 绑定原生分享按钮，分享时自动采集当前视频
    this._bindNativeShareCapture();

    // SPA 路由变化监听（抖音是 React SPA，URL 会变但不触发 load 事件）
    let lastUrl = window.location.href;
    let lastSecurityChallenge = detectDouyinSecurityChallenge({ root: document, href: window.location.href });
    this._urlObserver = new MutationObserver(() => {
      if (window.__lgboom_dy_injecting) return;
      const currentUrl = window.location.href;
      const currentSecurityChallenge = detectDouyinSecurityChallenge({ root: document, href: currentUrl });
      if (currentUrl !== lastUrl || currentSecurityChallenge !== lastSecurityChallenge) {
        lastUrl = currentUrl;
        lastSecurityChallenge = currentSecurityChallenge;
        console.log('[灵感爆爆爆] 抖音页面状态变化，重新注入 UI:', currentUrl);
        if (this._reinjectTimer) clearTimeout(this._reinjectTimer);
        this._reinjectTimer = setTimeout(() => {
          this._reinjectTimer = null;
          this._tryInject();
          this._scheduleSelectorBootstrapProbe(420);
        }, 200);
        // 二次兜底：200ms 后首次注入，600ms 后再补一次防止漏掉
        if (this._fallbackReinjectTimer) clearTimeout(this._fallbackReinjectTimer);
        this._fallbackReinjectTimer = setTimeout(() => {
          this._fallbackReinjectTimer = null;
          this._tryInject();
          this._scheduleSelectorBootstrapProbe(220);
        }, 600);
      }
    });

    this._urlObserver.observe(document.body, { childList: true, subtree: true });

    this._initialized = true;
    console.log('[灵感爆爆爆] 抖音平台适配器已初始化');
  },

  cleanupLifecycleListeners() {
    if (this._selectorProbeTimer) clearTimeout(this._selectorProbeTimer);
    if (this._reinjectTimer) clearTimeout(this._reinjectTimer);
    if (this._fallbackReinjectTimer) clearTimeout(this._fallbackReinjectTimer);
    this._selectorProbeTimer = null;
    this._reinjectTimer = null;
    this._fallbackReinjectTimer = null;

    this._urlObserver?.disconnect();
    this._urlObserver = null;

    if (this._nativeShareClickHandler) {
      document.removeEventListener('click', this._nativeShareClickHandler, true);
      this._nativeShareClickHandler = null;
      window.__lgboom_dy_native_share_bound = false;
    }

    if (typeof this._apiBridgeCleanup === 'function') {
      this._apiBridgeCleanup();
      this._apiBridgeCleanup = null;
    }

    cleanupDouyinInjectedUI();
    this._initialized = false;
  },

  async _readClipboardShareText() {
    try {
      if (!navigator.clipboard?.readText) return '';
      const text = await navigator.clipboard.readText();
      return String(text || '').trim();
    } catch {
      return '';
    }
  },

  _isNativeShareTrigger(target) {
    if (!target?.closest) return false;
    if (target.closest('.lgboom-dy-btn-group, .lgboom-dy-taskbar, .lgboom-dy-dialog-overlay, .lgboom-dy-progress, .lgboom-dy-work-indicator, .lgboom-dy-toast')) {
      return false;
    }
    const directMatch = target.closest(
      [
        '[data-e2e=\"video-player-share\"]',
        '[data-e2e=\"share-icon\"]',
        '[data-e2e=\"share-button\"]',
        'button[aria-label*=\"分享\"]',
        '[role=\"button\"][aria-label*=\"分享\"]',
      ].join(','),
    );
    return Boolean(directMatch);
  },

  _buildShareCaptureKey() {
    const url = new URL(window.location.href);
    const modalId = url.searchParams.get('modal_id') || '';
    const vid = url.searchParams.get('vid') || '';
    const activeVid = document.querySelector('[data-e2e="feed-active-video"]')?.getAttribute('data-e2e-vid') || '';
    const awemeId = document.querySelector('[data-e2e="video-info"]')?.getAttribute('data-e2e-aweme-id') || '';
    return [window.location.pathname, modalId, vid, activeVid, awemeId].filter(Boolean).join('|');
  },

  _sendBackgroundAction(action, payload = {}) {
    return new Promise((resolve, reject) => {
      try {
        chrome.runtime.sendMessage({ action, ...payload }, (response) => {
          const runtimeErr = chrome.runtime.lastError;
          if (runtimeErr) {
            reject(new Error(String(runtimeErr.message || runtimeErr)));
            return;
          }
          if (response?.error) {
            reject(new Error(String(response.error)));
            return;
          }
          resolve(response || { success: true });
        });
      } catch (err) {
        reject(err);
      }
    });
  },

  _getBatchVideoDialogConfig(mode = 'profile') {
    const isSearchMode = String(mode || '').trim() === 'search';
    return {
      title: isSearchMode ? '批量采集搜索结果视频' : '批量采集视频',
      description: isSearchMode
        ? '从当前搜索结果列表里批量采集视频。默认按你眼前的页面顺位采集，也可以改成按点赞 Top N 选取。'
        : '从当前博主页批量采集视频。默认按页面顺位采集，也可以改成按点赞 Top N 选取。',
      confirmText: '开始采集',
    };
  },

  _getBatchCommentDialogConfig(mode = 'profile') {
    const isSearchMode = String(mode || '').trim() === 'search';
    return {
      title: isSearchMode ? '批量采集搜索结果评论' : '批量采集博主页评论',
      description: isSearchMode
        ? '从当前搜索结果里选前 N 条视频采评论，可按顺位或高赞优先。'
        : '从当前博主页里选前 N 条视频采评论，可按顺位或高赞优先。',
      confirmText: '开始采集',
    };
  },

  _bindNativeShareCapture() {
    if (this._nativeShareClickHandler || window.__lgboom_dy_native_share_bound) return;
    window.__lgboom_dy_native_share_bound = true;

    this._nativeShareClickHandler = async (event) => {
      if (!this._isNativeShareTrigger(event.target)) return;

      const page = detectDouyinPageType();
      if (page.type !== DY_PAGE_TYPE.VIDEO_DETAIL && page.type !== DY_PAGE_TYPE.NOTE_DETAIL) return;

      const captureKey = this._buildShareCaptureKey();
      const now = Date.now();
      if (this._shareCaptureState.collecting) return;
      if (captureKey && this._shareCaptureState.lastKey === captureKey && now - this._shareCaptureState.lastAt < 1800) {
        return;
      }

      this._shareCaptureState.collecting = true;
      this._shareCaptureState.lastKey = captureKey;
      this._shareCaptureState.lastAt = now;

      showDouyinToast('已识别分享动作，正在匹配当前视频...', 'info');

      try {
        await new Promise((resolve) => setTimeout(resolve, 900));
        const shareText = await this._readClipboardShareText();
        const result = await collectDouyinVideo({
          triggerSource: 'native_share',
          shareText,
        });

        if (result.ok) {
          showDouyinToast(`已按分享动作采集：${result.data.title?.slice(0, 20) || result.data.noteId}`, 'success');
        } else {
          showDouyinToast(`分享采集失败：${result.error}`, 'error');
        }
      } catch (err) {
        showDouyinToast(`分享采集失败：${String(err?.message || err)}`, 'error');
      } finally {
        this._shareCaptureState.collecting = false;
      }
    };
    document.addEventListener('click', this._nativeShareClickHandler, true);
  },

  _clearBatchIndicator() {
    if (typeof this._batchTaskState.cleanupIndicator === 'function') {
      this._batchTaskState.cleanupIndicator();
    }
    this._batchTaskState.cleanupIndicator = null;
  },

  _syncBatchTaskUI({
    taskType = '',
    taskState = 'running',
    current = 0,
    total = 0,
    message = '',
    target = null,
  } = {}) {
    this._batchTaskState.taskType = taskType || this._batchTaskState.taskType;
    this._batchTaskState.current = current;
    this._batchTaskState.total = total;
    const renderState = resolveDouyinTaskbarRenderState({ taskState, message });
    this._batchTaskState.paused = renderState.taskState === 'paused';
    ensureDouyinTaskControlBar();
    updateDouyinTaskControlBar({
      visible: renderState.visible,
      taskType: this._batchTaskState.taskType,
      taskState: renderState.taskState,
      current,
      total,
      message: renderState.message,
    });

    if (renderState.shouldHideAfterRender) {
      hideDouyinProgressBar();
      hideDouyinTaskControlBar({ immediate: renderState.hideImmediate });
      this._clearBatchIndicator();
      return;
    }
  },

  syncTaskUI(progress = {}) {
    this._syncBatchTaskUI({
      taskType: progress.taskType || this._batchTaskState.taskType,
      taskState: progress.taskState || progress.status || 'running',
      current: progress.current || 0,
      total: progress.total || 0,
      message: progress.message || '',
    });
  },

  startBatchTask(taskType) {
    this._batchTaskState.taskType = taskType || '';
    ensureDouyinTaskControlBar();
    updateDouyinTaskControlBar({
      visible: true,
      taskType: this._batchTaskState.taskType,
      taskState: 'running',
      current: 0,
      total: 0,
      message: '准备中',
    });
  },

  hideTaskControlBar() {
    hideDouyinTaskControlBar();
  },

  setActiveTaskType(value) {
    this._batchTaskState.taskType = value || '';
  },

  attachExternalBatchController(controller = null) {
    this._batchTaskState.controller = controller || null;
  },

  pauseBatch() {
    this._pauseBatchTask();
  },

  resumeBatch() {
    this._resumeBatchTask();
  },

  stopBatch() {
    this._stopBatchTask();
  },

  _createManagedBatchController(runTask) {
    return createManagedTaskController(runTask, {
      onFinally: () => {
        this._batchTaskState.controller = null;
        this._syncBatchTaskUI({
          taskType: this._batchTaskState.taskType,
          taskState: 'done',
          current: this._batchTaskState.current,
          total: this._batchTaskState.total,
        });
      },
    });
  },

  _startManagedTask(taskType, total, runner) {
    if (this._batchTaskState.controller?.isRunning) {
      // 安全清理：如果残留任务超过 60 秒未完成，强制清除
      const staleMs = Date.now() - (this._batchTaskState.startedAt || 0);
      if (staleMs > 60000) {
        console.warn('[灵感爆爆爆] 检测到残留任务（超 ' + Math.round(staleMs / 1000) + 's），强制清除');
        this._clearBatchIndicator();
        this._batchTaskState.controller = null;
      } else {
        showDouyinToast('当前已有任务在执行，请先暂停或停止。', 'warning');
        return false;
      }
    }
    this._batchTaskState.taskType = taskType;
    this._batchTaskState.current = 0;
    this._batchTaskState.total = total;
    this._batchTaskState.paused = false;
    this._batchTaskState.startedAt = Date.now();
    const controller = this._createManagedBatchController(runner);
    this._batchTaskState.controller = controller;
    this._syncBatchTaskUI({
      taskType,
      taskState: 'running',
      current: 0,
      total,
      message: '任务已启动，可在右下角管理',
    });
    console.log(`[灵感爆爆爆] 任务启动: ${taskType}, total=${total}`);
    controller.start();
    return true;
  },

  _pauseBatchTask({
    taskType = this._batchTaskState.taskType,
    current = this._batchTaskState.current,
    total = this._batchTaskState.total,
    message = '已暂停，可点击继续恢复',
    toastMessage = '任务已暂停',
  } = {}) {
    if (!this._batchTaskState.controller?.isRunning) return;
    this._batchTaskState.controller.pause();
    this._syncBatchTaskUI({
      taskType,
      taskState: 'paused',
      current,
      total,
      message,
    });
    showDouyinToast(toastMessage, 'warning');
  },

  _pauseForSecurityChallenge({
    taskType = this._batchTaskState.taskType,
    current = this._batchTaskState.current,
    total = this._batchTaskState.total,
    message = '检测到抖音安全验证，请先完成验证后点击“继续”。',
  } = {}) {
    this._pauseBatchTask({
      taskType,
      current,
      total,
      message,
      toastMessage: '检测到抖音安全验证，任务已自动暂停',
    });
  },

  _resumeBatchTask() {
    if (!this._batchTaskState.controller?.isRunning) return;
    this._batchTaskState.controller.resume();
    this._syncBatchTaskUI({
      taskType: this._batchTaskState.taskType,
      taskState: 'running',
      current: this._batchTaskState.current,
      total: this._batchTaskState.total,
      message: '任务继续执行中...',
    });
    showDouyinToast('任务继续中...', 'info');
  },

  _stopBatchTask() {
    if (!this._batchTaskState.controller?.isRunning) {
      hideDouyinProgressBar();
      hideDouyinTaskControlBar();
      this._clearBatchIndicator();
      return;
    }
    this._batchTaskState.controller.stop();
    this._syncBatchTaskUI({
      taskType: this._batchTaskState.taskType,
      taskState: 'stopping',
      current: this._batchTaskState.current,
      total: this._batchTaskState.total,
      message: '正在停止并收尾当前任务，请稍候...',
    });
    showDouyinToast('已请求停止，正在收尾当前任务...', 'warning');
  },

  _injectApiCapture() {
    if (document.getElementById('__lgboom_dy_api_capture')) return;
    const script = document.createElement('script');
    script.id = '__lgboom_dy_api_capture';
    script.src = chrome.runtime.getURL('injected/douyinApiCapture.js');
    script.onload = () => script.remove();
    (document.head || document.documentElement).appendChild(script);
  },

  _bindApiBridge() {
    if (this._apiBridgeCleanup || window.__lgboom_dy_bridge_bound) return;
    window.__lgboom_dy_bridge_bound = true;

    const BRIDGE_EVENT = '__lgboom_dy_api_data__';
    const SEARCH_BRIDGE_EVENT = '__lgboom_dy_search_data__';
    const BRIDGE_SOURCE = 'lgboom-dy-api-capture';
    const REQUEST_SOURCE = 'lgboom-dy-content';
    const REQUEST_TYPE = '__lgboom_dy_api_data_request__';

    window.__lgboom_dy_video_data = window.__lgboom_dy_video_data || {};
    window.__lgboom_dy_search_pages = window.__lgboom_dy_search_pages || [];
    window.__lgboom_dy_bridge_stats = window.__lgboom_dy_bridge_stats || {
      eventCount: 0,
      messageCount: 0,
      mergedCount: 0,
      searchEventCount: 0,
      searchMessageCount: 0,
      searchMergedCount: 0,
      lastAt: 0,
      lastSource: '',
      lastIds: [],
    };

    const MAX_VIDEO_CACHE = 200;

    const evictOldVideoData = () => {
      const keys = Object.keys(window.__lgboom_dy_video_data);
      if (keys.length <= MAX_VIDEO_CACHE) return;
      const toRemove = keys.slice(0, keys.length - MAX_VIDEO_CACHE);
      for (const key of toRemove) {
        delete window.__lgboom_dy_video_data[key];
      }
    };

    const mergeItems = (items, source = '') => {
      const list = Array.isArray(items) ? items : [];
      if (list.length === 0) return;
      let merged = 0;
      const ids = [];

      for (const item of list) {
        const id = String(item?.id || '').trim();
        const data = item?.data && typeof item.data === 'object' ? item.data : null;
        if (!id || !data) continue;
        window.__lgboom_dy_video_data[id] = {
          ...(window.__lgboom_dy_video_data[id] || {}),
          ...data,
        };
        merged += 1;
        if (ids.length < 8) ids.push(id);
      }

      evictOldVideoData();

      if (merged > 0) {
        window.__lgboom_dy_bridge_stats.mergedCount += merged;
        window.__lgboom_dy_bridge_stats.lastAt = Date.now();
        window.__lgboom_dy_bridge_stats.lastSource = source || '';
        window.__lgboom_dy_bridge_stats.lastIds = ids;
      }
    };

    const mergeSearchPages = (pages, source = '') => {
      const list = Array.isArray(pages) ? pages : [];
      if (list.length === 0) return;

      let merged = 0;
      for (const page of list) {
        const keyword = String(page?.keyword || '').trim();
        const searchChannel = String(page?.searchChannel || '').trim() || 'aweme_general';
        const offset = Number(page?.offset || 0);
        const items = Array.isArray(page?.items) ? page.items : [];
        if (!keyword || items.length === 0) continue;

        const nextPage = {
          ...page,
          keyword,
          searchChannel,
          offset,
          sourceUrl: String(page?.sourceUrl || source || '').trim(),
          capturedAt: Number(page?.capturedAt || Date.now()),
        };
        const pageId = `${keyword}::${searchChannel}::${offset}`;
        const existing = Array.isArray(window.__lgboom_dy_search_pages)
          ? window.__lgboom_dy_search_pages
          : [];
        const filtered = existing.filter((entry) => {
          const entryId = `${String(entry?.keyword || '').trim()}::${String(entry?.searchChannel || '').trim() || 'aweme_general'}::${Number(entry?.offset || 0)}`;
          return entryId !== pageId;
        });
        filtered.push(nextPage);
        filtered.sort((a, b) => Number(a?.capturedAt || 0) - Number(b?.capturedAt || 0));
        window.__lgboom_dy_search_pages = filtered.slice(-20);
        merged += 1;
      }

      if (merged > 0) {
        window.__lgboom_dy_bridge_stats.searchMergedCount += merged;
        window.__lgboom_dy_bridge_stats.lastAt = Date.now();
        window.__lgboom_dy_bridge_stats.lastSource = source || '';
      }
    };

    const handleWindowBridgeEvent = (event) => {
      window.__lgboom_dy_bridge_stats.eventCount += 1;
      const detail = event?.detail || {};
      mergeItems(detail.items, detail.sourceUrl || '__event_window__');
    };

    const handleWindowSearchBridgeEvent = (event) => {
      window.__lgboom_dy_bridge_stats.searchEventCount += 1;
      const detail = event?.detail || {};
      mergeSearchPages(detail.pages, detail.sourceUrl || '__search_event_window__');
    };

    const handleDocumentBridgeEvent = (event) => {
      window.__lgboom_dy_bridge_stats.eventCount += 1;
      const detail = event?.detail || {};
      mergeItems(detail.items, detail.sourceUrl || '__event_document__');
    };

    const handleDocumentSearchBridgeEvent = (event) => {
      window.__lgboom_dy_bridge_stats.searchEventCount += 1;
      const detail = event?.detail || {};
      mergeSearchPages(detail.pages, detail.sourceUrl || '__search_event_document__');
    };

    const handleWindowMessage = (event) => {
      try {
        if (event.source !== window) return;
        const data = event.data || {};
        if (data.source !== BRIDGE_SOURCE) return;
        if (data.type === BRIDGE_EVENT) {
          window.__lgboom_dy_bridge_stats.messageCount += 1;
          mergeItems(data.payload?.items, data.payload?.sourceUrl || '__postmessage__');
          return;
        }
        if (data.type === SEARCH_BRIDGE_EVENT) {
          window.__lgboom_dy_bridge_stats.searchMessageCount += 1;
          mergeSearchPages(data.payload?.pages, data.payload?.sourceUrl || '__search_postmessage__');
        }
      } catch {
        // ignore
      }
    };

    window.addEventListener(BRIDGE_EVENT, handleWindowBridgeEvent);
    window.addEventListener(SEARCH_BRIDGE_EVENT, handleWindowSearchBridgeEvent);
    document.addEventListener(BRIDGE_EVENT, handleDocumentBridgeEvent);
    document.addEventListener(SEARCH_BRIDGE_EVENT, handleDocumentSearchBridgeEvent);
    window.addEventListener('message', handleWindowMessage);

    const requestSnapshotSync = () => {
      try {
        window.postMessage({ source: REQUEST_SOURCE, type: REQUEST_TYPE }, '*');
      } catch {
        // ignore
      }
    };

    requestSnapshotSync();
    const requestTimers = [
      setTimeout(requestSnapshotSync, 500),
      setTimeout(requestSnapshotSync, 1500),
    ];
    this._apiBridgeCleanup = () => {
      window.removeEventListener(BRIDGE_EVENT, handleWindowBridgeEvent);
      window.removeEventListener(SEARCH_BRIDGE_EVENT, handleWindowSearchBridgeEvent);
      document.removeEventListener(BRIDGE_EVENT, handleDocumentBridgeEvent);
      document.removeEventListener(SEARCH_BRIDGE_EVENT, handleDocumentSearchBridgeEvent);
      window.removeEventListener('message', handleWindowMessage);
      requestTimers.forEach((timerId) => clearTimeout(timerId));
      window.__lgboom_dy_bridge_bound = false;
    };
  },

  _tryInject() {
    const page = detectDouyinPageType();
    const securityChallenge = detectDouyinSecurityChallenge({ root: document, href: window.location.href });
    if (securityChallenge || (page.type !== DY_PAGE_TYPE.UNKNOWN && page.type !== DY_PAGE_TYPE.HOME)) {
      injectDouyinUI();
    }
  },

  _runSelectorBootstrapProbe() {
    const result = runDouyinSelectorBootstrapProbe({
      document,
      win: window,
    });
    const alertMessage = consumeSelectorHealthAlertMessage(result, { win: window });
    if (alertMessage) {
      showDouyinToast(alertMessage, 'warning');
    }
    return result;
  },

  _scheduleSelectorBootstrapProbe(delayMs = 420) {
    if (this._selectorProbeTimer) clearTimeout(this._selectorProbeTimer);
    this._selectorProbeTimer = setTimeout(() => {
      this._selectorProbeTimer = null;
      this._runSelectorBootstrapProbe();
    }, delayMs);
  },

  /**
   * 注入当前页面的操作按钮
   */
  injectUI() {
    injectDouyinUI();
  },

  /**
   * 处理按钮点击
   * @param {string} action
   * @param {Object} params
   */
  async handleButtonClick(action, params = {}) {
    console.log(`[灵感爆爆爆] 抖音按钮点击: ${action}`, params);

    const preflight = runDouyinSelectorPreflight(action, {
      params,
      document,
      win: window,
    });
    if (preflight.ok === false) {
      showDouyinToast(preflight.message || '当前页面结构未通过预检，请刷新后重试。', 'warning');
      return;
    }

    switch (action) {
      case 'dy_pauseBatch':
        this._pauseBatchTask();
        break;

      case 'dy_resumeBatch':
        this._resumeBatchTask();
        break;

      case 'dy_stopBatch':
        this._stopBatchTask();
        break;

      case 'dy_collectVideo': {
        await this._ensurePluginAuthorized();
        showDouyinToast('采集中...', 'info');
        const result = await collectDouyinVideo();
        if (result.ok) {
          showDouyinToast(`视频已采集：${result.data.title?.slice(0, 20) || result.data.noteId}`, 'success');
        } else {
          showDouyinToast(`采集失败：${result.error}`, 'error');
        }
        break;
      }

      case 'dy_downloadVideo': {
        await this._ensurePluginAuthorized();
        showDouyinToast('准备下载，请稍候...', 'info');
        const result = await downloadDouyinVideo();
        if (result.ok) {
          showDouyinToast('视频下载已开始', 'success');
        } else {
          showDouyinToast(`${result.error}`, 'error');
        }
        break;
      }

      case 'dy_collectComments': {
        await this._ensurePluginAuthorized();
        console.log('[灵感爆爆爆] 采集评论：弹出设置对话框');
        const values = await showDouyinActionDialog({
          title: '采集当前评论',
          description: '设置当前视频评论上限与采集深度。留空或填 0 表示尽量全部采集，默认包含一级与二级评论。',
          confirmText: '开始采集',
          fields: [
            {
              name: 'maxTotal',
              label: '最多采集评论数',
              type: 'number',
              placeholder: '例如 20，留空=全部',
              defaultValue: '',
              min: 0,
              helpText: '不填或填 0 表示尽可能采集全部评论。',
            },
            {
              name: 'allReplies',
              label: '尽量展开全部回复评论',
              type: 'checkbox',
              defaultValue: false,
              helpText: '关闭时速度更稳；开启后会继续尝试展开更多回复。',
            },
          ],
        });
        if (!values) {
          console.log('[灵感爆爆爆] 采集评论：用户取消对话框');
          break;
        }
        console.log('[灵感爆爆爆] 采集评论：用户确认，开始启动任务');
        const maxTotal = Math.max(0, parseInt(String(values.maxTotal || '').trim(), 10) || 0);
        const maxSubComments = values.allReplies ? 0 : BATCH_CONFIG.maxSubComments;
        const started = this._startManagedTask('singleComments', maxTotal || 0, async ({ shouldStop, waitIfPaused }) => {
          try {
            console.log('[灵感爆爆爆] 采集评论：任务 runner 开始执行');
            const result = await collectDouyinComments({
              maxTotal,
              maxSubComments,
              shouldStop,
              waitIfPaused,
              onSecurityPause: ({ message, current, total }) => {
                this._pauseForSecurityChallenge({
                  taskType: 'singleComments',
                  current,
                  total: total || resolveDouyinSingleCommentUiTotal({ maxTotal }),
                  message,
                });
              },
              onProgress: (p) => {
                this._syncBatchTaskUI({
                  taskType: 'singleComments',
                  taskState: 'running',
                  current: p.current || 0,
                  total: resolveDouyinSingleCommentUiTotal({ maxTotal }),
                  message: p.message || '正在采集当前评论...',
                });
              },
            });
            if (result?.stopped) {
              showDouyinToast('当前评论采集已停止', 'warning');
            } else {
              showDouyinToast(`评论采集完成：共 ${result.total} 条`, 'success');
            }
          } catch (err) {
            showDouyinToast(`评论采集失败：${String(err?.message || err)}`, 'error');
            throw err;
          }
        });
        if (started) {
          showDouyinToast('当前评论采集已启动，可在右下角管理任务', 'info');
        }
        break;
      }

      case 'dy_collectCommentImages': {
        await this._ensurePluginAuthorized();
        if (!isStrictDouyinDetailPage()) {
          showDouyinToast('请先进入抖音视频详情页，再执行评论图片区下载', 'warning');
          break;
        }
        console.log('[灵感爆爆爆] 评论图片下载：开始启动任务');
        const started = this._startManagedTask('commentImageDownload', 1, async ({ shouldStop, waitIfPaused }) => {
          try {
            console.log('[灵感爆爆爆] 评论图片下载：任务 runner 开始执行');
            this._syncBatchTaskUI({
              taskType: 'commentImageDownload',
              taskState: 'running',
              current: 0,
              total: 1,
              message: '正在定位当前视频，准备扫描评论区...',
            });
            const result = await downloadDouyinCommentImages({
              shouldStop,
              waitIfPaused,
              onSecurityPause: ({ message, current }) => {
                this._pauseForSecurityChallenge({
                  taskType: 'commentImageDownload',
                  current,
                  total: 1,
                  message,
                });
              },
              onProgress: (p) => {
                const total = p.total || Math.max(p.current || 1, 1);
                this._syncBatchTaskUI({
                  taskType: 'commentImageDownload',
                  taskState: 'running',
                  current: p.current || 0,
                  total,
                  message: p.message || '正在扫描评论区图片...',
                });
              },
            });
            if (result.stopped) {
              if (result.downloaded > 0) {
                showDouyinToast(`已停止扫描，已打包 ${result.downloaded} 张图片（已发现 ${result.scannedImages || result.total} 张）`, 'warning');
              } else {
                showDouyinToast(result.message || '评论图片区下载已停止', 'warning');
              }
            } else if (result.success) {
              showDouyinToast(`评论图片区下载完成：成功 ${result.downloaded}/${result.total}，高清 ${result.hdCount}`, 'success');
            } else {
              showDouyinToast(`${result.message || '评论图片区下载失败'}`, 'error');
            }
          } catch (err) {
            console.error('[灵感爆爆爆] 评论图片下载失败:', err);
            showDouyinToast(`评论图片区下载失败：${String(err?.message || err)}`, 'error');
            throw err;
          }
        });
        if (started) {
          showDouyinToast('评论图片区任务已启动，可在右下角管理任务', 'info');
        } else {
          console.warn('[灵感爆爆爆] 评论图片下载：任务启动被阻止（可能有残留任务）');
        }
        break;
      }

      case 'dy_collectAuthor': {
        await this._ensurePluginAuthorized();
        showDouyinToast('采集博主信息...', 'info');
        const result = await collectDouyinAuthor();
        if (result.ok) {
          showDouyinToast(`博主已采集：${result.data.name || result.data.userId}`, 'success');
        } else {
          showDouyinToast(`采集失败：${result.error}`, 'error');
        }
        break;
      }

      case 'dy_batchVideos': {
        await this._ensurePluginAuthorized();
        const dialogConfig = this._getBatchVideoDialogConfig(params.mode);
        const values = await showDouyinActionDialog({
          ...dialogConfig,
          fields: [
            {
              name: 'maxCount',
              label: '采集数量',
              type: 'number',
              defaultValue: '20',
              min: 1,
              max: 50,
              quickOptions: [5, 10, 20, 50],
              helpText: '建议从 10 或 20 开始，稳定后再放大数量。',
            },
            {
              name: 'topByLikes',
              label: '按点赞 Top N 选取',
              type: 'checkbox',
              defaultValue: false,
              helpText: '不勾选时，按你当前看到的列表顺位采集；勾选后会先按点赞排序，再取前 N 条。',
            },
          ],
        });
        if (!values) break;
        const maxCount = Math.min(50, Math.max(1, parseInt(String(values.maxCount || '').trim(), 10) || 20));
        const topByLikes = Boolean(values.topByLikes);
        hideDouyinProgressBar();
        hideDouyinTaskControlBar();
        this._clearBatchIndicator();
        try {
          await this._sendBackgroundAction(MSG.START_BATCH_NOTES, {
            mode: String(params.mode || 'profile'),
            count: maxCount,
            topByLikes,
          });
          showDouyinToast('批量视频已启动，可在右下角管理任务', 'info');
        } catch (err) {
          showDouyinToast(`批量采集失败：${String(err?.message || err)}`, 'error');
        }
        break;
      }

      case 'dy_batchComments': {
        await this._ensurePluginAuthorized();
        const dialogConfig = this._getBatchCommentDialogConfig(params.mode);
        const values = await showDouyinActionDialog({
          ...dialogConfig,
          fields: [
            {
              name: 'maxCount',
              label: '视频数',
              type: 'number',
              defaultValue: '10',
              min: 1,
              max: 20,
              quickOptions: [5, 10, 20],
              helpText: '建议先试 5 或 10 条。',
            },
            {
              name: 'topByLikes',
              label: '高赞优先（Top N）',
              type: 'checkbox',
              defaultValue: false,
              helpText: '关闭=按当前顺位；开启=按点赞排序后取前 N 条。',
            },
            {
              name: 'maxCommentsPerVideo',
              label: '单条评论上限',
              type: 'number',
              defaultValue: '',
              min: 0,
              placeholder: '例如 50；留空 = 采全',
              quickOptions: [20, 50, 100, 0],
              helpText: '留空或填 0 = 尽量采全。',
            },
            {
              name: 'allReplies',
              label: '展开更多回复',
              type: 'checkbox',
              defaultValue: false,
              helpText: '关闭=一级+二级；开启=继续展开更多层。',
            },
          ],
        });
        if (!values) break;
        const maxCount = Math.min(20, Math.max(1, parseInt(String(values.maxCount || '').trim(), 10) || 10));
        const topByLikes = Boolean(values.topByLikes);
        const maxCommentsPerVideo = Math.max(0, parseInt(String(values.maxCommentsPerVideo || '').trim(), 10) || 0);
        hideDouyinProgressBar();
        hideDouyinTaskControlBar();
        this._clearBatchIndicator();
        try {
          await this._sendBackgroundAction(MSG.START_BATCH_COMMENTS, {
            mode: String(params.mode || 'profile'),
            count: maxCount,
            topByLikes,
            commentLimit: maxCommentsPerVideo,
            commentDepthMode: values.allReplies ? 'allReplies' : 'twoLevel',
          });
          showDouyinToast(topByLikes ? 'Top N 批量评论已启动，可在右下角管理任务' : '顺位批量评论已启动，可在右下角管理任务', 'info');
        } catch (err) {
          showDouyinToast(`批量评论失败：${String(err?.message || err)}`, 'error');
        }
        break;
      }

      default:
        console.warn('[灵感爆爆爆] 未知抖音操作:', action);
    }
  },

  /**
   * 处理来自 popup/background 的消息
   */
  async handleMessage(message) {
    console.log('[灵感爆爆爆] 抖音平台收到消息:', message.action);
    return { ok: true, platform: 'douyin' };
  },

};

export default DouyinAdapter;
