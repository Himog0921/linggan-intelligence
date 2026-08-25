import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const projectRoot = path.resolve(__dirname, '..');

test('dashboard runtime no longer keeps the legacy vanilla source file', () => {
  const legacyDashboardSource = path.join(projectRoot, 'src/dashboard/dashboard.js');

  assert.equal(
    fs.existsSync(legacyDashboardSource),
    false,
    'legacy dashboard source should be removed after the React migration',
  );
});

test('architecture docs point dashboard source-of-truth at the React entry', () => {
  const architectureDoc = fs.readFileSync(path.join(projectRoot, 'docs/ARCHITECTURE.md'), 'utf8');

  assert.match(
    architectureDoc,
    /src\/dashboard\/index\.jsx/,
    'architecture doc should describe the React dashboard entry',
  );
  assert.doesNotMatch(
    architectureDoc,
    /\*\*Dashboard\*\* \| src\/dashboard\/dashboard\.html \+ dashboard\.js \| 数据看板 iframe：浏览\/搜索\/导出已采集数据 \|/,
    'architecture doc should not describe the removed vanilla dashboard source as the current implementation',
  );
});
