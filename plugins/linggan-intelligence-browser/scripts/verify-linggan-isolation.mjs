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
for (const permission of ['cookies', 'downloads', 'alarms', 'declarativeNetRequest', 'declarativeNetRequestWithHostAccess', 'notifications']) {
  assert(!(manifest.permissions || []).includes(permission), `unsupported legacy capability must not retain ${permission} permission`);
}

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
assert(activeContentSource.includes('injectLingganPendingPageControls'), 'active content entry must retain the Linggan-owned pending UI shell');
assert(activeContentSource.includes('createLingganPendingResult'), 'active content entry must return explicit pending results');
for (const forbiddenImport of [
  'contentDataRuntime',
  'messageListener',
  'xhsPageController',
  'douyinRuntime',
  'douyinBatchMessageHandlers',
  'shared/messaging',
  'managedTaskController',
  'noteCollector',
  'commentCollector',
  'authorCollector',
  'batchController',
]) {
  assert(!activeContentSource.includes(forbiddenImport), `active content entry must not load retired collector/workbench runtime: ${forbiddenImport}`);
}
const dashboardBridgeSource = read(path.join(root, 'src/content/dashboardBridge.js'));
assert(dashboardBridgeSource.includes("createLingganPendingResult('media_download')"), 'dashboard media action must return Linggan pending state');
assert(!dashboardBridgeSource.includes('downloadNoteMediaFromRecord'), 'dashboard media action must not invoke legacy downloader');

const dist = path.join(root, 'dist');
assert(existsSync(dist), 'dist is missing; run build first');
for (const file of walk(dist).filter((file) => /\.(?:js|html|json|css)$/.test(file))) {
  const contents = read(file);
  assert(!/lingganboom\.fun/i.test(contents), `built artifact includes old workbench host: ${path.relative(root, file)}`);
  assert(!/api\/(?:execution-stations|plugin-authorization)/i.test(contents), `built artifact includes old workbench endpoint: ${path.relative(root, file)}`);
}

const activeContentBundles = walk(dist)
  .filter((file) => /(?:^|\/)content(?:\.[^.]+)?\.js$/.test(file.replaceAll('\\', '/')));
assert(activeContentBundles.length > 0, 'built active content bundle is missing');
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
for (const file of activeContentBundles) {
  const contents = read(file);
  for (const token of forbiddenRuntimeTokens) {
    assert(!token.test(contents), `active content bundle retains retired workbench runtime token ${token}: ${path.relative(root, file)}`);
  }
}

console.log('Linggan isolation verified: legacy source is retained, active content bundle has no retired workbench runtime, and old workbench host is absent.');
