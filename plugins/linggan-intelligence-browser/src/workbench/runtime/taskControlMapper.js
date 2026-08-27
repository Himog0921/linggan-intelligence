import { validateTaskControl } from '../protocol/validator.js';
import { WORKBENCH_DISPATCH_TARGET, getSupportedRemoteTask } from '../protocol/schema.js';

export function mapTaskControlToInternalCommand(taskControl = {}, { tabId = null } = {}) {
  const validation = validateTaskControl(taskControl);
  if (!validation.valid) {
    const error = new Error('Invalid task control');
    error.validation = validation;
    throw error;
  }

  const taskConfig = getSupportedRemoteTask(taskControl.taskType);
  const action = taskConfig?.controlActions?.[taskControl.action];
  if (!action) {
    throw new Error(`Task type does not support control action: ${taskControl.taskType}:${taskControl.action}`);
  }

  return {
    dispatchTarget: taskConfig.dispatchTarget || WORKBENCH_DISPATCH_TARGET.CONTENT,
    action,
    payload: {
      tabId,
      command: String(taskControl.action || '').trim(),
      taskControl,
    },
    taskMeta: {
      externalTaskId: String(taskControl.taskId || '').trim(),
      externalTaskType: String(taskControl.taskType || '').trim(),
      protocolVersion: String(taskControl.protocolVersion || '').trim(),
      controlAction: String(taskControl.action || '').trim(),
    },
  };
}
