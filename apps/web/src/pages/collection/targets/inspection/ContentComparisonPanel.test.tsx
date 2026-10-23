import type { ReactNode } from 'react';
import { KeywordRounds } from './KeywordRounds';
import { QueryClient,QueryClientProvider } from '@tanstack/react-query';
import { cleanup,fireEvent,render,screen,waitFor } from '@testing-library/react';
import { afterEach,expect,it,vi } from 'vitest';
import { MemoryRouter } from 'react-router';
import { decodeDrawer } from '../api';
import { Context } from './researchContext';
import { ContentComparisonPanel } from './ContentComparisonPanel';
const target='00000000-0000-4000-8000-000000000001';
const a='00000000-0000-4000-8000-000000000002';
const b='00000000-0000-4000-8000-000000000003';
const c='00000000-0000-4000-8000-000000000004';
const request='00000000-0000-4000-8000-000000000005';
const data=decodeDrawer({target:{targetRef:target,kind:'creator',platform:'xhs',identity:'synthetic',handle:'synthetic',name:'合成作者',
  group:'未分组',domainName:null,lifecycle:'paused',monitoring:false,avatar:{state:'not_observed',path:null},archiveLabel:'尚未建立',archiveTone:'neutral',
  patrolLabel:'未开启',patrolTone:'neutral',works:'0',details:'0/0',recentNew:null,recentHits:null,last:null,next:null,
  action:{kind:'archive',label:'建立档案',tab:'overview',fragment:''}},inspector:null,contentGapAction:null,
  decision:{title:'合成说明',note:'合成说明',action:null},blocked:[],catalog:[],rules:[],lifecycle:{state:'not_read'}});
const answer=(value:unknown)=>new Response(JSON.stringify(value),{headers:{'Content-Type':'application/json'}});
const preview=(works:string[],revision='original')=>({targetRef:target,workRefs:works,knowledgeCutoff:'2026-10-23T00:00:00Z',
  materialRevisions:works.map(()=>revision),partial:false,taskConfigRef:target,taskRevision:1,canStart:true,reasons:[],methodVersion:'work-content-comparison.v1'});
function harness(client:QueryClient,works:string[],patch:(values:Record<string,string|null>,replace?:boolean)=>void,child:ReactNode=<ContentComparisonPanel/>,drawer=data){
  return <MemoryRouter><QueryClientProvider client={client}><Context.Provider value={{data:drawer,params:new URLSearchParams(),
    scope:{targetRef:target,domain:'all'},selected:works,panel:'compare',focusedWork:null,message:null,
    patch,toggle:vi.fn(),open:vi.fn(),back:vi.fn(),clear:vi.fn(),pinCutoff:vi.fn()}}>{child}</Context.Provider></QueryClientProvider></MemoryRouter>;
}
afterEach(()=>{cleanup();vi.restoreAllMocks();});
it('unconfirmed retry preserves the original command and payload after preview refresh and panel reopen',async()=>{
  const client=new QueryClient({defaultOptions:{queries:{retry:false}}}),patch=vi.fn();
  const posts:unknown[]=[];let version='original';
  vi.spyOn(globalThis,'fetch').mockImplementation(async (_path,init)=>{
    if(init?.method==='POST'){
      posts.push(JSON.parse(String(init.body)));
      if(posts.length===1)throw new TypeError('synthetic response lost');
      return answer({requestRef:request,batchRef:target,reused:true});
    }
    return answer(preview([a,b],version));
  });
  const view=render(harness(client,[a,b],patch));
  await waitFor(()=>expect(screen.getByRole('button',{name:'比较内容'})).toBeEnabled());
  fireEvent.click(screen.getByRole('button',{name:'比较内容'}));
  await screen.findByRole('button',{name:'按原请求核对'});
  view.rerender(harness(client,[b,c],patch));
  version='newer';
  view.rerender(harness(client,[a,b],patch));
  await client.invalidateQueries({queryKey:['target-inspection']});
  fireEvent.click(await screen.findByRole('button',{name:'按原请求核对'}));
  await waitFor(()=>expect(posts).toHaveLength(2));
  expect(posts[1]).toEqual(posts[0]);
  await waitFor(()=>expect(patch).toHaveBeenCalledWith({inspect_comparison_request:request,inspect_comparison_command:null}));
});
it('a late response retains its receipt without navigating the newly selected works',async()=>{
  const client=new QueryClient({defaultOptions:{queries:{retry:false}}}),patch=vi.fn();
  let finish:(response:Response)=>void=()=>{throw new Error('not submitted');};
  vi.spyOn(globalThis,'fetch').mockImplementation(async (path,init)=>{
    if(init?.method==='POST')return new Promise<Response>(resolve=>{finish=resolve;});
    const works=new URL(String(path),'http://localhost').searchParams.get('works')?.split(',')??[];
    return answer(preview(works));
  });
  const view=render(harness(client,[a,c],patch));
  await waitFor(()=>expect(screen.getByRole('button',{name:'比较内容'})).toBeEnabled());
  fireEvent.click(screen.getByRole('button',{name:'比较内容'}));
  await screen.findByRole('button',{name:'正在提交…'});
  view.rerender(harness(client,[b,c],patch));
  finish(answer({requestRef:request,batchRef:target,reused:false}));
  await waitFor(()=>expect(screen.getByRole('button',{name:'比较内容'})).toBeEnabled());
  expect(patch).not.toHaveBeenCalled();
  view.rerender(harness(client,[a,c],patch));
  expect(await screen.findByRole('button',{name:'读取研究结果'})).toBeInTheDocument();
});

it('历史轮次默认不被当前启用规则过滤，保留规则未记录的入口',async()=>{
  const client=new QueryClient({defaultOptions:{queries:{retry:false}}}),paths:string[]=[];
  const drawer={...data,rules:[{ruleRef:target,slot:'new',enabled:true,interval:86400,scroll:null,top:null,days:null,next:null,last:null,revision:1}]};
  vi.spyOn(globalThis,'fetch').mockImplementation(async path=>{
    paths.push(String(path));return answer({targetRef:target,domainRef:null,knowledgeCutoff:'2026-10-23T00:00:00Z',
      methodVersion:'target-inspection.v1',rounds:[],nextOffset:null});
  });
  render(harness(client,[],vi.fn(),<KeywordRounds/>,drawer));
  await waitFor(()=>expect(paths.length).toBeGreaterThan(0));
  expect(paths.every(path=>!new URL(path,'http://localhost').searchParams.has('rule'))).toBe(true);
  expect(screen.getByRole('option',{name:'全部已取得轮次（含规则未记录）'})).toBeInTheDocument();
});
