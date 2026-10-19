import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { calls, comment, DOMAIN, installFetch, json, OTHER, overview, policy, POLICY, renderPage, WORK } from './testKit';
afterEach(() => vi.unstubAllGlobals());
describe('D3 actual four-view React boundary', () => {
 it('keeps backend totals distinct from the bounded preview and zero distinct from unknown', async () => {
  installFetch(); renderPage();
  const region = await screen.findByLabelText('研究累计读数与昨日活动');
  expect(region).toHaveTextContent('可研究评论100条');expect(region).toHaveTextContent('当前研究信号70个');expect(region).toHaveTextContent('长期用户问题0个');
  await userEvent.click(screen.getByRole('tab',{name:'使用经历'}));
  expect(region).toHaveTextContent('当前研究信号70个');
  expect(calls.filter(call=>call.url.pathname.endsWith('/overview'))).toHaveLength(1);
  await userEvent.click(screen.getByRole('button',{name:'近 7 天'}));
  await userEvent.click(screen.getByRole('button',{name:'表格'}));
  expect(within(screen.getByRole('table')).getAllByRole('row')).toHaveLength(8);
  expect(within(screen.getByRole('table')).getAllByRole('cell').some(cell=>cell.textContent==='—')).toBe(true);
 });
 it('does not convert a failed overview read into an empty successful overview',async()=>{
  installFetch(url=>url.pathname.endsWith('/overview')?json({error:'unavailable'},503):undefined);renderPage();
  expect(await screen.findByText('总览读取失败；其他页签可单独查看已有研究结果。')).toBeInTheDocument();expect(screen.queryByLabelText('研究累计读数与昨日活动')).not.toBeInTheDocument();
 });
 it('renders literal source text, unknown likes and server-owned eligibility without executing markup',async()=>{
  installFetch();renderPage(`domain=${DOMAIN}&view=comments`);
  await screen.findByRole('table');
  expect(screen.getAllByRole('button',{name:'SYNTHETIC 原声 <script>不执行</script>'})).toHaveLength(2);
  expect(document.querySelector('script')).toBeNull();
  expect(screen.getByTitle('点赞数未知：采集时没有取得这条评论的点赞')).toHaveTextContent('—');
  const boxes=screen.getAllByRole('checkbox',{name:'选择这条评论进入研究'});expect(boxes[0]).toBeEnabled();expect(boxes[1]).toBeDisabled();
  await userEvent.click(boxes[0]!);expect(screen.getByText('已选 1 条 · 来自 1 篇作品')).toBeInTheDocument();expect(screen.getByRole('button',{name:'研究所选评论'})).toBeEnabled();
  expect(calls.every(call=>call.method==='GET')).toBe(true);
 });
 it('sends the complete submitted filter to the server and records it in the deep link',async()=>{
  installFetch();const {router}=renderPage(`domain=${DOMAIN}&view=comments`);await screen.findByRole('table');
  await userEvent.type(screen.getByRole('searchbox',{name:'字面搜索'}),'两字');await userEvent.selectOptions(screen.getByRole('combobox',{name:'研究标签'}),'need');await userEvent.click(screen.getByRole('button',{name:/更多筛选/}));await userEvent.selectOptions(screen.getByRole('combobox',{name:'点赞'}),'100');await userEvent.click(screen.getByRole('button',{name:'应用筛选'}));
  await waitFor(()=>expect(calls.filter(call=>call.url.pathname.endsWith('/comments')).at(-1)?.url.searchParams.get('q')).toBe('两字'));
  const call=calls.filter(call=>call.url.pathname.endsWith('/comments')).at(-1)!;expect(call.url.searchParams.get('signalKind')).toBe('need');expect(call.url.searchParams.get('minLikes')).toBe('100');expect(router.state.location.search).toContain('signalKind=need');expect(call.url.searchParams.get('domain')).toBe(DOMAIN);
 });
 it('moves tertiary navigation and verified counts into the shared context row, keeping the table flat', async()=> {
  installFetch();renderPage(`domain=${DOMAIN}&view=comments`);await screen.findByRole('table');
  const nav=screen.getByRole('navigation',{name:'评论研究视图'});
  expect(within(nav).getByRole('button',{name:'用户评论'})).toHaveAttribute('aria-current','page');
  expect(document.querySelector('.v7-context-section-nav')).toContainElement(nav);
  expect(screen.queryByText('评论证据库')).not.toBeInTheDocument();
  await waitFor(()=>expect(document.querySelector('.v7-context-count summary b')).toHaveTextContent('100'));
  expect(within(screen.getByRole('table')).getAllByRole('columnheader').map(cell=>cell.textContent)).toEqual(['选择','原声','点赞','所属作品','研究状态','研究标签','作者','点赞采集日','观测时间','改帖']);
  expect(within(screen.getByRole('table')).getAllByRole('row')).toHaveLength(3);
  expect(screen.queryByText(/已选 0/)).not.toBeInTheDocument();
 });
 it('retains comment selection across tertiary pages and exposes the compact batch bar only when selected',async()=>{
  installFetch();renderPage(`domain=${DOMAIN}&view=comments`);await screen.findByRole('table');
  await userEvent.click(screen.getAllByRole('checkbox',{name:'选择这条评论进入研究'})[0]!);
  expect(screen.getByText('已选 1 条 · 来自 1 篇作品')).toBeInTheDocument();
  await userEvent.click(screen.getByRole('button',{name:'概览'}));await screen.findByLabelText('研究累计读数与昨日活动');
  expect(screen.queryByText('已选 1 条 · 来自 1 篇作品')).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole('button',{name:'用户评论'}));await screen.findByRole('table');
  expect(screen.getAllByRole('checkbox',{name:'选择这条评论进入研究'})[0]).toBeChecked();
  await userEvent.click(screen.getByRole('button',{name:'清空选择'}));
  expect(screen.queryByText('已选 1 条 · 来自 1 篇作品')).not.toBeInTheDocument();
 });
 it('keeps research start independent of comment-directory failures',async()=>{
  installFetch(url=>url.pathname.endsWith('/comments')?json({error:'unavailable'},503):undefined);
  renderPage(`domain=${DOMAIN}&view=comments`);
  await screen.findByText(/评论目录读取失败/);
  expect(document.querySelector('.v7-context-count summary b')).toHaveTextContent('—');
  const start=screen.getByRole('button',{name:'发起研究'});expect(start).toBeEnabled();
  await userEvent.click(start);
  expect(await screen.findByRole('combobox',{name:'本次使用的方法版本'})).toBeInTheDocument();
 });
 it('retains an uncertain post-usage request identity and never updates optimistically',async()=>{
  let attempts=0;installFetch((url,options)=>{
   if(url.pathname.endsWith('/comments/post-usage')){attempts++;if(attempts===1)return Promise.reject(new TypeError('lost receipt'));const body=JSON.parse(String(options.body));return json({contract:'comment-post-usage.v1',requestRef:body.requestRef,commentKey:body, outcome:'replayed',postUsage:{state:'used',markedAt:null}});}
   if(url.pathname.endsWith('/comments')&&attempts>1)return json({domainRef:DOMAIN,items:[{...comment(),postUsage:{state:'used',markedAt:null}}],summary:{displayableCommentCount:1,eligibleCommentCount:1},indexCoverage:{indexedCount:1,pendingCount:0},page:{nextCursor:null}});
   return undefined;
  });renderPage(`domain=${DOMAIN}&view=comments`);await screen.findByRole('table');
  await userEvent.click(screen.getAllByRole('button',{name:'标记已改成帖子'})[0]!);await screen.findByText(/保存未获确认/);expect(screen.queryByText('已改成帖子',{exact:true})).not.toBeInTheDocument();await userEvent.click(screen.getAllByRole('button',{name:'标记已改成帖子'})[0]!);await screen.findByRole('button',{name:'撤销标记'});
  const writes=calls.filter(call=>call.method==='POST');expect(writes).toHaveLength(2);expect(writes[0]!.body?.requestRef).toBe(writes[1]!.body?.requestRef);expect(writes[0]!.body?.workRef).toBe(WORK);
 });
 it('aborts the old domain read and excludes its late result from the new domain',async()=>{
  let resolveOld:(value:Response)=>void=()=>{};
  installFetch(url=>url.pathname.endsWith('/overview')&&url.searchParams.get('domain')===DOMAIN?new Promise<Response>(resolve=>{resolveOld=resolve;}):undefined);
  const {router}=renderPage();await waitFor(()=>expect(calls.some(call=>call.url.pathname.endsWith('/overview'))).toBe(true));
  await act(()=>router.navigate(`/corpus/comments?domain=${OTHER}&view=overview`));await screen.findByLabelText('研究累计读数与昨日活动');
  expect(calls.find(call=>call.url.pathname.endsWith('/overview')&&call.url.searchParams.get('domain')===DOMAIN)?.signal?.aborted).toBe(true);
  const stale=overview();stale.corpusSummary.eligibleCommentCount=999;await act(async()=>{resolveOld(json(stale));await Promise.resolve();});
  expect(screen.getByLabelText('研究累计读数与昨日活动')).toHaveTextContent('可研究评论100条');expect(screen.getByLabelText('研究累计读数与昨日活动')).not.toHaveTextContent('999');
 });
 it('preserves overview reading preferences when visiting another view and returning',async()=>{
  installFetch();renderPage();await screen.findByLabelText('研究累计读数与昨日活动');
  await userEvent.click(screen.getByRole('button',{name:'近 7 天'}));
  await userEvent.click(screen.getByRole('button',{name:'表格'}));
  await userEvent.click(screen.getByRole('tab',{name:'使用经历'}));
  await userEvent.click(screen.getByRole('button',{name:'用户评论'}));await screen.findByRole('table');
  await userEvent.click(screen.getByRole('button',{name:'概览'}));await screen.findByLabelText('研究累计读数与昨日活动');
  expect(screen.getByRole('button',{name:'近 7 天'})).toHaveAttribute('aria-pressed','true');
  expect(screen.getByRole('button',{name:'表格'})).toHaveAttribute('aria-pressed','true');
  expect(screen.getByRole('tab',{name:'使用经历'})).toHaveAttribute('aria-selected','true');
  expect(within(screen.getByRole('table')).getAllByRole('row')).toHaveLength(8);
 });

 it('does not confirm an activation receipt for a different method',async()=>{
  const chosen='00000000-0000-4000-8000-000000000004';
  installFetch(url=>url.pathname.endsWith('/policies')?json({domainRef:DOMAIN,items:[policy(),{...policy(chosen),isActive:false}],page:{hasMore:false,nextCursor:null}}):url.pathname.endsWith('/activate')?json({domainRef:DOMAIN,policyRef:POLICY,isActive:true}):undefined);
  renderPage();await screen.findByLabelText('研究累计读数与昨日活动');await userEvent.click(screen.getByRole('button',{name:'发起研究'}));
  await screen.findByRole('combobox',{name:'本次使用的方法版本'});await userEvent.selectOptions(screen.getByRole('combobox',{name:'本次使用的方法版本'}),chosen);await userEvent.click(screen.getByRole('button',{name:'设为默认方法'}));
  expect(await screen.findByText(/切换默认方法未获确认/)).toBeInTheDocument();expect(screen.queryByText('已将所选方法设为当前领域默认版本。')).not.toBeInTheDocument();
 });
 it('keeps a confirmed activation separate from a failed directory refresh',async()=>{
  let reads=0;const chosen='00000000-0000-4000-8000-000000000004';
  installFetch(url=>url.pathname.endsWith('/policies')?(++reads===1?json({domainRef:DOMAIN,items:[policy(),{...policy(chosen),isActive:false}],page:{hasMore:false,nextCursor:null}}):json({error:'unavailable'},503)):url.pathname.endsWith('/activate')?json({domainRef:DOMAIN,policyRef:chosen,isActive:true}):undefined);
  renderPage();await screen.findByLabelText('研究累计读数与昨日活动');await userEvent.click(screen.getByRole('button',{name:'发起研究'}));
  await screen.findByRole('combobox',{name:'本次使用的方法版本'});await userEvent.selectOptions(screen.getByRole('combobox',{name:'本次使用的方法版本'}),chosen);await userEvent.click(screen.getByRole('button',{name:'设为默认方法'}));
  expect(await screen.findByText(/默认方法已按回执切换，但目录刷新失败/)).toBeInTheDocument();
 });
 it('does not navigate back to the old domain when its start receipt arrives late',async()=>{
  let finish:(value:Response)=>void=()=>{};
  installFetch((url,options)=>options.method==='POST'&&url.pathname.endsWith('/runs')?new Promise<Response>(resolve=>{finish=resolve;}):url.pathname.endsWith('/selection-preview')?json({targetCount:1,requestedWorkCount:1,coveredWorkCount:1,scopeCommentCount:1,exclusionCounts:{}}):undefined);
  const {router}=renderPage();await screen.findByLabelText('研究累计读数与昨日活动');await userEvent.click(screen.getByRole('button',{name:'发起研究'}));
  await userEvent.click(await screen.findByRole('checkbox',{name:'选择作品：合成作品'}));await userEvent.click(screen.getByRole('button',{name:'预览本次选择'}));await waitFor(()=>expect(screen.getByRole('button',{name:'确认并开始研究'})).toBeEnabled());await userEvent.click(screen.getByRole('button',{name:'确认并开始研究'}));
  const write=calls.find(call=>call.method==='POST'&&call.url.pathname.endsWith('/runs'))!;expect(write).toBeDefined();
  await act(()=>router.navigate(`/corpus/comments?domain=${OTHER}&view=overview`));await screen.findByLabelText('研究累计读数与昨日活动');
  await act(async()=>{finish(json({requestRef:write.body?.requestRef,outcome:'created',runRef:'00000000-0000-4000-8000-000000000005',targetCount:1,requestedWorkCount:1,coveredWorkCount:1}));await Promise.resolve();});
  expect(new URLSearchParams(router.state.location.search).get('domain')).toBe(OTHER);expect(new URLSearchParams(router.state.location.search).get('view')).toBe('overview');
 });

});
