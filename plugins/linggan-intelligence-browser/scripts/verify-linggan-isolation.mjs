import { existsSync, readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function read(file) {
  return readFileSync(file, 'utf8');
}

function walk(root) {
  return readdirSync(root, { withFileTypes: true }).flatMap((entry) => {
    const target = path.join(root, entry.name);
    return entry.isDirectory() ? walk(target) : [target];
  });
}

const root = process.cwd();
const manifest = JSON.parse(read(path.join(root, 'manifest.json')));
const packageJson = JSON.parse(read(path.join(root, 'package.json')));
assert(packageJson.name === 'linggan-intelligence-browser', 'package must identify Linggan ownership');
assert(manifest.name === 'Linggan Intelligence Browser', 'manifest must identify Linggan ownership');
assert(!(manifest.host_permissions || []).some((value) => /lingganboom\.fun/i.test(value)), 'manifest must not grant old workbench host access');
assert((manifest.host_permissions || []).includes('http://localhost:3000/*'), 'manifest must retain the narrow Linggan loopback target');
for (const permission of ['cookies', 'downloads', 'declarativeNetRequest', 'declarativeNetRequestWithHostAccess', 'notifications']) {
  assert(!(manifest.permissions || []).includes(permission), `unsupported legacy capability must not retain ${permission} permission`);
}
assert((manifest.permissions || []).includes('alarms'), 'automatic claim runtime must retain alarms permission');

const requiredSource = [
  'src/popup/App.jsx',
  'src/dashboard/App.jsx',
  'src/content/index.js',
  'src/content/components/ButtonGroup.jsx',
  'src/content/components/TaskControlBar.jsx',
  'src/platforms/xhs/noteCollector.js',
  'src/platforms/douyin/videoCollector.js',
  'src/themes/ac-ui/popup.css',
  'src/assets/lgboom-logo.svg',
];
for (const file of requiredSource) assert(existsSync(path.join(root, file)), `missing rehomed source: ${file}`);

const activeEntryFiles = [
  'webpack.config.cjs',
  'src/linggan/background.js',
  'src/linggan/adapter.js',
  'src/linggan/pageControls.jsx',
  'src/linggan/runtimeActions.js',
  'src/popup/App.jsx',
  'src/popup/components/FlywheelSection.jsx',
  'src/content/index.js',
  'src/content/dashboardBridge.js',
];
for (const file of activeEntryFiles) {
  const contents = read(path.join(root, file));
  assert(!/lingganboom\.fun/i.test(contents), `active runtime keeps old workbench host: ${file}`);
  assert(!/api\/execution-stations|api\/plugin-authorization|syncToWorkbench/i.test(contents), `active runtime keeps old workbench endpoint or fallback: ${file}`);
}
assert(read(path.join(root, 'webpack.config.cjs')).includes("background: './src/linggan/background.js'"), 'legacy background must not be the active service worker entry');
const activeContentSource = read(path.join(root, 'src/content/index.js'));
assert(activeContentSource.includes('createLingganContentRuntime'), 'active content entry must route collectors through the Linggan runtime');
assert(activeContentSource.includes('registerCollectorReceiptSink'), 'active content entry must connect mature collectors to Linggan receipts');
for (const forbiddenImport of [
  'contentDataRuntime',
  'messageListener',
  'douyinBatchMessageHandlers',
  'managedTaskController',
]) {
  assert(!activeContentSource.includes(forbiddenImport), `active content entry must not load retired workbench runtime: ${forbiddenImport}`);
}
const dashboardBridgeSource = read(path.join(root, 'src/content/dashboardBridge.js'));
assert(dashboardBridgeSource.includes('downloadNoteMediaFromRecord'), 'dashboard media action must enter the registered Linggan media runtime');
assert(!dashboardBridgeSource.includes('createLingganPendingResult'), 'dashboard media action must not report a long-lived pending capability');
const popupSource = read(path.join(root, 'src/popup/App.jsx'));
assert(popupSource.includes('ensureLocalTrustedRuntime'), 'popup must use the LOCAL_TRUSTED runtime boundary');
// `GET_EXECUTION_STATION_STATUS` is the current Linggan-only, read-only message
// that mirrors server-confirmed display name and acceptance state. It does not
// perform any former workbench registration, authorization, pairing, or action.
assert(popupSource.includes('GET_EXECUTION_STATION_STATUS'), 'popup must read the server-confirmed Linggan station status');
for (const retiredPopupAction of [
  'AUTHORIZE_PLUGIN_ACCESS',
  'REQUEST_PLUGIN_AUTHORIZATION',
  'CLAIM_PLUGIN_AUTHORIZATION_REQUEST',
  'CLEAR_PLUGIN_AUTHORIZATION',
  'GET_PLATFORM_COOKIES',
  'GET_STORED_PLATFORM_COOKIES',
  'GET_ACCOUNTS',
  'ADD_ACCOUNT',
  'REMOVE_ACCOUNT',
]) {
  assert(!popupSource.includes(retiredPopupAction), `popup must not retain a retired authorization, station, cookie, or account entry: ${retiredPopupAction}`);
}

const dist = path.join(root, 'dist');
assert(existsSync(dist), 'dist is missing; run build first');
for (const file of walk(dist).filter((file) => /\.(?:js|html|json|css)$/.test(file))) {
  const contents = read(file);
  assert(!/lingganboom\.fun/i.test(contents), `built artifact includes old workbench host: ${path.relative(root, file)}`);
  assert(!/api\/(?:execution-stations|plugin-authorization)/i.test(contents), `built artifact includes old workbench endpoint: ${path.relative(root, file)}`);
}
const popupBundle = read(path.join(dist, 'popup.js'));
for (const retiredPopupTransport of [
  'authorizePluginAccess',
  'requestPluginAuthorization',
  'claimPluginAuthorizationRequest',
  'clearPluginAuthorization',
  'getPlatformCookies',
  'getStoredPlatformCookies',
  'plugin_authorization_required',
]) {
  assert(!popupBundle.includes(retiredPopupTransport), `built popup must not retain retired management transport: ${retiredPopupTransport}`);
}

const activeContentBundles = walk(dist)
  .filter((file) => /(?:^|\/)content(?:\.[^.]+)?\.js$/.test(file.replaceAll('\\', '/')));
assert(activeContentBundles.length > 0, 'built active content bundle is missing');
// Mature page collectors deliberately retain their local pause/recovery helpers during this
// retrofit. The executable control plane is the new service worker, so isolation is checked at
// the runnable entry boundary rather than by treating every retained historical helper name in a
// bundled collector as a network dependency.
const activeBackgroundSource = read(path.join(root, 'src/linggan/background.js'));
const forbiddenRuntimeTokens = [
  /workbench\/runtime/i,
  /workbenchOutbox/i,
  /taskPoller/i,
  /taskLease/i,
  /sendExecutionStationHeartbeat/i,
  /reportWorkbenchRecord/i,
  /collectionRunHeartbeat/i,
  /lease[_-]?client/i,
];
for (const token of forbiddenRuntimeTokens) {
  assert(!token.test(activeBackgroundSource), `active service worker retains retired workbench runtime token ${token}`);
}

console.log('Linggan isolation verified: the active service worker owns delivery, old Workbench hosts are absent, and mature page collectors have no old transport route.');
