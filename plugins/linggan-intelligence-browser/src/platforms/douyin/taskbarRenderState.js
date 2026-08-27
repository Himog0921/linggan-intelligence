import { TASK_STATE } from '../../shared/constants.js';
import { isTerminalTaskState, normalizeTaskState } from '../../shared/taskUi.js';

export function resolveDouyinTaskbarRenderState({
  taskState = TASK_STATE.RUNNING,
  message = '',
} = {}) {
  const normalizedTaskState = normalizeTaskState(taskState);
  const shouldHideAfterRender = isTerminalTaskState(normalizedTaskState);

  return {
    taskState: normalizedTaskState,
    visible: true,
    shouldHideAfterRender,
    hideImmediate: normalizedTaskState === TASK_STATE.IDLE,
    message: String(message || '').trim(),
  };
}
