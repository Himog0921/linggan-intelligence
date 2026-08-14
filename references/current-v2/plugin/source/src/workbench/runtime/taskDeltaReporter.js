import { createDeltaOutbox } from './deltaOutbox.js';

function normalizeConfigResult(value) {
  return value && typeof value === 'object' && !Array.isArray(value) ? value : {};
}

export function createTaskDeltaReporter({
  store,
  commitTaskDelta,
  getFlywheelConfig,
  prepareRecordPayload,
  shouldPollWorkbenchTasks = () => true,
  executorInstanceId = '',
  getExecutorInstanceId = null,
  autoFlush = true,
  requireExecutionIdentity = true,
  captureJournal = null,
  pluginVersion = '',
} = {}) {
  const outbox = createDeltaOutbox({
    store,
    executorInstanceId,
    getExecutorInstanceId,
    autoFlush,
    requireExecutionIdentity,
    captureJournal,
    pluginVersion,
    prepareRecordPayload: async (record) => {
      const config = typeof getFlywheelConfig === 'function'
        ? normalizeConfigResult(await getFlywheelConfig())
        : {};
      return typeof prepareRecordPayload === 'function'
        ? prepareRecordPayload(config, record)
        : record.payload;
    },
    commitDelta: async (taskId, envelope) => {
      const config = typeof getFlywheelConfig === 'function'
        ? normalizeConfigResult(await getFlywheelConfig())
        : {};
      if (!shouldPollWorkbenchTasks(config)) {
        const error = new Error('workbench_sync_disabled');
        error.retryable = true;
        throw error;
      }
      if (typeof commitTaskDelta !== 'function') {
        const error = new Error('workbench_commit_delta_missing');
        error.retryable = true;
        throw error;
      }
      return commitTaskDelta(config, taskId, envelope);
    },
  });

  return outbox;
}
