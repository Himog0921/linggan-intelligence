'use strict';
const DEMO = {
  scope: '小红书 · 图文 · 近 90 天 · 合成统计 v1',
  topics: [
    {id:'routine',name:'晚间作业流程',children:5,works:320,authors:46,hits:32,hitAuthors:18,median:62,p90:1080,angle:'完整流程与条件选择',question:'时间不足时，哪些环节可以省略？'},
    {id:'reward',name:'奖励与坚持',children:4,works:40,authors:6,hits:8,hitAuthors:1,median:85,p90:2400,angle:'奖励工具与过程记录',question:'有了工具，为什么仍难以持续？'},
    {id:'remind',name:'家长提醒方式',children:3,works:160,authors:28,hits:24,hitAuthors:12,median:74,p90:1540,angle:'场景对照与回应方式',question:'怎样减少反复提醒的精力成本？'},
    {id:'screen',name:'屏幕时间约定',children:2,works:86,authors:19,hits:6,hitAuthors:4,median:43,p90:780,angle:'约定、冲突与执行条件',question:'不同照顾者的规则怎样保持一致？'},
    {id:'homework',name:'任务启动与持续',children:4,works:114,authors:23,hits:11,hitAuthors:8,median:56,p90:930,angle:'开始之前与开始之后',question:'开始之后仍需要陪伴，是同一类问题吗？'},
    {id:'unknown',name:'共同照顾中的协作与差异：不同家庭条件下的执行方式',children:0,works:0,authors:null,hits:null,hitAuthors:null,median:null,p90:null,angle:'观察不足',question:'尚无合格样本'}
  ],
  regions: [
    {id:'learn',name:'学习与执行',x:-3.8,z:-2.4,topic:'homework',note:'启动与持续出现值得区分的表达',kind:'new'},
    {id:'family',name:'家庭与陪伴',x:0,z:-2.1,topic:'remind',note:'需要复核持续提醒的执行成本',kind:'new'},
    {id:'emotion',name:'情绪与关系',x:3.5,z:-1.8,topic:'screen',note:'当前快照没有合格新发现',kind:'quiet'},
    {id:'daily',name:'日常流程',x:-3.5,z:1.4,topic:'routine',note:'完成一份细分方向比较',kind:'new'},
    {id:'methods',name:'方法与工具',x:0.2,z:1.8,topic:'reward',note:'高表现集中在一位作者，值得展开',kind:'quiet'},
    {id:'cooperate',name:'共同照顾',x:3.7,z:1.8,topic:'unknown',note:'观察不足，尚不能判断',kind:'unknown'}
  ],
  findings: [
    {id:'f1',region:'learn',title:'“开始不了”与“开始后停下”，可能需要分开理解',text:'合成样本中出现了两类不同场景。当前解释还需要其他独立来源检验。',topic:'homework'},
    {id:'f2',region:'methods',title:'奖励方向的高表现，集中在一位作者',text:'8 篇高表现作品来自同一位作者；可以展开其他作者和普通作品做对照。',topic:'reward'},
    {id:'f3',region:'daily',title:'三个细分方向的比较已有可读结果',text:'作品、作者分布和用户问题已放在同一张比较表里。',topic:'routine'}
  ],
  problems: [
    {id:'time',name:'家长每天可投入的时间有限',expressions:28,works:12,authors:7,quote:'我下班还要做饭，无法把每个环节都做完。',topic:'routine'},
    {id:'repeat',name:'需要持续提醒，难以独立推进',expressions:19,works:8,authors:5,quote:'能开始，但我一离开就停了。',topic:'homework'},
    {id:'keep',name:'开始尝试后，执行难以持续',expressions:16,works:7,authors:4,quote:'表格是做好了，第三天就没有再用了。',topic:'reward'}
  ],
  works: [
    {id:'w1',published:'2026.09.04',title:'把晚间流程分成可选择的三个环节',author:'样本作者 A',topic:'routine',likes:1450,body:'【合成内容】按家庭可用时间分别展示完整流程和保留关键环节的做法，用条件选择组织内容。',media:'正文已取得 · 图片待取得'},
    {id:'w2',published:'2026.09.02',title:'一张奖励表的七天使用记录',author:'样本作者 B',topic:'reward',likes:2680,body:'【合成内容】记录奖励表的使用过程和停止使用的节点。它呈现个人经历，不能证明方法效果。',media:'正文已取得 · 评论窗口部分'},
    {id:'w3',published:'2026.09.05',title:'孩子坐下之后，家长的提醒发生在哪里',author:'样本作者 C',topic:'remind',likes:1280,body:'【合成内容】以同一个晚间场景，对照不同提醒的发生位置与表达方式。',media:'正文已取得 · 媒体未取得'},
    {id:'w4',title:'不同照顾者对屏幕时间的约定',author:null,topic:'screen',likes:0,body:'【合成内容】讨论照顾者之间如何记录并理解约定，不包含有效性的事实结论。',media:'作者未知 · 评论尚未取得'},
    {id:'w5',title:'任务已经开始，为什么还需要人陪着：一个很长的合成标题用于检验完整阅读、两行截断和详细页面之间的关系',author:'样本作者 D',topic:'homework',likes:null,body:'【合成内容】将开始之前与开始之后分开描述，提出仍待观察的问题。',media:'互动未知 · 正文与评论可用'},
    {id:'w6',title:'极大读数的边界样本',author:'样本作者 E',topic:'routine',likes:1234567,body:'【合成边界样本】只用于验证数字缩写与精确值读取，不参与主题演示统计。',media:'边界演示 · 不计入统计'}
  ],
  external: [
    {id:'x1',title:'先问今天能学多久，再选择计划',domain:'考研自习',author:'外部样本 A',likes:2140,sort:'最多收藏',actual:20,note:'先区分时间条件，再提供对应选择。'},
    {id:'x2',title:'记录一次计划没有执行下去的过程',domain:'考研自习',author:'外部样本 B',likes:840,sort:'最新',actual:14,note:'失败点放在过程里，而非只展示结果。'},
    {id:'x3',title:'同一个日常场景的两种表达方式',domain:'自闭症干预',author:'外部样本 C',likes:1680,sort:'最多点赞',actual:20,note:'场景可参照，领域事实不可迁移。'}
  ]
};
const nav = {
  workbench:{name:'智能工作台',tabs:[['jobs','情报工作'],['watches','持续关注'],['results','情报成果'],['decisions','待我决定']]},
  insights:{name:'市场洞察',tabs:[['topics','主题地图'],['questions','用户问题'],['references','跨行业参照']]},
  corpus:{name:'语料库',tabs:[['evidence','证据库'],['comments','评论研究'],['creators','创作者'],['queries','已存查询']]},
  collection:{name:'采集',tabs:[['targets','观察目标'],['attention','待处理'],['operations','生产流'],['tasks','采集任务'],['runtime','执行工位']]}
};
const state = {home:'normal',selectedRegion:null,selected:new Set(['routine','reward']),sort:'works',descending:true,query:'',period:'90',job:null,request:'none',resultVersion:1,watch:false,notes:{},view:'table',htmlScene:false};
try {state.notes=JSON.parse(localStorage.getItem('linggan-design-notes-20260906')||'{}');} catch {state.notes={};}
const esc=s=>String(s??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const fmt=n=>n===null||n===undefined?'—':Number(n).toLocaleString('zh-CN');
const topic=id=>DEMO.topics.find(x=>x.id===id)||DEMO.topics[0];
const rate=t=>t.works&&t.hits!==null?`${(100*t.hits/t.works).toFixed(1)}%`:'—';
const link=(path,label,cl='')=>`<a class="${cl}" href="#${path}">${label}</a>`;
const button=(action,label,cl='',attrs='')=>`<button type="button" class="${cl}" data-action="${action}" ${attrs}>${label}</button>`;
