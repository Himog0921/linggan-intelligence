// T32「选择器检查成功 / 缺失 / 验证日期陈旧」的三态与两条硬边界：
// 1. `checked_at`（这次检查的时刻）与 `verified_at`（上次人工重验的日期）必须分开；
// 2. 出浏览器的快照只带受限形状，不带选择器串、DOM 文本或带签名的页面地址。
//
// 这里的快照不是手写的 fixture：三态都由**真实 producer**（小红书/抖音的 preflight）
// 跑出来，再经本插件自己的上报口收成受限快照。手写快照只能证明我对字段的理解一致，
// 证明不了页面真的会那样上报。

import test from 'node:test';
import assert from 'node:assert/strict';

import {
  SELECTOR_HEALTH_SNAPSHOT_FIELDS,
  buildSelectorHealthSnapshot,
  installSelectorHealthReporter,
} from '../src/shared/selectorHealth.js';
import { runXhsSelectorBootstrapProbe, runXhsSelectorPreflight } from '../src/platforms/xhs/selectorHealth.js';
import { runDouyinSelectorPreflight } from '../src/platforms/douyin/selectorHealth.js';
import { checkInLingganStation } from '../src/linggan/adapter.js';
import {
  SELECTOR_HEALTH_STORAGE_KEY,
  recordSelectorHealthReport,
  selectorHealthForCheckIn,
} from '../src/linggan/selectorHealthReport.js';

function createDocument(queryMap = {}) {
  return {
    title: '',
    querySelector(selector) {
      const value = queryMap[selector];
      if (Array.isArray(value)) return value[0] || null;
      return value || null;
    },
    querySelectorAll(selector) {
      const value = queryMap[selector];
      // 真的 `querySelectorAll` 永远给一个列表（哪怕空的），不是 undefined。
      return Array.isArray(value) ? value : [];
    },
  };
}

/// 真的内容脚本入口装的东西：把快照交给宿主。
function captureSnapshots(win) {
  const snapshots = [];
  installSelectorHealthReporter((snapshot) => {
    snapshots.push(snapshot);
    return Promise.resolve({ accepted: true });
  }, win);
  return snapshots;
}

function xhsSearchWin(href = 'https://www.xiaohongshu.com/search_result/coffee?keyword=%E5%92%96%E5%95%A1') {
  return { location: { href, pathname: new URL(href).pathname } };
}

test('selector health reports a checked-and-present check as success, with two separate moments', () => {
  const win = xhsSearchWin();
  const snapshots = captureSnapshots(win);

  runXhsSelectorBootstrapProbe({ document: createDocument({ '.feeds-container': { nodeType: 1 } }), win });

  assert.equal(snapshots.length, 1);
  const [snapshot] = snapshots;
  assert.equal(snapshot.platform, 'xhs');
  assert.equal(snapshot.pageType, 'search_results');
  assert.deepEqual(snapshot.checkedCategories, ['feed_container']);
  assert.deepEqual(snapshot.missingCategories, []);

  // 两个时刻是两个事实：一个是「刚刚看的」，一个是「四月/六月人工重验过的」。
  // 它们不一样才是常态——把检查时刻当成验证日期，等于把刚看过的页面说成刚验过的选择器。
  assert.match(snapshot.verifiedAt, /^2026-06-01T/);
  assert.notEqual(snapshot.checkedAt, snapshot.verifiedAt);
  assert.ok(Date.parse(snapshot.checkedAt) - Date.parse(snapshot.verifiedAt) > 30 * 24 * 60 * 60 * 1000);
});

test('selector health reports a checked-and-missing check as missing, not as unknown', () => {
  const win = xhsSearchWin();
  const snapshots = captureSnapshots(win);

  runXhsSelectorBootstrapProbe({ document: createDocument({}), win });

  assert.equal(snapshots.length, 1);
  // 「查了，没有」与「这次没查它」必须能分开：前者在 missingCategories 里，后者两个列表里都没有。
  assert.deepEqual(snapshots[0].checkedCategories, ['feed_container']);
  assert.deepEqual(snapshots[0].missingCategories, ['feed_container']);
});

test('selector health keeps a stale verification date visible instead of restamping it', () => {
  const staleWin = xhsSearchWin();
  const staleSnapshots = captureSnapshots(staleWin);
  runXhsSelectorBootstrapProbe({ document: createDocument({ '.feeds-container': { nodeType: 1 } }), win: staleWin });

  const freshWin = { location: { href: 'https://www.xiaohongshu.com/explore/6a8e86de00000000240043b0', pathname: '/explore/6a8e86de00000000240043b0' } };
  const freshSnapshots = captureSnapshots(freshWin);
  const detailDocument = createDocument({});
  detailDocument.scripts = [{
    textContent: 'window.__INITIAL_STATE__={"note":{"noteDetailMap":{"6a8e86de00000000240043b0":{}}}}',
  }];
  runXhsSelectorBootstrapProbe({ document: detailDocument, win: freshWin });

  // 四月验过的检查项到今天仍是「陈旧」，页面这次看它在不在，都不改变这个事实。
  assert.deepEqual(staleSnapshots[0].staleCategories, ['feed_container']);
  // SSR 那条检查没有固定验证日期，走的是「现在」——所以它不是陈旧。
  assert.deepEqual(freshSnapshots[0].staleCategories, []);
  assert.match(freshSnapshots[0].verifiedAt, /^20\d{2}-\d{2}-\d{2}T/);
});

test('the earliest verification date wins when one page check covers several categories', () => {
  const snapshot = buildSelectorHealthSnapshot({
    platform: 'xhs',
    action: 'batchNotes',
    pageType: 'search_results',
    checkedAt: Date.parse('2026-09-21T10:00:00Z'),
    checks: [
      { name: 'feed_container', ok: true, verifiedAt: '2026-06-01T00:00:00+08:00' },
      { name: 'older_check', ok: true, verifiedAt: '2026-04-28T00:00:00+08:00' },
    ],
  });

  // 不能拿最新的那份说「这些选择器都是六月验过的」。
  assert.equal(snapshot.verifiedAt, '2026-04-28T00:00:00+08:00');
  assert.deepEqual(snapshot.checkedCategories, ['feed_container', 'older_check']);
});

test('a selector health snapshot carries only whitelisted fields, never selectors, DOM text or signed URLs', () => {
  const signedHref = 'https://www.xiaohongshu.com/explore/6a8e86de00000000240043b0?xsec_token=SIGNED_TOKEN_VALUE';
  const win = { location: { href: signedHref, pathname: '/explore/6a8e86de00000000240043b0' } };
  const snapshots = captureSnapshots(win);

  runXhsSelectorPreflight('collectCommentImages', { document: createDocument({}), win });

  assert.equal(snapshots.length, 1);
  const snapshot = snapshots[0];
  assert.deepEqual(Object.keys(snapshot).sort(), [...SELECTOR_HEALTH_SNAPSHOT_FIELDS].sort());

  const serialized = JSON.stringify(snapshot);
  for (const secret of [
    'xsec_token', 'SIGNED_TOKEN_VALUE', 'xiaohongshu.com', 'https',
    '.comments-el', 'comments_container_selector', '评论容器',
  ]) {
    assert.equal(serialized.includes(secret), false, `快照不该带出 ${secret}`);
  }
});

test('selector health reports the page the check happened on, not the page the task wanted', () => {
  const win = xhsSearchWin();
  const snapshots = captureSnapshots(win);

  // 任务要的是博主页模式，人却停在搜索页：诊断要如实说这次是在搜索页看的，
  // 否则「页面不符」这条最该被看见的信息会被任务参数盖掉。
  runXhsSelectorPreflight('batchNotes', {
    params: { mode: 'profile' },
    document: createDocument({}),
    win,
  });

  assert.equal(snapshots[0].pageType, 'search_results');
  assert.deepEqual(snapshots[0].missingCategories, ['profile_feed_container']);
});

test('selector health maps both platforms local page spellings into one reported vocabulary', () => {
  const cases = [
    {
      expected: 'search_results',
      run: () => {
        const context = douyinPage('/search/%E5%92%96%E5%95%A1');
        const snapshots = captureSnapshots(context.win);
        runDouyinSelectorPreflight('dy_batchVideos', context);
        return snapshots[0];
      },
    },
    {
      expected: 'detail',
      run: () => {
        const context = douyinPage('/video/7123456789');
        const snapshots = captureSnapshots(context.win);
        runDouyinSelectorPreflight('dy_collectVideo', context);
        return snapshots[0];
      },
    },
    {
      expected: 'profile',
      run: () => {
        const context = douyinPage('/user/MS4wLjABAAAA');
        const snapshots = captureSnapshots(context.win);
        runDouyinSelectorPreflight('dy_collectAuthor', context);
        return snapshots[0];
      },
    },
    {
      expected: 'explore',
      run: () => {
        const win = { location: { href: 'https://www.xiaohongshu.com/', pathname: '/' } };
        const snapshots = captureSnapshots(win);
        runXhsSelectorBootstrapProbe({ document: createDocument({}), win });
        return snapshots[0];
      },
    },
  ];

  for (const { expected, run } of cases) {
    assert.equal(run()?.pageType, expected, `上报页面类型应为 ${expected}`);
  }
});

/// 探针和上报口必须落在**同一个** `win` 上：装了上报口的那个页面才会上报。
function douyinPage(pathname, queryMap = {}) {
  const document = createDocument(queryMap);
  const win = {
    location: {
      href: `https://www.douyin.com${pathname}`,
      pathname,
      search: '',
      origin: 'https://www.douyin.com',
    },
    document,
  };
  return { params: {}, document, win };
}

function createStorage(initial = {}) {
  let state = { ...initial };
  return {
    async get(key) {
      return key in state ? { [key]: state[key] } : {};
    },
    async set(patch) {
      state = { ...state, ...patch };
    },
    read: () => state,
  };
}

function snapshotFixture(overrides = {}) {
  return {
    platform: 'xhs',
    pageType: 'search_results',
    capability: 'discovery_search',
    checkedAt: '2026-09-21T10:00:00.000Z',
    verifiedAt: '2026-06-01T00:00:00+08:00',
    checkedCategories: ['feed_container'],
    missingCategories: ['feed_container'],
    staleCategories: ['feed_container'],
    ...overrides,
  };
}

test('consecutive misses are counted per category and reset by a check that finds it', async () => {
  const storage = createStorage();
  const record = (snapshot, now) => recordSelectorHealthReport(snapshot, { platform: 'xhs', storage, now });

  await record(snapshotFixture(), Date.parse('2026-09-21T10:00:00Z'));
  assert.deepEqual(
    storage.read()[SELECTOR_HEALTH_STORAGE_KEY].xhs.failureCounts,
    { feed_container: 1 },
  );

  await record(snapshotFixture(), Date.parse('2026-09-21T10:01:00Z'));
  assert.deepEqual(
    storage.read()[SELECTOR_HEALTH_STORAGE_KEY].xhs.failureCounts,
    { feed_container: 2 },
  );

  // 查了、它在：连续失败到此为止，条目消失（不是减一，也不是留着）。
  await record(snapshotFixture({ missingCategories: [] }), Date.parse('2026-09-21T10:02:00Z'));
  assert.deepEqual(storage.read()[SELECTOR_HEALTH_STORAGE_KEY].xhs.failureCounts, {});

  // 这次没查它：既不续，也不冒充已恢复。
  await record(snapshotFixture({ missingCategories: ['feed_container'] }), Date.parse('2026-09-21T10:03:00Z'));
  await record(
    snapshotFixture({ checkedCategories: ['other_check'], missingCategories: ['other_check'] }),
    Date.parse('2026-09-21T10:04:00Z'),
  );
  assert.deepEqual(
    storage.read()[SELECTOR_HEALTH_STORAGE_KEY].xhs.failureCounts,
    { feed_container: 1, other_check: 1 },
  );
});

test('a page can only report its own platform, and only for the two supported sites', async () => {
  const storage = createStorage();

  // 抖音页面替小红书报：拒收。
  assert.deepEqual(
    await recordSelectorHealthReport(snapshotFixture(), { platform: 'douyin', storage }),
    { accepted: false, reason: 'selector_health_platform_mismatch' },
  );
  // 两个站点之外的来源：拒收。
  assert.deepEqual(
    await recordSelectorHealthReport(snapshotFixture(), { platform: 'weibo', storage }),
    { accepted: false, reason: 'selector_health_source_unrecognized' },
  );
  // 连平台名都说不出来的东西：拒收，且不写任何记录。
  for (const invalid of [null, {}, [], 'xhs', { platform: 'xhs ; drop' }]) {
    assert.deepEqual(
      await recordSelectorHealthReport(invalid, { platform: 'xhs', storage }),
      { accepted: false, reason: 'selector_health_snapshot_invalid' },
      `${JSON.stringify(invalid)} 不是快照`,
    );
  }
  assert.equal(storage.read()[SELECTOR_HEALTH_STORAGE_KEY], undefined);

  // 平台名靠得住、别的字段是垃圾：收下，但每一项都收成 unknown——
  // 「说不上来的」写成 unknown，不是把垃圾当事实存下来。
  await recordSelectorHealthReport({ platform: 'xhs', pageType: 42 }, { platform: 'xhs', storage });
  const coerced = storage.read()[SELECTOR_HEALTH_STORAGE_KEY].xhs.snapshot;
  assert.equal(coerced.pageType, 'unknown');
  assert.equal(coerced.capability, 'unknown');
  assert.deepEqual(coerced.checkedCategories, []);
  storage.set ? await storage.set({ [SELECTOR_HEALTH_STORAGE_KEY]: undefined }) : null;

  // 收下之后存的是收口后的形状，不是页面报来的原样。
  const accepted = await recordSelectorHealthReport(
    snapshotFixture({ pageType: 'noteDetail', capability: 'DISCOVERY_SEARCH' }),
    { platform: 'xhs', storage, now: Date.parse('2026-09-21T10:00:00Z') },
  );
  assert.deepEqual(accepted, { accepted: true, platform: 'xhs' });
  const stored = storage.read()[SELECTOR_HEALTH_STORAGE_KEY].xhs;
  assert.equal(stored.snapshot.pageType, 'note_detail');
  assert.equal(stored.snapshot.capability, 'discovery_search');
  assert.equal(stored.recordedAt, '2026-09-21T10:00:00.000Z');
});

test('the check-in payload is the snapshot plus failure counts, with nothing platform-less invented', async () => {
  const storage = createStorage();
  await recordSelectorHealthReport(snapshotFixture(), {
    platform: 'xhs',
    storage,
    now: Date.parse('2026-09-21T10:00:00Z'),
  });

  const payload = await selectorHealthForCheckIn({ storage });

  // 只出现「有记录」的平台：没有记录不是「一切正常」。
  assert.deepEqual(Object.keys(payload), ['xhs']);
  assert.deepEqual(
    Object.keys(payload.xhs).sort(),
    [...SELECTOR_HEALTH_SNAPSHOT_FIELDS, 'failureCounts'].sort(),
  );
  assert.deepEqual(payload.xhs.failureCounts, { feed_container: 1 });
  // 本机记录的存储时刻不进报文：它只是本机的记录时间，不是服务端要知道的事实。
  assert.equal(JSON.stringify(payload).includes('recordedAt'), false);

  assert.deepEqual(await selectorHealthForCheckIn({ storage: createStorage() }), {});
});

test('the station check-in carries the selector health and stays silent when there is none', async () => {
  const health = { routes: { station: { checkIn: '/api/local/stations/installations' } } };
  const bodies = [];
  const fetchImpl = async (url, init) => {
    bodies.push(JSON.parse(init.body));
    return { ok: true, json: async () => ({ state: 'heartbeat' }) };
  };

  await checkInLingganStation({
    installKey: 'install-1',
    pluginVersion: '0.8.54',
    health,
    fetchImpl,
    selectorHealth: { xhs: { ...snapshotFixture(), failureCounts: { feed_container: 3 } } },
  });
  assert.deepEqual(bodies[0].selectorHealth.xhs.failureCounts, { feed_container: 3 });

  // 没有记录时不带这个字段：缺席就是「本机还没检查过」，不是「检查过、一切正常」。
  await checkInLingganStation({ installKey: 'install-1', pluginVersion: '0.8.54', health, fetchImpl });
  assert.equal('selectorHealth' in bodies[1], false);
});
