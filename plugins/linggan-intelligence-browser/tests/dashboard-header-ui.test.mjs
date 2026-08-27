import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

test('dashboard header keeps only the banner brand and removes extra title chrome', () => {
  const componentSource = fs.readFileSync(path.join(projectRoot, 'src/dashboard/App.jsx'), 'utf8');
  const styleSource = fs.readFileSync(path.join(projectRoot, 'src/dashboard/dashboard.css'), 'utf8');

  assert.match(componentSource, /dashboard-brand-banner-shell/);
  assert.match(componentSource, /BRAND_BANNER_SRC/);
  assert.doesNotMatch(componentSource, /BRAND_LOGO_SRC/);
  assert.doesNotMatch(componentSource, /dashboard-brand-mark/);
  assert.doesNotMatch(componentSource, /dashboard-brand-copy/);
  assert.doesNotMatch(componentSource, /灵感爆爆爆 数据面板/);

  assert.match(styleSource, /\.dashboard-nav\s*\{/);
  assert.match(styleSource, /padding:\s*10px 16px/);
  assert.match(styleSource, /gap:\s*12px/);
  assert.match(styleSource, /\.dashboard-brand-banner-shell/);
  assert.match(styleSource, /padding:\s*5px 5px/);
  assert.match(styleSource, /width:\s*154px/);
  assert.doesNotMatch(styleSource, /\.dashboard-brand-mark/);
  assert.doesNotMatch(styleSource, /\.dashboard-brand-logo/);
  assert.doesNotMatch(styleSource, /\.dashboard-brand-copy/);
  assert.doesNotMatch(styleSource, /\.dashboard-brand h1/);
});
