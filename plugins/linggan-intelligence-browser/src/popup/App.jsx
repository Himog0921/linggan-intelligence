import React, { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import '../extensionPublicPath.js';
import { MSG, COMMENT_DEPTH_MODE } from '../shared/constants.js';
import { formatLingganRuntimeNotice } from '../linggan/adapter.js';
import { LINGGAN_RUNTIME_ACTION } from '../linggan/runtimeActions.js';
import { requireControlReceipt } from '../linggan/controlReceipt.js';
import { BRAND_ASSETS, getBrandAssetUrl } from '../shared/brandAssets.js';
import { initThemeManager, setTheme, getCurrentTheme } from '../themes/themeManager.js';
import {
  PLATFORM, PAGE_MODE,
  detectPlatformByUrl, getModeFromUrl, getPageCapabilities,
  getPrimaryActionWarning, getSecondaryActionWarning, getBatchActionWarning,
  toFriendlyError, inferProgressStage,
  sendToTab, sendToBackground,
  unwrapTabResponseData,
  getPageContextText,
  isDouyinVideoUrl, isDouyinStrictDetailUrl,
} from './utils.js';

import TabNav from './components/TabNav.jsx';
import StatsSection from './components/StatsSection.jsx';
import ActionButtons from './components/ActionButtons.jsx';
import ProgressSection from './components/ProgressSection.jsx';
import PageContextInfo from './components/PageContextInfo.jsx';
import Notice from './components/Notice.jsx';
import FlywheelSection from './components/FlywheelSection.jsx';
import BatchSettingsModal from './components/BatchSettingsModal.jsx';
import ConfirmModal from './components/ConfirmModal.jsx';

const TABS = [
  { id: 'tab-collect', label: '采集', ariaControls: 'panel-collect' },
  { id: 'tab-data', label: '数据', ariaControls: 'panel-data' },
  { id: 'tab-config', label: '配置', ariaControls: 'panel-config' },
];

const BRAND_BANNER_SRC = getBrandAssetUrl(BRAND_ASSETS.banner);

const TASK_LEASE_STORAGE_KEY = 'lingganAdapterPendingTask';

function loadIdleClaimSnapshot(value = null) {
  if (!value || typeof value !== 'object') return null;
  const hasReason = Boolean(
    String(value.idleReasonCode || '').trim()
    || String(value.idleReasonMessage || '').trim()
    || String(value.reason?.code || '').trim()
    || String(value.reason?.message || '').trim(),
  );
  if (!hasReason) return null;
  return { ...value };
}

export default function App() {
  const [currentTheme, setCurrentThemeState] = useState('default');
  const [activeTab, setActiveTab] = useState('tab-collect');

  const [tabId, setTabId] = useState(null);
  const [tabUrl, setTabUrl] = useState('');
  const [platform, setPlatform] = useState(PLATFORM.UNKNOWN);
  const [mode, setMode] = useState(PAGE_MODE.UNKNOWN);
  const [isDyVideoPage, setIsDyVideoPage] = useState(false);
  const [isDyStrictDetailPage, setIsDyStrictDetailPage] = useState(false);
  const [isStableSearchList, setIsStableSearchList] = useState(false);
  const [capabilities, setCapabilities] = useState({});

  const [stats, setStats] = useState({ notes: null, comments: null, authors: null, statsState: 'unknown' });

  const [progressVisible, setProgressVisible] = useState(false);
  const [progressCurrent, setProgressCurrent] = useState(0);
  const [progressTotal, setProgressTotal] = useState(0);
  const [progressStatus, setProgressStatus] = useState('');
  const [progressStage, setProgressStage] = useState({ label: '', className: '', description: '' });
  const [progressDepthMode, setProgressDepthMode] = useState(null);

  const [batchControlsVisible, setBatchControlsVisible] = useState(false);
  const [batchPaused, setBatchPaused] = useState(false);
  const [batchStopping, setBatchStopping] = useState(false);
  const [busyActions, setBusyActions] = useState({});

  const [notice, setNotice] = useState({ message: '', type: 'info', visible: false });
  const [idleClaimSnapshot, setIdleClaimSnapshot] = useState(null);

  const [flywheelStatus, setFlywheelStatus] = useState('unconfigured');

  const [batchModalOpen, setBatchModalOpen] = useState(false);
  const [batchModalType, setBatchModalType] = useState('notes');
  const [batchModalPlatform, setBatchModalPlatform] = useState(PLATFORM.XHS);
  const [batchModalMode, setBatchModalMode] = useState('single');
  const [commentLimitOptions, setCommentLimitOptions] = useState(null);
  const batchModalResolveRef = useRef(null);

  const [confirmDialog, setConfirmDialog] = useState({
    open: false,
    title: '',
    message: '',
    detail: '',
    confirmText: '',
    confirmTone: 'danger',
    onConfirm: null,
  });
  const noticeTimerRef = useRef(null);
  const busyActionsRef = useRef({});

  useEffect(() => {
    let mounted = true;

    async function init() {
      try { await initThemeManager(); } catch {}
      const theme = (() => { try { return getCurrentTheme(); } catch { return 'default'; } })();
      if (mounted) {
        setCurrentThemeState(theme);
        if (theme === 'ac-ui') {
          document.body.setAttribute('data-theme', 'ac-ui');
        }
      }

      const [tab] = await chrome?.tabs?.query?.({ active: true, currentWindow: true }) || [];
      const url = tab?.url || '';
      const id = tab?.id;
      if (!mounted) return;

      setTabId(id);
      setTabUrl(url);

      let detectedPlatform = detectPlatformByUrl(url);
      let detectedMode = getModeFromUrl(url, detectedPlatform);
      let detectedIsDyVideo = detectedPlatform === PLATFORM.DOUYIN && isDouyinVideoUrl(url);
      let detectedIsDyStrict = detectedPlatform === PLATFORM.DOUYIN && isDouyinStrictDetailUrl(url);
      let detectedIsStableSearch = detectedMode === PAGE_MODE.SEARCH;

      if (id && detectedPlatform !== PLATFORM.UNKNOWN) {
        try {
          const response = await sendToTab(id, { action: MSG.GET_PAGE_CONTEXT }, { timeoutMs: 1800 });
          const pageContext = unwrapTabResponseData(response, response?.context || null) || response?.context || null;
          if (pageContext?.platform) {
            detectedPlatform = pageContext.platform;
            detectedMode = pageContext.mode || detectedMode;
            detectedIsDyVideo = Boolean(pageContext.isDyVideoPage);
            detectedIsDyStrict = Boolean(pageContext.isDyStrictDetailPage);
            detectedIsStableSearch = Boolean(pageContext.isStableSearchList);
          }
        } catch {}
      }

      const caps = {
        ...getPageCapabilities(detectedPlatform, detectedMode, {
          isDyVideoPage: detectedIsDyVideo,
          isDyStrictDetailPage: detectedIsDyStrict,
          isStableSearchList: detectedIsStableSearch,
        }),
      };

      if (!mounted) return;
      setPlatform(detectedPlatform);
      setMode(detectedMode);
      setIsDyVideoPage(detectedIsDyVideo);
      setIsDyStrictDetailPage(detectedIsDyStrict);
      setIsStableSearchList(detectedIsStableSearch);
      setCapabilities(caps);

      if (!id) {
        showNotice('没有找到当前页面，请切回小红书或抖音页面后重试。', 'warning');
        return;
      }
      if (detectedPlatform === PLATFORM.UNKNOWN) {
        showNotice('当前页面暂不支持，请打开小红书或抖音页面。', 'warning');
      }

      loadStats(id);

      setFlywheelStatus('configured');
    }

    init();
    return () => { mounted = false; };
  }, []);

  useEffect(() => {
    let mounted = true;
    const readTaskLeaseSnapshot = async () => {
      try {
        const data = await chrome?.storage?.local?.get?.(TASK_LEASE_STORAGE_KEY);
        if (!mounted) return;
        setIdleClaimSnapshot(loadIdleClaimSnapshot(data?.[TASK_LEASE_STORAGE_KEY] || null));
      } catch {
        if (mounted) setIdleClaimSnapshot(null);
      }
    };

    readTaskLeaseSnapshot();

    const handleStorageChange = (changes, areaName) => {
      if (areaName !== 'local' || !changes?.[TASK_LEASE_STORAGE_KEY]) return;
      setIdleClaimSnapshot(loadIdleClaimSnapshot(changes[TASK_LEASE_STORAGE_KEY]?.newValue || null));
    };

    chrome?.storage?.onChanged?.addListener?.(handleStorageChange);
    return () => {
      mounted = false;
      chrome?.storage?.onChanged?.removeListener?.(handleStorageChange);
    };
  }, []);

  useEffect(() => {
    if (!chrome?.runtime?.onMessage) return;
    const listener = (message) => {
      if (message.action === MSG.PROGRESS) {
        setProgressVisible(true);
        setProgressCurrent(message.current || 0);
        setProgressTotal(message.total || 0);
        const displayStatus = message.message || message.progressEvent?.message || message.status || '';
        setProgressStatus(displayStatus);
        setProgressDepthMode(message.commentDepthMode || null);
        const stage = inferProgressStage({
          statusText: displayStatus,
          taskState: message.taskState || message.progressEvent?.status,
          stage: message.stage || message.phase || message.progressEvent?.stage,
          current: message.current || 0,
          total: message.total || 0,
          error: message.error || message.progressEvent?.error,
        });
        setProgressStage(stage);
        const paused = message.taskState === 'paused';
        setBatchControlsVisible(true);
        setBatchPaused(paused);
        setBatchStopping(false);
        if (message.taskState === 'error' && message.error?.message) {
          showNoticeRef.current(message.error.message, 'warning');
        } else {
          hideNoticeRef.current?.();
        }
      }
      if (message.action === MSG.COLLECT_DONE) {
        setProgressVisible(false);
        setBatchControlsVisible(false);
        setBatchPaused(false);
        setBatchStopping(false);
        // A page-side reader finishing is not a Linggan admission receipt.  The active runtime
        // reports delivery separately; never turn this legacy progress event into success.
        showNotice('页面读取已结束；请等待 Linggan 本机交付或接纳状态。', 'info');
        chrome.tabs.query({ active: true, currentWindow: true }).then(([t]) => {
          if (t?.id) loadStats(t.id);
        });
      }
    };
    chrome.runtime.onMessage.addListener(listener);
    return () => chrome.runtime.onMessage.removeListener(listener);
  }, []);

  const showNotice = useCallback((message, type = 'info') => {
    if (noticeTimerRef.current) clearTimeout(noticeTimerRef.current);
    setNotice({ message, type, visible: true });
    noticeTimerRef.current = setTimeout(() => {
      setNotice((current) => ({ ...current, visible: false }));
      noticeTimerRef.current = null;
    }, type === 'error' ? 5000 : 3600);
  }, []);

  const hideNotice = useCallback(() => {
    if (noticeTimerRef.current) {
      clearTimeout(noticeTimerRef.current);
      noticeTimerRef.current = null;
    }
    setNotice({ message: '', type: 'info', visible: false });
  }, []);

  const showNoticeRef = useRef(showNotice);
  const hideNoticeRef = useRef(hideNotice);
  showNoticeRef.current = showNotice;
  hideNoticeRef.current = hideNotice;

  useEffect(() => () => {
    if (noticeTimerRef.current) clearTimeout(noticeTimerRef.current);
  }, []);

  const setBusyActionState = useCallback((key, busy) => {
    const next = { ...busyActionsRef.current };
    if (busy) next[key] = true;
    else delete next[key];
    busyActionsRef.current = next;
    setBusyActions(next);
  }, []);

  const withBusyAction = useCallback(async (key, job) => {
    if (!key) return job();
    if (busyActionsRef.current[key]) return undefined;
    setBusyActionState(key, true);
    try {
      return await job();
    } finally {
      setBusyActionState(key, false);
    }
  }, [setBusyActionState]);

  const showConfirmDialog = useCallback(({ title, message, detail = '', confirmText = '确认', confirmTone = 'danger' }) => {
    return new Promise((resolve) => {
      setConfirmDialog({
        open: true,
        title,
        message,
        detail,
        confirmText,
        confirmTone,
        onConfirm: resolve,
      });
    });
  }, []);

  const handleConfirmResolve = useCallback((result) => {
    if (confirmDialog.onConfirm) confirmDialog.onConfirm(result);
    setConfirmDialog({
      open: false,
      title: '',
      message: '',
      detail: '',
      confirmText: '',
      confirmTone: 'danger',
      onConfirm: null,
    });
  }, [confirmDialog]);

  const idleClaimNotice = useMemo(
    () => formatLingganRuntimeNotice(idleClaimSnapshot),
    [idleClaimSnapshot],
  );
  const displayNotice = notice.visible ? notice : idleClaimNotice;

  const loadStats = useCallback(async (id) => {
    try {
      const response = await sendToTab(id, { action: MSG.GET_STATS });
      const stats = unwrapTabResponseData(response, response) || {};
      if (stats) {
        setStats({
          notes: Number.isFinite(stats.notes) ? stats.notes : null,
          comments: Number.isFinite(stats.comments) ? stats.comments : null,
          authors: Number.isFinite(stats.authors) ? stats.authors : null,
          statsState: stats.statsState || 'unknown',
        });
      }
    } catch {
      setStats({ notes: null, comments: null, authors: null, statsState: 'unknown' });
    }
  }, []);

  // The testing package is deliberately LOCAL_TRUSTED. This check is named for the actual
  // boundary: it validates local execution readiness, not a retired account/station grant.
  const ensureLocalTrustedRuntime = useCallback(() => true, []);

  const handleThemeToggle = useCallback(async () => {
    const next = currentTheme === 'ac-ui' ? 'default' : 'ac-ui';
    try { await setTheme(next); } catch {}
    setCurrentThemeState(next);
    document.body.setAttribute('data-theme', next === 'ac-ui' ? 'ac-ui' : '');
  }, [currentTheme]);

  const handleCollectNote = useCallback(async () => {
    if (!ensureLocalTrustedRuntime()) return;
    if (!capabilities.canCollectPrimary) {
      showNotice(getPrimaryActionWarning(platform, mode, capabilities), 'warning');
      return;
    }
    await withBusyAction('collectPrimary', async () => {
      hideNotice();
      setProgressVisible(true);
      setProgressCurrent(0);
      setProgressTotal(1);
      setProgressStatus(platform === PLATFORM.DOUYIN ? '正在发起视频采集...' : '正在发起笔记采集...');
      try {
        await sendToTab(tabId, { action: LINGGAN_RUNTIME_ACTION.COLLECT_CURRENT_CONTENT });
      } catch (err) {
        setProgressVisible(false);
        showNotice(toFriendlyError(err), 'warning');
      }
    });
  }, [capabilities, platform, mode, tabId, hideNotice, showNotice, withBusyAction, ensureLocalTrustedRuntime]);

  const handleCollectSecondary = useCallback(async () => {
    if (!ensureLocalTrustedRuntime()) return;
    if (!capabilities.canCollectSecondary) {
      showNotice(getSecondaryActionWarning(platform, mode, capabilities), 'warning');
      return;
    }
    const isCommentScene = capabilities.secondaryAction === 'comment';
    let payload = { action: isCommentScene ? LINGGAN_RUNTIME_ACTION.COLLECT_CURRENT_COMMENTS : LINGGAN_RUNTIME_ACTION.COLLECT_CURRENT_AUTHOR };
    if (isCommentScene) {
      const settings = await openCommentLimitSettings({
        title: platform === PLATFORM.DOUYIN ? '抖音当前评论设置' : '小红书当前评论设置',
        subtitle: platform === PLATFORM.DOUYIN
          ? '设置当前视频评论上限与采集深度。留空或填 0 表示全部采集，包含二级评论。'
          : '设置当前笔记评论上限与采集深度。留空或填 0 表示全部采集，包含二级评论。',
        confirmText: '开始采集',
      });
      if (!settings) return;
      const commentDepthMode = settings.commentDepthMode === COMMENT_DEPTH_MODE.ALL_REPLIES
        ? COMMENT_DEPTH_MODE.ALL_REPLIES
        : COMMENT_DEPTH_MODE.TWO_LEVEL;
      payload = {
        action: LINGGAN_RUNTIME_ACTION.COLLECT_CURRENT_COMMENTS,
        maxTotal: settings.maxTotal,
        maxSubComments: commentDepthMode === COMMENT_DEPTH_MODE.ALL_REPLIES ? 0 : undefined,
        sortMode: 'hot',
        triggerSource: 'popup_manual',
        commentDepthMode,
      };
    }
    await withBusyAction('collectSecondary', async () => {
      hideNotice();
      setProgressVisible(true);
      setProgressCurrent(0);
      setProgressTotal(1);
      const action = isCommentScene ? LINGGAN_RUNTIME_ACTION.COLLECT_CURRENT_COMMENTS : LINGGAN_RUNTIME_ACTION.COLLECT_CURRENT_AUTHOR;
      setProgressStatus(isCommentScene ? '正在发起评论采集...' : '正在发起博主采集...');
      try {
        await sendToTab(tabId, isCommentScene ? payload : { action });
      } catch (err) {
        setProgressVisible(false);
        showNotice(toFriendlyError(err), 'warning');
      }
    });
  }, [capabilities, platform, mode, tabId, hideNotice, showNotice, withBusyAction, ensureLocalTrustedRuntime]);

  const handleCommentImages = useCallback(async () => {
    if (!ensureLocalTrustedRuntime()) return;
    if (!capabilities.canDownloadCommentImages) {
      showNotice('请先进入抖音严格详情页，再执行评论图片区下载。', 'warning');
      return;
    }
    const settings = await openCommentLimitSettings({
      title: '抖音评论图片区设置',
      subtitle: '设置评论扫描上限与采集深度。留空或填 0 表示尽量扫描全部评论并下载高清评论图片。',
      confirmText: '开始下载',
    });
    if (!settings) return;
    const commentDepthMode = settings.commentDepthMode === COMMENT_DEPTH_MODE.ALL_REPLIES
      ? COMMENT_DEPTH_MODE.ALL_REPLIES
      : COMMENT_DEPTH_MODE.TWO_LEVEL;
    await withBusyAction('commentImages', async () => {
      hideNotice();
      setProgressVisible(true);
      setProgressCurrent(0);
      setProgressTotal(1);
      setProgressStatus('正在确认评论图片区是否具备 Linggan 媒体回传合同...');
      try {
        const result = await sendToTab(tabId, {
          action: LINGGAN_RUNTIME_ACTION.ACQUIRE_COMMENT_MEDIA,
          maxTotal: settings.maxTotal,
          maxSubComments: commentDepthMode === COMMENT_DEPTH_MODE.ALL_REPLIES ? 0 : undefined,
          commentDepthMode,
        });
        setProgressVisible(false);
        if (result?.state === 'not_available') {
          showNotice(result?.message || '评论图片区暂不可用：未执行下载。', 'warning');
        } else if (result?.stopped) {
          showNotice(
            result?.downloaded > 0
              ? `评论图片区已停止，已打包 ${result?.downloaded || 0}/${result?.total || 0}，高清 ${result?.hdCount || 0}`
              : (result?.message || '评论图片区下载已停止'),
            'warning',
          );
        } else {
          showNotice(
            '评论图片区任务已交给 Linggan Runtime；请等待本机交付或接纳状态。',
            'success',
          );
        }
      } catch (err) {
        setProgressVisible(false);
        showNotice(toFriendlyError(err), 'warning');
      }
    });
  }, [capabilities, tabId, hideNotice, showNotice, withBusyAction, ensureLocalTrustedRuntime]);

  const handleBatchNotes = useCallback(async () => {
    if (!ensureLocalTrustedRuntime()) return;
    if (!capabilities.canBatchNotes) {
      showNotice(getBatchActionWarning(platform, mode, capabilities), 'warning');
      return;
    }
    const settings = await openBatchSettings('notes', platform);
    if (!settings) return;
    await withBusyAction('batchNotes', async () => {
      hideNotice();
      try {
        await sendToTab(tabId, {
          action: LINGGAN_RUNTIME_ACTION.START_BATCH_CONTENT,
          mode,
          count: settings.count,
          topByLikes: settings.topByLikes,
          searchFilters: settings.searchFilters,
        });
        setProgressVisible(true);
        setProgressCurrent(0);
        setProgressTotal(settings.count);
        setProgressStatus('批量笔记页面读取已启动；结果将进入 Linggan 本机交付队列');
        setBatchControlsVisible(true);
        setBatchPaused(false);
        setBatchStopping(false);
        showNotice(`批量笔记已开始页面读取：本轮最多 ${settings.count} 条，尚未代表 Linggan 已接纳。`, 'info');
      } catch (err) {
        setProgressVisible(false);
        setBatchControlsVisible(false);
        showNotice(toFriendlyError(err), 'warning');
      }
    });
  }, [capabilities, platform, mode, tabId, hideNotice, showNotice, withBusyAction, ensureLocalTrustedRuntime]);

  const handleBatchComments = useCallback(async () => {
    if (!ensureLocalTrustedRuntime()) return;
    if (!capabilities.canBatchComments) {
      showNotice(getBatchActionWarning(platform, mode, capabilities), 'warning');
      return;
    }
    const settings = await openBatchSettings('comments', platform);
    if (!settings) return;
    await withBusyAction('batchComments', async () => {
      hideNotice();
      try {
        await sendToTab(tabId, {
          action: LINGGAN_RUNTIME_ACTION.START_BATCH_COMMENTS,
          mode,
          count: settings.count,
          topByLikes: settings.topByLikes,
          searchFilters: settings.searchFilters,
          commentLimit: settings.commentLimit,
          commentDepthMode: settings.commentDepthMode,
        });
        setProgressVisible(true);
        setProgressCurrent(0);
        setProgressTotal(settings.count || 0);
        setProgressStatus('批量评论页面读取已启动；结果将进入 Linggan 本机交付队列');
        setProgressDepthMode(settings.commentDepthMode);
        setBatchControlsVisible(true);
        setBatchPaused(false);
        setBatchStopping(false);
        showNotice(`批量评论已开始页面读取：本轮最多 ${settings.count} 条，尚未代表 Linggan 已接纳。`, 'info');
      } catch (err) {
        setProgressVisible(false);
        setBatchControlsVisible(false);
        showNotice(toFriendlyError(err), 'warning');
      }
    });
  }, [capabilities, platform, mode, tabId, hideNotice, showNotice, withBusyAction, ensureLocalTrustedRuntime]);

  const handlePause = useCallback(async () => {
    await withBusyAction('pauseBatch', async () => {
      hideNotice();
      try {
        const result = await sendToTab(tabId, { action: LINGGAN_RUNTIME_ACTION.PAUSE_ACTIVE_BATCH });
        requireControlReceipt(result, 'paused');
        setBatchPaused(true);
        showNotice('任务已暂停，可随时继续。', 'info');
      } catch (err) {
        showNotice(toFriendlyError(err), 'warning');
      }
    });
  }, [tabId, hideNotice, showNotice, withBusyAction]);

  const handleResume = useCallback(async () => {
    await withBusyAction('resumeBatch', async () => {
      hideNotice();
      try {
        const result = await sendToTab(tabId, { action: LINGGAN_RUNTIME_ACTION.RESUME_ACTIVE_BATCH });
        requireControlReceipt(result, 'running');
        setBatchPaused(false);
        showNotice('任务继续执行中。', 'info');
      } catch (err) {
        showNotice(toFriendlyError(err), 'warning');
      }
    });
  }, [tabId, hideNotice, showNotice, withBusyAction]);

  const handleStop = useCallback(async () => {
    const confirmed = await showConfirmDialog({
      title: '确认停止当前任务',
      message: '停止后会结束当前批量执行，本轮未处理完的内容不会继续自动采集。',
      detail: '如果只是暂时离开，优先使用“暂停”，这样可以稍后继续当前进度。',
      confirmText: '确认停止',
      confirmTone: 'danger',
    });
    if (!confirmed) return;
    await withBusyAction('stopBatch', async () => {
      hideNotice();
      setBatchStopping(true);
      try {
        const result = await sendToTab(tabId, { action: LINGGAN_RUNTIME_ACTION.STOP_ACTIVE_BATCH });
        requireControlReceipt(result, 'stopped');
        setBatchControlsVisible(false);
        setProgressVisible(false);
        setBatchStopping(false);
        showNotice('任务已停止。', 'warning');
      } catch (err) {
        setBatchStopping(false);
        showNotice(toFriendlyError(err), 'warning');
      }
    });
  }, [tabId, hideNotice, showNotice, withBusyAction, showConfirmDialog]);

  const handleDashboard = useCallback(async () => {
    if (!ensureLocalTrustedRuntime()) return;
    await withBusyAction('openDashboard', async () => {
      hideNotice();
      try {
        const result = await sendToBackground(LINGGAN_RUNTIME_ACTION.TOGGLE_DASHBOARD, { tabId });
        if (!result?.success) throw new Error(result?.message || '本机暂存面板未打开');
      } catch (err) {
        showNotice(toFriendlyError(err), 'warning');
      }
    });
  }, [tabId, hideNotice, showNotice, withBusyAction, ensureLocalTrustedRuntime]);

  const handleExport = useCallback(async () => {
    showNotice('快速导出暂不可用：尚未具备 Linggan Runtime 导出合同，未导出任何数据。', 'warning');
  }, [showNotice]);

  const handleMaintenance = useCallback(async () => {
    showNotice('数据维护暂不可用：尚未具备 Linggan Runtime 数据维护合同，未修改任何本机数据。', 'warning');
  }, [showNotice]);

  const handleFlywheelTest = useCallback(async () => {
    await withBusyAction('flywheelTest', async () => {
      hideNotice();
      setFlywheelStatus('testing');
      try {
        const result = await sendToBackground(LINGGAN_RUNTIME_ACTION.TEST_FLYWHEEL_CONNECTION);
        if (result?.readiness?.reachable) {
          setFlywheelStatus('connected');
          showNotice('Linggan 本机服务可访问；真实采集接收合同仍需逐项接通。', 'info');
        } else {
          setFlywheelStatus('disconnected');
          showNotice(result?.readiness?.message || 'Linggan 本机服务当前不可访问。', 'warning');
        }
      } catch (err) {
        setFlywheelStatus('disconnected');
        showNotice(`Linggan 本机检查失败：${err.message || '无法连接'}`, 'warning');
      }
    });
  }, [hideNotice, showNotice, withBusyAction]);

  const openBatchSettings = useCallback((type, plat) => {
    return new Promise((resolve) => {
      batchModalResolveRef.current = resolve;
      setBatchModalType(type);
      setBatchModalPlatform(plat);
      setBatchModalMode(mode);
      setBatchModalOpen(true);
    });
  }, [mode]);

  const openCommentLimitSettings = useCallback((options) => {
    return new Promise((resolve) => {
      batchModalResolveRef.current = resolve;
      setBatchModalType('comments');
      setBatchModalPlatform(platform);
      setBatchModalMode('single');
      setCommentLimitOptions(options);
      setBatchModalOpen(true);
    });
  }, [platform]);

  const handleBatchModalConfirm = useCallback((settings) => {
    setBatchModalOpen(false);
    setCommentLimitOptions(null);
    if (batchModalResolveRef.current) {
      batchModalResolveRef.current(settings);
      batchModalResolveRef.current = null;
    }
  }, []);

  const handleBatchModalCancel = useCallback(() => {
    setBatchModalOpen(false);
    if (batchModalResolveRef.current) {
      batchModalResolveRef.current(null);
      batchModalResolveRef.current = null;
    }
    setCommentLimitOptions(null);
  }, []);

  const { scene, hint, tags } = getPageContextText(platform, mode, { isDyVideoPage, isDyStrictDetailPage, isStableSearchList });

  const platformLabel = platform === PLATFORM.XHS ? '小红书' : platform === PLATFORM.DOUYIN ? '抖音' : '未识别';
  const nextThemeLabel = currentTheme === 'ac-ui' ? '默认' : 'AC';
  const nextThemeTitle = currentTheme === 'ac-ui' ? '切换到默认主题' : '切换到 AC 主题';

  return (
    <div className="popup-container" data-theme={currentTheme === 'ac-ui' ? 'ac-ui' : undefined}>
      <header className="popup-header">
        <div className="header-brand-stage">
          <div className="header-brand-banner-shell" aria-hidden="true">
            <img className="header-brand-banner" src={BRAND_BANNER_SRC} alt="" />
          </div>
        </div>
        <div className="header-side">
          <div className="header-controls">
            <span className="header-badge" id="platformBadge">{platformLabel}</span>
            <button
              id="themeToggle"
              className="theme-toggle-btn"
              onClick={handleThemeToggle}
              title={nextThemeTitle}
              aria-label={nextThemeTitle}
            >
              {nextThemeLabel}
            </button>
          </div>
          <div className="header-copy">
            <h1>灵感爆爆爆</h1>
          </div>
        </div>
      </header>

      <TabNav tabs={TABS} activeTab={activeTab} onTabChange={setActiveTab} />

      <main>
        {activeTab === 'tab-collect' && (
          <div id="panel-collect" className="tab-panel" role="tabpanel" aria-labelledby="tab-collect">
            <PageContextInfo
              platform={platform}
              scene={scene}
              hint={hint}
              tags={tags}
              capabilities={capabilities}
            />

            <div className="action-card">
              <div className="actions-section">
                <ActionButtons
                  platform={platform}
                  capabilities={capabilities}
                  onCollectNote={handleCollectNote}
                  onCollectSecondary={handleCollectSecondary}
                  onCommentImages={handleCommentImages}
                  busyPrimary={Boolean(busyActions.collectPrimary)}
                  busySecondary={Boolean(busyActions.collectSecondary)}
                  busyCommentImages={Boolean(busyActions.commentImages)}
                />
                <div className="btn-row">
                  <button
                    id="btnBatchNotes"
                    className={`popup-btn primary small${busyActions.batchNotes ? ' is-busy' : ''}`}
                    disabled={!capabilities.canBatchNotes || Boolean(busyActions.batchNotes)}
                    onClick={handleBatchNotes}
                  >
                    {busyActions.batchNotes ? '启动中...' : (platform === PLATFORM.DOUYIN ? '批量视频' : '批量笔记')}
                  </button>
                  <button
                    id="btnBatchComments"
                    className={`popup-btn primary small${busyActions.batchComments ? ' is-busy' : ''}`}
                    disabled={!capabilities.canBatchComments || Boolean(busyActions.batchComments)}
                    onClick={handleBatchComments}
                  >
                    {busyActions.batchComments ? '启动中...' : '批量评论'}
                  </button>
                </div>
              </div>
            </div>

            <ProgressSection
              visible={progressVisible}
              current={progressCurrent}
              total={progressTotal}
              status={progressStatus}
              stage={progressStage}
              depthMode={progressDepthMode}
            />

            {batchControlsVisible && (
              <div id="batchControlRow" className="btn-row">
                {batchStopping ? (
                  <button className="popup-btn small" disabled>
                    停止中...
                  </button>
                ) : (
                  <>
                    <button
                      id="btnPause"
                      className={`popup-btn secondary small${busyActions.pauseBatch ? ' is-busy' : ''}`}
                      onClick={handlePause}
                      disabled={Boolean(busyActions.pauseBatch)}
                      style={{ display: batchPaused ? 'none' : 'block' }}
                    >
                      {busyActions.pauseBatch ? '暂停中...' : '暂停'}
                    </button>
                    <button
                      id="btnResume"
                      className={`popup-btn primary small${busyActions.resumeBatch ? ' is-busy' : ''}`}
                      onClick={handleResume}
                      disabled={Boolean(busyActions.resumeBatch)}
                      style={{ display: batchPaused ? 'block' : 'none' }}
                    >
                      {busyActions.resumeBatch ? '继续中...' : '继续'}
                    </button>
                    <button
                      id="btnStop"
                      className={`popup-btn danger small${busyActions.stopBatch ? ' is-busy' : ''}`}
                      onClick={handleStop}
                      disabled={Boolean(busyActions.stopBatch)}
                    >
                      {busyActions.stopBatch ? '停止中...' : '停止'}
                    </button>
                  </>
                )}
              </div>
            )}

            <div className="bottom-section">
              <button className={`popup-btn outline${busyActions.openDashboard ? ' is-busy' : ''}`} id="btnDashboard" onClick={handleDashboard} disabled={Boolean(busyActions.openDashboard)}>
                {busyActions.openDashboard ? '打开中...' : '打开本机暂存面板'}
              </button>
              <button className={`popup-btn outline${busyActions.quickExport ? ' is-busy' : ''}`} id="btnExport" onClick={handleExport} disabled={Boolean(busyActions.quickExport)}>
                {busyActions.quickExport ? '导出中...' : '快速导出'}
              </button>
              <button className={`popup-btn outline${busyActions.maintenance ? ' is-busy' : ''}`} id="btnMaintenance" onClick={handleMaintenance} disabled={Boolean(busyActions.maintenance)}>
                {busyActions.maintenance ? '整理中...' : '数据维护'}
              </button>
            </div>
          </div>
        )}

        {activeTab === 'tab-data' && (
          <div id="panel-data" className="tab-panel" role="tabpanel" aria-labelledby="tab-data">
            <StatsSection stats={stats} />
            <section className="context-section">
              <h2>本机数据状态</h2>
              <p>页面读取会先进入 Browser Producer 的本机待交付队列；只有 Linggan 回执确认后，才会成为可用 Evidence。此测试包不提供 Cookie、账号、工位或授权码管理入口。</p>
            </section>
          </div>
        )}

        {activeTab === 'tab-config' && (
          <div id="panel-config" className="tab-panel" role="tabpanel" aria-labelledby="tab-config">
            <FlywheelSection
              flywheelStatus={flywheelStatus}
              onTest={handleFlywheelTest}
              testing={Boolean(busyActions.flywheelTest)}
            />
          </div>
        )}
      </main>

      {displayNotice && <Notice {...displayNotice} onClose={notice.visible ? hideNotice : null} />}

      <BatchSettingsModal
        open={batchModalOpen}
        type={batchModalType}
        platform={batchModalPlatform}
        mode={batchModalMode}
        commentLimitOptions={commentLimitOptions}
        onConfirm={handleBatchModalConfirm}
        onCancel={handleBatchModalCancel}
      />

      <ConfirmModal
        open={confirmDialog.open}
        title={confirmDialog.title}
        message={confirmDialog.message}
        detail={confirmDialog.detail}
        confirmText={confirmDialog.confirmText}
        confirmTone={confirmDialog.confirmTone}
        onConfirm={() => handleConfirmResolve(true)}
        onCancel={() => handleConfirmResolve(false)}
      />
    </div>
  );
}
