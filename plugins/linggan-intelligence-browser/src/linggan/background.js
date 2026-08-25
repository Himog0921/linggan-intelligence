import { MSG } from '../shared/constants.js';
import {
  LINGGAN_LOCAL_ORIGIN,
  createLingganPendingResult,
  readLingganLocalReadiness,
} from './adapter.js';

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
    if (action === MSG.TOGGLE_DASHBOARD) return openDashboard();
    if (action === MSG.GET_FLYWHEEL_CONFIG || action === MSG.SAVE_FLYWHEEL_CONFIG) return getLingganStatus();
    if (action === MSG.GET_EXECUTION_STATION_STATUS) {
      const readiness = await readLingganLocalReadiness();
      return {
        success: true,
        registered: false,
        authorized: false,
        pluginVersion: chrome.runtime.getManifest().version,
        authorizationMessage: `${readiness.message} ${createLingganPendingResult('station_dispatch').message}`,
      };
    }
    if (action === MSG.TEST_FLYWHEEL_CONNECTION) return getLingganStatus();
    if (action === MSG.GET_STATS) return { success: true, notes: 0, comments: 0, authors: 0, source: 'browser_staging_only' };
    return createLingganPendingResult(action);
  }).then(sendResponse).catch((error) => {
    sendResponse({ success: false, code: 'linggan_adapter_error', message: String(error?.message || error) });
  });
  return true;
});
