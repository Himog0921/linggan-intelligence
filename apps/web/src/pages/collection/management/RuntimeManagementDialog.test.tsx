import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { createMemoryRouter, RouterProvider, useLocation } from 'react-router';
import { afterEach, expect, it, vi } from 'vitest';
import { CollectionFrame } from '../CollectionFrame';
import { runtimeDestination } from './runtimeDestination';
const projection={meta:{vacantStations:0,unclaimedInstallations:0},board:{state:'未知',tone:'unknown',badge:'未知',headNote:'',metrics:[]},lanes:null,stations:[],unclaimed:[],accountObservationAvailable:false,staffedCount:0,running:null,schedule:null,drawer:{coverage:'',readCount:0,picked:0,capabilityFailures:0,executionFailures:0,oldest:null,pauses:null,dispatched:'未知',succeeded:'未知'}};
const shell={domains:[],domainsReadable:true,selectedDomain:null,totalTargets:0,monitoringTargets:0,scheduler:'unknown'};
function Location(){return <output data-testid="route">{useLocation().search}</output>;}
function mount(){const client=new QueryClient({defaultOptions:{queries:{retry:false}}});const router=createMemoryRouter([{path:'/collection/tasks',element:<><CollectionFrame section="tasks"><p>原任务列表</p></CollectionFrame><Location/></>}],{initialEntries:['/collection/tasks?domain=all&page=3&q=保留条件']});const result=render(<QueryClientProvider client={client}><RouterProvider router={router}/></QueryClientProvider>);return {...result,router};}
function mockRead(){const fetch=vi.fn(async(path:string)=>Response.json(path.includes('/workspace/runtime')?{readable:true,asOf:null,returnTo:'/collection/tasks',projection}:shell));vi.stubGlobal('fetch',fetch);return fetch;}
afterEach(()=>{cleanup();vi.unstubAllGlobals();});
it('从面包屑原位打开与关闭，保留搜索分页并恢复焦点',async()=>{
  const fetch=mockRead();mount();const opener=screen.getByRole('button',{name:'工位管理，全局执行资源'});
  expect(screen.queryByText('采集与观察')).not.toBeInTheDocument();
  await userEvent.click(opener);await screen.findByRole('dialog',{name:'工位管理'});await screen.findByText('尚未登记工位，使用“新增工位”连接执行设备。');
  expect(new URLSearchParams(screen.getByTestId('route').textContent??'').get('runtime')).toBe('1');
  expect(fetch.mock.calls.some(([path])=>path.includes('/workspace/runtime?domain=all'))).toBe(true);
  await userEvent.click(screen.getByRole('button',{name:/今天在跑什么/}));expect(screen.getByText('当前执行情况读不到。')).toBeVisible();
  await userEvent.click(screen.getByRole('button',{name:/自动观察排程/}));expect(screen.getByText('自动观察排程当前读不到。')).toBeVisible();
  await userEvent.click(screen.getByRole('button',{name:'关闭工位管理'}));await waitFor(()=>expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  const route=new URLSearchParams(screen.getByTestId('route').textContent??'');expect(route.get('runtime')).toBeNull();expect(route.get('page')).toBe('3');expect(route.get('q')).toBe('保留条件');expect(opener).toHaveFocus();
});
it('关闭新增草稿先披露放弃后果，继续处理时保留输入且不写入',async()=>{
  const fetch=mockRead();mount();await userEvent.click(screen.getByRole('button',{name:'工位管理，全局执行资源'}));await screen.findByRole('button',{name:'＋ 新增工位'});
  await userEvent.click(screen.getByRole('button',{name:'＋ 新增工位'}));const input=screen.getByRole('textbox',{name:'工位名称'});await userEvent.clear(input);await userEvent.type(input,'未保存设备');
  await userEvent.click(screen.getByRole('button',{name:'关闭工位管理'}));expect(screen.getByText('有尚未保存的输入，关闭将放弃这些修改。')).toBeVisible();
  await userEvent.click(screen.getByRole('button',{name:'继续处理'}));expect(input).toHaveValue('未保存设备');
  await userEvent.click(screen.getByRole('button',{name:'关闭工位管理'}));await userEvent.click(screen.getByRole('button',{name:'放弃修改并关闭'}));expect(screen.queryByRole('dialog')).not.toBeInTheDocument();expect(fetch.mock.calls.every(([path])=>path.includes('/workspace/'))).toBe(true);
});
it('兼容旧工位深链并拦截外部或反斜杠返回地址',()=>{
  const destination=runtimeDestination(new URLSearchParams({return_to:'/collection/tasks?domain=all&page=3&run=run-1',station:'station-1'}));const url=new URL(destination,'http://local');expect(url.pathname).toBe('/collection/tasks');expect(url.searchParams.get('run')).toBe('run-1');expect(url.searchParams.get('page')).toBe('3');expect(url.searchParams.get('station')).toBe('station-1');expect(url.searchParams.get('runtime')).toBe('1');
  for(const target of ['https://outside.test','//outside.test','/collection/tasks\\@outside.test','/collection/runtime'])expect(runtimeDestination(new URLSearchParams({return_to:target}))).toBe('/collection/targets?runtime=1');
});

it('未确认登记收起后再次打开只核实同一请求，不发第二次写入',async()=>{
  let identity:unknown;let posts=0;
  vi.stubGlobal('fetch',vi.fn(async(path:string,options?:RequestInit)=>{
    if(path.includes('/workspace/runtime'))return Response.json({readable:true,asOf:null,returnTo:'/collection/tasks',projection});
    if(path.includes('/workspace/shell'))return Response.json(shell);
    if(path.endsWith('/context'))return Response.json({epoch:'runtime-modal-test'});
    if(options?.method==='POST'){posts++;const body:unknown=JSON.parse(String(options.body));if(typeof body==='object'&&body!==null&&'requestRef' in body)identity=body.requestRef;throw new TypeError('lost reply');}
    expect(path).toContain(`/receipts/${String(identity)}`);return Response.json({requestRef:identity,action:'runtime-register',outcome:'applied',code:null,location:'/collection/runtime'});
  }));
  mount();await userEvent.click(screen.getByRole('button',{name:'工位管理，全局执行资源'}));await screen.findByRole('button',{name:'＋ 新增工位'});await userEvent.click(screen.getByRole('button',{name:'＋ 新增工位'}));
  await userEvent.click(screen.getByRole('button',{name:'登记工位'}));await screen.findByText(/结果未确认/);
  await userEvent.click(screen.getByRole('button',{name:'关闭工位管理'}));await userEvent.click(screen.getByRole('button',{name:'收起并保留请求'}));
  await userEvent.click(screen.getByRole('button',{name:'工位管理，全局执行资源'}));await screen.findByRole('button',{name:'＋ 新增工位'});await userEvent.click(screen.getByRole('button',{name:'＋ 新增工位'}));expect(screen.getByRole('button',{name:'登记工位'})).toBeDisabled();
  await userEvent.click(screen.getByRole('button',{name:'核实同一请求'}));await waitFor(()=>expect(screen.getByRole('button',{name:'＋ 新增工位'})).toHaveAttribute('aria-expanded','false'));expect(posts).toBe(1);
});

it('浏览器后退经过同一草稿保护，继续编辑保留弹窗与地址',async()=>{
  mockRead();const {router}=mount();await userEvent.click(screen.getByRole('button',{name:'工位管理，全局执行资源'}));await screen.findByRole('button',{name:'＋ 新增工位'});await userEvent.click(screen.getByRole('button',{name:'＋ 新增工位'}));await userEvent.type(screen.getByRole('textbox',{name:'工位名称'}),'有修改');
  await router.navigate(-1);await screen.findByText('有尚未保存的输入，关闭将放弃这些修改。');expect(router.state.location.search).toContain('runtime=1');
  await userEvent.click(screen.getByRole('button',{name:'继续处理'}));expect(screen.getByRole('textbox',{name:'工位名称'})).toHaveValue('本机 Chrome有修改');
  await router.navigate(-1);await screen.findByText('有尚未保存的输入，关闭将放弃这些修改。');await userEvent.click(screen.getByRole('button',{name:'放弃修改并关闭'}));await waitFor(()=>expect(screen.queryByRole('dialog')).not.toBeInTheDocument());expect(router.state.location.search).not.toContain('runtime=1');
});
