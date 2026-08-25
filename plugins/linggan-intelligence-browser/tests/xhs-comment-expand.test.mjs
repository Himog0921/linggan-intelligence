import test from 'node:test';
import assert from 'node:assert/strict';

import {
  expandAllReplies,
  isExpandMoreReplyTrigger,
} from '../src/platforms/xhs/commentCollector.js';

function createExpandButton(text, onClick) {
  return {
    textContent: text,
    scrollIntoView() {},
    click() {
      onClick?.();
    },
  };
}

test('isExpandMoreReplyTrigger only matches expand-style reply controls', () => {
  assert.equal(isExpandMoreReplyTrigger('展开更多回复'), true);
  assert.equal(isExpandMoreReplyTrigger('展开 3 条回复'), true);
  assert.equal(isExpandMoreReplyTrigger('收起回复'), false);
  assert.equal(isExpandMoreReplyTrigger('回复'), false);
});

test('expandAllReplies recursively clicks newly revealed show-more buttons', async () => {
  let stage = 0;
  let subCount = 0;

  const first = createExpandButton('展开 2 条回复', () => {
    stage = 1;
    subCount = 2;
  });
  const second = createExpandButton('展开更多回复', () => {
    stage = 2;
    subCount = 4;
  });

  const parentCommentEl = {
    querySelectorAll(selector) {
      if (selector === 'div.show-more') {
        if (stage === 0) return [first];
        if (stage === 1) return [second];
        return [];
      }
      if (selector === '.comment-item.comment-item-sub') {
        return Array.from({ length: subCount }, (_, index) => ({ id: index }));
      }
      return [];
    },
  };

  await expandAllReplies(parentCommentEl, 5);

  assert.equal(stage, 2);
  assert.equal(subCount, 4);
});
