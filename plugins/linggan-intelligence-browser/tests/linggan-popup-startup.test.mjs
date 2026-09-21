import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import test from 'node:test';

import { createPopupStartupBoundary, popupPluginVersionLabel, popupStartupRecovery } from '../src/popup/startupRecovery.js';

const require = createRequire(import.meta.url);
const { transformSync } = require('@babel/core');
const appSource = readFileSync(new URL('../src/popup/App.jsx', import.meta.url), 'utf8');
const popupHtml = readFileSync(new URL('../src/popup/popup.html', import.meta.url), 'utf8');
const popupCss = readFileSync(new URL('../src/popup/popup.css', import.meta.url), 'utf8');
const webpackConfig = readFileSync(new URL('../webpack.config.cjs', import.meta.url), 'utf8');

function createReactHarness() {
  class Component {
    constructor(props) {
      this.props = props;
      this.state = {};
    }
  }

  const react = {
    Component,
    Fragment: Symbol('fragment'),
    createElement(type, props, ...children) {
      return { type, props: props || {}, children };
    },
    useState(value) {
      return [typeof value === 'function' ? value() : value, () => {}];
    },
    useEffect() {},
    useCallback(callback) {
      return callback;
    },
    useMemo(factory) {
      return factory();
    },
    useRef(value) {
      return { current: value };
    },
  };
  return { __esModule: true, default: react, ...react };
}

function moduleWithDefault() {
  return { __esModule: true, default: () => null };
}

function executeFirstPopupRender(source, runtimeNotice) {
  const compiled = transformSync(source, {
    filename: 'App.jsx',
    presets: [require.resolve('@babel/preset-react')],
    plugins: [require.resolve('@babel/plugin-transform-modules-commonjs')],
  }).code;
  const react = createReactHarness();
  const componentModule = moduleWithDefault();
  const modules = {
    react,
    '../extensionPublicPath.js': {},
    '../shared/constants.js': { MSG: { GET_STATS: 'GET_STATS' }, COMMENT_DEPTH_MODE: { ALL_REPLIES: 'all_replies', TWO_LEVEL: 'two_level' } },
    '../linggan/adapter.js': { formatLingganRuntimeNotice: runtimeNotice },
    '../linggan/runtimeActions.js': { LINGGAN_RUNTIME_ACTION: {} },
    '../linggan/controlReceipt.js': { requireControlReceipt() {} },
    '../shared/brandAssets.js': { BRAND_ASSETS: { banner: 'banner' }, getBrandAssetUrl() { return ''; } },
    '../themes/themeManager.js': { initThemeManager() {}, setTheme() {}, getCurrentTheme() { return 'default'; } },
    './utils.js': {
      PLATFORM: { XHS: 'xhs', DOUYIN: 'douyin', UNKNOWN: 'unknown' },
      PAGE_MODE: { SEARCH: 'search', UNKNOWN: 'unknown' },
      detectPlatformByUrl() { return 'unknown'; }, getModeFromUrl() { return 'unknown'; }, getPageCapabilities() { return {}; },
      getPrimaryActionWarning() { return ''; }, getSecondaryActionWarning() { return ''; }, getBatchActionWarning() { return ''; },
      toFriendlyError(error) { return String(error); }, inferProgressStage() { return {}; },
      sendToTab() {}, sendToBackground() {}, unwrapTabResponseData() {},
      getPageContextText() { return { scene: '', hint: '', tags: [] }; }, isDouyinVideoUrl() { return false; }, isDouyinStrictDetailUrl() { return false; },
    },
  };
  for (const component of [
    './components/TabNav.jsx', './components/StatsSection.jsx', './components/ActionButtons.jsx',
    './components/ProgressSection.jsx', './components/PageContextInfo.jsx', './components/Notice.jsx',
    './components/FlywheelSection.jsx', './components/BatchSettingsModal.jsx', './components/ConfirmModal.jsx',
  ]) modules[component] = componentModule;

  const module = { exports: {} };
  const localRequire = (name) => {
    if (!Object.hasOwn(modules, name)) throw new Error(`unexpected_popup_dependency:${name}`);
    return modules[name];
  };
  new Function('require', 'module', 'exports', compiled)(localRequire, module, module.exports);
  return module.exports.default();
}

function collectText(node, values = []) {
  if (typeof node === 'string') values.push(node);
  if (node && typeof node === 'object') {
    for (const child of node.children || []) collectText(child, values);
  }
  return values.join(' ');
}

test('the v0.4.0 missing formatter import fails at the actual first popup render, while the fixed App renders normally', () => {
  const v040Source = appSource.replace(
    "import { formatLingganRuntimeNotice } from '../linggan/adapter.js';\n",
    '',
  );
  assert.throws(
    () => executeFirstPopupRender(v040Source, () => 'unused'),
    ReferenceError,
  );
  const normalPopup = executeFirstPopupRender(appSource, () => 'runtime notice');
  assert.equal(normalPopup.type, 'div');
  assert.equal(normalPopup.props.className, 'popup-container');
});

test('the startup boundary has a normal path and a truthful no-new-action recovery path', () => {
  const react = createReactHarness().default;
  const PopupStartupBoundary = createPopupStartupBoundary(react);
  const normalChild = { type: 'normal-popup' };
  const boundary = new PopupStartupBoundary({ children: normalChild });
  assert.equal(boundary.render(), normalChild);

  boundary.state = PopupStartupBoundary.getDerivedStateFromError(new Error('fixture_startup_error'));
  const fallback = boundary.render();
  const fallbackText = collectText(fallback);
  assert.equal(fallback.type, 'main');
  assert.match(fallbackText, /本机状态目前未知，无法确认是否已读取页面信息/);
  assert.match(fallbackText, /本提示没有发起新的采集或传输/);
  // 版本不写死：读不到清单时就如实说不知道，绝不编一个数字。
  assert.match(fallbackText, /重新加载 Linggan Intelligence Browser（本机已安装 v未知）/);
  assert.doesNotMatch(fallbackText, /本次没有读取、采集或传输任何数据/);
});

test('the recovery surface uses the copied LIDS tokens without adding another visual token set', () => {
  const recoveryCss = popupCss.match(/\.popup-startup-failure\s*\{[\s\S]*?\n\}/)?.[0] || '';
  assert.match(popupHtml, /href="themes\/lids-tokens\.css"/);
  assert.match(webpackConfig, /apps\/api\/src\/local_web\/lids_tokens\.css/);
  assert.match(recoveryCss, /var\(--lgi-canvas-hi\)/);
  assert.match(recoveryCss, /var\(--lgi-font-sans\)/);
  assert.doesNotMatch(recoveryCss, /var\(--(?!lgi-)/);
  assert.doesNotMatch(recoveryCss, /#[0-9a-f]{3,8}/i);
  assert.equal(popupStartupRecovery().version, 'v未知');
});

test('the recovery surface names the version Chrome actually loaded, never a stale literal', () => {
  const manifest = JSON.parse(readFileSync(new URL('../manifest.json', import.meta.url), 'utf8'));
  const originalChrome = globalThis.chrome;
  try {
    globalThis.chrome = { runtime: { getManifest: () => ({ version: manifest.version }) } };
    assert.equal(popupPluginVersionLabel(), `v${manifest.version}`);
    assert.match(popupStartupRecovery().recovery, new RegExp(`本机已安装 v${manifest.version.replaceAll('.', '\\.')}`));

    // 读得到清单但里面不是版本号：同样说不知道，不把别的东西当版本。
    globalThis.chrome = { runtime: { getManifest: () => ({ version: 'current' }) } };
    assert.equal(popupPluginVersionLabel(), 'v未知');

    // 清单接口本身抛错（旧版浏览器/权限变化）：说不知道，而不是让渲染崩掉。
    globalThis.chrome = { runtime: { getManifest: () => { throw new Error('fixture_manifest_unavailable'); } } };
    assert.equal(popupPluginVersionLabel(), 'v未知');
  } finally {
    if (originalChrome === undefined) delete globalThis.chrome;
    else globalThis.chrome = originalChrome;
  }
});
