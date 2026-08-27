/**
 * 小红书笔记详情探针 v2。
 *
 * 详情、内容媒体和当前评论窗口属于同一逻辑结果。脚本只输出字段存在性、媒体/评论数量与 Coverage；不会读取或输出原文、账号、链接、Cookie 或接口数据。
 */
(function probeXhsNoteDetailV2() {
  const state = window.__INITIAL_STATE__ || null;
  const detailMap = state?.note?.noteDetailMap || null;
  const entry = detailMap && Object.entries(detailMap).find(([key]) => key && key !== 'undefined');
  const note = entry?.[1]?.note || entry?.[1] || null;
  const interact = note?.interactInfo || null;
  const detailRoot = document.querySelector('.note-detail-mask, .note-container, [class*="note-detail"]');
  const commentRoot = detailRoot?.querySelector('.comments-el, .comments-container, [class*="comment-container"]') || null;
  const hasExplicitEmptyCommentState = Boolean(commentRoot?.querySelector('.no-comments, [class*="no-comment"]'));
  const metricCount = Number(interact?.commentCount);
  const publicCommentCount = Number.isFinite(metricCount) && metricCount >= 0 ? Math.floor(metricCount) : null;
  const parentComments = [...(commentRoot?.querySelectorAll('.parent-comment') || [])];
  const allComments = [...(commentRoot?.querySelectorAll('.comment-item') || [])];
  const subComments = commentRoot?.querySelectorAll('.comment-item.comment-item-sub').length || 0;
  const media = {
    imageSlotCount: Array.isArray(note?.imageList) ? note.imageList.length : detailRoot?.querySelectorAll('.note-slider-img, .note-slider img').length || 0,
    hasVideo: Boolean(note?.video) || Boolean(detailRoot?.querySelector('video, [class*="video-player"]')),
    hasLivePhotoSignal: Boolean(note?.livePhoto) || Boolean(note?.livePhotoInfo) || Boolean(detailRoot?.querySelector('[class*="live-photo"]')),
  };
  const expectedCommentCount = publicCommentCount === null ? null : Math.min(publicCommentCount, 30);
  const actualCommentCount = Math.min(allComments.length, 30);

  const output = {
    probeVersion: '2.0',
    probeType: 'xhs-note-detail',
    sanitized: true,
    detail: {
      embeddedStatePresent: Boolean(state),
      detailStatePresent: Boolean(detailMap),
      fieldPresence: {
        title: Boolean(note?.title) || Boolean(detailRoot?.querySelector('[class*="title"]')),
        body: Boolean(note?.desc) || Boolean(detailRoot?.querySelector('.note-content, .note-text, [class*="desc"], [class*="content"]')),
        authorReference: Boolean(note?.user) || Boolean(detailRoot?.querySelector('[class*="author"], [class*="user"]')),
        publishTimeExpression: Boolean(note?.time) || Boolean(detailRoot?.querySelector('[class*="date"], [class*="time"]')),
        interaction: Boolean(interact) || Boolean(detailRoot?.querySelector('.like-wrapper, [class*="interact"]')),
        tags: Array.isArray(note?.tagList) ? note.tagList.length > 0 : Boolean(detailRoot?.querySelector('[class*="tag"]')),
      },
    },
    media,
    comments: {
      cap: 30,
      publicCommentCount,
      expectedCommentCount,
      renderedParentCommentCount: parentComments.length,
      renderedCommentRecordCount: allComments.length,
      renderedSubCommentCount: subComments,
      actualCommentCount,
      targetReached: expectedCommentCount !== null && actualCommentCount >= expectedCommentCount,
      commentRootPresent: Boolean(commentRoot),
      explicitEmptyStatePresent: hasExplicitEmptyCommentState,
      hasLoadMoreControl: Boolean(commentRoot?.querySelector('div.show-more, [class*="more"], [class*="expand"]')),
      fieldPresence: {
        author: allComments.some((comment) => Boolean(comment.querySelector('a.name, [class*="author"], [class*="user"]'))),
        body: allComments.some((comment) => Boolean(comment.querySelector('span:not([class]), [class*="content"], [class*="text"]'))),
        time: allComments.some((comment) => Boolean(comment.querySelector('.date, [class*="time"]'))),
        like: allComments.some((comment) => Boolean(comment.querySelector('.like-wrapper, [class*="like"]'))),
        image: allComments.some((comment) => Boolean(comment.querySelector('img'))),
        replyRelation: subComments > 0,
      },
    },
    coverage: {
      status: hasExplicitEmptyCommentState
        ? 'explicit_empty_state'
        : publicCommentCount === null
          ? 'public_comment_count_unknown'
          : actualCommentCount >= (expectedCommentCount || 0)
            ? 'target_reached'
            : 'target_not_reached',
      missingFromTarget: expectedCommentCount === null ? null : Math.max(expectedCommentCount - actualCommentCount, 0),
      stopReason: 'not_observed_by_passive_probe',
    },
    limitations: [
      '当前脚本只读取已呈现的评论，不自动展开、滚动、点击加载更多或调用接口。',
      '评论记录数是否包含展开后的二级回复，必须与页面默认行为一起登记。',
      '相对时间原始表达存在不等于平台精确发布时间。',
    ],
  };

  window.__XHS_NOTE_DETAIL_PROBE_V2__ = output;
  console.info('[probe-xhs-note-detail-v2]', output);
  return output;
})();
