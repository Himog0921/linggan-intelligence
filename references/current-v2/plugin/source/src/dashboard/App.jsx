import React, { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import '../extensionPublicPath.js';
import { MSG } from '../shared/constants.js';
import { BRAND_ASSETS, getBrandAssetUrl } from '../shared/brandAssets.js';
import { icon } from '../shared/icons.js';
import { getFeedbackMeta, getMediaStatusMeta } from '../shared/feedback.js';
import { generateCsv, downloadFile } from '../shared/utils.js';
import {
  extractHashtags, stripHashtags, cleanDisplayBodyText, getHashtagsForItem,
  formatReplyTargetLabel, formatCollectionRunLabel, formatBatchSelectionModeLabel,
  formatDataQualityLabel, formatQualityReasonLabel, formatSourceTierLabel,
  truncate, escapeHtml, debounce,
  sortByCreatedAt, formatLocalDate, normalizeUrl, toDisplayUrl, getPreferredRecordUrl, getUnifiedAuthorHandle,
  DASHBOARD_SYNC_TO_WORKBENCH_TIMEOUT_MS,
  buildWorkbenchSyncPayload, summarizeWorkbenchSyncResult, getItemId, getTabLabel, getColumns, getExportColumns, sendToParent, unwrapParentResponseData,
} from './utils.js';
import { formatTaskLeaseIdleNotice } from '../workbench/runtime/taskLeaseClient.js';

const TABS = [
  { key: 'notes', label: '笔记' },
  { key: 'comments', label: '评论' },
  { key: 'authors', label: '博主' },
];

const DASHBOARD_LOAD_CHUNK_SIZE = 200;
const PAGE_SIZE_OPTIONS = [50, 200, 500];
const LINK_ACTION_TEXT = {
  url: '打开',
  noteUrl: '打开',
  profileUrl: '打开',
  avatarUrl: '查看',
};

const BRAND_BANNER_SRC = getBrandAssetUrl(BRAND_ASSETS.banner);

function hasMediaValue(value) {
  if (Array.isArray(value)) return value.some(hasMediaValue);
  if (value && typeof value === 'object') {
    return [
      value.urlDefault,
      value.url_default,
      value.url,
      value.src,
      value.href,
      value.masterUrl,
      value.master_url,
      value.candidates,
      value.streams,
    ].some(hasMediaValue);
  }
  return Boolean(String(value || '').trim());
}

function getMediaDownloadOptions(item = {}) {
  const imageGroups = Array.isArray(item.imageCandidates) && item.imageCandidates.length > 0
    ? item.imageCandidates
    : (Array.isArray(item.images) ? item.images : []);
  const imageCount = imageGroups.filter(hasMediaValue).length;
  const liveCount = Array.isArray(item.livePhotoStreams) ? item.livePhotoStreams.filter(hasMediaValue).length : 0;
  const hasVideo = hasMediaValue([item.videoStreams, item.videoDownloadUrl, item.videoPlayUrl, item.video]);
  const isVideo = String(item.type || '').trim() === 'video' || hasVideo;
  const hasCover = hasMediaValue([
    item.cover,
    item.coverImg,
    item.coverUrl,
    item.thumbnail,
    Array.isArray(item.images) ? item.images[0] : '',
    Array.isArray(item.imageCandidates) ? item.imageCandidates[0] : '',
  ]);
  const options = [];
  if (hasCover) options.push({ value: 'cover', label: '下载封面', count: 1 });
  if (imageCount > 0 && (!isVideo || imageCount > 1)) {
    options.push({ value: 'images', label: '下载所有图片', count: imageCount });
  }
  if (liveCount > 0) options.push({ value: 'live', label: '下载 Live', count: liveCount });
  if (hasVideo) options.push({ value: 'video', label: '下载视频', count: 1 });
  return options;
}

function countSelectedMedia(options = [], selectedValues = []) {
  const selected = new Set(selectedValues);
  const hasImages = selected.has('images');
  return options
    .filter((item) => !(item.value === 'cover' && hasImages))
    .filter((item) => selected.has(item.value))
    .reduce((total, item) => total + Number(item.count || 0), 0);
}

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

function renderLinkAction(url, actionText = '打开') {
  const fullUrl = normalizeUrl(url);
  if (!fullUrl) return '-';
  return (
    <a
      href={fullUrl}
      target="_blank"
      rel="noopener noreferrer"
      className="table-link-action"
      title={fullUrl}
      aria-label={`${actionText}：${fullUrl}`}
    >
      <span className="table-link-action-icon" dangerouslySetInnerHTML={{ __html: icon('arrowDown', { size: 12 }) }} />
      <span className="table-link-action-text">{actionText}</span>
    </a>
  );
}

export default function App() {
  const [currentTab, setCurrentTab] = useState('notes');
  const [imgErrorIds, setImgErrorIds] = useState(new Set());
  const [allData, setAllData] = useState([]);
  const [searchKeyword, setSearchKeyword] = useState('');
  const [filterType, setFilterType] = useState('all');
  const [sortByTime, setSortByTime] = useState('desc');
  const [filterDate, setFilterDate] = useState('');
  const [currentPage, setCurrentPage] = useState(1);
  const [selectedIds, setSelectedIds] = useState({ notes: new Set(), comments: new Set(), authors: new Set() });
  const [notice, setNotice] = useState({ message: '', type: 'info', visible: false });
  const [confirmDialog, setConfirmDialog] = useState({
    open: false,
    title: '',
    message: '',
    detail: '',
    confirmText: '',
    confirmTone: 'danger',
    onConfirm: null,
  });
  const [mediaDownloadDialog, setMediaDownloadDialog] = useState({
    open: false,
    title: '',
    options: [],
    selected: [],
    onConfirm: null,
  });
  const [mediaPreview, setMediaPreview] = useState({ open: false, url: '', title: '', type: 'image' });
  const [loading, setLoading] = useState(false);
  const [idleClaimSnapshot, setIdleClaimSnapshot] = useState(null);
  const [busyActions, setBusyActions] = useState({});
  const [rowBusyActions, setRowBusyActions] = useState({});
  const [pageSize, setPageSize] = useState(() => {
    try {
      const v = Number(localStorage.getItem('lgboom_page_size'));
      return PAGE_SIZE_OPTIONS.includes(v) ? v : 50;
    } catch { return 50; }
  });

  const tableWrapperRef = useRef(null);
  const noticeTimerRef = useRef(null);
  const busyActionsRef = useRef({});
  const rowBusyActionsRef = useRef({});
  const loadGenRef = useRef(0);

  // ===== 加载数据 =====
  const loadData = useCallback(async (tab) => {
    const targetTab = tab || currentTab;
    const gen = ++loadGenRef.current;
    setAllData([]);
    setSelectedIds((prev) => ({ ...prev, [targetTab]: new Set() }));
    setLoading(true);
    try {
      const actionMap = { notes: MSG.GET_ALL_NOTES, comments: MSG.GET_ALL_COMMENTS, authors: MSG.GET_ALL_AUTHORS };
      const action = actionMap[targetTab];
      let offset = 0;
      let hasMore = true;
      let accumulated = [];
      while (hasMore) {
        if (gen !== loadGenRef.current) return;
        const response = await sendToParent(action, {
          offset,
          limit: DASHBOARD_LOAD_CHUNK_SIZE,
        }, { timeoutMs: 30000 });
        const data = unwrapParentResponseData(response, []) || [];
        accumulated = accumulated.concat(data);
        if (gen !== loadGenRef.current) return;
        setAllData(sortByCreatedAt(accumulated, sortByTime));
        offset += data.length;
        hasMore = Boolean(response?.hasMore) && data.length > 0;
      }
    } catch (e) {
      if (gen === loadGenRef.current) setAllData([]);
    } finally {
      if (gen === loadGenRef.current) setLoading(false);
    }
  }, [currentTab, sortByTime]);

  useEffect(() => {
    loadData(currentTab);
  }, [currentTab, loadData]);

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

  // ===== 筛选数据 =====
  const filteredData = useMemo(() => {
    const keyword = searchKeyword.trim().toLowerCase();
    let result = allData.filter((item) => {
      if (keyword) {
        const searchable = Object.values(item).map((v) => String(v || '').toLowerCase()).join(' ');
        if (!searchable.includes(keyword)) return false;
      }
      if (currentTab === 'notes' && filterType !== 'all') {
        if (item.type !== filterType) return false;
      }
      if (filterDate) {
        const itemDate = formatLocalDate(item.createdAt);
        if (!itemDate || itemDate !== filterDate) return false;
      }
      return true;
    });
    return sortByCreatedAt(result, sortByTime);
  }, [allData, searchKeyword, filterType, filterDate, sortByTime, currentTab]);

  // ===== 分页 =====
  const totalCount = filteredData.length;
  const totalPages = Math.max(1, Math.ceil(totalCount / pageSize));
  const safePage = Math.min(currentPage, totalPages);
  const pageData = useMemo(() => {
    const page = Math.min(currentPage, totalPages);
    const start = (page - 1) * pageSize;
    return filteredData.slice(start, start + pageSize);
  }, [filteredData, currentPage, totalPages, pageSize]);

  const handlePageSizeChange = useCallback((e) => {
    const v = Number(e.target.value);
    setPageSize(v);
    setCurrentPage(1);
    try { localStorage.setItem('lgboom_page_size', String(v)); } catch {}
  }, []);

  useEffect(() => {
    if (currentPage > totalPages && totalPages > 0) {
      setCurrentPage(totalPages);
    }
  }, [totalPages, currentPage]);

  // ===== 选中管理 =====
  const currentSelected = selectedIds[currentTab];
  const setCurrentSelected = useCallback((newSet) => {
    setSelectedIds((prev) => ({ ...prev, [currentTab]: newSet }));
  }, [currentTab]);

  const selectedCount = useMemo(() => {
    return allData.filter((item) => currentSelected.has(getItemId(item, currentTab))).length;
  }, [allData, currentSelected, currentTab]);

  const toggleSelect = useCallback((id) => {
    const next = new Set(currentSelected);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    setCurrentSelected(next);
  }, [currentSelected, setCurrentSelected]);

  const toggleSelectAll = useCallback(() => {
    const pageIds = pageData.map((item) => getItemId(item, currentTab)).filter(Boolean);
    const allSelected = pageIds.length > 0 && pageIds.every((id) => currentSelected.has(id));
    const next = new Set(currentSelected);
    if (allSelected) {
      pageIds.forEach((id) => next.delete(id));
    } else {
      pageIds.forEach((id) => next.add(id));
    }
    setCurrentSelected(next);
  }, [pageData, currentSelected, currentTab, setCurrentSelected]);

  const someSelected = pageData.some((item) => currentSelected.has(getItemId(item, currentTab)));
  const allSelected = pageData.length > 0 && pageData.every((item) => currentSelected.has(getItemId(item, currentTab)));

  // ===== 通知 =====
  const showNotice = useCallback((message, type = 'info') => {
    if (noticeTimerRef.current) clearTimeout(noticeTimerRef.current);
    setNotice({ message, type, visible: true });
    noticeTimerRef.current = setTimeout(() => {
      setNotice((n) => ({ ...n, visible: false }));
      noticeTimerRef.current = null;
    }, type === 'error' ? 5000 : 3600);
  }, []);

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

  const setRowBusyActionState = useCallback((key, busy) => {
    const next = { ...rowBusyActionsRef.current };
    if (busy) next[key] = true;
    else delete next[key];
    rowBusyActionsRef.current = next;
    setRowBusyActions(next);
  }, []);

  // ===== 确认弹窗 =====
  const showConfirm = useCallback(({ title, message, detail = '', confirmText, confirmTone = 'danger' }) => {
    return new Promise((resolve) => {
      setConfirmDialog({ open: true, title, message, detail, confirmText, confirmTone, onConfirm: resolve });
    });
  }, []);

  const handleConfirm = useCallback((result) => {
    if (confirmDialog.onConfirm) confirmDialog.onConfirm(result);
    setConfirmDialog((d) => ({
      ...d,
      open: false,
      detail: '',
      confirmTone: 'danger',
      onConfirm: null,
    }));
  }, [confirmDialog]);

  const showMediaDownloadDialog = useCallback((item) => new Promise((resolve) => {
    const options = getMediaDownloadOptions(item);
    if (options.length === 0) {
      resolve(null);
      return;
    }
    setMediaDownloadDialog({
      open: true,
      title: item?.title || item?.noteId || '媒体下载',
      options,
      selected: options.map((option) => option.value),
      onConfirm: resolve,
    });
  }), []);

  const toggleMediaDownloadType = useCallback((value) => {
    setMediaDownloadDialog((dialog) => {
      const selected = new Set(dialog.selected);
      if (selected.has(value)) selected.delete(value);
      else selected.add(value);
      return {
        ...dialog,
        selected: Array.from(selected),
      };
    });
  }, []);

  const handleMediaDownloadDialog = useCallback((confirmed) => {
    const resolver = mediaDownloadDialog.onConfirm;
    const selected = mediaDownloadDialog.selected;
    const options = mediaDownloadDialog.options;
    setMediaDownloadDialog({
      open: false,
      title: '',
      options: [],
      selected: [],
      onConfirm: null,
    });
    if (!resolver) return;
    if (!confirmed || selected.length === 0) {
      resolver(null);
      return;
    }
    resolver({
      mediaTypes: selected,
      count: countSelectedMedia(options, selected),
    });
  }, [mediaDownloadDialog]);

  const idleClaimNotice = useMemo(
    () => formatTaskLeaseIdleNotice(idleClaimSnapshot),
    [idleClaimSnapshot],
  );
  const displayNotice = notice.visible ? notice : idleClaimNotice;

  // ===== 导出 =====
  const handleExportCsv = useCallback(() => {
    withBusyAction('exportCsv', async () => {
      if (filteredData.length === 0) {
        showNotice('没有数据可导出。', 'warning');
        return;
      }
      const columns = getExportColumns(currentTab, allData);
      const headers = columns.map((c) => c.label);
      const rows = filteredData.map((item) => columns.map((c) => {
        let val = item[c.key];
        if (c.key === 'type') return val === 'video' ? '视频' : '图文';
        if (c.key === 'title') return stripHashtags(String(val || ''));
        if (c.key === 'content') return cleanDisplayBodyText(String(val || ''));
        if (c.key === 'hashtags') return getHashtagsForItem(item).join('\n');
        if (c.key === 'batchSelectionMode') return formatBatchSelectionModeLabel(val);
        if (c.key === 'dataQuality') return formatDataQualityLabel(val);
        if (c.key === 'qualityReason') return formatQualityReasonLabel(val);
        if (c.key === 'sourceTier') return formatSourceTierLabel(val);
        if (c.key === 'authorFollowed' || c.key === 'shareRestricted' || c.key === 'followedByMe') return val ? '是' : '否';
        if (c.key === 'handle') return getUnifiedAuthorHandle(item);
        if (c.key === 'images') return Array.isArray(val) ? val.join('\n') : '';
        if (c.key === 'atUserList') return Array.isArray(val) ? val.map((v) => `${v.nickname || ''}(${v.userId || ''})`).join('\n') : '';
        if (c.key === 'topicIds') return Array.isArray(val) ? val.join('\n') : '';
        if (c.key === 'url' || c.key === 'noteUrl' || c.key === 'profileUrl') return toDisplayUrl(getPreferredRecordUrl(item, c.key) || val);
        if (c.key === 'createdAt') return val ? new Date(val).toLocaleString('zh-CN') : '';
        return String(val ?? '');
      }));
      const csv = generateCsv(headers, rows);
      const filename = `灵感爆爆爆_${getTabLabel(currentTab)}_${new Date().toISOString().slice(0, 10)}`;
      downloadFile(csv, filename + '.csv', 'text/csv;charset=utf-8;');
      showNotice(`已导出 ${filteredData.length} 条${getTabLabel(currentTab)}：${filename}.csv`, 'success');
    });
  }, [filteredData, currentTab, allData, showNotice, withBusyAction]);

  const handleExportJson = useCallback(() => {
    withBusyAction('exportJson', async () => {
      if (filteredData.length === 0) {
        showNotice('没有数据可导出。', 'warning');
        return;
      }
      const json = JSON.stringify(filteredData, null, 2);
      const filename = `灵感爆爆爆_${getTabLabel(currentTab)}_${new Date().toISOString().slice(0, 10)}`;
      downloadFile(json, filename + '.json', 'application/json;charset=utf-8;');
      showNotice(`已导出 ${filteredData.length} 条${getTabLabel(currentTab)}：${filename}.json`, 'success');
    });
  }, [filteredData, currentTab, showNotice, withBusyAction]);

  const handleExportSelected = useCallback(() => {
    withBusyAction('exportSelected', async () => {
      if (currentSelected.size === 0) {
        showNotice(`请先勾选要导出的${getTabLabel(currentTab)}。`, 'warning');
        return;
      }
      const selectedItems = allData.filter((item) => currentSelected.has(getItemId(item, currentTab)));
      if (selectedItems.length === 0) {
        showNotice(`没有选中的${getTabLabel(currentTab)}可导出。`, 'warning');
        return;
      }
      const json = JSON.stringify(selectedItems, null, 2);
      const filename = `灵感爆爆爆_已选${getTabLabel(currentTab)}_${new Date().toISOString().slice(0, 10)}`;
      downloadFile(json, filename + '.json', 'application/json;charset=utf-8;');
      showNotice(`已导出 ${selectedItems.length} 条选中${getTabLabel(currentTab)}：${filename}.json`, 'success');
    });
  }, [currentSelected, allData, currentTab, showNotice, withBusyAction]);

  // ===== 删除 =====
  const handleDeleteSelected = useCallback(async () => {
    const selectedItems = allData.filter((item) => currentSelected.has(getItemId(item, currentTab)));
    if (selectedItems.length === 0) {
      showNotice(`请先勾选要删除的${getTabLabel(currentTab)}。`, 'warning');
      return;
    }
    const confirmed = await showConfirm({
      title: `确认删除选中${getTabLabel(currentTab)}`,
      message: `确定要删除已选中的 ${selectedItems.length} 条${getTabLabel(currentTab)}吗？此操作不可恢复。`,
      detail: '删除后这些记录会从插件本地数据库中移除，后续导出和同步都不会再出现。',
      confirmText: '删除选中',
    });
    if (!confirmed) return;
    await withBusyAction('deleteSelected', async () => {
      try {
        const deleteActionMap = { notes: MSG.DELETE_NOTE, comments: MSG.DELETE_COMMENT, authors: MSG.DELETE_AUTHOR };
        const deleteAction = deleteActionMap[currentTab];
        for (const item of selectedItems) {
          const idKey = currentTab === 'notes' ? 'noteId' : currentTab === 'comments' ? 'id' : 'userId';
          await sendToParent(deleteAction, { [idKey]: getItemId(item, currentTab) });
        }
        const next = new Set(currentSelected);
        selectedItems.forEach((item) => next.delete(getItemId(item, currentTab)));
        setCurrentSelected(next);
        showNotice(`已删除 ${selectedItems.length} 条选中${getTabLabel(currentTab)}。`, 'success');
        loadData(currentTab);
      } catch (error) {
        showNotice(`删除失败：${error.message || '未知错误'}`, 'error');
      }
    });
  }, [currentSelected, allData, currentTab, showNotice, showConfirm, loadData, setCurrentSelected, withBusyAction]);

  // ===== 清空 =====
  const handleClearAll = useCallback(async () => {
    const confirmed = await showConfirm({
      title: '确认清空数据',
      message: `确定要清空所有${getTabLabel(currentTab)}数据吗？此操作不可恢复。`,
      detail: '建议先导出一份 JSON 或 CSV 备份，再执行清空。',
      confirmText: '确认清空',
    });
    if (!confirmed) return;
    await withBusyAction('clearAll', async () => {
      try {
        const actionMap = { notes: MSG.CLEAR_ALL_NOTES, comments: MSG.CLEAR_ALL_COMMENTS, authors: MSG.CLEAR_ALL_AUTHORS };
        await sendToParent(actionMap[currentTab]);
        showNotice(`已清空当前${getTabLabel(currentTab)}数据。`, 'warning');
        loadData(currentTab);
      } catch (error) {
        showNotice(`清空失败：${error.message || '未知错误'}`, 'error');
      }
    });
  }, [currentTab, showNotice, showConfirm, loadData, withBusyAction]);

  // ===== 同步到工作台 =====
  const handleSyncToWorkbench = useCallback(async () => {
    const selectedItems = allData.filter((item) => currentSelected.has(getItemId(item, currentTab)));
    if (selectedItems.length === 0) {
      showNotice(`请先勾选要同步到工作台的${getTabLabel(currentTab)}。`, 'warning');
      return;
    }
    await withBusyAction('syncWorkbench', async () => {
      try {
        showNotice(`正在同步 ${selectedItems.length} 条${getTabLabel(currentTab)}到工作台...`, 'info');
        const payload = buildWorkbenchSyncPayload(currentTab, selectedItems);
        const result = await sendToParent(MSG.SYNC_TO_WORKBENCH, payload, {
          timeoutMs: DASHBOARD_SYNC_TO_WORKBENCH_TIMEOUT_MS,
        });
        if (!result) {
          showNotice('同步失败：工作台还没有返回结果，请稍后查看数据是否已写入；如果没有写入再重试。', 'error');
          return;
        }
        if (result?.success) {
          const {
            imported,
            skipped,
            invalid,
            total,
            detailText,
            outcomeText,
            failReason,
            monitorOutcomeConfirmed,
            mediaIncomplete,
            mediaRegistrationConfirmed,
          } = summarizeWorkbenchSyncResult(
            currentTab,
            selectedItems.length,
            result,
          );
          if (currentTab === 'comments') {
            const hasRetry = Number(result?.meta?.commentsFailed || 0) > 0;
            const hasPending = Number(result?.meta?.commentsQueued || 0) > 0;
            showNotice(
              `评论同步完成：${outcomeText || `已入库 ${imported} 条`}`,
              hasRetry ? 'warning' : hasPending ? 'info' : 'success',
            );
            return;
          }
          if (currentTab === 'authors') {
            showNotice(
              `博主同步完成：${outcomeText || `已新增监控来源 ${imported} 条`}`,
              !monitorOutcomeConfirmed ? 'info' : skipped > 0 ? 'warning' : imported > 0 ? 'success' : 'info',
            );
            return;
          }
          const invalidText = invalid > 0 ? `，无效 ${invalid} 条` : '';
          if (imported === total && !mediaIncomplete && mediaRegistrationConfirmed) {
            const outcomeSuffix = outcomeText ? `，${outcomeText}` : '';
            showNotice(`成功同步 ${total} 条${getTabLabel(currentTab)}到内容工作台${detailText}${outcomeSuffix}`, 'success');
          } else if (imported === total && (mediaIncomplete || !mediaRegistrationConfirmed)) {
            showNotice(`内容已同步 ${total} 条${getTabLabel(currentTab)}到内容工作台${detailText}，${outcomeText || '媒体登记未完成'}`, 'warning');
          } else if (imported > 0) {
            const reasonText = failReason ? `，原因：${failReason}` : '';
            showNotice(`部分同步成功：导入 ${imported} 条，跳过 ${skipped} 条${invalidText}${detailText}${reasonText}`, 'warning');
          } else {
            const reasonText = failReason ? `，原因：${failReason}` : '';
            showNotice(`所有${getTabLabel(currentTab)}都已存在或不可导入，跳过 ${skipped} 条${invalidText}${detailText}${reasonText}`, 'info');
          }
        } else {
          const errorMsg = result?.error || result?.meta?.failReason || '未知错误';
          showNotice(`同步失败：${errorMsg}`, 'error');
        }
      } catch (error) {
        showNotice(`同步失败：${error.message || '未知错误'}`, 'error');
      }
    });
  }, [currentSelected, allData, currentTab, showNotice, withBusyAction]);

  // ===== Tab 切换 =====
  const handleTabChange = useCallback((tab) => {
    setCurrentTab(tab);
    setCurrentPage(1);
    setFilterType('all');
  }, []);

  // ===== 列定义 =====
  const columns = useMemo(() => getColumns(currentTab, allData), [currentTab, allData]);

  // ===== 渲染单元格 =====
  const renderCell = useCallback((item, col) => {
    const val = item[col.key];
    const renderStatusPill = (label, tone, extraClass = '') => (
      <span className={`table-status-pill tone-${tone}${extraClass ? ` ${extraClass}` : ''}`}>{label}</span>
    );
    if (col.key === 'url' || col.key === 'noteUrl' || col.key === 'profileUrl' || col.key === 'avatarUrl') {
      const actionText = LINK_ACTION_TEXT[col.key] || '打开';
      return renderLinkAction(getPreferredRecordUrl(item, col.key) || val, actionText);
    }
    if (col.key === 'createdAt') {
      return val ? new Date(val).toLocaleString('zh-CN') : '';
    }
    if (col.key === 'lastUpdateTime') {
      return String(val || '-');
    }
    if (col.key === 'platform') {
      return renderStatusPill(val === 'douyin' ? '抖音' : '小红书', val === 'douyin' ? 'warning' : 'neutral');
    }
    if (col.key === 'contentId') {
      const contentId = String(item.contentId || item.noteId || '').trim();
      return <span title={contentId}>{truncate(contentId, 28)}</span>;
    }
    if (col.key === 'level') {
      const level = Number(val || 0);
      return level === 2 ? '二级' : level === 1 ? '一级' : '-';
    }
    if (col.key === 'type') {
      return val === 'video' ? '视频' : '图文';
    }
    if (col.key === 'title') {
      const cleanTitle = stripHashtags(String(val || ''));
      return <span title={String(val || '')}>{truncate(cleanTitle || String(val || ''), 40)}</span>;
    }
    if (col.key === 'content') {
      const cleanContent = cleanDisplayBodyText(String(val || ''));
      return <span title={cleanContent || String(val || '')}>{truncate(cleanContent || String(val || ''), 40)}</span>;
    }
    if (col.key === 'text') {
      const text = String(val || '').trim();
      return <span className="cell-clamp-3" title={text}>{text}</span>;
    }
    if (col.key === 'author' || col.key === 'authorName') {
      const text = String(val || '').trim();
      return <span title={text}>{truncate(text, 24)}</span>;
    }
    if (col.key === 'hashtags') {
      const list = getHashtagsForItem(item);
      return <span title={list.join('\n')}>{truncate(list.join(', '), 40)}</span>;
    }
    if (col.key === 'authorFollowed' || col.key === 'shareRestricted' || col.key === 'followedByMe') {
      return val ? '是' : '否';
    }
    if (col.key === 'avatar') {
      if (!val || val === 'undefined' || val === 'null' || imgErrorIds.has(item.id)) return '-';
      return (
        <img
          src={val}
          alt=""
          style={{ width: 32, height: 32, borderRadius: '50%', border: '1px solid #ccc', objectFit: 'cover' }}
          onError={() => setImgErrorIds(prev => new Set(prev).add(item.id))}
        />
      );
    }
    if (col.key === 'handle') {
      const handle = getUnifiedAuthorHandle(item);
      return <span title={handle}>{truncate(handle, 40)}</span>;
    }
    if (col.key === 'ipLocation') {
      const label = String(item.ipLocation || item.location || '').trim();
      return <span title={label}>{truncate(label, 20)}</span>;
    }
    if (col.key === 'batchSelectionMode') {
      return formatBatchSelectionModeLabel(val);
    }
    if (col.key === 'dataQuality') {
      const label = formatDataQualityLabel(val);
      const tone = String(val || '') === 'full' ? 'success' : String(val || '') === 'degraded' ? 'warning' : 'neutral';
      return <span title={String(val || '')}>{renderStatusPill(label, tone)}</span>;
    }
    if (col.key === 'qualityReason') {
      const label = formatQualityReasonLabel(val);
      return <span title={String(val || '')}>{truncate(label, 24)}</span>;
    }
    if (col.key === 'sourceTier') {
      const label = formatSourceTierLabel(val);
      const tone = String(val || '') === 'api' || String(val || '') === 'mixed' ? 'info' : String(val || '') === 'dom' ? 'warning' : 'neutral';
      return <span title={String(val || '')}>{renderStatusPill(label, tone)}</span>;
    }
    if (col.key === 'batchRank') {
      const rank = Number(val || 0);
      return rank > 0 ? rank : '-';
    }
    if (col.key === 'replyToUserName') {
      const label = formatReplyTargetLabel(item);
      return <span title={label}>{truncate(label, 20)}</span>;
    }
    if (col.key === 'collectionRunId') {
      const label = formatCollectionRunLabel(val);
      return <span title={String(val || '')}>{truncate(label, 18)}</span>;
    }
    if (col.key === 'commentId') {
      const text = String(val || '').trim();
      return <span title={text}>{truncate(text, 14)}</span>;
    }
    if (col.key === 'images') {
      const count = Array.isArray(item.images) ? item.images.length : 0;
      const tip = Array.isArray(item.images) ? item.images.join('\n') : '';
      return <span title={tip}>{count} 张</span>;
    }
    if (col.key === 'mediaPreview') {
      const thumbRaw = item.cover || item.coverImg || (Array.isArray(item.images) ? item.images[0] : '') || '';
      const thumb = toDisplayUrl(thumbRaw);
      const videoPreviewRaw = item.videoDownloadUrl || item.videoPlayUrl || item.video
        || (Array.isArray(item.videoStreams) ? item.videoStreams.find((s) => s?.url)?.url : '') || '';
      const videoPreview = toDisplayUrl(videoPreviewRaw);
      const hasVideo = Boolean(item.video || item.videoDownloadUrl || item.videoPlayUrl
        || (Array.isArray(item.videoStreams) && item.videoStreams.length > 0));
      const mediaCount = (Array.isArray(item.images) ? item.images.length : 0) + (hasVideo ? 1 : 0);
      const title = item.title || item.noteId || '媒体预览';
      if (!thumb && !videoPreview) {
        return <div className="media-cell"><span className="media-empty">无素材</span></div>;
      }
      if (!thumb && videoPreview) {
        return (
          <div className="media-cell">
            <button className="media-thumb-btn media-thumb-fallback"
              onClick={() => setMediaPreview({ open: true, url: videoPreview, title, type: 'video' })}
            >查看媒体</button>
            <span className="media-count">{mediaCount} 个（含视频）</span>
          </div>
        );
      }
      return (
        <div className="media-cell">
          <button className="media-thumb-btn"
            onClick={() => setMediaPreview({ open: true, url: thumb, title, type: 'image' })}
          >
            <img className="media-thumb" src={thumb} alt="缩略图" />
          </button>
          <span className="media-count">{mediaCount} 个{hasVideo ? '（含视频）' : ''}</span>
        </div>
      );
    }
    if (col.key === 'mediaDownloadStatus') {
      const statusRaw = String(item.mediaDownloadStatus || '待下载');
      const summary = item.mediaDownloadSummary || {};
      const failed = Number(summary.failed || 0);
      const success = Number(summary.success || 0);
      const total = Number(summary.total || 0);
      const mapped = getMediaStatusMeta(statusRaw);
      const tip = `成功 ${success}/${total}，失败 ${failed}`;
      return <span title={tip} className={`media-status tone-${mapped.tone}`}>{mapped.label}</span>;
    }
    if (col.key === 'atUserList') {
      const list = Array.isArray(item.atUserList) ? item.atUserList : [];
      const text = list.map((v) => v.nickname || v.userId).filter(Boolean).join('\n');
      return <span title={text}>{list.length}</span>;
    }
    if (col.key === 'topicIds') {
      const list = Array.isArray(item.topicIds) ? item.topicIds : [];
      return <span title={list.join('\n')}>{list.length}</span>;
    }
    return <span title={String(val ?? '')}>{truncate(String(val ?? ''), 40)}</span>;
  }, []);

  // ===== 分页跳转 =====
  const goToPage = useCallback((page) => {
    setCurrentPage(page);
    if (tableWrapperRef.current) tableWrapperRef.current.scrollTop = 0;
  }, []);

  const emptyState = useMemo(() => {
    if (loading && allData.length === 0) {
      return {
        title: '正在加载数据',
        hint: '正在从插件本地数据库读取当前标签页的数据，请稍候。',
        tone: 'info',
      };
    }
    if (allData.length === 0) {
      return {
        title: '还没有采集数据',
        hint: '先去小红书或抖音页面执行采集，再回到这里筛选、导出和同步。',
        tone: 'neutral',
      };
    }
    return {
      title: '当前筛选条件下没有结果',
      hint: '可以清空关键词、日期或类型筛选，看看是否还有其它数据。',
      tone: 'warning',
    };
  }, [loading, allData.length]);

  // ===== 渲染 =====
  const noticeMeta = displayNotice ? getFeedbackMeta(displayNotice.type) : null;
  return (
    <div className="dashboard-container">
      {displayNotice && (
        <section className={`dashboard-notice ${displayNotice.type}`}>
          <span
            className="dashboard-notice-icon"
            aria-hidden="true"
            dangerouslySetInnerHTML={{ __html: icon(noticeMeta.icon, { size: 16 }) }}
          />
          <div className="dashboard-notice-copy">
            <strong>{noticeMeta.title}</strong>
            <p>{displayNotice.message}</p>
          </div>
        </section>
      )}

      <nav className="dashboard-nav">
        <div className="dashboard-brand">
          <div className="dashboard-brand-banner-shell" aria-hidden="true">
            <img className="dashboard-brand-banner" src={BRAND_BANNER_SRC} alt="" />
          </div>
        </div>
        <div className="nav-tabs">
          {TABS.map((tab) => (
            <button
              key={tab.key}
              className={`tab ${currentTab === tab.key ? 'active' : ''}`}
              onClick={() => handleTabChange(tab.key)}
              dangerouslySetInnerHTML={{
                __html: `${icon(tab.key === 'notes' ? 'note' : tab.key === 'comments' ? 'comment' : 'author', { size: 14 })} ${tab.label}`,
              }}
            />
          ))}
        </div>
      </nav>

      <div className="toolbar">
        <input
          type="text"
          className="search-input"
          placeholder="搜索标题 / 正文 / 作者 / 话题..."
          value={searchKeyword}
          onChange={(e) => { setSearchKeyword(e.target.value); setCurrentPage(1); }}
        />
        {currentTab === 'notes' && (
          <select
            className="filter-select"
            value={filterType}
            onChange={(e) => { setFilterType(e.target.value); setCurrentPage(1); }}
          >
            <option value="all">全部</option>
            <option value="normal">图文</option>
            <option value="video">视频</option>
          </select>
        )}
        <select
          className="filter-select"
          value={sortByTime}
          onChange={(e) => { setSortByTime(e.target.value); setCurrentPage(1); }}
        >
          <option value="desc">按采集时间: 最新</option>
          <option value="asc">按采集时间: 最早</option>
        </select>
        <input
          type="date"
          className="filter-select"
          title="按采集日期筛选"
          value={filterDate}
          onChange={(e) => { setFilterDate(e.target.value); setCurrentPage(1); }}
        />
        <div className="toolbar-actions">
          <button
            className="toolbar-btn secondary"
            style={{ display: selectedCount > 0 ? 'inline-block' : 'none' }}
            onClick={handleExportSelected}
            disabled={Boolean(busyActions.exportSelected)}
          >
            {busyActions.exportSelected ? '导出中...' : '导出选中'}
          </button>
          <button
            className="toolbar-btn danger"
            style={{ display: selectedCount > 0 ? 'inline-block' : 'none' }}
            onClick={handleDeleteSelected}
            disabled={Boolean(busyActions.deleteSelected)}
          >
            {busyActions.deleteSelected ? '删除中...' : '删除选中'}
          </button>
          <button
            className="toolbar-btn primary"
            style={{ display: selectedCount > 0 ? 'inline-block' : 'none' }}
            onClick={handleSyncToWorkbench}
            disabled={Boolean(busyActions.syncWorkbench)}
          >
            {busyActions.syncWorkbench ? '同步中...' : '同步到工作台'}
          </button>
          <button className="toolbar-btn" onClick={handleExportCsv} disabled={Boolean(busyActions.exportCsv)}>
            {busyActions.exportCsv ? '导出中...' : '导出 CSV'}
          </button>
          <button className="toolbar-btn" onClick={handleExportJson} disabled={Boolean(busyActions.exportJson)}>
            {busyActions.exportJson ? '导出中...' : '导出 JSON'}
          </button>
          <button className="toolbar-btn danger" onClick={handleClearAll} disabled={Boolean(busyActions.clearAll)}>
            {busyActions.clearAll ? '清空中...' : '清空'}
          </button>
        </div>
      </div>

      <div className="data-summary">
        <span>共 {totalCount} 条</span>
        {selectedCount > 0 && (
          <>
            <span className="summary-divider">·</span>
            <span>已选 {selectedCount} 条</span>
          </>
        )}
      </div>

      <div className="data-table-wrapper" ref={tableWrapperRef}>
        {loading && allData.length > 0 && (
          <div className="loading-more-bar">正在加载更多数据...</div>
        )}
        {totalCount === 0 ? (
          <div className="empty-state">
            <span
              className={`empty-state-icon tone-${emptyState.tone}`}
              aria-hidden="true"
              dangerouslySetInnerHTML={{ __html: icon(loading ? 'loader' : emptyState.tone === 'warning' ? 'alertTriangle' : 'package', { size: 42 }) }}
            />
            <p>{emptyState.title}</p>
            <p className="empty-hint">{emptyState.hint}</p>
          </div>
        ) : (
          <table className={`data-table data-table-${currentTab}`}>
            <thead>
              <tr>
                <th className="select-col">
                  <input
                    type="checkbox"
                    checked={allSelected}
                    ref={(el) => { if (el) el.indeterminate = someSelected && !allSelected; }}
                    onChange={toggleSelectAll}
                  />
                </th>
                {columns.map((col) => (
                  <th key={col.key} className={col.className}>{col.label}</th>
                ))}
                <th>操作</th>
              </tr>
            </thead>
            <tbody>
              {pageData.map((item) => {
                const id = getItemId(item, currentTab);
                const isChecked = currentSelected.has(id);
                return (
                  <tr key={id} className={isChecked ? 'is-selected' : ''}>
                    <td className="select-col">
                      <input
                        type="checkbox"
                        checked={isChecked}
                        onChange={() => toggleSelect(id)}
                      />
                    </td>
                    {columns.map((col) => (
                      <td key={col.key} className={col.className}>
                        {renderCell(item, col)}
                      </td>
                    ))}
                    <td className="actions-cell">
                      {currentTab === 'notes' && (
                        <button
                          className="download-btn"
                          onClick={async (e) => {
                            const noteId = String(item.noteId || '').trim();
                            if (!noteId) return;
                            const selection = await showMediaDownloadDialog(item);
                            if (!selection) return;
                            const busyKey = `download:${noteId}`;
                            if (rowBusyActionsRef.current[busyKey]) return;
                            setRowBusyActionState(busyKey, true);
                            try {
                              showNotice(`正在下载 ${selection.count || 0} 个媒体文件...`, 'info');
                              const result = await sendToParent(MSG.DOWNLOAD_NOTE_MEDIA, {
                                noteId,
                                mediaTypes: selection.mediaTypes,
                              }, { timeoutMs: 10 * 60 * 1000 });
                              if (result?.success) {
                                const s = result.summary || {};
                                const refreshedText = s.refreshed ? ' 已自动刷新媒体链接后重试。' : '';
                                const failedCount = s.failed || 0;
                                const msg = failedCount > 0
                                  ? `下载完成：成功 ${s.success || 0}/${s.total || 0}，失败 ${failedCount}。${refreshedText || '部分资源可能已过期，建议重新打开笔记采集一次后再重试下载。'}`
                                  : `下载完成：成功 ${s.success || 0}/${s.total || 0}，失败 0。${refreshedText}`;
                                showNotice(msg, failedCount > 0 ? 'warning' : 'success');
                                loadData(currentTab);
                              } else {
                                showNotice(`下载失败：${result?.error || '未知错误'}`, 'error');
                              }
                            } catch (err) {
                              showNotice(`下载失败：${err?.message || err}`, 'error');
                            } finally {
                              setRowBusyActionState(busyKey, false);
                            }
                          }}
                          disabled={Boolean(rowBusyActions[`download:${String(item.noteId || '').trim()}`])}
                        >
                          {rowBusyActions[`download:${String(item.noteId || '').trim()}`] ? '下载中...' : '媒体'}
                        </button>
                      )}
                      <button
                        className="delete-btn"
                        onClick={async () => {
                          const busyKey = `delete:${id}`;
                          if (rowBusyActionsRef.current[busyKey]) return;
                          const confirmed = await showConfirm({
                            title: '确认删除',
                            message: '确定要删除这条数据吗？此操作不可恢复。',
                            detail: '删除后这条记录不会再参与导出、同步或媒体下载。',
                            confirmText: '删除',
                          });
                          if (!confirmed) return;
                          setRowBusyActionState(busyKey, true);
                          try {
                            const deleteActionMap = { notes: MSG.DELETE_NOTE, comments: MSG.DELETE_COMMENT, authors: MSG.DELETE_AUTHOR };
                            const idKey = currentTab === 'notes' ? 'noteId' : currentTab === 'comments' ? 'id' : 'userId';
                            await sendToParent(deleteActionMap[currentTab], { [idKey]: id });
                            showNotice('删除成功', 'success');
                            loadData(currentTab);
                          } catch (error) {
                            showNotice(`删除失败：${error.message || '未知错误'}`, 'error');
                          } finally {
                            setRowBusyActionState(busyKey, false);
                          }
                        }}
                        disabled={Boolean(rowBusyActions[`delete:${id}`])}
                      >
                        {rowBusyActions[`delete:${id}`] ? '删除中...' : '删除'}
                      </button>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        )}
      </div>

      {totalCount > 0 && (
        <div className="pagination-bar">
          <div className="pagination-info">
            <span>共 {totalCount} 条 · 第 {safePage}/{totalPages} 页</span>
            <label className="page-size-selector">
              每页
              <select className="filter-select" value={pageSize} onChange={handlePageSizeChange}>
                {PAGE_SIZE_OPTIONS.map((opt) => (
                  <option key={opt} value={opt}>{opt}</option>
                ))}
              </select>
              条
            </label>
          </div>
          <div className="pagination-controls">
            <button disabled={safePage <= 1} onClick={() => goToPage(safePage - 1)}>上一页</button>
            {Array.from({ length: totalPages }, (_, i) => i + 1)
              .filter((p) => {
                if (p === 1 || p === totalPages) return true;
                return Math.abs(p - safePage) <= 2;
              })
              .map((p, idx, arr) => (
                <React.Fragment key={p}>
                  {idx > 0 && arr[idx - 1] !== p - 1 && <span>...</span>}
                  <button
                    className={p === safePage ? 'active' : ''}
                    onClick={() => goToPage(p)}
                  >
                    {p}
                  </button>
                </React.Fragment>
              ))}
            <button disabled={safePage >= totalPages} onClick={() => goToPage(safePage + 1)}>下一页</button>
          </div>
        </div>
      )}

      {/* 确认弹窗 */}
      {confirmDialog.open && (
        <div className="dashboard-dialog-overlay" style={{ display: 'flex' }} aria-hidden="false">
          <div className="dashboard-dialog" role="dialog" aria-modal="true">
            <span className={`dashboard-dialog-badge tone-${confirmDialog.confirmTone}`}>{confirmDialog.confirmTone === 'danger' ? '高风险操作' : '确认操作'}</span>
            <h2>{confirmDialog.title}</h2>
            <p>{confirmDialog.message}</p>
            {confirmDialog.detail ? <div className="dashboard-dialog-detail">{confirmDialog.detail}</div> : null}
            <div className="dashboard-dialog-actions">
              <button className="toolbar-btn" onClick={() => handleConfirm(false)}>取消</button>
              <button className="toolbar-btn danger" onClick={() => handleConfirm(true)}>{confirmDialog.confirmText}</button>
            </div>
          </div>
        </div>
      )}

      {/* 媒体下载选择 */}
      {mediaDownloadDialog.open && (
        <div className="dashboard-dialog-overlay" style={{ display: 'flex' }} aria-hidden="false">
          <div className="dashboard-dialog media-download-dialog" role="dialog" aria-modal="true">
            <span className="dashboard-dialog-badge">媒体下载</span>
            <h2>下载媒体文件</h2>
            <p>{mediaDownloadDialog.title}</p>
            <div className="media-download-options">
              {mediaDownloadDialog.options.map((option) => {
                const checked = mediaDownloadDialog.selected.includes(option.value);
                return (
                  <label key={option.value} className={`media-download-option${checked ? ' is-selected' : ''}`}>
                    <input
                      type="checkbox"
                      checked={checked}
                      onChange={() => toggleMediaDownloadType(option.value)}
                    />
                    <span>{option.label}</span>
                    <em>{option.count} 个</em>
                  </label>
                );
              })}
            </div>
            <div className="dashboard-dialog-actions">
              <button className="toolbar-btn" onClick={() => handleMediaDownloadDialog(false)}>取消</button>
              <button
                className="toolbar-btn"
                disabled={countSelectedMedia(mediaDownloadDialog.options, mediaDownloadDialog.selected) === 0}
                onClick={() => handleMediaDownloadDialog(true)}
              >
                下载选中
              </button>
            </div>
          </div>
        </div>
      )}

      {/* 媒体预览 */}
      {mediaPreview.open && (
        <div className="media-preview-modal" style={{ display: 'flex' }} onClick={() => {
          document.querySelector('.media-preview-video')?.pause();
          setMediaPreview((p) => ({ ...p, open: false }));
        }}>
          <div className="media-preview-backdrop" />
          <div className="media-preview-panel" onClick={(e) => e.stopPropagation()}>
            <button className="media-preview-close" type="button" onClick={() => {
              document.querySelector('.media-preview-video')?.pause();
              setMediaPreview((p) => ({ ...p, open: false }));
            }}>×</button>
            <div className="media-preview-title">{mediaPreview.title || '媒体预览'}</div>
            {mediaPreview.type === 'video' ? (
              <video className="media-preview-video" src={mediaPreview.url} controls playsinline style={{ display: 'block', maxWidth: '78vw', maxHeight: '72vh', borderRadius: '12px' }} />
            ) : (
              <img className="media-preview-image" src={mediaPreview.url} alt={mediaPreview.title} />
            )}
          </div>
        </div>
      )}
    </div>
  );
}
