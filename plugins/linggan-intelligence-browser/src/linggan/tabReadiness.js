const DEFAULT_STABLE_FOR_MS = 1500;
const DEFAULT_PROBE_RETRY_MS = 250;

/**
 * Wait until the tab has stopped navigating and the final document's content runtime responds.
 *
 * Xiaohongshu can finish `/discovery/item/...` and then redirect to `/explore/...`. Treating the
 * first `complete` event as ready sends the collection action into a document that is about to be
 * destroyed. The quiet interval is restarted by every URL/status navigation update; the final
 * probe proves that the content script belongs to the same URL Chrome currently reports.
 */
export function waitForStableTab({
  tabs,
  tabId,
  readinessAction,
  timeoutMs = 20000,
  stableForMs = DEFAULT_STABLE_FOR_MS,
  probeRetryMs = DEFAULT_PROBE_RETRY_MS,
}) {
  return new Promise((resolve) => {
    let settled = false;
    let timeoutHandle = null;
    let stabilityHandle = null;
    let latestStatus = '';
    let latestUrl = '';

    const clearStabilityTimer = () => {
      if (!stabilityHandle) return;
      clearTimeout(stabilityHandle);
      stabilityHandle = null;
    };

    const finish = (ready) => {
      if (settled) return;
      settled = true;
      clearStabilityTimer();
      if (timeoutHandle) clearTimeout(timeoutHandle);
      tabs.onUpdated.removeListener(listener);
      resolve(ready);
    };

    const armProbe = (delayMs = stableForMs) => {
      clearStabilityTimer();
      if (settled || latestStatus !== 'complete' || !latestUrl) return;
      const candidateUrl = latestUrl;
      stabilityHandle = setTimeout(() => {
        stabilityHandle = null;
        void confirmStableDocument(candidateUrl);
      }, delayMs);
    };

    const retryProbe = () => {
      if (settled || latestStatus !== 'complete' || !latestUrl) return;
      armProbe(probeRetryMs);
    };

    async function confirmStableDocument(candidateUrl) {
      if (settled) return;
      try {
        const tab = await tabs.get(tabId);
        const currentStatus = String(tab?.status || '');
        const currentUrl = String(tab?.url || '');
        latestStatus = currentStatus;
        latestUrl = currentUrl;
        if (currentStatus !== 'complete' || !currentUrl || currentUrl !== candidateUrl) {
          armProbe();
          return;
        }

        const response = await tabs.sendMessage(tabId, { action: readinessAction });
        const contentUrl = String(response?.context?.url || '');
        if (response?.success === true && contentUrl === currentUrl) {
          finish(true);
          return;
        }
        retryProbe();
      } catch {
        // The final document can be complete just before its content script is injected. Retry
        // only inside the outer timeout; never turn a missing content runtime into readiness.
        retryProbe();
      }
    }

    function listener(updatedTabId, changeInfo = {}, tab = {}) {
      if (updatedTabId !== tabId) return;
      const navigationChanged = typeof changeInfo.url === 'string'
        || changeInfo.status === 'loading'
        || changeInfo.status === 'complete';
      if (!navigationChanged) return;
      latestStatus = String(changeInfo.status || tab.status || latestStatus);
      latestUrl = String(changeInfo.url || tab.url || latestUrl);
      armProbe();
    }

    // Subscribe before reading the snapshot so a fast page cannot complete between the two.
    tabs.onUpdated.addListener(listener);
    void tabs.get(tabId)
      .then((tab) => {
        latestStatus = String(tab?.status || latestStatus);
        latestUrl = String(tab?.url || latestUrl);
        armProbe();
      })
      .catch(() => finish(false));
    timeoutHandle = setTimeout(() => finish(false), timeoutMs);
  });
}
