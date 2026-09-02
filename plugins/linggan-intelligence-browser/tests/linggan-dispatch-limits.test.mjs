import assert from 'node:assert/strict';
import test from 'node:test';

import {
  dispatchedCommentMaxTotal,
  dispatchedMaximumQuota,
} from '../src/linggan/localExecutionSupport.js';

test('scheduled unlimited comments preserve null quota and zero collector limit', () => {
  const spec = { commentLimit: 'not_requested', maximumQuota: null };
  assert.equal(dispatchedMaximumQuota(spec, 1), null);
  assert.equal(dispatchedCommentMaxTotal(spec, 'comments'), 0);
  assert.equal(dispatchedCommentMaxTotal(spec, 'replies'), 0);
});

test('scheduled finite comment limits remain explicit', () => {
  assert.equal(dispatchedCommentMaxTotal({ commentLimit: 30, maximumQuota: 30 }, 'comments'), 30);
  assert.equal(dispatchedCommentMaxTotal({ commentLimit: 'not_requested', maximumQuota: 45 }, 'replies'), 45);
  assert.equal(dispatchedMaximumQuota({ maximumQuota: 'not_requested' }, 1), 1);
});
