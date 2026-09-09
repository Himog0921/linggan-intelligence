// Synthetic DOM proof: no provider calls, private sources, or runtime mutations.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import vm from 'node:vm';

const path = new URL('../../apps/api/src/local_web/comment_intelligence.js', import.meta.url);
const original = readFileSync(path, 'utf8');
function setup(overrides = {}, search = '?view=voices&limit=50') {
  const nodes = new Map();
  const listeners = new Map();
  class Element {
    constructor() { this.dataset = {}; this.classList = { toggle() {}, add() {}, remove() {} }; this.style = { setProperty() {} }; this.hidden = true; this.innerHTML = ''; this.scrollTop = 0; this.open = false; }
    addEventListener(kind, callback) { listeners.set(kind, callback); }
    setAttribute() {}
    getBoundingClientRect() { return { top: 0, bottom: 0, width: 1200, left: 0 }; }
    focus() {}
    showModal() { this.open = true; }
    close() { this.open = false; }
    querySelector() { return null; }
  }
  const get = (id) => { if (!nodes.has(id)) nodes.set(id, new Element()); return nodes.get(id); };
  const main = get('main'); main.dataset.initialView = 'voices';
  const fixture = {
    scope: { from: '2026-09-01T00:00:00Z', to: '2026-09-08T00:00:00Z', resultRevision: '1', asOf: '2026-09-08T00:00:00Z' },
    summary: { comments: 150, analyzed: 26, eligible: 150, lenses: { need: 12, story: 6, quote: 3, solution: 1 } },
    page: { total: 150, items: [] }, rules: { advancedReleaseEnabled: false }, problemCandidates: [], candidateTotal: 0,
    daily: { items: [], schedule: { enabled: false } }, ...overrides,
  };
  const requests = [];
  const dailySettings = overrides.dailySettings || {
    schedule: {
      revision: 3,
      configRef: 'config-ref',
      sourceLimit: 100,
      enabled: false,
      autoPolicy: {
        continuousNew: true,
        historicalEnabled: false,
        historyStart: '',
        outdatedPolicy: 'disabled',
        unknownRetryMaxAttempts: 1,
        unknownRetryTokenLimit: 2000,
        dayTokenLimit: 100000,
        replayTokenLimit: 12000,
        semanticTokenLimit: 8000,
      },
    },
  };
  const window = {
    addEventListener() {},
    CommentDaily: {
      onRefresh() {},
      openSettings() {},
      mediaHtml() { return ''; },
      async readSettings() { return structuredClone(dailySettings); },
      async saveSchedule(body) {
        requests.push({ url: '/api/local/comment-research/daily/schedule', body });
        return { schedule: structuredClone(dailySettings.schedule) };
      },
    },
  };
  const context = {
    window, document: { querySelector: (q) => q === '[data-initial-view]' ? main : null, querySelectorAll: () => [], getElementById: get, createElement: () => new Element(), addEventListener: (k, cb) => listeners.set(k, cb), visibilityState: 'visible', body: { dataset: { corpusDomain: 'adhd' }, append() {} } },
    location: { search, pathname: '/corpus/comments' }, history: { replaceState() {}, pushState() {} },
    URLSearchParams, ResizeObserver: class { observe() {} }, AbortController, setTimeout: () => 1, clearTimeout() {},
    fetch: async (url, options) => {
      requests.push({ url, body: options.body && JSON.parse(options.body) });
      return { ok: true, json: async () => structuredClone(fixture) };
    }, crypto: { randomUUID: () => 'command-ref' },
  };
  const instrumented = original.replace(/  load\(\)\.then\(\(\) => \{\n    if \(params.get\("source"\)\) openSource\(params.get\("source"\)\);\n  \}\);/, '  window.test = { state, selected, selectedSources, selectSource, navigate, render, renderPagination, renderSelection, renderProblems, renderDaily, candidateList, observations, voiceTable, researchReason, clearSelectionForScope, handle, openRequest, openContextSettings, openResearchSettings, openReplayDetail, openPrepare, startReplay, runCommand: (f) => command(f), setData: (v) => { data = v; }, setBatch: (v) => { batchDetail = v; } };');
  assert.notEqual(instrumented, original, 'test instrumentation must locate startup without changing product logic');
  vm.runInNewContext(instrumented, context);
  window.test.setData(fixture);
  return { app: window.test, nodes, get, fixture, requests, context, listeners };
}
const comments = (start, count) => Array.from({ length: count }, (_, i) => ({ sourceRef: `source-${start + i}`, body: `合成原声 ${start + i}`, workTitle: '合成作品' }));

test('50 selections survive page, sort and limit changes; deselecting a page preserves the other page', async () => {
  const { app, fixture } = setup();
  fixture.page.items = comments(0, 50);
  fixture.page.items.forEach((s) => app.selectSource(s, true));
  await app.navigate({ offset: 50 });
  assert.equal(app.selected.size, 50);
  fixture.page.items = comments(50, 50);
  fixture.page.items.forEach((s) => app.selectSource(s, true));
  assert.equal(app.selected.size, 100);
  await app.navigate({ sort: 'likes' });
  await app.navigate({ limit: 20 });
  assert.equal(app.selected.size, 100);
  fixture.page.items.forEach((s) => app.selectSource(s, false));
  assert.equal(app.selected.size, 50);
  assert.ok(app.selected.has('source-0'));
  assert.ok(!app.selected.has('source-50'));
});

test('actual scope change clears hidden selections with visible feedback', async () => {
  const { app, get } = setup();
  app.selectSource(comments(0, 1)[0], true);
  await app.navigate({ text: '新的关键词' });
  assert.equal(app.selected.size, 0);
  assert.match(get('feedback').textContent, /筛选范围已改变.*1/);
  assert.equal(app.selectedSources.size, 0);
});

test('reapplying identical filter does not erase selection', async () => {
  const { app } = setup();
  app.selectSource(comments(0, 1)[0], true);
  await app.navigate({ text: '' });
  assert.equal(app.selected.size, 1);
});

test('pagination exposes exact numbered and jump actions without loading every comment', () => {
  const { app, fixture, get } = setup();
  fixture.page.total = 5081;
  app.state.offset = 50 * 40;
  app.renderPagination();
  assert.match(get('ci-pagination').innerHTML, /data-page="41"[^>]*aria-current="page"/);
  assert.match(get('ci-pagination').innerHTML, /data-page="102"/);
  assert.match(get('ci-pagination').innerHTML, /id="ci-page-jump"/);
  assert.match(get('ci-pagination').innerHTML, /max="102"/);
});

test('problem candidates remain visible without stable problems and escape source content', () => {
  const { app, fixture, get } = setup();
  fixture.problemCandidates = [{ candidateRef: 'c1', name: '<script>不可执行</script>', meaning: '合成边界', comments: 2, works: 1, sourceRefs: ['source-1'], evidence: [{ quote: '用户原声' }] }];
  fixture.candidateTotal = 1;
  app.renderProblems();
  const html = get('results').innerHTML;
  assert.match(html, /待整理的问题表达/);
  assert.match(html, /data-ci="candidate-voices"/);
  assert.match(html, /data-ci="candidate-create"/);
  assert.ok(html.includes('&lt;script&gt;'));
  assert.ok(!html.includes('<script>'));
});

test('advanced observations disabled is not presented as no discovery', () => {
  const { app } = setup();
  const html = app.observations([]);
  assert.match(html, /高级观察尚未启用/);
  assert.doesNotMatch(html, /未发现符合规则/);
});

test('daily first screen separates meaningful, no-signal and failed outputs', () => {
  const { app, fixture, get } = setup();
  app.state.batchRef = 'b1';
  fixture.daily.items = [{ batchRef: 'b1', counts: { succeeded: 20, no_signal: 6, failed: 24 }, total: 50, works: 11, kind: 'manual' }];
  app.setBatch({ calls: [{ invocationRef: 'i1', state: 'failed', requestedComments: 7, acceptedComments: 0, workTitle: '合成作品', inputTokens: null, outputTokens: null, usageUnknown: true, reservedTokens: 18000, failureCode: 'provider_failed' }] });
  app.renderDaily();
  const html = get('results').innerHTML;
  for (const label of ['提取到研究结果', '未提取到信号', '分析未完成', '模型怎样处理这一批', '18,000', '预留不等于已确认消耗']) assert.ok(html.includes(label), label);
  assert.match(html, /data-ci="request-detail"/);
  assert.doesNotMatch(html, /本次未发现符合规则/);
});

test('old trace details explain absent retention and never invent historical prompt', async () => {
  const { app, context, get } = setup();
  context.fetch = async () => ({ ok: true, json: async () => ({ availability: 'NOT_RECORDED', call: { state: 'failed', requestedComments: 7, usageUnknown: true }, events: [], validation: [] }) });
  await app.openRequest('b1', 'i1');
  const html = get('ci-command-fields').innerHTML;
  assert.match(html, /没有保留输入与返回/);
  assert.match(html, /未记录分阶段事件/);
  assert.doesNotMatch(html, /模型实际拿到的材料/);
});

test('context controls disclose retention and do not save or enable anything on open', async () => {
  const { app, context, get, requests } = setup();
  context.fetch = async (url, options) => {
    requests.push({ url, body: options.body && JSON.parse(options.body) });
    return { ok: true, json: async () => url.includes('model-settings') ? { config: { inputTokenLimit: 16000, outputTokenLimit: 3000 } } : { revision: 4, policy: { workBody: true, ocr: true, asr: true, parent: true, existingProblems: true, recallLimit: 10, maxComments: 7, recordContent: false } } };
  };
  await app.openContextSettings();
  const html = get('ci-command-fields').innerHTML;
  assert.match(html, /保留输入与返回 24 小时/);
  assert.match(html, /保存这里的上下文设置不会开启自动研究/);
  assert.doesNotMatch(html, /name="enabled"/);
  assert.equal(requests.filter((r) => r.body).length, 0);
});


test('daily opening resolves the latest batch and refetches its exact scope', async () => {
  const { app, fixture, requests } = setup();
  fixture.daily.items = [{ batchRef: 'newest', total: 50, works: 11, counts: { succeeded: 20, no_signal: 6, failed: 24 } }];
  await app.navigate({ view: 'daily' });
  assert.equal(app.state.batchRef, 'newest');
  assert.ok(requests.some((r) => r.url.startsWith('/api/local/comment-intelligence?') && r.url.includes('batchRef=newest')));
  assert.ok(requests.some((r) => r.url.startsWith('/api/local/comment-research/daily/newest')));
});

test('context save sends frozen expected revision and context policy only', async () => {
  const { app, context, fixture, requests } = setup();
  context.fetch = async (url, options) => {
    requests.push({ url, body: options.body && JSON.parse(options.body) });
    return { ok: true, json: async () => url === '/api/local/model-settings' ? { config: {} } : url.includes('/context-settings') ? { revision: 4, policy: { workBody: true, ocr: true, asr: true, parent: true, existingProblems: true, recallLimit: 10, maxComments: 7, recordContent: false } } : structuredClone(fixture) };
  };
  await app.openContextSettings();
  const values = new Map([['workBody', 'on'], ['parent', 'on'], ['recallLimit', '4'], ['maxComments', '3']]);
  await app.runCommand({ get: (k) => values.get(k), has: (k) => values.has(k) });
  const mutations = requests.filter((r) => r.body);
  assert.equal(mutations.length, 1);
  assert.equal(mutations[0].url, '/api/local/comment-research/daily/context-settings');
  assert.deepEqual(mutations[0].body, { expectedRevision: 4, policy: { workBody: true, ocr: false, asr: false, parent: true, existingProblems: false, recallLimit: 4, maxComments: 3, recordContent: false } });
});

const researchSettingsFixture = () => ({
  activeRule: {
    ruleRevisionRef: 'baseline-rule',
    canonicalHash: 'baseline-hash',
    schemaVersion: 'comment-packet.v4',
    selectorVersion: 'context-selector.v4',
    ruleVersion: 'comment-research.v4',
    purpose: { title: '基线研究方向', instruction: '只提取明确表达。' },
    fieldDefinitions: [{ kind: 'need', name: '需求', definition: '明确需要帮助。' }],
    examples: [{ field: 'need', polarity: 'positive', comment: '我需要更具体的支持。', expected: '算作明确需求。' }],
  },
  candidates: [{
    ruleRevisionRef: 'candidate-rule',
    canonicalHash: 'candidate-hash',
    schemaVersion: 'comment-packet.v5',
    selectorVersion: 'context-selector.v5',
    ruleVersion: 'comment-research.v5',
    creationSource: 'user_candidate',
    purpose: { title: '候选研究方向', instruction: '保留明确例外。' },
    fieldDefinitions: [{ kind: 'need', name: '需求', definition: '明确需要帮助。' }],
    examples: [],
  }],
  upgradePolicy: { revision: 8, enabled: false, selectedCandidateRuleRevisionRef: null },
  replays: { items: [] },
  programSafeguards: '来源资格、结构接纳和预算仍由服务端检查。',
  replaySampling: '按作品均衡抽样；最多 120 个成员。',
});

function installResearchSettingsResponses(context, requests, fixture, settings = researchSettingsFixture()) {
  const contextSettings = {
    revision: 4,
    policy: { workBody: true, ocr: true, asr: false, parent: true, existingProblems: true, recallLimit: 6, maxComments: 8, recordContent: false },
  };
  const modelSettings = {
    config: { configRef: 'config-ref' },
    model: { modelConnected: true },
    models: [{ modelRef: 'config-ref', modelId: 'synthetic-model' }],
  };
  context.fetch = async (url, options = {}) => {
    requests.push({ url, body: options.body && JSON.parse(options.body) });
    if (url === '/api/local/comment-research/daily/context-settings')
      return { ok: true, json: async () => structuredClone(contextSettings) };
    if (url === '/api/local/model-settings')
      return { ok: true, json: async () => structuredClone(modelSettings) };
    if (url === '/api/local/comment-intelligence/research-settings')
      return { ok: true, json: async () => structuredClone(settings) };
    return { ok: true, json: async () => structuredClone(fixture) };
  };
  return { contextSettings, modelSettings, settings };
}

function researchSettingsForm(overrides = {}) {
  const values = new Map(Object.entries({
    historyStart: '', outdatedPolicy: 'current_only', dayTokenLimit: '100000',
    unknownRetryMaxAttempts: '1', unknownRetryTokenLimit: '2000', replayTokenLimit: '12000',
    semanticTokenLimit: '8000', sourceLimit: '100', recallLimit: '6', maxComments: '8',
    upgradeCandidate: '', ...overrides.values,
  }));
  const checked = new Set(overrides.checked || ['continuousNew', 'workBody', 'ocr', 'parent', 'existingProblems']);
  return { get: (key) => values.get(key), has: (key) => checked.has(key) };
}

test('research settings reads all policy surfaces before save and persists only the real configuration DTOs', async () => {
  const { app, context, fixture, requests, get } = setup();
  installResearchSettingsResponses(context, requests, fixture);
  await app.openResearchSettings();
  const html = get('ci-command-fields').innerHTML;
  assert.match(html, /每日自动研究评论上限/);
  assert.match(html, /仅更新启用后的评论/);
  assert.match(html, /不包含历史评论/);
  assert.match(html, /候选规则与自动采用/);
  assert.match(html, /版本比较（shadow）/);
  assert.equal(requests.filter((request) => request.body).length, 0, 'opening settings is read-only');

  await app.runCommand(researchSettingsForm());
  const mutations = requests.filter((request) => request.body);
  assert.deepEqual(mutations.map((request) => request.url), [
    '/api/local/comment-research/daily/context-settings',
    '/api/local/comment-research/daily/schedule',
  ]);
  assert.equal(mutations[0].body.expectedRevision, 4);
  assert.equal(mutations[1].body.expectedRevision, 3);
  assert.equal(mutations[1].body.sourceLimit, 100);
  assert.equal(mutations[1].body.autoPolicy.outdatedPolicy, 'current_only');
  assert.equal(mutations[1].body.autoPolicy.dayTokenLimit, 100000);
  assert.equal(mutations[1].body.autoPolicy.replayTokenLimit, 12000);
});

test('historical start is a Beijing date-time control and persists its explicit +08:00 instant', async () => {
  const { app, context, fixture, requests, get } = setup({
    dailySettings: {
      schedule: {
        revision: 3,
        configRef: 'config-ref',
        sourceLimit: 100,
        enabled: false,
        autoPolicy: {
          continuousNew: true, historicalEnabled: false, historyStart: '2026-08-31T16:00:00Z',
          outdatedPolicy: 'disabled', unknownRetryMaxAttempts: 0, unknownRetryTokenLimit: 0,
          dayTokenLimit: 100000, replayTokenLimit: 0, semanticTokenLimit: 0,
        },
      },
    },
  });
  installResearchSettingsResponses(context, requests, fixture);
  await app.openResearchSettings();
  const html = get('ci-command-fields').innerHTML;
  assert.match(html, /历史起点（北京时间；仅在补齐历史时使用）/);
  assert.match(html, /name="historyStart" type="datetime-local" step="60" value="2026-09-01T00:00"/);

  await app.runCommand(researchSettingsForm({ values: { historyStart: '2026-10-02T03:04' } }));
  const schedule = requests.find((request) => request.url === '/api/local/comment-research/daily/schedule');
  assert.equal(schedule.body.autoPolicy.historyStart, '2026-10-02T03:04:00+08:00');
});

test('research settings reports a partial save without discarding the current form values', async () => {
  const { app, context, fixture, requests } = setup();
  installResearchSettingsResponses(context, requests, fixture);
  await app.openResearchSettings();
  context.window.CommentDaily.saveSchedule = async (body) => {
    requests.push({ url: '/api/local/comment-research/daily/schedule', body });
    throw new Error('日程修订冲突');
  };
  const form = researchSettingsForm();
  await assert.rejects(app.runCommand(form), /上下文已保存；其余设置未完成：日程修订冲突。请保留当前输入/);
  assert.equal(form.get('dayTokenLimit'), '100000');
  assert.deepEqual(requests.filter((request) => request.body).map((request) => request.url), [
    '/api/local/comment-research/daily/context-settings',
    '/api/local/comment-research/daily/schedule',
  ]);
});

test('automatic coverage exposes the shared daily comment slots and distinct retry accounting', () => {
  const { app, fixture, get } = setup();
  fixture.automaticResearch = {
    scope: 'entire_own_domain',
    schedule: { enabled: true, sourceLimit: 240, policy: { continuousNew: true, historicalEnabled: true, historyStart: '2026-09-01T00:00:00+08:00', outdatedPolicy: 'current_only' } },
    queue: { newIntake: 11, recovery: 2, historical: 18 },
    usage: { admittedComments: 72, calls: 9, chargedTokens: 32000, unknownUsageCalls: 1, unknownRecoveryTokens: 1200, replayTokens: 4000, semanticTokens: 3000 },
    unresearchedHistory: 18,
    accounting: '合成账本说明。',
  };
  app.state.view = 'overview';
  app.render();
  const html = get('results').innerHTML;
  assert.match(html, /今日自动研究上限<\/dt><dd>72 \/ 240 条/);
  assert.match(html, /同一评论重试不会重复占用评论槽位/);
  assert.match(html, /手动指定范围遵守自己的授权与总 Token 上限/);
  assert.match(html, /整个自有领域/);
  assert.match(html, /尚无已分析的代表原声/);
  assert.match(html, /data-ci="voices"[^>]*>查看全部原声/);
  assert.doesNotMatch(html, /&lt;button/);
});

test('first research is a compact normal task state and failure reason is rendered once', () => {
  const { app } = setup();
  const source = { sourceRef: 'first', body: '尚未研究的合成原声', workTitle: '合成作品', resultState: 'unstudied', executionState: 'idle', reasonCode: 'first_research' };
  const firstResearch = app.voiceTable([source]);
  assert.match(firstResearch, /结果 · 未研究/);
  assert.match(firstResearch, /执行 · 尚未排队/);
  assert.doesNotMatch(firstResearch, /此项未通过校验|打开详情核对具体原因/);
  const failed = app.voiceTable([{ ...source, executionState: 'failed', reasonCode: 'provider_failed' }]);
  assert.equal([...failed.matchAll(/供应商未交付完整结果/g)].length, 1);
});

test('voice table emits six aligned header and data cells before narrow-column hiding', () => {
  const { app } = setup();
  const html = app.voiceTable([{ sourceRef: 'aligned', body: '合成原声', workTitle: '合成作品' }]);
  const header = html.match(/<thead><tr>(.*?)<\/tr><\/thead>/s)?.[1] || '';
  const row = html.match(/<tbody>(.*?)<\/tbody>/s)?.[1] || '';
  const count = (markup, tag) => (markup.match(new RegExp(`<${tag}\\b`, 'g')) || []).length;
  assert.equal(count(header, 'th'), 6);
  assert.equal(count(row, 'td'), 6);
  assert.equal((row.match(/<\/td>/g) || []).length, 6);
  assert.match(header, /<th class="ci-col-tags">研究结果与执行<\/th>/);
});

test('candidate editor presents Chinese field and example controls instead of pipe-delimited input', async () => {
  const { app, context, fixture, get } = setup();
  installResearchSettingsResponses(context, [], fixture);
  await app.openResearchSettings();
  await app.handle('create-rule-candidate', { dataset: {} });
  const html = get('ci-command-fields').innerHTML;
  assert.match(html, /研究字段/);
  assert.match(html, /字段名称/);
  assert.match(html, /新增字段/);
  assert.match(html, /正反例/);
  assert.match(html, />算作<\/option>/);
  assert.match(html, /新增例子/);
  assert.doesNotMatch(html, /每行：字段类型 \| 名称 \| 定义/);
});

test('candidate editor converts visible controls into the rule DTO only when saving a draft', async () => {
  const { app, context, fixture, requests, get } = setup();
  installResearchSettingsResponses(context, requests, fixture);
  const baseFetch = context.fetch;
  context.fetch = async (url, options = {}) => {
    if (url === '/api/local/comment-intelligence/rules') {
      requests.push({ url, body: options.body && JSON.parse(options.body) });
      return { ok: true, json: async () => ({ ruleRevisionRef: 'new-candidate' }) };
    }
    return baseFetch(url, options);
  };
  await app.openResearchSettings();
  await app.handle('create-rule-candidate', { dataset: {} });
  const row = (values) => ({ querySelector: (selector) => ({ value: values[selector] }) });
  get('research-form-dialog').querySelectorAll = (selector) => {
    if (selector === '[data-rule-field]') return [row({ '[data-rule-field-kind]': 'need', '[data-rule-field-name]': '明确需求', '[data-rule-field-definition]': '用户明确要求具体帮助。' })];
    if (selector === '[data-rule-example]') return [row({ '[data-rule-example-field]': 'need', '[data-rule-example-polarity]': 'negative', '[data-rule-example-comment]': '今天下雨。', '[data-rule-example-expected]': '不算需求。' })];
    return [];
  };
  await app.runCommand({ get: (key) => ({ title: '候选方向', instruction: '保留真实边界。' })[key] });
  const mutation = requests.find((request) => request.url === '/api/local/comment-intelligence/rules');
  assert.deepEqual(mutation.body.fieldDefinitions, [{ kind: 'need', name: '明确需求', definition: '用户明确要求具体帮助。' }]);
  assert.deepEqual(mutation.body.examples, [{ field: 'need', polarity: 'negative', comment: '今天下雨。', expected: '不算需求。' }]);
});

test('missing automatic-upgrade candidate is rejected before any setting write', async () => {
  const { app, context, fixture, requests } = setup();
  installResearchSettingsResponses(context, requests, fixture);
  await app.openResearchSettings();
  await assert.rejects(
    app.runCommand(researchSettingsForm({ checked: ['continuousNew', 'workBody', 'ocr', 'parent', 'existingProblems', 'upgradeEnabled'] })),
    /启用自动采用前必须选择一个已保存候选规则/,
  );
  assert.equal(requests.filter((request) => request.body).length, 0);
});

test('replay detail renders object summaries as explicit safe fields and nested usage', async () => {
  const { app, context, get } = setup();
  context.fetch = async () => ({ ok: true, json: async () => ({
    run: { state: 'succeeded', comparison: null },
    sampleSet: { frozenCount: 1, selectorVersion: 'selector.v5', memberHash: 'member-hash' },
    items: [{ memberOrdinal: 1, repeatOrdinal: 0, side: 'candidate', state: 'succeeded', structureAccepted: true, summary: { normalization: 'json_fence', structureAccepted: true, noSignal: false, invariantViolations: [], validation: [{ code: 'field_schema_invalid' }] }, usage: { inputTokens: 120, outputTokens: 42 }, elapsedMs: 91 }],
  }) });
  await app.openReplayDetail('replay-ref');
  const html = get('ci-command-fields').innerHTML;
  assert.match(html, /输出封装/);
  assert.match(html, /json_fence/);
  assert.match(html, /校验诊断/);
  assert.match(html, /1 项（不展示原始返回）/);
  assert.match(html, /输入 120 \/ 输出 42 Token/);
  assert.doesNotMatch(html, /\[object Object\]/);
});

test('shadow creation reads the run envelope and presents insufficient evidence without a provider call', async () => {
  const { app, context, fixture, requests, get } = setup();
  installResearchSettingsResponses(context, requests, fixture);
  const baseFetch = context.fetch;
  context.fetch = async (url, options = {}) => {
    if (url === '/api/local/comment-intelligence/replays') {
      requests.push({ url, body: options.body && JSON.parse(options.body) });
      return { ok: true, json: async () => ({ state: 'insufficient_evidence', reason: '合成样本不足' }) };
    }
    return baseFetch(url, options);
  };
  await app.openResearchSettings();
  get('research-form-dialog').querySelector = (selector) => {
    if (selector === '#ci-upgrade-candidate') return { value: 'candidate-rule' };
    if (selector === '#ci-replay-budget') return { value: '12000' };
    return null;
  };
  await app.handle('start-replay', { dataset: {} });
  const mutation = requests.find((request) => request.url === '/api/local/comment-intelligence/replays');
  assert.deepEqual(mutation.body, {
    runRef: 'command-ref', candidateRuleRevisionRef: 'candidate-rule', configRef: 'config-ref', authorizedTokenBudget: 12000,
  });
  assert.equal(get('ci-command-feedback').textContent, '合成样本不足');
  assert.equal(requests.filter((request) => request.url.includes('provider')).length, 0);
});

test('available trace shows escaped actual content and field-level failures, not invented thinking', async () => {
  const { app, context, get } = setup();
  app.setBatch({ calls: [{ invocationRef: 'i1', modelId: 'synthetic-model', requestedComments: 1, state: 'succeeded' }] });
  context.fetch = async () => ({ ok: true, json: async () => ({
    availability: 'AVAILABLE', policy: { workBody: false, parent: true },
    input: { contract: 'comment-research.v3', system: '合成约束', work: {title:'合成标题',body:'合成正文',mediaTexts:[{kind:'ocr',text:'图片中的已提取文字'},{kind:'asr',text:'音频中的已转录文字'}]}, comments: [{ commentRef: 'C001', text: '<img onerror=alert(1)>', parent: null }], existingProblems: [{candidateRef:'P001',definition:{name:'真实结构问题名',meaning:'真实结构边界'}}] },
    output: { text: '{"comments":[]}', received: true, truncated: true },
    events: [{ kind: 'request_started', at: '2026-09-08T01:00:00Z' }],
    validation: [{ commentRef: 'C001', code: 'missing_comment', path: '$.comments', expected: 'C001', actual: 'missing' }],
  }) });
  await app.openRequest('b1', 'i1');
  const html = get('ci-command-fields').innerHTML;
  assert.ok(html.includes('synthetic-model'));
  assert.ok(html.includes('未纳入 · 作品正文'));
  assert.ok(html.includes('&lt;img onerror=alert(1)&gt;'));
  assert.ok(!html.includes('<img onerror='));
  assert.ok(html.includes('模型遗漏了这条评论'));
  assert.ok(html.includes('$.comments'));
  assert.ok(html.includes('开始调用模型'));
  assert.ok(html.includes('思考关闭'));
  assert.ok(html.includes('真实结构问题名'));
  assert.ok(html.includes('真实结构边界'));
  assert.ok(html.includes('展示已截短，校验使用原始完整返回'));
  assert.ok(html.includes('查看脱敏返回'));
  const workSection = html.split('<summary>作品内容</summary>')[1].split('</details>')[0];
  assert.ok(workSection.includes('图片中的已提取文字'));
  assert.ok(workSection.includes('音视频转录'));
  assert.ok(workSection.includes('音频中的已转录文字'));
  assert.ok(!workSection.includes('mediaTexts'));
  assert.ok(!workSection.includes('&quot;kind&quot;'));
  assert.ok(!workSection.includes('<pre>'));
});


test('daily navigation drops hidden voice filters and explicit return restores the voice scope', async () => {
  const { app, fixture } = setup();
  app.state.text = '旧检索'; app.state.lenses = 'need'; app.state.processingState = 'failed'; app.state.sourceRefs = 'source-1';
  fixture.daily.items = [{ batchRef: 'newest', total: 50, works: 11, counts: {} }];
  await app.navigate({ view: 'daily', batchRef: '' });
  for (const key of ['text', 'lenses', 'processingState', 'sourceRefs']) assert.equal(app.state[key], '');
  await app.handle('voices', {});
  assert.equal(app.state.text, '旧检索');
  assert.equal(app.state.lenses, 'need');
  assert.equal(app.state.processingState, 'failed');
  assert.equal(app.state.sourceRefs, 'source-1');
  assert.equal(app.state.batchRef, '');
});

test('direct daily URL clears hidden filters while preserving explicit batch', () => {
  const { app } = setup({}, '?view=daily&batchRef=batch-explicit&text=old&lenses=need&sourceRefs=source-1&processingState=failed&bookmarkedOnly=true');
  assert.equal(app.state.batchRef, 'batch-explicit');
  assert.equal(app.state.text, '');
  assert.equal(app.state.lenses, '');
  assert.equal(app.state.sourceRefs, '');
  assert.equal(app.state.processingState, '');
  assert.equal(app.state.bookmarkedOnly, false);
});


test('changing page size blocks selection and research until the matching result arrives', async () => {
  const { app, fixture, context, get } = setup();
  app.state.limit = 20;
  fixture.page.items = comments(0, 20);
  const control = get('synthetic-select-page');
  control.disabled = false;
  context.document.querySelectorAll = (selector) => selector.includes('[data-ci-select-page]') ? [control] : [];
  let release;
  let calls = 0;
  context.fetch = async () => {
    calls++;
    await new Promise((resolve) => { release = resolve; });
    return { ok: true, json: async () => structuredClone(fixture) };
  };
  const pending = app.navigate({ limit: 50 });
  assert.equal(app.state.limit, 50);
  assert.equal(control.disabled, true);
  await app.handle('select-page', {});
  await app.handle('prepare-selected', {});
  assert.equal(app.selected.size, 0, 'old 20-row result must not be selected under new page size');
  assert.equal(calls, 1, 'no research request may be prepared during the mismatch');
  assert.match(get('feedback').textContent, /当前范围尚未读取完成/);
  fixture.page.items = comments(0, 50);
  release();
  await pending;
  assert.equal(control.disabled, false);
  await app.handle('select-page', {});
  assert.equal(app.selected.size, 50);
});
