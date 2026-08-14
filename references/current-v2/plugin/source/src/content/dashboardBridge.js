import { normalizeCompatResponse } from '../shared/responseEnvelope.js';

function generateNonce() {
  const arr = new Uint8Array(16);
  crypto.getRandomValues(arr);
  return Array.from(arr, (b) => b.toString(16).padStart(2, '0')).join('');
}

async function storeDashboardNonce(nonce) {
  try {
    const payload = { dashboardNonce: nonce, dashboardNonceAt: Date.now() };
    const areas = [chrome.storage.session, chrome.storage.local]
      .filter(Boolean)
      .filter((area, index, list) => list.indexOf(area) === index);
    await Promise.all(areas.map((area) => area.set(payload)));
  } catch (e) {
    console.error('[DashboardBridge] Failed to store nonce:', e);
  }
}

async function clearDashboardNonce() {
  try {
    const areas = [chrome.storage.session, chrome.storage.local]
      .filter(Boolean)
      .filter((area, index, list) => list.indexOf(area) === index);
    await Promise.all(areas.map((area) => area.remove(['dashboardNonce', 'dashboardNonceAt'])));
  } catch (e) {
    // ignore
  }
}

export function createDashboardBridge({
  MSG,
  noteStore,
  commentStore,
  authorStore,
  downloadNoteMediaFromRecord,
  _testNonce = null,
  _testDashboardWindow = null,
} = {}) {
  let dashboardIframe = null;
  let dashboardOverlay = null;
  let currentNonce = _testNonce;
  let dashboardMessageRegistered = false;
  let dashboardOverlayClickHandler = null;

  function getTrustedDashboardWindow() {
    return dashboardIframe?.contentWindow || _testDashboardWindow || null;
  }

  function isDashboardMessageSourceTrusted(event) {
    const trustedDashboardWindow = getTrustedDashboardWindow();
    if (!trustedDashboardWindow) return true;
    return event.source === trustedDashboardWindow;
  }

  function readPageRequest(data = {}) {
    const limit = Number(data?.limit || 0);
    if (!Number.isFinite(limit) || limit <= 0) return null;
    return {
      offset: Math.max(0, Number(data?.offset || 0)),
      limit: Math.max(1, Math.min(500, limit)),
    };
  }

  async function readStoreRecords(store, data = {}) {
    const page = readPageRequest(data);
    if (!page) return store.getAll();
    const [records, total] = await Promise.all([
      store.getPage(page),
      store.count(),
    ]);
    return {
      success: true,
      data: records,
      total,
      offset: page.offset,
      limit: page.limit,
      hasMore: page.offset + records.length < total,
    };
  }

  async function toggleDashboard() {
    if (dashboardIframe && document.body.contains(dashboardIframe)) {
      await closeDashboard();
      return;
    }

    currentNonce = generateNonce();
    await storeDashboardNonce(currentNonce);

    dashboardOverlay = document.createElement('div');
    Object.assign(dashboardOverlay.style, {
      position: 'fixed',
      top: '0', left: '0', right: '0', bottom: '0',
      background: 'rgba(0,0,0,0.4)',
      zIndex: '2147483640',
    });
    dashboardOverlayClickHandler = () => {
      void toggleDashboard();
    };
    dashboardOverlay.addEventListener('click', dashboardOverlayClickHandler);

    dashboardIframe = document.createElement('iframe');
    const dashboardUrl = new URL(chrome.runtime.getURL('dashboard.html'));
    dashboardIframe.src = dashboardUrl.toString();
    Object.assign(dashboardIframe.style, {
      position: 'fixed',
      top: '2%',
      left: '3%',
      width: '94%',
      height: '96%',
      border: 'none',
      borderRadius: '16px',
      boxShadow: '0 16px 48px rgba(0,0,0,0.3)',
      zIndex: '2147483641',
      background: '#fff',
    });

    document.body.appendChild(dashboardOverlay);
    document.body.appendChild(dashboardIframe);
  }

  async function closeDashboard() {
    if (dashboardOverlayClickHandler && dashboardOverlay?.removeEventListener) {
      dashboardOverlay.removeEventListener('click', dashboardOverlayClickHandler);
    }
    dashboardIframe?.remove();
    dashboardOverlay?.remove();
    dashboardIframe = null;
    dashboardOverlay = null;
    dashboardOverlayClickHandler = null;
    currentNonce = null;
    await clearDashboardNonce();
  }

  const dashboardMessageHandlers = {
    [MSG.GET_ALL_NOTES]: (data) => readStoreRecords(noteStore, data),
    [MSG.GET_ALL_COMMENTS]: (data) => readStoreRecords(commentStore, data),
    [MSG.GET_ALL_AUTHORS]: (data) => readStoreRecords(authorStore, data),
    [MSG.DOWNLOAD_NOTE_MEDIA]: async (data) => {
      const noteId = data.noteId || '';
      if (!noteId) return { success: false, error: 'noteId required' };
      const note = await noteStore.getById(noteId);
      if (!note) return { success: false, error: 'note not found' };
      const summary = await downloadNoteMediaFromRecord(note, {
        mediaTypes: Array.isArray(data.mediaTypes) ? data.mediaTypes : undefined,
      });
      return { success: true, summary };
    },
    [MSG.CLEAR_ALL_NOTES]: () => noteStore.clear(),
    [MSG.CLEAR_ALL_COMMENTS]: () => commentStore.clear(),
    [MSG.CLEAR_ALL_AUTHORS]: () => authorStore.clear(),
    [MSG.DELETE_NOTE]: (data) => noteStore.deleteById(data.noteId),
    [MSG.DELETE_COMMENT]: (data) => commentStore.deleteById(data.id),
    [MSG.DELETE_AUTHOR]: (data) => authorStore.deleteById(data.userId),
    [MSG.SYNC_TO_WORKBENCH]: async (data) => {
      // 转发到 background script 进行实际同步
      // dashboard 无法直接访问外部 API（CORS限制）
      try {
        const result = await chrome.runtime.sendMessage({
          action: MSG.SYNC_TO_WORKBENCH,
          notes: Array.isArray(data.notes) ? data.notes : [],
          comments: Array.isArray(data.comments) ? data.comments : [],
          authors: Array.isArray(data.authors) ? data.authors : [],
        });
        return result || { success: false, error: 'No response from background' };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  };

  function normalizeDashboardMessageResponse(action, result) {
    const normalizedAction = String(action || '').trim();
    if (
      normalizedAction === MSG.GET_ALL_NOTES ||
      normalizedAction === MSG.GET_ALL_COMMENTS ||
      normalizedAction === MSG.GET_ALL_AUTHORS
    ) {
      return normalizeCompatResponse(result, {
        dataValue: Array.isArray(result) ? result : [],
      });
    }
    return normalizeCompatResponse(result);
  }

  async function handleDashboardMessageEvent(event) {
    if (event.data?.source !== 'lgboom-dashboard') return false;
    if (!isDashboardMessageSourceTrusted(event)) {
      console.warn('[DashboardBridge] Rejected message: untrusted dashboard window');
      return false;
    }
    if (!currentNonce || event.data?.nonce !== currentNonce) {
      console.warn('[DashboardBridge] Rejected message: invalid or missing nonce');
      return false;
    }

    const { action, nonce: _nonce, ...data } = event.data;
    const port = event.ports?.[0];
    if (!port) return true;

    const handler = dashboardMessageHandlers[action];
    if (handler) {
      try {
        const result = await handler(data);
        console.log(`[DashboardBridge] ${action} →`, Array.isArray(result) ? `${result.length} items` : result);
        port.postMessage(normalizeDashboardMessageResponse(action, result));
      } catch (err) {
        console.error(`[DashboardBridge] ${action} error:`, err);
        port.postMessage(normalizeDashboardMessageResponse(action, {
          success: false,
          error: err.message,
        }));
      }
    } else {
      console.warn(`[DashboardBridge] unknown action: ${action}`);
      port.postMessage(normalizeDashboardMessageResponse(action, {
        success: false,
        error: 'unknown_action',
      }));
    }
    return true;
  }

  function registerDashboardBridge() {
    if (dashboardMessageRegistered) return;
    window.addEventListener('message', handleDashboardMessageEvent);
    dashboardMessageRegistered = true;
  }

  function unregisterDashboardBridge() {
    if (!dashboardMessageRegistered) return;
    window.removeEventListener('message', handleDashboardMessageEvent);
    dashboardMessageRegistered = false;
  }

  async function destroyDashboardBridge() {
    await closeDashboard();
    unregisterDashboardBridge();
  }

  return {
    toggleDashboard,
    registerDashboardBridge,
    unregisterDashboardBridge,
    destroyDashboardBridge,
    handleDashboardMessageEvent,
  };
}
