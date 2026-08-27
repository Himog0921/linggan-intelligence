import { createNoteMediaDownloadService } from './noteMediaDownload.js';
import { createContentMessageHandlers } from './messageHandlers.js';
import { createDashboardBridge } from './dashboardBridge.js';
import { noteStore } from '../db/noteStore.js';
import { commentStore } from '../db/commentStore.js';
import { authorStore } from '../db/authorStore.js';
import { collectionRunStore } from '../db/collectionRunStore.js';
import { mediaAssetStore } from '../db/mediaAssetStore.js';
import { backfillLegacyAiReadyFields } from '../db/legacyDataMaintenance.js';
import { createResultPackager } from '../workbench/runtime/resultPackager.js';
import { createPlatformAdapterRegistry, PLATFORM_ID } from '../platforms/registry.js';

export function createContentDataRuntime({
  MSG,
  isDouyinPage,
  assertPluginAuthorized,
  collectNote,
  collectComments,
  collectAuthor,
  collectDouyinVideo,
  collectDouyinComments,
  downloadDouyinCommentImages,
  collectDouyinAuthor,
  BatchNoteController,
  reportDone,
  reportProgress,
  reportWorkbenchRecord,
  batchMessageHandlers,
  getBatchNoteCtrl,
  getBatchCommentCtrl,
  extractNoteId,
  generateCsv,
  downloadFile,
  sendToBackground,
  loadDouyinRuntime,
  discoverXhsSurfaceNotes,
  discoverDouyinSurfaceTargets,
} = {}) {
  const basePlatformRegistry = createPlatformAdapterRegistry();
  const { downloadNoteMediaFromRecord } = createNoteMediaDownloadService({
    MSG,
    noteStore,
    sendToBackground,
    collectNote,
    loadDouyinRuntime,
    extractNoteId,
  });

  const dashboardBridge = createDashboardBridge({
    MSG,
    noteStore,
    commentStore,
    authorStore,
    downloadNoteMediaFromRecord,
  });

  const resultPackager = createResultPackager({
    collectionRunStore,
    noteStore,
    commentStore,
    authorStore,
    mediaAssetStore,
  });

  async function getPageContext() {
    if (isDouyinPage()) {
      const douyinRuntime = await loadDouyinRuntime();
      const registry = createPlatformAdapterRegistry({
        douyin: {
          detectDouyinPageType: douyinRuntime.detectDouyinPageType,
          detectDouyinSearchBatchContext: douyinRuntime.detectDouyinSearchBatchContext,
          detectDouyinSecurityChallenge: douyinRuntime.detectDouyinSecurityChallenge,
          isStrictDouyinDetailPage: douyinRuntime.isStrictDouyinDetailPage,
          getWindow: () => window,
          getDocument: () => document,
        },
      });
      return registry.require(PLATFORM_ID.DOUYIN).checkCapability({}, { win: window, root: document });
    }

    return basePlatformRegistry.require(PLATFORM_ID.XHS).checkCapability({}, { win: window });
  }

  const messageHandlers = createContentMessageHandlers({
    MSG,
    isDouyinPage,
    assertPluginAuthorized,
    collectNote,
    collectComments,
    collectAuthor,
    collectDouyinVideo,
    collectDouyinComments,
    downloadDouyinCommentImages,
    collectDouyinAuthor,
    BatchNoteController,
    noteStore,
    commentStore,
    authorStore,
    reportDone,
    reportProgress,
    reportWorkbenchRecord,
    batchMessageHandlers,
    getBatchNoteCtrl,
    getBatchCommentCtrl,
    extractNoteId,
    downloadNoteMediaFromRecord,
    generateCsv,
    downloadFile,
    backfillLegacyAiReadyFields,
    getPageContext,
    collectionRunStore,
    discoverXhsSurfaceNotes,
    discoverDouyinSurfaceTargets,
    packageWorkbenchResult: async ({ collectionRunId = '', externalTaskId = '' } = {}) => {
      if (collectionRunId) {
        return resultPackager.packageByCollectionRunId(collectionRunId);
      }
      return resultPackager.packageByExternalTaskId(externalTaskId);
    },
  });

  return {
    noteStore,
    commentStore,
    authorStore,
    downloadNoteMediaFromRecord,
    dashboardBridge,
    messageHandlers,
  };
}
