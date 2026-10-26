import { cleanup,fireEvent,render,screen,waitFor,within } from '@testing-library/react';
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
function installReads(paged=false,kind='keyword',equalTime=false,recent=false,catalogCount=1,roundVariant='regular'){
  const availableRounds=roundVariant==='unknown'?rounds.map(round=>({...round,ruleRef:null})):rounds;
  const observed=equalTime?observations.map(o=>({...o,observedAt:observations[0]!.observedAt,observedEpochMs:observations[0]!.observedEpochMs})):observations;
  vi.spyOn(globalThis,'fetch').mockImplementation(async (path,init)=>{
    expect(init?.method).toBe('GET');const u=new URL(String(path),'http://localhost');requests.push(u);
    if(!u.pathname.includes('/inspection/'))return answer({...drawer,target:{...drawer.target,kind},catalog:Array.from({length:catalogCount},(_,index)=>({...drawer.catalog[0],publicRef:index===0?work:id(100+index),title:index===0?'合成作品':`目录作品${index+1}`}))});
    if(u.pathname.endsWith('/work-comparison')){
      const values:Record<string,number>={[work]:9,[id(4)]:17,[id(5)]:18};
      const ordered=(u.searchParams.get('works')??'').split(',');
      const mode=u.searchParams.get('mode')??'current';
      const refs=ordered.slice(1).map(id=>values[id]??0).sort((a,b)=>a-b);
      const median=mode==='age_24h'||refs.length===0?null:((refs[Math.floor((refs.length-1)/2)]??0)+(refs[Math.ceil((refs.length-1)/2)]??0))/2;
      return answer({targetRef:target,knowledgeCutoff:cutoff,methodVersion:'v1',mode,ageHours:mode==='age_24h'?24:null,toleranceMinutes:null,
        items:ordered.map(id=>({workRef:id,status:'unavailable',resource:null,metrics:mode==='age_24h'?Object.fromEntries(['likes','comments','collects','shares'].map(key=>[key,{value:null,state:'unknown',observationRef:null,observedAt:null,observedEpochMs:null,sourceKind:null,precision:'unknown'}])):metrics(values[id]??0,observations[0]!.observedEpochMs,id),publishedEpochMs:null,reasons:mode==='age_24h'?['no_observation_near_24_hours']:[]})),
        referenceStatistics:Object.fromEntries(['likes','comments','collects','shares'].map(key=>[key,{median,count:mode==='age_24h'?0:refs.length}]))});
    }
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
    if(u.pathname.endsWith('/rounds'))return answer({targetRef:target,domainRef:null,knowledgeCutoff:cutoff,methodVersion:'v1',rounds:roundVariant==='paged'?(u.searchParams.get('offset')==='50'?availableRounds.slice(2):availableRounds.slice(0,2)):availableRounds,nextOffset:roundVariant==='paged'&&u.searchParams.get('offset')!=='50'?50:null});
    if(u.pathname.endsWith('/round-comparison')){
      const left=availableRounds.find(o=>o.roundRef===u.searchParams.get('left')),right=availableRounds.find(o=>o.roundRef===u.searchParams.get('right'));
      if(!left||!right||left.observedEpochMs>right.observedEpochMs)return answer({code:'left_round_must_not_be_newer'},400);
      return answer({targetRef:target,knowledgeCutoff:cutoff,methodVersion:'v1',left,right,comparisonState:recent?'limited':'comparable',reasons:recent?['right_round_incomplete']:[],rows:recent?[19,0,null].map((delta,index)=>({workRef:id(50+index),membership:'common',discoveryLabel:'repeat',
        left:{workRef:id(50+index),title:`变化作品${index}`,creator:null,position:1,metrics:observations[1]!.metrics,firstSeenInScope:false,libraryPresence:'existing',seenBeforeLeft:true},
        right:{workRef:id(50+index),title:`变化作品${index}`,creator:null,position:1,metrics:observations[0]!.metrics,firstSeenInScope:false,libraryPresence:'existing',seenBeforeLeft:true},
        metrics:Object.fromEntries(['likes','comments','collects','shares'].map(key=>[key,{left:observations[1]!.metrics.likes,right:observations[0]!.metrics.likes,delta,elapsedSeconds:86400,percent:null,comparable:delta!==null,reason:delta===null?'metric_not_observed':null}]))})):[],
        commonCount:1,enteredCount:0,absentCount:0,truncated:false});
    }
    const current=availableRounds.find(r=>u.pathname.endsWith('/rounds/'+r.roundRef));
    if(current){
      await new Promise(resolve=>setTimeout(resolve,15));
      return answer({targetRef:target,knowledgeCutoff:cutoff,methodVersion:'v1',round:current,
        works:[{workRef:work,title:'合成作品',creator:null,position:1,metrics:observations[0]!.metrics,firstSeenInScope:recent,libraryPresence:roundVariant==='unknown'?'unknown':'existing',seenBeforeLeft:true}],truncated:false});
    }
    throw new Error('Unexpected read: '+u.pathname);
  });
}
function RouterDrawer(){const [params]=useSearchParams();return <><output aria-label="导航状态">{params.toString()}</output><TargetDrawer targetRef={target} params={params} commands={commands} revision={0}/></>;}
function mount(extra:Record<string,string>){
  const query=new URLSearchParams({drawer:target,dtab:'works',domain:'all',inspect_keyword_view:'rounds',inspect_as_of:cutoff,...extra});
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
  await screen.findByText('共同命中');fireEvent.click(screen.getByRole('combobox',{name:'本轮'}));fireEvent.click(screen.getByRole('option',{name:/2026-01-02/}));
  await waitFor(()=>expect(requests.filter(u=>u.pathname.endsWith('/round-comparison')).at(-1)?.searchParams.get('left')).toBe(id(31)));
  expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('combobox',{name:'本轮'}));fireEvent.click(screen.getByRole('option',{name:/2026-01-01/}));
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
  await screen.findByText('共同命中');fireEvent.click(screen.getByRole('combobox',{name:'上轮'}));
  expect(screen.getByRole('option',{name:/2026-01-03/})).toHaveAttribute('aria-disabled','true');
  expect(screen.getByRole('option',{name:/2026-01-02/})).toHaveAttribute('aria-disabled','true');
  expect(screen.getByRole('option',{name:/2026-01-01/})).not.toHaveAttribute('aria-disabled');
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


it.each(['creator','keyword'])('默认首屏读取%s真实轮次，展示首次发现、可比差值与覆盖限制，仅发GET',async kind=>{
  installReads(false,kind,false,true);mount({dtab:'overview'});
  expect(await screen.findByRole('link',{name:'最近观察'})).toHaveAttribute('aria-current','page');
  expect(await screen.findByText('+19')).toBeInTheDocument();
  expect(screen.getByText('本轮只取得部分结果。')).toBeInTheDocument();
  expect(await screen.findByRole('button',{name:'合成作品'})).toBeInTheDocument();
  expect(screen.queryByRole('button',{name:'变化作品1'})).not.toBeInTheDocument();
  expect(screen.queryByRole('button',{name:'变化作品2'})).not.toBeInTheDocument();
  expect(requests.filter(u=>u.pathname.endsWith('/rounds'))).toHaveLength(1);
  expect(commands.run).not.toHaveBeenCalled();
});
it('最近观察进入本轮明细与轨迹后返回原首屏，Esc先关闭共享下拉',async()=>{
  installReads(false,'creator',false,true);mount({dtab:'overview'});
  const workButton=await screen.findByRole('button',{name:'合成作品'});
  workButton.focus();fireEvent.click(workButton);await screen.findByRole('heading',{name:'观察轨迹'});
  fireEvent.click(screen.getByRole('button',{name:'返回'}));
  await waitFor(()=>expect(screen.getByRole('button',{name:'合成作品'})).toHaveFocus());
  fireEvent.click(screen.getByRole('combobox',{name:'选择观察轮次'}));
  fireEvent.keyDown(screen.getByRole('combobox',{name:'选择观察轮次'}),{key:'Escape'});
  expect(screen.queryByRole('listbox')).not.toBeInTheDocument();expect(screen.getByRole('link',{name:'关闭'})).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button',{name:'本轮作品'}));
  expect(await screen.findByRole('heading',{name:'历史轮次'})).toBeInTheDocument();
  expect(await screen.findByRole('button',{name:'合成作品'})).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button',{name:'返回'}));
  expect(await screen.findByRole('region',{name:'最近观察'})).toBeInTheDocument();
  expect(requests.filter(u=>!u.pathname.includes('/inspection/'))).toHaveLength(1);
});
it('主作品切换重排请求并读取新的参照统计，24小时未知不回退当前值',async()=>{
  installReads();mount({inspect_panel:'compare',inspect_works:[work,id(4),id(5)].join(',')});
  expect(await screen.findByText('17.5')).toBeInTheDocument();
  fireEvent.click(screen.getByRole('combobox',{name:'主作品'}));
  fireEvent.click(within(screen.getByRole('listbox',{name:'主作品'})).getByRole('option',{name:'作品 2'}));
  expect(await screen.findByText('13.5')).toBeInTheDocument();
  expect(requests.filter(u=>u.pathname.endsWith('/work-comparison')).at(-1)?.searchParams.get('works')).toBe([id(4),work,id(5)].join(','));
  fireEvent.click(screen.getByRole('button',{name:'发布后 24 小时'}));
  expect(await screen.findAllByText('无发布后 24 小时附近的真实记录')).toHaveLength(3);
  expect(screen.queryByText('13.5')).not.toBeInTheDocument();
  expect(commands.run).not.toHaveBeenCalled();
});

it('作品分页与筛选保留跨页选择，不重新读取目标，巡查隐藏选择条',async()=>{
  installReads(false,'keyword',false,false,31);mount({inspect_keyword_view:'catalog'});
  fireEvent.click(await screen.findByRole('checkbox',{name:'将合成作品加入对照'}));
  fireEvent.click(screen.getByRole('button',{name:'下一页'}));
  fireEvent.click(await screen.findByRole('checkbox',{name:'将目录作品26加入对照'}));
  fireEvent.change(screen.getByRole('searchbox'),{target:{value:'目录作品31'}});
  fireEvent.click(screen.getByRole('button',{name:'筛选'}));
  expect(await screen.findByRole('button',{name:'目录作品31'})).toBeInTheDocument();
  expect(screen.getByLabelText('作品对照选择')).toHaveTextContent('已选 2 / 3 篇');
  expect(screen.queryByRole('checkbox',{name:'将合成作品加入对照'})).not.toBeInTheDocument();
  expect(requests.filter(u=>!u.pathname.includes('/inspection/'))).toHaveLength(1);
  fireEvent.click(screen.getByRole('link',{name:'巡查'}));
  await screen.findByText('巡检规则');expect(screen.queryByLabelText('作品对照选择')).not.toBeInTheDocument();
});

it('历史规则与入库先后未知时只说本目标首次发现，不自动比较未知规则',async()=>{
  installReads(false,'creator',false,true,1,'unknown');mount({dtab:'overview'});
  await screen.findByRole('button',{name:'合成作品'});
  expect(screen.getByRole('heading',{name:'本目标首次发现'})).toBeInTheDocument();
  expect(screen.getByText(/入库先后未确认/)).toBeInTheDocument();
  expect(screen.queryByText(/库内已有/)).not.toBeInTheDocument();
  expect(requests.filter(u=>u.pathname.endsWith('/round-comparison'))).toHaveLength(0);
});
it.each(['overview','works'])('轮次分页%s选择新页首轮，不把旧本轮和跨页远轮比较',async dtab=>{
  installReads(false,'keyword',false,true,1,'paged');mount({dtab,inspect_round_view:'compare'});
  await screen.findByText('共同命中');fireEvent.click(screen.getByRole('button',{name:'更早轮次'}));
  await waitFor(()=>expect(screen.getByLabelText('导航状态')).toHaveTextContent('inspect_right_round='+id(31)));
  expect(requests.filter(u=>u.pathname.endsWith('/round-comparison')).some(u=>u.searchParams.get('right')===id(33)&&u.searchParams.get('left')===id(31))).toBe(false);
});
