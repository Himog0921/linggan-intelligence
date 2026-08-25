import test from 'node:test';
import assert from 'node:assert/strict';

import { getPreferredRecordUrl } from '../src/dashboard/utils.js';

test('getPreferredRecordUrl prefers xhs comment rawUrl when it preserves xsec_token share link', () => {
  const comment = {
    platform: 'xhs',
    noteUrl: 'https://www.xiaohongshu.com/explore/680123456789abcdef012345',
    rawUrl: 'https://www.xiaohongshu.com/user/profile/5f1234567890abcd12345678/680123456789abcdef012345?xsec_token=abc123',
  };

  assert.equal(
    getPreferredRecordUrl(comment, 'noteUrl'),
    'https://www.xiaohongshu.com/user/profile/5f1234567890abcd12345678/680123456789abcdef012345?xsec_token=abc123',
  );
});

test('getPreferredRecordUrl keeps existing xhs share link when noteUrl already has xsec_token', () => {
  const comment = {
    platform: 'xhs',
    noteUrl: 'https://www.xiaohongshu.com/explore/680123456789abcdef012345?xsec_token=abc123',
    rawUrl: 'https://www.xiaohongshu.com/explore/680123456789abcdef012345',
  };

  assert.equal(
    getPreferredRecordUrl(comment, 'noteUrl'),
    'https://www.xiaohongshu.com/explore/680123456789abcdef012345?xsec_token=abc123',
  );
});

test('getPreferredRecordUrl leaves douyin comment links unchanged', () => {
  const comment = {
    platform: 'douyin',
    noteUrl: 'https://www.douyin.com/video/7488888888888888888',
    rawUrl: 'https://www.douyin.com/video/7999999999999999999',
  };

  assert.equal(
    getPreferredRecordUrl(comment, 'noteUrl'),
    'https://www.douyin.com/video/7488888888888888888',
  );
});
