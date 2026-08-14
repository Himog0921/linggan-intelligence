import db from './index.js';
import { normalizeCommentRecord } from './recordNormalization.js';

export const commentStore = {
  async upsert(comment) {
    const normalizedComment = normalizeCommentRecord(comment);
    // 按 commentId + noteId + platform 去重
    const existing = await db.comments
      .where('commentId').equals(normalizedComment.commentId)
      .and(c => c.noteId === normalizedComment.noteId && String(c.platform || 'xhs') === String(normalizedComment.platform || 'xhs'))
      .first();
    if (existing) {
      await db.comments.update(existing.id, normalizedComment);
    } else {
      await db.comments.add(normalizedComment);
    }
  },

  async bulkUpsert(comments) {
    if (!comments || comments.length === 0) return;

    await db.transaction('rw', db.comments, async () => {
      // 1. Normalize and deduplicate within the batch (keep last by commentId + noteId + platform)
      const seen = new Map();
      for (const comment of comments) {
        const normalized = normalizeCommentRecord(comment);
        const key = `${normalized.commentId}|${normalized.noteId}|${String(normalized.platform || 'xhs')}`;
        seen.set(key, normalized);
      }
      const uniqueComments = Array.from(seen.values());

      // 2. Query existing records by commentId
      const commentIds = uniqueComments.map(c => c.commentId);
      const existingRecords = await db.comments.where('commentId').anyOf(commentIds).toArray();

      const existingMap = new Map();
      for (const rec of existingRecords) {
        const key = `${rec.commentId}|${rec.noteId}|${String(rec.platform || 'xhs')}`;
        existingMap.set(key, rec);
      }

      const toAdd = [];
      const toUpdate = [];

      for (const comment of uniqueComments) {
        const key = `${comment.commentId}|${comment.noteId}|${String(comment.platform || 'xhs')}`;
        const existing = existingMap.get(key);
        if (existing) {
          toUpdate.push({ ...comment, id: existing.id });
        } else {
          toAdd.push(comment);
        }
      }

      if (toAdd.length > 0) {
        await db.comments.bulkAdd(toAdd);
      }
      if (toUpdate.length > 0) {
        await db.comments.bulkPut(toUpdate);
      }
    });
  },

  async getAll() {
    const comments = await db.comments.orderBy('createdAt').reverse().toArray();
    return comments.map(normalizeCommentRecord);
  },

  async getPage({ offset = 0, limit = 100 } = {}) {
    const safeOffset = Math.max(0, Number(offset || 0));
    const safeLimit = Math.max(1, Number(limit || 100));
    const comments = await db.comments
      .orderBy('createdAt')
      .reverse()
      .offset(safeOffset)
      .limit(safeLimit)
      .toArray();
    return comments.map(normalizeCommentRecord);
  },

  async getByNoteId(noteId) {
    const comments = await db.comments.where('noteId').equals(noteId).toArray();
    return comments.map(normalizeCommentRecord);
  },

  async getByCollectionRunId(collectionRunId) {
    const id = String(collectionRunId || '').trim();
    if (!id) return [];
    const comments = await db.comments.where('collectionRunId').equals(id).toArray();
    return comments.map(normalizeCommentRecord);
  },

  async search(keyword) {
    const comments = await db.comments
      .filter(c => c.text?.includes(keyword) || c.author?.includes(keyword))
      .toArray();
    return comments.map(normalizeCommentRecord);
  },

  async filterByLocation(location) {
    const comments = await db.comments
      .filter(c => c.location?.includes(location))
      .toArray();
    return comments.map(normalizeCommentRecord);
  },

  async deleteById(id) {
    await db.comments.delete(id);
  },

  async clear() {
    await db.comments.clear();
  },

  async count() {
    return db.comments.count();
  },
};
