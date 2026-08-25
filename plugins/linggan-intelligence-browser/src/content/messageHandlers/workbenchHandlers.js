import { REMOTE_TASK_CONTROL_ACTION } from '../../workbench/protocol/schema.js';

function applyLegacyBatchControl({ action, getBatchNoteCtrl, getBatchCommentCtrl } = {}) {
  const batchNoteCtrl = getBatchNoteCtrl?.();
  const batchCommentCtrl = getBatchCommentCtrl?.();
  if (
    action === REMOTE_TASK_CONTROL_ACTION.STOP
    || action === REMOTE_TASK_CONTROL_ACTION.DELETE
  ) {
    batchNoteCtrl?.stop?.();
    batchCommentCtrl?.stop?.();
    return;
  }
  if (action === REMOTE_TASK_CONTROL_ACTION.PAUSE) {
    batchNoteCtrl?.pause?.();
    batchCommentCtrl?.pause?.();
    return;
  }
  if (action === REMOTE_TASK_CONTROL_ACTION.RESUME) {
    batchNoteCtrl?.resume?.();
    batchCommentCtrl?.resume?.();
  }
}

export function createWorkbenchHandlers({
  MSG,
  remoteControlRegistry,
  packageWorkbenchResult,
  getBatchNoteCtrl,
  getBatchCommentCtrl,
} = {}) {
  return {
    [MSG.WORKBENCH_TASK_CONTROL]: async (msg = {}) => {
      const taskControl = msg.taskControl || {};
      const action = String(taskControl.action || msg.command || '').trim();

      if (!msg.taskControl) {
        applyLegacyBatchControl({ action, getBatchNoteCtrl, getBatchCommentCtrl });
        return {
          success: true,
          accepted: true,
          taskId: String(msg.taskId || '').trim(),
          controlAction: action,
        };
      }

      const control = remoteControlRegistry.findRemoteControl(msg);
      if (!control) {
        return {
          success: false,
          accepted: false,
          error: 'no_active_remote_control_task',
        };
      }

      return {
        ...control.apply(action),
        taskId: String(taskControl.taskId || msg.taskId || '').trim(),
        controlAction: action,
      };
    },

    [MSG.WORKBENCH_GET_RESULT_PACKAGE]: async (msg = {}) => {
      const collectionRunId = String(msg.collectionRunId || '').trim();
      const externalTaskId = String(msg.externalTaskId || '').trim();
      if (!collectionRunId && !externalTaskId) {
        return { success: false, error: 'collectionRunId or externalTaskId required' };
      }
      if (typeof packageWorkbenchResult !== 'function') {
        return { success: false, error: 'workbench result packager unavailable' };
      }
      const result = await packageWorkbenchResult({
        collectionRunId,
        externalTaskId,
      });
      return { success: true, result };
    },
  };
}
