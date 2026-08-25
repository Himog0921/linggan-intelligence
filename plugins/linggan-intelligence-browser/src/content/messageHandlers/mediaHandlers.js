import { COMMENT_DEPTH_MODE } from '../../shared/constants.js';

export function createMediaHandlers({
  MSG,
  isDouyinPage,
  ensurePluginAuthorized,
  createRemoteRun,
  finalizeRemoteRun,
  remoteControlRegistry,
  downloadDouyinCommentImages,
  noteStore,
  downloadNoteMediaFromRecord,
} = {}) {
  return {
    [MSG.DOWNLOAD_CURRENT_COMMENT_IMAGES]: async (msg = {}) => {
      await ensurePluginAuthorized();
      if (!isDouyinPage()) {
        throw new Error('当前页面暂不支持从 Popup 下载评论图片区');
      }
      const triggerSource = String(msg.triggerSource || 'popup_manual').trim() || 'popup_manual';
      const commentDepthMode = String(msg.commentDepthMode || COMMENT_DEPTH_MODE.TWO_LEVEL) === COMMENT_DEPTH_MODE.ALL_REPLIES
        ? COMMENT_DEPTH_MODE.ALL_REPLIES
        : COMMENT_DEPTH_MODE.TWO_LEVEL;
      const maxTotal = Math.max(0, Number(msg.maxTotal || 0) || 0);
      const maxSubComments = commentDepthMode === COMMENT_DEPTH_MODE.ALL_REPLIES ? 0 : Math.max(0, Number(msg.maxSubComments || 0) || 0);
      const createRemoteImageRun = () => createRemoteRun({
        platform: 'douyin',
        triggerSource,
        remoteTaskMeta: msg.externalTaskMeta,
        taskType: 'commentImageDownload',
        config: {
          maxTotal,
          maxSubComments,
          commentDepthMode,
        },
      });
      const runImageDownload = async (remoteRun = null) => {
        const controlBinding = remoteControlRegistry.bindRemoteControl({
          remoteRun,
          remoteTaskMeta: msg.externalTaskMeta,
        });
        try {
          const result = await downloadDouyinCommentImages({
            maxTotal,
            maxSubComments,
            commentDepthMode,
            collectionRunId: remoteRun?.collectionRunId || '',
            shouldStop: controlBinding.control.shouldStop,
            waitIfPaused: controlBinding.control.waitIfPaused,
          });
          const hasUsefulResult = Boolean(result?.success) || Number(result?.downloaded || 0) > 0 || Number(result?.total || 0) === 0;
          if (!hasUsefulResult && !result?.stopped) {
            throw new Error(result?.message || '评论图片区下载失败');
          }
          await finalizeRemoteRun(remoteRun, result?.stopped ? 'stopped' : 'done', {
            itemsPlanned: result.total || 0,
            itemsSucceeded: result.downloaded || 0,
            itemsFailed: result.failed || 0,
            totalComments: result.note?.totalComments || 0,
            totalImages: result.total || 0,
            scannedImages: result.scannedImages || result.total || 0,
            hdCount: result.hdCount || 0,
            sdCount: result.sdCount || 0,
            contentId: String(result?.note?.contentId || '').trim(),
            targetIds: [String(result?.note?.platformContentId || '').trim()].filter(Boolean),
            noImages: !result?.success && Number(result?.total || 0) === 0,
            zipName: String(result?.zipName || '').trim(),
          });
          return {
            success: true,
            stopped: Boolean(result?.stopped),
            total: result.total || 0,
            downloaded: result.downloaded || 0,
            failed: result.failed || 0,
            hdCount: result.hdCount || 0,
            sdCount: result.sdCount || 0,
            collectionRunId: remoteRun?.collectionRunId || result.collectionRunId || '',
          };
        } catch (error) {
          await finalizeRemoteRun(remoteRun, 'failed', {
            error: String(error?.message || error),
            itemsPlanned: 0,
            itemsSucceeded: 0,
            itemsFailed: 1,
          });
          throw error;
        } finally {
          controlBinding.release();
        }
      };

      if (msg.asyncDispatch) {
        const remoteRun = await createRemoteImageRun();
        Promise.resolve()
          .then(() => runImageDownload(remoteRun))
          .catch((error) => {
            console.error('[灵感爆爆爆] 远程抖音评论图片区下载失败:', error);
          });
        return {
          success: true,
          accepted: true,
          pending: true,
          collectionRunId: remoteRun?.collectionRunId || '',
        };
      }

      const remoteRun = await createRemoteImageRun();
      return runImageDownload(remoteRun);
    },

    [MSG.DOWNLOAD_NOTE_MEDIA]: async (msg) => {
      await ensurePluginAuthorized();
      const noteId = msg.noteId || '';
      if (!noteId) return { success: false, error: 'noteId required' };
      const note = await noteStore.getById(noteId);
      if (!note) return { success: false, error: 'note not found' };
      const summary = await downloadNoteMediaFromRecord(note, {
        mediaTypes: Array.isArray(msg.mediaTypes) ? msg.mediaTypes : undefined,
      });
      return { success: true, summary };
    },
  };
}
