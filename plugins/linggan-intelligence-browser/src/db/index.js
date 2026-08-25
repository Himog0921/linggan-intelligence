import Dexie from 'dexie';

const db = new Dexie('LingganBoomDB');

db.version(1).stores({
  notes: 'noteId, url, title, type, authorId, authorName, likes, collects, comments, releaseDate, createdAt, syncStatus',
  comments: '++id, commentId, noteId, noteUrl, text, author, profileUrl, location, parentCommentId, createdAt, syncStatus',
  authors: 'userId, redId, name, fans, follows, interactions, createdAt, syncStatus',
});

// v2: 评论新增 likes 字段索引
db.version(2).stores({
  notes: 'noteId, url, title, type, authorId, authorName, likes, collects, comments, releaseDate, createdAt, syncStatus',
  comments: '++id, commentId, noteId, noteUrl, text, author, profileUrl, location, likes, parentCommentId, createdAt, syncStatus',
  authors: 'userId, redId, name, fans, follows, interactions, createdAt, syncStatus',
});

// v3: authors 新增 profileUrl 字段索引（用于主页链接展示与检索）
db.version(3).stores({
  notes: 'noteId, url, title, type, authorId, authorName, likes, collects, comments, releaseDate, createdAt, syncStatus',
  comments: '++id, commentId, noteId, noteUrl, text, author, profileUrl, location, likes, parentCommentId, createdAt, syncStatus',
  authors: 'userId, redId, name, profileUrl, fans, follows, interactions, createdAt, syncStatus',
});

// v4: 扩展探查报告新增字段索引（地域/关系/状态）
db.version(4).stores({
  notes: 'noteId, url, title, type, authorId, authorName, likes, collects, comments, releaseDate, ipLocation, lastUpdateTime, authorFollowed, shareRestricted, createdAt, syncStatus',
  comments: '++id, commentId, noteId, noteUrl, text, author, authorId, profileUrl, location, ipLocation, likes, parentCommentId, createdAt, syncStatus',
  authors: 'userId, redId, name, profileUrl, fans, follows, interactions, ipLocation, gender, accountStatus, followedByMe, createdAt, syncStatus',
});

// v5: 多平台支持 — 所有表新增 platform 字段索引
// platform 值: 'xhs' | 'douyin' | 'bilibili' | 'youtube' | 'reddit'
// 存量数据（v4 及以前）platform 字段为 undefined，视为 'xhs'（历史数据全为小红书）
db.version(5).stores({
  notes: 'noteId, platform, url, title, type, authorId, authorName, likes, collects, comments, releaseDate, ipLocation, lastUpdateTime, authorFollowed, shareRestricted, createdAt, syncStatus',
  comments: '++id, commentId, platform, noteId, noteUrl, text, author, authorId, profileUrl, location, ipLocation, likes, parentCommentId, createdAt, syncStatus',
  authors: 'userId, platform, redId, name, profileUrl, fans, follows, interactions, ipLocation, gender, accountStatus, followedByMe, createdAt, syncStatus',
});

// v6: AI-ready 数据契约基线
db.version(6).stores({
  notes: 'noteId, contentId, platformContentId, platform, url, title, type, authorId, authorEntityId, authorName, likes, collects, comments, releaseDate, publishedAt, collectedAt, ipLocation, lastUpdateTime, mediaDownloadStatus, dataSource, triggerSource, shareShortUrl, createdAt, syncStatus',
  comments: '++id, commentEntityId, commentId, platform, contentId, noteId, noteUrl, text, author, authorId, profileUrl, location, ipLocation, likes, parentCommentId, rootCommentId, level, replyToCommentId, replyToUserName, publishedAt, collectedAt, sortMode, collectionRunId, createdAt, syncStatus',
  authors: 'userId, authorEntityId, platformAuthorId, platform, handle, secUserId, redId, name, profileUrl, fans, follows, interactions, ipLocation, gender, accountStatus, followedByMe, collectedAt, createdAt, syncStatus',
  collectionRuns: 'collectionRunId, platform, taskType, pageType, triggerSource, status, startedAt, finishedAt, createdAt',
  mediaAssets: 'assetId, contentId, assetType, role, quality, downloadStatus, lastResolvedAt, createdAt',
});

// v7: mediaAssets 增加 collectionRunId 索引，支持按任务追溯媒体资产
db.version(7).stores({
  notes: 'noteId, contentId, platformContentId, platform, url, title, type, authorId, authorEntityId, authorName, likes, collects, comments, releaseDate, publishedAt, collectedAt, ipLocation, lastUpdateTime, mediaDownloadStatus, dataSource, triggerSource, shareShortUrl, createdAt, syncStatus',
  comments: '++id, commentEntityId, commentId, platform, contentId, noteId, noteUrl, text, author, authorId, profileUrl, location, ipLocation, likes, parentCommentId, rootCommentId, level, replyToCommentId, replyToUserName, publishedAt, collectedAt, sortMode, collectionRunId, createdAt, syncStatus',
  authors: 'userId, authorEntityId, platformAuthorId, platform, handle, secUserId, redId, name, profileUrl, fans, follows, interactions, ipLocation, gender, accountStatus, followedByMe, collectedAt, createdAt, syncStatus',
  collectionRuns: 'collectionRunId, platform, taskType, pageType, triggerSource, status, startedAt, finishedAt, createdAt',
  mediaAssets: 'assetId, contentId, collectionRunId, assetType, role, quality, downloadStatus, lastResolvedAt, createdAt',
});

// v8: collectionRuns 增加远程任务映射字段，支持工作台任务与本地执行记录对齐
db.version(8).stores({
  notes: 'noteId, contentId, platformContentId, platform, url, title, type, authorId, authorEntityId, authorName, likes, collects, comments, releaseDate, publishedAt, collectedAt, ipLocation, lastUpdateTime, mediaDownloadStatus, dataSource, triggerSource, shareShortUrl, createdAt, syncStatus',
  comments: '++id, commentEntityId, commentId, platform, contentId, noteId, noteUrl, text, author, authorId, profileUrl, location, ipLocation, likes, parentCommentId, rootCommentId, level, replyToCommentId, replyToUserName, publishedAt, collectedAt, sortMode, collectionRunId, createdAt, syncStatus',
  authors: 'userId, authorEntityId, platformAuthorId, platform, handle, secUserId, redId, name, profileUrl, fans, follows, interactions, ipLocation, gender, accountStatus, followedByMe, collectedAt, createdAt, syncStatus',
  collectionRuns: 'collectionRunId, externalTaskId, externalTaskType, executorInstanceId, protocolVersion, platform, taskType, pageType, triggerSource, status, resultUploadStatus, lastHeartbeatAt, startedAt, finishedAt, createdAt',
  mediaAssets: 'assetId, contentId, collectionRunId, assetType, role, quality, downloadStatus, lastResolvedAt, createdAt',
});

// v9: 工作台事件/记录增量 outbox，支持插件离线重试与幂等上传
db.version(9).stores({
  notes: 'noteId, contentId, platformContentId, platform, url, title, type, authorId, authorEntityId, authorName, likes, collects, comments, releaseDate, publishedAt, collectedAt, ipLocation, lastUpdateTime, mediaDownloadStatus, dataSource, triggerSource, shareShortUrl, createdAt, syncStatus',
  comments: '++id, commentEntityId, commentId, platform, contentId, noteId, noteUrl, text, author, authorId, profileUrl, location, ipLocation, likes, parentCommentId, rootCommentId, level, replyToCommentId, replyToUserName, publishedAt, collectedAt, sortMode, collectionRunId, createdAt, syncStatus',
  authors: 'userId, authorEntityId, platformAuthorId, platform, handle, secUserId, redId, name, profileUrl, fans, follows, interactions, ipLocation, gender, accountStatus, followedByMe, collectedAt, createdAt, syncStatus',
  collectionRuns: 'collectionRunId, externalTaskId, externalTaskType, executorInstanceId, protocolVersion, platform, taskType, pageType, triggerSource, status, resultUploadStatus, lastHeartbeatAt, startedAt, finishedAt, createdAt',
  mediaAssets: 'assetId, contentId, collectionRunId, assetType, role, quality, downloadStatus, lastResolvedAt, createdAt',
  workbenchOutbox: 'id, taskId, pluginRunId, idempotencyKey, kind, status, nextAttemptAt, createdAt',
});

// v10: workbenchOutbox 增加复合索引 [status+nextAttemptAt+createdAt]，避免 listPending 全表扫描
db.version(10).stores({
  notes: 'noteId, contentId, platformContentId, platform, url, title, type, authorId, authorEntityId, authorName, likes, collects, comments, releaseDate, publishedAt, collectedAt, ipLocation, lastUpdateTime, mediaDownloadStatus, dataSource, triggerSource, shareShortUrl, createdAt, syncStatus',
  comments: '++id, commentEntityId, commentId, platform, contentId, noteId, noteUrl, text, author, authorId, profileUrl, location, ipLocation, likes, parentCommentId, rootCommentId, level, replyToCommentId, replyToUserName, publishedAt, collectedAt, sortMode, collectionRunId, createdAt, syncStatus',
  authors: 'userId, authorEntityId, platformAuthorId, platform, handle, secUserId, redId, name, profileUrl, fans, follows, interactions, ipLocation, gender, accountStatus, followedByMe, collectedAt, createdAt, syncStatus',
  collectionRuns: 'collectionRunId, externalTaskId, externalTaskType, executorInstanceId, protocolVersion, platform, taskType, pageType, triggerSource, status, resultUploadStatus, lastHeartbeatAt, startedAt, finishedAt, createdAt',
  mediaAssets: 'assetId, contentId, collectionRunId, assetType, role, quality, downloadStatus, lastResolvedAt, createdAt',
  workbenchOutbox: 'id, taskId, pluginRunId, idempotencyKey, kind, status, nextAttemptAt, createdAt, [status+nextAttemptAt+createdAt]',
});

// v11: 采集账号管理表，支持多账号 Cookie 轮换与配额追踪
db.version(11).stores({
  notes: 'noteId, contentId, platformContentId, platform, url, title, type, authorId, authorEntityId, authorName, likes, collects, comments, releaseDate, publishedAt, collectedAt, ipLocation, lastUpdateTime, mediaDownloadStatus, dataSource, triggerSource, shareShortUrl, createdAt, syncStatus',
  comments: '++id, commentEntityId, commentId, platform, contentId, noteId, noteUrl, text, author, authorId, profileUrl, location, ipLocation, likes, parentCommentId, rootCommentId, level, replyToCommentId, replyToUserName, publishedAt, collectedAt, sortMode, collectionRunId, createdAt, syncStatus',
  authors: 'userId, authorEntityId, platformAuthorId, platform, handle, secUserId, redId, name, profileUrl, fans, follows, interactions, ipLocation, gender, accountStatus, followedByMe, collectedAt, createdAt, syncStatus',
  collectionRuns: 'collectionRunId, externalTaskId, externalTaskType, executorInstanceId, protocolVersion, platform, taskType, pageType, triggerSource, status, resultUploadStatus, lastHeartbeatAt, startedAt, finishedAt, createdAt',
  mediaAssets: 'assetId, contentId, collectionRunId, assetType, role, quality, downloadStatus, lastResolvedAt, createdAt',
  workbenchOutbox: 'id, taskId, pluginRunId, idempotencyKey, kind, status, nextAttemptAt, createdAt, [status+nextAttemptAt+createdAt]',
  accounts: 'accountId, name, status, platform, lastUsedAt, createdAt',
});

// v12: workbenchOutbox 添加 idempotencyKey 唯一索引，消除 enqueue 竞态条件
db.version(12).stores({
  notes: 'noteId, contentId, platformContentId, platform, url, title, type, authorId, authorEntityId, authorName, likes, collects, comments, releaseDate, publishedAt, collectedAt, ipLocation, lastUpdateTime, mediaDownloadStatus, dataSource, triggerSource, shareShortUrl, createdAt, syncStatus',
  comments: '++id, commentEntityId, commentId, platform, contentId, noteId, noteUrl, text, author, authorId, profileUrl, location, ipLocation, likes, parentCommentId, rootCommentId, level, replyToCommentId, replyToUserName, publishedAt, collectedAt, sortMode, collectionRunId, createdAt, syncStatus',
  authors: 'userId, authorEntityId, platformAuthorId, platform, handle, secUserId, redId, name, profileUrl, fans, follows, interactions, ipLocation, gender, accountStatus, followedByMe, collectedAt, createdAt, syncStatus',
  collectionRuns: 'collectionRunId, externalTaskId, externalTaskType, executorInstanceId, protocolVersion, platform, taskType, pageType, triggerSource, status, resultUploadStatus, lastHeartbeatAt, startedAt, finishedAt, createdAt',
  mediaAssets: 'assetId, contentId, collectionRunId, assetType, role, quality, downloadStatus, lastResolvedAt, createdAt',
  workbenchOutbox: 'id, taskId, pluginRunId, &idempotencyKey, kind, status, nextAttemptAt, createdAt, [status+nextAttemptAt+createdAt]',
  accounts: 'accountId, name, status, platform, lastUsedAt, createdAt',
}).upgrade(async (tx) => {
  const rows = await tx.table('workbenchOutbox').toArray();
  const seen = new Map();
  const duplicates = [];
  for (const row of rows) {
    const key = row.idempotencyKey;
    if (!key) continue;
    if (seen.has(key)) {
      duplicates.push(row.id);
    } else {
      seen.set(key, row.id);
    }
  }
  if (duplicates.length > 0) {
    await tx.table('workbenchOutbox').bulkDelete(duplicates);
  }
});

// v13: notes/authors 增加 collectionRunId 索引，避免按任务打包结果时全表扫描
db.version(13).stores({
  notes: 'noteId, contentId, platformContentId, platform, collectionRunId, url, title, type, authorId, authorEntityId, authorName, likes, collects, comments, releaseDate, publishedAt, collectedAt, ipLocation, lastUpdateTime, mediaDownloadStatus, dataSource, triggerSource, shareShortUrl, createdAt, syncStatus',
  comments: '++id, commentEntityId, commentId, platform, contentId, noteId, noteUrl, text, author, authorId, profileUrl, location, ipLocation, likes, parentCommentId, rootCommentId, level, replyToCommentId, replyToUserName, publishedAt, collectedAt, sortMode, collectionRunId, createdAt, syncStatus',
  authors: 'userId, authorEntityId, platformAuthorId, platform, collectionRunId, handle, secUserId, redId, name, profileUrl, fans, follows, interactions, ipLocation, gender, accountStatus, followedByMe, collectedAt, createdAt, syncStatus',
  collectionRuns: 'collectionRunId, externalTaskId, externalTaskType, executorInstanceId, protocolVersion, platform, taskType, pageType, triggerSource, status, resultUploadStatus, lastHeartbeatAt, startedAt, finishedAt, createdAt',
  mediaAssets: 'assetId, contentId, collectionRunId, assetType, role, quality, downloadStatus, lastResolvedAt, createdAt',
  workbenchOutbox: 'id, taskId, pluginRunId, &idempotencyKey, kind, status, nextAttemptAt, createdAt, [status+nextAttemptAt+createdAt]',
  accounts: 'accountId, name, status, platform, lastUsedAt, createdAt',
});

// v14: captureJournal 采集事实账本（2026-07-13 事故架构升级）。
// 采集内容先落不可变账本，与租约是否仍有效解耦；即使出站行被判死信，
// 采集事实仍可经 plugin_local_recovery 显式恢复导入，不会无声丢失。
db.version(14).stores({
  notes: 'noteId, contentId, platformContentId, platform, collectionRunId, url, title, type, authorId, authorEntityId, authorName, likes, collects, comments, releaseDate, publishedAt, collectedAt, ipLocation, lastUpdateTime, mediaDownloadStatus, dataSource, triggerSource, shareShortUrl, createdAt, syncStatus',
  comments: '++id, commentEntityId, commentId, platform, contentId, noteId, noteUrl, text, author, authorId, profileUrl, location, ipLocation, likes, parentCommentId, rootCommentId, level, replyToCommentId, replyToUserName, publishedAt, collectedAt, sortMode, collectionRunId, createdAt, syncStatus',
  authors: 'userId, authorEntityId, platformAuthorId, platform, collectionRunId, handle, secUserId, redId, name, profileUrl, fans, follows, interactions, ipLocation, gender, accountStatus, followedByMe, collectedAt, createdAt, syncStatus',
  collectionRuns: 'collectionRunId, externalTaskId, externalTaskType, executorInstanceId, protocolVersion, platform, taskType, pageType, triggerSource, status, resultUploadStatus, lastHeartbeatAt, startedAt, finishedAt, createdAt',
  mediaAssets: 'assetId, contentId, collectionRunId, assetType, role, quality, downloadStatus, lastResolvedAt, createdAt',
  workbenchOutbox: 'id, taskId, pluginRunId, &idempotencyKey, kind, status, nextAttemptAt, createdAt, [status+nextAttemptAt+createdAt]',
  accounts: 'accountId, name, status, platform, lastUsedAt, createdAt',
  captureJournal: 'entryId, taskId, capturedAt, payloadHash',
});

// v15: 诊断和接单闸门需要频繁读取各状态最老记录。增加复合索引后可直接
// 取每个状态的首行，不再反序列化包含 payload/snapshot 的全量 outbox。
db.version(15).stores({
  workbenchOutbox: 'id, taskId, pluginRunId, &idempotencyKey, kind, status, nextAttemptAt, createdAt, [status+nextAttemptAt+createdAt], [status+createdAt]',
});

export default db;
