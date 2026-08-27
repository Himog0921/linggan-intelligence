import { noteStore } from '../../db/noteStore.js';
import { emitCollectorReceipt } from '../../runtime/collectorReceiptSink.js';
import { collectComments } from './commentCollector.js';
import { buildXhsDetailCaptureReceipt, normalizeXhsDetailCommentLimit } from './captureReceipt.js';
import { collectNote } from './noteCollector.js';

function deliveryState(deliveries = []) {
  const states = deliveries.map((value) => String(value?.delivery || value || '')).filter(Boolean);
  if (states.length > 0 && states.every((state) => state === 'acknowledged')) return 'acknowledged';
  if (states.some((state) => state === 'rejected' || state === 'terminal')) return 'partial_delivery_failure';
  return 'pending';
}

export function aggregateCommentAndReplyDelivery(commentDelivery = {}) {
  const comments = String(commentDelivery?.delivery || 'unknown');
  const replies = String(commentDelivery?.replies?.delivery || '');
  if (!replies) return comments;
  if (comments === 'rejected' || comments === 'terminal' || replies === 'rejected' || replies === 'terminal') {
    return 'terminal';
  }
  if (comments === 'acknowledged' && replies === 'acknowledged') return 'acknowledged';
  return 'pending';
}

function detailDeliveryMessage(lanes = {}) {
  const laneSummary = [
    ['content', '笔记详情'],
    ['mediaSlots', '媒体观察'],
    ['comments', '评论与回复'],
  ].map(([key, label]) => {
    const value = String(lanes[key] || 'unknown');
    const status = value === 'acknowledged'
      ? '已接纳'
      : (value === 'pending' || value === 'queued' ? '待本机交付' : (value === 'rejected' || value === 'terminal' ? '未接纳' : '状态未知'));
    return `${label}：${status}`;
  }).join('；');

  return `Linggan 接纳回执：${laneSummary}`;
}

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
    const commentAndReplyDelivery = aggregateCommentAndReplyDelivery(commentResult.lingganDelivery);
    const state = deliveryState([
      note.lingganDelivery,
      note.lingganMediaDelivery,
      commentAndReplyDelivery,
    ]);
    const lanes = {
      content: note.lingganDelivery?.delivery || 'unknown',
      mediaSlots: note.lingganMediaDelivery?.delivery || 'unknown',
      comments: commentAndReplyDelivery,
    };
    note.lingganDetailPackageDelivery = {
      state,
      lanes,
      message: detailDeliveryMessage(lanes),
    };
    await noteStore.upsert(note);
  }

  return { note, comments: commentResult.comments, commentResult, receipt };
}
