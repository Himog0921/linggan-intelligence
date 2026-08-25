/**
 * detail_probe 任务作者页接力链路探查脚本（在小红书页面 DevTools Console 运行）
 *
 * 背景：
 *   detail_probe 任务拿到无签名 profile relay URL（/user/profile/{authorId}/{noteId}）时，
 *   插件当前会把它规范成裸 explore/{noteId} 直接打开，触发 30017 风控。
 *   修复方向有两个待验证：
 *     A. 完整恢复"作者页接力"——打开作者主页根，profile 模式锁定 noteId 后点开
 *     B. 保持 detail 模式——但打开带 xsec_token 签名的详情页
 *   本脚本一次拿齐判断两个方向可行性所需的全部页面事实，不分多轮。
 *
 * 用法（只需跑 2 次，各复制一次输出给 AI）：
 *   场景 1：打开任一小红书【作者主页】（URL 含 /user/profile/{authorId}），粘贴本脚本到 Console，回车，复制输出。
 *   场景 2：从该作者主页【点开一篇笔记】进入详情页，URL 会变成 /explore/{noteId} 或 /discovery/item/{noteId}，
 *           再粘贴本脚本到 Console，回车，复制输出。
 *
 *   两次输出一起给 AI，AI 就能判断：作者主页能不能锁定 noteId、签名 token 在哪、详情页能否直接采。
 */
(function probeDetailProbeRelay() {
  const STABLE_ID_REGEX = /^[a-f0-9]{24}$/i;
  const TOKEN_PATTERN = /xsec_token=([^&#]+)/i;

  function extractNoteIdFromUrl(url) {
    const match = String(url || '').match(/\/(?:explore|discovery\/item|search_result)\/([a-f0-9]{24})/i);
    return match ? match[1] : '';
  }

  function extractAuthorIdFromUrl(url) {
    const match = String(url || '').match(/\/user\/profile\/([a-f0-9]{24})/i);
    return match ? match[1] : '';
  }

  function pickImageUrl(el) {
    if (!el) return '';
    const srcset = el.getAttribute('srcset') || '';
    const srcsetLast = srcset.split(',').map((s) => s.trim().split(/\s+/)[0]).filter(Boolean).pop() || '';
    return el.currentSrc || el.src || el.getAttribute('data-src') || srcsetLast || '';
  }

  function detectPageType() {
    const href = location.href;
    if (/\/user\/profile\//i.test(href)) return 'profile';
    if (/\/(?:explore|discovery\/item)\//i.test(href)) return 'detail';
    if (/\/search_result/i.test(href)) return 'search';
    return 'unknown';
  }

  function readInitialNoteState() {
    const raw = window.__INITIAL_STATE__?.note?.noteDetailMap || {};
    const keys = Object.keys(raw).filter((k) => STABLE_ID_REGEX.test(k));
    return {
      hasState: Boolean(window.__INITIAL_STATE__),
      hasNoteMap: Boolean(window.__INITIAL_STATE__?.note?.noteDetailMap),
      allKeys: Object.keys(raw),
      validNoteKeys: keys,
      firstNoteSummary: keys.length > 0 ? {
        noteId: raw[keys[0]]?.note?.noteId || raw[keys[0]]?.noteId || '',
        title: raw[keys[0]]?.note?.title || raw[keys[0]]?.title || '',
        hasInteractInfo: Boolean(raw[keys[0]]?.note?.interactInfo || raw[keys[0]]?.interactInfo),
        hasImageList: Array.isArray(raw[keys[0]]?.note?.imageList || raw[keys[0]]?.imageList) && (raw[keys[0]]?.note?.imageList || raw[keys[0]]?.imageList).length > 0,
      } : null,
    };
  }

  function detectRiskControl() {
    // 检测页面是否出现风控/验证/错误信号（30017 或其他）
    const bodyText = String(document.body?.innerText || '').slice(0, 5000);
    const signals = [];
    if (/30017|风控|验证|滑动验证|安全验证/i.test(bodyText)) signals.push('risk_control_text');
    if (/页面不存在|已删除|无法查看|笔记已不在/i.test(bodyText)) signals.push('not_found_text');
    if (/登录|sign in/i.test(bodyText) && bodyText.length < 800) signals.push('login_required_text');
    return {
      bodyTextLength: bodyText.length,
      signals,
      firstBodyChars: bodyText.slice(0, 200),
    };
  }

  function scanProfileCards() {
    // 作者主页列表卡片：noteId / href / 是否带 xsec_token / 封面
    const cards = [];
    const containers = ['#userPostedFeeds', '.feeds-container'];
    let usedContainer = '';
    for (const c of containers) {
      const root = document.querySelector(c);
      if (!root) continue;
      usedContainer = c;
      const sections = root.querySelectorAll('section');
      sections.forEach((section) => {
        const coverLink = section.querySelector('a.cover');
        if (!coverLink) return;
        const href = coverLink.getAttribute('href') || '';
        const noteId = (href.match(/([a-f0-9]{24})/i) || [])[1] || '';
        if (!noteId) return;
        const titleEl = section.querySelector('.title') || section.querySelector('.footer span');
        const likesEl = section.querySelector('.like-wrapper .count');
        const img = coverLink.querySelector('img, picture img, source');
        cards.push({
          noteId,
          href,
          hrefHasToken: TOKEN_PATTERN.test(href),
          token: (href.match(TOKEN_PATTERN) || [])[1] || '',
          title: titleEl?.textContent?.trim()?.slice(0, 40) || '',
          likes: likesEl?.textContent?.trim() || '',
          cover: pickImageUrl(img)?.slice(0, 80) || '',
        });
      });
      if (cards.length > 0) break;
    }
    return { usedContainer, cardCount: cards.length, firstCards: cards.slice(0, 5), allNoteIds: cards.map((c) => c.noteId) };
  }

  function scanDetailPage() {
    const href = location.href;
    const noteIdFromUrl = extractNoteIdFromUrl(href);
    const tokenFromUrl = (href.match(TOKEN_PATTERN) || [])[1] || '';
    const noteState = readInitialNoteState();
    return {
      url: href,
      urlHasToken: TOKEN_PATTERN.test(href),
      tokenFromUrl,
      noteIdFromUrl,
      urlMatchesDetailInState: noteState.validNoteKeys.includes(noteIdFromUrl),
      noteState,
      riskControl: detectRiskControl(),
    };
  }

  // ===== 主逻辑 =====
  const pageType = detectPageType();
  const summary = {
    probe: 'detail-probe-relay',
    time: new Date().toISOString(),
    pageType,
    url: location.href,
    authorIdInUrl: extractAuthorIdFromUrl(location.href),
  };

  if (pageType === 'profile') {
    summary.profileScan = scanProfileCards();
    summary.noteStateOnProfile = readInitialNoteState();
    summary.riskControl = detectRiskControl();
    console.log('%c[probe-detail-probe-relay] 作者主页扫描结果', 'color:#d8554f;font-weight:bold');
    console.log(JSON.stringify(summary, null, 2));
    console.log('关键判断：');
    console.log('  1. profileScan.cardCount > 0 → 作者主页能扫到卡片（方向 A 前提）');
    console.log('  2. firstCards[].hrefHasToken → 卡片链接是否自带 xsec_token（方向 B 签名来源）');
    console.log('  3. noteStateOnProfile.validNoteKeys.length → 作者主页是否预载了笔记详情数据（detail 模式能否在作者主页跑）');
    console.log('请复制上面 JSON 给 AI。然后点开一篇笔记，在详情页再跑一次本脚本。');
    return summary;
  }

  if (pageType === 'detail') {
    summary.detailScan = scanDetailPage();
    console.log('%c[probe-detail-probe-relay] 详情页扫描结果', 'color:#3bb8d8;font-weight:bold');
    console.log(JSON.stringify(summary, null, 2));
    console.log('关键判断：');
    console.log('  1. detailScan.urlHasToken → 进入详情页的 URL 是否保留 xsec_token');
    console.log('  2. detailScan.noteIdFromUrl 匹配 noteState.validNoteKeys → __INITIAL_STATE__ 是否就绪');
    console.log('  3. detailScan.riskControl.signals → 是否出现风控/删除/登录提示');
    console.log('请复制上面 JSON 给 AI。');
    return summary;
  }

  summary.message = `当前页类型 ${pageType} 不是作者主页也不是详情页，请在作者主页或笔记详情页运行。`;
  console.warn(summary.message);
  console.log(JSON.stringify(summary, null, 2));
  return summary;
})();
