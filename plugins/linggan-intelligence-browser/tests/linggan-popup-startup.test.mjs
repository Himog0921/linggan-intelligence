import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

const app = readFileSync(new URL('../src/popup/App.jsx', import.meta.url), 'utf8');
const entry = readFileSync(new URL('../src/popup/index.jsx', import.meta.url), 'utf8');

test('Popup imports the runtime notice formatter before its first render', () => {
  assert.match(
    app,
    /import\s*\{\s*formatLingganRuntimeNotice\s*\}\s*from\s*'\.\.\/linggan\/adapter\.js';/,
  );
  assert.match(app, /formatLingganRuntimeNotice\(idleClaimSnapshot\)/);
});

test('Popup renders an honest recoverable fallback if App fails during startup', () => {
  assert.match(entry, /class PopupStartupBoundary extends React\.Component/);
  assert.match(entry, /static getDerivedStateFromError\(\)/);
  assert.match(entry, /<PopupStartupBoundary>\s*<App\s*\/>\s*<\/PopupStartupBoundary>/s);
  assert.match(entry, /本机状态目前未知/);
  assert.match(entry, /本次没有读取、采集或传输任何数据/);
  assert.match(entry, /重新加载 Linggan Intelligence Browser v0\.4\.1/);
  assert.doesNotMatch(entry, /LINGGAN_RUNTIME_ACTION|sendToBackground|sendToTab/);
});
