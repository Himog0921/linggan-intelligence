/**
 * 抖音评论子回复完整性探针（在抖音视频页 DevTools Console 执行）
 *
 * 目标：验证 /aweme/v1/web/comment/list/reply/ API 是否可调用
 * 以及当前采集器的子回复覆盖是否完整
 */
(async function probeDouyinCommentReply() {
  function getAwemeId() {
    const match = location.pathname.match(/\/(video|note)\/([A-Za-z0-9_-]+)/);
    return match?.[2] || new URL(location.href).searchParams.get('modal_id') || '';
  }

  const awemeId = getAwemeId();
  if (!awemeId) {
    console.error('[probe] 未能解析 awemeId，请在抖音视频页执行');
    return { error: 'no_aweme_id' };
  }

  // 1. 获取第一页主评论
  let firstPageComments = [];
  let commentApiResult = null;
  try {
    const url = `/aweme/v1/web/comment/list/?aweme_id=${awemeId}&cursor=0&count=10&item_type=0`;
    const resp = await fetch(url, { credentials: 'include' });
    const json = await resp.json();
    commentApiResult = {
      status: resp.status,
      statusCode: json?.status_code,
      commentCount: json?.comments?.length ?? 0,
      hasMore: json?.has_more,
      total: json?.total,
    };
    firstPageComments = Array.isArray(json?.comments) ? json.comments : [];
  } catch (err) {
    return { error: `主评论 API 失败: ${err?.message || err}` };
  }

  // 2. 找一条有子回复的评论，测试 reply API
  const withReplies = firstPageComments.find(
    (c) => Number(c.reply_comment_total ?? c.reply_count ?? 0) > 0
  );

  let replyApiResult = null;
  if (withReplies) {
    const commentId = String(withReplies.cid || withReplies.comment_id || '');
    const replyTotal = Number(withReplies.reply_comment_total ?? withReplies.reply_count ?? 0);
    try {
      const url = `/aweme/v1/web/comment/list/reply/?item_id=${awemeId}&comment_id=${commentId}&cursor=0&count=10`;
      const resp = await fetch(url, { credentials: 'include' });
      const json = await resp.json();
      replyApiResult = {
        commentId,
        claimedReplyTotal: replyTotal,
        status: resp.status,
        statusCode: json?.status_code,
        replyCount: json?.comments?.length ?? 0,
        hasMore: json?.has_more,
        sampleReply: json?.comments?.[0]
          ? {
              cid: json.comments[0].cid,
              text: String(json.comments[0].text || '').slice(0, 60),
              replyToCommentId: json.comments[0].reply_id,
              hasImages: Array.isArray(json.comments[0].image_list) && json.comments[0].image_list.length > 0,
            }
          : null,
      };
    } catch (err) {
      replyApiResult = { error: String(err?.message || err) };
    }
  }

  const output = {
    time: new Date().toISOString(),
    url: location.href,
    awemeId,
    commentApiResult,
    replyApiResult,
    diagnosis: [],
  };

  if (commentApiResult?.statusCode === 0) {
    output.diagnosis.push('✅ 主评论 API 可调用');
  } else {
    output.diagnosis.push(`⚠️ 主评论 API 状态异常: status_code=${commentApiResult?.statusCode}`);
  }
  if (replyApiResult && !replyApiResult.error) {
    if (replyApiResult.statusCode === 0) {
      output.diagnosis.push('✅ 子回复 API 可调用');
      if (replyApiResult.hasMore) {
        output.diagnosis.push('⚠️ 子回复有更多页，需分页采集');
      }
    } else {
      output.diagnosis.push(`⚠️ 子回复 API 状态异常: status_code=${replyApiResult.statusCode}`);
    }
  } else if (!withReplies) {
    output.diagnosis.push('ℹ️ 当前视频前10条评论无子回复，无法测试 reply API');
  }

  window.__DY_COMMENT_REPLY_PROBE__ = output;
  console.group('[probe-douyin-comment-reply]');
  console.log('result:', output);
  console.log('json:', JSON.stringify(output, null, 2));
  console.groupEnd();
  return output;
})();
