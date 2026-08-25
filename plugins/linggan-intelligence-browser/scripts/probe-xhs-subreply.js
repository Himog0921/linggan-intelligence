/**
 * XHS 子回复完整性探针（在小红书笔记详情页 DevTools Console 执行）
 *
 * 目标：验证当前 DOM 展开方式能否采集到全部子回复
 * 对比 DOM 展开数量 vs API 返回数量，找出缺口
 */
(async function probeXhsSubreply() {
  function getNoteId() {
    const match = location.pathname.match(/\/explore\/([a-z0-9]+)/i)
      || location.pathname.match(/\/discovery\/item\/([a-z0-9]+)/i);
    return match?.[1] || '';
  }

  const noteId = getNoteId();
  if (!noteId) {
    console.error('[probe] 未能解析 noteId，请在笔记详情页执行');
    return { error: 'no_note_id' };
  }

  // 1. 从 DOM 统计当前可见的评论
  const container = document.querySelector('.comments-container');
  const parentComments = container ? [...container.querySelectorAll('.parent-comment')] : [];

  const domStats = parentComments.slice(0, 5).map((el) => {
    const mainEl = el.querySelector('.comment-item:not(.comment-item-sub)');
    const subEls = el.querySelectorAll('.comment-item.comment-item-sub');
    const showMoreEl = el.querySelector('div.show-more');
    const showMoreText = showMoreEl?.textContent?.trim() || '';
    const commentId = mainEl?.id || mainEl?.getAttribute('data-id') || '';
    const authorEl = mainEl?.querySelector('a.name');
    return {
      commentId,
      author: authorEl?.textContent?.trim() || '',
      domSubCount: subEls.length,
      hasShowMore: Boolean(showMoreEl),
      showMoreText,
      // 从"展开X条回复"提取声称的总数
      claimedTotal: Number(showMoreText.match(/(\d+)/)?.[1] || 0),
    };
  });

  // 2. 对前2条有子回复的评论，调用 API 验证实际总数
  const apiChecks = [];
  for (const stat of domStats.filter((s) => s.commentId).slice(0, 2)) {
    const url = `/api/sns/web/v2/comment/sub/page?note_id=${noteId}&root_comment_id=${stat.commentId}&num=10&cursor=`;
    try {
      const resp = await fetch(url, { credentials: 'include' });
      const json = await resp.json();
      const apiSubCount = json?.data?.comments?.length ?? 0;
      const hasMore = Boolean(json?.data?.has_more);
      apiChecks.push({
        commentId: stat.commentId,
        domSubCount: stat.domSubCount,
        apiSubCount,
        apiHasMore: hasMore,
        gap: apiSubCount - stat.domSubCount,
        apiCode: json?.code,
      });
    } catch (err) {
      apiChecks.push({ commentId: stat.commentId, error: String(err?.message || err) });
    }
  }

  const output = {
    time: new Date().toISOString(),
    noteId,
    totalParentComments: parentComments.length,
    domStats,
    apiChecks,
    diagnosis: [],
  };

  const hasGap = apiChecks.some((c) => (c.gap ?? 0) > 0 || c.apiHasMore);
  if (hasGap) {
    output.diagnosis.push('⚠️ DOM 展开后子回复数量少于 API 返回，存在遗漏');
  } else if (apiChecks.length > 0) {
    output.diagnosis.push('✅ DOM 展开与 API 数量一致，当前方案完整');
  }
  if (apiChecks.some((c) => c.error)) {
    output.diagnosis.push('⚠️ 部分 API 调用失败，结果不完整');
  }

  window.__XHS_SUBREPLY_PROBE__ = output;
  console.group('[probe-xhs-subreply]');
  console.log('result:', output);
  console.log('json:', JSON.stringify(output, null, 2));
  console.groupEnd();
  return output;
})();
