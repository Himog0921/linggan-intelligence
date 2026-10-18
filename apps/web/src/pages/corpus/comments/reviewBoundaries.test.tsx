import { act, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { calls, comment, DOMAIN, installFetch, json, OTHER, POLICY, policy, renderPage, WORK } from './testKit';
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
 it('initial overview performs only shell and overview reads', async () => {
  installFetch(); renderPage(); await screen.findByLabelText('研究累计读数与昨日活动');
  expect(calls.map(c => c.url.pathname)).toEqual(['/api/local/corpus/shell', '/api/local/comment-study/overview']); expect(calls.every(c => c.method === 'GET')).toBe(true);
 });
 it('reports shell failure distinctly and offers a working retry', async () => {
  let reads = 0; installFetch(url => url.pathname === '/api/local/corpus/shell' && ++reads === 1 ? json({ error: 'unavailable' }, 503) : undefined); renderPage();
  await screen.findByText(/评论研究准备信息读取失败/); expect(screen.queryByText(/请先在页头选择观察领域/)).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: '重试读取' })); await screen.findByLabelText('研究累计读数与昨日活动'); expect(reads).toBe(2);
 });
 it('reopening retries setup after a failed read', async () => {
  let reads = 0; installFetch(url => url.pathname.endsWith('/setup') && ++reads === 1 ? json({ error: 'unavailable' }, 503) : undefined); renderPage(); await screen.findByLabelText('研究累计读数与昨日活动');
  await userEvent.click(screen.getByRole('button', { name: '发起研究' })); await screen.findAllByText(/无法读取准备信息/);
  await userEvent.click(screen.getByRole('button', { name: '关闭' })); await userEvent.click(screen.getByRole('button', { name: '发起研究' })); await screen.findByRole('checkbox', { name: '选择作品：合成作品' }); expect(reads).toBe(2);
 });
 it('finds the active method on a later directory page and uses its pointer for CAS', async () => {
  const newer = '00000000-0000-4000-8000-000000000014';
  installFetch(url => url.pathname.endsWith('/policies') ? json({ domainRef: DOMAIN, items: url.searchParams.has('cursor') ? [policy()] : [{ ...policy(newer), isActive: false }], page: { hasMore: !url.searchParams.has('cursor'), nextCursor: url.searchParams.has('cursor') ? null : 'older-page' } }) : url.pathname.endsWith('/activate') ? json({ domainRef: DOMAIN, policyRef: newer, isActive: true }) : undefined);
  renderPage(); await screen.findByLabelText('研究累计读数与昨日活动'); await userEvent.click(screen.getByRole('button', { name: '发起研究' })); const select = await screen.findByRole('combobox', { name: '本次使用的方法版本' }); expect(select).toHaveValue(POLICY);
  expect(calls.filter(c => c.url.pathname.endsWith('/policies')).map(c => c.url.searchParams.get('cursor'))).toEqual([null, 'older-page']);
  await userEvent.selectOptions(select, newer); await userEvent.click(screen.getByRole('button', { name: '设为默认方法' })); await screen.findByText('已将所选方法设为当前领域默认版本。'); expect(calls.find(c => c.url.pathname.endsWith('/activate'))?.body).toEqual({ expectedActivePolicyRef: POLICY });
 });
 it('retains cross-page comment selection, history cursor and the exact comments preview scope', async () => {
  installFetch(url => url.pathname.endsWith('/comments') ? json({ domainRef: DOMAIN, items: [comment(url.searchParams.has('cursor') ? 'c2' : 'c1', true)], summary: { displayableCommentCount: 2, eligibleCommentCount: 2 }, indexCoverage: { indexedCount: 2, pendingCount: 0 }, page: { nextCursor: url.searchParams.has('cursor') ? null : 'page2' } }) : url.pathname.endsWith('/selection-preview') ? json({ targetCount: 2, requestedWorkCount: 1, coveredWorkCount: 1, scopeCommentCount: 2, exclusionCounts: {} }) : undefined);
  const { router } = renderPage(`domain=${DOMAIN}&view=comments`); await userEvent.click(await screen.findByRole('checkbox', { name: '选择这条评论进入研究' })); await userEvent.click(screen.getByRole('button', { name: '下一页' }));
  await waitFor(() => expect(document.querySelector('input[data-comment-id="c2"]')).not.toBeNull()); await userEvent.click(screen.getByRole('checkbox', { name: '选择这条评论进入研究' })); expect(router.state.location.search).toContain('cursor=page2');
  await userEvent.click(screen.getByRole('button', { name: '上一页' })); await waitFor(() => expect(document.querySelector('input[data-comment-id="c1"]')).not.toBeNull()); expect(screen.getByRole('checkbox', { name: '选择这条评论进入研究' })).toBeChecked();
  await act(() => router.navigate(-1)); await waitFor(() => expect(document.querySelector('input[data-comment-id="c2"]')).not.toBeNull()); expect(screen.getByRole('checkbox', { name: '选择这条评论进入研究' })).toBeChecked();
  await userEvent.click(screen.getByRole('button', { name: '研究所选评论' })); await userEvent.click(await screen.findByRole('button', { name: '预览本次选择' })); await screen.findByText(/预计冻结 2 条/);
  expect(calls.find(c => c.url.pathname.endsWith('/selection-preview'))?.body).toMatchObject({ scope: { kind: 'comments', commentKeys: [{ workRef: WORK, commentExternalId: 'c1' }, { workRef: WORK, commentExternalId: 'c2' }] }, workRoles: [{ contentPublicRef: WORK, observationRole: 'primary' }] });
 });
 it('reopening clears old preview and receipt but retains an uncertain request identity', async () => {
  installFetch((url, options) => options.method === 'POST' && url.pathname.endsWith('/runs') ? Promise.reject(new TypeError('lost receipt')) : url.pathname.endsWith('/selection-preview') ? json({ targetCount: 1, requestedWorkCount: 1, coveredWorkCount: 1, scopeCommentCount: 1, exclusionCounts: {} }) : undefined);
  renderPage(); await screen.findByLabelText('研究累计读数与昨日活动'); await userEvent.click(screen.getByRole('button', { name: '发起研究' })); await userEvent.click(await screen.findByRole('checkbox', { name: '选择作品：合成作品' }));
  await userEvent.click(screen.getByRole('button', { name: '预览本次选择' })); await waitFor(() => expect(screen.getByRole('button', { name: '确认并开始研究' })).toBeEnabled()); await userEvent.click(screen.getByRole('button', { name: '确认并开始研究' })); await screen.findByText(/创建结果未获确认/);
  await userEvent.click(screen.getByRole('button', { name: '关闭' })); await userEvent.click(screen.getByRole('button', { name: '发起研究' })); expect(screen.getByRole('button', { name: '核对并重试原启动请求' })).toBeEnabled(); expect(document.querySelector('#selection-preview')).toBeEmptyDOMElement(); expect(document.querySelector('#run-result')).toHaveTextContent('上次启动回执尚未确认');
  await userEvent.click(screen.getByRole('button', { name: '核对并重试原启动请求' })); await screen.findByText(/创建结果未获确认/);
  const writes = calls.filter(c => c.method === 'POST' && c.url.pathname.endsWith('/runs')); expect(writes).toHaveLength(2); expect(writes[0]!.body?.requestRef).toMatch(/^[0-9a-f]{8}-[0-9a-f-]{27}$/i); expect(writes[0]!.body?.requestRef).toBe(writes[1]!.body?.requestRef);
 });
 it('changing an uncertain research intent requires a new preview and a different request identity', async () => {
  installFetch((url, options) => options.method === 'POST' && url.pathname.endsWith('/runs') ? Promise.reject(new TypeError('lost receipt')) : url.pathname.endsWith('/selection-preview') ? json({ targetCount: 1, requestedWorkCount: 1, coveredWorkCount: 1, scopeCommentCount: 1, exclusionCounts: {} }) : undefined);
  renderPage(); await screen.findByLabelText('研究累计读数与昨日活动'); await userEvent.click(screen.getByRole('button', { name: '发起研究' })); await userEvent.click(await screen.findByRole('checkbox', { name: '选择作品：合成作品' }));
  await userEvent.click(screen.getByRole('button', { name: '预览本次选择' })); await waitFor(() => expect(screen.getByRole('button', { name: '确认并开始研究' })).toBeEnabled()); await userEvent.click(screen.getByRole('button', { name: '确认并开始研究' })); await screen.findByText(/创建结果未获确认/);
  const budget = screen.getByRole('spinbutton', { name: '本次最多评论数' }); await userEvent.clear(budget); await userEvent.type(budget, '2'); expect(screen.getByRole('button', { name: '确认并开始研究' })).toBeDisabled(); expect(screen.queryByRole('button', { name: '核对并重试原启动请求' })).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole('button', { name: '预览本次选择' })); await waitFor(() => expect(screen.getByRole('button', { name: '确认并开始研究' })).toBeEnabled()); await userEvent.click(screen.getByRole('button', { name: '确认并开始研究' })); await waitFor(() => expect(calls.filter(c => c.method === 'POST' && c.url.pathname.endsWith('/runs'))).toHaveLength(2));
  const writes = calls.filter(c => c.method === 'POST' && c.url.pathname.endsWith('/runs')); expect(writes[0]!.body?.requestRef).toMatch(/^[0-9a-f]{8}-[0-9a-f-]{27}$/i); expect(writes[1]!.body?.requestRef).toMatch(/^[0-9a-f]{8}-[0-9a-f-]{27}$/i); expect(writes[1]!.body?.requestRef).not.toBe(writes[0]!.body?.requestRef); expect(writes[1]!.body?.limits).toMatchObject({ commentBudget: 2 });
 });
 it('the overview unfiled-expression link opens all unmerged clues', async () => {
  installFetch(url => url.pathname.endsWith('/problem-candidates') ? json({ expressions: [], page: { nextCursor: null } }) : undefined); const { router } = renderPage(); await screen.findByLabelText('研究累计读数与昨日活动');
  await userEvent.click(screen.getByRole('button', { name: '查看未建档表达' })); await waitFor(() => expect(calls.some(c => c.url.pathname.endsWith('/problem-candidates'))).toBe(true)); expect(router.state.location.search).toContain('state=clues'); expect(screen.getByRole('button', { name: '未归并线索' })).toHaveAttribute('aria-pressed', 'true');
 });
 it('a newly restricted detail also hides its stale list title, definition and quote', async () => {
  installFetch(url => url.pathname.endsWith('/problem-cards') ? json({ contract: 'comment-study.problem-cards.v1', domainRef: DOMAIN, problems: [problem()], totalCount: 1, page: { nextCursor: null } }) : url.pathname.endsWith('/problems/' + PROBLEM) ? json({ problem: problem(false), revisionHistory: [], sourceDistribution: null }) : url.pathname.endsWith('/evidence') ? json({ problemRef: PROBLEM, evidence: [], page: { nextCursor: null } }) : undefined);
  renderPage(`domain=${DOMAIN}&view=problems`); await userEvent.click(await screen.findByRole('button', { name: '查看问题：SYNTHETIC 旧标题' })); await screen.findByRole('heading', { name: '问题定义当前不可读取', level: 2 }); await waitFor(() => expect(screen.queryByText('SYNTHETIC 旧引文')).not.toBeInTheDocument());
  expect(screen.queryByText('SYNTHETIC 旧定义')).not.toBeInTheDocument(); await userEvent.click(screen.getByRole('button', { name: '关闭 ×' })); expect(screen.queryByText('SYNTHETIC 旧标题')).not.toBeInTheDocument();
 });
 it('a late work lookup cannot navigate back after leaving comments', async () => {
  let finish: (value: Response) => void = () => {}; installFetch(url => url.pathname.endsWith('/works') ? new Promise<Response>(resolve => { finish = resolve; }) : undefined);
  const { router } = renderPage(`domain=${DOMAIN}&view=comments`); await screen.findByRole('table'); await userEvent.click(screen.getByRole('button', { name: '查找作品' })); await userEvent.click(screen.getByRole('button', { name: '研究批次' }));
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
 it('a confirmed creation receipt and preview do not survive reopen or a later run visit', async () => {
  let created=false; installFetch((url, options) => options.method === 'POST' && url.pathname.endsWith('/runs') ? (created=true,json({requestRef:JSON.parse(String(options.body)).requestRef,outcome:'created',runRef:RUN,targetCount:1,requestedWorkCount:1,coveredWorkCount:1})) : url.pathname.endsWith('/selection-preview') ? json({targetCount:1,requestedWorkCount:1,coveredWorkCount:1,scopeCommentCount:1,exclusionCounts:{}}) : url.pathname.endsWith('/runs') ? json({domainRef:DOMAIN,runs:created?[{...run(),state:'queued',selectionContract:'comment-study.run-selection.v2',controlVersion:0,dispatchState:'enabled',finishedAt:null,targetCount:1,pendingCount:1},run(RUN2,'SYNTHETIC 方法二')]:[run(RUN2,'SYNTHETIC 方法二')],page:{nextCursor:null}}) : runReply(url));
  const {router}=renderPage(`domain=${DOMAIN}&view=runs&runRef=${RUN2}&panel=targets&targetRef=old-target&signalRef=old-signal`, true);await screen.findByRole('region',{name:'本次冻结范围'});await userEvent.click(screen.getByRole('button',{name:'发起研究'}));await userEvent.click(await screen.findByRole('checkbox',{name:'选择作品：合成作品'}));await userEvent.click(screen.getByRole('button',{name:'预览本次选择'}));await waitFor(()=>expect(screen.getByRole('button',{name:'确认并开始研究'})).toBeEnabled());await userEvent.click(screen.getByRole('button',{name:'确认并开始研究'}));await waitFor(()=>expect(document.querySelector('#study-run-control-feedback')).toHaveTextContent(/已创建 .*已进入模型执行队列/));expect(new URLSearchParams(router.state.location.search).has('targetRef')).toBe(false);expect(new URLSearchParams(router.state.location.search).has('signalRef')).toBe(false);await waitFor(()=>expect(document.querySelector('[aria-label="本次运行操作"] [data-run-control="pause"]')).toHaveAttribute('data-run',RUN));await act(()=>router.navigate(-1));await screen.findByRole('heading',{name:'本次研究 · SYNTHETIC 方法二'});expect(document.querySelector('#study-run-control-feedback')).toBeEmptyDOMElement();await act(()=>router.navigate(1));await screen.findByRole('heading',{name:'本次研究 · SYNTHETIC 方法一'});expect(document.querySelector('#study-run-control-feedback')).toBeEmptyDOMElement();
  await userEvent.click(screen.getByRole('button',{name:'发起研究'}));expect(screen.getByRole('button',{name:'确认并开始研究'})).toBeDisabled();expect(document.querySelector('#run-result')).toBeEmptyDOMElement();await userEvent.click(screen.getByRole('button',{name:'关闭'}));await userEvent.click(screen.getByRole('button',{name:'用户评论'}));await screen.findByRole('table');await userEvent.click(screen.getByRole('button',{name:'研究批次'}));await screen.findByRole('region',{name:'本次冻结范围'});expect(document.querySelector('#study-run-control-feedback')).toBeEmptyDOMElement();
 });
 it.each([['known',true],['restricted',false]] as const)('a %s clue applies current readability to its recorded author name',async(sourceState,readable)=>{
  const key={workRef:WORK,commentExternalId:'c1'};
  installFetch(url=>url.pathname.endsWith('/problem-candidates')?json({expressions:[{signalRef:'signal',commentKey:key,sourceState:'known',commentText:'SYNTHETIC 原声',proposition:'SYNTHETIC 表达',authorDisplayName:'SYNTHETIC 账号名',kind:'need',state:'deferred_novel',pairOutcomes:[]}],page:{nextCursor:null}}):url.pathname.endsWith('/comments/detail')?json({commentKey:key,source:{sourceState},comment:{commentText:'SYNTHETIC 原声'}}):undefined);
  renderPage(`domain=${DOMAIN}&view=problems&state=clues`);await userEvent.click(await screen.findByRole('button',{name:'查看这条未归并表达'}));if(readable)expect(await screen.findByText('SYNTHETIC 账号名')).toBeInTheDocument();else{await screen.findByRole('heading',{name:'来源当前不可读取'});expect(screen.queryByText('SYNTHETIC 账号名')).not.toBeInTheDocument();}
 });

 it('a failed background shell refresh preserves uncertain intent and its original request identity',async()=>{
  let shellReads=0;
  installFetch((url,options)=>url.pathname==='/api/local/corpus/shell'&&++shellReads>1?json({error:'unavailable'},503):options.method==='POST'&&url.pathname.endsWith('/runs')?Promise.reject(new TypeError('lost receipt')):url.pathname.endsWith('/selection-preview')?json({targetCount:1,requestedWorkCount:1,coveredWorkCount:1,scopeCommentCount:1,exclusionCounts:{}}):undefined);
  const {queryClient}=renderPage();await screen.findByLabelText('研究累计读数与昨日活动');await userEvent.click(screen.getByRole('button',{name:'发起研究'}));await userEvent.click(await screen.findByRole('checkbox',{name:'选择作品：合成作品'}));await userEvent.click(screen.getByRole('button',{name:'预览本次选择'}));await waitFor(()=>expect(screen.getByRole('button',{name:'确认并开始研究'})).toBeEnabled());await userEvent.click(screen.getByRole('button',{name:'确认并开始研究'}));await screen.findByText(/创建结果未获确认/);
  await act(async()=>{await queryClient.refetchQueries({queryKey:['corpus-shell',DOMAIN]});});expect(shellReads).toBe(2);await screen.findByText(/领域信息刷新失败/);expect(screen.getByRole('button',{name:'重新读取领域信息'})).toBeInTheDocument();expect(document.querySelector('#study-dialog')).toHaveAttribute('open');expect(screen.queryByText(/评论研究准备信息读取失败/)).not.toBeInTheDocument();expect(screen.getByRole('checkbox',{name:'选择作品：合成作品'})).toBeChecked();
  await userEvent.click(screen.getByRole('button',{name:'核对并重试原启动请求'}));await screen.findByText(/创建结果未获确认/);const writes=calls.filter(c=>c.method==='POST'&&c.url.pathname.endsWith('/runs'));expect(writes).toHaveLength(2);expect(writes[0]!.body?.requestRef).toMatch(/^[0-9a-f]{8}-[0-9a-f-]{27}$/i);expect(writes[0]!.body?.requestRef).toBe(writes[1]!.body?.requestRef);
 });
 it('an older work search cannot overtake a newer pending search within the same route',async()=>{
  const finish=new Map<string,(value:Response)=>void>();installFetch(url=>url.pathname.endsWith('/works')?new Promise<Response>(resolve=>{finish.set(url.searchParams.get('q')||'',resolve);}):undefined);renderPage(`domain=${DOMAIN}&view=comments`);await screen.findByRole('table');
  const input=screen.getByRole('searchbox',{name:'所属作品'});await userEvent.type(input,'older');await userEvent.click(screen.getByRole('button',{name:'查找作品'}));await userEvent.clear(input);await userEvent.type(input,'newer');await userEvent.click(screen.getByRole('button',{name:'查找作品'}));
  await act(async()=>{finish.get('older')!(json({items:[{workRef:WORK,displayTitle:'SYNTHETIC 旧搜索'}],page:{nextCursor:null}}));await Promise.resolve();});expect(screen.queryByRole('option',{name:'SYNTHETIC 旧搜索'})).not.toBeInTheDocument();
  await act(async()=>{finish.get('newer')!(json({items:[{workRef:WORK,displayTitle:'SYNTHETIC 新搜索'}],page:{nextCursor:null}}));});expect(await screen.findByRole('option',{name:'SYNTHETIC 新搜索'})).toBeInTheDocument();
 });
 it('a work lookup cannot restore an old filter after the comment route changes',async()=>{
  let finish:(value:Response)=>void=()=>{};installFetch(url=>url.pathname.endsWith('/works')?new Promise<Response>(resolve=>{finish=resolve;}):undefined);const {router}=renderPage(`domain=${DOMAIN}&view=comments`);await screen.findByRole('table');await userEvent.click(screen.getByRole('button',{name:'查找作品'}));await userEvent.type(screen.getByRole('searchbox',{name:'字面搜索'}),'new-filter');await userEvent.click(screen.getByRole('button',{name:'应用筛选'}));await waitFor(()=>expect(router.state.location.search).toContain('q=new-filter'));
  await act(async()=>{finish(json({items:[{workRef:WORK,displayTitle:'SYNTHETIC 过期搜索'}],page:{nextCursor:null}}));await Promise.resolve();});expect(router.state.location.search).toContain('q=new-filter');expect(screen.queryByRole('option',{name:'SYNTHETIC 过期搜索'})).not.toBeInTheDocument();
 });

});
