// Synthetic, deterministic fixtures. No production data or network operations.
export const variants = {
 A:{name:'连续账本',thesis:'先比较任务，再按需检视',description:'六列账本占满工作区，详情在浮层中打开。保留最多横向比较空间。',tradeoff:'逐条打开详情，跨任务核对需要反复切换。',zones:'T → V / R → B → P；Z05 按需打开'},
 B:{name:'主从工作面',thesis:'边选任务，边读进展与成果',description:'左侧精简索引，右侧常驻详情；两边共享同一个工作面。',tradeoff:'列表的范围与材料明细移入右栏，同时比较的字段变少。',zones:'T → V / R → [B 索引 | Z05 详情] → P'},
 C:{name:'分组展开账本',thesis:'按阻碍与进展定位，在原位读细节',description:'当前页按既有状态归组，单行展开进展与成果；不增加新的业务状态。',tradeoff:'展开后页面更长；分组数只代表当前页，不能当作全库统计。',zones:'T → V / R → B 分组与展开 → P'}
};
export const filters=[['all','全部任务'],['active','进行中'],['attention','需我处理'],['results','已有成果'],['ended','已结束']];
export const domains=['全部领域','ADHD','考研自习'];
export const ranks=['综合','最新','最多点赞','最多收藏','最多评论'];
export const rows=[
 {id:'DEMO-026',name:'家庭任务启动',domain:'ADHD',source:'临时关键词',rank:'综合',limit:50,fixed:37,mode:'sample_30',status:'active',stage:'作品详情获取中',next:'等待当前执行交付',details:18,comments:126,reused:6,added:84,cumulative:210,gap:'实际发现 37 / 上限 50；列表已结束',created:'10.16 / 10:42',updated:'10:45',revision:3,leases:1,queued:19},
 {id:'DEMO-025',name:'亲子沟通',domain:'ADHD',source:'临时关键词',rank:'最多评论',limit:30,fixed:30,mode:'all_public',status:'blocked',stage:'需要处理资源限制',next:'查看工位可用情况',details:12,comments:284,reused:4,added:160,cumulative:444,gap:'部分评论不可公开读取；已得材料保留',created:'10.16 / 09:18',updated:'10:37',revision:5,leases:0,queued:18},
 {id:'DEMO-024',name:'林间观察 · 示例目标',domain:'ADHD',source:'已有创作者目标',rank:null,limit:20,fixed:null,mode:'sample_30',status:'waiting',stage:'等待调度',next:'尚未取得有效租约',details:0,comments:0,reused:0,added:0,cumulative:0,gap:'尚未开始发现；0 为已知当前值',created:'10.15 / 18:30',updated:'18:30',revision:1,leases:0,queued:1},
 {id:'DEMO-023',name:'已选 4 篇作品',domain:'ADHD',source:'精确所选作品',rank:null,limit:4,fixed:4,mode:'all_public',status:'ended',stage:'本次结束',next:'材料已可读',details:4,comments:126,reused:4,added:69,cumulative:126,gap:null,created:'10.15 / 15:07',updated:'16:10',revision:8,leases:0,queued:0},
 {id:'DEMO-022',name:'复习中的时间分配',domain:'考研自习',source:'临时关键词',rank:'最新',limit:20,fixed:20,mode:'sample_30',status:'paused',stage:'本次已暂停',next:'可继续未完成的工作',details:8,comments:32,reused:2,added:20,cumulative:52,gap:'暂停不删除已取得材料',created:'10.15 / 14:20',updated:'14:53',revision:4,leases:0,queued:12},
 {id:'DEMO-021',name:'作业与时间管理',domain:'ADHD',source:'临时关键词',rank:'综合',limit:10,fixed:7,mode:'all_public',status:'ended',stage:'本次结束 · 有缺口',next:'查看缺口与已得成果',details:7,comments:0,reused:0,added:0,cumulative:0,gap:'公开评论无法读取；0 不代表不存在评论',created:'10.15 / 13:10',updated:'13:48',revision:6,leases:0,queued:0},
 {id:'DEMO-020',name:'日常记录 · 历史任务',domain:'ADHD',source:'历史记录',rank:null,limit:null,fixed:null,mode:null,status:'legacy',stage:'历史任务 · 已结束',next:'读取原始诊断',details:null,comments:null,reused:null,added:null,cumulative:null,gap:'历史范围、评论模式与新增计数不可核对',created:'10.14 / 12:08',updated:'12:15',revision:null,leases:null,queued:null},
 {id:'DEMO-019',name:'备考计划执行',domain:'考研自习',source:'临时关键词',rank:'最多收藏',limit:15,fixed:15,mode:'sample_30',status:'stopped',stage:'已停止 · 在途交付中',next:'等待已产生材料完成交付',details:3,comments:12,reused:1,added:8,cumulative:20,gap:'未认领工作已取消，停止后材料仍可能增加',created:'10.14 / 10:08',updated:'11:04',revision:7,leases:1,queued:0}
];
export const qty=v=>v===null||v===undefined?'未知':v.toLocaleString('zh-CN');
export const commentLabel=m=>m==='all_public'?'尽量采全，含回复':m==='sample_30'?'每篇最多 30 条样本，含回复':'评论模式未知';
export const statusLabel=r=>({active:'执行中',blocked:'需处理',waiting:'待调度',paused:'已暂停',ended:'本次结束',legacy:'历史记录',stopped:'已停止'})[r.status];
export const tone=r=>r.status==='active'?'orange':r.status==='blocked'?'amber':['waiting','legacy','paused'].includes(r.status)?'muted':'ink';
export const canControl=r=>!['ended','legacy','stopped'].includes(r.status)&&Number.isInteger(r.revision);
export function matchFilter(r,f){return f==='all'||f==='active'&&['active','waiting','paused','blocked'].includes(r.status)||f==='attention'&&r.status==='blocked'||f==='results'&&((r.details??0)>0||(r.comments??0)>0)||f==='ended'&&['ended','legacy','stopped'].includes(r.status)}
export function groupOf(r){return r.status==='blocked'?'需我处理':['active','waiting','paused'].includes(r.status)?'正在推进':'本次结束 / 历史记录'}
export const regionFacts=[
 ['Z01','全局页头','品牌、全局入口、工位管理','任务数、页面搜索、对象控制'],
 ['Z02-P','位置路径','采集 / 采集任务；真实上级可返回','把 ADHD 等领域当作路由层级'],
 ['Z02-S','当前范围','独立领域选择器；影响列表查询','发起采集、实时运行大读数'],
 ['Z03','模块导航','观察目标 / 采集任务；当前项唯一','工位管理挤入日常侧栏、任务筛选'],
 ['Z04-T','工作工具栏','FIND、刷新、新建单次采集','暂停某条运行、通用大 KPI'],
 ['Z04-V / R','视图与结果','五个既有视图、匹配数、读取时间','伪单标签标题、把分组数当全库数'],
 ['Z04-B','任务主体','运行身份、范围、材料、进展、时间、详情入口','把底层工单直接充当一行采集运行'],
 ['Z04-F','局部反馈','旧数据读取失败、空态、资源受限提示','覆盖仍可读材料的全屏报错'],
 ['Z04-P','分页','20 条 / 页、页码、总量、保留范围与查询','把当前页当所有记录'],
 ['Z05-H / N / B / A','对象检视','对象身份、进展/成果、诊断、合法控制','全局配置、无资格的继续与停止']
];
export const auditFindings=[
 ['已核实 · 保留','当前任务页已有隐藏的可访问标题，没有重复的可见单项标签；一行代表一次持久化运行。','TasksPage / CollectionFrame + 真实页面'],
 ['已核实 · 优化','任务页没有直接领域选择器；查询与深链接已有领域参数。建议把范围独立放入 Z02-S。','CollectionFrame / TasksPage'],
 ['已核实 · 优化','调度、巡查、时区、运行总量和管理入口混在上下文栏。建议保留必要全局告警，把匹配数放回结果行。','真实页面 / CollectionFrame'],
 ['已核实 · 优化','列表未知数使用 —，详情使用“未知”。建议统一未知、已知 0、不适用的不同表达。','TasksPage / tasksApi'],
 ['源码推导 · 待故障验收','列表错误分支只在空列表渲染；保留旧行时应有本地读取失败与截至时间。真实故障未注入。','TasksPage'],
 ['已核实 · 提议新增确认','当前停止直接发送控制命令；建议在确认中解释取消未认领工作、保留材料和不可继续。未执行真实停止。','RunDrawer / runRequests'],
 ['已核实 · 保留','临时关键词、已有目标、精确所选作品具有不同来源合同；结果数、复用、新增、累计必须分开。','collection-single-run-001 / 实现'],
 ['规范缺口 · 待决策','P09 允许 Z02-S 但未列 C05；B 的常驻非模态详情、C 的内联详情也需补充组合许可。此包仅提出差异。','spec/registry.json → P09']
];
