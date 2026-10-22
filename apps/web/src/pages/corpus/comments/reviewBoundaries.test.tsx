import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { calls, comment, DOMAIN, installFetch, json, OTHER, renderPage, WORK } from './testKit';
const RUN = '00000000-0000-4000-8000-000000000011', RUN2 = '00000000-0000-4000-8000-000000000012', PROBLEM = '00000000-0000-4000-8000-000000000013';
const run = (ref = RUN, name = 'SYNTHETIC 方法一') => ({ runRef: ref, state: 'stopped', methodName: name, createdAt: '2026-10-18T01:00:00Z' });
const detail = (ref = RUN, name = 'SYNTHETIC 方法一') => ({ run: { ...run(ref, name), method: { name }, scope: {}, semanticSummary: null, costSummary: null } });
const problem = (readable = true) => ({ problemRef: PROBLEM, title: 'SYNTHETIC 旧标题', definition: 'SYNTHETIC 旧定义', definitionReadable: readable, supportCommentCount: 1, supportAuthorCount: 1, supportWorkCount: 1, preview: { commentKey: { workRef: WORK, commentExternalId: 'c1' }, sourceState: 'known', commentText: 'SYNTHETIC 旧引文' } });

function runReply(url: URL) {
 if (url.pathname.endsWith('/runs')) return json({ domainRef: url.searchParams.get('domain'), runs: [run(), run(RUN2, 'SYNTHETIC 方法二')], page: { nextCursor: null } });
 if (url.pathname.endsWith('/targets')) return json({ runRef: url.searchParams.get('runRef'), targets: [], page: { nextCursor: null } });
 if (url.pathname.endsWith('/' + RUN)) return json(detail());
 if (url.pathname.endsWith('/' + RUN2)) return json(detail(RUN2, 'SYNTHETIC 方法二'));
 return undefined;
}
afterEach(() => vi.unstubAllGlobals());
describe('D3 review regression boundaries', () => {
 it.each(['not-a-domain', '00000000-0000-4000-8000-000000000099'])('does not propagate the server-rejected domain %s into corpus navigation', async rejected => {
  installFetch(url => url.pathname === '/api/local/corpus/shell' ? json({ domains: [{ domainRef: DOMAIN, name: '合成领域一', status: 'active', targetCount: 2, sampleCount: 3 }], selectedDomain: null, collectionState: '观察中' }) : undefined);
  renderPage(`domain=${rejected}`); await screen.findByText('请先在页头选择观察领域，再查看评论研究。');
  expect(document.querySelector('.v7-side a[href^="/corpus/evidence"]')).toHaveAttribute('href', '/corpus/evidence');
  expect(document.querySelector('.v7-side a[href^="/corpus/creators"]')).toHaveAttribute('href', '/corpus/creators');
  expect(calls.filter(c => c.url.pathname !== '/api/local/ai-settings').map(c => c.url.pathname)).toEqual(['/api/local/corpus/shell']);
  expect(calls.some(c => c.url.pathname === '/api/local/ai-settings')).toBe(false);
 });
 it('uses the server-resolved domain for corpus navigation', async () => {
  installFetch(url => url.pathname === '/api/local/corpus/shell' ? json({ domains: [{ domainRef: OTHER, name: '合成领域二', status: 'active', targetCount: 0, sampleCount: null }], selectedDomain: OTHER, collectionState: '观察中' }) : undefined);
  renderPage(`domain=${DOMAIN}`); await screen.findByLabelText('研究累计读数与昨日活动');
  expect(document.querySelector('.v7-side a[href^="/corpus/evidence"]')).toHaveAttribute('href', `/corpus/evidence?domain=${OTHER}`);
  expect(calls.find(c => c.url.pathname.endsWith('/overview'))?.url.searchParams.get('domain')).toBe(OTHER);
 });
 it('retries a failed clue source read without dismissing its drawer', async () => {
  const key = { workRef: WORK, commentExternalId: 'c1' }; let reads = 0;
  installFetch(url => url.pathname.endsWith('/problem-candidates') ? json({ expressions: [{ signalRef: 'signal', commentKey: key, sourceState: 'known', commentText: 'SYNTHETIC 原声', proposition: 'SYNTHETIC 表达', kind: 'need', state: 'deferred_novel', pairOutcomes: [] }], page: { nextCursor: null } }) : url.pathname.endsWith('/comments/detail') ? ++reads === 1 ? json({ error: 'unavailable' }, 503) : json({ commentKey: key, source: { sourceState: 'known' }, comment: { commentText: 'SYNTHETIC 原声' } }) : undefined);
  renderPage(`domain=${DOMAIN}&view=problems&state=clues`); await userEvent.click(await screen.findByRole('button', { name: '查看这条未归并表达' }));
  await screen.findByRole('heading', { name: '线索暂时不可读取' }); await userEvent.click(screen.getByRole('button', { name: '重新读取' }));
  const drawer = screen.getByRole('dialog'); expect(await within(drawer).findByRole('heading', { name: 'SYNTHETIC 表达' })).toBeInTheDocument(); expect(reads).toBe(2);
  expect(drawer).toHaveAttribute('open');
  expect(calls.filter(c => c.url.pathname.endsWith('/comments/detail')).every(c => c.method === 'GET' && c.url.searchParams.get('workRef') === WORK && c.url.searchParams.get('commentExternalId') === 'c1')).toBe(true);
 });
 it('initial overview reads only shell and overview, never AI settings or historical methods', async () => {
  installFetch(); renderPage(); await screen.findByLabelText('研究累计读数与昨日活动');
  expect(screen.getByRole('link', { name: /采集/ })).toHaveTextContent('观察中');
  expect([...new Set(calls.map(c => c.url.pathname))].sort()).toEqual(['/api/local/corpus/shell', '/api/local/comment-study/overview'].sort()); expect(calls.every(c => c.method === 'GET')).toBe(true);
 });
 it('reports shell failure distinctly and offers a working retry', async () => {
  let reads = 0; installFetch(url => url.pathname === '/api/local/corpus/shell' && ++reads === 1 ? json({ error: 'unavailable' }, 503) : undefined); renderPage();
  await screen.findByText(/评论研究准备信息读取失败/); expect(screen.queryByText(/请先在页头选择观察领域/)).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: '重试读取' })); await screen.findByLabelText('研究累计读数与昨日活动'); expect(reads).toBe(2);
 });
 it('retains cross-page comment selection and history cursor without a retired execution action', async () => {
  installFetch(url => url.pathname.endsWith('/comments') ? json({ domainRef: DOMAIN, items: [comment(url.searchParams.has('cursor') ? 'c2' : 'c1', true)], summary: { displayableCommentCount: 2, eligibleCommentCount: 2 }, indexCoverage: { indexedCount: 2, pendingCount: 0 }, page: { nextCursor: url.searchParams.has('cursor') ? null : 'page2' } }) : url.pathname.endsWith('/selection-preview') ? json({ targetCount: 2, requestedWorkCount: 1, coveredWorkCount: 1, scopeCommentCount: 2, exclusionCounts: {} }) : undefined);
  const { router } = renderPage(`domain=${DOMAIN}&view=comments`); await userEvent.click(await screen.findByRole('button', { name: '选择评论' })); await userEvent.click(screen.getByRole('checkbox', { name: '选择这条评论' })); await userEvent.click(screen.getByRole('button', { name: '下一页' }));
  await waitFor(() => expect(document.querySelector('input[data-comment-id="c2"]')).not.toBeNull()); await userEvent.click(screen.getByRole('checkbox', { name: '选择这条评论' })); expect(router.state.location.search).toContain('cursor=page2');
  await userEvent.click(screen.getByRole('button', { name: '上一页' })); await waitFor(() => expect(document.querySelector('input[data-comment-id="c1"]')).not.toBeNull()); expect(screen.getByRole('checkbox', { name: '选择这条评论' })).toBeChecked();
  await act(() => router.navigate(-1)); await waitFor(() => expect(document.querySelector('input[data-comment-id="c2"]')).not.toBeNull()); expect(screen.getByRole('checkbox', { name: '选择这条评论' })).toBeChecked();
  expect(screen.queryByRole('button', { name: '研究所选评论' })).not.toBeInTheDocument();
  expect(calls.every(c => c.method === 'GET')).toBe(true);
 });
 it('the overview new-expression metric opens matching unfiled clues', async () => {
  installFetch(url => url.pathname.endsWith('/problem-candidates') ? json({ expressions: [], page: { nextCursor: null } }) : undefined); const { router } = renderPage(); await screen.findByLabelText('研究累计读数与昨日活动');
  await userEvent.click(screen.getByRole('button', { name: /新表达待建档/ })); await waitFor(() => expect(calls.some(c => c.url.pathname.endsWith('/problem-candidates'))).toBe(true)); expect(router.state.location.search).toContain('state=clues'); expect(screen.getByRole('button', { name: '未归并线索' })).toHaveAttribute('aria-pressed', 'true');
 });
 it('a newly restricted detail also hides its stale list title, definition and quote', async () => {
  installFetch(url => url.pathname.endsWith('/problem-cards') ? json({ contract: 'comment-study.problem-cards.v1', domainRef: DOMAIN, problems: [problem()], totalCount: 1, page: { nextCursor: null } }) : url.pathname.endsWith('/problems/' + PROBLEM) ? json({ problem: problem(false), revisionHistory: [], sourceDistribution: null }) : url.pathname.endsWith('/evidence') ? json({ problemRef: PROBLEM, evidence: [], page: { nextCursor: null } }) : undefined);
  renderPage(`domain=${DOMAIN}&view=problems`); await userEvent.click(await screen.findByRole('button', { name: '查看问题：SYNTHETIC 旧标题' })); await screen.findByRole('heading', { name: '问题定义当前不可读取', level: 2 }); await waitFor(() => expect(screen.queryByText('SYNTHETIC 旧引文')).not.toBeInTheDocument());
  expect(screen.queryByText('SYNTHETIC 旧定义')).not.toBeInTheDocument(); await userEvent.click(screen.getByRole('button', { name: '关闭 ×' })); expect(screen.queryByText('SYNTHETIC 旧标题')).not.toBeInTheDocument();
 });
 it('a late work lookup cannot navigate back after leaving comments', async () => {
  let finish: (value: Response) => void = () => {}; installFetch(url => url.pathname.endsWith('/works') ? new Promise<Response>(resolve => { finish = resolve; }) : undefined);
  const { router } = renderPage(`domain=${DOMAIN}&view=comments`); await screen.findByRole('table'); await userEvent.click(screen.getByRole('combobox',{name:'搜索范围'})); await userEvent.click(screen.getByRole('option',{name:'作品标题'})); await userEvent.type(screen.getByRole('searchbox',{name:'字面搜索'}),'延迟'); await userEvent.click(screen.getByRole('button',{name:'筛选'})); await userEvent.click(screen.getByRole('button',{name:'完成'})); await userEvent.click(screen.getByRole('button', { name: '研究批次' }));
  await act(async () => { finish(json({ items: [], page: { nextCursor: null } })); await Promise.resolve(); }); expect(new URLSearchParams(router.state.location.search).get('view')).toBe('runs');
 });
 it('historical missing selection counts stay unknown and history open moves focus to the new detail', async () => {
  installFetch(runReply); renderPage(`domain=${DOMAIN}&view=runs`); await screen.findByRole('region', { name: '本次冻结范围' }); expect(screen.getByRole('region', { name: '本次冻结范围' })).toHaveTextContent('选中 — 篇');
  await userEvent.click(screen.getByText('查看全部研究运行（已加载 2 次）')); vi.mocked(HTMLElement.prototype.scrollIntoView).mockClear(); await userEvent.click(screen.getByRole('button', { name: '查看结果' })); await waitFor(() => expect(screen.getByRole('heading', { name: '本次研究 · SYNTHETIC 方法二' })).toHaveFocus()); expect(HTMLElement.prototype.scrollIntoView).toHaveBeenCalled();
 });
 it('does not describe a pending run detail as missing records', async () => {
  let finish: (value: Response) => void = () => {}; installFetch(url => url.pathname.endsWith('/' + RUN) ? new Promise<Response>(resolve => { finish = resolve; }) : runReply(url)); renderPage(`domain=${DOMAIN}&view=runs`);
  await screen.findByRole('heading', { name: '本次研究 · 正在读取方法…' }); expect(screen.getByText(/正在读取运行时间/)).toBeInTheDocument(); await act(async () => { finish(json(detail())); }); await screen.findByRole('region', { name: '本次运行摘要' });
 });
 it.each(['restricted', 'unavailable', 'unknown'])('never renders a %s request body even if the server includes it', async sourceState => {
  const request = { invocationRef: 'invocation', runRef: RUN, sourceState, recordingState: 'recorded', requestManifest: { message: 'SYNTHETIC 应隐藏的请求原文' } };
  installFetch(url => url.pathname.endsWith('/requests') ? json({ runRef: RUN, requests: [request], page: { nextCursor: null } }) : url.pathname.endsWith('/requests/invocation') ? json({ request }) : runReply(url)); renderPage(`domain=${DOMAIN}&view=runs&runRef=${RUN}&panel=requests`);
  await userEvent.click(await screen.findByRole('button', { name: '查看本次请求详情' })); await screen.findByText(/本次请求正文快照/); expect(screen.queryByText(/SYNTHETIC 应隐藏的请求原文/)).not.toBeInTheDocument();
 });
 it('a late old-domain run directory cannot replace the current domain list', async () => {
  let finish: (value: Response) => void = () => {}; installFetch(url => url.pathname.endsWith('/runs') && url.searchParams.get('domain') === DOMAIN ? new Promise<Response>(resolve => { finish = resolve; }) : url.pathname.endsWith('/runs') ? json({ domainRef: OTHER, runs: [run(RUN2, 'SYNTHETIC 新领域方法')], page: { nextCursor: null } }) : runReply(url));
  const { router } = renderPage(`domain=${DOMAIN}&view=runs`); await waitFor(() => expect(calls.some(c => c.url.pathname.endsWith('/runs'))).toBe(true)); await act(() => router.navigate(`/corpus/comments?domain=${OTHER}&view=runs`)); await userEvent.click(await screen.findByText('查看全部研究运行（已加载 1 次）')); await screen.findByText(/SYNTHETIC 新领域方法/);
  expect(calls.find(c => c.url.pathname.endsWith('/runs') && c.url.searchParams.get('domain') === DOMAIN)?.signal?.aborted).toBe(true); await act(async () => { finish(json({ domainRef: DOMAIN, runs: [run()], page: { nextCursor: null } })); await Promise.resolve(); }); expect(screen.queryByText(/SYNTHETIC 方法一/)).not.toBeInTheDocument();
 });
 it.each([['known',true],['restricted',false]] as const)('a %s clue applies current readability to its recorded author name',async(sourceState,readable)=>{
  const key={workRef:WORK,commentExternalId:'c1'};
  installFetch(url=>url.pathname.endsWith('/problem-candidates')?json({expressions:[{signalRef:'signal',commentKey:key,sourceState:'known',commentText:'SYNTHETIC 原声',proposition:'SYNTHETIC 表达',authorDisplayName:'SYNTHETIC 账号名',kind:'need',state:'deferred_novel',pairOutcomes:[]}],page:{nextCursor:null}}):url.pathname.endsWith('/comments/detail')?json({commentKey:key,source:{sourceState},comment:{commentText:'SYNTHETIC 原声'}}):undefined);
  renderPage(`domain=${DOMAIN}&view=problems&state=clues`);await userEvent.click(await screen.findByRole('button',{name:'查看这条未归并表达'}));if(readable)expect(await screen.findByText('SYNTHETIC 账号名')).toBeInTheDocument();else{await screen.findByRole('heading',{name:'来源当前不可读取'});expect(screen.queryByText('SYNTHETIC 账号名')).not.toBeInTheDocument();}
 });

 it('an older work search cannot overtake a newer pending search within the same route',async()=>{
  const finish=new Map<string,(value:Response)=>void>();installFetch(url=>url.pathname.endsWith('/works')?new Promise<Response>(resolve=>{finish.set(url.searchParams.get('q')||'',resolve);}):undefined);renderPage(`domain=${DOMAIN}&view=comments`);await screen.findByRole('table');
  await userEvent.click(screen.getByRole('combobox',{name:'搜索范围'}));await userEvent.click(screen.getByRole('option',{name:'作品标题'}));
  const input=screen.getByRole('searchbox',{name:'字面搜索'});await userEvent.type(input,'older');await userEvent.click(screen.getByRole('button',{name:'筛选'}));await userEvent.click(screen.getByRole('button',{name:'完成'}));await userEvent.clear(input);await userEvent.type(input,'newer');await userEvent.keyboard('{Enter}');
  await act(async()=>{finish.get('older')!(json({items:[{workRef:WORK,displayTitle:'SYNTHETIC 旧搜索'}],page:{nextCursor:null}}));await Promise.resolve();});
  await act(async()=>{finish.get('newer')!(json({items:[{workRef:WORK,displayTitle:'SYNTHETIC 新搜索'}],page:{nextCursor:null}}));});
  expect(await screen.findByRole('option',{name:'SYNTHETIC 新搜索'})).toBeInTheDocument();
  expect(screen.queryByRole('option',{name:'SYNTHETIC 旧搜索'})).not.toBeInTheDocument();
 });
 it('a work lookup cannot restore an old filter after the comment route changes',async()=>{
  let finish:(value:Response)=>void=()=>{};installFetch(url=>url.pathname.endsWith('/works')?new Promise<Response>(resolve=>{finish=resolve;}):undefined);const {router}=renderPage(`domain=${DOMAIN}&view=comments`);await screen.findByRole('table');await userEvent.click(screen.getByRole('combobox',{name:'搜索范围'}));await userEvent.click(screen.getByRole('option',{name:'作品标题'}));await userEvent.type(screen.getByRole('searchbox',{name:'字面搜索'}),'过期');await userEvent.click(screen.getByRole('button',{name:'筛选'}));await userEvent.click(screen.getByRole('button',{name:'完成'}));await userEvent.click(screen.getByRole('combobox',{name:'搜索范围'}));await userEvent.click(screen.getByRole('option',{name:'评论文本'}));await userEvent.clear(screen.getByRole('searchbox',{name:'字面搜索'}));await userEvent.type(screen.getByRole('searchbox',{name:'字面搜索'}),'new-filter');await userEvent.keyboard('{Enter}');await waitFor(()=>expect(router.state.location.search).toContain('q=new-filter'));
  await act(async()=>{finish(json({items:[{workRef:WORK,displayTitle:'SYNTHETIC 过期搜索'}],page:{nextCursor:null}}));await Promise.resolve();});expect(router.state.location.search).toContain('q=new-filter');expect(screen.queryByRole('option',{name:'SYNTHETIC 过期搜索'})).not.toBeInTheDocument();
 });

 it('keeps frozen method history in the run detail instead of global page chrome', async () => {
  installFetch(runReply); renderPage(); await screen.findByLabelText('研究累计读数与昨日活动');
  expect(screen.queryByText('查看历史方法版本')).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: '研究批次' }));
  await userEvent.click(await screen.findByRole('tab', { name: '本次方法' }));
  expect(await screen.findByText(/不会用当前默认方法补写这次运行/)).toBeInTheDocument();
  expect(calls.some(c => c.url.pathname.endsWith('/policies'))).toBe(false);
  expect(calls.every(c => c.method === 'GET')).toBe(true);
 });
 it('a failed background shell refresh preserves the current comment filter and selection without a write', async () => {
  let shellReads = 0;
  installFetch(url => url.pathname === '/api/local/corpus/shell' && ++shellReads > 1 ? json({ error: 'unavailable' }, 503) : undefined);
  const { queryClient } = renderPage(`domain=${DOMAIN}&view=comments&q=kept`);
  await screen.findByRole('table'); await userEvent.click(screen.getByRole('button', { name: '选择评论' })); await userEvent.click(screen.getAllByRole('checkbox', { name: '选择这条评论' })[0]!);
  await act(async () => { await queryClient.refetchQueries({ queryKey: ['corpus-shell', DOMAIN] }); });
  await screen.findByText(/领域信息刷新失败/);
  expect(screen.getByRole('searchbox', { name: '字面搜索' })).toHaveValue('kept');
  expect(screen.getAllByRole('checkbox', { name: '选择这条评论' })[0]).toBeChecked();
  expect(calls.every(c => c.method === 'GET')).toBe(true);
 });
 it('never invents missing frozen limits in run details', async () => {
  installFetch(runReply); renderPage(`domain=${DOMAIN}&view=runs`);
  await userEvent.click(await screen.findByRole('tab', { name: '本次方法' }));
  expect(await screen.findByText(/本次冻结上限：评论 未记录 条/)).toBeInTheDocument();
  expect(screen.queryByText(/默认评论上限 0/)).not.toBeInTheDocument();
 });

});
