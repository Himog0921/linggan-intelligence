import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

test('popup cookie fetch scopes requests to the current platform', () => {
  const popupSource = fs.readFileSync(path.join(projectRoot, 'src/popup/App.jsx'), 'utf8');
  const backgroundSource = fs.readFileSync(path.join(projectRoot, 'src/background/index.js'), 'utf8');

  assert.match(popupSource, /sendToBackground\(MSG\.GET_PLATFORM_COOKIES,\s*\{\s*platform\s*\}\)/);
  assert.match(popupSource, /currentPlatform=\{platform\}/);
  assert.match(popupSource, /if \(platform === PLATFORM\.XHS && xhs\?\.count > 0\)/);
  assert.match(popupSource, /if \(platform === PLATFORM\.DOUYIN && dy\?\.count > 0\)/);
  assert.match(backgroundSource, /const requestedPlatform = String\(msg\.platform \|\| ''\)\.trim\(\)/);
  assert.match(backgroundSource, /const activeConfigs = requestedPlatform && platformConfig\[requestedPlatform\]/);
  assert.match(backgroundSource, /Object\.entries\(activeConfigs\)/);
  assert.match(backgroundSource, /const mergedResults = \{\s*\.\.\.\(stored\.platformCookies \|\| \{\}\),\s*\.\.\.results,/s);
});

test('popup account creation is scoped to the active platform', () => {
  const popupSource = fs.readFileSync(path.join(projectRoot, 'src/popup/App.jsx'), 'utf8');
  const modalSource = fs.readFileSync(path.join(projectRoot, 'src/popup/components/AddAccountModal.jsx'), 'utf8');
  const backgroundSource = fs.readFileSync(path.join(projectRoot, 'src/background/index.js'), 'utf8');
  const cookieManagerSource = fs.readFileSync(path.join(projectRoot, 'src/workbench/runtime/cookieManager.js'), 'utf8');

  assert.match(popupSource, /currentPlatform=\{platform\}/);
  assert.match(popupSource, /sendToBackground\(MSG\.GET_PLATFORM_COOKIES,\s*\{\s*platform\s*\}\)/);
  assert.doesNotMatch(popupSource, /sendToBackground\(MSG\.GET_PLATFORM_COOKIES,\s*\{\s*platform:\s*PLATFORM\.XHS\s*\}\)/);
  assert.match(modalSource, /currentPlatform/);
  assert.match(modalSource, /platform:\s*currentPlatformKey/);
  assert.match(modalSource, /currentPlatformKey === 'douyin'/);
  assert.match(backgroundSource, /selectAccountForWorkbenchTask\(task,\s*platform\)/);
  assert.match(backgroundSource, /if \(account\.runtimeSession\)/);
  assert.match(backgroundSource, /injectCookiesForAccount\(account\.cookieJson,\s*platform\)/);
  assert.match(cookieManagerSource, /export async function selectAccountForWorkbenchTask/);
  assert.match(cookieManagerSource, /douyin:\s*'\.douyin\.com'/);
});

test('popup workbench sync reads records from the active platform tab', () => {
  const popupSource = fs.readFileSync(path.join(projectRoot, 'src/popup/App.jsx'), 'utf8');

  assert.match(popupSource, /const dataTabQuery = platform === PLATFORM\.DOUYIN/);
  assert.match(popupSource, /\*:\/\/\*\.douyin\.com\/\*/);
  assert.match(popupSource, /\*:\/\/\*\.xiaohongshu\.com\/\*/);
  assert.match(popupSource, /const platformText = platform === PLATFORM\.DOUYIN \? '抖音' : '小红书'/);
  assert.doesNotMatch(popupSource, /const xhsTabs = await chrome\.tabs\.query/);
  assert.doesNotMatch(popupSource, /请先打开小红书页面，插件需要通过页面读取采集数据。/);
});
