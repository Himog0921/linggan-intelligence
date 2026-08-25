import { getUnifiedAuthorHandle } from '../../shared/utils.js';

const normalizeCsvValue = (value) => {
  if (value == null) return '';
  if (Array.isArray(value)) return JSON.stringify(value);
  if (typeof value === 'object') return JSON.stringify(value);
  return value;
};

export function createDataHandlers({
  MSG,
  ensurePluginAuthorized,
  noteStore,
  commentStore,
  authorStore,
  generateCsv,
  downloadFile,
  backfillLegacyAiReadyFields,
  getPageContext,
} = {}) {
  return {
    [MSG.GET_STATS]: async () => ({
      notes: await noteStore.count(),
      comments: await commentStore.count(),
      authors: await authorStore.count(),
    }),

    [MSG.GET_PAGE_CONTEXT]: async () => ({
      success: true,
      context: typeof getPageContext === 'function' ? await getPageContext() : null,
    }),

    [MSG.GET_ALL_NOTES]: async () => {
      await ensurePluginAuthorized();
      return noteStore.getAll();
    },
    [MSG.GET_ALL_COMMENTS]: async () => {
      await ensurePluginAuthorized();
      return commentStore.getAll();
    },
    [MSG.GET_ALL_AUTHORS]: async () => {
      await ensurePluginAuthorized();
      return authorStore.getAll();
    },

    [MSG.DELETE_NOTE]: (msg) => noteStore.deleteById(msg.noteId),
    [MSG.DELETE_COMMENT]: (msg) => commentStore.deleteById(msg.id),
    [MSG.DELETE_AUTHOR]: (msg) => authorStore.deleteById(msg.userId),

    [MSG.CLEAR_ALL_NOTES]: () => noteStore.clear(),
    [MSG.CLEAR_ALL_COMMENTS]: () => commentStore.clear(),
    [MSG.CLEAR_ALL_AUTHORS]: () => authorStore.clear(),

    [MSG.EXPORT_CSV]: async (msg) => {
      await ensurePluginAuthorized();
      const type = msg.type || 'notes';
      if (type === 'notes') {
        const notes = await noteStore.getAll();
        const headers = [
          'platform','contentId','platformContentId','noteId','url','canonicalUrl','title','bodyText','hashtags',
          'type','likes','collects','comments','shares','authorEntityId','authorId','authorName','releaseDate',
          'publishedAt','collectedAt','ipLocation','lastUpdateTime','topicIds','atUserList','shareRestricted',
          'authorFollowed','cover','videoDownloadUrl','mediaDownloadStatus','dataSource','triggerSource','shareShortUrl',
          'batchSelectionMode','batchRank','batchLikesSnapshot','searchKeyword','searchPageUrl',
          'collectionRunId','collectorVersion','rawSource','rawUrl','rawShareText','rawDomText','rawPayload',
          'dataQuality','qualityReason','sourceTier',
        ];
        const rows = notes.map((n) => headers.map((h) => normalizeCsvValue(n[h])));
        const csv = generateCsv(headers, rows);
        downloadFile(csv, `灵感爆爆爆_笔记_${Date.now()}.csv`);
      } else if (type === 'comments') {
        const comments = await commentStore.getAll();
        const headers = [
          'platform','commentEntityId','commentId','contentId','noteId','text','author','likes','profileUrl','location',
          'ipLocation','avatarUrl','authorId','parentCommentId','rootCommentId','level','searchKeyword','searchPageUrl',
          'replyToCommentId','replyToUserName','publishedAt','publishedAtText','collectedAt','sortMode','collectionRunId',
          'collectorVersion','rawSource','rawUrl','rawShareText','rawDomText','rawPayload',
          'dataQuality','qualityReason','sourceTier',
        ];
        const rows = comments.map((c) => headers.map((h) => normalizeCsvValue(c[h])));
        const csv = generateCsv(headers, rows);
        downloadFile(csv, `灵感爆爆爆_评论_${Date.now()}.csv`);
      } else if (type === 'authors') {
        const authors = await authorStore.getAll();
        const headers = [
          'platform','authorEntityId','platformAuthorId','userId','profileUrl','handle','secUserId','redId','name',
          'douyinId','fans','follows','interactions','location','description','ipLocation','accountStatus','followedByMe','collectedAt',
          'collectorVersion','rawSource','rawUrl','rawDomText','rawShareText','rawPayload',
          'dataQuality','qualityReason','sourceTier',
        ];
        const rows = authors.map((a) => headers.map((h) => {
          if (h === 'handle') return normalizeCsvValue(getUnifiedAuthorHandle(a));
          return normalizeCsvValue(a[h]);
        }));
        const csv = generateCsv(headers, rows);
        downloadFile(csv, `灵感爆爆爆_博主_${Date.now()}.csv`);
      }
      return { success: true };
    },

    [MSG.EXPORT_JSON]: async () => {
      await ensurePluginAuthorized();
      const data = {
        notes: await noteStore.getAll(),
        comments: await commentStore.getAll(),
        authors: await authorStore.getAll(),
        exportedAt: new Date().toISOString(),
      };
      const json = JSON.stringify(data, null, 2);
      downloadFile(json, `灵感爆爆爆_全部数据_${Date.now()}.json`, 'application/json');
      return { success: true };
    },

    [MSG.RUN_DATA_MAINTENANCE]: async () => {
      await ensurePluginAuthorized();
      if (typeof backfillLegacyAiReadyFields !== 'function') {
        throw new Error('数据维护能力未接入');
      }
      const stats = await backfillLegacyAiReadyFields();
      return { success: true, stats };
    },

    [MSG.TOGGLE_DASHBOARD]: async () => {
      await ensurePluginAuthorized();
      return { success: true, toggleDashboard: true };
    },

    getDocumentCookie: async () => {
      await ensurePluginAuthorized();
      return { success: true, cookieString: document.cookie || '' };
    },
  };
}
