import test from 'node:test';
import assert from 'node:assert/strict';

import {
  PLATFORM,
  PAGE_MODE,
  getPageCapabilities,
  getPageContextText,
} from '../src/popup/utils.js';

test('xhs profile popup exposes manual author collection while keeping batch actions', () => {
  const capabilities = getPageCapabilities(PLATFORM.XHS, PAGE_MODE.PROFILE);

  assert.equal(capabilities.canCollectPrimary, false);
  assert.equal(capabilities.canCollectSecondary, true);
  assert.equal(capabilities.secondaryAction, 'author');
  assert.equal(capabilities.canBatchNotes, true);
  assert.equal(capabilities.canBatchComments, true);
});

test('xhs profile page context copy mentions author collection availability', () => {
  const context = getPageContextText(PLATFORM.XHS, PAGE_MODE.PROFILE);

  assert.match(context.hint, /手动采集当前博主/);
  assert.deepEqual(context.tags, ['博主可用', '批量可用']);
});
