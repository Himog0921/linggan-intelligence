import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { calls, comment, DOMAIN, installFetch, json, renderPage, WORK } from './testKit';

afterEach(() => vi.unstubAllGlobals());
describe('D3 actual four-view React boundary', () => {
 it('renders literal source text, unknown likes and server-owned eligibility without executing markup',async()=>{
  installFetch();renderPage(`domain=${DOMAIN}&view=comments`);
  await screen.findByRole('table');
  expect(screen.getAllByRole('button',{name:'SYNTHETIC 原声 <script>不执行</script>'})).toHaveLength(2);
  expect(document.querySelector('script')).toBeNull();
  expect(document.querySelector('select[aria-label="搜索范围"]')).toBeNull();
  expect(screen.getByTitle('点赞数未知：采集时没有取得这条评论的点赞')).toHaveTextContent('—');
  expect(screen.queryByRole('checkbox',{name:'选择这条评论'})).not.toBeInTheDocument();
  expect(screen.getByRole('button',{name:'选择评论'})).toHaveAttribute('title','进入批量选择模式');
  const selectMode = screen.getByRole('button',{name:'选择评论'});await userEvent.click(selectMode);
  const boxes=screen.getAllByRole('checkbox',{name:'选择这条评论'});expect(boxes[0]).toBeEnabled();expect(boxes[1]).toBeDisabled();
  const pageSelect=screen.getByRole('button',{name:/勾选本页/});expect(pageSelect.compareDocumentPosition(screen.getByRole('button',{name:'取消选择'})) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();await userEvent.click(boxes[0]!);expect(screen.getByRole('button',{name:/勾选本页/})).toBeInTheDocument();expect(screen.queryByText('已选 1 条 · 来自 1 篇作品')).not.toBeInTheDocument();expect(screen.queryByRole('button',{name:'研究所选评论'})).not.toBeInTheDocument();
  expect(calls.every(call=>call.method==='GET')).toBe(true);
 });
 it('sends the complete submitted filter to the server and records it in the deep link',async()=>{
  installFetch();const {router}=renderPage(`domain=${DOMAIN}&view=comments`);await screen.findByRole('table');
  await userEvent.type(screen.getByRole('searchbox',{name:'字面搜索'}),'两字');await userEvent.click(screen.getByRole('combobox',{name:'研究标签'}));await userEvent.click(screen.getByRole('option',{name:'需求'}));await userEvent.click(screen.getByRole('button',{name:'筛选'}));await userEvent.click(screen.getByRole('combobox',{name:'点赞'}));await userEvent.click(screen.getByRole('option',{name:'100 以上'}));await userEvent.click(screen.getByRole('button',{name:'完成'}));
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
  await userEvent.click(screen.getAllByRole('button',{name:'标记已改成帖子'})[0]!);const receiptStatus=await screen.findByText(/保存未获确认/);expect(receiptStatus.tagName).toBe('P');expect(receiptStatus).toHaveAttribute('data-comment-toast');expect(receiptStatus).toHaveAttribute('data-error','true');expect(screen.queryByText('已改成帖子',{exact:true})).not.toBeInTheDocument();await userEvent.click(screen.getAllByRole('button',{name:'标记已改成帖子'})[0]!);await screen.findByRole('button',{name:'撤销标记'});expect(screen.getByText('已确认原请求回执，没有重复记录。')).toHaveAttribute('data-comment-toast');
  const writes=calls.filter(call=>call.method==='POST');expect(writes).toHaveLength(2);expect(writes[0]!.body?.requestRef).toBe(writes[1]!.body?.requestRef);expect(writes[0]!.body?.workRef).toBe(WORK);
 });

});
