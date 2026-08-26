import { noteStore } from '../../db/noteStore.js';
import { emitCollectorReceipt } from '../../runtime/collectorReceiptSink.js';
import { collectComments } from './commentCollector.js';
import { buildXhsDetailCaptureReceipt, normalizeXhsDetailCommentLimit } from './captureReceipt.js';
import { collectNote } from './noteCollector.js';

// This composes one user-visible collection intent. Delivery still uses the established
// detail/media/comment lanes, but all three are produced only after the single package has an
// explicit comment-coverage receipt.
export async function collectXhsNoteDetailPackage(wd = window, options = {}) {
  const commentLimit = normalizeXhsDetailCommentLimit(options.commentLimit);
  const includeComments = options.includeComments !== false;
  const note = await collectNote(wd, {
    ...options,
    deferLingganDelivery: true,
  });

  let commentResult = {
    total: 0,
    comments: [],
    stopReason: includeComments ? 'unknown' : 'comments_not_requested',
  };
  if (includeComments) {
    try {
      commentResult = await collectComments({
        noteId: note.noteId,
        noteUrl: note.url,
        maxTotal: commentLimit,
        maxSubComments: options.maxSubComments,
        commentDepthMode: options.commentDepthMode,
        collectionRunId: options.collectionRunId,
        shouldStop: options.shouldStop,
        waitIfPaused: options.waitIfPaused,
        onProgress: options.onCommentProgress,
        persist: options.persistComments !== false,
        emitReceipt: false,
      });
    } catch (error) {
      commentResult = {
        total: 0,
        comments: [],
        stopReason: 'comment_collection_failed',
        error: String(error?.message || error || 'comment_collection_failed'),
      };
    }
  }

  const receipt = buildXhsDetailCaptureReceipt({
    note,
    commentResult,
    requestedCommentLimit: commentLimit,
  });
  note.detailCapture = receipt;
  Object.defineProperty(note, '__xhsDetailPackage', {
    value: { comments: commentResult.comments, commentResult, receipt },
    enumerable: false,
    configurable: true,
  });
  await noteStore.upsert(note);

  if (options.deferLingganDelivery !== true) {
    note.lingganDelivery = await emitCollectorReceipt('contentDetail', note, { platform: 'xhs', options });
    note.lingganMediaDelivery = await emitCollectorReceipt('mediaSlots', note, { platform: 'xhs', options });
    commentResult.lingganDelivery = await emitCollectorReceipt('comments', commentResult, {
      platform: 'xhs',
      noteId: note.noteId,
      options: { maxTotal: commentLimit, maxSubComments: options.maxSubComments, commentDepthMode: options.commentDepthMode },
    });
  }

  return { note, comments: commentResult.comments, commentResult, receipt };
}
