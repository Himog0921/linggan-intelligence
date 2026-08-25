import assert from 'node:assert/strict';
import test from 'node:test';

import {
  buildCurrentVisibleXhsDiscoveryPackage,
  validateCurrentXhsSearchSurface,
} from '../src/linggan/xhsVisibleSearchReader.js';

function element({ textContent = '', attributes = {}, className = '', selectors = {}, rect = { top: 0, left: 0 } } = {}) {
  return {
    textContent,
    className,
    parentElement: null,
    hidden: false,
    getAttribute(name) { return attributes[name] || ''; },
    querySelector(selector) { return selectors[selector] || null; },
    getBoundingClientRect() { return rect; },
  };
}

function card({ id, title, creator, published, top, left, cover = '', variant = 'legacy' }) {
  const coverLink = element({ attributes: { href: `/explore/${id}?xsec_token=test` } });
  const image = element({ attributes: { src: cover } });
  const selectors = {
    'a.cover': coverLink,
    'a.cover img, a.cover picture img, a.cover source, img': image,
  };
  if (variant === 'legacy') {
    selectors['.footer span'] = element({ textContent: title });
    selectors['.author-wrapper .name'] = element({ textContent: creator });
    selectors.time = element({ textContent: published });
  } else {
    selectors['[data-testid="note-title"]'] = element({ textContent: title });
    selectors['[data-testid="creator-name"]'] = element({ textContent: creator });
    selectors['[data-testid="published-at"]'] = element({ textContent: published });
  }
  return element({ selectors, rect: { top, left } });
}

function surface({ selectedSort = true, sections = [] } = {}) {
  const comprehensive = element({
    textContent: '综合',
    attributes: selectedSort ? { 'aria-selected': 'true' } : {},
    className: selectedSort ? 'selected' : '',
  });
  const doc = {
    querySelectorAll(selector) {
      if (selector === 'button,[role="button"],div,span') return [comprehensive];
      if (selector === '.feeds-container section, #exploreFeeds section') return sections;
      return [];
    },
  };
  return { doc, win: { scrollY: 0, getComputedStyle: () => ({ display: 'block', visibility: 'visible' }) } };
}

test('reader only accepts the fixed current XHS ADHD comprehensive search surface', () => {
  const { doc, win } = surface();
  assert.equal(validateCurrentXhsSearchSurface({
    doc, win, url: 'https://www.xiaohongshu.com/search_result?keyword=ADHD',
  }).ok, true);
  assert.equal(validateCurrentXhsSearchSurface({
    doc, win, url: 'https://www.xiaohongshu.com/search_result?keyword=%E5%A4%9A%E5%8A%A8',
  }).code, 'unexpected_query');
  assert.equal(validateCurrentXhsSearchSurface({
    doc, win, url: 'https://www.xiaohongshu.com/explore/note-1',
  }).code, 'not_xhs_search_results');
});

test('reader refuses to infer comprehensive sorting from a URL or default', () => {
  const { doc, win } = surface({ selectedSort: false });
  const result = buildCurrentVisibleXhsDiscoveryPackage({
    doc, win, url: 'https://www.xiaohongshu.com/search_result?keyword=ADHD',
  });
  assert.equal(result.ok, false);
  assert.equal(result.code, 'comprehensive_sort_not_confirmed');
});

test('reader preserves visual card order, supports legacy and alternate card labels, and only reports an observed external cover candidate', () => {
  const { doc, win } = surface({ sections: [
    card({ id: 'note-right', title: '右侧', creator: '作者B', published: '2026-08-25T00:00:00Z', top: 100, left: 500, cover: 'https://cdn.example/right.jpg' }),
    card({ id: 'note-left', title: '左侧', creator: '作者A', published: '2026-08-24T00:00:00Z', top: 100, left: 20, cover: 'https://cdn.example/left.jpg', variant: 'alternate' }),
  ] });
  const result = buildCurrentVisibleXhsDiscoveryPackage({
    doc, win, url: 'https://www.xiaohongshu.com/search_result?keyword=ADHD',
    now: () => new Date('2026-08-25T12:00:00.000Z'),
  });
  assert.equal(result.ok, true);
  assert.deepEqual(result.package.cards.map((item) => item.content.platformContentId), ['note-left', 'note-right']);
  assert.equal(result.package.cards[0].occurrence.resultPosition, 1);
  assert.equal(result.package.cards[0].occurrence.observedAt, '2026-08-25T12:00:00.000Z');
  assert.equal(result.package.cards[0].content.coverCandidate.observedExternalUri, 'https://cdn.example/left.jpg');
  assert.equal(result.package.coverage.visibleCards, 2);
  assert.deepEqual(result.package.coverage, {
    unit: 'visible_search_card', visibleCards: 2, discoveredCards: 2,
    emittedCards: 2, failedCards: 0, notAttemptedCards: 0, stoppedReason: 'unknown',
  });
  assert.equal(result.package.coverage.stoppedReason, 'unknown');
  assert.deepEqual(result.localCoverage, {
    renderedCardCandidates: 2, discoveredCards: 2, emittedCards: 2, failedCards: 0, notAttemptedCards: 0, stoppedReason: 'unknown',
  });
});

test('reader records only first 20 known surface candidates and does not convert the remainder into platform claims', () => {
  const sections = Array.from({ length: 23 }, (_, index) => card({
    id: `note-${index + 1}`, title: `卡片 ${index + 1}`, creator: '作者', published: '', top: index * 60, left: 0,
  }));
  const { doc, win } = surface({ sections });
  const result = buildCurrentVisibleXhsDiscoveryPackage({
    doc, win, url: 'https://www.xiaohongshu.com/search_result?keyword=ADHD',
    now: () => new Date('2026-08-25T12:00:00.000Z'),
  });
  assert.equal(result.ok, true);
  assert.equal(result.package.cards.length, 20);
  assert.equal(result.package.coverage.discoveredCards, 20);
  assert.equal(result.package.coverage.emittedCards, 20);
  assert.equal(result.package.coverage.failedCards, 0);
  assert.equal(result.package.coverage.notAttemptedCards, 3);
  assert.equal(result.package.coverage.stoppedReason, 'quota_reached');
  assert.equal(result.localCoverage.notAttemptedCards, 3);
  assert.equal(result.localCoverage.renderedCardCandidates, 23);
});

test('reader keeps valid cards when one currently inspected card lacks a distinct stable identity', () => {
  const { doc, win } = surface({ sections: [
    card({ id: 'note-a', title: '第一张', creator: '作者', published: '', top: 0, left: 0 }),
    card({ id: 'note-a', title: '重复链接', creator: '作者', published: '', top: 60, left: 0 }),
    card({ id: 'note-c', title: '第三张', creator: '作者', published: '', top: 120, left: 0 }),
  ] });
  const result = buildCurrentVisibleXhsDiscoveryPackage({
    doc, win, url: 'https://www.xiaohongshu.com/search_result?keyword=ADHD',
    now: () => new Date('2026-08-25T12:00:00.000Z'),
  });

  assert.equal(result.ok, true);
  assert.deepEqual(result.package.cards.map((item) => item.content.platformContentId), ['note-a', 'note-c']);
  assert.deepEqual(result.package.coverage, {
    unit: 'visible_search_card', visibleCards: 2, discoveredCards: 3,
    emittedCards: 2, failedCards: 1, notAttemptedCards: 0, stoppedReason: 'unknown',
  });
});
