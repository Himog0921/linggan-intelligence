/**
 * detail_probe 任务本地存储 URL 状态探查脚本
 *
 * 背景：
 *   方向 B（保持 detail 模式 + 打开签名详情页）能否成立，取决于插件本地 IndexedDB
 *   是否稳定保存了带 xsec_token 的签名链接（rawUrl / url / canonicalUrl / noteUrl）。
 *   detail_probe 任务的目标 noteId 往往是监控中心首次发现的、本地还没采过的新笔记，
 *   所以核心问题是：待采的 noteId 在本地有没有签名记录？
 *
 * 用法（只需跑 1 次，复制输出给 AI）：
 *   方式 1（推荐）：在小红书任一页面（插件已注入）的 DevTools Console 运行。
 *                   插件 IndexedDB 与页面同源（xiaohongshu.com），可直接读。
 *   方式 2：在插件 Dashboard 里运行（Dashboard 是 content script 用 iframe 开的，同源）。
 *
 *   如果你已知 detail_probe 任务卡住的具体 noteId（从工作台任务详情或 progress.txt 里能找到），
 *   把它填进 TARGET_NOTE_ID 再跑，能精确查这个 noteId 的本地签名状态。
 */
(function probeNoteStoredUrls() {
  const TARGET_NOTE_ID = ''; // 可选：填入 detail_probe 卡住的 noteId，精确查它的本地状态

  const DB_NAME = 'LingganBoomDB';
  const STORE = 'notes';

  async function readNotes() {
    if (!window.indexedDB) return { error: 'indexedDB_unavailable' };
    return new Promise((resolve) => {
      const req = indexedDB.open(DB_NAME);
      req.onerror = () => resolve({ error: `open_failed: ${req.error?.message || ''}` });
      req.onsuccess = () => {
        const db = req.result;
        if (!db.objectStoreNames.contains(STORE)) {
          resolve({ error: `no_store: ${STORE}` });
          db.close();
          return;
        }
        const tx = db.transaction(STORE, 'readonly');
        const store = tx.objectStore(STORE);
        const getAllReq = store.getAll();
        getAllReq.onsuccess = () => {
          db.close();
          resolve({ ok: true, rows: getAllReq.result || [] });
        };
        getAllReq.onerror = () => {
          db.close();
          resolve({ error: `getAll_failed: ${getAllReq.error?.message || ''}` });
        };
      };
    });
  }

  const TOKEN_PATTERN = /xsec_token=/i;

  function summarizeRow(row) {
    return {
      noteId: row.noteId || '',
      platform: row.platform || '',
      url: String(row.url || '').slice(0, 120),
      urlHasToken: TOKEN_PATTERN.test(row.url || ''),
      rawUrl: String(row.rawUrl || '').slice(0, 120),
      rawUrlHasToken: TOKEN_PATTERN.test(row.rawUrl || ''),
      canonicalUrl: String(row.canonicalUrl || '').slice(0, 120),
      canonicalUrlHasToken: TOKEN_PATTERN.test(row.canonicalUrl || ''),
      noteUrl: String(row.noteUrl || '').slice(0, 120),
      noteUrlHasToken: TOKEN_PATTERN.test(row.noteUrl || ''),
      collectedAt: row.collectedAt || row.createdAt || null,
    };
  }

  (async () => {
    const result = {
      probe: 'note-stored-urls',
      time: new Date().toISOString(),
      pageUrl: location.href,
      targetNoteId: TARGET_NOTE_ID || null,
    };

    const read = await readNotes();
    if (read.error) {
      result.error = read.error;
      console.warn('[probe-note-stored-urls] 读取失败：', read.error);
      console.log(JSON.stringify(result, null, 2));
      return;
    }

    const rows = read.rows;
    const targetRow = TARGET_NOTE_ID
      ? rows.find((r) => r.noteId === TARGET_NOTE_ID)
      : null;

    // 统计签名 URL 覆盖率
    let withRawToken = 0;
    let withAnyToken = 0;
    rows.forEach((r) => {
      if (TOKEN_PATTERN.test(r.rawUrl || '')) withRawToken += 1;
      if (TOKEN_PATTERN.test(`${r.rawUrl || ''} ${r.url || ''} ${r.canonicalUrl || ''} ${r.noteUrl || ''}`)) withAnyToken += 1;
    });

    result.totalNotes = rows.length;
    result.notesWithRawUrlToken = withRawToken;
    result.notesWithAnyTokenField = withAnyToken;
    result.tokenCoverage = rows.length > 0 ? `${withRawToken}/${rows.length} rawUrl 带 token，${withAnyToken}/${rows.length} 任一字段带 token` : '空表';

    if (targetRow) {
      result.targetRow = summarizeRow(targetRow);
      result.targetRowVerdict = result.targetRow.rawUrlHasToken || result.targetRow.urlHasToken
        ? '✓ 该 noteId 本地有签名记录，方向 B 可复用'
        : '✗ 该 noteId 本地无签名记录，方向 B 需走兜底';
    } else if (TARGET_NOTE_ID) {
      result.targetRow = null;
      result.targetRowVerdict = `✗ noteId ${TARGET_NOTE_ID} 不在本地库（detail_probe 待采新笔记通常都是这种情况）`;
    }

    // 最近 5 条样本
    const recent = [...rows]
      .sort((a, b) => (b.collectedAt || b.createdAt || 0) - (a.collectedAt || a.createdAt || 0))
      .slice(0, 5)
      .map(summarizeRow);
    result.recentSamples = recent;

    console.log('%c[probe-note-stored-urls] 本地签名 URL 状态', 'color:#7dd87a;font-weight:bold');
    console.log(JSON.stringify(result, null, 2));
    console.log('关键判断：');
    console.log('  1. tokenCoverage → 历史采过的笔记是否普遍保存了带 token 的 rawUrl');
    console.log('  2. targetRowVerdict → 当前 detail_probe 卡住的 noteId 本地有没有签名（通常没有，因为是新笔记）');
    console.log('请复制上面 JSON 给 AI。');
    return result;
  })();
})();
