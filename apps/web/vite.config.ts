import { execFileSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import react from '@vitejs/plugin-react';
import type { Plugin } from 'vite';
import { defineConfig } from 'vitest/config';

const webRoot = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(webRoot, '../..');
// LIDS Token 与共享外壳样式的唯一来源仍在旧页面目录（DEC-0010 / WEB-REACT-001 §4.5）。
const sharedStyleRoot = path.join(repoRoot, 'apps/api/src/local_web');

/** 本次构建对应的源码版本。本机运行链由 sync.sh 显式传入，与后端写进身份文件的是同一个值。 */
function buildRevision(): string {
  const fromRuntime = process.env.LINGGAN_WEB_BUILD_REVISION?.trim();
  if (fromRuntime) return fromRuntime;
  try {
    return execFileSync('git', ['rev-parse', '--short', 'HEAD'], { cwd: repoRoot, encoding: 'utf8' }).trim();
  } catch {
    return 'unknown';
  }
}

/** 产物里写一份 build-info.json，Axum 用它判断前端与后端是不是同一个版本。 */
function buildInfo(): Plugin {
  return {
    name: 'linggan-build-info',
    apply: 'build',
    generateBundle() {
      this.emitFile({
        type: 'asset',
        fileName: 'build-info.json',
        source: `${JSON.stringify({ revision: buildRevision(), builtAt: new Date().toISOString() })}\n`,
      });
    },
  };
}

// 开发服务器默认不代理任何接口：3000 端口是正在运行的本机服务，误连会改到真实设置。
// 需要时显式设置 LINGGAN_WEB_DEV_API_ORIGIN（例如隔离测试库上起的候选服务）。
const devApiOrigin = process.env.LINGGAN_WEB_DEV_API_ORIGIN?.trim();

export default defineConfig({
  plugins: [react(), buildInfo()],
  base: '/ui-assets/',
  // Frozen legacy Collection geometry keeps the existing DOM classes during the React port.
  css: { modules: { globalModulePaths: [/LegacyCollection\.module\.css$/] } },
  build: {
    outDir: 'dist',
    assetsDir: '',
    emptyOutDir: true,
    sourcemap: false,
  },
  server: {
    fs: { allow: [webRoot, sharedStyleRoot] },
    ...(devApiOrigin ? { proxy: { '/api': { target: devApiOrigin } } } : {}),
  },
  test: {
    environment: 'jsdom',
    setupFiles: ['./src/test/setup.ts'],
    include: ['src/**/*.test.{ts,tsx}'],
    restoreMocks: true,
  },
});
