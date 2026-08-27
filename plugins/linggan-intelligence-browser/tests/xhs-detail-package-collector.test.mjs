import assert from 'node:assert/strict';
import test from 'node:test';

import { aggregateCommentAndReplyDelivery } from '../src/platforms/xhs/detailPackageCollector.js';

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
