import db from './index.js';
import { normalizeNoteRecord } from './recordNormalization.js';

export const noteStore = {
  async upsert(note) {
    await db.notes.put(normalizeNoteRecord(note));
  },

  async bulkUpsert(notes) {
    if (!notes || notes.length === 0) return;
    await db.transaction('rw', db.notes, async () => {
      await db.notes.bulkPut(notes.map(normalizeNoteRecord));
    });
  },

  async getAll() {
    const notes = await db.notes.orderBy('createdAt').reverse().toArray();
    return notes.map(normalizeNoteRecord);
  },

  async getPage({ offset = 0, limit = 100 } = {}) {
    const safeOffset = Math.max(0, Number(offset || 0));
    const safeLimit = Math.max(1, Number(limit || 100));
    const notes = await db.notes
      .orderBy('createdAt')
      .reverse()
      .offset(safeOffset)
      .limit(safeLimit)
      .toArray();
    return notes.map(normalizeNoteRecord);
  },

  async getById(noteId) {
    const note = await db.notes.get(noteId);
    return note ? normalizeNoteRecord(note) : null;
  },

  async getByCollectionRunId(collectionRunId) {
    const id = String(collectionRunId || '').trim();
    if (!id) return [];
    const notes = await db.notes
      .where('collectionRunId').equals(id)
      .toArray();
    return notes.map(normalizeNoteRecord);
  },

  async updateById(noteId, patch) {
    const existing = await db.notes.get(noteId);
    await db.notes.put(normalizeNoteRecord({
      ...(existing || {}),
      ...(patch || {}),
      noteId,
    }));
  },

  async search(keyword) {
    const notes = await db.notes
      .filter(n => n.title?.includes(keyword) || n.content?.includes(keyword) || n.authorName?.includes(keyword))
      .toArray();
    return notes.map(normalizeNoteRecord);
  },

  async filterByType(type) {
    const notes = await db.notes.where('type').equals(type).toArray();
    return notes.map(normalizeNoteRecord);
  },

  async deleteById(noteId) {
    await db.notes.delete(noteId);
  },

  async clear() {
    await db.notes.clear();
  },

  async count() {
    return db.notes.count();
  },
};
