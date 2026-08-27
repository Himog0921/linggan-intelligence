// 工位分叉构建。
//
// 官方版本线（0.4.8 及其后续）属于另一条并行的插件工作流，本脚本不碰它：源码里的
// manifest.json / package.json 保持不动，版本号只在产物里改写。
//
// 产物用四段版本 0.4.8.1，它排在 0.4.8 之后、0.4.9 之前，因此另一条线无论发 0.4.9
// 还是 0.5.x 都不会与它撞号。version_name 写明这是分叉，装上后在扩展页一眼可辨。
import { execFileSync } from 'node:child_process';
import { readFile, writeFile, rm, mkdir, cp } from 'node:fs/promises';
import path from 'node:path';

const FORK_VERSION = '0.4.8.1';
const FORK_LABEL = '0.4.8.1 · 工位报到分叉';
const outputDir = path.join('releases', `linggan-intelligence-browser-v${FORK_VERSION}-station`);

execFileSync('npm', ['run', 'build'], { stdio: 'inherit' });

await rm(outputDir, { recursive: true, force: true });
await mkdir(outputDir, { recursive: true });
await cp('dist', outputDir, { recursive: true });

const manifestPath = path.join(outputDir, 'manifest.json');
const manifest = JSON.parse(await readFile(manifestPath, 'utf8'));
manifest.version = FORK_VERSION;
manifest.version_name = FORK_LABEL;
manifest.name = `${manifest.name}（工位分叉）`;
await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);

console.log(`station fork built: ${outputDir} (version ${FORK_VERSION})`);
