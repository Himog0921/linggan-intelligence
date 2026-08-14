import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

test('detail inject button groups use banner brand variant and drop outer shadow shell', () => {
  const buttonGroupSource = fs.readFileSync(path.join(projectRoot, 'src/content/components/ButtonGroup.jsx'), 'utf8');
  const xhsSource = fs.readFileSync(path.join(projectRoot, 'src/platforms/xhs/uiInjector.js'), 'utf8');
  const douyinSource = fs.readFileSync(path.join(projectRoot, 'src/platforms/douyin/uiInjector.js'), 'utf8');

  assert.match(buttonGroupSource, /const INJECT_BANNER_SRC = getBrandAssetUrl\(BRAND_ASSETS\.banner\)/);
  assert.match(buttonGroupSource, /brandVariant = 'logo'/);
  assert.match(buttonGroupSource, /const isBannerBrand = brandVariant === 'banner'/);
  assert.match(buttonGroupSource, /const bannerShellStyle = isBannerBrand/);
  assert.match(buttonGroupSource, /padding:\s*compact \? '4px 6px' : '5px 6px'/);
  assert.match(buttonGroupSource, /border:\s*`3px solid \$\{DEFAULT_TOKENS\.line\}`/);
  assert.match(buttonGroupSource, /background:\s*'#fff8d6'/);

  assert.match(xhsSource, /brandVariant:\s*'banner'/);
  assert.match(xhsSource, /justifyContent:\s*'center'/);
  assert.match(xhsSource, /width:\s*'100%'/);
  assert.match(xhsSource, /boxShadow:\s*'none'/);
  assert.doesNotMatch(xhsSource, /boxShadow:\s*'5px 5px 0 #121212'/);

  assert.match(douyinSource, /brandVariant:\s*'banner'/);
  assert.match(douyinSource, /justifyContent:\s*'center'/);
  assert.match(douyinSource, /width:\s*'100%'/);
  assert.match(douyinSource, /boxShadow:\s*'none'/);
  assert.doesNotMatch(douyinSource, /boxShadow:\s*'5px 5px 0 #121212'/);
});

test('xhs search batch bar stays short without shrinking the square logo', () => {
  const xhsSource = fs.readFileSync(path.join(projectRoot, 'src/platforms/xhs/uiInjector.js'), 'utf8');
  const batchBlock = xhsSource.match(/function injectBatchButtons\(mode\) \{[\s\S]*?header\.parentElement\.insertBefore\(container, header\);\n\}/)?.[0] || '';

  assert.match(batchBlock, /function injectBatchButtons\(mode\)/);
  assert.match(batchBlock, /containerStyle:\s*\{[\s\S]*padding:\s*'8px 14px'/);
  assert.match(batchBlock, /containerStyle:\s*\{[\s\S]*marginBottom:\s*'10px'/);
  assert.match(batchBlock, /buttonStyle:\s*\{[\s\S]*padding:\s*'7px 16px'/);
  assert.match(batchBlock, /buttonStyle:\s*\{[\s\S]*boxShadow:\s*'1px 1px 0 #121212'/);
  assert.doesNotMatch(batchBlock, /compact:\s*true/);
});

test('floating inject button groups can be dragged and remember position without changing xhs top batch bar', () => {
  const buttonGroupSource = fs.readFileSync(path.join(projectRoot, 'src/content/components/ButtonGroup.jsx'), 'utf8');
  const xhsSource = fs.readFileSync(path.join(projectRoot, 'src/platforms/xhs/uiInjector.js'), 'utf8');
  const douyinSource = fs.readFileSync(path.join(projectRoot, 'src/platforms/douyin/uiInjector.js'), 'utf8');

  assert.match(buttonGroupSource, /const FLOATING_POSITION_PREFIX = 'lgbbb\.content\.float'/);
  assert.match(buttonGroupSource, /function getFloatingPositionStorageKey\(floatingKey = ''\)/);
  assert.match(buttonGroupSource, /await chrome\.storage\?\.local\?\.get\(storageKey\)/);
  assert.match(buttonGroupSource, /window\.localStorage\.setItem\(storageKey, JSON\.stringify\(value\)\)/);
  assert.match(buttonGroupSource, /function clampFloatingPosition\(left = 0, top = 0, width = 0, height = 0\)/);
  assert.match(buttonGroupSource, /const startFloatingDrag = \(event\) => \{/);
  assert.match(buttonGroupSource, /event\.currentTarget\.setPointerCapture\?\.\(event\.pointerId\)/);
  assert.match(buttonGroupSource, /cursor: isDragging \? 'grabbing' : 'grab'/);

  assert.match(xhsSource, /floatingKey:\s*'xhs\.note-detail'/);
  assert.match(xhsSource, /floatingKey:\s*'xhs\.profile'/);
  assert.doesNotMatch(xhsSource, /floatingKey:\s*'xhs\.search'/);

  assert.match(douyinSource, /floatingKey:\s*page\.type === DY_PAGE_TYPE\.NOTE_DETAIL \? 'douyin\.note-detail' : 'douyin\.video-detail'/);
  assert.match(douyinSource, /floatingKey:\s*'douyin\.search'/);
  assert.match(douyinSource, /floatingKey:\s*'douyin\.profile'/);
});

test('douyin floating action buttons are cleared when platform verification is visible', () => {
  const douyinSource = fs.readFileSync(path.join(projectRoot, 'src/platforms/douyin/uiInjector.js'), 'utf8');
  const douyinAdapterSource = fs.readFileSync(path.join(projectRoot, 'src/platforms/douyin/index.js'), 'utf8');

  assert.match(douyinSource, /from '\.\/securityChallenge\.js'/);
  assert.match(
    douyinSource,
    /cleanupDouyinInjectedUI\(\{ includeTaskbar: false \}\);[\s\S]*detectDouyinSecurityChallenge\(\{ root: document, href: window\.location\.href \}\)/,
    'douyin UI injection should clear old action buttons before returning on verification pages',
  );
  assert.match(
    douyinAdapterSource,
    /currentSecurityChallenge !== lastSecurityChallenge/,
    'douyin route observer should notice verification overlays even when the URL does not change',
  );
  assert.match(
    douyinAdapterSource,
    /securityChallenge \|\| \(page\.type !== DY_PAGE_TYPE\.UNKNOWN && page\.type !== DY_PAGE_TYPE\.HOME\)/,
    'douyin injection should run once to remove stale buttons when verification is detected',
  );
});
