/**
 * XHS 评论 API 可行性探针（在小红书笔记详情页 DevTools Console 执行）
 *
 * 目标：验证 /api/sns/web/v2/comment/page 是否可从页面上下文直接调用
 * 如果可行，可替代当前脆弱的 DOM 解析方案（T3 → T1.5）
 */
(async function probeXhsCommentApi() {
  function getNoteId() {
    const match = location.pathname.match(/\/explore\/([a-z0-9]+)/i)
      || location.pathname.match(/\/discovery\/item\/([a-z0-9]+)/i);
    return match?.[1] || '';
  }

  function getXsecToken() {
    try {
      const state = window.__INITIAL_STATE__?.note?.noteDetailMap;
      if (!state) return '';
      for (const key of Object.keys(state)) {
        if (!key || key === 'undefined' || !/^[a-f0-9]{24}$/i.test(key)) continue;
        const entry = state[key];
        const note = entry?.note || entry;
        const token = note?.xsecToken || note?.xsec_token || '';
        if (token) return token;
      }
    } catch { /* ignore */ }
    return '';
  }

  const noteId = getNoteId();
  const xsecToken = getXsecToken();

  if (!noteId) {
    console.error('[probe] 未能解析 noteId，请在笔记详情页执行');
    return { error: 'no_note_id' };
  }

  const url = `/api/sns/web/v2/comment/page?note_id=${noteId}&cursor=&top_comment_id=&image_formats=jpg,webp,avif`;

  let apiResult = null;
  let apiError = null;
  try {
    const resp = await fetch(url, { credentials: 'include' });
    const json = await resp.json();
    apiResult = {
      status: resp.status,
      ok: resp.ok,
      code: json?.code,
      success: json?.success,
      commentCount: json?.data?.comments?.length ?? 0,
      hasMore: json?.data?.cursor !== undefined,
      cursor: json?.data?.cursor,
      sampleComment: json?.data?.comments?.[0]
        ? {
            id: json.data.comments[0].id,
            content: String(json.data.comments[0].content || '').slice(0, 80),
            likeCount: json.data.comments[0].like_count,
            ipLocation: json.data.comments[0].ip_location,
            subCommentCount: json.data.comments[0].sub_comment_count,
            hasImages: Array.isArray(json.data.comments[0].pictures) && json.data.comments[0].pictures.length > 0,
            imageCount: json.data.comments[0].pictures?.length ?? 0,
          }
        : null,
    };
  } catch (err) {
    apiError = String(err?.message || err);
  }

  // 子回复 API 测试（如果主评论 API 成功且有评论）
  let subApiResult = null;
  if (apiResult?.sampleComment?.id && apiResult.sampleComment.subCommentCount > 0) {
    const rootCommentId = apiResult.sampleComment.id;
    const subUrl = `/api/sns/web/v2/comment/sub/page?note_id=${noteId}&root_comment_id=${rootCommentId}&num=10&cursor=`;
    try {
      const resp = await fetch(subUrl, { credentials: 'include' });
      const json = await resp.json();
      subApiResult = {
        status: resp.status,
        code: json?.code,
        success: json?.success,
        subCommentCount: json?.data?.comments?.length ?? 0,
        hasMore: json?.data?.cursor !== undefined,
        sampleSub: json?.data?.comments?.[0]
          ? {
              id: json.data.comments[0].id,
              content: String(json.data.comments[0].content || '').slice(0, 60),
              targetComment: json.data.comments[0].target_comment?.id,
            }
          : null,
      };
    } catch (err) {
      subApiResult = { error: String(err?.message || err) };
    }
  }

  const output = {
    time: new Date().toISOString(),
    url: location.href,
    noteId,
    xsecToken: xsecToken ? xsecToken.slice(0, 20) + '...' : '(未找到)',
    apiCallable: Boolean(apiResult && !apiError),
    apiResult,
    apiError,
    subApiResult,
    diagnosis: [],
  };

  if (!apiResult && apiError) {
    output.diagnosis.push(`API 调用失败: ${apiError}`);
  } else if (apiResult?.code !== 0 && apiResult?.success !== true) {
    output.diagnosis.push(`API 返回非成功状态: code=${apiResult?.code}`);
  } else {
    output.diagnosis.push('✅ API 可直接调用，可替代 DOM 解析方案');
    if (apiResult.sampleComment?.hasImages) {
      output.diagnosis.push(`✅ 评论含图片字段 (pictures)，图片采集可行`);
    }
  }

  window.__XHS_COMMENT_API_PROBE__ = output;
  console.group('[probe-xhs-comment-api]');
  console.log('result:', output);
  console.log('json:', JSON.stringify(output, null, 2));
  console.groupEnd();
  return output;
})();
