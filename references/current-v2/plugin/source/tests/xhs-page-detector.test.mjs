import test from 'node:test';
import assert from 'node:assert/strict';

import { detectPageType } from '../src/platforms/xhs/pageDetector.js';

const originalWindow = globalThis.window;

test('detectPageType treats signed xhs relay note urls as note detail pages', () => {
  globalThis.window = {
    location: {
      href: 'https://www.xiaohongshu.com/user/profile/65f44e0b000000000500f029/69d67b88000000002102cded?xsec_token=abc123&xsec_source=pc_user',
      pathname: '/user/profile/65f44e0b000000000500f029/69d67b88000000002102cded',
    },
  };

  assert.deepEqual(detectPageType(), {
    type: 'noteDetail',
    url: 'https://www.xiaohongshu.com/user/profile/65f44e0b000000000500f029/69d67b88000000002102cded?xsec_token=abc123&xsec_source=pc_user',
  });
});

test.after(() => {
  globalThis.window = originalWindow;
});
