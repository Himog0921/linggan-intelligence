export function sanitizeFilename(name = '') {
  return String(name || 'douyin_video')
    .slice(0, 40)
    .replace(/[\\/:*?"<>|]/g, '_')
    .trim() || 'douyin_video';
}

// ========== 页面上下文下载（MAIN world fetch，携带完整 cookie） ==========
// 注入脚本（douyinApiCapture.js）运行在页面 MAIN world，
// 它的 origFetch 携带页面完整登录态，能通过抖音 CDN 鉴权。
// Content script 通过 CustomEvent 与其通信。

const PAGE_DOWNLOAD_REQ_EVENT = '__lgboom_page_download_req__';
const PAGE_DOWNLOAD_RES_EVENT = '__lgboom_page_download_res__';

async function downloadViaPageContext(candidates, safeName) {
  if (!candidates || candidates.length === 0) {
    return { ok: false, error: 'no_candidates' };
  }
  // 仅在抖音页面可用
  if (!window.location?.host?.includes('douyin.com')) {
    return { ok: false, error: 'not_on_douyin' };
  }

  const requestId = Date.now().toString(36) + Math.random().toString(36).slice(2);
  const filename = `灵感爆爆爆_抖音视频_${safeName}.mp4`;

  return new Promise((resolve) => {
    const timeout = setTimeout(() => {
      window.removeEventListener(PAGE_DOWNLOAD_RES_EVENT, handler);
      resolve({ ok: false, error: 'page_download_timeout' });
    }, 120000);

    function handler(e) {
      const detail = e.detail || {};
      if (detail.requestId !== requestId) return;
      clearTimeout(timeout);
      window.removeEventListener(PAGE_DOWNLOAD_RES_EVENT, handler);
      resolve(detail.ok ? { ok: true } : { ok: false, error: detail.error || 'page_download_failed' });
    }

    window.addEventListener(PAGE_DOWNLOAD_RES_EVENT, handler);
    window.dispatchEvent(new CustomEvent(PAGE_DOWNLOAD_REQ_EVENT, {
      detail: { urls: candidates, filename, requestId },
    }));
  });
}

async function tryFetchDownload(url, { credentials = 'include', headers = {} } = {}) {
  const resp = await fetch(url, { credentials, headers });
  if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
  const blob = await resp.blob();
  if (!blob || blob.size <= 0) throw new Error('empty_blob');
  return blob;
}

async function tryXhrDownload(url, { headers = {} } = {}) {
  return new Promise((resolve, reject) => {
    const xhr = new XMLHttpRequest();
    xhr.open('GET', url, true);
    xhr.responseType = 'blob';
    Object.entries(headers).forEach(([k, v]) => xhr.setRequestHeader(k, v));
    xhr.onload = () => {
      if (xhr.status >= 200 && xhr.status < 300) {
        if (xhr.response && xhr.response.size > 0) resolve(xhr.response);
        else reject(new Error('empty_blob'));
      } else {
        reject(new Error(`HTTP ${xhr.status}`));
      }
    };
    xhr.onerror = () => reject(new Error('XHR failed'));
    xhr.send();
  });
}

function triggerBlobDownload(blob, safeName, ext = 'mp4') {
  const objectUrl = URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = objectUrl;
  anchor.download = `灵感爆爆爆_${safeName}.${ext}`;
  anchor.style.display = 'none';
  document.body.appendChild(anchor);
  anchor.click();
  document.body.removeChild(anchor);
  setTimeout(() => URL.revokeObjectURL(objectUrl), 30000);
}

export async function downloadViaBlobFallback(candidates, safeName) {
  // 1. 优先使用页面上下文下载（MAIN world fetch，带完整登录态，能通过 CDN 鉴权）
  const pageResult = await downloadViaPageContext(candidates, safeName);
  if (pageResult.ok) return pageResult;

  const referer = window.location.href || 'https://www.douyin.com/';
  const errors = [];

  for (const candidate of candidates) {
    // 2a. fetch 带 credentials，不带 Range（避免触发 CORS preflight）
    try {
      const blob = await tryFetchDownload(candidate, {
        credentials: 'include',
        headers: { Referer: referer },
      });
      triggerBlobDownload(blob, safeName);
      return { ok: true };
    } catch (err) {
      errors.push(`${candidate} (fetch-cred) -> ${String(err?.message || err)}`);
    }

    // 2b. fetch 不带 credentials（某些 CDN 对 credentials 要求与 CORS 冲突）
    try {
      const blob = await tryFetchDownload(candidate, {
        credentials: 'omit',
        headers: { Referer: referer },
      });
      triggerBlobDownload(blob, safeName);
      return { ok: true };
    } catch (err) {
      errors.push(`${candidate} (fetch-omit) -> ${String(err?.message || err)}`);
    }

    // 2c. XHR（不带 Range）
    try {
      const blob = await tryXhrDownload(candidate, { Referer: referer });
      triggerBlobDownload(blob, safeName);
      return { ok: true };
    } catch (err) {
      errors.push(`${candidate} (xhr) -> ${String(err?.message || err)}`);
    }

    // 2d. 带闭合 Range 的降级
    try {
      const blob = await tryFetchDownload(candidate, {
        credentials: 'include',
        headers: { Referer: referer, Range: 'bytes=0-5242880' },
      });
      triggerBlobDownload(blob, safeName);
      return { ok: true };
    } catch (err) {
      errors.push(`${candidate} (fetch-range) -> ${String(err?.message || err)}`);
    }
  }

  return { ok: false, error: errors[0] || 'blob_fallback_failed' };
}
