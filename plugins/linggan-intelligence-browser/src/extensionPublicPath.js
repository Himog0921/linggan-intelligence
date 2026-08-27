/* global __webpack_public_path__ */

// 统一锁定动态分包的加载基址，避免在内容脚本场景下回退到页面站点域名。
if (typeof chrome !== 'undefined' && chrome.runtime?.getURL && typeof __webpack_public_path__ !== 'undefined') {
  __webpack_public_path__ = chrome.runtime.getURL('');
}
