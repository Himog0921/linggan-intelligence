import { cleanup,fireEvent,render,screen,waitFor } from '@testing-library/react';
import { QueryClient,QueryClientProvider } from '@tanstack/react-query';
import { MemoryRouter,useSearchParams } from 'react-router';
import { afterEach,beforeEach,expect,it,vi } from 'vitest';
import { TargetDrawer } from '../TargetDrawer';
import type { Commands } from '../useWorkspace';

const id=(n:number)=>`00000000-0000-4000-8000-${String(n).padStart(12,'0')}`;
const target=id(1),work=id(2),rule=id(3),cutoff='2026-01-04T00:00:00Z';
const answer=(data:unknown,status=200)=>new Response(JSON.stringify(data),{status,headers:{'Content-Type':'application/json'}});
const commands:Commands={run:vi.fn(async()=>false),verify:vi.fn(async()=>undefined),busy:false,feedback:null};
const drawer={target:{targetRef:target,kind:'keyword',platform:'xhs',identity:'synthetic',handle:'synthetic',name:'合成目标',
  group:'未分组',domainName:null,lifecycle:'paused',monitoring:false,avatar:{state:'not_observed',path:null},archiveLabel:'尚未建立',archiveTone:'neutral',
  patrolLabel:'未开启',patrolTone:'neutral',works:'1',details:'1/1',recentNew:null,recentHits:null,last:null,next:null,
  action:{kind:'archive',label:'建立档案',tab:'overview',fragment:''}},inspector:null,contentGapAction:null,
  decision:{title:'合成说明',note:'合成说明',action:null},blocked:[],rules:[],lifecycle:{state:'not_read'},
  catalog:[{publicRef:work,externalId:'synthetic-work',title:'合成作品',creatorName:null,creatorId:null,position:1,published:null,
    source:'patrol',kind:null,complete:true,detailLabel:'已取得',detailState:'known',media:'未知',comments:null,last:null}]};
function metrics(value:number,epoch:number,ref:string){
  const metric={value,state:'known',observationRef:ref,observedAt:new Date(epoch).toISOString(),observedEpochMs:epoch,sourceKind:'discovery_search',precision:'exact'};
  return {likes:metric,comments:metric,collects:metric,shares:metric};
}
const observations=[3,2,1].map(day=>{const epoch=Date.parse(`2026-01-0${day}T00:00:00Z`);return {observationRef:id(10+day),packageRef:id(20+day),roundRef:id(30+day),
  targetRef:target,targetName:'合成目标',ruleSlot:'latest',sourceKind:'discovery_search',observedAt:new Date(epoch).toISOString(),observedEpochMs:epoch,
  acceptedAt:new Date(epoch+1000).toISOString(),metrics:metrics(day===3?21:day===2?12:8,epoch,id(10+day)),position:day,lateArrival:false};});
const rounds=observations.map(o=>({roundRef:o.roundRef,packageRef:o.packageRef,ruleRef:rule,ruleRevisionRef:rule,slot:'latest',observedAt:o.observedAt,
  observedEpochMs:o.observedEpochMs,acceptedAt:o.acceptedAt,coverage:'complete',hits:1,conditions:{ranking:'latest'}}));
const requests:URL[]=[];
function installReads(paged=false,kind='keyword',equalTime=false){
  const observed=equalTime?observations.map(o=>({...o,observedAt:observations[0]!.observedAt,observedEpochMs:observations[0]!.observedEpochMs})):observations;
  vi.spyOn(globalThis,'fetch').mockImplementation(async (path,init)=>{
    expect(init?.method).toBe('GET');const u=new URL(String(path),'http://localhost');requests.push(u);
    if(!u.pathname.includes('/inspection/'))return answer({...drawer,target:{...drawer.target,kind}});
    if(u.pathname.endsWith('/work-comparison'))return answer({targetRef:target,knowledgeCutoff:cutoff,methodVersion:'v1',mode:'current',ageHours:null,toleranceMinutes:null,items:[],referenceStatistics:Object.fromEntries(['likes','comments','collects','shares'].map(key=>[key,{median:null,count:0}]))});
    if(u.pathname.endsWith('/related'))return answer({workRef:work,knowledgeCutoff:cutoff,scannedCount:2,truncated:false,retrievalBasis:'canonical_dossier_lexical_candidates',
      candidates:[{workRef:id(4),title:'合成参照',publishedAt:'2025-01-01T00:00:00Z',temporalRelation:'older',selectionBasis:'title',materialAvailable:true}]});
    if(u.pathname.endsWith('/observations'))return answer({workRef:work,knowledgeCutoff:cutoff,methodVersion:'v1',
      observations:paged?(u.searchParams.get('offset')==='50'?observed.slice(2):observed.slice(0,2)):observed,
      nextOffset:paged&&u.searchParams.get('offset')!=='50'?50:null});
    if(u.pathname.endsWith('/observation-comparison')){
      const left=observed.find(o=>o.observationRef===u.searchParams.get('left')),right=observed.find(o=>o.observationRef===u.searchParams.get('right'));
      if(!left||!right||left.observedEpochMs>right.observedEpochMs)return answer({code:'left_observation_must_not_be_newer'},400);
      const difference={left:left.metrics.likes,right:right.metrics.likes,delta:left.observedEpochMs===right.observedEpochMs?null:right.metrics.likes.value-left.metrics.likes.value,
        elapsedSeconds:left.observedEpochMs===right.observedEpochMs?null:(right.observedEpochMs-left.observedEpochMs)/1000,percent:null,comparable:left.observedEpochMs!==right.observedEpochMs,reason:left.observedEpochMs===right.observedEpochMs?'observation_order_unknown':null};
      return answer({workRef:work,knowledgeCutoff:cutoff,methodVersion:'v1',left,right,metrics:{likes:difference,comments:difference,collects:difference,shares:difference}});
    }
    if(u.pathname.endsWith('/rounds'))return answer({targetRef:target,domainRef:null,knowledgeCutoff:cutoff,methodVersion:'v1',rounds,nextOffset:null});
    if(u.pathname.endsWith('/round-comparison')){
      const left=rounds.find(o=>o.roundRef===u.searchParams.get('left')),right=rounds.find(o=>o.roundRef===u.searchParams.get('right'));
      if(!left||!right||left.observedEpochMs>right.observedEpochMs)return answer({code:'left_round_must_not_be_newer'},400);
      return answer({targetRef:target,knowledgeCutoff:cutoff,methodVersion:'v1',left,right,comparisonState:'comparable',reasons:[],rows:[],
        commonCount:1,enteredCount:0,absentCount:0,truncated:false});
    }
    const current=rounds.find(r=>u.pathname.endsWith('/rounds/'+r.roundRef));
    if(current){
      await new Promise(resolve=>setTimeout(resolve,15));
      return answer({targetRef:target,knowledgeCutoff:cutoff,methodVersion:'v1',round:current,
        works:[{workRef:work,title:'合成作品',creator:null,position:1,metrics:observations[0]!.metrics,firstSeenInScope:false,libraryPresence:'existing',seenBeforeLeft:true}],truncated:false});
    }
    throw new Error('Unexpected read: '+u.pathname);
  });
}
function RouterDrawer(){const [params]=useSearchParams();return <><output aria-label="导航状态">{params.toString()}</output><TargetDrawer targetRef={target} params={params} commands={commands} revision={0}/></>;}
function mount(extra:Record<string,string>){
  const query=new URLSearchParams({drawer:target,dtab:'works',domain:'all',inspect_as_of:cutoff,...extra});
  render(<MemoryRouter initialEntries={['/collection/targets?'+query]}><QueryClientProvider client={new QueryClient({defaultOptions:{queries:{retry:false}}})}>
    <RouterDrawer/>
  </QueryClientProvider></MemoryRouter>);
}
beforeEach(()=>{requests.length=0;vi.stubGlobal('requestAnimationFrame',(fn:FrameRequestCallback)=>setTimeout(()=>fn(0),0));
  vi.stubGlobal('cancelAnimationFrame',(n:number)=>clearTimeout(n));
  Object.defineProperty(HTMLElement.prototype,'scrollTo',{configurable:true,value:function(this:HTMLElement,options:ScrollToOptions){this.scrollTop=options.top??0;}});
});
afterEach(()=>{cleanup();vi.restoreAllMocks();vi.unstubAllGlobals();});
it.each([['2026-01-03','2026-01-02'],['2026-01-02','2026-01-03']])('观察按真实时间比较，勾选顺序 %s → %s',async(first,second)=>{
  installReads();mount({inspect_panel:'trace',inspect_work:work});
  fireEvent.click(await screen.findByRole('checkbox',{name:new RegExp(first)}));
  fireEvent.click(await screen.findByRole('checkbox',{name:new RegExp(second)}));
  expect(await screen.findByText(/↑ 9/)).toBeInTheDocument();expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  expect(screen.getByRole('heading',{name:'合成作品'})).toBeInTheDocument();
});
it('跨页选择保留第一次观察时间，不因倒序列表产生无效比较',async()=>{
  installReads(true);mount({inspect_panel:'trace',inspect_work:work});
  fireEvent.click(await screen.findByRole('checkbox',{name:/2026-01-03/}));fireEvent.click(screen.getByRole('button',{name:'下一页'}));
  fireEvent.click(await screen.findByRole('checkbox',{name:/2026-01-01/}));
  expect(await screen.findByText(/↑ 13/)).toBeInTheDocument();expect(screen.queryByRole('alert')).not.toBeInTheDocument();
});
it('切换较早本轮时重选其前一同规则观察，最早轮次不谎称仅有一轮',async()=>{
  installReads();mount({inspect_round_view:'compare',inspect_right_round:id(33),inspect_left_round:id(32)});
  await screen.findByText('共同命中');fireEvent.change(screen.getByRole('combobox',{name:'本轮'}),{target:{value:id(32)}});
  await waitFor(()=>expect(requests.filter(u=>u.pathname.endsWith('/round-comparison')).at(-1)?.searchParams.get('left')).toBe(id(31)));
  expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  fireEvent.change(screen.getByRole('combobox',{name:'本轮'}),{target:{value:id(31)}});
  expect(await screen.findByText(/未找到本轮之前的同规则观察/)).toBeInTheDocument();
  expect(screen.queryByText(/当前只有一轮/)).not.toBeInTheDocument();expect(screen.queryByRole('alert')).not.toBeInTheDocument();
});
it('面板内研究导航不重读和卸载目标，返回恢复触发作品与滚动位置',async()=>{
  installReads();mount({inspect_right_round:id(33)});
  const opener=await screen.findByRole('button',{name:'合成作品'});const body=document.querySelector('.c-dw-body');
  if(!(body instanceof HTMLElement))throw new Error('missing drawer body');Object.defineProperty(body,'scrollHeight',{configurable:true,value:1000});body.scrollTop=120;opener.focus();fireEvent.click(opener);
  await screen.findByRole('heading',{name:'观察轨迹'});fireEvent.click(screen.getByRole('button',{name:'返回'}));
  await waitFor(()=>expect(screen.getByRole('button',{name:'合成作品'})).toHaveFocus());
  expect(body.scrollTop).toBe(120);
  expect(requests.filter(u=>!u.pathname.includes('/inspection/'))).toHaveLength(1);
});

it('候选只有部分可读材料时不宣称详情已取得',async()=>{
  installReads(false,'creator');mount({inspect_panel:'similar',inspect_work:work});
  expect(await screen.findByText('有可读材料')).toBeInTheDocument();expect(screen.queryByText('已取得详情')).not.toBeInTheDocument();
});

it('刷新后的跨页单选无法确认时间时解释并允许清除，不发送倒置请求',async()=>{
  installReads(true);mount({inspect_panel:'trace',inspect_work:work,inspect_trace_offset:'50',inspect_left_observation:id(13)});
  fireEvent.click(await screen.findByRole('checkbox',{name:/2026-01-01/}));
  expect(await screen.findByText(/刷新后无法确认其时间/)).toBeInTheDocument();
  expect(requests.filter(u=>u.pathname.endsWith('/observation-comparison'))).toHaveLength(0);
  fireEvent.click(screen.getByRole('button',{name:'清除观察选择'}));
  fireEvent.click(screen.getByRole('checkbox',{name:/2026-01-01/}));fireEvent.click(screen.getByRole('button',{name:'上一页'}));
  fireEvent.click(await screen.findByRole('checkbox',{name:/2026-01-03/}));
  expect(await screen.findByText(/↑ 13/)).toBeInTheDocument();
});
it('相同时间的不同观察保留原值，不编造顺序或变化量',async()=>{
  installReads(false,'keyword',true);mount({inspect_panel:'trace',inspect_work:work});
  const choices=await screen.findAllByRole('checkbox');fireEvent.click(choices[0]!);fireEvent.click(choices[1]!);
  expect(await screen.findByText(/观察时点无法确立先后/)).toBeInTheDocument();
  expect(screen.queryByRole('alert')).not.toBeInTheDocument();expect(screen.queryByText(/↑/)).not.toBeInTheDocument();
});
it('上轮不能选择同轮或较新的轮次',async()=>{
  installReads();mount({inspect_round_view:'compare',inspect_right_round:id(32),inspect_left_round:id(31)});
  await screen.findByText('共同命中');const left=screen.getByRole('combobox',{name:'上轮'});
  expect(left.querySelector(`option[value="${id(33)}"]`)).toBeDisabled();
  expect(left.querySelector(`option[value="${id(32)}"]`)).toBeDisabled();
  expect(left.querySelector(`option[value="${id(31)}"]`)).not.toBeDisabled();
});
it.each([false,true])('返回时等待列表高度，用户主动操作取消恢复 %s',async(cancel)=>{
  installReads();mount({inspect_right_round:id(33),inspect_works:work+','+id(4)});
  await screen.findByRole('button',{name:'合成作品'});const opener=screen.getByRole('button',{name:'并排对照'});
  const body=document.querySelector('.c-dw-body');if(!(body instanceof HTMLElement))throw new Error('missing drawer body');
  Object.defineProperty(body,'scrollHeight',{configurable:true,get:()=>screen.queryByRole('button',{name:'合成作品'})?1000:0});
  body.scrollTop=120;opener.focus();fireEvent.click(opener);await screen.findByRole('heading',{name:'作品并排对照'});
  fireEvent.click(await screen.findByRole('button',{name:'返回原列表或图表'}));
  body.scrollTop=0;if(cancel)fireEvent.wheel(body);
  await screen.findByRole('button',{name:'合成作品'});
  if(cancel){await new Promise(resolve=>setTimeout(resolve,30));expect(body.scrollTop).toBe(0);}
  else{await waitFor(()=>expect(body.scrollTop).toBe(120));expect(screen.getByRole('button',{name:'并排对照'})).toHaveFocus();}
});
