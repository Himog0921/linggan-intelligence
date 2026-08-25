import { MSG } from '../../shared/constants.js';
import { normalizeCompatResponse } from '../../shared/responseEnvelope.js';

const WORKBENCH_MESSAGE_ACTIONS = new Set([
  MSG.WORKBENCH_CAPABILITY_CHECK,
  MSG.WORKBENCH_DISPATCH_TASK,
  MSG.WORKBENCH_TASK_CONTROL,
  MSG.WORKBENCH_GET_RESULT_PACKAGE,
  MSG.WORKBENCH_LOCAL_CONTROL_EVENT,
  MSG.WORKBENCH_RECORD_DELTA,
  MSG.WORKBENCH_DELTA_FLUSH,
]);

export function isWorkbenchMessageAction(action = '') {
  return WORKBENCH_MESSAGE_ACTIONS.has(String(action || '').trim());
}

export function normalizeWorkbenchMessageResponse(action = '', result) {
  if (!isWorkbenchMessageAction(action)) return result;
  return normalizeCompatResponse(result);
}
