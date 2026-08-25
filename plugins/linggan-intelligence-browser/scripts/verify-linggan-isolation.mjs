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
  'src/popup/App.jsx',
  'src/popup/components/FlywheelSection.jsx',
  'src/content/index.js',
  'src/content/dashboardBridge.js',
  'src/content/xhsPageController.js',
  'src/platforms/douyin/index.js',
];
for (const file of activeEntryFiles) {
  const contents = read(path.join(root, file));
  assert(!/lingganboom\.fun/i.test(contents), `active runtime keeps old workbench host: ${file}`);
  assert(!/api\/execution-stations|api\/plugin-authorization|syncToWorkbench/i.test(contents), `active runtime keeps old workbench endpoint or fallback: ${file}`);
}
assert(read(path.join(root, 'webpack.config.cjs')).includes("background: './src/linggan/background.js'"), 'legacy background must not be the active service worker entry');
assert(read(path.join(root, 'src/content/xhsPageController.js')).includes('ensurePluginAuthorized'), 'XHS page actions must remain behind the Linggan pending guard');
assert(read(path.join(root, 'src/platforms/douyin/index.js')).includes('assertLingganCapability'), 'Douyin actions must remain behind the Linggan pending guard');

const dist = path.join(root, 'dist');
assert(existsSync(dist), 'dist is missing; run build first');
for (const file of walk(dist).filter((file) => /\.(?:js|html|json|css)$/.test(file))) {
  const contents = read(file);
  assert(!/lingganboom\.fun/i.test(contents), `built artifact includes old workbench host: ${path.relative(root, file)}`);
  assert(!/api\/(?:execution-stations|plugin-authorization)/i.test(contents), `built artifact includes old workbench endpoint: ${path.relative(root, file)}`);
}

console.log('Linggan isolation verified: legacy source is retained, active runtime is Linggan-owned and old workbench host is absent.');
