import { MSG } from './constants.js';
import { mapErrorToProtocolError } from '../workbench/runtime/errorMapper.js';
import { normalizeProgressEvent, toLegacyProgressMessage } from '../workbench/runtime/progressEvent.js';

/**
 * 检查扩展 context 是否仍然有效
 */
export function isContextValid() {
  try {
    return !!chrome.runtime?.id;
  } catch {
    return false;
  }
}

/**
 * 发送消息到 Background Service Worker
 */
export function sendToBackground(action, data = {}, { timeoutMs = 15000 } = {}) {
  if (!isContextValid()) {
    return Promise.reject(new Error('Extension context invalidated'));
  }
  return new Promise((resolve, reject) => {
    const timer = Number.isFinite(Number(timeoutMs)) && Number(timeoutMs) > 0
      ? setTimeout(() => {
        reject(new Error(`sendToBackground timeout: ${String(action || 'unknown_action')}`));
      }, Number(timeoutMs))
      : null;

    chrome.runtime.sendMessage({ action, ...data }, (response) => {
      if (timer) clearTimeout(timer);
      if (chrome.runtime.lastError) {
        reject(chrome.runtime.lastError);
      } else {
        resolve(response);
      }
    });
  });
}

function isTabContextError(message = '') {
  return /Receiving end does not exist|context invalidated|The message port closed/i.test(String(message || ''));
}

async function reinjectContentScript(tabId, {
  contentScriptFiles = ['vendor.js', 'content.js'],
  contentCssFiles = ['content.css'],
} = {}) {
  if (!chrome?.scripting?.executeScript) {
    throw new Error('chrome.scripting.executeScript unavailable');
  }
  if (Array.isArray(contentCssFiles) && contentCssFiles.length > 0 && chrome?.scripting?.insertCSS) {
    try {
      await chrome.scripting.insertCSS({
        target: { tabId },
        files: contentCssFiles,
      });
    } catch {
      // CSS is helpful for the floating UI, but message recovery only requires JS.
    }
  }
  await chrome.scripting.executeScript({
    target: { tabId },
    files: contentScriptFiles,
  });
}

function sendRawToTab(tabId, payload, { allowContextError = false, timeoutMs = 0 } = {}) {
  return new Promise((resolve, reject) => {
    const timer = Number.isFinite(Number(timeoutMs)) && Number(timeoutMs) > 0
      ? setTimeout(() => {
        reject(new Error(`sendToTab timeout: ${String(payload?.action || 'unknown_action')}`));
      }, Number(timeoutMs))
      : null;

    chrome.tabs.sendMessage(tabId, payload, (response) => {
      if (timer) clearTimeout(timer);
      const runtimeErr = chrome.runtime.lastError;
      if (runtimeErr) {
        const message = String(runtimeErr.message || runtimeErr);
        if (allowContextError && isTabContextError(message)) {
          resolve({
            success: false,
            skipped: true,
            recoverable: true,
            error: message,
          });
          return;
        }
        reject(new Error(message));
        return;
      }
      if (response?.error) {
        reject(new Error(response.error));
        return;
      }
      resolve(response || { success: true });
    });
  });
}

export async function sendToTab(tabId, payload, {
  allowContextError = false,
  timeoutMs = 0,
  autoReconnect = false,
  contentScriptFiles,
  contentCssFiles,
} = {}) {
  try {
    return await sendRawToTab(tabId, payload, { allowContextError, timeoutMs });
  } catch (error) {
    const message = String(error?.message || error || '');
    if (!autoReconnect || allowContextError || !isTabContextError(message)) {
      throw error;
    }
    await reinjectContentScript(tabId, { contentScriptFiles, contentCssFiles });
    return sendRawToTab(tabId, payload, { allowContextError, timeoutMs });
  }
}

/**
 * 广播进度消息（从 content script 发到 popup/background）
 */
export function reportProgress(current, total, status, meta = {}) {
  if (!isContextValid()) return;
  const error = meta.error
    ? mapErrorToProtocolError(meta.error)
    : (String(meta.taskState || '').trim() === 'error' ? mapErrorToProtocolError(status) : null);
  const progressEvent = normalizeProgressEvent({
    current,
    total,
    status,
    ...meta,
    error,
  });
  const legacy = toLegacyProgressMessage(progressEvent);
  chrome.runtime.sendMessage({
    action: MSG.PROGRESS,
    ...legacy,
    progressEvent,
  });
}

export function reportTaskError(error, meta = {}) {
  if (!isContextValid()) return;
  const protocolError = mapErrorToProtocolError(error, meta);
  const progressEvent = normalizeProgressEvent({
    current: Number(meta.current || 0),
    total: Number(meta.total || 0),
    status: protocolError.message,
    taskState: 'error',
    phase: meta.phase || meta.stage || 'finalizing',
    taskType: meta.taskType,
    metrics: meta.metrics,
    heartbeatAt: meta.heartbeatAt,
    error: protocolError,
  });
  const legacy = toLegacyProgressMessage(progressEvent);
  chrome.runtime.sendMessage({
    action: MSG.PROGRESS,
    ...legacy,
    progressEvent,
  });
}

export function reportWorkbenchRecord({
  recordType = '',
  externalRecordId = '',
  record = {},
  collectionRunId = '',
  externalTaskId = '',
  sequence = Date.now(),
  collectedAt = '',
} = {}) {
  if (!isContextValid()) return;
  chrome.runtime.sendMessage({
    action: MSG.WORKBENCH_RECORD_DELTA,
    recordType,
    externalRecordId,
    record: record && typeof record === 'object' && !Array.isArray(record) ? record : {},
    collectionRunId,
    externalTaskId,
    sequence,
    collectedAt,
  });
}

export function reportWorkbenchRecords(records = []) {
  if (!isContextValid()) return;
  const normalizedRecords = (Array.isArray(records) ? records : [])
    .map((item) => ({
      recordType: String(item?.recordType || '').trim(),
      externalRecordId: String(item?.externalRecordId || '').trim(),
      record: item?.record && typeof item.record === 'object' && !Array.isArray(item.record)
        ? item.record
        : {},
      collectionRunId: String(item?.collectionRunId || '').trim(),
      externalTaskId: String(item?.externalTaskId || '').trim(),
      sequence: Number(item?.sequence || Date.now()),
      collectedAt: String(item?.collectedAt || '').trim(),
    }))
    .filter((item) => item.recordType && Object.keys(item.record).length > 0);
  if (normalizedRecords.length === 0) return;
  chrome.runtime.sendMessage({
    action: MSG.WORKBENCH_RECORD_DELTA,
    records: normalizedRecords,
  });
}

/**
 * 广播采集完成消息
 */
export function reportDone(type, count, meta = {}) {
  if (!isContextValid()) return;
  chrome.runtime.sendMessage({
    action: MSG.COLLECT_DONE,
    type,
    count,
    ...meta,
  });
}
