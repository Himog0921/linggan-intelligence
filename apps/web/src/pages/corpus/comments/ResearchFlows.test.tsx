import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { calls, comment, DOMAIN, installFetch, json, plan, problemRow, renderPage, WORK, writes } from './testKit';

afterEach(() => vi.unstubAllGlobals());
const TIME = '2026-11-20T01:00:00Z';
const run = (batchRef: string, extra: Record<string, unknown> = {}) => ({ batchRef, createdAt: TIME, state: 'open', unitCount: 2, cancelRequested: false, ...extra });
const runDetail = (batchRef: string, units: ({ state: string; failureCode?: string | null } & Record<string, unknown>)[], retry: unknown[] = []) => ({
 batchRef, cancelRequested: false, retrySelections: retry,
 units: units.map((unit, i) => ({ unitRef: `unit-${i}`, workRef: WORK, failureCode: null, targetCount: 1, ...unit })),
 targets: [{ state: 'succeeded', failureCode: null, count: 1 }], usage: { modelCalls: 2, inputTokens: 1200, outputTokens: 300 },
});

describe('B-04 研究记录对话框', () => {
 it('opens from the old view=runs link, shows "—" when the new run fields are missing, and expands unit detail', async () => {
  installFetch(url => {
   if (url.pathname.endsWith('/v2/runs')) return json({ domainRef: DOMAIN, runs: [run('batch-old')], consolidationTaskRevision: 1, consolidations: [] });
   if (url.pathname.endsWith('/v2/runs/batch-old')) return json(runDetail('batch-old', [{ state: 'running' }, { state: 'pending' }]));
   return undefined;
  });
  const { router } = renderPage(`domain=${DOMAIN}&view=runs&runRef=batch-old`);
  const dialog = await screen.findByRole('dialog', { name: '研究记录' });
  const row = (await within(dialog).findByText('BATCH-OL')).closest('tr')!;
  expect(within(row).getByText('进行中')).toBeInTheDocument();
  expect(within(row).getAllByText('—').length).toBeGreaterThanOrEqual(3);
  expect(within(row).getByText('2 个单元')).toBeInTheDocument();
  expect(await within(dialog).findByText('正在研究该作品的评论，结果生成后会更新。')).toBeInTheDocument();
  expect(within(dialog).getByText('等待执行，尚未生成本次研究结果。')).toBeInTheDocument();
  expect(screen.getByRole('button', { name: /^研究记录，1项进行中/ })).toHaveTextContent('1 项进行中');
  await userEvent.click(within(dialog).getByRole('button', { name: '关闭研究记录' }));
  await waitFor(() => expect(screen.queryByRole('dialog', { name: '研究记录' })).toBeNull());
  expect(new URLSearchParams(router.state.location.search).get('view')).toBe('overview');
 });

 it('uses unit counts and outcomes when present and retries failed units through the preview', async () => {
  const retry = [{ sourceRef: 'source-c1', observationRole: 'primary', workRef: WORK, commentExternalId: 'c1' }];
  installFetch(url => {
   if (url.pathname.endsWith('/v2/runs')) return json({ domainRef: DOMAIN, consolidationTaskRevision: 1, consolidations: [], runs: [
    run('batch-part', { state: 'completed', targetCount: 3, workCount: 2, unitCounts: { completed: 1, failed: 1, pending: 0 }, outcomes: { signals: 2, needsContext: 0, noSignal: 1 } })] });
   if (url.pathname.endsWith('/v2/runs/batch-part')) return json(runDetail('batch-part', [{ state: 'succeeded', workTitle: '合成作品标题', outcomes: { signals: 2, needsContext: 1, noSignal: 0 } }, { state: 'failed', failureCode: 'ai_provider_failed', workTitle: null, outcomes: null }], retry));
   if (url.pathname.endsWith('/v2/preview')) return json(plan());
   return undefined;
  });
  renderPage(`domain=${DOMAIN}&view=overview`);
  const user = userEvent.setup();
  await user.click(await screen.findByRole('button', { name: /^研究记录/ }));
  const dialog = await screen.findByRole('dialog', { name: '研究记录' });
  const row = (await within(dialog).findByText('BATCH-PA')).closest('tr')!;
  expect(within(row).getByText('部分失败')).toBeInTheDocument();
  expect(row).toHaveTextContent('3条评论'); expect(row).toHaveTextContent('2 篇作品 · 2 个单元');
  expect(row).toHaveTextContent('1 / 2 完成'); expect(row).toHaveTextContent('2有信号');
  expect(screen.getByRole('button', { name: /^研究记录，1项有失败/ })).toHaveTextContent('1 项有失败');
  await user.click(within(row).getByRole('button', { name: '展开批次 BATCH-PA' }));
  expect(await within(dialog).findByText('合成作品标题')).toBeInTheDocument();
  expect(within(dialog).getByText('有信号 2 · 需要语境 1 · 无信号 0')).toBeInTheDocument();
  expect(within(dialog).getByText(`作品 ${WORK.slice(0, 8).toUpperCase()}`, { exact: false })).toBeInTheDocument();
  await user.click(within(row).getByRole('button', { name: '重试批次 BATCH-PA 中失败的单元' }));
  const preview = await screen.findByRole('dialog', { name: '本次研究预览' });
  await within(preview).findByRole('button', { name: '提交 1 条研究' });
  expect(calls.find(call => call.url.pathname.endsWith('/v2/preview'))?.body?.selections).toEqual([{ workRef: WORK, sourceRef: 'source-c1', observationRole: 'primary' }]);
  expect(writes()).toEqual([]);
 });

 it('accepts the extra cancelled unit count from the backend and shows an all-cancelled batch as cancelled', async () => {
  installFetch(url => url.pathname.endsWith('/v2/runs') ? json({ domainRef: DOMAIN, consolidationTaskRevision: 1, consolidations: [], runs: [
   run('batch-cxl', { state: 'cancelled', targetCount: 2, workCount: 1, unitCounts: { completed: 0, failed: 0, pending: 0, cancelled: 2 }, outcomes: { signals: 0, needsContext: 0, noSignal: 0 } })] }) : undefined);
  renderPage(`domain=${DOMAIN}&view=overview&records=1`);
  const row = (await within(await screen.findByRole('dialog', { name: '研究记录' })).findByText('BATCH-CX')).closest('tr')!;
  expect(within(row).getByText('已取消')).toBeInTheDocument();
  expect(screen.getByRole('button', { name: /^研究记录$/ })).toBeInTheDocument();
 });

 it('keeps the #643 daily schedule usable inside the records dialog and previews a freshly re-read list', async () => {
  let enabled = false, autoSubmit = false, sourceRef = 'source-c1';
  installFetch((url, request) => {
   if (url.pathname.endsWith('/v2/automation')) {
    if (request.method === 'POST') { enabled = true; autoSubmit = JSON.parse(String(request.body)).autoSubmit === true; }
    const setting = { domainRef: DOMAIN, enabled, autoSubmit, minuteOfDay: 540, timeZone: 'Asia/Shanghai', selectionLimit: enabled ? 25 : 300, revision: enabled ? 1 : 0, enabledFrom: enabled ? TIME : null, nextRunAt: enabled ? TIME : null };
    if (request.method === 'POST') return json(setting);
    return json({ setting, nextBefore: null, selections: enabled ? [{ selectionRef: 'daily-1', scheduledAt: TIME, generatedAt: TIME, targetCount: 1, submissionCounts: autoSubmit ? { waiting_l1: 1 } : {} }] : [] });
   }
   if (url.pathname.endsWith('/automation/selections/daily-1')) return json({ domainRef: DOMAIN, selectionRef: 'daily-1', selections: [{ workRef: WORK, sourceRef, commentExternalId: 'c1', observationRole: 'primary' }] });
   return undefined;
  });
  renderPage(`domain=${DOMAIN}&view=runs`);
  const user = userEvent.setup();
  await user.click(await screen.findByRole('button', { name: '自动化设置' }));
  const settings = within(screen.getByRole('dialog', { name: '评论研究自动化设置' }));
  await user.click(await settings.findByRole('checkbox', { name: '启用定时生成清单' }));
  const limit = settings.getByRole('spinbutton', { name: '每份清单最多评论数' });
  await user.clear(limit); await user.type(limit, '25');
  await user.click(settings.getByRole('button', { name: '保存设置' }));
  expect((await screen.findAllByText('自动化设置已保存。')).length).toBeGreaterThan(0);
  expect(writes().map(call => call.url.pathname)).toEqual(['/api/local/comment-research/v2/automation']);
  expect(writes()[0]?.body).toMatchObject({ domainRef: DOMAIN, enabled: true, minuteOfDay: 540, selectionLimit: 25, expectedRevision: 0, timeZone: 'Asia/Shanghai', autoSubmit: false });
  await user.click(await screen.findByRole('button', { name: /预览清单/ }));
  const preview = await screen.findByRole('dialog', { name: '本次研究预览' });
  await within(preview).findByRole('button', { name: '提交 1 条研究' });
  await user.click(within(preview).getByRole('button', { name: '返回修改' }));
  sourceRef = 'source-c1-reobserved';
  await user.click(screen.getByRole('button', { name: /预览清单/ }));
  await within(await screen.findByRole('dialog', { name: '本次研究预览' })).findByRole('button', { name: '提交 1 条研究' });
  const previews = calls.filter(call => call.url.pathname.endsWith('/v2/preview'));
  expect(previews.at(-1)?.body?.selections).toEqual([{ workRef: WORK, sourceRef, observationRole: 'primary' }]);
  expect(calls.some(call => call.method === 'POST' && call.url.pathname.endsWith('/v2/runs'))).toBe(false);
  await user.click(within(screen.getByRole('dialog', { name: '本次研究预览' })).getByRole('button', { name: '返回修改' }));
  await user.click(screen.getByRole('button', { name: '自动化设置' }));
  const updatedSettings = within(screen.getByRole('dialog', { name: '评论研究自动化设置' }));
  await user.click(updatedSettings.getByRole('checkbox', { name: '自动提交符合条件的评论' }));
  await user.click(updatedSettings.getByRole('button', { name: '保存设置' }));
  expect(await screen.findByText('等待作品底稿 1 条')).toBeInTheDocument();
  expect(writes().at(-1)?.body).toMatchObject({ autoSubmit: true });

 });
});

describe('B-04 研究预览', () => {
 it('leads with what will be submitted, lists what will not, and lets the operator rerun researched comments', async () => {
  installFetch((url, request) => {
   if (url.pathname.endsWith('/v2/preview')) {
    const rerun = JSON.parse(String(request.body)).rerun;
    return json(plan(rerun ? { selectedCount: 2, researchableCount: 2, alreadyResearchedCount: 1 } : { selectedCount: 2, researchableCount: 1, alreadyResearchedCount: 1, alreadyQueuedCount: 0 }));
   }
   return undefined;
  });
  renderPage(`domain=${DOMAIN}&view=comments`); await screen.findByRole('table');
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: '选择评论' }));
  await user.click(screen.getAllByRole('checkbox', { name: '选择这条评论' })[0]!);
  await user.click(screen.getByRole('button', { name: /预览研究计划/ }));
  const dialog = await screen.findByRole('dialog', { name: '本次研究预览' });
  await within(dialog).findByText('按所属作品分组');
  expect(dialog.querySelector('strong')).toHaveTextContent('将提交 1 条评论 · 预计调用 1 次');
  expect(within(dialog).getByText('已选 2 条，其中 1 条不会提交')).toBeInTheDocument();
  expect(within(dialog).getByText('合成作品')).toBeInTheDocument();
  await user.click(within(dialog).getByRole('button', { name: '改为重新研究' }));
  await within(dialog).findByText(/其中 1 条已有研究结果：本次会生成新结果/);
  expect(calls.filter(call => call.url.pathname.endsWith('/v2/preview')).at(-1)?.body?.rerun).toBe(true);
  expect(within(dialog).getByRole('button', { name: '提交 2 条研究' })).toBeEnabled();
  expect(writes()).toEqual([]);
 });

 it('lists comments waiting for work context one by one and submits L1 only on request', async () => {
  installFetch(url => {
   if (url.pathname.endsWith('/v2/preview')) return json(plan({ selectedCount: 1, researchableCount: 0, unitCount: 0, expectedModelCalls: 0, workCount: 0, waitingL1: { targetCount: 1, workRefs: [WORK] } }));
   if (url.pathname === '/api/local/content-understanding/preview') return json({ domainRef: DOMAIN, workRef: WORK, inputHash: 'l1', knowledgeCutoff: TIME, taskConfigRef: 'cfg', taskRevision: 1, canStart: true, reasons: [] });
   if (url.pathname === '/api/local/content-understanding') return json({ requestRef: 'l1-request', reused: false });
   return undefined;
  });
  renderPage(`domain=${DOMAIN}&view=comments`); await screen.findByRole('table');
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: '选择评论' }));
  await user.click(screen.getAllByRole('checkbox', { name: '选择这条评论' })[0]!);
  await user.click(screen.getByRole('button', { name: /预览研究计划/ }));
  const dialog = await screen.findByRole('dialog', { name: '本次研究预览' });
  expect(await within(dialog).findByText('合成作品 · 作品语境未就绪')).toBeInTheDocument();
  expect(within(dialog).getByRole('button', { name: '开始研究' })).toBeDisabled();
  expect(writes()).toEqual([]);
  await user.click(within(dialog).getByRole('button', { name: '补充作品语境' }));
  expect(await within(dialog).findByText(/1 个作品已收到作品语境补充回执/)).toBeInTheDocument();
  expect(writes().map(call => call.url.pathname)).toEqual(['/api/local/content-understanding']);
 });
});

describe('B-04 研究预览 · 逐作品计划', () => {
 const OTHER_WORK = '00000000-0000-4000-8000-000000000004';
 it('shows per-work groups and lists each excluded comment with its reason and entry point', async () => {
  installFetch(url => url.pathname.endsWith('/v2/preview') ? json(plan({ selectedCount: 4, researchableCount: 1, workCount: 1, alreadyQueuedCount: 1, waitingL1: { targetCount: 1, workRefs: [OTHER_WORK] },
   excludedCounts: { creatorVoice: 1 },
   groups: [{ workRef: WORK, workTitle: '合成作品', selectedCount: 3, researchableCount: 1, unitCount: 1, alreadyResearchedCount: 2, waitingL1: false, activeCount: 1 },
    { workRef: OTHER_WORK, workTitle: '另一篇合成作品', selectedCount: 1, researchableCount: 0, unitCount: 0, alreadyResearchedCount: 0, waitingL1: true, activeCount: 0 }],
   excludedTargets: [{ workRef: WORK, commentExternalId: 'c1', reason: 'already_queued' }, { workRef: OTHER_WORK, commentExternalId: 'x9', reason: 'waiting_l1' }, { workRef: WORK, commentExternalId: 'c7', reason: 'creatorVoice' }],
   excludedTargetsTruncated: true })) : undefined);
  renderPage(`domain=${DOMAIN}&view=comments`); await screen.findByRole('table');
  const user = userEvent.setup();
  await user.click(screen.getByRole('button', { name: '选择评论' }));
  await user.click(screen.getAllByRole('checkbox', { name: '选择这条评论' })[0]!);
  await user.click(screen.getByRole('button', { name: /预览研究计划/ }));
  const dialog = await screen.findByRole('dialog', { name: '本次研究预览' });
  const first = (await within(dialog).findByText('将提交 1 条 · 1 个研究单元 · 已有结果 2 条 · 已选 3 条')).closest('div')!.parentElement!;
  expect(first).toHaveTextContent('合成作品'); expect(first).toHaveTextContent('可进入研究 · 1 条进行中');
  const second = within(dialog).getAllByText('另一篇合成作品')[0]!.closest('div')!.parentElement!;
  expect(second).toHaveTextContent('待作品语境');
  expect(within(second).getByRole('button', { name: '补充作品语境' })).toBeEnabled();
  expect(within(dialog).getByText('合成作品 · 正在研究，不会重复提交')).toBeInTheDocument();
  expect(within(dialog).getAllByText('SYNTHETIC 原声 <script>不执行</script>').length).toBe(1);
  const waiting = within(dialog).getByText('作品语境未就绪').closest('div')!.parentElement!;
  expect(waiting).toHaveTextContent('另一篇合成作品');
  expect(within(waiting).getByRole('button', { name: '补充作品语境' })).toBeInTheDocument();
  expect(within(dialog).getByText('创作者本人评论')).toBeInTheDocument();
  expect(within(dialog).getByText('仅列出前 200 条')).toBeInTheDocument();
  await user.click(within(dialog).getByRole('button', { name: '查看进行中' }));
  expect(await screen.findByRole('dialog', { name: '研究记录' })).toBeInTheDocument();
  expect(writes()).toEqual([]);
 });
});

describe('B-04 抽屉与用户问题', () => {
 const latest = { batchRef: 'batch-1', state: 'succeeded', failureCode: null, result: { requestRef: 'request-1', outcome: 'signals', reason: null, workContext: { summary: '合成作品语境' },
  signals: [{ kind: 'need', proposition: '需要外部提醒', quote: '原声', problem: { problemRef: 'problem-1', label: '合成问题：作业启动困难', state: 'formal', authorCount: 2 } }] } };
 const detail = (id = 'c1') => ({ domainRef: DOMAIN, commentKey: { workRef: WORK, commentExternalId: id }, comment: { ...comment(id), commentText: 'SYNTHETIC 原声文本' }, source: { sourceRef: 'source-' + id, sourceState: 'known', displayState: null }, work: { displayTitle: '合成作品', displayTitleSource: 'platform_title' }, parentContext: null });
 const problem = { ...problemRow(), actor: '家长', goal: '开始作业', barrier: '启动困难', context: null, redirectedFrom: null, history: [],
  evidence: [{ requestRef: 'request-1', workRef: WORK, workTitle: '合成作品', commentExternalId: 'c1', proposition: '需要外部提醒', quote: '原声' }, { requestRef: 'request-2', workRef: WORK, workTitle: '合成作品', commentExternalId: 'c2', proposition: '第二位作者', quote: '文本' }] };
 function routes(url: URL) {
  if (url.pathname.endsWith('/comments/detail')) return json(detail(url.searchParams.get('commentExternalId') ?? 'c1'));
  if (url.pathname.endsWith('/v2/latest')) return json(latest);
  if (url.pathname.endsWith('/v2/problems/problem-1')) return json(problem);
  if (url.pathname.endsWith('/v2/problems')) return json({ domainRef: DOMAIN, problems: [problemRow()], candidates: [] });
  return undefined;
 }

 it('highlights the evidence quote inside the comment and offers rerun as a secondary action', async () => {
  installFetch(routes);
  renderPage(`domain=${DOMAIN}&view=comments&detail=comment&commentWorkRef=${WORK}&commentExternalId=c1`);
  const drawer = await screen.findByRole('dialog', { name: '评论详情' });
  const mark = await within(drawer).findByText('原声', { selector: 'mark' });
  expect(mark.closest('blockquote')).toHaveTextContent('SYNTHETIC 原声文本');
  expect(within(drawer).getAllByText('原声', { selector: 'blockquote, mark' })).toHaveLength(1);
  expect(within(drawer).getByText('合成作品语境')).toBeInTheDocument();
  expect(within(drawer).getByText('重新研究会生成新结果；现有结果保留到新结果完成为止。')).toBeInTheDocument();
  expect(within(drawer).getByRole('button', { name: '重新研究' })).toHaveAttribute('data-variant', 'outline');
  await userEvent.click(within(drawer).getByRole('button', { name: /合成问题：作业启动困难/ }));
  const problemDrawer = await screen.findByRole('dialog', { name: '用户问题档案' });
  await userEvent.click(within(problemDrawer).getByRole('button', { name: '返回上一项' }));
  expect(await screen.findByRole('dialog', { name: '评论详情' })).toBeInTheDocument();
 });

 it('explains the formal threshold and shows every supporting comment in the comment table', async () => {
  installFetch(routes);
  const { router } = renderPage(`domain=${DOMAIN}&view=problems`);
  const card = (await screen.findByRole('heading', { name: '合成问题：作业启动困难' })).closest('article')!;
  expect(within(card).getByText('正式门槛 2 位')).toBeInTheDocument();
  expect(card.querySelector('[class*="_pa-card-desc_"]')).toBeNull();
  await userEvent.click(within(card).getByRole('button', { name: '打开合成问题：作业启动困难档案' }));
  const drawer = await screen.findByRole('dialog', { name: '用户问题档案' });
  expect(await within(drawer).findByText('已有 2 位独立作者支持，达到正式门槛（2 位）。')).toBeInTheDocument();
  await userEvent.click(within(drawer).getByRole('button', { name: /查看全部支持评论/ }));
  await waitFor(() => expect(new URLSearchParams(router.state.location.search).get('supportOf')).toBe('problem-1'));
  expect(await screen.findByText('支持问题：合成问题：作业启动困难')).toBeInTheDocument();
  const list = () => calls.filter(call => call.url.pathname.endsWith('/comment-study/comments')).at(-1)!.url.searchParams;
  await waitFor(() => expect(list().get('problemRef')).toBe('problem-1'));
  expect(calls.some(call => call.url.pathname.endsWith('/comments/detail'))).toBe(false);
 });

 it('filters the comment list by problemRef together with other filters and paging, replayable from the URL', async () => {
  installFetch(url => url.pathname.endsWith('/comment-study/comments')
   ? json({ domainRef: DOMAIN, items: [comment()], summary: { displayableCommentCount: 23, eligibleCommentCount: 23 }, indexCoverage: { indexedCount: 23, pendingCount: 0 }, page: { nextCursor: null, totalCount: 23 } })
   : routes(url));
  const { router } = renderPage(`domain=${DOMAIN}&view=comments&supportOf=problem-1&state=studied&page=2`);
  expect(await screen.findByText('支持问题：合成问题：作业启动困难')).toBeInTheDocument();
  const list = () => calls.filter(call => call.url.pathname.endsWith('/comment-study/comments')).at(-1)!.url.searchParams;
  await waitFor(() => expect(list().get('problemRef')).toBe('problem-1'));
  expect(list().get('studyState')).toBe('studied'); expect(list().get('offset')).toBe('10');
  expect(await screen.findByText('第 11–20 条 · 共 23 条')).toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: '第 3 页' }));
  await waitFor(() => expect(list().get('offset')).toBe('20'));
  expect(list().get('problemRef')).toBe('problem-1');
  await userEvent.click(screen.getByRole('button', { name: '取消问题筛选' }));
  await waitFor(() => expect(new URLSearchParams(router.state.location.search).get('supportOf')).toBeNull());
  expect(new URLSearchParams(router.state.location.search).get('state')).toBe('studied');
  await waitFor(() => expect(list().get('problemRef')).toBeNull());
 });

 it('shows the one-line summary only when the server provides it', async () => {
  installFetch(url => url.pathname.endsWith('/v2/problems') ? json({ domainRef: DOMAIN, problems: [problemRow('p1', { summary: '孩子知道要写作业，但难以开始。' }), problemRow('p2', { label: '候选问题', state: 'candidate', authorCount: 1, summary: null })], candidates: [] }) : undefined);
  renderPage(`domain=${DOMAIN}&view=problems`);
  expect(await screen.findByText('孩子知道要写作业，但难以开始。')).toBeInTheDocument();
  expect(screen.getByRole('combobox', { name: '问题状态' })).toHaveTextContent('全部问题 · 2');
  const candidate = screen.getByRole('heading', { name: '候选问题' }).closest('article')!;
  expect(candidate.querySelector('[class*="_pa-card-desc_"]')).toBeNull();
  expect(candidate.querySelectorAll('[class*="_filled_"]')).toHaveLength(1);
 });
});
