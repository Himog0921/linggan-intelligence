// OPS-MIGRATION-001 D3 · 运营总览脚本的文字规则：结论只由接口给的数字拼出，未知写「—」，上期为 0 不比。
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';
import vm from 'node:vm';

const source = readFileSync(new URL('./ops_overview.js', import.meta.url), 'utf8');

function loadText() {
  const sandbox = { module: { exports: {} } };
  vm.runInNewContext(source, sandbox);
  return sandbox.module.exports.OverviewText;
}

const plain = (value) => JSON.parse(JSON.stringify(value));

test('uncovered blocks say not migrated; covered empty blocks keep their own empty state', () => {
  const { blockEmptyText } = loadText();
  for (const label of ['当期无发布', '当期无矩阵数据', '当期没有运营数据']) {
    assert.equal(blockEmptyText({ covered: null }, label), '这段时间还没有迁入的数据');
    assert.equal(blockEmptyText({ covered: { from: '2026-10-10', to: '2026-10-11', days: 2 } }, label), label);
  }
});

test('trend labels keep partial counts separate from complete buckets and missing imports', () => {
  const { trendPointNote } = loadText();
  assert.equal(trendPointNote({ coverage: 'not_migrated', coveredTo: null }), '还没迁入');
  assert.equal(trendPointNote({ coverage: 'partial', coveredTo: '2026-10-11' }), '按 2026-10-11 及以前计算');
  assert.equal(trendPointNote({ coverage: 'full', coveredTo: '2026-10-11' }), '');
});

test('the trend image name includes partial bucket cutoffs for assistive reading', () => {
  const { trendAccessibleLabel } = loadText();
  assert.equal(trendAccessibleLabel([{ coverage: 'full' }], 'day'), '加微与爆款长趋势，1 个日');
  assert.equal(trendAccessibleLabel([
    { label: '2026-09', coverage: 'full' },
    { label: '2026-10', coverage: 'partial', coveredTo: '2026-10-11' },
  ], 'month'), '加微与爆款长趋势，2 个月；2026-10 · 按 2026-10-11 及以前计算');
});

test('changes read as arrows; a missing or zero previous value is never a fall', () => {
  const { delta } = loadText();
  assert.deepEqual(plain(delta(null)), { text: '环比 —', tone: '' });
  assert.deepEqual(plain(delta(0)), { text: '持平上期', tone: '' });
  assert.deepEqual(plain(delta(-70)), { text: '↓70%', tone: 'down' });
  assert.deepEqual(plain(delta(12.5)), { text: '↑12.5%', tone: 'up' });
});

test('conclusions are sentences over the figures the page already shows', () => {
  const { insight } = loadText();
  assert.deepEqual(plain(insight({ kind: 'pace', pace: 'behind', achievedPct: 20, projectedPct: 35 })), {
    tone: 'risk',
    text: '目标有缺口：按当前节奏预计完成 35%',
  });
  assert.equal(insight({ kind: 'pace', pace: 'achieved', achievedPct: 120 }).text, '目标已达成（完成 120%）');
  assert.deepEqual(plain(insight({ kind: 'pace', pace: 'on_track', achievedPct: 40, projectedPct: 104 })), {
    tone: 'ok',
    text: '节奏正常：按当前节奏预计完成 104%',
  });
  assert.deepEqual(
    plain(insight({ kind: 'pace', pace: 'on_track', achievedPct: 17.1, projectedPct: 48 })),
    { tone: '', text: '按当前节奏预计完成 48%' },
    'a projection short of the target is never called on pace',
  );
  assert.equal(insight({ kind: 'pace', pace: 'on_track', projectedPct: null }).text, '按当前节奏预计完成 —');
  assert.equal(insight({ kind: 'drag', metric: 'links_opened', deltaPct: -70 }).text, '链接打开下滑是主因（环比 −70%）');
  assert.equal(insight({ kind: 'review', names: ['运营甲', '运营乙'] }).text, '运营甲、运营乙需今天复盘');
});

test('each attention item says who, what and the next step', () => {
  const { attention } = loadText();
  const drop = attention({ kind: 'drop', operator: '运营甲', metric: 'published', dropPct: -100, previousValue: 1, currentValue: 0 });
  assert.deepEqual(plain(drop), { text: '运营甲 · 发布比上期下降 100%（1 → 0）', action: '查看排行并复盘下降原因' });
  assert.deepEqual(
    plain(attention({ kind: 'zero_report', operator: '运营乙', count: 11, zeroMetrics: [{ metric: 'links_opened', previousValue: 51 }, { metric: 'followers_gained', previousValue: 32 }] })),
    { text: '运营乙 · 链接打开、加微本期 11 天都记的是 0（上期 51、32）', action: '确认是否漏填（切换前在工作台补）' },
    'zeros that may be unfilled are a check, never a fall',
  );
  assert.equal(
    attention({ kind: 'pending_views', operator: '运营甲', count: 2, oldestDate: '2024-03-05' }).text,
    '运营甲 · 待补阅读量 2 篇，最早 2024-03-05',
  );
  assert.equal(
    attention({ kind: 'owner_mismatch', operator: '运营乙', account: '账号乙', accountOperator: '运营甲', count: 1 }).text,
    '账号「账号乙」由 运营甲 负责，发布人是 运营乙（1 条）',
  );
});

test('the ranking says which figure fell, in a few words', () => {
  const { rankingStep } = loadText();
  assert.equal(rankingStep({ riskKind: 'drop', riskMetric: 'published' }), '复盘发布下降');
  assert.equal(rankingStep({ riskKind: 'drop', riskMetric: 'followers_gained' }), '复盘加微下降');
  assert.equal(rankingStep({ riskKind: 'zero_report', riskMetric: null }), '确认是否漏填');
  assert.equal(rankingStep({ riskKind: 'pending_views', riskMetric: null }), '补阅读量');
  assert.equal(rankingStep({ riskKind: 'owner_mismatch', riskMetric: null }), '确认负责人');
  assert.equal(rankingStep({ riskKind: null, riskMetric: null }), '');
});

test('the folded part of today\'s list says how many and opens in place', () => {
  const { moreLabel } = loadText();
  assert.equal(moreLabel(11, false), '另有 11 项，展开');
  assert.equal(moreLabel(11, true), '收起');
});

test('bar heights scale to the largest known value and draw nothing for unknown', () => {
  const { scale, number } = loadText();
  const height = scale([4, null, 0, 8], 100);
  assert.equal(height(8), 100);
  assert.equal(height(4), 50);
  assert.equal(height(null), 0);
  assert.equal(scale([null, 0], 100)(0), 0, 'all zero or unknown: no bars, no division by zero');
  assert.equal(number(null), '—');
  assert.equal(number(0), '0');
});

test('the window line says which days are counted and what is not migrated yet', () => {
  const { windowLine } = loadText();
  const asOf = { exportedAtBeijing: '2026-10-12 22:43' };
  const base = { current: { from: '2026-10-12', to: '2026-10-13', days: 2 }, completeThrough: '2026-10-11' };
  assert.equal(
    windowLine({ ...base, covered: null, previous: null }, asOf),
    '2026-10-12 ~ 2026-10-13 · 这段时间还没有迁入的数据（数据截至 10-12 22:43，按 10-11 及以前计算）',
  );
  const month = {
    current: { from: '2026-10-01', to: '2026-10-13', days: 13 },
    covered: { from: '2026-10-01', to: '2026-10-11', days: 11 },
    previous: { from: '2026-09-01', to: '2026-09-11', days: 11 },
    completeThrough: '2026-10-11',
  };
  assert.equal(
    windowLine(month, asOf),
    '2026-10-01 ~ 2026-10-13（按已迁入的 2026-10-01 ~ 2026-10-11（11 天）计算）· 比上期 2026-09-01 ~ 2026-09-11',
  );
  const yesterday = {
    current: { from: '2026-10-11', to: '2026-10-11', days: 1 },
    covered: { from: '2026-10-11', to: '2026-10-11', days: 1 },
    previous: { from: '2026-10-10', to: '2026-10-10', days: 1 },
    completeThrough: '2026-10-11',
  };
  assert.equal(windowLine(yesterday, asOf), '2026-10-11（1 天）· 比上期 2026-10-10', 'a single day is written once');
});

