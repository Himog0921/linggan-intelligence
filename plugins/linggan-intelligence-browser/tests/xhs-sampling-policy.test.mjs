import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

/**
 * 采样口径的三个纯函数住在 content 脚本里，那个模块导入了一堆浏览器环境的东西。
 * 这里只把这几个函数抠出来单独求值——它们不碰 DOM，是纯计算，值得单测；
 * 为此把整个内容脚本搬进 jsdom 反而会让这几条断言依赖一堆无关的东西。
 */
const source = readFileSync(
  fileURLToPath(new URL('../src/content/xhsPageController.js', import.meta.url)),
  'utf8',
);

function extract(name) {
  const at = source.indexOf(`function ${name}(`);
  assert.notEqual(at, -1, `${name} 应当存在`);
  let depth = 0;
  let started = false;
  for (let i = at; i < source.length; i += 1) {
    if (source[i] === '{') {
      depth += 1;
      started = true;
    } else if (source[i] === '}') {
      depth -= 1;
      if (started && depth === 0) return source.slice(at, i + 1);
    }
  }
  throw new Error(`${name} 的函数体没有闭合`);
}

// parseCount 是这几个函数唯一的外部依赖，按真实实现给一个等价物。
const preamble = `
  function parseCount(text) {
    const raw = String(text ?? '').trim();
    if (/万/.test(raw)) return Math.round(parseFloat(raw) * 10000);
    const n = parseFloat(raw.replace(/[^0-9.]/g, ''));
    return Number.isFinite(n) ? n : 0;
  }
`;

const { samplingFiltersFromTaskSpec, pickTopByLikes } = new Function(
  `${preamble}
   ${extract('samplingFiltersFromTaskSpec')}
   ${extract('pickTopByLikes')}
   return { samplingFiltersFromTaskSpec, pickTopByLikes };`,
)();

test('服务端的排序词翻成页面筛选值', () => {
  for (const [ranking, sortBasis] of [
    ['most_liked', 'most_liked'],
    ['most_collected', 'most_collected'],
    ['most_commented', 'most_commented'],
    ['latest', 'latest'],
    ['comprehensive', 'general'],
  ]) {
    assert.equal(
      samplingFiltersFromTaskSpec({ target: { ranking } }).sortBasis,
      sortBasis,
    );
  }
});

test('认不出来的排序不设筛选，让页面保持当前排序', () => {
  const filters = samplingFiltersFromTaskSpec({ target: { ranking: 'by_vibes' } });
  assert.equal(filters.sortBasis, undefined);
});

test('没有口径的任务不产生任何筛选', () => {
  assert.deepEqual(samplingFiltersFromTaskSpec({ target: {} }), {});
  assert.deepEqual(samplingFiltersFromTaskSpec(undefined), {});
});

test('天数向上取到不小于请求的那一档——宁可多采也不能漏采', () => {
  const at = (days) => samplingFiltersFromTaskSpec({ target: { publishedWithinDays: days } }).publishTime;
  assert.equal(at(1), 'one_day');
  // 要 3 天却给「一天内」会漏掉第 2、3 天的内容；给「一周内」只是多采一些。
  assert.equal(at(3), 'one_week');
  assert.equal(at(7), 'one_week');
  assert.equal(at(30), 'half_year');
  // 不填就是不限，不该凭空造一个时间档位。
  assert.equal(at(undefined), undefined);
  assert.equal(at(0), undefined);
});

test('按点赞取前 N，且返回顺序仍是页面原始位置', () => {
  const cards = [
    { id: 'a', likes: '100' },
    { id: 'b', likes: '3万' },
    { id: 'c', likes: '5' },
    { id: 'd', likes: '2000' },
  ];
  const picked = pickTopByLikes(cards, 2);
  // 选中的是 b(3万) 与 d(2000)，但顺序按它们在页面上的先后。
  assert.deepEqual(picked.map((card) => card.id), ['b', 'd']);
  // 名次单独记着——它是「爆」徽章的依据，不能靠数组顺序推断。
  assert.deepEqual(picked.map((card) => card.__topRank), [1, 2]);
});

test('加载数不足 N 时原样返回，不做无谓重排', () => {
  const cards = [{ id: 'a', likes: '1' }, { id: 'b', likes: '2' }];
  assert.equal(pickTopByLikes(cards, 20), cards);
  assert.equal(pickTopByLikes(cards, 0), cards);
  assert.equal(pickTopByLikes(cards, undefined), cards);
});

/**
 * 这一组守的是「回执不许替一个未发生的状态背书」。
 *
 * 真实事故：`readCurrentXhsSearchFilterSnapshot` 直接读 `window.__INITIAL_STATE__`，
 * 而 content script 跑在 Chrome 的隔离世界里，按设计看不见页面的 JS 变量——于是它恒
 * 返回默认值「沿用当前」。一次真的按最多点赞采回的样本（点赞 4309→3363→1206 严格
 * 递减）被回执写成了「筛选没生效」，反过来污蔑了一次正常采集。
 */
test('筛选快照必须经注入读取，且注册在可注入类型表里', () => {
  const filters = readFileSync(
    fileURLToPath(new URL('../src/platforms/xhs/searchFilters.js', import.meta.url)),
    'utf8',
  );
  const utils = readFileSync(
    fileURLToPath(new URL('../src/shared/utils.js', import.meta.url)),
    'utf8',
  );
  const controller = readFileSync(
    fileURLToPath(new URL('../src/content/xhsPageController.js', import.meta.url)),
    'utf8',
  );

  assert.match(filters, /readXhsSearchFilterSnapshotByInject/);
  assert.match(filters, /getByInject\(wd, 'searchFilters'\)/);
  // 没在 fileMap 里注册，注入会直接抛 Unknown inject type。
  assert.match(utils, /searchFilters: 'injected\/searchFilters\.js'/);
  // 回执必须用注入版；用回同步版就等于把这个缺陷放回去。
  assert.match(controller, /await readXhsSearchFilterSnapshotByInject/);
  assert.ok(
    !/effective: filterOutcome\?\.snapshot/.test(controller),
    '回执不能再用同步快照——它在真实页面上读不到',
  );
});

test('注入脚本必须被 manifest 放行，否则加载即失败', async () => {
  const manifest = JSON.parse(
    readFileSync(fileURLToPath(new URL('../manifest.json', import.meta.url)), 'utf8'),
  );
  const xhs = manifest.web_accessible_resources.find((entry) =>
    entry.matches.some((match) => match.includes('xiaohongshu')),
  );
  assert.ok(xhs.resources.includes('injected/searchFilters.js'));
});

test('读不到时说读不到，不伪装成「什么都没选」', () => {
  const injected = readFileSync(
    fileURLToPath(new URL('../src/injected/searchFilters.js', import.meta.url)),
    'utf8',
  );
  // readable 把「读不到」与「页面上什么都没选」分开——两者的含义完全不同。
  assert.match(injected, /readable: Boolean\(window\.__INITIAL_STATE__\)/);
  const filters = readFileSync(
    fileURLToPath(new URL('../src/platforms/xhs/searchFilters.js', import.meta.url)),
    'utf8',
  );
  assert.match(filters, /readable: false/);
});
