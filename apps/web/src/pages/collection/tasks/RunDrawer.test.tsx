import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { RunDrawer } from './RunDrawer';
const run='a0000000-0000-4000-8000-000000000001',domain='a0000000-0000-4000-8000-000000000002';
const data={run_ref:run,domain_ref:domain,name:'合成进行中任务',domain_name:'合成领域',source_kind:'temporary_keyword',legacy:false,ended:false,control_intent:'running',phase:'executing',comment_mode:'sample_30',created_at:'2026-10-14T00:00:00Z',updated_at:'2026-10-14T00:00:00Z',revision:1,work_limit:20,selected_works:2,available_details:0,delivered_comments:0};
afterEach(()=>{cleanup();vi.unstubAllGlobals();sessionStorage.clear();});
it('停止先展示后果，返回不发命令；只读遮罩可关闭详情',async()=>{
  const fetch=vi.fn(async()=>new Response(JSON.stringify(data),{status:200,headers:{'Content-Type':'application/json'}}));vi.stubGlobal('fetch',fetch);
  const close=vi.fn();
  render(<QueryClientProvider client={new QueryClient({defaultOptions:{queries:{retry:false}}})}><RunDrawer id={run} domain={domain} onClose={close}/></QueryClientProvider>);
  fireEvent.click(await screen.findByRole('button',{name:'停止本次'}));
  expect(screen.getByRole('heading',{name:'停止本次采集？'})).toBeInTheDocument();
  expect(screen.getByText(/停止后不能继续这次任务/)).toBeInTheDocument();
  expect(fetch.mock.calls).toHaveLength(1);
  fireEvent.click(screen.getByRole('button',{name:'返回进展'}));
  expect(screen.getByRole('button',{name:'停止本次'})).toBeInTheDocument();
  expect(fetch.mock.calls).toHaveLength(1);
  fireEvent.click(screen.getByRole('dialog'),{clientX:-1,clientY:-1});expect(close).toHaveBeenCalledOnce();
});

it('进展仅呈现业务信息，历史与内部诊断不渲染也不请求',async()=>{
  const fetch=vi.fn(async()=>new Response(JSON.stringify({...data,legacy:true,ended:true,revision:null,control_intent:null}),{status:200,headers:{'Content-Type':'application/json'}}));vi.stubGlobal('fetch',fetch);
  render(<QueryClientProvider client={new QueryClient({defaultOptions:{queries:{retry:false}}})}><RunDrawer id={run} domain={domain} onClose={()=>{}}/></QueryClientProvider>);
  await screen.findByText('历史采集');
  expect(screen.getByRole('heading',{name:'采集进展'})).toBeInTheDocument();
  expect(screen.queryByText('执行记录与诊断')).not.toBeInTheDocument();
  expect(screen.queryByText(run)).not.toBeInTheDocument();
  expect(screen.queryByText(/有效租约|冻结范围|Attempt|Package|Receipt/)).not.toBeInTheDocument();
  expect(fetch.mock.calls).toHaveLength(1);
});

it('主页浅看与文字版精选：进展里不把「本来就不做」写成 0 篇',async()=>{
  const open=async(over:object)=>{
    const fetch=vi.fn(async()=>new Response(JSON.stringify({...data,ended:true,phase:'ended',control_intent:'running',...over}),{status:200,headers:{'Content-Type':'application/json'}}));vi.stubGlobal('fetch',fetch);
    return render(<QueryClientProvider client={new QueryClient({defaultOptions:{queries:{retry:false}}})}><RunDrawer id={run} domain={domain} onClose={()=>{}}/></QueryClientProvider>);
  };
  const preview=await open({source_kind:'creator_preview',comment_mode:'none',available_details:0,completed_discussions:0});
  await screen.findByText('只看主页卡片，不取详情、评论与媒体');
  expect(screen.getByText('浅看只到发现，不取详情')).toBeInTheDocument();
  expect(screen.getByText('本次不采评论')).toBeInTheDocument();
  expect(screen.queryByText(/已取得 0 篇/)).not.toBeInTheDocument();
  expect(screen.queryByText(/0 篇已结束本轮采集/)).not.toBeInTheDocument();
  preview.unmount();
  await open({source_kind:'creator_selected_works',comment_mode:'none',available_details:2,completed_discussions:0});
  await screen.findByText('只取笔记文字，不采评论与媒体');
  expect(screen.getByText('已取得 2 篇')).toBeInTheDocument();
  expect(screen.getByText('本次不采评论')).toBeInTheDocument();
  expect(screen.queryByText(/0 篇已结束本轮采集/)).not.toBeInTheDocument();
});
