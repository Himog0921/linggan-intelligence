import test from 'node:test';
import assert from 'node:assert/strict';

import { hasObservedInteractionCount, pickInteractionCount } from '../src/platforms/xhs/authorCollector.js';

test('author interaction count distinguishes an observed zero from an unavailable field', () => {
  assert.equal(pickInteractionCount({ '粉丝': 0 }, ['fans', 'followers', '粉丝']), 0);
  assert.equal(pickInteractionCount({ followers: 128 }, ['fans', 'followers', '粉丝']), 128);
  assert.equal(pickInteractionCount({}, ['fans', 'followers', '粉丝']), null);
});

test('author interaction count does not turn an unparseable display value into zero', () => {
  assert.equal(hasObservedInteractionCount('暂无'), false);
  assert.equal(hasObservedInteractionCount('—'), false);
  assert.equal(hasObservedInteractionCount('0'), true);
  assert.equal(hasObservedInteractionCount('1.2万'), true);
});
