import { parseCount, extractNoteId } from '../../shared/utils.js';
import { readEmbeddedXhsNoteDetailMap } from './embeddedInitialState.js';

export const POPUP_SELECTORS = [
  '.note-detail-mask',
  '.note-container',
  '#noteContainer',
  '.note-detail',
  '[class*="note-detail"]',
  '[class*="noteDetail"]',
  '#detail-container',
  '.detail-container',
  '[id*="noteContainer"]',
];

export const CLOSE_SELECTORS = [
  '.close-circle .close.close-mask-dark',
  '.close-circle',
  '[class*="close-circle"]',
  '.note-detail-mask',
  '[class*="note-detail-mask"]',
  '[class*="back-btn"]',
  '.back-icon',
];

export function isNoteDetailReady() {
  for (const sel of POPUP_SELECTORS) {
    try {
      const el = document.querySelector(sel);
      if (el && el.offsetWidth > 0 && el.offsetHeight > 0) return true;
    } catch {
      // 忽略无效选择器
    }
  }
  return /\/explore\/[a-z0-9]+/i.test(window.location.pathname)
    || /\/discovery\/item\/[a-z0-9]+/i.test(window.location.pathname);
}

export function isPopupOpen() {
  for (const sel of POPUP_SELECTORS) {
    try {
      const el = document.querySelector(sel);
      if (el && el.offsetWidth > 0 && el.offsetHeight > 0) return true;
    } catch {
      // 忽略无效选择器
    }
  }
  return false;
}

export function isElementVisible(el) {
  if (!el) return false;
  try {
    const rect = typeof el.getBoundingClientRect === 'function'
      ? el.getBoundingClientRect()
      : { width: el.offsetWidth || 0, height: el.offsetHeight || 0 };
    const width = Number(rect?.width ?? el.offsetWidth ?? 0);
    const height = Number(rect?.height ?? el.offsetHeight ?? 0);
    if (width <= 0 || height <= 0) return false;
    const view = el.ownerDocument?.defaultView || window;
    const style = view?.getComputedStyle?.(el);
    if (!style) return true;
    if (style.display === 'none' || style.visibility === 'hidden') return false;
    if (Number(style.opacity || '1') === 0) return false;
    return true;
  } catch {
    return false;
  }
}

function getElementDepth(el) {
  let depth = 0;
  let cursor = el?.parentElement || null;
  while (cursor) {
    depth += 1;
    cursor = cursor.parentElement || null;
  }
  return depth;
}

function getVisibleMatches(selectors, root = document) {
  const candidates = [];
  const seen = new Set();
  selectors.forEach((selector) => {
    let matches = [];
    try {
      matches = Array.from(root.querySelectorAll(selector));
    } catch {
      matches = [];
    }
    matches.forEach((el) => {
      if (!el || seen.has(el) || !isElementVisible(el)) return;
      seen.add(el);
      candidates.push(el);
    });
  });
  return candidates;
}

export function getActiveNoteDetailRoot(root = document) {
  const candidates = getVisibleMatches(POPUP_SELECTORS, root);
  if (candidates.length === 0) return null;

  return candidates.sort((a, b) => {
    const depthDiff = getElementDepth(b) - getElementDepth(a);
    if (depthDiff !== 0) return depthDiff;
    const aRect = a.getBoundingClientRect?.() || { width: a.offsetWidth || 0, height: a.offsetHeight || 0 };
    const bRect = b.getBoundingClientRect?.() || { width: b.offsetWidth || 0, height: b.offsetHeight || 0 };
    const areaDiff = (aRect.width * aRect.height) - (bRect.width * bRect.height);
    return areaDiff;
  })[0];
}

function readCommentMetaText(scope) {
  return String(scope?.innerText || '').slice(0, 6000);
}

export function getActiveCommentsContext(root = document) {
  const detailRoot = getActiveNoteDetailRoot(root);
  const scopes = [detailRoot, root].filter(Boolean);
  const commentSelectors = ['.comments-container', '[class*="comments"]'];
  let best = null;

  scopes.forEach((scope, scopeIndex) => {
    const matches = getVisibleMatches(commentSelectors, scope);
    matches.forEach((container) => {
      const commentItems = container.querySelectorAll?.('.parent-comment, .comment-item') || [];
      const text = readCommentMetaText(container);
      const hasMeta = /共\s*\d+\s*条评论/.test(text) || /- THE END -/.test(text);
      const hasExplicitEmptyState = /(暂无评论|还没有评论|还木有评论|还没有人评论|暂无回复)/.test(text);
      const score = (scope === detailRoot ? 100 : 0)
        + (scopeIndex === 0 ? 30 : 0)
        + (commentItems.length > 0 ? 20 : 0)
        + (hasMeta ? 10 : 0)
        + (hasExplicitEmptyState ? 6 : 0)
        + Math.min(commentItems.length, 20);
      if (!best || score > best.score) {
        best = {
          root: detailRoot,
          container,
          hasCommentItems: commentItems.length > 0,
          hasCommentMeta: hasMeta,
          hasExplicitEmptyState,
          text,
          score,
        };
      }
    });
  });

  return best || {
    root: detailRoot,
    container: null,
    hasCommentItems: false,
    hasCommentMeta: false,
    hasExplicitEmptyState: false,
    text: '',
    score: 0,
  };
}

export function shouldWaitForNoteState(pathname = window.location.pathname, noteId = '') {
  const path = String(pathname || '');
  const id = String(noteId || '').trim();
  if (/\/explore\/[a-z0-9]+/i.test(path) || /\/discovery\/item\/[a-z0-9]+/i.test(path) || /\/search_result\/[a-z0-9]+/i.test(path)) {
    return !id || path.includes(id);
  }
  return false;
}

export function isRiskControlPage() {
  const href = window.location.href || '';
  if (/captcha|verify|verification|security|safety|risk/i.test(href)) return true;
  if (/error_code=300017/i.test(href)) return true;
  if (/website-login\/error/i.test(href)) return true;
  const text = (document.body?.innerText || '').slice(0, 3000);
  if (/(安全验证|异常访问|访问受限|操作频繁|请完成验证|请稍后再试)/.test(text)) return true;
  if (/300017/.test(text) && /(限制|频繁|异常)/.test(text)) return true;
  return false;
}

export function isErrorCode300017() {
  const href = window.location.href || '';
  if (/error_code=300017/i.test(href)) return true;
  if (/website-login\/error/i.test(href)) return true;
  const text = (document.body?.innerText || '').slice(0, 3000);
  return /访问链接异常/.test(text) || (/300017/.test(text) && /(限制|频繁|异常)/.test(text));
}

export function waitForNoteState(noteId, timeout = 10000) {
  return new Promise((resolve) => {
    const start = Date.now();
    const check = () => {
      try {
        const noteMap = window.__INITIAL_STATE__?.note?.noteDetailMap
          || readEmbeddedXhsNoteDetailMap(document);
        if (noteMap && noteMap[noteId]) {
          resolve(true);
          return;
        }
      } catch {
        // ignore
      }
      if (Date.now() - start > timeout) {
        resolve(false);
        return;
      }
      setTimeout(check, 180);
    };
    check();
  });
}

export function findNoteElementById(noteId, containerSelector) {
  const sections = document.querySelectorAll(`${containerSelector} section`);
  for (const section of sections) {
    const coverLink = section.querySelector('a.cover');
    if (!coverLink) continue;
    const href = coverLink.getAttribute('href') || '';
    const id = extractNoteId(href);
    if (id === noteId) return section;
  }
  return null;
}

export function formatCompactCount(value) {
  const n = Number(value || 0);
  if (!Number.isFinite(n) || n <= 0) return '0';
  if (n >= 100000000) return `${(n / 100000000).toFixed(n >= 1000000000 ? 0 : 1).replace(/\.0$/, '')}亿`;
  if (n >= 10000) return `${(n / 10000).toFixed(n >= 100000 ? 0 : 1).replace(/\.0$/, '')}万`;
  return String(Math.round(n));
}
