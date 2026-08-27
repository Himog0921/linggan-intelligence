/**
 * 抖音评论图片字段探针（在抖音视频页 DevTools Console 执行）
 *
 * 目标：验证评论中图片字段的结构，确认 image_list 路径和格式
 */
(async function probeDouyinCommentImage() {
  function getAwemeId() {
    const match = location.pathname.match(/\/(video|note)\/([A-Za-z0-9_-]+)/);
    return match?.[2] || new URL(location.href).searchParams.get('modal_id') || '';
  }

  const awemeId = getAwemeId();
  if (!awemeId) {
    console.error('[probe] 未能解析 awemeId，请在抖音视频页执行');
    return { error: 'no_aweme_id' };
  }

  // 翻多页找含图片的评论
  let imageCommentSamples = [];
  let cursor = 0;
  let pagesScanned = 0;

  while (imageCommentSamples.length < 2 && pagesScanned < 5) {
    let json;
    try {
      const url = `/aweme/v1/web/comment/list/?aweme_id=${awemeId}&cursor=${cursor}&count=20&item_type=0`;
      const resp = await fetch(url, { credentials: 'include' });
      json = await resp.json();
    } catch (err) {
      break;
    }

    pagesScanned++;
    const comments = Array.isArray(json?.comments) ? json.comments : [];
    if (comments.length === 0) break;

    for (const c of comments) {
      const images = c.image_list || c.images || [];
      if (Array.isArray(images) && images.length > 0) {
        const sample = images[0];
        imageCommentSamples.push({
          commentId: c.cid,
          text: String(c.text || '').slice(0, 50),
          imageCount: images.length,
          imageFields: Object.keys(sample || {}),
          urlCandidates: [
            sample?.origin_url?.url_list?.[0],
            sample?.url_list?.[0],
            sample?.download_url_list?.[0],
          ].filter(Boolean).map((u) => String(u).slice(0, 100)),
          width: sample?.width,
          height: sample?.height,
        });
      }
      // 也检查子回复
      const replies = c.reply_comment_total > 0 ? [] : [];
      void replies;
    }

    if (!json?.has_more) break;
    cursor = json?.cursor ?? cursor + 20;
  }

  const output = {
    time: new Date().toISOString(),
    awemeId,
    pagesScanned,
    imageCommentFound: imageCommentSamples.length > 0,
    imageCommentSamples,
    diagnosis: [],
  };

  if (imageCommentSamples.length === 0) {
    output.diagnosis.push(`ℹ️ 扫描 ${pagesScanned} 页未发现含图片评论，该视频评论可能无图`);
  } else {
    output.diagnosis.push(`✅ 发现 ${imageCommentSamples.length} 条含图评论`);
    const hasUrl = imageCommentSamples.some((s) => s.urlCandidates.length > 0);
    if (hasUrl) {
      output.diagnosis.push('✅ image_list 含可用 URL，当前采集器字段路径正确');
    } else {
      output.diagnosis.push('⚠️ image_list 存在但 URL 字段为空，需检查字段路径');
    }
  }

  window.__DY_COMMENT_IMAGE_PROBE__ = output;
  console.group('[probe-douyin-comment-image]');
  console.log('result:', output);
  console.log('json:', JSON.stringify(output, null, 2));
  console.groupEnd();
  return output;
})();
