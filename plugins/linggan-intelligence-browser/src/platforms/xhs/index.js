/**
 * XHS Platform — 小红书平台模块统一导出
 */

// 导入所有 XHS 模块
export { collectNote, discoverNotesFromDOM, discoverWithScroll } from './noteCollector.js';
export { collectComments, collectCommentImages } from './commentCollector.js';
export { collectAuthor } from './authorCollector.js';
export { BatchNoteController, BatchCommentController } from './batchController.js';
export {
  injectUI,
  toggleStopButton,
  togglePauseResumeButtons,
  showToast,
  showCommentLimitDialog,
  showMediaDownloadDialog,
  showBatchSettingsDialog,
  ensureTaskControlBar,
  updateTaskControlBar,
  hideTaskControlBar,
} from './uiInjector.js';

export default {
  id: 'xhs',
  hostPattern: /xiaohongshu\.com/,
};
