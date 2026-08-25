function normalizeId(value = '') {
  return String(value || '').trim();
}

function uniqueIds(values = []) {
  return Array.from(new Set(values.map(normalizeId).filter(Boolean)));
}

function normalizeTabId(value = 0) {
  const tabId = Number(value || 0);
  return Number.isFinite(tabId) && tabId > 0 ? tabId : 0;
}

function uniqueTabIds(values = []) {
  return Array.from(new Set(values.map(normalizeTabId).filter(Boolean)));
}

export function taskExecutionCleanupKeys(activeTask = {}) {
  const taskId = normalizeId(activeTask?.taskId);
  const externalTaskId = normalizeId(activeTask?.externalTaskId);
  const pluginRunId = normalizeId(activeTask?.pluginRunId);
  const pluginOpenedTabId = normalizeTabId(activeTask?.pluginOpenedTabId);

  return {
    registryIds: uniqueIds([taskId, externalTaskId, pluginRunId]),
    navigationIds: uniqueIds([taskId, externalTaskId, pluginRunId]),
    tabIds: uniqueTabIds([pluginOpenedTabId]),
  };
}

export function normalizeNavigatedTaskTabsSnapshot(value = {}) {
  const source = value && typeof value === 'object' && !Array.isArray(value)
    ? value
    : {};
  const normalized = {};
  for (const [rawTaskId, rawTabId] of Object.entries(source)) {
    const taskId = normalizeId(rawTaskId);
    const tabId = Number(rawTabId || 0);
    if (!taskId || !Number.isFinite(tabId) || tabId <= 0) continue;
    normalized[taskId] = tabId;
  }
  return normalized;
}

export function rememberNavigatedTaskTab(snapshot = {}, taskId = '', tabId = 0) {
  const next = normalizeNavigatedTaskTabsSnapshot(snapshot);
  const normalizedTaskId = normalizeId(taskId);
  const normalizedTabId = Number(tabId || 0);
  if (!normalizedTaskId || !Number.isFinite(normalizedTabId) || normalizedTabId <= 0) {
    return next;
  }
  next[normalizedTaskId] = normalizedTabId;
  return next;
}

export function removeNavigatedTaskTabs(snapshot = {}, taskIds = []) {
  const next = normalizeNavigatedTaskTabsSnapshot(snapshot);
  for (const taskId of uniqueIds(taskIds)) {
    delete next[taskId];
  }
  return next;
}
