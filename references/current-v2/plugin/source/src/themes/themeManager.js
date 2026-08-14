/**
 * 主题管理器 — 负责读取/写入/监听主题偏好
 * 支持 'default' 和 'ac-ui' 两套风格
 */

const THEME_KEY = 'lgboom_theme';

const listeners = new Set();

/** 当前缓存的主题值 */
let cachedTheme = 'default';

/**
 * 初始化主题管理器
 * 应在 popup 或 content script 启动时调用一次
 */
export async function initThemeManager() {
  const stored = await chrome.storage.local.get(THEME_KEY);
  cachedTheme = stored[THEME_KEY] || 'default';

  // 监听 storage 变化，自动同步主题
  chrome.storage.onChanged.addListener((changes, area) => {
    if (area !== 'local') return;
    if (changes[THEME_KEY]) {
      cachedTheme = changes[THEME_KEY].newValue || 'default';
      listeners.forEach((fn) => fn(cachedTheme));
    }
  });

  return cachedTheme;
}

/** 获取当前主题名称 */
export function getCurrentTheme() {
  return cachedTheme;
}

/** 设置主题 */
export async function setTheme(themeName) {
  cachedTheme = themeName;
  await chrome.storage.local.set({ [THEME_KEY]: themeName });
}

