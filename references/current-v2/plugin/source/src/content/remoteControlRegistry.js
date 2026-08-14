import { REMOTE_TASK_CONTROL_ACTION } from '../workbench/protocol/schema.js';

const normalizeControlKey = (value = '') => String(value || '').trim();

function createRemoteControlHandle() {
  let paused = false;
  let stopped = false;
  let pauseResolve = null;

  const releasePause = () => {
    if (pauseResolve) {
      pauseResolve();
      pauseResolve = null;
    }
  };

  return {
    shouldStop: () => stopped,
    async waitIfPaused() {
      while (paused && !stopped) {
        await new Promise((resolve) => {
          pauseResolve = resolve;
        });
      }
    },
    apply(action = '') {
      const normalizedAction = String(action || '').trim();
      if (normalizedAction === REMOTE_TASK_CONTROL_ACTION.PAUSE) {
        if (stopped) return { success: false, accepted: false, error: 'task_already_stopped' };
        paused = true;
        return { success: true, accepted: true, status: 'paused' };
      }
      if (normalizedAction === REMOTE_TASK_CONTROL_ACTION.RESUME) {
        if (stopped) return { success: false, accepted: false, error: 'task_already_stopped' };
        paused = false;
        releasePause();
        return { success: true, accepted: true, status: 'running' };
      }
      if (
        normalizedAction === REMOTE_TASK_CONTROL_ACTION.STOP
        || normalizedAction === REMOTE_TASK_CONTROL_ACTION.DELETE
      ) {
        stopped = true;
        paused = false;
        releasePause();
        return { success: true, accepted: true, status: 'stopping' };
      }
      return { success: false, accepted: false, error: 'unsupported_control_action' };
    },
  };
}

export function createRemoteControlRegistry() {
  const activeRemoteControls = new Map();

  function bindRemoteControl({ remoteRun = null, remoteTaskMeta = {} } = {}) {
    const control = createRemoteControlHandle();
    const keys = [
      remoteTaskMeta?.externalTaskId,
      remoteRun?.collectionRunId,
    ].map(normalizeControlKey).filter(Boolean);
    keys.forEach((key) => activeRemoteControls.set(key, control));

    return {
      control,
      release() {
        keys.forEach((key) => {
          if (activeRemoteControls.get(key) === control) {
            activeRemoteControls.delete(key);
          }
        });
      },
    };
  }

  function findRemoteControl(msg = {}) {
    const taskControl = msg.taskControl || {};
    const keys = [
      taskControl.taskId,
      taskControl.collectionRunId,
      msg.taskId,
      msg.collectionRunId,
    ].map(normalizeControlKey).filter(Boolean);
    for (const key of keys) {
      const control = activeRemoteControls.get(key);
      if (control) return control;
    }
    return null;
  }

  return {
    bindRemoteControl,
    findRemoteControl,
    createRemoteControlHandle,
  };
}
