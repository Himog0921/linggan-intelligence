import test from 'node:test';
import assert from 'node:assert/strict';

import {
  XHS_SEARCH_FILTERS,
  applyXhsSearchFilters,
  normalizeXhsSearchFilters,
  readCurrentXhsSearchFilterSnapshot,
  readXhsSearchResultFeedSnapshot,
  summarizeXhsSearchFilters,
  waitForXhsSearchResultsSettled,
} from '../src/platforms/xhs/searchFilters.js';

test('readCurrentXhsSearchFilterSnapshot maps Xiaohongshu active search filters', () => {
  const win = {
    __INITIAL_STATE__: {
      search: {
        filterParams: [
          { type: 'sort_type', tags: ['最多评论'] },
          { type: 'filter_note_type', tags: ['图文'] },
          { type: 'filter_note_time', tags: ['一周内'] },
          { type: 'filter_note_range', tags: ['已看过'] },
          { type: 'filter_pos_distance', tags: ['附近'] },
        ],
      },
    },
  };

  assert.deepEqual(readCurrentXhsSearchFilterSnapshot(win), {
    sortBasis: 'most_commented',
    noteType: 'image',
    publishTime: 'one_week',
    labels: {
      sortBasis: '最多评论',
      noteType: '图文',
      publishTime: '一周内',
    },
    raw: {
      sort_type: ['最多评论'],
      filter_note_type: ['图文'],
      filter_note_time: ['一周内'],
    },
  });
});

test('readCurrentXhsSearchFilterSnapshot unwraps live Xiaohongshu ref filter state', () => {
  const win = {
    __INITIAL_STATE__: {
      search: {
        filterParams: {
          __v_isRef: true,
          _rawValue: [
            { type: 'sort_type', tags: ['collect_descending'] },
            { type: 'filter_note_type', tags: ['不限'] },
            { type: 'filter_note_time', tags: ['半年内'] },
            { type: 'filter_note_range', tags: ['不限'] },
            { type: 'filter_pos_distance', tags: ['不限'] },
          ],
        },
      },
    },
  };

  assert.deepEqual(readCurrentXhsSearchFilterSnapshot(win), {
    sortBasis: 'most_collected',
    noteType: 'all',
    publishTime: 'half_year',
    labels: {
      sortBasis: '最多收藏',
      noteType: '不限',
      publishTime: '半年内',
    },
    raw: {
      sort_type: ['collect_descending'],
      filter_note_type: ['不限'],
      filter_note_time: ['半年内'],
    },
  });
});

test('normalizeXhsSearchFilters keeps only the three supported search filters', () => {
  assert.deepEqual(normalizeXhsSearchFilters({
    sortBasis: 'most_liked',
    noteType: 'video',
    publishTime: 'half_year',
    searchRange: 'viewed',
    positionDistance: 'nearby',
  }), {
    sortBasis: 'most_liked',
    noteType: 'video',
    publishTime: 'half_year',
  });

  assert.deepEqual(normalizeXhsSearchFilters({}), XHS_SEARCH_FILTERS.defaults);
});

test('summarizeXhsSearchFilters describes current-page and explicit filters', () => {
  assert.equal(
    summarizeXhsSearchFilters(XHS_SEARCH_FILTERS.defaults),
    '沿用当前筛选',
  );
  assert.equal(
    summarizeXhsSearchFilters({ sortBasis: 'latest', noteType: 'image', publishTime: 'one_day' }),
    '最新 · 图文 · 一天内',
  );
});

test('applyXhsSearchFilters clicks only sort, note type, and publish time options', async () => {
  const clicked = [];
  const elements = [
    makeFilterPanelButton('筛选'),
    makeFilterOption('排序依据', '最多评论', clicked),
    makeFilterOption('笔记类型', '图文', clicked),
    makeFilterOption('发布时间', '一周内', clicked),
    makeFilterOption('搜索范围', '未看过', clicked),
    makeFilterOption('位置距离', '附近', clicked),
  ];
  const doc = {
    querySelectorAll: () => elements,
  };

  const result = await applyXhsSearchFilters({
    sortBasis: 'most_commented',
    noteType: 'image',
    publishTime: 'one_week',
    searchRange: 'unseen',
    positionDistance: 'nearby',
  }, {
    document: doc,
    win: { location: { href: 'https://www.xiaohongshu.com/search_result?keyword=A' } },
    waitMs: 0,
  });

  assert.equal(result.applied, true);
  assert.deepEqual(clicked, [
    '排序依据:最多评论',
    '笔记类型:图文',
    '发布时间:一周内',
  ]);
});

test('applyXhsSearchFilters retries past inert duplicate filter wrappers', async () => {
  const clicked = [];
  const sortGroup = makeTreeElement({
    textContent: '排序依据 综合 最多评论',
    className: 'filters',
    children: [
      makeTreeElement({ tagName: 'SPAN', textContent: '排序依据' }),
      makeTreeElement({
        textContent: '最多评论',
        className: 'tags',
        click: () => clicked.push('inert-wrapper'),
        children: [
          makeTreeElement({
            textContent: '最多评论',
            className: 'tags',
            click(element) {
              element.className = 'tags active';
              clicked.push('real-option');
            },
          }),
        ],
      }),
    ],
  });
  const doc = makeTreeDocument([
    makeTreeElement({ textContent: '筛选' }),
    sortGroup,
    makeTreeElement({
      textContent: '笔记类型 不限 图文',
      className: 'filters',
      children: [
        makeTreeElement({ tagName: 'SPAN', textContent: '笔记类型' }),
        makeTreeElement({ textContent: '不限', className: 'tags active' }),
        makeTreeElement({ textContent: '图文', className: 'tags' }),
      ],
    }),
    makeTreeElement({
      textContent: '发布时间 不限 一周内',
      className: 'filters',
      children: [
        makeTreeElement({ tagName: 'SPAN', textContent: '发布时间' }),
        makeTreeElement({ textContent: '不限', className: 'tags active' }),
        makeTreeElement({ textContent: '一周内', className: 'tags' }),
      ],
    }),
  ]);

  const result = await applyXhsSearchFilters({
    sortBasis: 'most_commented',
  }, {
    document: doc,
    win: { location: { href: 'https://www.xiaohongshu.com/search_result?keyword=A' } },
    waitMs: 0,
  });

  assert.equal(result.applied, true);
  assert.deepEqual(clicked, ['real-option']);
});

test('readXhsSearchResultFeedSnapshot extracts real note cards from the search feed', () => {
  const doc = makeFeedDocument([
    [
      { noteId: 'note_a', title: '第一篇', likes: '18' },
      { noteId: 'note_b', title: '第二篇', likes: '2.1万' },
    ],
  ]);

  const snapshot = readXhsSearchResultFeedSnapshot(doc);

  assert.equal(snapshot.hasFeedContainer, true);
  assert.equal(snapshot.count, 2);
  assert.deepEqual(snapshot.notes.map((note) => note.noteId), ['note_a', 'note_b']);
  assert.match(snapshot.signature, /note_a:第一篇:18/);
});

test('readXhsSearchResultFeedSnapshot ignores hidden loading placeholders', () => {
  const doc = makeFeedDocument([
    [{ noteId: 'note_a', title: '第一篇', likes: '18' }],
  ]);
  const originalGetComputedStyle = globalThis.getComputedStyle;
  doc.querySelectorAll = () => [{
    className: 'loading',
    textContent: '加载中',
    parentElement: null,
    getAttribute: () => '',
    getBoundingClientRect: () => ({ width: 1500, height: 64 }),
    getClientRects: () => [{}],
  }];
  globalThis.getComputedStyle = () => ({
    display: 'flex',
    visibility: 'hidden',
    opacity: '1',
  });

  try {
    const snapshot = readXhsSearchResultFeedSnapshot(doc);

    assert.equal(snapshot.isLoading, false);
    assert.equal(snapshot.count, 1);
  } finally {
    globalThis.getComputedStyle = originalGetComputedStyle;
  }
});

test('waitForXhsSearchResultsSettled waits until filtered notes change and stabilize', async () => {
  const previousSnapshot = readXhsSearchResultFeedSnapshot(makeFeedDocument([
    [{ noteId: 'old_note', title: '旧列表', likes: '1' }],
  ]));
  const doc = makeFeedDocument([
    [{ noteId: 'old_note', title: '旧列表', likes: '1' }],
    [{ noteId: 'new_note', title: '新列表', likes: '99' }],
    [{ noteId: 'new_note', title: '新列表', likes: '99' }],
  ]);

  const result = await waitForXhsSearchResultsSettled({
    document: doc,
    previousSnapshot,
    timeoutMs: 100,
    intervalMs: 0,
    stableRounds: 2,
    minWaitMs: 0,
    unchangedGraceMs: 1000,
  });

  assert.equal(result.settled, true);
  assert.equal(result.changed, true);
  assert.equal(result.snapshot.notes[0].noteId, 'new_note');
});

test('waitForXhsSearchResultsSettled skips when there is no search feed baseline', async () => {
  const result = await waitForXhsSearchResultsSettled({
    document: {
      querySelector: () => null,
      querySelectorAll: () => [],
    },
  });

  assert.equal(result.skipped, true);
  assert.equal(result.reason, 'no_baseline_feed');
});

function makeFilterPanelButton(text) {
  return {
    textContent: text,
    getAttribute: () => '',
    closest: () => null,
    click: () => {},
  };
}

function makeFilterOption(group, label, clicked) {
  const section = { textContent: `${group} ${label}` };
  const element = {
    textContent: label,
    className: '',
    getAttribute: (name) => (name === 'aria-pressed' ? 'false' : ''),
    closest: () => section,
    click: () => {
      element.className = 'active';
      clicked.push(`${group}:${label}`);
    },
  };
  return element;
}

function makeTreeElement({
  tagName = 'DIV',
  textContent = '',
  className = '',
  children = [],
  click,
} = {}) {
  const element = {
    tagName,
    textContent,
    className,
    children,
    parentElement: null,
    getAttribute: () => '',
    click: () => click?.(element),
    contains(other) {
      if (other === element) return true;
      return children.some((child) => child.contains?.(other));
    },
    closest() {
      let node = element;
      while (node) {
        if (String(node.className || '').includes('filters')) return node;
        node = node.parentElement;
      }
      return null;
    },
    querySelectorAll() {
      return flattenChildren(children);
    },
  };
  children.forEach((child) => {
    child.parentElement = element;
  });
  return element;
}

function flattenChildren(children = []) {
  return children.flatMap((child) => [child, ...flattenChildren(child.children || [])]);
}

function makeTreeDocument(children = []) {
  return {
    querySelectorAll: () => flattenChildren(children),
  };
}

function makeFeedDocument(sequence = []) {
  let readIndex = 0;
  const normalizedSequence = sequence.length > 0 ? sequence : [[]];
  const feed = {
    querySelectorAll(selector) {
      if (selector !== 'section') return [];
      const notes = normalizedSequence[Math.min(readIndex, normalizedSequence.length - 1)];
      readIndex += 1;
      return notes.map(makeFeedSection);
    },
  };

  return {
    querySelector(selector) {
      return selector === '.feeds-container' ? feed : null;
    },
    querySelectorAll: () => [],
  };
}

function makeFeedSection({ noteId, title, likes }) {
  const link = {
    href: `https://www.xiaohongshu.com/search_result/${noteId}?xsec_token=token`,
    getAttribute(name) {
      return name === 'href' ? `/search_result/${noteId}?xsec_token=token` : '';
    },
  };
  const titleEl = { textContent: title };
  const likesEl = { textContent: likes };
  return {
    textContent: `${title} ${likes}`,
    querySelector(selector) {
      if (selector === 'a.cover' || selector.includes('/search_result/')) return link;
      if (selector === '.title' || selector === '.footer span') return titleEl;
      if (selector === '.like-wrapper .count') return likesEl;
      return null;
    },
  };
}
