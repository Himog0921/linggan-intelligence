import test from 'node:test';
import assert from 'node:assert/strict';

import { buildSignedXhsDetailExecutionUrl } from '../src/linggan/xhsExecutionTarget.js';

test('signed profile discovery URL becomes the runnable XHS detail URL', () => {
  const noteId = '6a8e86de00000000240043b0';
  const source = `https://www.xiaohongshu.com/user/profile/author/${noteId}?xsec_token=ABC_123%3D&xsec_source=pc_user`;
  assert.equal(
    buildSignedXhsDetailExecutionUrl(noteId, source),
    `https://www.xiaohongshu.com/discovery/item/${noteId}?source=webshare&xhsshare=pc_web&xsec_token=ABC_123%3D&xsec_source=pc_share`,
  );
});

test('XHS detail execution refuses missing tokens, wrong identities, and foreign hosts', () => {
  const noteId = '6a8e86de00000000240043b0';
  assert.equal(buildSignedXhsDetailExecutionUrl(noteId, `https://www.xiaohongshu.com/explore/${noteId}`), null);
  assert.equal(buildSignedXhsDetailExecutionUrl(noteId, 'https://www.xiaohongshu.com/explore/other?xsec_token=ABC'), null);
  assert.equal(buildSignedXhsDetailExecutionUrl(noteId, `https://example.com/explore/${noteId}?xsec_token=ABC`), null);
});
