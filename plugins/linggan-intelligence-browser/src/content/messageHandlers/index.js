import { createRemoteControlRegistry } from '../remoteControlRegistry.js';
import { createCollectionHandlers } from './collectionHandlers.js';
import { createDataHandlers } from './dataHandlers.js';
import { createMediaHandlers } from './mediaHandlers.js';
import { createWorkbenchHandlers } from './workbenchHandlers.js';

const normalizeRemoteTaskMeta = (meta = {}) => ({
  externalTaskId: String(meta.externalTaskId || '').trim(),
  externalTaskType: String(meta.externalTaskType || '').trim(),
  executorInstanceId: String(meta.executorInstanceId || '').trim(),
  protocolVersion: String(meta.protocolVersion || '').trim(),
  monitorMeta: meta.monitorMeta || null,
});

export function createContentMessageHandlers({
  MSG,
  assertPluginAuthorized,
  collectionRunStore,
  getPageContext,
  remoteControlRegistry = createRemoteControlRegistry(),
  ...deps
} = {}) {
  const ensurePluginAuthorized = async () => {
    if (typeof assertPluginAuthorized === 'function') {
      return assertPluginAuthorized();
    }
    return null;
  };

  const createRemoteRun = async ({
    platform,
    triggerSource,
    remoteTaskMeta,
    taskType,
    config = {},
    meta = {},
  } = {}) => {
    const externalTaskMeta = normalizeRemoteTaskMeta(remoteTaskMeta);
    if (!externalTaskMeta.externalTaskId || !collectionRunStore?.createRun) {
      return null;
    }
    const pageContext = typeof getPageContext === 'function' ? await getPageContext() : null;
    const pageType = String(pageContext?.pageType || pageContext?.mode || '').trim();
    const runMeta = {
      pageUrl: String(globalThis.window?.location?.href || '').trim(),
      ...meta,
    };
    if (remoteTaskMeta?.monitorMeta) {
      runMeta.monitorMeta = remoteTaskMeta.monitorMeta;
    }
    return collectionRunStore.createRun({
      externalTaskId: externalTaskMeta.externalTaskId,
      externalTaskType: externalTaskMeta.externalTaskType,
      executorInstanceId: externalTaskMeta.executorInstanceId,
      protocolVersion: externalTaskMeta.protocolVersion,
      platform: String(platform || pageContext?.platform || '').trim(),
      taskType: String(taskType || '').trim(),
      pageType,
      triggerSource: String(triggerSource || 'workbench_dispatch').trim() || 'workbench_dispatch',
      resultUploadStatus: 'pending_upload',
      lastHeartbeatAt: Date.now(),
      config,
      meta: runMeta,
    });
  };

  const finalizeRemoteRun = async (run, status, patch = {}) => {
    const runId = String(run?.collectionRunId || '').trim();
    if (!runId || !collectionRunStore) return;
    if (status === 'done') {
      await collectionRunStore.markDone(runId, patch);
      return;
    }
    if (status === 'stopped') {
      await collectionRunStore.markStopped(runId, patch);
      return;
    }
    if (status === 'failed') {
      await collectionRunStore.markFailed(runId, patch.error || '博主采集失败', patch);
    }
  };

  const handlerDeps = {
    MSG,
    ...deps,
    ensurePluginAuthorized,
    createRemoteRun,
    finalizeRemoteRun,
    remoteControlRegistry,
    collectionRunStore,
    getPageContext,
  };

  return {
    ...createCollectionHandlers(handlerDeps),
    ...createMediaHandlers(handlerDeps),
    ...createWorkbenchHandlers(handlerDeps),
    ...createDataHandlers(handlerDeps),
  };
}
