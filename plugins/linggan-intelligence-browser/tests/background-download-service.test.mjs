import test from 'node:test';
import assert from 'node:assert/strict';

import {
  buildDownloadHeaders,
  dedupeCandidates,
  sanitizeDownloadFilename,
} from '../src/background/downloadService.js';

test('background download service sanitizes filenames without dropping folder paths', () => {
  assert.equal(
    sanitizeDownloadFilename('/灵感爆爆爆//作者:作品?.jpg'),
    '灵感爆爆爆/作者_作品_.jpg',
  );
});

test('background download service dedupes candidate urls and filters empty values', () => {
  assert.deepEqual(
    dedupeCandidates([' https://a.example/img.jpg ', '', 'https://a.example/img.jpg', 'https://b.example/img.jpg']),
    ['https://a.example/img.jpg', 'https://b.example/img.jpg'],
  );
});

test('background download service filters unsafe request headers', () => {
  assert.deepEqual(
    buildDownloadHeaders([
      { name: 'Referer', value: 'https://www.xiaohongshu.com/' },
      { name: 'Cookie', value: 'secret=1' },
      { name: 'Bad Header', value: 'x' },
    ]),
    [{ name: 'Referer', value: 'https://www.xiaohongshu.com/' }],
  );
});
