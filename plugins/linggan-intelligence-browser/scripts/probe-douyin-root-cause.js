/**
 * 抖音根因诊断探针（在问题页面 DevTools Console 执行）
 *
 * 目标：
 * 1. 对齐 modal_id / activeVid / awemeId 三类 ID；
 * 2. 判断“当前作用域”是否拿到正确卡片；
 * 3. 判断下载链路是 blob-only 还是已有 http(s) 直链；
 * 4. 输出可直接用于修复决策的证据对象。
 */
(async function dyRootCauseProbe() {
  const short = (v, n = 180) => {
    const s = String(v ?? '');
    return s.length > n ? `${s.slice(0, n)}...` : s;
  };
  const isTimeLike = (t = '') =>
    /(刚刚|昨天|前天|\d+\s*(秒|分钟|小时|天|周|月|年)前|\d{1,2}月\d{1,2}日|\d{1,2}:\d{2}|\d{4}-\d{1,2}-\d{1,2})/.test(t);

  const u = new URL(location.href);
  const modalId = u.searchParams.get('modal_id') || '';
  const vid = u.searchParams.get('vid') || '';

  const active = document.querySelector('[data-e2e="feed-active-video"]');
  const activeVid = String(
    active?.getAttribute('data-e2e-vid')
    || active?.dataset?.e2eVid
    || active?.getAttribute('data-video-id')
    || '',
  ).trim();

  const scopeRoot = active?.closest('[class*="video_"]') || active || document;
  const q = (sel) => scopeRoot.querySelector(sel) || document.querySelector(sel);

  const infoEl = q('[data-e2e="video-info"]');
  const awemeId = String(infoEl?.getAttribute('data-e2e-aweme-id') || '').trim();

  const descEl = q('[data-e2e="video-desc"], [data-e2e="detail-video-info"]');
  const nickEl = q('[data-e2e="feed-video-nickname"], [data-e2e="user-info"]');
  const videoEl = q('video');

  const videoInfoText = (infoEl?.textContent || '').trim();
  const ipDirect = (videoInfoText.match(/IP属地[:：]?\s*([\u4e00-\u9fa5·]{2,20})/) || [,''])[1] || '';

  // 模拟 legacy 解析：按 · 拆分后，取最后一个“非时间”片段
  const parts = videoInfoText.split(/[·|｜]/).map((s) => s.trim()).filter(Boolean);
  let ipLegacy = '';
  for (let i = parts.length - 1; i >= 0; i -= 1) {
    const p = parts[i];
    if (isTimeLike(p)) continue;
    if (/(展开|收起|发布时间|举报|点赞|评论|收藏|分享|第\d+集|#)/.test(p)) continue;
    ipLegacy = p;
    break;
  }

  const store = window.__lgboom_dy_video_data || {};
  const keySet = Array.from(new Set([awemeId, modalId, activeVid, vid].filter(Boolean)));
  const storeHits = {};
  keySet.forEach((k) => {
    storeHits[k] = store[k] ? {
      hasPlay: Boolean(store[k].videoPlayUrl),
      hasDownload: Boolean(store[k].videoDownloadUrl),
      play: short(store[k].videoPlayUrl || ''),
      download: short(store[k].videoDownloadUrl || ''),
      desc: short(store[k].desc || ''),
      fetchedAt: store[k].fetchedAt || 0,
      sourceUrl: short(store[k].sourceUrl || ''),
    } : null;
  });

  async function tryDetail(id) {
    if (!id) return null;
    const out = {
      id,
      ok: false,
      status: 0,
      status_code: null,
      hasDetail: false,
      hasPlay: false,
      hasDownload: false,
    };
    try {
      const resp = await fetch(
        `/aweme/v1/web/aweme/detail/?aweme_id=${encodeURIComponent(id)}&aid=6383`,
        { credentials: 'include' },
      );
      out.status = resp.status;
      const json = await resp.json().catch(() => null);
      out.ok = resp.ok;
      out.status_code = json?.status_code ?? null;
      const d = json?.aweme_detail;
      out.hasDetail = Boolean(d);
      out.hasPlay = Boolean(d?.video?.play_addr?.url_list?.[0]);
      out.hasDownload = Boolean(d?.video?.download_addr?.url_list?.[0]);
      out.play = short(d?.video?.play_addr?.url_list?.[0] || '');
      out.download = short(d?.video?.download_addr?.url_list?.[0] || '');
      out.desc = short(d?.desc || '');
    } catch (e) {
      out.error = String(e?.message || e);
    }
    return out;
  }

  const detailChecks = [];
  for (const id of keySet) {
    // eslint-disable-next-line no-await-in-loop
    detailChecks.push(await tryDetail(id));
  }

  const output = {
    time: new Date().toISOString(),
    url: location.href,
    ids: { modalId, vid, activeVid, awemeId },
    scope: {
      activeFound: Boolean(active),
      scopeClass: short(scopeRoot?.className || ''),
      desc: short(descEl?.textContent || ''),
      nickname: short(nickEl?.textContent || ''),
      videoInfo: short(videoInfoText, 260),
      videoSrc: short(videoEl?.currentSrc || videoEl?.src || '', 260),
    },
    ipDiagnosis: {
      ipDirect,
      ipLegacy,
    },
    captureDiagnosis: {
      injectedInstalled: Boolean(window.__lgboom_dy_api_capture_installed),
      storeSize: Object.keys(store).length,
      recentKeys: Object.keys(store).slice(-10),
      storeHits,
      detailChecks,
    },
  };

  window.__DY_ROOT_CAUSE_PROBE__ = output;
  console.group('[dy-root-cause-probe]');
  console.log('result:', output);
  console.log('json:', JSON.stringify(output, null, 2));
  console.groupEnd();
  return output;
})();
