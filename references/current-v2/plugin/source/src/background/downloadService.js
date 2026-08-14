export function dedupeCandidates(input = []) {
  const list = Array.isArray(input) ? input : [input];
  const seen = new Set();
  const normalized = [];
  for (const raw of list) {
    if (!raw) continue;
    const url = String(raw).trim();
    if (!url || seen.has(url)) continue;
    seen.add(url);
    normalized.push(url);
  }
  return normalized;
}

export function sanitizeDownloadFilename(filename = '灵感爆爆爆/下载文件') {
  const normalized = String(filename || '灵感爆爆爆/下载文件')
    .replace(/[\\:*?"<>|]/g, '_')
    .replace(/\/+/g, '/')
    .replace(/^\/+/, '')
    .trim();
  return normalized || '灵感爆爆爆/下载文件';
}

export function buildDownloadHeaders(customHeaders = []) {
  const list = Array.isArray(customHeaders) ? [...customHeaders] : [];
  if (list.length === 0) return [];
  const forbidden = new Set([
    'accept-charset', 'accept-encoding', 'access-control-request-headers', 'access-control-request-method',
    'connection', 'content-length', 'cookie', 'cookie2', 'date', 'dnt', 'expect', 'host', 'keep-alive',
    'origin', 'sec-fetch-site', 'sec-fetch-mode', 'sec-fetch-user', 'sec-fetch-dest',
    'te', 'trailer', 'transfer-encoding', 'upgrade', 'via',
  ]);
  const valid = [];
  for (const item of list) {
    const name = String(item?.name || '').trim();
    const value = String(item?.value || '');
    if (!name) continue;
    const lower = name.toLowerCase();
    if (forbidden.has(lower)) continue;
    if (!/^[!#$%&'*+\-.^_`|~0-9A-Za-z]+$/.test(name)) continue;
    valid.push({ name, value });
  }
  return valid;
}

function arrayBufferToBase64(buffer) {
  const bytes = new Uint8Array(buffer);
  const chunkSize = 0x8000;
  let binary = '';
  for (let i = 0; i < bytes.length; i += chunkSize) {
    const chunk = bytes.subarray(i, i + chunkSize);
    binary += String.fromCharCode(...chunk);
  }
  return btoa(binary);
}

export async function fetchBinaryAsDataUrl(candidates = []) {
  const urls = dedupeCandidates(candidates);
  for (let i = 0; i < urls.length; i += 1) {
    const candidate = urls[i];
    try {
      const response = await fetch(candidate, { credentials: 'include' });
      if (!response.ok) continue;
      const buffer = await response.arrayBuffer();
      if (!buffer || buffer.byteLength <= 0) continue;
      const contentType = String(response.headers.get('content-type') || '').trim() || 'application/octet-stream';
      return {
        success: true,
        candidate,
        candidateIndex: i,
        contentType,
        dataUrl: `data:${contentType};base64,${arrayBufferToBase64(buffer)}`,
      };
    } catch {
      // try next candidate
    }
  }
  return { success: false };
}

export function waitDownloadFinished(downloadId, timeoutMs = 180000) {
  return new Promise((resolve) => {
    let done = false;
    const timer = setTimeout(() => {
      if (done) return;
      done = true;
      chrome.downloads.onChanged.removeListener(onChanged);
      resolve({ success: false, reason: 'timeout' });
    }, timeoutMs);

    const onChanged = (delta) => {
      if (done || delta.id !== downloadId || !delta.state) return;
      const state = delta.state.current;
      if (state !== 'complete' && state !== 'interrupted') return;
      done = true;
      clearTimeout(timer);
      chrome.downloads.onChanged.removeListener(onChanged);
      if (state === 'complete') {
        resolve({ success: true, reason: 'complete' });
      } else {
        resolve({
          success: false,
          reason: delta.error?.current || 'interrupted',
        });
      }
    };

    chrome.downloads.onChanged.addListener(onChanged);
  });
}

export async function tryDownloadCandidate(url, filename, options = {}) {
  const {
    saveAs = false,
    conflictAction = 'uniquify',
    timeoutMs = 180000,
    headers = [],
    waitForCompletion = true,
  } = options;
  let downloadId;
  try {
    downloadId = await chrome.downloads.download({
      url,
      filename,
      saveAs,
      conflictAction,
      headers,
    });
  } catch (err) {
    const msg = String(err?.message || err || '');
    if (!/headers|Unexpected property|Unsafe request header name/i.test(msg)) {
      throw err;
    }
    downloadId = await chrome.downloads.download({
      url,
      filename,
      saveAs,
      conflictAction,
    });
  }
  if (!downloadId && downloadId !== 0) {
    return { success: false, reason: 'download_id_empty' };
  }
  if (!waitForCompletion) {
    return { success: true, reason: 'queued', downloadId };
  }
  const result = await waitDownloadFinished(downloadId, timeoutMs);
  if (!result.success && downloadId) {
    try { await chrome.downloads.remove(downloadId); } catch {}
  }
  return { ...result, downloadId };
}
