import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  POPUP_SYNC_TO_WORKBENCH_TIMEOUT_MS,
  sendToBackground,
  toFriendlyError,
} from '../src/popup/utils.js';

const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

test('popup current-content section collapses duplicate disabled copy into a single empty-state hint', () => {
  const source = fs.readFileSync(path.join(projectRoot, 'src/popup/components/ActionButtons.jsx'), 'utf8');
  assert.match(source, /action-empty-state/);
  assert.match(source, /当前页还不是可执行采集面板/);
  assert.match(source, /action-empty-tags/);
  assert.match(source, /if \(!hasCurrentContentActions\)/);
  assert.match(source, /busyPrimary/);
  assert.match(source, /busySecondary/);
});

test('popup data tab keeps cookie status compact and account count badge consistent', () => {
  const componentSource = fs.readFileSync(path.join(projectRoot, 'src/popup/components/CookieAccountSection.jsx'), 'utf8');
  const styleSource = fs.readFileSync(path.join(projectRoot, 'src/popup/popup.css'), 'utf8');

  assert.match(componentSource, /Cookie 状态/);
  assert.match(componentSource, /采集账号/);
  assert.match(componentSource, /currentPlatform/);
  assert.match(componentSource, /cookie-status-card/);
  assert.match(componentSource, /cookie-status-row single-platform/);
  assert.match(componentSource, /cookie-fetch-btn/);
  assert.match(componentSource, /获取 Cookie/);
  assert.match(componentSource, /section-count-badge/);
  assert.match(componentSource, /个账号/);
  assert.match(styleSource, /\.cookie-status-card/);
  assert.match(styleSource, /\.cookie-status-row\.single-platform/);
  assert.match(styleSource, /grid-template-columns:\s*minmax\(0,\s*1fr\)\s+auto/);
  assert.match(styleSource, /\.cookie-fetch-btn/);
  assert.match(styleSource, /\.section-count-badge/);
  assert.match(styleSource, /\.data-subsection/);
});

test('popup header uses a left-stage centered banner with the title under the right-side controls', () => {
  const componentSource = fs.readFileSync(path.join(projectRoot, 'src/popup/App.jsx'), 'utf8');
  const styleSource = fs.readFileSync(path.join(projectRoot, 'src/popup/popup.css'), 'utf8');

  assert.match(componentSource, /header-brand-stage/);
  assert.match(componentSource, /header-side/);
  assert.match(componentSource, /header-controls/);
  assert.match(componentSource, /header-copy/);
  assert.match(componentSource, /header-brand-banner-shell/);
  assert.match(componentSource, /<h1>灵感爆爆爆<\/h1>/);
  assert.doesNotMatch(componentSource, /header-brand-mark/);
  assert.doesNotMatch(componentSource, /小红书数据采集工具箱/);

  assert.match(styleSource, /\.header-brand-stage/);
  assert.match(styleSource, /grid-template-columns:\s*minmax\(0,\s*1fr\)\s+auto/);
  assert.match(styleSource, /align-items:\s*stretch/);
  assert.match(styleSource, /--header-content-height:\s*74px/);
  assert.match(styleSource, /\.header-side/);
  assert.match(styleSource, /display:\s*grid/);
  assert.match(styleSource, /justify-items:\s*end/);
  assert.match(styleSource, /padding-bottom:\s*calc\(\(var\(--header-content-height\)\s*-\s*var\(--header-banner-shell-height\)\)\s*\/\s*2\s*-\s*1px\)/);
  assert.match(styleSource, /font-family:\s*var\(--headline\)/);
  assert.match(styleSource, /font-size:\s*19px/);
  assert.match(styleSource, /transform:\s*translateY\(1px\)/);
  assert.match(styleSource, /\.header-brand-banner-shell/);
  assert.doesNotMatch(styleSource, /\.header-brand-mark/);
  assert.doesNotMatch(styleSource, /\.header-topline/);
  assert.doesNotMatch(styleSource, /\.popup-header \.subtitle/);
});

test('xhs batch dialog titles use unified 批量采集 copy', () => {
  const source = fs.readFileSync(path.join(projectRoot, 'src/content/xhsPageController.js'), 'utf8');
  assert.match(source, /title:\s*'批量采集笔记'/);
  assert.match(source, /title:\s*'批量采集评论'/);
  assert.doesNotMatch(source, /批量笔记设置/);
  assert.doesNotMatch(source, /批量评论设置/);
});

test('popup placeholders, disabled buttons, and checkbox controls use unified styling hooks', () => {
  const addAccountSource = fs.readFileSync(path.join(projectRoot, 'src/popup/components/AddAccountModal.jsx'), 'utf8');
  const styleSource = fs.readFileSync(path.join(projectRoot, 'src/popup/popup.css'), 'utf8');

  assert.match(addAccountSource, /className="add-account-input"/);
  assert.match(addAccountSource, /className="add-account-textarea"/);
  assert.match(addAccountSource, /placeholder="例如：100"/);
  assert.match(addAccountSource, /modal-inline-notice/);
  assert.doesNotMatch(addAccountSource, /alert\(/);

  assert.match(styleSource, /\.flywheel-input::placeholder/);
  assert.match(styleSource, /\.add-account-input::placeholder/);
  assert.match(styleSource, /\.popup-btn:disabled/);
  assert.match(styleSource, /\.popup-btn\.is-busy/);
  assert.match(styleSource, /background:\s*#ece7df/);
  assert.match(styleSource, /\.batch-checkbox input \{/);
  assert.match(styleSource, /appearance:\s*none/);
});

test('popup tool buttons share one horizontal row instead of stacking as three full-width rows', () => {
  const componentSource = fs.readFileSync(path.join(projectRoot, 'src/popup/App.jsx'), 'utf8');
  const styleSource = fs.readFileSync(path.join(projectRoot, 'src/popup/popup.css'), 'utf8');

  assert.match(componentSource, /className="bottom-section"/);
  assert.match(componentSource, /打开工作台/);
  assert.match(componentSource, /快速导出/);
  assert.match(componentSource, /数据维护/);

  assert.match(styleSource, /\.bottom-section \{/);
  assert.match(styleSource, /display:\s*grid/);
  assert.match(styleSource, /grid-template-columns:\s*repeat\(3,\s*minmax\(0,\s*1fr\)\)/);
  assert.match(styleSource, /\.bottom-section \.popup-btn \{/);
  assert.match(styleSource, /white-space:\s*nowrap/);
  assert.doesNotMatch(styleSource, /\.bottom-section \{\s*display:\s*flex;\s*flex-direction:\s*column/s);
});

test('popup batch modal remembers smart defaults and renders a live execution summary', () => {
  const modalSource = fs.readFileSync(path.join(projectRoot, 'src/popup/components/BatchSettingsModal.jsx'), 'utf8');
  const feedbackSource = fs.readFileSync(path.join(projectRoot, 'src/shared/feedback.js'), 'utf8');
  const styleSource = fs.readFileSync(path.join(projectRoot, 'src/popup/popup.css'), 'utf8');

  assert.match(modalSource, /inferPopupBatchDefaults/);
  assert.match(modalSource, /readPopupBatchSettings/);
  assert.match(modalSource, /writePopupBatchSettings/);
  assert.match(modalSource, /batch-plan-card/);
  assert.match(modalSource, /智能默认值/);

  assert.match(feedbackSource, /inferPopupBatchDefaults/);
  assert.match(feedbackSource, /summarizeBatchPlan/);
  assert.match(feedbackSource, /getPopupBatchSettingsStorageKey/);

  assert.match(styleSource, /\.batch-plan-card/);
  assert.match(styleSource, /\.batch-plan-badge/);
  assert.match(styleSource, /\.confirm-dialog-detail/);
});

test('popup notice uses structured icon and copy blocks instead of a single bare text line', () => {
  const noticeSource = fs.readFileSync(path.join(projectRoot, 'src/popup/components/Notice.jsx'), 'utf8');
  const styleSource = fs.readFileSync(path.join(projectRoot, 'src/popup/popup.css'), 'utf8');

  assert.match(noticeSource, /getFeedbackMeta/);
  assert.match(noticeSource, /popup-notice-icon/);
  assert.match(noticeSource, /popup-notice-copy/);
  assert.match(noticeSource, /popup-notice-close/);

  assert.match(styleSource, /\.popup-notice-icon/);
  assert.match(styleSource, /\.popup-notice-copy/);
  assert.match(styleSource, /\.popup-notice-close/);
});

test('popup workbench config presents one connection flow and hides manual pairing code binding', () => {
  const componentSource = fs.readFileSync(path.join(projectRoot, 'src/popup/components/FlywheelSection.jsx'), 'utf8');
  const appSource = fs.readFileSync(path.join(projectRoot, 'src/popup/App.jsx'), 'utf8');
  const styleSource = fs.readFileSync(path.join(projectRoot, 'src/popup/popup.css'), 'utf8');

  assert.match(componentSource, /内容工作台/);
  assert.match(componentSource, /placeholder="https:\/\/lingganboom\.fun"/);
  assert.match(componentSource, /线上正式站/);
  assert.match(componentSource, /本地 3000/);
  assert.match(componentSource, /最近一次使用的地址/);
  assert.match(componentSource, /flywheel-heading-side/);
  assert.match(componentSource, /flywheel-preset-row/);
  assert.match(componentSource, /flywheel-preset-chip/);
  assert.match(componentSource, /插件授权/);
  assert.match(componentSource, /输入授权码/);
  assert.match(componentSource, /连接插件/);
  assert.match(componentSource, /授权成功后会自动创建工位/);
  assert.match(componentSource, /按任务优先级接单/);
  assert.doesNotMatch(componentSource, /输入配对码/);
  assert.doesNotMatch(componentSource, /绑定工位/);
  assert.doesNotMatch(componentSource, /先完成插件授权，再使用内容工作台设置里生成的配对码绑定执行设备/);
  assert.match(componentSource, /全部同步到工作台/);

  assert.match(appSource, /CONTENT_WORKBENCH_PROD_URL/);
  assert.match(appSource, /CONTENT_WORKBENCH_LOCAL_URL/);
  assert.match(appSource, /handleUseWorkbenchPreset/);
  assert.match(appSource, /AUTHORIZE_PLUGIN_ACCESS/);
  assert.match(appSource, /CLEAR_PLUGIN_AUTHORIZATION/);
  assert.match(appSource, /请输入内容工作台设置里生成的授权码/);
  assert.match(appSource, /插件已连接，工位也已自动准备好/);
  assert.match(appSource, /内容工作台已就绪/);
  assert.doesNotMatch(appSource, /REGISTER_EXECUTION_STATION/);
  assert.doesNotMatch(appSource, /stationPairingCode/);

  assert.match(styleSource, /\.flywheel-heading-side/);
  assert.match(styleSource, /\.flywheel-preset-row/);
  assert.match(styleSource, /\.flywheel-preset-chip/);
  assert.match(styleSource, /\.flywheel-preset-chip\.active/);
});

test('popup execution station diagnostics expose local runtime state in user-facing copy', () => {
  const componentSource = fs.readFileSync(path.join(projectRoot, 'src/popup/components/FlywheelSection.jsx'), 'utf8');
  const backgroundSource = fs.readFileSync(path.join(projectRoot, 'src/background/index.js'), 'utf8');
  const outboxSource = fs.readFileSync(path.join(projectRoot, 'src/db/workbenchOutboxStore.js'), 'utf8');
  const styleSource = fs.readFileSync(path.join(projectRoot, 'src/popup/popup.css'), 'utf8');

  assert.match(backgroundSource, /pluginVersion:\s*getPluginVersion\(\)/);
  assert.match(backgroundSource, /identity:\s*summarizeStationIdentityForDiagnostics\(identity\)/);
  assert.match(backgroundSource, /currentTask[,}]/);
  assert.match(backgroundSource, /activeLockCount:\s*activeLocks\.length/);
  assert.match(backgroundSource, /unsentOutboxCount/);
  assert.match(outboxSource, /async countUnsent/);

  assert.match(componentSource, /插件版本/);
  assert.match(componentSource, /工位编号/);
  assert.match(componentSource, /当前任务/);
  assert.match(componentSource, /本机锁/);
  assert.match(componentSource, /待发送/);
  assert.match(componentSource, /死信（不再自动发送）/);
  assert.match(componentSource, /工位诊断/);
  assert.match(componentSource, /页面权限正常/);
  assert.match(componentSource, /缺少页面权限/);
  assert.match(componentSource, /需登录/);

  assert.match(styleSource, /\.station-diagnostics/);
  assert.match(styleSource, /\.station-diagnostic-grid/);
  assert.match(styleSource, /\.station-runtime-row/);
});

test('popup maps manual collection blockers to clear user messages', () => {
  assert.equal(
    toFriendlyError(new Error('account_busy')),
    '这个平台账号正在执行另一条采集，请等当前任务结束后再试。',
  );
  assert.equal(
    toFriendlyError(new Error('账号登录状态不可用：bad_cookie')),
    '当前平台账号需要重新登录，请先在对应平台页面完成登录，再回到插件继续采集。',
  );
  assert.equal(
    toFriendlyError(new Error('permission_denied')),
    '浏览器助手缺少当前平台页面权限，请重新授权插件后再试。',
  );
});

test('popup background messages time out with a visible user hint', async () => {
  const previousChrome = globalThis.chrome;
  globalThis.chrome = {
    runtime: {
      sendMessage() {},
    },
  };

  try {
    await assert.rejects(
      () => sendToBackground('registerExecutionStation', {}, { timeoutMs: 1 }),
      /插件后台暂时没有回应/,
    );
  } finally {
    globalThis.chrome = previousChrome;
  }
});

test('popup all-data workbench sync uses its dedicated long response window', () => {
  const appSource = fs.readFileSync(path.join(projectRoot, 'src/popup/App.jsx'), 'utf8');
  const syncCallStart = appSource.indexOf('sendToBackground(MSG.SYNC_TO_WORKBENCH');
  const syncCallSource = appSource.slice(syncCallStart, syncCallStart + 600);

  assert.equal(POPUP_SYNC_TO_WORKBENCH_TIMEOUT_MS, 120000);
  assert.notEqual(syncCallStart, -1);
  assert.match(syncCallSource, /timeoutMs:\s*POPUP_SYNC_TO_WORKBENCH_TIMEOUT_MS/);
});
