import React, { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import '../extensionPublicPath.js';
import { MSG, COMMENT_DEPTH_MODE } from '../shared/constants.js';
import { BRAND_ASSETS, getBrandAssetUrl } from '../shared/brandAssets.js';
import { initThemeManager, setTheme, getCurrentTheme } from '../themes/themeManager.js';
import {
  PLATFORM, PAGE_MODE,
  detectPlatformByUrl, getModeFromUrl, getPageCapabilities,
  getPrimaryActionWarning, getSecondaryActionWarning, getBatchActionWarning,
  toFriendlyError, formatMaintenanceStats, inferProgressStage,
  sendToTab, sendToBackground, POPUP_SYNC_TO_WORKBENCH_TIMEOUT_MS,
  unwrapTabResponseData,
  mapNoteToFlywheel, mapCommentToFlywheel, mapAuthorToFlywheel,
  getPageContextText,
  isDouyinVideoUrl, isDouyinStrictDetailUrl,
} from './utils.js';
import { formatTaskLeaseIdleNotice } from '../workbench/runtime/taskLeaseClient.js';

import TabNav from './components/TabNav.jsx';
import StatsSection from './components/StatsSection.jsx';
import ActionButtons from './components/ActionButtons.jsx';
import ProgressSection from './components/ProgressSection.jsx';
import PageContextInfo from './components/PageContextInfo.jsx';
import Notice from './components/Notice.jsx';
import FlywheelSection from './components/FlywheelSection.jsx';
import CookieAccountSection from './components/CookieAccountSection.jsx';
import BatchSettingsModal from './components/BatchSettingsModal.jsx';
import AddAccountModal from './components/AddAccountModal.jsx';
import ConfirmModal from './components/ConfirmModal.jsx';

const TABS = [
  { id: 'tab-collect', label: '采集', ariaControls: 'panel-collect' },
  { id: 'tab-data', label: '数据', ariaControls: 'panel-data' },
  { id: 'tab-config', label: '配置', ariaControls: 'panel-config' },
];

const CONTENT_WORKBENCH_PROD_URL = 'https://lingganboom.fun';
const CONTENT_WORKBENCH_LOCAL_URL = 'http://localhost:3000';

const BRAND_BANNER_SRC = getBrandAssetUrl(BRAND_ASSETS.banner);

const TASK_LEASE_STORAGE_KEY = 'workbenchActiveTaskLease';

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

  const [stats, setStats] = useState({ notes: 0, comments: 0, authors: 0 });

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

  const [flywheelUrl, setFlywheelUrl] = useState('');
  const [flywheelStatus, setFlywheelStatus] = useState('unconfigured');
  const [authorizationCode, setAuthorizationCode] = useState('');
  const [stationStatus, setStationStatus] = useState({ registered: false });

  const [cookieStatus, setCookieStatus] = useState({ xhs: null, douyin: null });
  const [accounts, setAccounts] = useState([]);

  const [batchModalOpen, setBatchModalOpen] = useState(false);
  const [batchModalType, setBatchModalType] = useState('notes');
  const [batchModalPlatform, setBatchModalPlatform] = useState(PLATFORM.XHS);
  const [batchModalMode, setBatchModalMode] = useState('single');
  const [commentLimitOptions, setCommentLimitOptions] = useState(null);
  const batchModalResolveRef = useRef(null);

  const [addAccountModalOpen, setAddAccountModalOpen] = useState(false);
  const [removingAccountId, setRemovingAccountId] = useState('');
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

      try {
        const flywheelConfig = await sendToBackground?.(MSG.GET_FLYWHEEL_CONFIG) ?? null;
        if (!flywheelConfig) throw new Error('skip');
        if (flywheelConfig?.serverUrl) {
          setFlywheelUrl(flywheelConfig.serverUrl);
          setFlywheelStatus('configured');
        }
        loadStationStatus();
      } catch {}

      try {
        const storedResult = await sendToBackground(MSG.GET_STORED_PLATFORM_COOKIES);
        if (storedResult?.results) {
          setCookieStatus(storedResult.results);
        }
      } catch {}

      loadAccounts();
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
        showNotice('采集完成。', 'info');
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

  const handleWorkbenchUrlChange = useCallback((nextUrl) => {
    const value = String(nextUrl || '');
    setFlywheelUrl(value);
    setFlywheelStatus(value.trim() ? 'configured' : 'unconfigured');
  }, []);

  const handleUseWorkbenchPreset = useCallback(async (serverUrl) => {
    const value = String(serverUrl || '').trim();
    handleWorkbenchUrlChange(value);
    try {
      await sendToBackground(MSG.SAVE_FLYWHEEL_CONFIG, {
        config: { serverUrl: value, enabled: true },
      });
    } catch {}
  }, [handleWorkbenchUrlChange]);

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
    () => formatTaskLeaseIdleNotice(idleClaimSnapshot),
    [idleClaimSnapshot],
  );
  const displayNotice = notice.visible ? notice : idleClaimNotice;

  const loadStats = useCallback(async (id) => {
    try {
      const response = await sendToTab(id, { action: MSG.GET_STATS });
      const stats = unwrapTabResponseData(response, response) || {};
      if (stats) {
        setStats({
          notes: stats.notes || 0,
          comments: stats.comments || 0,
          authors: stats.authors || 0,
        });
      }
    } catch {}
  }, []);

  const loadAccounts = useCallback(async () => {
    try {
      const response = await sendToBackground('getAccounts');
      setAccounts(response?.accounts || []);
    } catch {
      setAccounts([]);
    }
  }, []);

  const loadStationStatus = useCallback(async () => {
    try {
      const status = await sendToBackground(MSG.GET_EXECUTION_STATION_STATUS);
      setStationStatus(status || { registered: false });
    } catch {
      setStationStatus({ registered: false });
    }
  }, []);

  const requirePluginAuthorization = useCallback(() => {
    if (stationStatus?.authorized) return true;
    setActiveTab('tab-config');
    showNotice(
      stationStatus?.authorizationMessage || '当前浏览器还没有插件授权。可以从内容工作台重新下载安装，或在插件里发起授权申请。',
      'warning',
    );
    return false;
  }, [showNotice, stationStatus]);

  const handleThemeToggle = useCallback(async () => {
    const next = currentTheme === 'ac-ui' ? 'default' : 'ac-ui';
    try { await setTheme(next); } catch {}
    setCurrentThemeState(next);
    document.body.setAttribute('data-theme', next === 'ac-ui' ? 'ac-ui' : '');
  }, [currentTheme]);

  const handleCollectNote = useCallback(async () => {
    if (!requirePluginAuthorization()) return;
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
        await sendToTab(tabId, { action: MSG.COLLECT_SINGLE_NOTE });
      } catch (err) {
        setProgressVisible(false);
        showNotice(toFriendlyError(err), 'warning');
      }
    });
  }, [capabilities, platform, mode, tabId, hideNotice, showNotice, withBusyAction, requirePluginAuthorization]);

  const handleCollectSecondary = useCallback(async () => {
    if (!requirePluginAuthorization()) return;
    if (!capabilities.canCollectSecondary) {
      showNotice(getSecondaryActionWarning(platform, mode, capabilities), 'warning');
      return;
    }
    const isCommentScene = capabilities.secondaryAction === 'comment';
    let payload = { action: isCommentScene ? MSG.COLLECT_SINGLE_COMMENT : MSG.COLLECT_AUTHOR };
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
        action: MSG.COLLECT_SINGLE_COMMENT,
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
      const action = isCommentScene ? MSG.COLLECT_SINGLE_COMMENT : MSG.COLLECT_AUTHOR;
      setProgressStatus(isCommentScene ? '正在发起评论采集...' : '正在发起博主采集...');
      try {
        await sendToTab(tabId, isCommentScene ? payload : { action });
      } catch (err) {
        setProgressVisible(false);
        showNotice(toFriendlyError(err), 'warning');
      }
    });
  }, [capabilities, platform, mode, tabId, hideNotice, showNotice, withBusyAction, requirePluginAuthorization]);

  const handleCommentImages = useCallback(async () => {
    if (!requirePluginAuthorization()) return;
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
      setProgressStatus('正在下载当前视频评论图片区...');
      try {
        const result = await sendToTab(tabId, {
          action: MSG.DOWNLOAD_CURRENT_COMMENT_IMAGES,
          maxTotal: settings.maxTotal,
          maxSubComments: commentDepthMode === COMMENT_DEPTH_MODE.ALL_REPLIES ? 0 : undefined,
          commentDepthMode,
        });
        setProgressVisible(false);
        if (result?.stopped) {
          showNotice(
            result?.downloaded > 0
              ? `评论图片区已停止，已打包 ${result?.downloaded || 0}/${result?.total || 0}，高清 ${result?.hdCount || 0}`
              : (result?.message || '评论图片区下载已停止'),
            'warning',
          );
        } else {
          showNotice(
            `评论图片区下载完成：成功 ${result?.downloaded || 0}/${result?.total || 0}，高清 ${result?.hdCount || 0}`,
            'success',
          );
        }
      } catch (err) {
        setProgressVisible(false);
        showNotice(toFriendlyError(err), 'warning');
      }
    });
  }, [capabilities, tabId, hideNotice, showNotice, withBusyAction, requirePluginAuthorization]);

  const handleBatchNotes = useCallback(async () => {
    if (!requirePluginAuthorization()) return;
    if (!capabilities.canBatchNotes) {
      showNotice(getBatchActionWarning(platform, mode, capabilities), 'warning');
      return;
    }
    const settings = await openBatchSettings('notes', platform);
    if (!settings) return;
    await withBusyAction('batchNotes', async () => {
      hideNotice();
      try {
        await sendToBackground(MSG.START_BATCH_NOTES, {
          tabId,
          mode,
          count: settings.count,
          topByLikes: settings.topByLikes,
          searchFilters: settings.searchFilters,
        });
        setProgressVisible(true);
        setProgressCurrent(0);
        setProgressTotal(settings.count);
        setProgressStatus('批量笔记任务已启动');
        setBatchControlsVisible(true);
        setBatchPaused(false);
        setBatchStopping(false);
        showNotice(`已启动批量笔记：本轮预计采集 ${settings.count} 条。`, 'info');
      } catch (err) {
        setProgressVisible(false);
        setBatchControlsVisible(false);
        showNotice(toFriendlyError(err), 'warning');
      }
    });
  }, [capabilities, platform, mode, tabId, hideNotice, showNotice, withBusyAction, requirePluginAuthorization]);

  const handleBatchComments = useCallback(async () => {
    if (!requirePluginAuthorization()) return;
    if (!capabilities.canBatchComments) {
      showNotice(getBatchActionWarning(platform, mode, capabilities), 'warning');
      return;
    }
    const settings = await openBatchSettings('comments', platform);
    if (!settings) return;
    await withBusyAction('batchComments', async () => {
      hideNotice();
      try {
        await sendToBackground(MSG.START_BATCH_COMMENTS, {
          tabId,
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
        setProgressStatus('批量评论任务已启动');
        setProgressDepthMode(settings.commentDepthMode);
        setBatchControlsVisible(true);
        setBatchPaused(false);
        setBatchStopping(false);
        showNotice(`已启动批量评论：本轮预计处理 ${settings.count} 条内容。`, 'info');
      } catch (err) {
        setProgressVisible(false);
        setBatchControlsVisible(false);
        showNotice(toFriendlyError(err), 'warning');
      }
    });
  }, [capabilities, platform, mode, tabId, hideNotice, showNotice, withBusyAction, requirePluginAuthorization]);

  const handlePause = useCallback(async () => {
    await withBusyAction('pauseBatch', async () => {
      hideNotice();
      try {
        await Promise.all([
          sendToBackground(MSG.PAUSE_BATCH_NOTES, { tabId }),
          sendToBackground(MSG.PAUSE_BATCH_COMMENTS, { tabId }),
        ]);
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
        await Promise.all([
          sendToBackground(MSG.RESUME_BATCH_NOTES, { tabId }),
          sendToBackground(MSG.RESUME_BATCH_COMMENTS, { tabId }),
        ]);
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
        await Promise.all([
          sendToBackground(MSG.STOP_BATCH_NOTES, { tabId }),
          sendToBackground(MSG.STOP_BATCH_COMMENTS, { tabId }),
        ]);
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
    if (!requirePluginAuthorization()) return;
    await withBusyAction('openDashboard', async () => {
      hideNotice();
      try {
        await sendToBackground(MSG.TOGGLE_DASHBOARD, { tabId });
      } catch (err) {
        showNotice(toFriendlyError(err), 'warning');
      }
    });
  }, [tabId, hideNotice, showNotice, withBusyAction, requirePluginAuthorization]);

  const handleExport = useCallback(async () => {
    if (!requirePluginAuthorization()) return;
    await withBusyAction('quickExport', async () => {
      hideNotice();
      try {
        await sendToTab(tabId, { action: MSG.EXPORT_JSON });
        showNotice('导出任务已发起。', 'success');
      } catch (err) {
        showNotice(toFriendlyError(err), 'warning');
      }
    });
  }, [tabId, hideNotice, showNotice, withBusyAction, requirePluginAuthorization]);

  const handleMaintenance = useCallback(async () => {
    if (!requirePluginAuthorization()) return;
    if (platform === PLATFORM.UNKNOWN) {
      showNotice('请先打开小红书或抖音页面，再执行数据维护。', 'warning');
      return;
    }
    await withBusyAction('maintenance', async () => {
      hideNotice();
      setProgressVisible(true);
      setProgressCurrent(0);
      setProgressTotal(1);
      setProgressStatus('正在整理历史数据...');
      try {
        const response = await sendToTab(tabId, { action: MSG.RUN_DATA_MAINTENANCE });
        setProgressVisible(false);
        const mStats = unwrapTabResponseData(response, response?.stats || {}) || {};
        showNotice(formatMaintenanceStats(mStats), 'success');
        loadStats(tabId);
      } catch (err) {
        setProgressVisible(false);
        showNotice(toFriendlyError(err), 'warning');
      }
    });
  }, [platform, tabId, hideNotice, showNotice, loadStats, withBusyAction, requirePluginAuthorization]);

  const handlePluginAuthorize = useCallback(async () => {
    const serverUrl = flywheelUrl.trim();
    const code = authorizationCode.trim();
    if (!serverUrl) {
      showNotice('请先配置工作台地址。', 'warning');
      return;
    }
    if (!code) {
      showNotice('请输入内容工作台设置里生成的授权码。', 'warning');
      return;
    }
    await withBusyAction('pluginAuthorize', async () => {
      hideNotice();
      try {
        const result = await sendToBackground(MSG.AUTHORIZE_PLUGIN_ACCESS, {
          serverUrl,
          authorizationCode: code,
          browserLabel: navigator.userAgent || '',
        });
        if (!result?.success) {
          throw new Error(result?.error || '授权失败');
        }
        setAuthorizationCode('');
        await loadStationStatus();
        showNotice('插件已连接，工位也已自动准备好。', 'success');
      } catch (err) {
        showNotice(`授权失败：${toFriendlyError(err)}`, 'warning');
      }
    });
  }, [authorizationCode, flywheelUrl, hideNotice, loadStationStatus, showNotice, withBusyAction]);

  const handlePluginAuthorizationRequest = useCallback(async () => {
    const serverUrl = flywheelUrl.trim();
    if (!serverUrl) {
      showNotice('请先配置工作台地址。', 'warning');
      return;
    }
    await withBusyAction('requestPluginAuthorization', async () => {
      hideNotice();
      try {
        const result = await sendToBackground(MSG.REQUEST_PLUGIN_AUTHORIZATION, {
          serverUrl,
          browserLabel: navigator.userAgent || '',
        });
        await loadStationStatus();
        showNotice(result?.message || '授权申请已发送，请等待内容工作台审批。', 'success');
      } catch (err) {
        showNotice(`申请失败：${toFriendlyError(err)}`, 'warning');
      }
    });
  }, [flywheelUrl, hideNotice, loadStationStatus, showNotice, withBusyAction]);

  const handlePluginAuthorizationClaim = useCallback(async () => {
    const serverUrl = flywheelUrl.trim();
    if (!serverUrl) {
      showNotice('请先配置工作台地址。', 'warning');
      return;
    }
    await withBusyAction('claimPluginAuthorization', async () => {
      hideNotice();
      try {
        const result = await sendToBackground(MSG.CLAIM_PLUGIN_AUTHORIZATION_REQUEST, {
          serverUrl,
          browserLabel: navigator.userAgent || '',
        });
        await loadStationStatus();
        if (result?.authorized) {
          showNotice('授权已生效，工位也已自动准备好。', 'success');
          return;
        }
        showNotice(result?.message || '申请还在等待工作台审批。', 'info');
      } catch (err) {
        showNotice(`检查失败：${toFriendlyError(err)}`, 'warning');
      }
    });
  }, [flywheelUrl, hideNotice, loadStationStatus, showNotice, withBusyAction]);

  const handleClearPluginAuthorization = useCallback(async () => {
    const confirmed = await showConfirmDialog({
      title: '清除插件授权',
      message: '清除后，这个浏览器将失去插件使用资格，并解除当前工位绑定。',
      detail: '如果只是临时停止接单，请在内容工作台关闭这个工位的接单开关。',
      confirmText: '确认清除',
      confirmTone: 'danger',
    });
    if (!confirmed) return;
    await withBusyAction('clearPluginAuthorization', async () => {
      hideNotice();
      try {
        await sendToBackground(MSG.CLEAR_PLUGIN_AUTHORIZATION);
        setAuthorizationCode('');
        await loadStationStatus();
        showNotice('插件授权已清除。', 'warning');
      } catch (err) {
        showNotice(`清除失败：${toFriendlyError(err)}`, 'warning');
      }
    });
  }, [hideNotice, loadStationStatus, showConfirmDialog, showNotice, withBusyAction]);

  const handleFlywheelTest = useCallback(async () => {
    const serverUrl = flywheelUrl.trim();
    if (!serverUrl) {
      showNotice('请输入工作台地址。', 'warning');
      return;
    }
    await withBusyAction('flywheelTest', async () => {
      hideNotice();
      setFlywheelStatus('testing');
      try {
        const url = serverUrl.replace(/\/+$/, '').replace(/^(?!https?:\/\/)/, 'http://');
        const resp = await fetch(`${url}/api/collect/status`, { signal: AbortSignal.timeout(5000) });
        if (resp.ok) {
          setFlywheelStatus('connected');
          await sendToBackground(MSG.SAVE_FLYWHEEL_CONFIG, { config: { serverUrl, enabled: true } });
          showNotice('连接成功！内容工作台已就绪。', 'success');
        } else {
          setFlywheelStatus('disconnected');
          showNotice(`连接失败：服务器返回 ${resp.status}`, 'warning');
        }
      } catch (err) {
        setFlywheelStatus('disconnected');
        showNotice(`连接失败：${err.message || '无法连接'}`, 'warning');
      }
    });
  }, [flywheelUrl, hideNotice, showNotice, withBusyAction]);

  const handleSyncToFlywheel = useCallback(async () => {
    if (!requirePluginAuthorization()) return;
    const serverUrl = flywheelUrl.trim();
    if (!serverUrl) {
      showNotice('请先配置工作台地址。', 'warning');
      return;
    }
    if (platform === PLATFORM.UNKNOWN) {
      showNotice('请先打开小红书或抖音页面，再同步采集数据。', 'warning');
      return;
    }
    await withBusyAction('syncFlywheel', async () => {
      hideNotice();
      await sendToBackground(MSG.SAVE_FLYWHEEL_CONFIG, { config: { serverUrl, enabled: true } });
      setProgressVisible(true);
      setProgressCurrent(0);
      setProgressTotal(1);
      setProgressStatus('正在读取本地数据...');
      try {
        const dataTabQuery = platform === PLATFORM.DOUYIN ? '*://*.douyin.com/*' : '*://*.xiaohongshu.com/*';
        const platformText = platform === PLATFORM.DOUYIN ? '抖音' : '小红书';
        const dataTabs = await chrome.tabs.query({ url: dataTabQuery });
        const dataTabId = dataTabs.find(t => t.id === tabId)?.id || dataTabs[0]?.id;
        if (!dataTabId) {
          setProgressVisible(false);
          showNotice(`请先打开${platformText}页面，插件需要通过页面读取采集数据。`, 'warning');
          return;
        }

        const [notesResp, commentsResp, authorsResp] = await Promise.all([
          sendToTab(dataTabId, { action: MSG.GET_ALL_NOTES }),
          sendToTab(dataTabId, { action: MSG.GET_ALL_COMMENTS }),
          sendToTab(dataTabId, { action: MSG.GET_ALL_AUTHORS }),
        ]);

        const notes = Array.isArray(notesResp) ? notesResp : (notesResp?.data || []);
        const comments = Array.isArray(commentsResp) ? commentsResp : (commentsResp?.data || []);
        const authors = Array.isArray(authorsResp) ? authorsResp : (authorsResp?.data || []);

        if (!notes || notes.length === 0) {
          setProgressVisible(false);
          showNotice(`没有可同步的数据。请先在${platformText}页面采集内容。`, 'warning');
          return;
        }

        setProgressStatus(`正在发送 ${notes.length} 条${platformText}内容到飞轮...`);

        const result = await sendToBackground(MSG.SYNC_TO_WORKBENCH, {
          notes: notes.map(mapNoteToFlywheel),
          comments: (comments || []).map(mapCommentToFlywheel),
          authors: (authors || []).map(mapAuthorToFlywheel),
        }, {
          timeoutMs: POPUP_SYNC_TO_WORKBENCH_TIMEOUT_MS,
        });
        setProgressVisible(false);
        if (result?.success) {
          const m = result.meta || {};
          const imported = result.imported || 0;
          const skipped = result.skipped || 0;
          showNotice(`发送完成：导入 ${imported} 条，跳过 ${skipped} 条（笔记 ${m.notesReceived || 0}，评论 ${m.commentsReceived || 0}，博主 ${m.authorsReceived || 0}）`, imported > 0 ? 'success' : 'warning');
        } else {
          showNotice(`发送失败：${result?.error || '工作台拒绝了这次同步'}`, 'warning');
        }
      } catch (err) {
        setProgressVisible(false);
        showNotice(`发送失败：${err.message || '网络错误'}`, 'warning');
      }
    });
  }, [flywheelUrl, tabId, platform, hideNotice, showNotice, withBusyAction, requirePluginAuthorization]);

  const handleGetCookies = useCallback(async () => {
    if (!requirePluginAuthorization()) return;
    if (platform === PLATFORM.UNKNOWN) {
      showNotice('请先打开小红书或抖音页面，再抓取当前平台 Cookie。', 'warning');
      return;
    }
    await withBusyAction('getCookies', async () => {
      hideNotice();
      setProgressVisible(true);
      setProgressCurrent(0);
      setProgressTotal(1);
      const platformText = platform === PLATFORM.DOUYIN ? '抖音' : '小红书';
      setProgressStatus(`正在获取${platformText} Cookie...`);
      try {
        const result = await sendToBackground(MSG.GET_PLATFORM_COOKIES, { platform });
        setProgressVisible(false);
        setCookieStatus(result.results || {});
        if (result.success) {
          const xhs = result.results?.xhs;
          const dy = result.results?.douyin;

          if (platform === PLATFORM.XHS && xhs?.count > 0) {
            const name = `小红书-${new Date().toLocaleDateString('zh-CN')} ${new Date().toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })}`;
            await sendToBackground('addAccount', {
              name,
              cookieJson: JSON.stringify(xhs.cookies),
              platform: 'xhs',
              dailyQuotaLimit: 100,
            });
            loadAccounts();
          }

          if (platform === PLATFORM.DOUYIN && dy?.count > 0) {
            const name = `抖音-${new Date().toLocaleDateString('zh-CN')} ${new Date().toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })}`;
            await sendToBackground('addAccount', {
              name,
              cookieJson: JSON.stringify(dy.cookies),
              platform: 'douyin',
              dailyQuotaLimit: 100,
            });
            loadAccounts();
          }

          const currentResult = platform === PLATFORM.DOUYIN ? dy : xhs;
          const currentLabel = platform === PLATFORM.DOUYIN ? '抖音' : '小红书';
          const accountNote = platform === PLATFORM.XHS && xhs?.count > 0
            ? '，小红书 Cookie 已自动保存为采集账号。'
            : platform === PLATFORM.DOUYIN && dy?.count > 0
              ? '，抖音 Cookie 已自动保存为采集账号。'
              : '';
          showNotice(`获取成功：${currentLabel} ${currentResult?.count || 0} 条${accountNote}`, 'success');
        } else {
          showNotice(`获取 Cookie 失败，请确认当前${platformText}页面已登录。`, 'warning');
        }
      } catch (err) {
        setProgressVisible(false);
        showNotice(toFriendlyError(err), 'warning');
      }
    });
  }, [platform, hideNotice, showNotice, loadAccounts, withBusyAction, requirePluginAuthorization]);

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

  const handleAddAccount = useCallback(async (accountData) => {
    if (!requirePluginAuthorization()) {
      return { success: false, error: stationStatus?.authorizationMessage || 'plugin_authorization_required' };
    }
    try {
      const response = await sendToBackground('addAccount', accountData);
      if (response?.success) {
        loadAccounts();
        showNotice('采集账号已保存。', 'success');
        return { success: true };
      }
      return { success: false, error: response?.error || '添加失败' };
    } catch (err) {
      return { success: false, error: toFriendlyError(err) };
    }
  }, [loadAccounts, showNotice, requirePluginAuthorization, stationStatus]);

  const handleOpenAddAccount = useCallback(async () => {
    if (!requirePluginAuthorization()) return;
    await withBusyAction('openAddAccount', async () => {
      setAddAccountModalOpen(true);
    });
  }, [withBusyAction, requirePluginAuthorization]);

  const handleRemoveAccount = useCallback(async (accountId) => {
    const target = accounts.find((item) => item.accountId === accountId);
    const confirmed = await showConfirmDialog({
      title: '确认删除采集账号',
      message: `删除后，这个账号不会再参与执行或监控。`,
      detail: target?.name ? `将删除账号：${target.name}` : '删除后不可恢复，需要重新提取或手动添加。',
      confirmText: '确认删除',
      confirmTone: 'danger',
    });
    if (!confirmed) return;
    setRemovingAccountId(accountId);
    try {
      await sendToBackground('removeAccount', { accountId });
      loadAccounts();
      showNotice('采集账号已删除。', 'success');
    } catch (err) {
      showNotice(toFriendlyError(err), 'warning');
    } finally {
      setRemovingAccountId('');
    }
  }, [accounts, loadAccounts, showConfirmDialog, showNotice]);

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
                {busyActions.openDashboard ? '打开中...' : '打开工作台'}
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
            <CookieAccountSection
              currentPlatform={platform}
              cookieStatus={cookieStatus}
              accounts={accounts}
              onGetCookies={handleGetCookies}
              onOpenAddAccount={handleOpenAddAccount}
              onRemoveAccount={handleRemoveAccount}
              gettingCookies={Boolean(busyActions.getCookies)}
              openingAddAccount={Boolean(busyActions.openAddAccount)}
              removingAccountId={removingAccountId}
            />
          </div>
        )}

        {activeTab === 'tab-config' && (
          <div id="panel-config" className="tab-panel" role="tabpanel" aria-labelledby="tab-config">
            <FlywheelSection
              flywheelUrl={flywheelUrl}
              flywheelStatus={flywheelStatus}
              authorizationCode={authorizationCode}
              authorizationStatus={stationStatus}
              stationStatus={stationStatus}
              onUrlChange={handleWorkbenchUrlChange}
              onUsePresetUrl={handleUseWorkbenchPreset}
              onAuthorizationCodeChange={setAuthorizationCode}
              onAuthorize={handlePluginAuthorize}
              onRequestAuthorization={handlePluginAuthorizationRequest}
              onClaimAuthorization={handlePluginAuthorizationClaim}
              onClearAuthorization={handleClearPluginAuthorization}
              onTest={handleFlywheelTest}
              onSync={handleSyncToFlywheel}
              testing={Boolean(busyActions.flywheelTest)}
              authorizing={Boolean(busyActions.pluginAuthorize)}
              requestingAuthorization={Boolean(busyActions.requestPluginAuthorization)}
              claimingAuthorization={Boolean(busyActions.claimPluginAuthorization)}
              clearingAuthorization={Boolean(busyActions.clearPluginAuthorization)}
              syncing={Boolean(busyActions.syncFlywheel)}
              presetUrls={{
                production: CONTENT_WORKBENCH_PROD_URL,
                local: CONTENT_WORKBENCH_LOCAL_URL,
              }}
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

      <AddAccountModal
        open={addAccountModalOpen}
        onClose={() => setAddAccountModalOpen(false)}
        onConfirm={handleAddAccount}
        currentPlatform={platform}
        onExtractCookie={async () => {
          try {
            const result = await sendToBackground(MSG.GET_PLATFORM_COOKIES, { platform });
            const current = platform === PLATFORM.DOUYIN ? result?.results?.douyin : result?.results?.xhs;
            const currentLabel = platform === PLATFORM.DOUYIN ? '抖音' : '小红书';
            return {
              success: Number(current?.count || 0) > 0,
              cookies: Number(current?.count || 0) > 0 ? current.cookies : null,
              allResults: result?.results,
              error: result?.success ? null : `未检测到${currentLabel} Cookie`,
            };
          } catch (err) {
            return { success: false, error: err.message };
          }
        }}
        onCookieResult={setCookieStatus}
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
