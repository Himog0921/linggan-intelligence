import { createLingganPendingResult } from '../linggan/adapter.js';

const DASHBOARD_ACTION = {
  GET_ALL_NOTES: 'getAllNotes',
  GET_ALL_COMMENTS: 'getAllComments',
  GET_ALL_AUTHORS: 'getAllAuthors',
  DOWNLOAD_NOTE_MEDIA: 'downloadNoteMedia',
  CLEAR_ALL_NOTES: 'clearAllNotes',
  CLEAR_ALL_COMMENTS: 'clearAllComments',
  CLEAR_ALL_AUTHORS: 'clearAllAuthors',
  DELETE_NOTE: 'deleteNote',
  DELETE_COMMENT: 'deleteComment',
  DELETE_AUTHOR: 'deleteAuthor',
};

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
  noteStore,
  commentStore,
  authorStore,
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
    [DASHBOARD_ACTION.GET_ALL_NOTES]: (data) => readStoreRecords(noteStore, data),
    [DASHBOARD_ACTION.GET_ALL_COMMENTS]: (data) => readStoreRecords(commentStore, data),
    [DASHBOARD_ACTION.GET_ALL_AUTHORS]: (data) => readStoreRecords(authorStore, data),
    [DASHBOARD_ACTION.DOWNLOAD_NOTE_MEDIA]: () => createLingganPendingResult('media_download'),
    [DASHBOARD_ACTION.CLEAR_ALL_NOTES]: () => noteStore.clear(),
    [DASHBOARD_ACTION.CLEAR_ALL_COMMENTS]: () => commentStore.clear(),
    [DASHBOARD_ACTION.CLEAR_ALL_AUTHORS]: () => authorStore.clear(),
    [DASHBOARD_ACTION.DELETE_NOTE]: (data) => noteStore.deleteById(data.noteId),
    [DASHBOARD_ACTION.DELETE_COMMENT]: (data) => commentStore.deleteById(data.id),
    [DASHBOARD_ACTION.DELETE_AUTHOR]: (data) => authorStore.deleteById(data.userId),
  };

  function normalizeDashboardMessageResponse(action, result) {
    const normalizedAction = String(action || '').trim();
    if (
      normalizedAction === DASHBOARD_ACTION.GET_ALL_NOTES ||
      normalizedAction === DASHBOARD_ACTION.GET_ALL_COMMENTS ||
      normalizedAction === DASHBOARD_ACTION.GET_ALL_AUTHORS
    ) {
      if (Array.isArray(result)) return { success: true, data: result };
      return result && typeof result === 'object' ? { success: result.success !== false, ...result } : { success: true, data: [] };
    }
    if (result && typeof result === 'object') return { success: result.success !== false, ...result };
    return { success: true, data: result };
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
