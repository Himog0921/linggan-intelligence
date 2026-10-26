import type { ReactNode } from 'react';
import { DossierReadout } from './DossierReadout';
import { dossierJson,dossierMaterialJson,workRef } from '../../../corpus/evidence/evidenceFixtures';
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
function harness(client:QueryClient,works:string[],patch:(values:Record<string,string|null>,replace?:boolean)=>void,child:ReactNode=<ContentComparisonPanel/>,drawer=data,params=new URLSearchParams()){
  return <MemoryRouter><QueryClientProvider client={client}><Context.Provider value={{data:drawer,params,
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
  fireEvent.click(screen.getByRole('combobox',{name:'观察规则'}));
  expect(screen.getByRole('option',{name:'全部规则'})).toBeInTheDocument();
});

it('完整阅读复用 V2 正式入口和同一截点，保留表格、对话、角色与位置',async()=>{
  const paths:string[]=[];const cutoff='2026-10-23T08:00:00Z';let withdrawn=false;
  const payload=dossierJson(2,{materials:[dossierMaterialJson(1,{blocks:[
    {ref:'table-proof',kind:'table',speaker:'other',columns:['条件','行动'],rows:[['5mg','不要加量']],bbox:[0.1,0.2,0.5,0.3]},
    {ref:'dialogue-proof',kind:'dialogue',turns:[{speaker:'other',text:'没有改善'},{speaker:'author',text:'先复核'}],time:{startMs:100,endMs:200}},
  ]})]});
  vi.spyOn(globalThis,'fetch').mockImplementation(async path=>{paths.push(String(path));return withdrawn?new Response('{}',{status:503,headers:{'Content-Type':'application/json'}}):answer(payload);});
  const client=new QueryClient({defaultOptions:{queries:{retry:false}}});
  render(harness(client,[],vi.fn(),<DossierReadout work={workRef(2)} cutoff={cutoff}/>));
  fireEvent.click(screen.getByRole('button',{name:'展开已取得的底稿'}));
  expect(await screen.findByRole('cell',{name:'5mg'})).toBeInTheDocument();
  expect(screen.getByText('不要加量')).toBeInTheDocument();expect(screen.getByText('没有改善')).toBeInTheDocument();
  expect(screen.getByText('先复核')).toBeInTheDocument();expect(screen.getByText('0.100–0.200 秒')).toBeInTheDocument();expect(screen.getAllByText(/他人/).length).toBeGreaterThan(0);
  expect(paths).toHaveLength(1);const path=paths[0];if(path===undefined)throw new Error('未调用正式读取');const url=new URL(path,'http://localhost');
  expect(url.pathname).toBe(`/api/local/work-resources/${workRef(2)}/research-dossier`);
  expect(url.searchParams.get('asOf')).toBe(cutoff);expect(url.searchParams.has('domain')).toBe(false);
  withdrawn=true;await client.invalidateQueries({queryKey:['target-inspection']});await screen.findByRole('alert');
  expect(screen.queryByText('5mg')).not.toBeInTheDocument();expect(screen.queryByText('没有改善')).not.toBeInTheDocument();
});

it.each([403,409,503])('来源重读被拒绝 %s 后隐藏缓存中的比较结论和引用正文',async status=>{
  const client=new QueryClient({defaultOptions:{queries:{retry:false}}});let rejected=false;
  const cutoff='2026-10-23T00:00:00Z';
  const materials=[a,b].map(work=>({workRef:work,knowledgeCutoff:cutoff,materialRevision:'revision',dossierRevision:'v2',
    ownerContract:'research-dossier.v2',partial:false,limitations:[],fragments:[{citationId:'block-'+work,
      sourceRef:work,sourceVersion:'source-v2',field:'ocr',start:0,end:5,structure:{ref:'block-'+work,kind:'paragraph',speaker:'other',text:'已撤回引用正文'},
      issues:[],speaker:'other',readingRef:work,unitRef:work,knowledgeCutoff:cutoff,sourceLocation:{slotKey:'page-1',page:1}}]}));
  const payload={requestRef:request,batchRef:target,knowledgeCutoff:cutoff,state:'succeeded',waitReason:null,failureCode:null,
    result:{relation:'supplemented',claims:[{kind:'added_condition',statement:'已撤回的比较结论',targetCitations:[{workRef:a,citationId:'block-'+a}],
      referenceCitations:[{workRef:b,citationId:'block-'+b}]}],limitations:[],partial:false,methodVersion:'v1'},materials,outdated:false,methodVersion:'v1'};
  vi.spyOn(globalThis,'fetch').mockImplementation(async path=>String(path).includes('/preview')?answer(preview([a,b])):
    rejected?new Response('{}',{status,headers:{'Content-Type':'application/json'}}):answer(payload));
  render(harness(client,[a,b],vi.fn(),<ContentComparisonPanel/>,data,new URLSearchParams({inspect_comparison_request:request})));
  expect(await screen.findByText('已撤回的比较结论')).toBeInTheDocument();expect(screen.getAllByText('已撤回引用正文')).toHaveLength(2);
  rejected=true;await client.invalidateQueries({queryKey:['target-inspection']});await screen.findByRole('alert');
  expect(screen.queryByText('已撤回的比较结论')).not.toBeInTheDocument();expect(screen.queryByText('已撤回引用正文')).not.toBeInTheDocument();
});


it('真实准入关闭原因分别说明，预览不会提交模型研究',async()=>{
  const works=['00000000-0000-4000-8000-000000000006','00000000-0000-4000-8000-000000000007'];
  const client=new QueryClient({defaultOptions:{queries:{retry:false}}});
  const fetch=vi.spyOn(globalThis,'fetch').mockResolvedValue(answer({...preview(works),canStart:false,
    reasons:['daily_budget_unconfigured','model_configuration_unqualified','executor_off','global_off','task_work_comparison_off']}));
  render(harness(client,works,vi.fn()));
  const reasons=await screen.findByText(/每日额度尚未配置/);
  for(const text of ['模型配置尚不可用','AI 执行器未开启','全局 AI 开关未开启','内容对照任务未开启'])expect(reasons).toHaveTextContent(text);
  expect(screen.getByRole('button',{name:'比较内容'})).toBeDisabled();
  expect(screen.queryByText(/当前研究暂不能执行/)).not.toBeInTheDocument();
  expect(fetch.mock.calls.every(([,init])=>init?.method==='GET')).toBe(true);
});
