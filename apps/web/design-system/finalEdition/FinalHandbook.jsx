import React,{useEffect,useState} from 'react';
import snapshot from '../generated/source-data.json';
import {spec,byId,region,component,interaction,patterns,pageGroups,stateLabel} from '../dsReview/data.js';
import {Button,Find,Facts,Mark,Modal,Pill} from '../dsReview/ui.jsx';
import {Select} from '../dsReview/Select.jsx';
import {ComponentDemo} from '../dsReview/ComponentDemo.jsx';
import {InteractionDemo} from '../dsReview/InteractionDemo.jsx';
import CompositionReview from '../compositionReview/CompositionReview.jsx';
import InteractionReview from '../interactionReview/InteractionReview.jsx';
import '../dsReview/review.css';
import '../compositionReview/composition.css';

const rule=snapshot.rules,type=rule.typography,color=rule.color.values;
const inventory=snapshot.inventory;
const tabs=[['standard','标准总览','01'],['regions','区域与页型',`${inventory.regions} / ${inventory.patterns}`],['pages','页面组合',`${inventory.activePages} / ${inventory.activeModes}`],['components','组件状态',String(inventory.components)],['interactions','交互规范',String(inventory.interactions)],['visual','字体与材质','B / M2'],['agent','Agent 执行','L0–L3']];
const decisions=[
 ['Q01–02','B 排版','字号、行高、字重按同一语义角色使用；窄屏不缩小字号。','visual','scale'],
 ['Q03','OPPO Sans 原声','中文界面、评论原文与访谈引文共用 Sans；来源与系统判断以结构区分。','visual','families'],
 ['Q04','C 焦点','鼠标暖底轻反馈；键盘使用内部定位，FIND 可反白，不在输入外加一圈。','visual','focus'],
 ['Q05','M2 材质','直角、有限硬阴影、边缘纹理；正文、原声和表格不铺装饰底纹。','visual','material'],
 ['Q06','A 壳层与图标折叠','领域选择、三级页面 Tab 留在面包屑模块；窄屏局部滚动，侧栏可手动折叠。','regions','Z02'],
 ['Q07','L1 单行账本',`${rule['ledger-layout'].decision.rowHeight}px 行高、任务名称 ${rule['ledger-layout'].widths.name}px；字段可自定义、局部横滚，一个任务一行。`,'pages','P09','runs'],
 ['Q08','检索 B / 功能 C','所有含搜索的列表页采用 B；其余功能页采用 C，入口表单保留其例外。','pages','P05','list'],
 ['Q09','S1 关闭与请求跟踪','保护未保存输入；在途请求收起后继续跟踪原请求；停止后不能恢复。','interactions','flow'],
 ['Q10','四类复用 / 七项晋升门槛','基础复用、页面适配、领域专属、先补合同；目录示例不自动晋升 CMP。','components','C16'],
 ['Q11','自动换入 / F2 / 数字对齐','字体 swap；Manrope 普通数字右对齐，小数列按小数点对齐，未知不写成零。','visual','numbers'],
 ['Q12','E1 / 白场媒体','保留作品卡和工位局部材质例外；媒体白场、contain、原色，不铺测量底纹。','pages','P03','collection'],
 ['U24','统一语义色板','使用已采用的 color.json；颜色表达角色，不能代替权限或业务状态。','visual','colors'],
];
const modules=[
 ['标准件同步','component-sync.md / component-contracts.json','判定实现修正、共享标准或页面例外；合同、真实组件、状态展示和HTML同次更新。'],
 ['视觉与气质','system.md','明确结构、留白和重音；延续无边框 V2。'],
 ['字体与数字','typography.json / typography.md','按角色使用完整字族、字号、字重与行高；保留未知和原始精度。'],
 ['壳层与导航','shell-layout.json / shell-zones.md','壳负责 Z01/Z02/Z03；领域和三级页面 Tab 均留 Z02。'],
 ['区域与页型','composition-layout.json / regions.md / patterns.md','先识别任务、对象与页型，再组合区域；空区省略，避免重复身份。'],
 ['基础控件与焦点','focus.json / primitives.md','查现成变体；实现键盘、指针、输入法及禁用反馈。'],
 ['材质与媒体','control-material.json / media-material.json','M2 仅在适用控件与壳边缘；E1 例外不得扩散。'],
 ['异步与浮层','overlay-policy.json / interaction-contracts.md','声明触发、未保存、提交、退出、原请求跟踪和停止后果。'],
 ['组件复用','component-reuse.json / component-promotion.md','先查四类目录；按七项门槛逐项取得证据。'],
 ['业务数据边界','data-boundaries.md / 对应 PAGE 与业务合同','区分来源与判断、执行与成果、未知与零；不从视觉推导业务资格。'],
];
function readRoute(){const [tab='standard',id='',mode='',zone='Z04-B']=decodeURIComponent(location.hash.slice(1)).split('/');return {tab:tabs.some(x=>x[0]===tab)?tab:'standard',id,mode,zone};}
function saveJson(value,name){const url=URL.createObjectURL(new Blob([JSON.stringify(value,null,2)],{type:'application/json'}));const a=document.createElement('a');a.href=url;a.download=name;a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);}
const toText=value=>typeof value==='string'?value:JSON.stringify(value);

export function FinalHandbook(){
 const [route,setRoute]=useState(readRoute),[filter,setFilter]=useState(''),[width,setWidth]=useState('auto'),[labels,setLabels]=useState(true),[state,setState]=useState('idle'),[variant,setVariant]=useState(''),[license,setLicense]=useState(false),[notice,setNotice]=useState('');
 function nav(tab,id='',mode='',zone='Z04-B'){setRoute({tab,id,mode,zone});setFilter('');location.hash=[tab,id,mode,zone].map(encodeURIComponent).join('/');}
 useEffect(()=>{const pop=()=>setRoute(readRoute());window.addEventListener('hashchange',pop);document.title='Linggan Intelligence · 当前设计系统';return()=>window.removeEventListener('hashchange',pop)},[]);
 useEffect(()=>{setState('idle');setVariant('');const frame=requestAnimationFrame(()=>{const target=['standard','visual','agent'].includes(route.tab)&&document.getElementById('lf-'+route.id);if(target)target.scrollIntoView({block:'start',behavior:'instant'});else window.scrollTo({top:0,behavior:'instant'});});return()=>cancelAnimationFrame(frame)},[route.tab,route.id,route.mode]);
 const page=byId(spec.pages,route.id==='P17'?'P13':route.id)||spec.pages.find(p=>p.id==='P05'),mode=byId(page.modes,route.mode)||page.modes[0];
 const c=component(route.id)||component('C16'),i=interaction(route.id)||interaction('I01'),z=region(route.id)||region('Z02');
 const cstate=c.states.includes(state)?state:c.states[0],cvariant=c.variants.includes(variant)?variant:c.variants[0];
 const reuse=rule['component-reuse'].components.find(x=>x.id===c.id),reuseRoute=rule['component-reuse'].routes[reuse?.route];
 const categories=[...new Set(spec.components.map(c=>c.category))];
 const match=x=>(x.id+' '+(x.label||x.name)).toLowerCase().includes(filter.toLowerCase());
 const pageActions=spec.actions.filter(a=>a.pageId===page.id);
 const palette={'--ds-ink':color.ink,'--ds-sub':color.body,'--ds-muted':color.secondary,'--ds-orange':color.mark,'--ds-orange-text':color.signal,'--ds-focus':color.info,'--lf-side':rule['shell-layout'].layout.sideWidth+'px'};
 function internalLink(event){const a=event.target.closest('a');if(!a)return;const href=a.getAttribute('href')||'';if(href.startsWith('?page=')){event.preventDefault();const q=new URLSearchParams(href.slice(1));if(q.get('review')==='overlays')nav('interactions','flow');else if(q.get('section')==='typography'){setLicense(href.includes('license'));nav('visual','families')}else if(q.get('section')==='components')nav('components',q.get('c')||'C16');else if(q.get('section')==='pages')nav('pages',q.get('p')||'P05',q.get('mode')||'');else nav('standard');}}
 const items=route.tab==='pages'?spec.pages:route.tab==='components'?spec.components:route.tab==='interactions'?[{id:'flow',name:'S1 完整流程演练'},...spec.interactions]:route.tab==='regions'?spec.regions:[];
 function selectItem(id){nav(route.tab,id)}
 return <div className="ds-review lf-shell" style={palette} onClickCapture={internalLink}>
  <header className="lf-header"><a className="lf-brand" href="#standard"><span>LI</span><strong>Linggan Intelligence</strong></a><div className="lf-edition"><i/>设计系统 · 当前确认规范<span className="ds-mono">同步构建</span><Button onClick={()=>saveJson(snapshot,'LIDS-confirmed-standard.json')}>导出规范 JSON ↗</Button></div></header>
  <nav className="lf-tabs" aria-label="最终设计规范章节">{tabs.map(([id,name,count])=><button key={id} aria-current={route.tab===id?'page':undefined} onClick={()=>nav(id)}>{name}<small>{count}</small></button>)}</nav>
  <div className={'lf-layout '+(['standard','visual','agent'].includes(route.tab)||route.id==='flow'?'lf-wide':'')}>
   <aside className="lf-index" aria-label="规范目录"><div className="lf-index-label">LIDS / CURRENT STANDARD</div>{items.length>0&&<label className="lf-index-search"><b>FIND</b><input aria-label="筛选规范目录" placeholder="名称或编号" value={filter} onChange={e=>setFilter(e.target.value)}/></label>}
    {route.tab==='pages'?pageGroups.map(([name,ids])=><section key={name}><h2>{name}</h2>{ids.map(id=>byId(spec.pages,id)).filter(p=>p.status!=='retired'&&match(p)).map(p=><IndexButton key={p.id} item={p} active={page.id===p.id} onClick={()=>nav('pages',p.id)}/>)}</section>):route.tab==='components'?categories.map(category=><section key={category}><h2>{category}</h2>{spec.components.filter(x=>x.category===category&&match(x)).map(x=><IndexButton key={x.id} item={x} active={c.id===x.id} onClick={()=>nav('components',x.id)}/>)}</section>):items.length?items.filter(match).map(x=><IndexButton key={x.id} item={x} active={route.id===x.id||(route.tab==='regions'&&z.id===x.id)||(route.tab==='interactions'&&i.id===x.id&&route.id!=='flow')} onClick={()=>selectItem(x.id)}/>):<><p className="lf-index-note">规范已经确认。按当前任务展开模块，参数只读正式 LIDS 源；不再并行选择旧方案。</p>{(route.tab==='visual'?[['families','字族分工'],['scale','B 排版角色'],['numbers','数字对齐'],['focus','C 焦点'],['material','M2 材质'],['colors','语义色板']]:route.tab==='agent'?[['entry','L0 执行入口'],['modules','L1 按需展开'],['handoff','L2 / L3 交付'],['gates','七项晋升门槛']]:[['adopted','已确认决定'],['principles','组合原则'],['sources','来源与边界']]).map(([id,name])=><a key={id} href={'#'+route.tab+'/'+id} onClick={e=>{e.preventDefault();document.getElementById('lf-'+id)?.scrollIntoView({behavior:'smooth',block:'start'})}}>{name}<span>↘</span></a>)}</>}
    <footer>规范已确认<br/>组件认证与业务接入另计</footer>
   </aside>
   <main className="lf-main" id="lf-main">
    {items.length>0&&<div className="lf-mobile-picker"><Select label="规范目录" value={route.tab==='pages'?page.id:route.tab==='components'?c.id:route.id||items[0].id} onChange={selectItem} options={items.map(x=>({value:x.id,label:x.label||x.name,meta:x.id}))}/></div>}
    {route.tab==='standard'?<>
     <Heading eyebrow="LIDS / CONFIRMED EDITION" title="一套已经确定的设计标准" text="让每个页面有清楚的区域职责，让组件和交互有同一条执行依据。沿用无边框 V2，以结构、排版与有限材质建立 Linggan 的气质。"/>
     <div className="lf-coverage">{[['当前页面',inventory.activePages],['页面模式',inventory.activeModes],['历史定位',inventory.retiredPages],['页型',inventory.patterns],['区域',inventory.regions],['组件分类',inventory.components],['交互合同',inventory.interactions]].map(([name,count])=><span key={name}><b>{count}</b>{name}</span>)}</div>
     <section id="lf-adopted"><SectionTitle number="01" title="已确认的唯一标准"/><div className="lf-decisions">{decisions.map(([id,name,desc,tab,target,mid])=><button key={id} onClick={()=>nav(tab,target,mid||'')}><span className="lf-code">{id}</span><strong>{name}</strong><span>{desc}</span><b aria-hidden="true">↗</b></button>)}</div></section>
     <section id="lf-principles"><SectionTitle number="02" title="先组合职责，再选择组件"/><div className="lf-principles">{[
      ['位置清楚','Z01 是全局，Z02 是位置与范围，Z03 是页面导航。领域选择和三级页面 Tab 留在面包屑模块。'],
      ['任务优先','检索页服务找材料与对象；概览才让关键读数成为主体。列表不因为有数据就增加大统计带。'],
      ['动作就近','集合动作归工具区，对象动作归行或详情，精确勾选动作归批量区，表单动作只提交当前表单。'],
      ['身份不重复','侧栏已经选中页面时，不再造一个同名的单项 Tab；保留可访问标题和真实对象标题。'],
      ['原声有边界','原文、系统判断与人工决定分开；未知、未分析、不适用与已知零分别表达。'],
      ['窄屏有取舍方法','重排工具、图标折叠或局部横滚；不缩字号、不把一个任务拆成多行，也不随意删除字段。'],
     ].map(([name,text],n)=><article key={name}><span className="lf-code">0{n+1}</span><div><h3>{name}</h3><p>{text}</p></div></article>)}</div></section>
     <div className="lf-callout">C09 / C16 已直接复用项目真实共享组件；其余目录为合成示例。来源指纹：{snapshot.fingerprint?.slice(0,12)}</div><SourceBoundary onLicense={()=>setLicense(true)}/>
    </>:route.tab==='regions'?<>
     <Heading eyebrow={z.id+' / REGION'} title={z.name} text={z.purpose}/>
     <div className="lf-region-map" aria-label="页面区域职责示意">{['Z01','Z02','Z03','Z04-T','Z04-V','Z04-R','Z04-B','Z04-F','Z04-P','Z05-H','Z05-N','Z05-B','Z05-A'].filter(id=>region(id)).map(id=><button key={id} className={id===z.id?'active':''} onClick={()=>nav('regions',id)}><b>{id}</b>{region(id)?.name}</button>)}</div>
     <Facts items={[["允许内容",z.allow],["禁止内容",z.exclude],["位置",z.position||'依本页模式组合，不机械制造空区']]}/>
     <section><SectionTitle number="PT" title="11 种页型如何组合"/>{spec.patterns.map(p=><details className="lf-pattern" key={p.id}><summary><b>{p.id}</b><strong>{p.name}</strong><span>{p.task}</span></summary><p>{p.composition}</p><p>{p.layout}</p><div className="lf-chips">{spec.pages.filter(page=>page.status!=='retired').flatMap(page=>page.modes.filter(m=>m.patternId===p.id).map(m=><Button key={page.id+m.id} onClick={()=>nav('pages',page.id,m.id)}>{page.name} · {m.name} ↗</Button>))}</div></details>)}</section>
    </>:route.tab==='pages'?<>
     <Heading eyebrow={page.id+' / '+mode.patternId+' · '+patterns[mode.patternId]?.name} title={page.name} text={page.corePriority}/>
     <div className="lf-tools"><label>页面模式<Select label="页面模式" value={mode.id} onChange={mid=>nav('pages',page.id,mid)} options={page.modes.map(m=>({value:m.id,label:m.name}))}/></label><label>预览宽度<Select label="预览宽度" value={width} onChange={setWidth} options={['auto','1440','1024','820','390'].map(v=>({value:v,label:v==='auto'?'适应容器':v+'px'}))}/></label><label className="lf-check"><input type="checkbox" checked={labels} onChange={e=>setLabels(e.target.checked)}/>区域标注</label><Mark tone="orange">{rule['composition-layout'].modes[page.id+':'+mode.id]?.variant} 组合 · 已确认</Mark></div>
     <div className="lf-preview-board"><CompositionReview key={page.id+':'+mode.id} embedded sceneId={page.id+':'+mode.id} selectedRegion={route.zone} onRegion={zone=>nav('pages',page.id,mode.id,zone)} showLabels={labels} previewWidth={width} onPageNavigate={(id,mid)=>nav('pages',id,mid||'')}/></div>
     <p className="lf-caption">点击预览区域查看职责。这里展示当前组合与合成交互，不作为真实业务执行回执。</p>
     <div className="lf-chips">{mode.zoneOrder.map(id=><Pill key={id} active={route.zone===id} onClick={()=>nav('pages',page.id,mode.id,id)}>{id} · {region(id)?.name}</Pill>)}</div>
     <section><SectionTitle number="A" title="本页操作与读数"/><Facts items={[["读数规则",page.metricRule],["操作规则",page.actionRule]]}/>{pageActions.map(a=><div className="lf-action-rule" key={a.id}><strong>{a.label}</strong><span>{a.zone} · {({collection:'当前集合',object:'当前对象',form:'当前表单',selection:'精确勾选集合',collection_configuration:'集合配置'})[a.owner]||a.owner}</span><p>{a.contract}</p></div>)}</section>
    </>:route.tab==='components'?<>
     <Heading eyebrow={c.id+' / '+c.category} title={c.label} text={c.name}/><div className="lf-tools"><label>合法变体<Select label="组件变体" value={cvariant} onChange={setVariant} options={c.variants}/></label><Mark>{reuseRoute?.label||'先补合同'}</Mark><span className="lf-caption">分类合同与合成样例 · 不自动晋升正式 CMP</span></div>
     {snapshot.sharedComponents?.[c.id]&&<details className="lf-live-contract"><summary><Mark tone="orange">真实组件 · 同源展示 · 源码与合同</Mark></summary><Facts items={[["实现",snapshot.sharedComponents[c.id].implementation],["样式",snapshot.sharedComponents[c.id].styles],["状态",snapshot.sharedComponents[c.id].states.join(' / ')],["原生交互",snapshot.sharedComponents[c.id].nativeStates?.join(' / ')],["测试",snapshot.sharedComponents[c.id].tests.join('、')]]}/></details>}<div className="ds-states" role="group" aria-label="组件状态">{c.states.map(s=><button key={s} aria-pressed={cstate===s} className={cstate===s?'active':''} onClick={()=>setState(s)}>{stateLabel(s)}<small>{s}</small></button>)}</div>
     <div className="ds-component-stage lf-component-stage"><ComponentDemo key={c.id+cvariant+cstate} c={c} variant={cvariant} state={cstate}/></div>
     <section><SectionTitle number="C" title="结构、职责与复用边界"/><Facts items={[["结构",c.anatomy],["负责",c.owns],["不负责",c.excludes],["复用方式",reuseRoute?.description||'先补输入与合同'],['具体边界',reuse?.boundary||c.excludes]]}/></section>
     <section><SectionTitle number="I" title="关联交互"/><div className="lf-chips">{c.interactionIds.map(id=><Button key={id} onClick={()=>nav('interactions',id)}>{id} · {interaction(id)?.name} ↗</Button>)}</div></section>
    </>:route.tab==='interactions'?(route.id==='flow'?<><Heading eyebrow="S1 / OPERABLE FLOW" title="浮层、离开与长任务" text="未保存保护、在途请求收起、核对原请求回执，以及停止后不可恢复。手动送达合成回执，画面不会冒充真实成功。"/><div className="lf-flow"><InteractionReview embedded/></div></>:<><Heading eyebrow={i.id+' / INTERACTION'} title={i.name} text={i.trigger}/><Button tone="primary" onClick={()=>nav('interactions','flow')}>打开 S1 完整流程演练 ↗</Button><div className="lf-interaction-demo"><InteractionDemo key={i.id} item={i}/></div><section><SectionTitle number="I" title="执行合同"/><Facts items={[["正常结果",i.expected],["失败分支",i.failure],["流程",i.flow],["返回与取消",i.returnAndCancel],["边界场景",i.boundaryCases]]}/></section></>):route.tab==='visual'?<VisualStandards onNotice={setNotice} onLicense={()=>setLicense(true)}/>:<>
     <Heading eyebrow="AGENT / PROGRESSIVE DISCLOSURE" title="把设计系统变成执行标准" text="每次任务先读短入口，再按命中的模块展开。把页面、区域、组件、状态和证据串成一份可验收的变更说明。"/>
     <section id="lf-entry"><SectionTitle number="L0" title="每次 UI 任务的短入口"/><ol className="lf-steps"><li>确认用户结果、当前授权、checkout 与 HEAD；读取项目 AGENTS.md、docs/README.md、docs/current-state.md。</li><li>读取 docs/governance/ui-design-execution.md 与 docs/design/lids/README.md，识别受影响的页面 / 模式及业务对象。</li><li>声明区域组合、复用组件、状态与直接共享依赖；只展开下方命中的模块。</li></ol></section>
     <section id="lf-modules"><SectionTitle number="L1" title="按模块展开，不通读整个目录"/>{modules.map(([name,file,requirement])=><article className="lf-module" key={name}><h3>{name}</h3><code>{file}</code><p>{requirement}</p></article>)}</section>
     <section id="lf-handoff"><SectionTitle number="L2 / L3" title="从页面合同到验收交付"/><ol className="lf-steps"><li>读取对应 PAGE、产品 / 数据合同；P / PT / Z / C / B / I 是定位坐标，不能代替真实合同。</li><li>提交一份 UI Change Manifest：用户结果、页面 / 区域、标准来源、组件复用、动作作用对象、状态矩阵、文件范围。</li><li>验证常态、空、加载、错误、部分、受限、未确认；核对中文输入法、键盘、长文本、窄屏、缩放与返回位置。</li><li>分别记录：设计规则确认、组件样例、正式 CMP、业务接入、主线合并、运行验证和用户验收。不得跨层宣称完成。</li></ol></section>
     <section id="lf-sync"><SectionTitle number="SYNC" title="修改标准件后同步"/><ol className="lf-steps"><li>分类：实现修正 / 已授权共享标准 / 有边界的页面例外；读取命中组件合同和调用方。</li><li>更新唯一规则或真实组件，补齐对应状态与测试；用 lids:record 显式登记分类、理由与授权来源。</li><li>运行 lids:sync，再运行 lids:check 与定向回归。前端构建和 CI 检查生成漂移，HTML按本次来源指纹再生。</li></ol><p>当前闭合 C09 / C16；其余目录仍按四类复用与七门槛逐项接入。源指纹：{snapshot.fingerprint}</p></section><section id="lf-gates"><SectionTitle number="07" title="组件晋升的七项门槛"/>{rule['component-reuse'].gates.map((g,n)=><div className="lf-gate" key={g}><b>0{n+1}</b><p>{g}</p></div>)}</section>
     <p className="lf-callout">HTML 是当前规则的交互导出。AI 修改数值前必须回到项目正式 JSON；不在 HTML、页面 CSS 和治理入口各维护一套值。</p>
    </>}
    {notice&&<div className="lf-callout" role="status">{notice}<Button tone="quiet" onClick={()=>setNotice('')}>收起</Button></div>}
   </main>
   {!['standard','visual','agent'].includes(route.tab)&&route.id!=='flow'&&<aside className="lf-inspector" aria-label="当前条目合同"><div className="lf-index-label">CONTRACT / 当前依据</div>{route.tab==='pages'?<><span className="lf-code">{route.zone}</span><h2>{region(route.zone)?.name||'主体业务内容'}</h2><p>{region(route.zone)?.purpose}</p><InspectorBlock title="可以放">{region(route.zone)?.allow}</InspectorBlock><InspectorBlock title="不能放" danger>{region(route.zone)?.exclude}</InspectorBlock><InspectorBlock title="本页的实际内容">{page.regionContents.find(x=>x.id===route.zone)?.content||'共享职责按需出现；无内容不预留空白。'}</InspectorBlock><InspectorBlock title="适用组件"><div className="lf-chips">{mode.componentIds.filter(id=>region(route.zone)?.allowedComponentIds.includes(id)).map(id=><Pill key={id} onClick={()=>nav('components',id)}>{id}</Pill>)}</div></InspectorBlock><InspectorBlock title="本区域业务块">{mode.businessIds.map(id=>byId(spec.businessBlocks,id)).filter(b=>b?.zoneIds.includes(route.zone)).map(b=><div className="lf-business-block" key={b.id}><strong>{b.id} · {b.name}</strong><p>{b.contract}</p><small>{b.excludes}</small></div>)}{!mode.businessIds.some(id=>byId(spec.businessBlocks,id)?.zoneIds.includes(route.zone))&&'没有专属业务块；按共享区域合同组合。'}</InspectorBlock><InspectorBlock title="最低交互清单"><div className="lf-chips">{mode.requiredInteractionIds.map(id=><Pill key={id} onClick={()=>nav('interactions',id)}>{id}</Pill>)}</div></InspectorBlock></>:route.tab==='components'?<><span className="lf-code">{c.id}</span><h2>{reuseRoute?.label}</h2><p>{reuseRoute?.description}</p><InspectorBlock title="允许区域">{c.allowedZones.map(id=><button className="lf-text-link" key={id} onClick={()=>nav('regions',id)}>{id} · {region(id)?.name}</button>)}</InspectorBlock><InspectorBlock title="来源页面"><div className="lf-chips">{c.sourcePageIds.map(id=><Pill key={id} onClick={()=>nav('pages',id)}>{id}</Pill>)}</div></InspectorBlock><InspectorBlock title="认证边界" danger>48 项是组件分类与复用路线；本次导出没有授予正式 CMP 认证。七项门槛不变。</InspectorBlock></>:route.tab==='interactions'?<><span className="lf-code">{i.id}</span><h2>最低证据</h2>{i.minimumEvidence.map(x=><p key={x}>— {x}</p>)}<InspectorBlock title="相关组件"><div className="lf-chips">{i.componentIds.map(id=><Pill key={id} onClick={()=>nav('components',id)}>{id}</Pill>)}</div></InspectorBlock><InspectorBlock title="适用页面"><div className="lf-chips">{i.pages.map(id=><Pill key={id} onClick={()=>nav('pages',id)}>{id}</Pill>)}</div></InspectorBlock></>:<><span className="lf-code">{z.id}</span><h2>组合边界</h2><p>区域是职责坐标，不是强制固定行数。区域可同带组合，空区省略。</p><InspectorBlock title="允许组件"><div className="lf-chips">{z.allowedComponentIds.map(id=><Pill key={id} onClick={()=>nav('components',id)}>{id}</Pill>)}</div></InspectorBlock><InspectorBlock title="固定导航规则">领域选择与三级页面导航留在 Z02。集合视图归 Z04-V；对象内部分区归 Z05-N。</InspectorBlock></>}</aside>}
  </div>
  <footer className="lf-footer"><span>Linggan Intelligence · 已确认规范快照 · 2026-10-20</span><span>合成演示 / 无业务请求</span><button onClick={()=>setLicense(true)}>字体许可与鸣谢 ↗</button></footer>
  {license&&<Modal title="字体许可与鸣谢" onClose={()=>setLicense(false)} footer={<Button onClick={()=>setLicense(false)}>关闭</Button>}><p>本软件使用 OPPO Sans、Albert Sans、Alumni Sans、Manrope 与 JetBrains Mono。下列许可为原件内容，字体未转换或改写；完整原件随规范包提供。</p>{snapshot.licenses.map(l=><details className="lf-license" key={l.file}><summary>{l.file}</summary><pre>{l.text}</pre><small>SHA-256: {l.sha256}</small></details>)}</Modal>}
 </div>;
}
function IndexButton({item,active,onClick}){return <button className={'lf-index-button '+(active?'active':'')} aria-current={active?'page':undefined} onClick={onClick}><span>{item.id}</span><b>{item.label||item.name}</b></button>}
function Heading({eyebrow,title,text}){return <div className="lf-heading"><div className="lf-code">{eyebrow}</div><h1>{title}</h1><p>{text}</p></div>}
function SectionTitle({number,title}){return <h2 className="lf-section-title"><span>{number}</span>{title}</h2>}
function InspectorBlock({title,danger,children}){return <section className={'lf-inspector-block '+(danger?'danger':'')}><h3>{title}</h3><div>{children}</div></section>}
function SourceBoundary({onLicense}){return <section id="lf-sources"><SectionTitle number="03" title="来源与交付边界"/><p>本版汇总已确认的 Q01–Q12 及统一语义色板。规则取自项目正式 LIDS；页面、组件与交互复用既有独立演示，不连接业务接口。</p><div className="lf-source-grid">{snapshot.sources.map(s=><details key={s.path}><summary>{s.path.split('/').pop()}</summary><code>{s.sha256}</code></details>)}</div><p className="lf-callout">设计规则已确认。17 页 / 26 模式属于盘点与示例覆盖，48 项属于组件分类；它们不等同于全部业务页面已完成迁移或全部组件已认证。业务接入与验收记录另行核对。</p><Button onClick={onLicense}>查看字体许可与鸣谢 ↗</Button></section>}
function VisualStandards({onNotice,onLicense}){
 const [sample,setSample]=useState('让真实材料成为判断的依据。 Designing with Linggan 0123456789'),[query,setQuery]=useState(''),[materialScope,setMaterialScope]=useState('current');
 const families=[['中文界面 / 原声','chinese','OPPO Sans','原声、评论原文、中文正文'],['英文界面','english','Albert Sans','英文界面与辅助说明'],['英文展示','display','Alumni Sans','足够大的英文展示标题'],['数据与时间','data','Manrope','普通数字、编号、日期与对齐列'],['技术代码','code','JetBrains Mono','代码、诊断片段；不覆盖普通数据']];
 return <><Heading eyebrow="FOUNDATIONS / B · C · M2 · E1" title="字体、排版与材质共用一套参数" text="字族按角色分工，字号、行高与字重成组使用。颜色与纹理服务层级和交互，普通内容保留白色连续工作面。"/>
 <section id="lf-families"><SectionTitle number="01" title="已选字族"/><label className="lf-sample-input">更换样本文字<input value={sample} onChange={e=>setSample(e.target.value)}/></label>{families.map(([label,key,name,usage])=><article className="lf-font-row" key={key}><div><h3>{label}</h3><b>{name}</b><p>{usage}</p></div><div style={{fontFamily:type.families[key],fontSize:key==='display'?40:22,fontWeight:key==='display'?300:400,lineHeight:1.4}}>{key==='display'?'Designing with Linggan':sample}</div></article>)}<p>加载后自动换入，先显示 F2 可读回退。使用真实字重，禁止合成加粗。品牌字标保留既有专用角色。</p><Button onClick={onLicense}>字体许可与鸣谢 ↗</Button></section>
 <section id="lf-scale"><SectionTitle number="02" title="B 排版角色"/><div className="lf-table-scroll"><table className="lf-type-table"><thead><tr><th>语义角色</th><th>字号</th><th>字重</th><th>行高</th><th>效果样例</th></tr></thead><tbody>{Object.entries(type.roles).map(([key,v])=><tr key={key}><th>{type.roleLabels[key]}</th><td>{v.size}px</td><td>{v.weight}</td><td>{v.lineHeight}</td><td style={{fontSize:v.size,fontWeight:v.weight,lineHeight:v.lineHeight,fontFamily:key.startsWith('display')?type.families.display:['data','metric','metricLarge'].includes(key)?type.families.data:type.families.ui}}>{key.startsWith('display')?'Intelligence':key==='data'||key.startsWith('metric')?'17,212':'从材料到判断'}</td></tr>)}</tbody></table></div></section>
 <section id="lf-numbers"><SectionTitle number="03" title="数字对齐与未知"/><div className="lf-number-examples"><article><h3>普通数字 · 右对齐</h3>{[['样本一','17,212'],['样本二','885'],['已知为零','0'],['尚未取得','未知']].map(([k,v])=><div key={k}><span>{k}</span><b>{v}</b></div>)}</article><article><h3>小数列 · 小数点对齐</h3>{['1284.20','67.125','9.0','0','未知'].map(v=><div key={v}><span>原值</span>{v==='未知'?<b>未知</b>:<b className="lf-decimal"><span>{v.split('.')[0]}</span><span>{v.includes('.')?'.':''}</span><span>{v.split('.')[1]||''}</span></b>}</div>)}</article></div><p>使用 Manrope 与等宽数字特性。小数列保留原有精度，不补造小数位；未知、未分析、不适用与 0 分开。</p></section>
 <section id="lf-focus"><SectionTitle number="04" title="C 焦点 · 鼠标轻，键盘清楚"/><Find value={query} onChange={setQuery} label="焦点与 FIND 演示"/><p className="lf-caption">点入输入框，再用 Tab / Shift+Tab 比较。FIND 是识别装饰；作用范围下拉保留同一材质，不在外侧增加线圈。</p><label className="ds-field lf-field">表单字段<input placeholder="点击或用键盘定位"/><small>鼠标暖底；键盘内部定位。错误与禁用仍保留各自语义。</small></label></section>
 <section id="lf-material"><SectionTitle number="05" title="M2 材质 · 集中在可操作边缘"/><div className="lf-chips"><Button tone="primary" onClick={()=>onNotice('主动作仅为材质演示，没有执行采集。')}>单次采集</Button><Button onClick={()=>onNotice('次动作仅在本页演示。')}>核对范围</Button><Button tone="quiet" onClick={()=>onNotice('文字操作用于低强调或对象内动作。')}>查看依据 ↗</Button><Button disabled>当前不可操作</Button><Select label="材质演示下拉" value={materialScope} onChange={setMaterialScope} options={[{value:'current',label:'当前范围'},{value:'other',label:'另一合成选项'}]}/></div><div className="lf-parameter-grid">{Object.entries(rule['control-material'].parameters).map(([k,v])=><div key={k}><code>{k}</code><b>{String(v)}{typeof v==='number'&&!k.toLowerCase().includes('opacity')?'px':''}</b></div>)}</div><p>纹理不进入原声、正文或表格底。作品卡保留 M-06，工位保留受控 M-02E 边缘；媒体舞台用白场、contain 和原色。</p></section>
 <section id="lf-colors"><SectionTitle number="06" title="统一语义色板"/><div className="lf-colors">{Object.entries(color).map(([name,value])=><article key={name}><div style={{background:value}}/><code>{name}</code><b>{value}</b></article>)}</div></section>
 </>;
}
