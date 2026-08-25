import { normalizeWorkbenchMessageResponse } from '../workbench/protocol/responseEnvelope.js';
import { normalizeContentMessageResponse } from './protocol/responseEnvelope.js';

export function normalizeRuntimeMessageResponse(action, result) {
  return normalizeContentMessageResponse(
    action,
    normalizeWorkbenchMessageResponse(action, result),
  );
}

export function createRuntimeMessageListener({
  loadContentDataRuntime,
  isContextValid,
} = {}) {
  return (message, sender, sendResponse) => {
    if (!isContextValid()) return;

    Promise.resolve(loadContentDataRuntime())
      .then((runtime) => {
        const handler = runtime.messageHandlers[message.action];
        if (!handler) {
          sendResponse(undefined);
          return;
        }
        return Promise.resolve(handler(message)).then(async (result) => {
          if (result?.toggleDashboard) {
            await runtime.dashboardBridge.toggleDashboard();
            sendResponse({ success: true });
            return;
          }
          sendResponse(normalizeRuntimeMessageResponse(message.action, result));
        });
      })
      .catch((err) => {
        sendResponse(normalizeRuntimeMessageResponse(message.action, {
          success: false,
          error: err.message,
        }));
      });
    return true;
  };
}
