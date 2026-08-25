import DouyinAdapter from '../platforms/douyin/index.js';
import { collectDouyinVideo, refreshDouyinNoteMediaById } from '../platforms/douyin/videoCollector.js';
import { collectDouyinAuthor } from '../platforms/douyin/authorCollector.js';
import { batchCollectDouyinProfileVideos, batchCollectDouyinProfileComments } from '../platforms/douyin/batchController.js';
import { discoverDouyinBatchTargets } from '../platforms/douyin/batchDiscovery.js';
import { collectDouyinComments, downloadDouyinCommentImages } from '../platforms/douyin/commentCollector.js';
import {
  detectDouyinPageType,
  detectDouyinSearchBatchContext,
  isStrictDouyinDetailPage,
  extractDouyinContentId as extractDouyinContentIdImpl,
} from '../platforms/douyin/pageDetector.js';
import { detectDouyinSecurityChallenge } from '../platforms/douyin/securityChallenge.js';

export {
  DouyinAdapter,
  collectDouyinVideo,
  refreshDouyinNoteMediaById,
  collectDouyinAuthor,
  batchCollectDouyinProfileVideos,
  batchCollectDouyinProfileComments,
  discoverDouyinBatchTargets,
  collectDouyinComments,
  downloadDouyinCommentImages,
  detectDouyinPageType,
  detectDouyinSearchBatchContext,
  detectDouyinSecurityChallenge,
  isStrictDouyinDetailPage,
};

export function extractDouyinContentId(url) {
  return extractDouyinContentIdImpl(url);
}
