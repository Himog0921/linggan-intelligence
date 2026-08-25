/**
 * 补充探查：读取正确的 noteId 对应数据（修复 "undefined" key 问题）
 * 在同一个笔记详情页 Console 执行
 */
(function probeXhsNoteDataFix() {
  const state = window.__INITIAL_STATE__;
  const noteDetailMap = state?.note?.noteDetailMap;
  if (!noteDetailMap) return console.error('noteDetailMap 不存在');

  // 跳过 "undefined" key，取真实 noteId
  const realKeys = Object.keys(noteDetailMap).filter(k => k !== 'undefined');
  if (realKeys.length === 0) return console.error('无真实 noteId');

  const noteId = realKeys[0];
  const entry = noteDetailMap[noteId];

  // 检查数据结构
  const noteData = entry?.note || entry;
  const fields = {};
  const checkFields = [
    'title', 'desc', 'type', 'noteId', 'id', 'ipLocation',
    'lastUpdateTime', 'time', 'tagList', 'atUserList',
    'imageList', 'video', 'interactInfo', 'user', 'shareInfo'
  ];

  for (const f of checkFields) {
    const v = noteData?.[f];
    if (v == null || v === '') {
      fields[f] = 'MISSING';
    } else if (Array.isArray(v)) {
      fields[f] = { type: 'array', length: v.length, firstKeys: v[0] ? Object.keys(v[0]).slice(0, 6) : [] };
    } else if (typeof v === 'object') {
      fields[f] = { type: 'object', keys: Object.keys(v).slice(0, 10) };
    } else {
      fields[f] = { type: typeof v, value: String(v).slice(0, 120) };
    }
  }

  // 也检查 entry 顶层结构
  const output = {
    noteId,
    entryKeys: Object.keys(entry || {}).slice(0, 15),
    noteDataKeys: Object.keys(noteData || {}).slice(0, 30),
    fields,
    // Vue ref 检测
    isEntryRef: entry?.__v_isRef === true,
    hasEntryRawValue: entry?._rawValue !== undefined,
  };

  console.log('[probe-note-fix] JSON:', JSON.stringify(output, null, 2));
  return output;
})();
