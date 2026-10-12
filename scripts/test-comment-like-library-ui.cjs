#!/usr/bin/env node
// Execute the real command, directory renderer and view renderer with synthetic transport.
// No worker, model provider or persistent database is involved.
const assert = require('node:assert/strict');
const { test } = require('node:test');
const { readFileSync } = require('node:fs');
const { join } = require('node:path');
const vm = require('node:vm');
const source = readFileSync(process.argv[2] || join(__dirname, '../apps/api/src/local_web/comment_study.js'), 'utf8');
function section(start, end) {
  const from = source.indexOf(start);
  const to = source.indexOf(end, from);
  assert.ok(from >= 0 && to > from, `missing actual source section: ${start}`);
  return source.slice(from, to);
}
const code = [
  section('function likeCountCell(item)', 'function restoreCommentFilter(snapshot)'),
  section('async function renderCommentsTab()', 'function bindCommentsView()'),
  section('async function recordPostUsage(button)', 'function detailSection('),
  section('async function renderActiveTab()', 'function bindRunControls(container)'),
].join('\n');
function fixture(filter, action) {
  const item = { commentKey: { workRef: 'synthetic-work', commentExternalId: 'synthetic-comment' }, postUsage: { state: filter } };
  const state = { q: '合成', studyState: 'studied', signalKind: 'experience', voiceRole: 'reader', sort: 'like_desc', minLikes: '100', postUsage: filter, workRef: 'synthetic-work', cursor: 'old-page-2', history: [null], workChoices: [], response: { items: [item], summary: { displayableCommentCount: 1 } }, summary: { displayableCommentCount: 1 } };
  const status = { textContent: '' };
  const result = { innerHTML: '', textContent: '', setAttribute() {}, querySelector() { return null; } };
  const cell = { querySelector() { return status; }, querySelectorAll() { return [button]; } };
  const button = { disabled: false, dataset: { postUsageWork: 'synthetic-work', postUsageId: 'synthetic-comment', postUsage: action }, closest() { return cell; } };
  const calls = { writes: [], reads: [], routes: 0, ids: 0 };
  const context = {
    commentCatalogState: state, commentPostUsageFeedback: '', activeView: 'comments', renderToken: 0,
    endpoint: 'synthetic/', domainRef: 'synthetic-domain', Intl, Number,
    beijingDate: new Intl.DateTimeFormat('zh-CN'), shortBeijingDate: () => '',
    selectedCommentKeys: new Map(), MAX_SELECTED_COMMENTS: 100, MAX_SELECTED_WORKS: 20,
    selectedCommentWorkCount: () => 0, commentSelectionKey: key => key.commentExternalId,
    signalKindLabel: { experience: '经历' }, workTitleSourceLabel: {},
    commentEligibility: () => '合成', commentStatus: () => '已研究', esc: value => String(value ?? ''),
    document: { querySelector: () => result }, bindCommentsView() {},
    crypto: { randomUUID() { calls.ids++; return 'synthetic-request'; } },
    resetCommentPage() { state.cursor = null; state.history = []; },
    syncStudyRoute() { calls.routes++; },
    catalogQuery: (path, params) => ({ path, params }),
    async get(query) {
      calls.reads.push(query);
      assert.equal(state.response, null, 'old directory cache must be invalidated');
      assert.equal(state.summary, null, 'old count cache must be invalidated');
      assert.equal(query.params.cursor, null, 'new snapshot must not reuse page 2 cursor');
      assert.equal(query.params.postUsage, filter);
      assert.equal(query.params.sort, 'like_desc');
      assert.equal(query.params.minLikes, '100');
      assert.equal(query.params.signalKind, 'experience');
      return { items: [], summary: { displayableCommentCount: 0, eligibleCommentCount: 0 }, indexCoverage: { indexedCount: 0, pendingCount: 0 }, page: { asOf: 'synthetic-new-snapshot', nextCursor: null } };
    },
    async fetch(path, options) {
      const body = JSON.parse(options.body); calls.writes.push(body);
      return { ok: true, async json() { return { contract: 'comment-post-usage.v1', requestRef: body.requestRef, outcome: 'applied', commentKey: item.commentKey, postUsage: { state: action === 'mark' ? 'used' : 'unused', markedAt: null } }; } };
    },
  };
  vm.createContext(context); vm.runInContext(code, context);
  context.TAB_RENDERERS = { comments: context.renderCommentsTab };
  return { context, state, status, result, button, calls, item };
}
for (const [filter, action] of [['used', 'unmark'], ['unused', 'mark']]) {
  test(`${filter}: saved ${action} refreshes page 2, membership and count with the current filters`, async () => {
    const f = fixture(filter, action);
    await f.context.recordPostUsage(f.button);
    assert.equal(f.calls.writes.length, 1); assert.equal(f.calls.reads.length, 1);
    assert.equal(f.calls.routes, 1); assert.equal(f.state.history.length, 0);
    assert.equal(f.state.response.items.length, 0); assert.equal(f.state.response.summary.displayableCommentCount, 0);
    assert.match(f.result.innerHTML, /当前筛选可显示 0 条/);
    assert.match(f.result.innerHTML, /改帖标记已保存/);
    assert.equal(f.state.postUsage, filter);
  });
}
test('unconfirmed save retains the original view and retries the same request reference', async () => {
  const f = fixture('unused', 'mark');
  const successfulFetch = f.context.fetch;
  f.context.fetch = async (path, options) => { f.calls.writes.push(JSON.parse(options.body)); throw new Error('synthetic lost response'); };
  await f.context.recordPostUsage(f.button);
  assert.equal(f.calls.reads.length, 0); assert.equal(f.state.cursor, 'old-page-2');
  assert.equal(f.item.postUsage.state, 'unused'); assert.equal(f.button.disabled, false);
  assert.match(f.status.textContent, /保存未获确认/);
  f.context.fetch = successfulFetch;
  await f.context.recordPostUsage(f.button);
  assert.equal(f.calls.writes[0].requestRef, f.calls.writes[1].requestRef);
  assert.equal(f.calls.ids, 1);
});
test('confirmed save with failed directory read keeps the save receipt and reports the read failure', async () => {
  const f = fixture('used', 'unmark');
  f.context.get = async () => { throw new Error('synthetic read failure'); };
  await f.context.recordPostUsage(f.button);
  assert.match(f.result.innerHTML, /改帖标记已保存/); assert.match(f.result.innerHTML, /列表刷新失败/);
  assert.doesNotMatch(f.result.innerHTML, /保存未获确认/);
  assert.equal(f.state.response, null); assert.equal(f.state.summary, null);
});
test('malformed HTTP success is not presented as a confirmed save', async () => {
  const f = fixture('unused', 'mark');
  f.context.fetch = async () => ({ ok: true, json: async () => ({}) });
  await f.context.recordPostUsage(f.button);
  assert.equal(f.calls.reads.length, 0); assert.match(f.status.textContent, /回执不符合约定/);
  assert.equal(f.item.postUsage.state, 'unused'); assert.equal(f.state.cursor, 'old-page-2');
});
test('a completed save does not render comments over a different active view', async () => {
  const f = fixture('used', 'unmark');
  const transport = f.context.fetch;
  f.context.fetch = async (...args) => { f.context.activeView = 'runs'; return transport(...args); };
  await f.context.recordPostUsage(f.button);
  assert.equal(f.calls.reads.length, 0); assert.equal(f.calls.routes, 0);
  assert.equal(f.state.cursor, null); assert.equal(f.state.response, null);
});
