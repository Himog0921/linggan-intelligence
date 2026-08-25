import test from 'node:test';
import assert from 'node:assert/strict';

import {
  extractContentIdentityFromUrl,
  extractProfileIdentityFromUrl,
  parseTargetIdentity,
  sameTargetIdentity,
} from '../src/shared/targetIdentity.js';

test('target identity normalizes xhs profile urls consistently', () => {
  assert.equal(
    extractProfileIdentityFromUrl('https://www.xiaohongshu.com/user/profile/6926D8F4000000003702C666?xsec_token=abc'),
    '6926d8f4000000003702c666',
  );
  assert.equal(
    extractProfileIdentityFromUrl('https://www.xiaohongshu.com/profile/6926d8f4000000003702c666'),
    '6926d8f4000000003702c666',
  );
});

test('target identity normalizes xhs detail urls including profile relay urls', () => {
  assert.equal(
    extractContentIdentityFromUrl('https://www.xiaohongshu.com/explore/xhs_69BAAD5E00000000230055EF?xsec_token=abc'),
    '69baad5e00000000230055ef',
  );
  assert.equal(
    extractContentIdentityFromUrl('https://www.xiaohongshu.com/user/profile/6926d8f4000000003702c666/69baad5e00000000230055ef'),
    '69baad5e00000000230055ef',
  );
});

test('target identity normalizes douyin profile and detail urls', () => {
  assert.equal(
    extractProfileIdentityFromUrl('https://www.douyin.com/user/MS4wLjABAAAAabcDEF?from_tab_name=main'),
    'ms4wljabaaaaabcdef',
  );
  assert.equal(
    extractProfileIdentityFromUrl('https://www.douyin.com/@MogStudio'),
    'mogstudio',
  );
  assert.equal(
    extractContentIdentityFromUrl('https://www.douyin.com/note/dy_7321309610927770930?previous_page=app_code_link'),
    '7321309610927770930',
  );
});

test('target identity exposes one parsed shape for route fingerprinting', () => {
  assert.deepEqual(
    parseTargetIdentity('https://www.xiaohongshu.com/user/profile/author_1/note_1'),
    {
      platform: 'xhs',
      profileId: 'author_1',
      contentId: 'note_1',
    },
  );
});

test('sameTargetIdentity compares profile and detail identities with the same rules', () => {
  assert.equal(
    sameTargetIdentity(
      'profile',
      'https://www.xiaohongshu.com/user/profile/Author_1',
      'https://www.xiaohongshu.com/profile/author_1',
    ),
    true,
  );
  assert.equal(
    sameTargetIdentity(
      'detail',
      'https://www.xiaohongshu.com/user/profile/author_1/note_1',
      'https://www.xiaohongshu.com/explore/note_2',
    ),
    false,
  );
});
