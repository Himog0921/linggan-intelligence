/**
 * 通过外部脚本文件从页面 JS 上下文获取数据
 * 使用 <script src> 注入 web_accessible_resources 中的文件，
 * 绕过 CSP 对内联脚本的限制
 *
 * @param {Window} wd - 目标 window（主页面或 iframe）
 * @param {string} type - 'noteMap' | 'user'
 */
let cachedRuntimeId = '';
try {
  cachedRuntimeId = chrome.runtime?.id || '';
} catch {
  cachedRuntimeId = '';
}

function getInjectScriptUrl(file) {
  try {
    const url = chrome.runtime.getURL(file);
    const runtimeId = chrome.runtime?.id || '';
    if (runtimeId) cachedRuntimeId = runtimeId;
    return url;
  } catch {
    if (!cachedRuntimeId) return '';
    // 扩展热更新后，旧 content script 可能失去 runtime context。
    // 这时退回到缓存的扩展 ID，尽可能保证注入链路不中断。
    return `chrome-extension://${cachedRuntimeId}/${file}`;
  }
}

export function getByInject(wd, type) {
  return new Promise((resolve, reject) => {
    const fileMap = { noteMap: 'injected/noteMap.js', user: 'injected/user.js' };
    const file = fileMap[type];
    if (!file) {
      reject(new Error(`Unknown inject type: ${type}`));
      return;
    }

    const scriptUrl = getInjectScriptUrl(file);
    if (!scriptUrl) {
      reject(new Error(`Extension context invalidated`));
      return;
    }

    const timeout = setTimeout(() => {
      wd.removeEventListener('message', handler);
      reject(new Error(`Inject ${type} timeout`));
    }, 6000);

    const handler = (event) => {
      if (event.source !== wd) return;
      if (event.data?.type !== type) return;
      wd.removeEventListener('message', handler);
      clearTimeout(timeout);
      resolve(event.data.data);
    };
    wd.addEventListener('message', handler);

    // 外部文件注入：用 src 加载，绕过 CSP 内联限制
    // 加 cache-bust 参数避免浏览器缓存旧版本
    const script = wd.document.createElement('script');
    script.src = scriptUrl + '?t=' + Date.now();
    script.onload = () => script.remove();
    script.onerror = () => {
      wd.removeEventListener('message', handler);
      clearTimeout(timeout);
      script.remove();
      reject(new Error(`Failed to load ${file}`));
    };
    wd.document.documentElement.appendChild(script);
  });
}

/**
 * 随机延迟
 */
export function randomDelay(min, max) {
  const ms = Math.random() * (max - min) + min;
  return new Promise(resolve => setTimeout(resolve, ms));
}

/**
 * 解析数字（支持 "1.2万" "10万+" 格式）
 */
export function parseCount(text) {
  if (!text) return 0;
  if (typeof text === 'number') return Math.round(text);

  if (typeof text === 'object') {
    const candidates = [
      text.displayText,
      text.display_text,
      text.displayCount,
      text.display_count,
      text.text,
      text.countText,
      text.count_text,
      text.value,
      text.count,
      text.num,
      text.number,
    ].filter((candidate) => candidate != null && candidate !== '');

    if (candidates.length === 0) return 0;

    // 优先信任带单位的展示字段（如 "10万+"），避免被 count=10 这类缩略值截断
    let unitBasedMax = 0;
    let numericMax = 0;
    for (const candidate of candidates) {
      const parsed = parseCount(candidate);
      numericMax = Math.max(numericMax, parsed);

      if (typeof candidate === 'string' && /[万亿千wWkK+]/.test(candidate)) {
        unitBasedMax = Math.max(unitBasedMax, parsed);
      }
    }

    if (unitBasedMax > 0) return unitBasedMax;
    return numericMax;
  }

  let normalized = String(text)
    .trim()
    .replace(/\s+/g, '')
    .replace(/,/g, '')
    .replace(/，/g, '')
    .replace(/＋/g, '+')
    .replace(/\+/g, '');

  if (!normalized) return 0;

  if (/[wW]$/.test(normalized)) {
    return Math.round(parseFloat(normalized) * 10000) || 0;
  }
  if (/[kK]$/.test(normalized)) {
    return Math.round(parseFloat(normalized) * 1000) || 0;
  }
  if (normalized.includes('千')) {
    return Math.round(parseFloat(normalized) * 1000) || 0;
  }
  if (normalized.includes('万')) {
    return Math.round(parseFloat(normalized) * 10000) || 0;
  }
  if (normalized.includes('亿')) {
    return Math.round(parseFloat(normalized) * 100000000) || 0;
  }

  const numericPart = normalized.match(/-?\d+(\.\d+)?/);
  if (!numericPart) return 0;
  return Math.round(parseFloat(numericPart[0])) || 0;
}

/**
 * 安全化 URL（确保完整路径）
 */
export function safeUrl(url) {
  if (!url) return '';
  if (url.startsWith('http')) return url;
  if (url.startsWith('//')) return 'https:' + url;
  if (url.startsWith('/')) return 'https://www.xiaohongshu.com' + url;
  return url;
}

/**
 * 从 URL 中提取笔记 ID
 */
export function extractNoteId(url) {
  if (!url) return '';
  const match = url.match(/\/explore\/([a-z0-9]+)/i) ||
                url.match(/\/discovery\/item\/([a-z0-9]+)/i) ||
                url.match(/\/search_result\/([a-z0-9]+)/i) ||
                url.match(/\/user\/profile\/[a-z0-9]+\/([a-z0-9]+)/i);
  return match ? match[1] : url.split('/').pop()?.split('?')[0] || '';
}

/**
 * CSV 转义
 */
export function csvEscape(value) {
  if (value == null) return '';
  const str = String(value);
  if (str.includes(',') || str.includes('"') || str.includes('\n')) {
    return '"' + str.replace(/"/g, '""') + '"';
  }
  return str;
}

/**
 * 生成带 BOM 的 CSV 内容
 */
export function generateCsv(headers, rows) {
  const headerLine = headers.join(',');
  const dataLines = rows.map(row => row.map(csvEscape).join(','));
  return '\ufeff' + [headerLine, ...dataLines].join('\n');
}

/**
 * 触发文件下载
 */
export function downloadFile(content, filename, mimeType = 'text/csv;charset=utf-8;') {
  const blob = new Blob([content], { type: mimeType });
  const a = document.createElement('a');
  a.href = URL.createObjectURL(blob);
  a.download = filename;
  a.click();
  URL.revokeObjectURL(a.href);
}

export function getHighQualityImageCandidates(url) {
  if (!url) return [];
  const raw = safeUrl(url);
  const candidates = new Set();
  const addCandidate = (value) => {
    if (!value) return;
    const normalized = safeUrl(value).trim();
    if (!normalized) return;
    candidates.add(normalized);
  };

  const noQuery = raw.split('?')[0];
  // 优先尝试更接近原图的地址，再回退到原始 URL
  addCandidate(noQuery);
  // 常见样式后缀：xxx.jpg!nd_dft_wlteh_webp_3
  if (noQuery.includes('!')) {
    addCandidate(noQuery.split('!')[0]);
  }

  if (raw.includes('x-oss-process=')) {
    addCandidate(raw.replace(/([?&])x-oss-process=[^&]*/g, '').replace(/[?&]$/, ''));
  }
  if (raw.includes('imageView2') || raw.includes('imageslim') || raw.includes('thumbnail')) {
    addCandidate(raw.replace(/([?&])(imageView2|imageslim|thumbnail)=[^&]*/g, '').replace(/[?&]$/, ''));
  }
  addCandidate(raw);

  const normalized = [];
  for (const item of candidates) {
    if (item && !normalized.includes(item)) normalized.push(item);
  }
  return normalized;
}

export function toHighQualityImageUrl(url) {
  const candidates = getHighQualityImageCandidates(url);
  return candidates[0] || url;
}

function collectVideoStreamUrls(item = {}) {
  const urls = [];
  const push = (value) => {
    if (!value) return;
    if (Array.isArray(value)) {
      value.forEach(push);
      return;
    }
    if (typeof value === 'object') {
      push(value.masterUrl || value.master_url || value.url || value.backupUrl || value.backup_url);
      return;
    }
    const url = String(value || '').trim();
    if (url && !urls.includes(url)) urls.push(url);
  };

  push(item.masterUrl);
  push(item.master_url);
  push(item.url);
  push(item.backupUrl);
  push(item.backup_url);
  push(item.backupUrls);
  push(item.backup_urls);
  push(item.urlList);
  push(item.url_list);
  return urls;
}

export function pickBestVideoStream(stream = {}) {
  // 小红书会调整流分类名（已实测从 h264/h265 等改为 EF4/EF5）。流条目本身
  // 仍提供同一套 masterUrl、backupUrls、尺寸和码率字段，因此以实际存在的数组
  // 为准，不再把外层分类名当作协议。
  const pools = Object.values(stream || {}).flatMap((group) => (
    Array.isArray(group) ? group : []
  ));
  if (pools.length === 0) return { url: '', streams: [] };

  const scored = pools.flatMap((item) => {
    const bitrate = Number(item.avgBitrate || item.bitrate || item.avg_bitrate || 0);
    const width = Number(item.width || item.vwidth || 0);
    const height = Number(item.height || item.vheight || 0);
    const score = bitrate * 10 + width * height;
    return collectVideoStreamUrls(item).map((url, index) => ({
      url,
      bitrate,
      width,
      height,
      qualityType: item.qualityType || item.quality_type || '',
      score: score - index,
    }));
  }).filter((item) => Boolean(item.url));

  scored.sort((a, b) => b.score - a.score);
  return {
    url: scored[0]?.url || '',
    streams: scored,
  };
}

function isImageBlob(blob) {
  return String(blob?.type || '').startsWith('image/');
}

async function getImageSize(blob) {
  try {
    if (typeof createImageBitmap === 'function') {
      const bitmap = await createImageBitmap(blob);
      const width = Number(bitmap.width || 0);
      const height = Number(bitmap.height || 0);
      bitmap.close?.();
      return { width, height };
    }
  } catch {
    // ignore
  }

  return new Promise((resolve) => {
    try {
      const objectUrl = URL.createObjectURL(blob);
      const img = new Image();
      img.onload = () => {
        const width = Number(img.naturalWidth || img.width || 0);
        const height = Number(img.naturalHeight || img.height || 0);
        URL.revokeObjectURL(objectUrl);
        resolve({ width, height });
      };
      img.onerror = () => {
        URL.revokeObjectURL(objectUrl);
        resolve({ width: 0, height: 0 });
      };
      img.src = objectUrl;
    } catch {
      resolve({ width: 0, height: 0 });
    }
  });
}

async function fetchCandidateBlob(url, timeoutMs) {
  let timeout = null;
  try {
    const controller = new AbortController();
    timeout = setTimeout(() => controller.abort('timeout'), timeoutMs);
    const response = await fetch(url, { mode: 'cors', signal: controller.signal });
    if (!response.ok) {
      throw new Error(`HTTP ${response.status}`);
    }
    const blob = await response.blob();
    const mimeType = String(blob?.type || response.headers.get('content-type') || '').toLowerCase();
    if (mimeType && !mimeType.startsWith('image/') && !mimeType.startsWith('video/')) {
      throw new Error(`Unsupported content-type: ${mimeType}`);
    }
    const size = Number(blob.size || 0);
    let width = 0;
    let height = 0;
    let score = size;
    if (isImageBlob(blob)) {
      const sizeInfo = await getImageSize(blob);
      width = sizeInfo.width;
      height = sizeInfo.height;
      score = width * height * 10 + size;
    }
    return { success: true, url, blob, size, width, height, score };
  } catch (err) {
    return { success: false, url, error: String(err?.message || err) };
  } finally {
    if (timeout) clearTimeout(timeout);
  }
}

/**
 * 下载单个媒体文件
 */
export async function downloadMediaFile(url, filename, options = {}) {
  const {
    shouldStop = () => false,
    waitIfPaused = async () => {},
    timeoutMs = 18000,
  } = options;
  const baseCandidates = Array.isArray(url) ? url : [url];
  const candidates = [];
  baseCandidates.forEach((item) => {
    getHighQualityImageCandidates(item).forEach((candidate) => {
      if (candidate && !candidates.includes(candidate)) candidates.push(candidate);
    });
  });
  const fallbackUrl = Array.isArray(url) ? (url[0] || '') : url;

  for (let i = 0; i < candidates.length; i++) {
    await waitIfPaused();
    if (shouldStop()) {
      return { success: false, stopped: true, sourceUrl: '', quality: 'SD' };
    }
    const candidate = candidates[i];
    const probe = await fetchCandidateBlob(candidate, timeoutMs);
    if (!probe.success) {
      if (probe.error.includes('aborted') && shouldStop()) {
        return { success: false, stopped: true, sourceUrl: candidate, quality: 'SD' };
      }
      console.warn('[灵感爆爆爆] 下载重试:', filename, probe.error);
      continue;
    }

    if (shouldStop()) {
      return { success: false, stopped: true, sourceUrl: probe.url, quality: 'SD' };
    }
    const objectUrl = URL.createObjectURL(probe.blob);
    const a = document.createElement('a');
    a.href = objectUrl;
    a.download = filename;
    a.click();
    URL.revokeObjectURL(objectUrl);

    const isHdBySize = probe.width >= 1080 || probe.height >= 1080 || probe.size >= 450 * 1024;
    const quality = isHdBySize ? 'HD' : 'SD';
    return {
      success: true,
      sourceUrl: probe.url,
      quality,
      size: probe.size,
      width: probe.width,
      height: probe.height,
    };
  }

  try {
    await waitIfPaused();
    if (shouldStop()) {
      return { success: false, stopped: true, sourceUrl: '', quality: 'SD' };
    }
    const response = await fetch(fallbackUrl, { mode: 'cors' });
    if (!response.ok) {
      throw new Error(`HTTP ${response.status}`);
    }
    const blob = await response.blob();
    if (shouldStop()) {
      return { success: false, stopped: true, sourceUrl: fallbackUrl, quality: 'SD' };
    }
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = filename;
    a.click();
    URL.revokeObjectURL(a.href);
    return { success: true, sourceUrl: fallbackUrl, quality: 'SD' };
  } catch (err) {
    console.warn('[灵感爆爆爆] 下载失败:', filename, err);
    return { success: false, sourceUrl: fallbackUrl, quality: 'SD', error: err.message };
  }
}

/**
 * 下载笔记的所有媒体文件（图片+视频）
 */
export async function downloadNoteMedia(note) {
  const prefix = (note.title || note.noteId).replace(/[\\/:*?"<>|]/g, '_').slice(0, 30);
  const downloadResults = [];

  // 下载图片
  if (note.images && note.images.length > 0) {
    for (let i = 0; i < note.images.length; i++) {
      const ext = note.images[i].match(/\.(jpg|jpeg|png|gif|webp)/i)?.[1] || 'jpg';
      const result = await downloadMediaFile(note.images[i], `${prefix}_图${i + 1}.${ext}`);
      downloadResults.push(result);
      // 每张图片间隔 500ms，避免浏览器拦截
      await new Promise(r => setTimeout(r, 500));
    }
  }

  // 下载视频
  if (note.video) {
    const result = await downloadMediaFile(note.video, `${prefix}_视频.mp4`);
    downloadResults.push(result);
  }

  return downloadResults;
}

/**
 * 提取博主的统一标识符（优先 handle > redId > douyinId）
 */
export function getUnifiedAuthorHandle(author = {}) {
  return String(author.handle || author.redId || author.douyinId || '').trim();
}

export function normalizeServerUrl(serverUrl = '', fallback = '') {
  const raw = String(serverUrl || fallback || '').trim();
  return raw
    .replace(/\/+$/, '')
    .replace(/^(?!https?:\/\/)/, 'http://');
}
