import db from './index.js';
import { normalizeAuthorRecord } from './recordNormalization.js';

function getUnifiedAuthorHandle(author = {}) {
  return normalizeAuthorRecord(author).handle;
}

export const authorStore = {
  async upsert(author) {
    await db.authors.put(normalizeAuthorRecord(author));
  },

  async bulkUpsert(authors) {
    if (!authors || authors.length === 0) return;
    await db.transaction('rw', db.authors, async () => {
      await db.authors.bulkPut(authors.map(normalizeAuthorRecord));
    });
  },

  async getAll() {
    const authors = await db.authors.orderBy('createdAt').reverse().toArray();
    return authors.map(normalizeAuthorRecord);
  },

  async getPage({ offset = 0, limit = 100 } = {}) {
    const safeOffset = Math.max(0, Number(offset || 0));
    const safeLimit = Math.max(1, Number(limit || 100));
    const authors = await db.authors
      .orderBy('createdAt')
      .reverse()
      .offset(safeOffset)
      .limit(safeLimit)
      .toArray();
    return authors.map(normalizeAuthorRecord);
  },

  async getById(userId) {
    const author = await db.authors.get(userId);
    return author ? normalizeAuthorRecord(author) : null;
  },

  async getByCollectionRunId(collectionRunId) {
    const id = String(collectionRunId || '').trim();
    if (!id) return [];
    const authors = await db.authors
      .where('collectionRunId').equals(id)
      .toArray();
    return authors.map(normalizeAuthorRecord);
  },

  async search(keyword) {
    const authors = await db.authors
      .filter(a =>
        a.name?.includes(keyword)
        || getUnifiedAuthorHandle(a).includes(keyword)
        || a.redId?.includes(keyword)
        || a.handle?.includes(keyword)
        || a.douyinId?.includes(keyword)
        || a.description?.includes(keyword)
      )
      .toArray();
    return authors.map(normalizeAuthorRecord);
  },

  async deleteById(userId) {
    await db.authors.delete(userId);
  },

  async clear() {
    await db.authors.clear();
  },

  async count() {
    return db.authors.count();
  },
};
