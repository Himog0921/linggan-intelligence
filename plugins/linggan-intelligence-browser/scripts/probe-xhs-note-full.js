/**
 * 小红书笔记页全量探查 v1.0（2026-04-18）
 *
 * 在任意小红书笔记详情页（/explore/xxx 或 /discovery/item/xxx）DevTools Console 执行
 *
 * 覆盖：
 * 1. __INITIAL_STATE__ 路径结构验证
 * 2. 笔记字段完整性
 * 3. 评论区 DOM 选择器全量验证
 * 4. 评论正文提取逻辑可靠性
 * 5. 媒体资源可达性
 */
(function probeXhsNoteFull() {
  const PROBE_VERSION = '1.0';

  function short(v, max = 160) {
    const t = String(v ?? '');
    return t.length > max ? t.slice(0, max) + '...' : t;
  }

  function selectorCheck(sel, context) {
    const root = context || document;
    const el = root.querySelector(sel);
    if (!el) return { found: false, text: '', tag: '', classes: '', html: '' };
    return {
      found: true,
      text: short((el.textContent || '').trim()),
      tag: el.tagName,
      classes: short(el.className || ''),
      html: short((el.outerHTML || '').replace(/\s+/g, ' '), 200),
    };
  }

  function selectorCountCheck(sel, context) {
    const root = context || document;
    const els = root.querySelectorAll(sel);
    return {
      count: els.length,
      samples: Array.from(els).slice(0, 3).map(el => ({
        tag: el.tagName,
        text: short((el.textContent || '').trim(), 80),
        classes: short(el.className || ''),
      })),
    };
  }

  // ============ 1. __INITIAL_STATE__ ============
  const state = window.__INITIAL_STATE__ || null;
  const noteModule = state?.note || null;
  const noteDetailMap = noteModule?.noteDetailMap || null;

  let noteData = null;
  let noteId = '';
  if (noteDetailMap) {
    // 跳过 "undefined" 等非真实 noteId 的 key
    const keys = Object.keys(noteDetailMap).filter(k => k !== 'undefined' && /^[a-f0-9]{24}$/.test(k));
    if (keys.length > 0) {
      noteId = keys[0];
      noteData = noteDetailMap[noteId]?.note || noteDetailMap[noteId];
    } else {
      // 兜底：取第一个非 "undefined" key
      const fallbackKeys = Object.keys(noteDetailMap).filter(k => k !== 'undefined');
      if (fallbackKeys.length > 0) {
        noteId = fallbackKeys[0];
        noteData = noteDetailMap[noteId]?.note || noteDetailMap[noteId];
      }
    }
  }

  // Vue ref 拆包检测
  function detectVueRef(obj, label) {
    if (!obj) return { isRef: false, hasRawValue: false, label };
    const hasRaw = '_rawValue' in obj;
    const hasRef = '__v_isRef' in obj;
    return { isRef: hasRef, hasRawValue: hasRaw, label, keys: Object.keys(obj).slice(0, 10) };
  }

  const stateCheck = {
    hasState: Boolean(state),
    stateTopKeys: state ? Object.keys(state).slice(0, 20) : [],
    hasNoteModule: Boolean(noteModule),
    noteModuleKeys: noteModule ? Object.keys(noteModule).slice(0, 10) : [],
    hasDetailMap: Boolean(noteDetailMap),
    detailMapKeys: noteDetailMap ? Object.keys(noteDetailMap) : [],
    noteIdFound: noteId,
    noteRefCheck: detectVueRef(noteDetailMap?.[noteId], 'noteDetailMap[noteId]'),
    noteDataKeys: noteData ? Object.keys(noteData).slice(0, 30) : [],
  };

  // ============ 2. 笔记字段完整性 ============
  const fields = {};
  if (noteData) {
    const fieldMap = {
      title: noteData.title,
      desc: noteData.desc,
      type: noteData.type,
      noteId: noteData.noteId || noteData.id,
      ipLocation: noteData.ipLocation,
      lastUpdateTime: noteData.lastUpdateTime,
      time: noteData.time,
      shareInfo: noteData.shareInfo,
      tagList: noteData.tagList,
      atUserList: noteData.atUserList,
      imageList: noteData.imageList,
      video: noteData.video,
    };
    for (const [k, v] of Object.entries(fieldMap)) {
      if (v == null || v === '') {
        fields[k] = { status: 'missing', value: null };
      } else if (Array.isArray(v)) {
        fields[k] = { status: 'ok', type: 'array', length: v.length, sample: short(JSON.stringify(v[0]), 120) };
      } else if (typeof v === 'object') {
        fields[k] = { status: 'ok', type: 'object', keys: Object.keys(v).slice(0, 10) };
      } else {
        fields[k] = { status: 'ok', type: typeof v, value: short(v) };
      }
    }
    // 互动数据
    const interact = noteData.interactInfo || {};
    fields.interactInfo = {
      status: interact ? 'ok' : 'missing',
      liked: interact.liked,
      likedCount: interact.likedCount,
      collectedCount: interact.collectedCount,
      commentCount: interact.commentCount,
      shareCount: interact.shareCount,
      followed: interact.followed,
    };
    // 作者信息
    const author = noteData.user || {};
    fields.author = {
      status: author.userId ? 'ok' : 'missing',
      userId: author.userId,
      nickname: short(author.nickname || author.name || ''),
      avatar: short(author.avatar || author.imageb || '', 100),
    };
  }

  // ============ 3. 评论区 DOM 选择器验证 ============
  const commentSelectors = {
    container: selectorCheck('.comments-container'),
    parentComment: selectorCountCheck('.parent-comment'),
    mainCommentItem: selectorCountCheck('.comment-item:not(.comment-item-sub)'),
    subCommentItem: selectorCountCheck('.comment-item.comment-item-sub'),
    commentAuthor: selectorCountCheck('a.name'),
    commentLikeCount: selectorCountCheck('.like-wrapper .count'),
    commentLikeFallback: selectorCountCheck('[class*="like"] .count'),
    commentIpLocation: selectorCountCheck('.date .location'),
    commentAvatar: selectorCountCheck('.avatar img.avatar-item'),
    commentAvatarFallback: selectorCountCheck('.avatar img'),
    commentAuthorId: selectorCountCheck('.avatar a[data-user-id]'),
    showMoreBtn: selectorCountCheck('div.show-more'),
    replyBtn: selectorCountCheck('span.count'),
    commentImage: selectorCountCheck('.comment-item img'),
  };

  // ============ 4. 评论正文提取逻辑验证 ============
  // 模拟 commentCollector.js 的 span:not([class]) 逻辑
  const commentTextTest = { tested: 0, succeeded: 0, failed: 0, samples: [] };
  const firstComment = document.querySelector('.comment-item:not(.comment-item-sub)');
  if (firstComment) {
    const innerContainer = firstComment.querySelector('.comment-inner-container') || firstComment;
    const spans = innerContainer.querySelectorAll('span:not([class])');
    const timeRe = /^\d{1,2}-\d{1,2}$|^\d{4}-\d{2}|^刚刚$|^昨天|^\d+\s*(天|小时|分钟)前$/;
    commentTextTest.tested = spans.length;
    Array.from(spans).forEach((s, i) => {
      const text = (s.textContent || '').trim();
      const isTime = timeRe.test(text);
      commentTextTest.samples.push({
        index: i,
        text: short(text, 80),
        isEmpty: text.length === 0,
        isTime,
        parentTag: s.parentElement?.tagName,
        parentClass: short(s.parentElement?.className || '', 60),
      });
      if (text.length > 2 && !isTime) commentTextTest.succeeded++;
    });
    if (commentTextTest.succeeded === 0) commentTextTest.failed = 1;
  }

  // 测试多个评论的 commentId 来源
  const commentIdTest = { total: 0, hasDataId: 0, hasId: 0, synthetic: 0 };
  document.querySelectorAll('.comment-item').forEach(el => {
    commentIdTest.total++;
    if (el.dataset?.id) commentIdTest.hasDataId++;
    else if (el.id) commentIdTest.hasId++;
    else commentIdTest.synthetic++;
  });

  // ============ 5. 列表页选择器（如果在发现/搜索页也可测试） ============
  const listSelectors = {
    feedsContainer: selectorCheck('.feeds-container'),
    userPostedFeeds: selectorCheck('#userPostedFeeds'),
    sectionCards: selectorCountCheck('section'),
    cardCoverLink: selectorCountCheck('section a.cover'),
    cardFooterTitle: selectorCountCheck('.footer span'),
    cardLikeCount: selectorCountCheck('.like-wrapper .count'),
    cardPlayIcon: selectorCountCheck('.play-icon'),
  };

  // ============ 诊断 ============
  const diagnosis = [];
  if (!state) diagnosis.push('CRITICAL: window.__INITIAL_STATE__ 不存在');
  if (!noteModule) diagnosis.push('CRITICAL: __INITIAL_STATE__.note 不存在');
  if (!noteDetailMap) diagnosis.push('CRITICAL: __INITIAL_STATE__.note.noteDetailMap 不存在');
  if (!noteData) diagnosis.push('CRITICAL: 未找到笔记数据');
  if (noteData && !noteData.interactInfo) diagnosis.push('WARNING: 笔记缺少 interactInfo');
  if (!commentSelectors.container.found) diagnosis.push('WARNING: 评论容器 .comments-container 未找到（可能页面未加载评论）');
  if (commentSelectors.parentComment.count === 0) diagnosis.push('WARNING: 无 .parent-comment 评论组');
  if (commentTextTest.failed) diagnosis.push('RISK: 评论正文 span:not([class]) 提取失败——该启发式方法可能已失效');
  if (commentIdTest.synthetic > 0) diagnosis.push(`RISK: ${commentIdTest.synthetic}/${commentIdTest.total} 条评论无 data-id，将使用合成ID`);
  if (commentSelectors.commentIpLocation.count === 0 && commentSelectors.parentComment.count > 0) {
    diagnosis.push('INFO: 评论 IP 属地选择器 .date .location 匹配 0 个（可能该页评论不显示属地）');
  }

  const output = {
    probeVersion: PROBE_VERSION,
    probeType: 'xhs-note-full',
    time: new Date().toISOString(),
    url: location.href,
    noteId,
    stateCheck,
    noteFields: fields,
    commentSelectors,
    commentTextTest,
    commentIdTest,
    listSelectors,
    diagnosis,
  };

  window.__XHS_NOTE_PROBE__ = output;
  console.group('[probe-xhs-note-full]');
  console.log('result:', output);
  console.log('json:', JSON.stringify(output, null, 2));
  console.groupEnd();
  return output;
})();
