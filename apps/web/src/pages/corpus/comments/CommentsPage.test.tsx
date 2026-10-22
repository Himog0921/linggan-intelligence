import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { calls, comment, DOMAIN, installFetch, json, OTHER, overview, renderPage, WORK } from './testKit';

afterEach(() => vi.unstubAllGlobals());
describe('D3 actual four-view React boundary', () => {
 it('keeps backend totals distinct from the bounded preview and zero distinct from unknown', async () => {
  installFetch(); renderPage();
  const region = await screen.findByLabelText('研究累计读数与昨日活动');
  expect(region).toHaveTextContent('可研究评论100条');expect(region).toHaveTextContent('当前研究信号70个');expect(region).toHaveTextContent('长期用户问题0个');
  await userEvent.click(screen.getByRole('tab',{name:'使用经历'}));
  expect(region).toHaveTextContent('当前研究信号70个');
  expect(calls.filter(call=>call.url.pathname.endsWith('/overview'))).toHaveLength(1);
  expect(screen.getByRole('button',{name:'近 56 天'})).toHaveAttribute('aria-pressed','true');
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
  expect(screen.queryByRole('checkbox',{name:'选择这条评论'})).not.toBeInTheDocument();
  expect(screen.getByRole('button',{name:'选择评论'})).toHaveAttribute('title','进入批量选择模式');
  await userEvent.click(screen.getByRole('button',{name:'选择评论'}));
  const boxes=screen.getAllByRole('checkbox',{name:'选择这条评论'});expect(boxes[0]).toBeEnabled();expect(boxes[1]).toBeDisabled();
  await userEvent.click(boxes[0]!);expect(screen.getByText('已选 1 条 · 来自 1 篇作品')).toBeInTheDocument();expect(screen.queryByRole('button',{name:'研究所选评论'})).not.toBeInTheDocument();
  expect(calls.every(call=>call.method==='GET')).toBe(true);
 });
 it('sends the complete submitted filter to the server and records it in the deep link',async()=>{
  installFetch();const {router}=renderPage(`domain=${DOMAIN}&view=comments`);await screen.findByRole('table');
  await userEvent.type(screen.getByRole('searchbox',{name:'字面搜索'}),'两字');await userEvent.click(screen.getByRole('combobox',{name:'研究标签'}));await userEvent.click(screen.getByRole('option',{name:'需求'}));await userEvent.click(screen.getByRole('button',{name:/更多筛选/}));await userEvent.click(screen.getByRole('combobox',{name:'点赞'}));await userEvent.click(screen.getByRole('option',{name:'100 以上'}));await userEvent.click(screen.getByRole('button',{name:'应用筛选'}));
  await waitFor(()=>expect(calls.filter(call=>call.url.pathname.endsWith('/comments')).at(-1)?.url.searchParams.get('q')).toBe('两字'));
  const call=calls.filter(call=>call.url.pathname.endsWith('/comments')).at(-1)!;expect(call.url.searchParams.get('signalKind')).toBe('need');expect(call.url.searchParams.get('minLikes')).toBe('100');expect(router.state.location.search).toContain('signalKind=need');expect(call.url.searchParams.get('domain')).toBe(DOMAIN);
 });
 it('keeps FIND clearing local until the user explicitly applies the comment filter',async()=>{
  installFetch();renderPage(`domain=${DOMAIN}&view=comments`);await screen.findByRole('table');
  const reads=()=>calls.filter(call=>call.url.pathname.endsWith('/comments')).length;
  await userEvent.type(screen.getByRole('searchbox',{name:'字面搜索'}),'待清空');
  await userEvent.click(screen.getByRole('button',{name:'清空字面搜索'}));
  expect(reads()).toBe(1);
  expect(screen.getByRole('searchbox',{name:'字面搜索'})).toHaveValue('');
 });
 it('keeps a matched work in the filter draft until it is explicitly applied',async()=>{
  installFetch();const {router}=renderPage(`domain=${DOMAIN}&view=comments`);await screen.findByRole('table');
  const commentReads=()=>calls.filter(call=>call.url.pathname.endsWith('/comments')).length;
  await userEvent.click(screen.getByRole('button',{name:/更多筛选/}));await userEvent.type(screen.getByRole('searchbox',{name:'所属作品'}),'合成');await userEvent.click(screen.getByRole('button',{name:'查找作品'}));
  await userEvent.click(await screen.findByRole('combobox',{name:'已匹配作品'}));await userEvent.click(await screen.findByRole('option',{name:'合成作品'}));
  const readsBeforeApply=commentReads();expect(router.state.location.search).not.toContain('workRef=');
  await userEvent.click(screen.getByRole('button',{name:'应用筛选'}));await waitFor(()=>expect(router.state.location.search).toContain(`workRef=${WORK}`));
  expect(commentReads()).toBeGreaterThan(readsBeforeApply);expect(calls.filter(call=>call.url.pathname.endsWith('/comments')).at(-1)?.url.searchParams.get('workRef')).toBe(WORK);
 });
 it('moves tertiary navigation and verified counts into the shared context row, keeping the table flat', async()=> {
  installFetch();renderPage(`domain=${DOMAIN}&view=comments`);await screen.findByRole('table');
  const nav=screen.getByRole('navigation',{name:'评论研究视图'});
  expect(within(nav).getByRole('button',{name:'用户评论'})).toHaveAttribute('aria-current','page');
  expect(document.querySelector('.v7-context-section-nav')).toContainElement(nav);
  expect(screen.queryByText('评论证据库')).not.toBeInTheDocument();
  await waitFor(()=>expect(document.querySelector('.v7-context-count summary b')).toHaveTextContent('100'));
  expect(within(screen.getByRole('table')).getAllByRole('columnheader').map(cell=>cell.textContent)).toEqual(['原声','点赞','所属作品','研究状态','研究标签','作者','点赞采集日','观测时间','改帖']);
  expect(within(screen.getByRole('table')).getAllByRole('row')).toHaveLength(3);
  expect(screen.queryByText(/已选 0/)).not.toBeInTheDocument();
 });
 it('keeps retired research read-only without unrelated AI settings on the page', async()=> {
  installFetch();renderPage();await screen.findByLabelText('研究累计读数与昨日活动');
  expect(screen.queryByRole('button',{name:'发起研究'})).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole('button',{name:'研究批次'}));
  await waitFor(()=>expect(screen.getByRole('button',{name:'研究批次'})).toHaveAttribute('aria-current','page'));
  expect(screen.queryByRole('button',{name:'发起研究'})).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole('button',{name:'用户问题'}));
  await waitFor(()=>expect(screen.getByRole('button',{name:'用户问题'})).toHaveAttribute('aria-current','page'));
  expect(screen.queryByRole('button',{name:'发起研究'})).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole('button',{name:'用户评论'}));await screen.findByRole('table');
  expect(screen.queryByRole('button',{name:'发起研究'})).not.toBeInTheDocument();
  expect(screen.queryByRole('link',{name:'去 AI 设置'})).not.toBeInTheDocument();
  expect(calls.every(call=>call.method==='GET')).toBe(true);
 });
 it('retains comment selection across tertiary pages and exposes the compact batch bar only when selected',async()=>{
  installFetch();renderPage(`domain=${DOMAIN}&view=comments`);await screen.findByRole('table');
  await userEvent.click(screen.getByRole('button',{name:'选择评论'}));
  await userEvent.click(screen.getAllByRole('checkbox',{name:'选择这条评论'})[0]!);
  expect(screen.getByText('已选 1 条 · 来自 1 篇作品')).toBeInTheDocument();
  await userEvent.click(screen.getByRole('button',{name:'概览'}));await screen.findByLabelText('研究累计读数与昨日活动');
  expect(screen.queryByText('已选 1 条 · 来自 1 篇作品')).not.toBeInTheDocument();
  await userEvent.click(screen.getByRole('button',{name:'用户评论'}));await screen.findByRole('table');
  expect(screen.getAllByRole('checkbox',{name:'选择这条评论'})[0]).toBeChecked();
  await userEvent.click(screen.getByRole('button',{name:'清空选择'}));
  expect(screen.queryByText('已选 1 条 · 来自 1 篇作品')).not.toBeInTheDocument();
 });
 it('keeps comment-directory failures local',async()=>{
  installFetch(url=>url.pathname.endsWith('/comments')?json({error:'unavailable'},503):undefined);
  renderPage(`domain=${DOMAIN}&view=comments`);
  await screen.findByText(/评论目录读取失败/);
  expect(document.querySelector('.v7-context-count summary b')).toHaveTextContent('—');
  expect(screen.queryByRole('button',{name:'发起研究'})).not.toBeInTheDocument();
  expect(screen.queryByRole('link',{name:'去 AI 设置'})).not.toBeInTheDocument();
  expect(calls.every(call=>call.method==='GET')).toBe(true);
 });

 it('retains an uncertain post-usage request identity and never updates optimistically',async()=>{
  let attempts=0;installFetch((url,options)=>{
   if(url.pathname.endsWith('/comments/post-usage')){attempts++;if(attempts===1)return Promise.reject(new TypeError('lost receipt'));const body=JSON.parse(String(options.body));return json({contract:'comment-post-usage.v1',requestRef:body.requestRef,commentKey:body, outcome:'replayed',postUsage:{state:'used',markedAt:null}});}
   if(url.pathname.endsWith('/comments')&&attempts>1)return json({domainRef:DOMAIN,items:[{...comment(),postUsage:{state:'used',markedAt:null}}],summary:{displayableCommentCount:1,eligibleCommentCount:1},indexCoverage:{indexedCount:1,pendingCount:0},page:{nextCursor:null}});
   return undefined;
  });renderPage(`domain=${DOMAIN}&view=comments`);await screen.findByRole('table');
  await userEvent.click(screen.getAllByRole('button',{name:'标记已改成帖子'})[0]!);const receiptStatus=await screen.findByText(/保存未获确认/);expect(receiptStatus.tagName).toBe('P');expect(screen.queryByText('已改成帖子',{exact:true})).not.toBeInTheDocument();await userEvent.click(screen.getAllByRole('button',{name:'标记已改成帖子'})[0]!);await screen.findByRole('button',{name:'撤销标记'});
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

});
