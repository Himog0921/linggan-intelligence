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
  const window = { addEventListener() {}, CommentDaily: { onRefresh() {}, openSettings() {} } };
  const context = {
    window, document: { querySelector: (q) => q === '[data-initial-view]' ? main : null, querySelectorAll: () => [], getElementById: get, createElement: () => new Element(), addEventListener: (k, cb) => listeners.set(k, cb), visibilityState: 'visible', body: { dataset: { corpusDomain: 'adhd' }, append() {} } },
    location: { search, pathname: '/corpus/comments' }, history: { replaceState() {}, pushState() {} },
    URLSearchParams, ResizeObserver: class { observe() {} }, AbortController, setTimeout: () => 1, clearTimeout() {},
    fetch: async (url, options) => {
      requests.push({ url, body: options.body && JSON.parse(options.body) });
      return { ok: true, json: async () => structuredClone(fixture) };
    }, crypto: { randomUUID: () => 'command-ref' },
  };
  const instrumented = original.replace(/  load\(\)\.then\(\(\) => \{\n    if \(params.get\("source"\)\) openSource\(params.get\("source"\)\);\n  \}\);/, '  window.test = { state, selected, selectedSources, selectSource, navigate, render, renderOverview, renderPagination, renderSelection, renderProblems, callTable, callPurpose, fieldRepairInputHtml, fieldRepairReceiptHtml, continuityReadout, renderDaily, renderChanges, candidateList, observations, cleaningHtml, semanticNotice, commentContextHtml, relationExpressionHtml, automationSummary, semanticOrganizationSummary, semanticOrganizationList, openJudgment, openJudgmentDecision, clearSelectionForScope, handle, openRequest, openContextSettings, openPrepare, runCommand: (f) => command(f), setData: (v) => { data = v; }, setBatch: (v) => { batchDetail = v; } };');
  assert.notEqual(instrumented, original, 'test instrumentation must locate startup without changing product logic');
  vm.runInNewContext(instrumented, context);
  window.test.setData(fixture);
  return { app: window.test, nodes, get, fixture, requests, context, listeners };
}

test('primary navigation is research focused and daily execution remains an auxiliary entry', () => {
  const { get } = setup();
  const html = get('main').innerHTML;
  const navigation = html.split('<nav class="lgi-research-tabs"')[1].split('</nav>')[0];
  assert.match(navigation, /data-view="changes"/);
  assert.match(navigation, /变化观察/);
  assert.doesNotMatch(navigation, /data-view="daily"|data-view="runs"/);
  assert.match(html, /data-ci="runs"/);
});

test('changes keeps its date range and cannot inherit an execution batch', async () => {
  const { app, requests } = setup({}, '?view=runs&batchRef=b1&from=2026-09-01T00:00:00Z&to=2026-09-08T00:00:00Z');
  await app.handle('changes', { dataset: {} });
  assert.equal(app.state.view, 'changes');
  assert.equal(app.state.batchRef, '');
  assert.ok(requests.at(-1).url.includes('view=changes'));
  assert.ok(!requests.at(-1).url.includes('batchRef='));
  assert.equal(app.state.from, '2026-09-01T00:00:00Z');
});

test('unknown problem accounting stays unknown and an unconfigured relation model is visible', () => {
  const { app, fixture, get } = setup();
  fixture.problemAutomation = { status: 'NOT_CONFIGURED', needsJudgment: [] };
  app.renderProblems();
  const html = get('results').innerHTML;
  assert.match(html, /问题归并尚未配置/);
  assert.match(html, /<dt>稳定问题<\/dt><dd>—<\/dd>/);
  assert.match(html, /不需要逐条确认/);
});

test('problem counts distinguish expressions, unresolved boundaries and development stages', () => {
  const { app, fixture, get } = setup();
  fixture.problemSummary = { analyzedComments: 26, expressionCount: 16, unmergedExpressions: 5, needsJudgment: 1, emergingProblems: 2, stableProblems: 1, unclassifiedProblems: 3 };
  fixture.problems = [{ problemRef: 'p', name: '合成问题', comments: 2, works: 1, lifecycle: 'emerging' }];
  app.renderProblems();
  const html = get('results').innerHTML;
  assert.match(html, /<dt>问题表达<\/dt><dd>16<\/dd>/);
  assert.match(html, /<dt>待自动复核<\/dt><dd>1<\/dd>/);
  assert.match(html, /<span class="ci-tag">新兴问题<\/span>/);
  assert.match(html, /3 个历史问题尚未标定发展阶段/);
});

test('semantic organization keeps accepted expressions visible before a stable cluster exists', () => {
  const { app, fixture, get } = setup();
  fixture.semanticOrganization = {
    atomCount: 9,
    unclusteredCount: 2,
    byKind: { problem: 4, need: 3, solution: 2 },
    unclustered: [
      { atomRef: 'a1', kind: 'need', meaning: '需要更容易坚持的奖励表', sourceRef: 'source-1', reason: 'below_min_common_core' },
      { atomRef: 'a2', kind: 'story', meaning: '晚间陪写作业的经历', sourceRef: 'source-2', reason: 'missing_embedding' },
    ],
    scope: 'current_filtered_accepted_results',
    qualityMeaning: '稳定性不等于语义正确率；未通过组织条件的表达仍可查看原声。',
  };
  app.renderOverview();
  assert.match(get('results').innerHTML, /已经理解/);
  assert.match(get('results').innerHTML, /等待整理/);
  app.renderProblems();
  const html = get('results').innerHTML;
  assert.match(html, /已经理解、等待整理的表达/);
  assert.match(html, /需求 3/);
  assert.match(html, /共同核心不足，暂不归并/);
  assert.match(html, /等待语义向量准备/);
  assert.match(html, /data-ci="source"/);
  assert.match(html, /稳定性只决定是否归并，不抹去已经接纳的表达/);
});

test('semantic organization makes each real budget blocker distinct from waiting for a vector', () => {
  const { app, fixture } = setup();
  fixture.semanticOrganization = {
    atomCount: 3,
    unclusteredCount: 3,
    byKind: { need: 3 },
    unclustered: [
      { atomRef: 'a1', kind: 'need', meaning: '预算受限表达一', reason: 'purpose_budget_exhausted' },
      { atomRef: 'a2', kind: 'need', meaning: '预算受限表达二', reason: 'day_budget_exhausted' },
      { atomRef: 'a3', kind: 'need', meaning: '预算受限表达三', reason: 'unknown_recovery_budget_exhausted' },
    ],
  };
  const html = app.semanticOrganizationList();
  for (const reason of ['语义整理额度不足，请检查研究设置', '今日总额度已用完，等待恢复', '未知用量恢复额度不足'])
    assert.match(html, new RegExp(reason));
  assert.doesNotMatch(html, /等待语义向量准备/);
});

test('problem details disclose related work topics without classifying the comment into a topic', () => {
  const { app, fixture, get } = setup();
  fixture.problem = { problemRef: 'p1', name: '合成问题', meaning: '需要持续的陪伴安排', revision: 1 };
  fixture.relatedTopics = [{ topicRef: 'topic-1', definitionRef: 'definition-1', definitionVersion: 2, name: '亲子晚间协作', canonicalKey: 'synthetic-topic', works: 2, comments: 5, associationRefs: ['assoc-1'], relation: 'source_work_member', meaning: '围绕晚间陪伴与家庭协作的既有作品主题' }];
  app.renderProblems();
  const html = get('results').innerHTML;
  assert.match(html, /相关作品所属主题 · 1/);
  assert.match(html, /亲子晚间协作/);
  assert.match(html, /围绕晚间陪伴与家庭协作的既有作品主题/);
  assert.match(html, /不表示这条评论已被正式分类到主题/);
  assert.doesNotMatch(html, /评论已归类到主题/);
});

test('actual per-comment context fragments are rendered without inventing missing work text', () => {
  const { app } = setup();
  const html = app.commentContextHtml({ context: { selectorVersion: 'test.v1', fragments: [{ fragmentRef: 'F001', kind: 'ocr_text', text: '<img onerror=alert(1)>图片方法' }, { fragmentRef: 'F002', kind: 'parent', text: '父评论真实片段' }] } });
  assert.match(html, /图片文字片段/);
  assert.match(html, /父评论真实片段/);
  assert.match(html, /&lt;img/);
  assert.doesNotMatch(html, /<img/);
  assert.match(app.commentContextHtml({ context: { fragments: [] } }), /没有选出可用片段/);
});

test('cleaning explains current derived text and deterministic noise without exposing raw codes as labels', () => {
  const { app } = setup();
  const html = app.cleaningHtml({ version: 'comment-clean.v2', state: 'context', text: '有效果吗?', reasons: ['emoji_removed', 'mention_removed', 'context_dependent'] });
  assert.match(html, /移除表情符号/);
  assert.match(html, /移除可确定边界的提及/);
  assert.match(html, /有效果吗\?/);
  assert.doesNotMatch(html, /emoji_removed/);
  assert.match(app.cleaningHtml(null), /尚未提供/);
});

test('partial acceptance and uncertainty do not imply no signal or whole-comment failure', () => {
  const { app } = setup();
  const partial = app.semanticNotice({ semantic: { acceptance: 'partial', outcome: 'interpretable', rejectedFields: [{ path: 'labels[1]', code: 'field_schema_invalid' }] } });
  assert.match(partial, /部分结果已保留/);
  assert.match(partial, /labels\[1\]/);
  assert.doesNotMatch(partial, /未提取到研究信号/);
  const uncertain = app.semanticNotice({ semantic: { acceptance: 'complete', outcome: 'uncertain', uncertaintyReason: '指代对象缺失' } });
  assert.match(uncertain, /含义尚不确定/);
  assert.match(uncertain, /指代对象缺失/);
});

test('boundary decisions submit actual candidate and problem identities with both revisions', async () => {
  const { app, fixture, requests } = setup();
  fixture.problemAutomation = { needsJudgment: [{ candidateRef: 'candidate-uuid', name: '问题表达', meaning: '边界', revision: 2, sourceRefs: [], targets: [{ problemRef: 'problem-uuid', name: '已有问题', meaning: '定义', definitionRevision: 7 }] }] };
  app.openJudgmentDecision('candidate-uuid');
  const values = new Map([['targetRef', 'problem-uuid'], ['relation', 'different'], ['reason', '对象不同，保持边界']]);
  await app.runCommand({ get: (k) => values.get(k) });
  const mutation = requests.find((r) => r.body?.kind === 'candidate_resolve').body;
  assert.equal(mutation.expectedRevision, 2);
  assert.deepEqual(mutation.payload, { candidateRef: 'candidate-uuid', targetRef: 'problem-uuid', targetDefinitionRevision: 7, relation: 'different' });
});

test('judgment queue includes uncertain boundaries only, ordinary candidates stay separate', () => {
  const { app, fixture, get } = setup();
  fixture.problemCandidates = [{ candidateRef: 'ordinary', name: 'SYNTHETIC尚未归并的独立表达' }];
  fixture.problemAutomation = { needsJudgment: [] };
  app.openJudgment();
  assert.match(get('ci-command-fields').innerHTML, /当前没有需要你判断/);
  assert.doesNotMatch(get('ci-command-fields').innerHTML, /SYNTHETIC尚未归并的独立表达/);
});

test('relation requests disclose their actual expression and evidence', () => {
  const { app } = setup();
  const html = app.relationExpressionHtml({ candidateRef: 'real-candidate', name: '提醒与时间', meaning: '无法持续提醒', comment: '我下班还要做饭 <script>', evidence: [{ quote: '下班还要做饭' }] });
  assert.match(html, /本次需要归并的问题表达/);
  assert.match(html, /real-candidate/);
  assert.match(html, /下班还要做饭/);
  assert.doesNotMatch(html, /<script>/);
});

test('research current results prepares all 300 selected-scope comments without silently taking 100', async () => {
  const { app, fixture, requests } = setup();
  fixture.page.total = 300;
  await app.openPrepare();
  assert.ok(requests.some(r => r.url.endsWith('/prepare')));
  assert.equal(requests.filter(r => r.url.includes('offset=50')).length, 0);
});

test('over-limit result scope offers an explicit small trial without sending prepare', async () => {
  const { app, fixture, requests, nodes } = setup();
  fixture.page.total = 3001;
  await app.openPrepare();
  assert.equal(requests.length, 0);
  const html = [...nodes.values()].map(n => n.innerHTML).join('');
  assert.match(html, /超过单次 3,000 条上限/);
  assert.match(html, /前 100 条快速试跑/);
});

test('relation request inspector renders Task B inputs and its own output contract', async () => {
  const { app, nodes } = setup({ purpose: 'problem_relation', availability: 'AVAILABLE', input: { contract: 'comment-relation.v1', system: '真实约束', expression: { name: '持续提醒', meaning: '时间不足', comment: '晚上还要做饭', candidateRef: 'candidate-real' }, existingProblems: [{ name: '家长时间', meaning: '不能全程陪伴', evidenceSamples: [{ quote: '无法一直陪伴' }], humanBoundaries: ['需要区分时间与知识'] }] } });
  await app.openRequest('batch-real', 'call-real');
  const html = [...nodes.values()].map(n => n.innerHTML).join('');
  assert.match(html, /本次需要归并的问题表达/);
  assert.match(html, /晚上还要做饭/);
  assert.match(html, /需要区分时间与知识/);
  assert.match(html, /输出判断相同、相关、不同或尚不确定/);
  assert.doesNotMatch(html, /评论与对应上下文 · .*条/);
});


test('accepted Task B recovery presents usage review without labelling analysis as failed', async () => {
  const { app, nodes } = setup({ problemAutomation: { status: 'ready', states: [{ state: 'needs_judgment', reason: 'usage_review_required', count: 1 }], needsJudgment: [] }, availability: 'AVAILABLE', purpose: 'problem_relation', validation: { accepted: true, usageReviewRequired: true }, events: [{ kind: 'accepted_result_recovered', at: '2026-09-08T00:00:00Z' }] });
  const call = { invocationRef: 'recovered-call', state: 'succeeded', purpose: 'problem_relation', failureCode: 'usage_review_required', usageUnknown: true };
  const table = app.callTable([call], 'real-batch');
  assert.match(table, /结果已接纳，用量待核对/);
  assert.doesNotMatch(table, /未接纳研究输出|此项未通过校验|ci-failure-text/);
  const summary = app.automationSummary();
  assert.match(summary, /结果已接纳，用量待核对；边界等待自动复核/);
  assert.doesNotMatch(summary, /处理失败/);
  app.setBatch({ calls: [call] });
  await app.openRequest('real-batch', 'recovered-call');
  const html = [...nodes.values()].map(n => n.innerHTML).join('');
  assert.match(html, /已恢复接纳结果/);
  assert.match(html, /结果已接纳，用量待核对/);
  assert.doesNotMatch(html, /此项未通过校验|ci-failure-text|不能据此判断全部通过/);
});

test('field repair uses its own label, retained input and supplement receipt without synthetic events', () => {
  const { app } = setup();
  const call = { invocationRef: 'repair-call', purpose: 'field_repair', state: 'partial', requestedComments: 1, requestedFields: 2, acceptedComments: 1, workTitle: '合成作品', modelId: 'synthetic' };
  const table = app.callTable([call], 'repair-batch');
  assert.match(table, /补齐未通过的字段/);
  assert.match(table, /2 个未通过字段 · 1 条评论/);
  assert.match(table, /补齐结果已写入补充分析/);
  const input = app.fieldRepairInputHtml({ contract: 'comment-field-repair.v1', selectedFields: ['labels[0]', 'evidence[0]'], comments: [{ commentRef: 'source-1', text: '真实保留的评论' }], system: '只补齐字段', policy: { context: 'frozen' }, outputSchema: { type: 'object' } });
  assert.match(input, /本次实际补齐范围/);
  assert.match(input, /labels\[0\]/);
  assert.match(input, /真实保留的评论/);
  assert.match(input, /只补齐字段/);
  assert.match(input, /输出结构/);
  assert.doesNotMatch(input, /已收到模型返回|开始调用模型/);
  const receipt = app.fieldRepairReceiptHtml({ baseAnalysisRef: 'base-analysis', supplementAnalysisRef: 'supplement-analysis', state: 'succeeded_partial', fields: ['labels[0]'] });
  assert.match(receipt, /字段补齐回执/);
  assert.match(receipt, /base-analysis/);
  assert.match(receipt, /supplement-analysis/);
  assert.match(receipt, /部分补齐结果已接纳/);
});

test('replay continuity is read-only and never presents stability as semantic accuracy', () => {
  const { app } = setup();
  const html = app.continuityReadout({
    stabilityNotAccuracy: true,
    followUps: [{ priorRunRef: 'prior-run', reason: 'required_new_samples', state: 'waiting_daily_budget', failureCode: 'replay_authorized_budget_exhausted', successorRunRef: null }],
    explanations: [{ runRef: 'explanation-run', state: 'succeeded', reservedTokens: 600, expiresAt: '2026-09-10T00:00:00Z', result: { stabilityNotAccuracy: true, summary: '只解释工程差异', uncertainty: '不比较语义正确性' } }],
    rollbackReceipts: [{ triggeringRunRef: 'trigger-run', candidateRuleRevisionRef: 'candidate-rule', restoredRuleRevisionRef: 'restored-rule', createdAt: '2026-09-09T00:00:00Z', healthEvidence: { state: 'degraded', completedPairs: 9, eligibleCount: 12 } }],
  });
  assert.match(html, /比较后续与回退记录 · 3/);
  assert.match(html, /等待当日共享额度/);
  assert.match(html, /只解释工程差异/);
  assert.match(html, /已恢复规则/);
  assert.match(html, /稳定性、结构和分布检查不等于语义准确率/);
  assert.doesNotMatch(html, /<button/);
  assert.doesNotMatch(html, /语义准确率已证明/);
});
