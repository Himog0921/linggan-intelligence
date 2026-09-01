import assert from 'node:assert/strict';
import test from 'node:test';

import {
  aggregateCommentAndReplyDelivery,
  attemptDetailLaneDelivery,
  separateCommentAndReplyDelivery,
} from '../src/platforms/xhs/detailPackageCollector.js';

test('detail receipt treats an unacknowledged reply package as part of the comment lane', () => {
  assert.equal(aggregateCommentAndReplyDelivery({
    delivery: 'acknowledged',
    replies: { delivery: 'terminal' },
  }), 'terminal');
  assert.equal(aggregateCommentAndReplyDelivery({
    delivery: 'pending',
    replies: { delivery: 'acknowledged' },
  }), 'pending');
});

test('detail receipt does not invent a reply delivery when no reply package was produced', () => {
  assert.equal(aggregateCommentAndReplyDelivery({ delivery: 'acknowledged' }), 'acknowledged');
});

test('detail receipt keeps comments and replies as independent delivery facts', () => {
  assert.deepEqual(separateCommentAndReplyDelivery({
    delivery: 'acknowledged',
    replies: { delivery: 'rejected' },
  }), {
    comments: 'acknowledged',
    replies: 'rejected',
  });
  assert.deepEqual(separateCommentAndReplyDelivery({ delivery: 'acknowledged' }), {
    comments: 'acknowledged',
    replies: 'not_applicable',
  });
});

test('one failed detail lane becomes an explicit rejection and does not abort later lane delivery', async () => {
  const calls = [];
  const content = await attemptDetailLaneDelivery('content_detail', async () => {
    calls.push('content');
    throw new Error('background_queue_timeout');
  });
  const media = await attemptDetailLaneDelivery('media_slots', async () => {
    calls.push('media');
    return { delivery: 'pending' };
  });

  assert.deepEqual(calls, ['content', 'media']);
  assert.deepEqual(content, {
    delivery: 'rejected',
    code: 'content_detail_queue_failed',
    message: 'background_queue_timeout',
  });
  assert.deepEqual(media, { delivery: 'pending' });
});
