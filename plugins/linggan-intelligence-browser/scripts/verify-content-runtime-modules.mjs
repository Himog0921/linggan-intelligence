import { execFileSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import path from 'node:path';

const webpackCli = path.join(process.cwd(), 'node_modules', 'webpack', 'bin', 'webpack.js');
if (!existsSync(webpackCli)) throw new Error('webpack CLI is missing; run npm ci first');

const rawStats = execFileSync(process.execPath, [
  webpackCli,
  '--config', 'webpack.config.cjs',
  '--mode', 'production',
  '--json',
], {
  cwd: process.cwd(),
  encoding: 'utf8',
  maxBuffer: 64 * 1024 * 1024,
});
const stats = JSON.parse(rawStats);
const contentChunkIds = new Set(
  (stats.chunks || [])
    .filter((chunk) => (chunk.names || []).includes('content'))
    .map((chunk) => chunk.id),
);
if (contentChunkIds.size === 0) throw new Error('webpack stats has no content entry chunk');

const activeContentModules = new Set();
function collectModules(modules = [], inheritedChunks = []) {
  for (const module of modules) {
    const chunks = Array.isArray(module.chunks) && module.chunks.length > 0
      ? module.chunks
      : inheritedChunks;
    if (chunks.some((chunk) => contentChunkIds.has(chunk))) {
      activeContentModules.add(String(module.name || module.identifier || ''));
      collectModules(module.modules || [], chunks);
    }
  }
}
collectModules(stats.modules || []);

for (const required of [
  './src/linggan/contentRuntimeAdapter.js',
  './src/runtime/collectorReceiptSink.js',
  './src/content/xhsPageController.js',
  './src/platforms/xhs/noteCollector.js',
]) {
  if (![...activeContentModules].some((name) => name.includes(required))) {
    throw new Error(`active content module graph is missing the retained collector/runtime bridge: ${required}`);
  }
}

const forbidden = [
  /(?:^|\/)src\/background\/index\.js$/,
  /(?:^|\/)src\/workbench\/runtime\/(?:taskPoller|taskLeaseClient|outbox|sync)\.js$/,
  /(?:^|\/)src\/sync\//,
];
for (const moduleName of activeContentModules) {
  for (const pattern of forbidden) {
    if (pattern.test(moduleName.replaceAll('\\', '/'))) {
      throw new Error(`active content module graph loads retired workbench runtime: ${moduleName}`);
    }
  }
}

console.log(`active content module graph verified: ${activeContentModules.size} modules; retained page collectors enter Linggan Runtime and no retired poller/lease/sync runtime is loaded.`);
