# CREATOR-READ-PERFORMANCE-001 · 创作者首屏读取性能

> 状态: 活跃计划
> 最后核对: 2026-10-29
> 适用范围: 语料创作者目录读取、共享媒体索引及本包隔离证明
> 事实来源: Mog 当前对话确认的诊断及连续实施授权；base 2c6dbf5f
> 冲突时以谁为准: 用户最新授权、真实源码/数据库/浏览器证据；不放宽既有资格

## 用户结果与 Claim

保护车道；用户已确认本交付包标识，无额外 Issue/消息外发。协调者 Codex / root，
task-id CREATOR-READ-PERFORMANCE-001；base `2c6dbf5fffceb36fbc9298f12d8025b2871405d7`；
branch `codex/creator-read-performance`，工作树 `.worktrees/creator-read-performance`。
目标是缩短进入、刷新、分页筛选和作者详情等待，保留完整统计、同一作品匹配、当前来源资格。

独占：creator_discovery 列表读适配、对应 API、性能索引 migration、必要证明和本包记录。
共享：migration 登记/fixture、docs 索引由本协调者窄集成。
避开另一个 creator-context-read-filter 工作包正在修改的 research_dossier.rs、material_query_sql.rs、
material_social_read.rs、target_inspection 和其余消费者。通过现有标准 dossier 读取接口展开，
不吸收该包尚未批准的行为或修改其文件。
禁止：正式数据库 migration/写入、runtime-main 变更、服务重启、worker 启用、采集、模型外发、合并部署。

## 实施与验证

1. 修复已实测的 media_observation_ref 查询索引缺口和作品清单资格查询访问路径；仅写 migration，隔离库应用。
2. 完整领域 Current 与判断资格仍参与统计；有作品研究、人工调整或账号研究依据的作品照常完整读取。
   无判断依赖的作品可暂缓完整档案，仅当前页代表作品展开；正文搜索仍读取所有相关正文。
   单次 repeatable-read / as_of 内完成，不使用跨请求资格缓存，不提前截断统计或跨作品拼接条件。
3. 比较原完整读与候选 JSON 的全部业务字段；验证跨页、同作品反例、原文变化、限制与撤回、作者支持依据、未知值。
   优化后若前端剩余串行成本可忽略则保持原读取/取消语义，不引入额外缓存。
4. 隔离真实 PostgreSQL、原 creator/dossier 回归和真实只读候选浏览器，分别实测进入、刷新、分页筛选与作者详情。
   实际正式 migration/合并/部署由后续 exact-head 授权处理，不能把候选耗时写成正式上线。

诊断基线：ADHD 3274 篇研究用途作品、1516 作者；50条 API 两次19.74秒，25条21.54秒；
浏览器进入43.74秒、刷新23.15秒、作者概览0.30秒；有限样本非P95。100篇档案清单样本333次下载表顺扫，
该表17241行且只有主键；JIT和generic/custom对照无明显改善。
临时日志、汇总、只读候选脚本在系统临时目录，本包生成物不入 Git、不保存原始材料正文。

## 第一轮候选验证记录（发布前快照）

候选实现与独立审阅 PASS，尚未提交/推送、合并或正式迁移/部署，Mog验收待处理。

- 预检通过；API构建/检查通过。隔离入口 `bash scripts/test-research-dossier-postgres.sh --creator-read`：
  5项creator＋9项dossier通过。124篇作品跨页、完整统计、同作品正负反例、来源更新/人工依据失效、
  页内75篇代表作仅1批inventory读取（不展开全部124篇）、两索引真实应用均已验证；临时DB/container/volume清理通过。
- 原fixture存在两处调用类型漂移及删除AI control的过期做法，已修复为合成owner持真实advisory fence，
  按pending→running→attempt receipt→terminal推进，不禁用trigger或启动真实executor/provider。
- 正式3001 vs候选3116五组GET：默认、第二页、高赞、名单外、同作品组合；除读取时刻外完整JSON相等，
  包含1516作者/3274作品和来源/判断字段。实库同作品组合为0项，非空正负语义由隔离矩阵覆盖。
  三个错误路径（未设阈值/非法筛选/不存在领域）状态与错误JSON相同。
- 候选API样本1.36–2.08秒，对应原API20.21–27.99秒；正式库未应用0182，因此不含新增索引收益。
- 浏览器样本（含工具开销，非P95）：进入2849/2442/3027ms；刷新3062/2828/2929ms；
  翻页2396/1998/2213ms；名单外筛选4291/1996/5639/2311/2095ms；作者概览270/271/275ms。
  未丢弃慢样本，筛选仍存在2.0–5.6秒波动；console error0。原前端未改，未引入资格缓存。
- 独立审阅/root/commit_reviewer PASS；亲自API check、相关单元1项、脚本语法、迁移摘要/diff通过，
  并核对上述PG/HTTP汇总。10个代码/脚本/SQL文件按排序路径+NUL+bytes+NUL的SHA256：
  `5d0c942f71346f5cda903b0a7a719bce96579391dca6af73f337411f84552699`。
- 通过既有dossier读口保留共享读取所有权，本次缩小重复Current/完整档案的适用集合；
  当前页及有依据的作品仍经标准底稿完整读取，不宣称所有重复读取已经消除。
- 正文搜索为保证全部合格片段匹配，仍保留完整档案读取；未证明其已达到普通作者筛选的耗时。
- 正式migration、最终CI、精确提交合并和部署均未发生；报告和汇总在系统临时目录，不含原文/凭据。


## 本机发布授权接续 · 2026-10-29

Mog在当前对话明确授权本候选提交、推送、合并main、执行唯一0182迁移、部署并刷新3001，
两个worker继续停用；既有AI执行器禁用也保留。发布使用原始配置、备份与实际迁移/进程/页面回读。
不扩展到另一工作包、插件重载、供应商调用或新平台采集。此段只记录授权，实际结果另留发布回执。


## 第二轮重复读取与首屏优化 · 2026-10-29

Mog已授权按诊断顺序继续实施、保留完整统计/同一作品匹配/来源资格、补充分段计时，并实测首次进入、刷新、分页筛选、作者详情。
基线 main/runtime `075505c10b3b2ea4b32e1c1dfef86fb7f6bcebfc`；第一轮 PR #516 已合并且0182已部署至本机3001。
第二轮 branch `codex/creator-read-performance-v2`，复用本包原干净工作树与其编译目录，不另建工作副本。
本段是当前 Claim；不沿用第一轮已消费的提交/合并/部署授权，也不执行新迁移。

### 实施范围与所有权

- 独占 creator_discovery.rs、creator_discovery/list_read.rs及其新增同目录助手、对应creators API/定向测试。
- 共享窄接缝：research_dossier.rs的read_in组装复用、material_query_sql.rs的Current采集包资格CTE；
  与creator-context-read-filter分支的creation_work_directory/domain_usage过滤行不重叠，不吸收该分支。
- 标准Current与底稿仍是唯一事实入口；只复用当前repeatable-read事务/asOf中的结果，无跨请求权限/事实缓存。
- 计时只输出固定阶段名和耗时，不含原文、来源引用、用户标识或凭据；业务JSON合同保持一致。
- 禁止正式配置/迁移/服务切换、worker启用、模型/平台调用；候选仅在自有loopback端口运行，实库连接强制只读。

### 验证与交付

1. 先消除作者全表重复扫描、代表作Current重读及两次完整聚合；再核验标准SQL共用资格读取收益。
2. 保留全域完整统计和同一作品谓词，未分析代表作仅补齐显示字段，不重判未知或丢弃来源限制。
3. 隔离PG覆盖完整业务JSON等价、非空同作品正负例、分页/排序、来源变化、人工与账号依据失效，
   并用查询计数证明Current与inventory不随代表作重读；补标准Current来源/字段回归及API计时头验证。
4. 同等数据、未改前端的只读候选与正式基线比较，分别保留首次进入、刷新、分页筛选、作者详情全部样本；
   区分候选进程首次请求、浏览器新文档和真正冷机缓存，保留慢样本，不虚报P95或已部署。
5. 必要验证后安排一次独立只读审阅；正式提交合并和部署另按最终候选授权。

诊断证据：/tmp/creator-entry-diagnosis-2026-10-29.md；API 1479–1570ms，浏览器2103–2306ms，
此前5435ms无分段trace且本轮未复现。3274作品/1516作者；当前页88代表作，Current读取路径7+1+1，
作者重复匹配约496万次，aggregate两次。计时需要覆盖pool、scope、current、qualification、projection、dossier、aggregate、representatives和response。

第二轮直接依赖补充：隔离查询计数揭示material_reading::source::current逐正文重读Current（75篇代表作额外75次），
因此窄增加material_reading/read.rs、source.rs及模块导出接缝，复用同事务Current的body_source；
unrestricted、来源版本相等、媒体资格与选定读本检查全部保留。不影响另包领域过滤修改。


### 第二轮候选交付记录

实现完成，尚未提交/推送/合并/正式部署；不改变当前3001的075505c1或0182。
13代码/测试/脚本文件摘要：`c27f59df85a57fa0a6359db7390d2fa06fc9bf7cfa6b268551f9fa41f829b7c6`。

- 标准Current一次读取全域，向标准底稿及正文来源校验复用同一事务/asOf的Current；
  原正文unrestricted、来源版本比对、媒体资格与字节guard保留。隔离75篇代表作的Current计数由发现的76次降至1次，inventory仍1批。
- 账号依据仅处理实际有记录的作者，未展开作品指纹延至可见代表作补齐；仅补展示/来源字段，不重算全域aggregate。
- 标准SQL共用合格detail/engagement集合，保留时间、来源、逐字段条件与tie-break；未采用性能退化的package-list CTE方案。
- Server-Timing闭集数值阶段已接API，不改业务JSON，不记录原文、引用或身份。
- 最终预检、API check/build、改动格式/脚本/diff通过。隔离真实PG43项通过：creator5、dossier9、material projection20、reading5、corpus3、API timing1；自有DB/container/volume/media清理验证通过。
- 独立/root/commit_reviewer PASS；亲自API check/lib1/语法/diff、Current SQL展开逐token等价及代码摘要，核对43PG与实际JSON汇总；没有正式运行操作。
- 正式3001对只读候选：默认/第二页/名单外/高赞/作者搜索/同作品组合六组，除顶层读取时刻外完整业务JSON一致；
  三个错误状态/JSON相同；共享作品资源和完整底稿两个200正例业务JSON也一致（底稿另归一化/dossier/asOf）。
  同作品实库0项，不用它冒充非空正例，正负矩阵由隔离fixture覆盖。
- 首个候选API624ms，正式同组1786ms；六组候选467–656ms，正式1246–1786ms。
  第一条计时：Current347ms、底稿87ms、aggregate54ms、projection44ms、response6ms，其余阶段及总和见本地回执。
- 浏览器新文档首次进入1272ms（已运行候选进程，非清空所有缓存的冷机）；初轮站内进入1102/1240/2470ms，
  刷新3533/3064/3606ms。慢样本完整保留，不宣称稳定低于2秒。
- 增加临时GET/HEAD计时桥后，在同浏览器源将同次页面与Server-Timing对齐：刷新1075/1117/1093/1068/1070ms，
  对应接口619–689ms；站内进入1624/1189/1194ms，首条Current687ms，后两条438/455ms。
  这些追加观测没有复现先前3秒多等待，不能据此追认早先慢样本的原因。
- 分页897/1269/1058ms，名单外筛选914/1029/970ms，作者详情274/288/406ms；均核对页码、选中状态、50行及1516作者/3274作品，console error0。
- 候选及计时桥、临时标签均已停止/关闭；正式3001 PID/版本、全部业务计数、策略/环境摘要与worker停用状态未变。
- 复用原工作副本/target，无新worktree或外部CARGO_TARGET_DIR；本树有未提交成果，按Mog规则保留，合并后再按60分钟无写入护栏回收。

本地完整回执 `/tmp/creator-read-v2-result-2026-10-29.md`，计时/等价/PG/浏览器/身份汇总均在`/tmp/creator-read-v2-*`；不含真实原文或凭据。


### 第二轮发布授权接续 · 2026-10-29

Mog在当前对话明确授权上述最终候选提交、推送、合并main、部署并刷新本机3001；两个worker继续停用，既有AI执行器disabled也保留。
本轮不含新增迁移，不执行migrate或数据重算；发布先核最终提交的CI与迁移账本，再从干净的精确main源使用当前受控installer。
原3条刷新慢样本及未归因边界继续保留，不因合并部署声明已解决长尾。实际提交/CI/合并/进程/浏览器证据另写本地发布回执。
