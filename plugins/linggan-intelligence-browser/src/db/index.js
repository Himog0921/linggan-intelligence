import Dexie from 'dexie';

// Browser-local convenience cache only. It deliberately has no Workbench
// schema, remote task identity, cookie, outbox, or fallback-delivery table.
// Durable Linggan delivery has its own bounded stores under src/linggan/.
const db = new Dexie('LingganIntelligenceBrowserLocalCacheV2');

db.version(1).stores({
  notes: 'noteId, platform, url, title, type, authorId, authorName, likes, collects, comments, releaseDate, createdAt, syncStatus',
  comments: '++id, commentId, platform, noteId, noteUrl, text, author, authorId, profileUrl, location, likes, parentCommentId, createdAt, syncStatus',
  authors: 'userId, platform, redId, name, profileUrl, fans, follows, interactions, createdAt, syncStatus',
  collectionRuns: 'collectionRunId, platform, taskType, pageType, triggerSource, status, startedAt, finishedAt, createdAt',
  mediaAssets: 'assetId, contentId, collectionRunId, assetType, role, quality, downloadStatus, lastResolvedAt, createdAt',
});

export default db;
