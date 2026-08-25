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

if (![...activeContentModules].some((name) => name.includes('./src/linggan/pageControls.jsx'))) {
  throw new Error('active content module graph does not include the Linggan pending UI shell');
}

const forbidden = [
  /(?:^|\/)src\/workbench\//,
  /(?:^|\/)src\/sync\//,
];
for (const moduleName of activeContentModules) {
  for (const pattern of forbidden) {
    if (pattern.test(moduleName.replaceAll('\\', '/'))) {
      throw new Error(`active content module graph loads retired workbench runtime: ${moduleName}`);
    }
  }
}

console.log(`active content module graph verified: ${activeContentModules.size} modules; no src/workbench/** or src/sync/** module paths`);
