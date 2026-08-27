import test from 'node:test';
import assert from 'node:assert/strict';

import {
  getColumns,
  getExportColumns,
  formatDataQualityLabel,
  formatSourceTierLabel,
  formatQualityReasonLabel,
} from '../src/dashboard/utils.js';

test('dashboard columns expose quality fields for notes comments and authors', () => {
  const noteColumns = getColumns('notes', [{ platform: 'xhs' }]).map((column) => column.key);
  const commentColumns = getColumns('comments', [{ platform: 'xhs' }]).map((column) => column.key);
  const authorColumns = getColumns('authors', [{ platform: 'xhs' }]).map((column) => column.key);

  for (const key of ['dataQuality', 'qualityReason', 'sourceTier']) {
    assert.ok(noteColumns.includes(key), `notes columns should include ${key}`);
    assert.ok(commentColumns.includes(key), `comments columns should include ${key}`);
    assert.ok(authorColumns.includes(key), `authors columns should include ${key}`);
  }
});

test('dashboard export columns expose quality fields for notes comments and authors', () => {
  const noteColumns = getExportColumns('notes', [{ platform: 'douyin' }]).map((column) => column.key);
  const commentColumns = getExportColumns('comments', [{ platform: 'xhs' }]).map((column) => column.key);
  const authorColumns = getExportColumns('authors', [{ platform: 'douyin' }]).map((column) => column.key);

  for (const key of ['dataQuality', 'qualityReason', 'sourceTier']) {
    assert.ok(noteColumns.includes(key), `notes export columns should include ${key}`);
    assert.ok(commentColumns.includes(key), `comments export columns should include ${key}`);
    assert.ok(authorColumns.includes(key), `authors export columns should include ${key}`);
  }
});

test('dashboard notes columns hide low-signal xhs metadata fields', () => {
  const noteColumns = getColumns('notes', [{ platform: 'xhs' }]).map((column) => column.key);
  const noteExportColumns = getExportColumns('notes', [{ platform: 'xhs' }]).map((column) => column.key);

  for (const key of ['atUserList', 'authorFollowed', 'shareRestricted']) {
    assert.ok(!noteColumns.includes(key), `notes table should not include ${key}`);
    assert.ok(!noteExportColumns.includes(key), `notes export should not include ${key}`);
  }
});

test('dashboard quality labels render readable Chinese values', () => {
  assert.equal(formatDataQualityLabel('full'), '完整');
  assert.equal(formatDataQualityLabel('degraded'), '降级');
  assert.equal(formatDataQualityLabel('seed'), '种子');
  assert.equal(formatDataQualityLabel(''), '-');

  assert.equal(formatSourceTierLabel('api'), 'API');
  assert.equal(formatSourceTierLabel('mixed'), '混合');
  assert.equal(formatSourceTierLabel('seed'), '种子');
  assert.equal(formatSourceTierLabel(''), '-');

  assert.equal(formatQualityReasonLabel('api_snapshot_partial'), 'API 快照不完整');
  assert.equal(formatQualityReasonLabel('synthetic_comment_id'), '合成评论 ID');
  assert.equal(formatQualityReasonLabel('monitor_surface_seed'), '监控 surface 种子');
  assert.equal(formatQualityReasonLabel('render_user_mismatch'), '渲染态用户不匹配');
  assert.equal(formatQualityReasonLabel('custom_reason'), 'custom reason');
  assert.equal(formatQualityReasonLabel(''), '-');
});
