import {
  LINGGAN_LOCAL_ORIGIN,
  createLingganPendingResult,
  readLingganLocalReadiness,
} from './adapter.js';
import { LINGGAN_RUNTIME_ACTION } from './runtimeActions.js';

async function openDashboard() {
  await chrome.tabs.create({ url: chrome.runtime.getURL('dashboard.html') });
  return { success: true };
}

async function getLingganStatus() {
  const readiness = await readLingganLocalReadiness();
  return {
    success: true,
    serverUrl: LINGGAN_LOCAL_ORIGIN,
    enabled: true,
    mode: 'linggan_adapter_pending',
    readiness,
  };
}

chrome.runtime.onMessage.addListener((message = {}, _sender, sendResponse) => {
  const action = String(message.action || '').trim();
  Promise.resolve().then(async () => {
    if (action === LINGGAN_RUNTIME_ACTION.TOGGLE_DASHBOARD) return openDashboard();
    if (action === LINGGAN_RUNTIME_ACTION.GET_FLYWHEEL_CONFIG || action === LINGGAN_RUNTIME_ACTION.SAVE_FLYWHEEL_CONFIG) return getLingganStatus();
    if (action === LINGGAN_RUNTIME_ACTION.GET_EXECUTION_STATION_STATUS) {
      const readiness = await readLingganLocalReadiness();
      return {
        success: true,
        registered: false,
        authorized: false,
        pluginVersion: chrome.runtime.getManifest().version,
        authorizationMessage: `${readiness.message} ${createLingganPendingResult('station_dispatch').message}`,
      };
    }
    if (action === LINGGAN_RUNTIME_ACTION.TEST_FLYWHEEL_CONNECTION) return getLingganStatus();
    if (action === LINGGAN_RUNTIME_ACTION.GET_STATS) return { success: true, notes: 0, comments: 0, authors: 0, source: 'browser_staging_only' };
    return createLingganPendingResult(action);
  }).then(sendResponse).catch((error) => {
    sendResponse({ success: false, code: 'linggan_adapter_error', message: String(error?.message || error) });
  });
  return true;
});
