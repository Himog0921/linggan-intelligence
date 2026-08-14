export function createVideoDomHelpers({
  SEL,
  queryInActiveVideo,
  getApiVideoData,
  normalizeIpLocation,
  parseLocationFromInfoText,
} = {}) {
  function parseDyNumber(text = '') {
    const s = String(text).trim();
    const numMatch = s.match(/([\d.]+)\s*([万亿]?)/);
    if (!numMatch) return 0;
    const num = parseFloat(numMatch[1]) || 0;
    const unit = numMatch[2];
    if (unit === '万') return Math.round(num * 10000);
    if (unit === '亿') return Math.round(num * 100000000);
    return Math.round(num);
  }

  function readCount(selector, scopeHint = '') {
    const el = queryInActiveVideo(selector, scopeHint);
    if (!el) return 0;
    const ariaLabel = el.getAttribute('aria-label') || '';
    const textContent = el.textContent || '';
    const hasDigit = (value) => /\d/.test(value);
    const raw = hasDigit(ariaLabel) ? ariaLabel
      : hasDigit(textContent) ? textContent
        : ariaLabel || textContent;
    return parseDyNumber(raw);
  }

  function extractNicknameFromEl(el) {
    if (!el) return '';
    for (const node of el.childNodes) {
      if (node.nodeType === Node.TEXT_NODE) {
        const text = node.textContent.trim().replace(/^@/, '');
        if (text) return text;
      }
    }
    return el.textContent
      .replace(/认证徽章[\s\S]*/g, '')
      .replace(/(粉丝|获赞|关注)[\s\S]*/g, '')
      .replace(/作者[\s\S]*/g, '')
      .replace(/^@/, '')
      .trim();
  }

  function extractIpLocation(apiData = null, scopeHint = '') {
    const fromApi = normalizeIpLocation(apiData?.ipLocation || '');
    if (fromApi) return fromApi;
    const infoEl = queryInActiveVideo(SEL.videoInfo, scopeHint);
    if (!infoEl) return '';
    return parseLocationFromInfoText(infoEl.textContent || '');
  }

  function extractCoverImg(videoId, apiData = null, scopeHint = '') {
    const cached = apiData || getApiVideoData(videoId);
    if (cached?.coverImg) return cached.coverImg;

    const videoEl = queryInActiveVideo(SEL.videoEl, scopeHint);
    if (videoEl?.poster) return videoEl.poster;

    const coverSelectors = [
      'img[data-e2e="video-cover"]',
      '[data-e2e="player-container"] img',
      '.xgplayer-poster img',
      '.xgplayer img[class*="poster"]',
    ];
    for (const selector of coverSelectors) {
      const img = queryInActiveVideo(selector, scopeHint);
      if (img?.src && !img.src.includes('data:')) return img.src;
    }

    const ogImg = document.querySelector('meta[property="og:image"]');
    if (ogImg?.content) return ogImg.content;
    return '';
  }

  function extractAuthorIdFromDOM(scopeHint = '') {
    const linkEl = queryInActiveVideo('[data-e2e="user-info"] a[href*="/user/"]', scopeHint)
      || document.querySelector('a[href*="/user/"]');
    if (linkEl) {
      const match = linkEl.getAttribute('href').match(/\/user\/([A-Za-z0-9_\-]+)/);
      if (match) return match[1];
    }
    return '';
  }

  function waitForElement(selector, timeoutMs = 3000) {
    return new Promise((resolve, reject) => {
      const el = document.querySelector(selector);
      if (el) {
        resolve(el);
        return;
      }
      const timer = setTimeout(() => {
        observer.disconnect();
        reject(new Error('timeout'));
      }, timeoutMs);
      const observer = new MutationObserver(() => {
        const found = document.querySelector(selector);
        if (found) {
          clearTimeout(timer);
          observer.disconnect();
          resolve(found);
        }
      });
      observer.observe(document.body, { childList: true, subtree: true });
    });
  }

  function waitForContentSettle(timeoutMs = 2500, hintId = '') {
    return new Promise((resolve) => {
      let lastText = queryInActiveVideo(SEL.desc, hintId)?.textContent || '';
      let stableTimer = null;

      function checkStable() {
        const current = queryInActiveVideo(SEL.desc, hintId)?.textContent || '';
        if (current !== lastText) {
          lastText = current;
          clearTimeout(stableTimer);
          stableTimer = setTimeout(() => resolve(), 200);
        }
      }

      setTimeout(() => {
        checkStable();
        const observer = new MutationObserver(checkStable);
        observer.observe(document.body, { childList: true, subtree: true });
        setTimeout(() => {
          observer.disconnect();
          resolve();
        }, timeoutMs);
      }, 300);
    });
  }

  return {
    readCount,
    extractNicknameFromEl,
    extractIpLocation,
    extractCoverImg,
    extractAuthorIdFromDOM,
    waitForElement,
    waitForContentSettle,
  };
}
