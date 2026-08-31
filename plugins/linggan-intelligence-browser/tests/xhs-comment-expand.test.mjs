import test from 'node:test';
import assert from 'node:assert/strict';

import {
  createCommentActionGate,
  expandAllReplies,
  expandNextReply,
  isExpandMoreReplyTrigger,
} from '../src/platforms/xhs/commentCollector.js';

test('comment action gate enforces a minimum interval and pause checks before every action', async () => {
  let now = 1000;
  const waits = [];
  const pauses = [];
  const gate = createCommentActionGate({
    minimumCooldownMs: 1200,
    now: () => now,
    sleep: async (milliseconds) => {
      waits.push(milliseconds);
      now += milliseconds;
    },
    waitIfPaused: async () => pauses.push('checked'),
  });

  assert.equal(await gate.before(), true);
  now += 200;
  assert.equal(await gate.before(), true);
  assert.deepEqual(waits, [1000]);
  assert.equal(pauses.length, 4);
});

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

test('expandNextReply performs only one click and checks pause boundaries around it', async () => {
  const events = [];
  const buttons = [
    createExpandButton('展开 2 条回复', () => events.push('click:first')),
    createExpandButton('展开更多回复', () => events.push('click:second')),
  ];
  const parentCommentEl = {
    querySelectorAll(selector) {
      if (selector === 'div.show-more') return buttons;
      if (selector === '.comment-item.comment-item-sub') return [];
      return [];
    },
  };

  const result = await expandNextReply(parentCommentEl, {
    waitIfPaused: async () => events.push('pause-check'),
    shouldStop: () => false,
    waitBeforeAction: async () => events.push('cooldown'),
    waitAfterAction: async () => events.push('stable'),
  });

  assert.equal(result.acted, true);
  assert.deepEqual(events, [
    'pause-check',
    'cooldown',
    'pause-check',
    'click:first',
    'stable',
    'pause-check',
  ]);
});

test('an offscreen reply control is revealed in one loop and clicked only by a later loop', async () => {
  let visible = false;
  let clicks = 0;
  let scrolls = 0;
  const button = {
    textContent: '展开 2 条回复',
    getBoundingClientRect: () => visible
      ? ({ top: 10, bottom: 20 })
      : ({ top: 200, bottom: 220 }),
    scrollIntoView() { scrolls += 1; visible = true; },
    click() { clicks += 1; },
  };
  const parentCommentEl = {
    querySelectorAll(selector) {
      if (selector === 'div.show-more') return [button];
      if (selector === '.comment-item.comment-item-sub') return [];
      return [];
    },
  };
  globalThis.window = {
    innerHeight: 100,
    getComputedStyle: () => ({ overflow: 'visible', overflowY: 'visible' }),
  };
  globalThis.document = {
    documentElement: {
      getBoundingClientRect: () => ({ top: 0, bottom: 100 }),
    },
  };
  button.parentElement = null;

  const first = await expandNextReply(parentCommentEl, {
    waitBeforeAction: async () => {},
    waitAfterAction: async () => {},
  });
  assert.equal(first.reason, 'reply_control_revealed');
  assert.equal(scrolls, 1);
  assert.equal(clicks, 0);

  const second = await expandNextReply(parentCommentEl, {
    waitBeforeAction: async () => {},
    waitAfterAction: async () => {},
  });
  assert.equal(second.reason, 'reply_expanded');
  assert.equal(clicks, 1);
  delete globalThis.window;
  delete globalThis.document;
});

test('a reply control within one fractional pixel of the viewport edge is clicked without a reveal loop', async () => {
  let clicks = 0;
  let scrolls = 0;
  const scrollParent = {
    parentElement: null,
    getBoundingClientRect: () => ({ top: 121, bottom: 768 }),
  };
  const button = {
    parentElement: scrollParent,
    textContent: '展开更多回复',
    getBoundingClientRect: () => ({ top: 736.03125, bottom: 768.03125 }),
    scrollIntoView() { scrolls += 1; },
    click() { clicks += 1; },
  };
  const parentCommentEl = {
    querySelectorAll(selector) {
      if (selector === 'div.show-more') return [button];
      if (selector === '.comment-item.comment-item-sub') return [];
      return [];
    },
  };
  globalThis.window = {
    innerHeight: 900,
    getComputedStyle: (element) => element === scrollParent
      ? ({ overflow: 'visible', overflowY: 'auto' })
      : ({ overflow: 'visible', overflowY: 'visible' }),
  };
  globalThis.document = {
    documentElement: {
      getBoundingClientRect: () => ({ top: 0, bottom: 900 }),
    },
  };

  try {
    const result = await expandNextReply(parentCommentEl, {
      waitBeforeAction: async () => {},
      waitAfterAction: async () => {},
    });

    assert.equal(result.reason, 'reply_expanded');
    assert.equal(clicks, 1);
    assert.equal(scrolls, 0);
  } finally {
    delete globalThis.window;
    delete globalThis.document;
  }
});
