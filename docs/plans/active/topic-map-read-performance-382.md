# TOPIC-MAP-READ-PERFORMANCE-382：主题图谱进入性能

> 状态: 活跃计划
> 实施进度: PR #389 历史已发布；2026-10-13 首屏后续候选实现与实测通过，冷重建仍慢，未提交/发布
> 最后核对: 2026-10-13
> 适用范围: Mog当前对话授权的主题图谱读取性能后续包，关联Issue #382
> 事实来源: 初始main@1f729850、当前基线main@ffcdb1eb、正式/候选只读实测、当前HTTP与来源资格合同
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实代码与验证；计划不扩大用户授权

## 授权、Claim与边界

Mog授权按首次重复请求、概览媒体读取、来源复核顺序优化，保留完整统计和来源资格校验，完成后实测进入耗时。root / topic-map-read-performance-382，初始base `1f729850ada813ce4806372e2b3a27e99d689263`，实施中main合入PR #388后本地基线更新到`ffcdb1eb112c5c6ede69ffd12da58c10e006393a`；分支`codex/topic-map-read-performance-382`、专属`.worktrees/topic-map-read-performance-382`。共享根工作区保持原状。GitHub Claim发布被自动审批拒绝：本机路径及授权边界未获向该目的地发送的明确授权；本地Claim作为当前实施留痕，发布仍待许可。

Exclusive：topic_map/read、core_read/load及来源复核seam，API topic_map.rs/.js，主题专用的共享Work Resource/Media轻量读取，必要的同源测试和脚本。Shared：文档索引、current-state、2026-10进度、HTTP合同及生成物登记。本任务内实施最小共享依赖；不改其他页面读取、视觉Token、领域统计语义、研究方法或调度。

初始实施边界禁止：migration/schema、共享库写入、研究恢复/新Run、模型外发/预算变化、真实采集、插件、共享main直接修改及未经授权的merge/deploy。现有Issue及代码只是事实，不扩大权限。提交前独立审核遵循AGENTS.md。

2026-10-12发布授权：Mog在当前对话明确要求“提交，推送，合并到main，然后刷新3000”。root作为本包受委托集成执行者，复用Issue #382与当前专属分支；完成commit-reviewer审查、远端检查后执行PR合并及本机runtime-main受控更新。该授权仅覆盖已实测的主题图谱性能包和必要发布留痕，不授权数据库迁移、模型/采集调用或研究策略变更。下文完成回执记录的是候选测量阶段状态，后续提交、CI、合并及运行回执在对应PR记录。

## 基线与实施顺序

正式样本：HTML 3.19ms；已有领域刷新14616ms；新标签侧栏进入29230ms；单主题选择14293ms。领域概览GET 14290ms/21489881bytes/1915作品/38主题；media字段16519572bytes。研究进度GET103.1ms。测量含浏览器自动化开销，是有界样本而非P95。

1. 页面按既有Topic API的数据库名称排序，在昂贵请求前确定可信领域，避免全域图谱再默认领域的串行读取；已有显式URL、领域切换和同域深链优先。
2. 概览使用canonical共享读取的轻量投影，保留真实标题/OCR封面回退、互动、当前限制与缩略图；媒体详情仍由既有原文读取按需返回。完整作品集合与服务端统计不分页截断。
3. 概览不组装未返回的采集轨迹/评论线程，但直接调用原有默认证据片段seam；事务局部jit=off/force_custom_plan，结束恢复连接设置。注释和结果只复核实际引用的作品；比较必须包含完整冻结参与作品，即使其中作品被UI过滤。复用单次请求的已资格化输入，禁止跨请求缓存来源许可。研究窗口分母按领域/配置/作品在本次请求内复用，结果仍逐条恢复/验证。

## UI读取回执、表面和依赖

已读取AGENTS、docs/README/current-state、UI execution contract、design README/governance、LIDS README与现有主题手册、HTTP合同。分类：交互/读取实现；不改视觉或业务状态。沿用Token→既有Readout/Status/Material Card→概览及阅读器Pattern→/topics。

| 表面 | 保持的行为和状态 | 依赖 | 验收 |
|---|---|---|---|
| 首次入口、显式领域/主题深链 | 合格统计显示前保持正在读取；空领域不是空业务库 | 页面领域picker、URL/session state | 单次首次请求、URL优先、领域切换 |
| 主题概览/树、平台/统计窗口/旅程 | 全范围统计、unknown/null、子主题并集与我方历史口径 | canonical Work Resource、研究资格 | 轻量与完整投影统计/身份等价、过滤回归 |
| 材料卡片与封面 | 同一合格封面选择及标题；详情按需读取 | 共享media disposition/cover selector | 清理、撤回、标题回退、缩略图 |
| 判断与原文阅读器、比较、备选 | 每次重核来源；受限材料/定义不展示旧研究 | 完整冻结manifest与资格门 | 撤回、版本改变、比较参与者被筛除、旧合同 |
| 失败与返回路径 | 失败不伪造空库，不继续展示旧资格；保留重试 | 现有fetch epoch与错误表达 | 乱序/关闭/失败回归 |

## 验证、生成物与完成层级

维护实际源码回归：JS真实初始化/请求序列，合成PostgreSQL完整统计与撤回/版本/跨作品依赖。运行既有主题核心原生proof，验证其测试数据资源清理。候选API仅只读业务库、独立loopback端口、不启动worker；用CUA实测证据库侧栏→概览及刷新/单主题，记录响应大小、作品数和完整统计对比。不保存真实正文/凭据，不执行写命令或模型。

生成物沿既有登记`/tmp/topic-map-v41-*`；Cargo缓存在worktree ignored target或已登记临时路径。生成物不入Git。代码、自动验证、候选实测、main合并、正式运行、Mog验收分别记录。

停止条件：需要改变业务口径、放宽资格、跨任务所有权冲突或新增真实权限时报告具体边界。本包完成后报告候选耗时及统计/资格证明，正式3000仍需独立合并发布授权。

## 2026-10-12 完成回执

代码保留在专属分支未提交；main基线ffcdb1eb，PR #388的名称修复已完整保留。正式3000监听PID11554，cwd/txt核验为runtime-main；候选3001监听PID31240，cwd/txt核验为本worktree。候选连接强制default_transaction_read_only=on，不启动worker、不修改运行身份。最终测量后关闭本次候选进程和自建标签，保留用户原标签。

| 路径 | 诊断基线 | 最终候选 |
|---|---|---|
| 新标签语料侧栏→领域概览 | 29230ms | 5174ms |
| 已有领域概览刷新 | 14616ms | 5723/5423/4928ms，中位5423ms |
| 选择单主题 | 14293ms | 4628ms |
| 完整概览API、当前稳定1924作品 | 正式14747.7/14583.4ms | 4700.8ms |
| 同一稳定范围响应体 | 21621052bytes | 5855685bytes |

浏览器从动作到目标标题可见计时，含自动化开销；首次/下钻各1次、刷新3次，是有界现场样本，不作P95/SLA声明。当前实测仍约5秒，未宣称秒开。较早测量库为1915作品；实施期间实时入库增至1924，正式研究状态读取为active，本任务没有恢复、暂停、新建研究或模型外发。运行策略不由本性能包改变。

稳定窗口顺序读取正式→候选→正式，前后正式业务投影一致。去除asOf、按集合引用归一化并仅把media比较为选中cover后：1924/1924作品无差异，包含preview/evidenceFragment/sourceManifest/research；完整scope/statistics/topics/journey/sources/candidates/changes/alternatives/ownCreators/methodVersion/sourceBoundary/domains全部一致。独立重算全部作品与每个主题的样本总量、作者、点赞已知/未知分母、中位/P90及阈值计数，差异0。38主题、80篇附研究作品均保留。前两次窗口有实时媒体/指纹更新，未用于宣布逐字段等价；最终稳定窗口来源指纹差异0。

验证：70项隔离PostgreSQL全通过（主题内核/旧方法/恢复/未知/来源/结构，17项共享材料投影，2项评论预算，1项HTTP）；108项相关lib单元、2项非ignored API单元、19组JS真实初始化/行为回归通过。新负例覆盖501篇跨批不截断、父子union、unknown、空引用不回退全域、默认正文/封面OCR回退、已清理/受限封面、比较参考参与者被默认范围排除后撤回即失效；8次公共概览读取结束后连接jit/plan_cache_mode恢复原值。build、变更文件格式、JS/bash语法、diff与项目治理通过；浏览器console error=0。隔离容器与卷cleanup verified。

格式检查说明：creator_discovery.rs继承main/PR #388的已有排版差异，验证其格式差异与main完全相同，本包新增代码未引入差异；其余变更Rust文件rustfmt检查通过。计划状态已按治理闭集修正为“活跃计划”，最终治理检查通过。收尾已终止候选PID31240，3001无监听；正式3000仍为PID11554/main@ffcdb1eb。仅关闭本次自建测量标签4和5。

早期轻量路径漏默认正文，被统计/旧方法回归发现后改为直接复用原有fragment seam；两处新增fixture错误（只追加用途与清理必须引用materialization）已修正，最终全套通过。临时SQL内联计划试验变慢已撤回，临时计时代码已删除；这些失败不计为通过。

证据仅聚合指标/差异及界面：`/tmp/topic-map-v41-performance-comparison-confirmed.json`、`performance-browser-final.json`、`performance-candidate.png`（同前缀）、`performance-postgres-pass.log`、`performance-unit-final.log`、`performance-ui.log`、`performance-governance.log`。真实正文和凭据未保存。当前完成层级为本地源码+自动验证+只读候选实测；提交/push/PR/merge/deploy/Mog正式页面验收未执行。GitHub Claim仍仅在本计划留痕，自动审批拒绝状态未绕过。

## 提交前复核

commit-reviewer发现并确认两处问题：完整材料列表把作者身份当作归属来源，已恢复author_attribution_source并扩展列表/单条MATCHED一致性验证；HTML注册表创建顺序与旧Topic API名称排序不同，已改为数据库同序轻量读取，并增加先建BBB再建AAA的HTTP反例。完整统计/来源资格与媒体轻量路径不因这些修正放宽。最终回归结果与exact commit在PR发布回执记录。

提交前最终回执：71项隔离PostgreSQL通过、0失败，含新增批量列表作者归属及HTML名称排序反例；容器和卷cleanup verified。19组前端回归及API cargo check复跑通过。commit-reviewer独立复审PASS，无剩余阻塞；exact commit在PR核验受审源码diff。此前GitHub Claim被自动审批拒绝，具体声明/责任人授权已向Mog发出，未绕过。


## 2026-10-13 再次进入性能优化（当前执行 Claim）

Mog 本轮要求测试并优化当前主题图谱。复用 Issue #382，不拆任务、不增加并发。执行 root，分支 `codex/topic-map-read-performance-followup-382`，独立 worktree `.worktrees/topic-map-read-performance-followup-382`，基线 main `9e45e01c`。授权到源码实施、隔离合成回归、强制只读候选 API 与浏览器计时；本轮未授权提交/push/合并/部署，不沿用旧包发布授权。旧发布证据仍按当时版本保留。

当前基线：正式浏览器侧栏进入 16924ms；API 13053ms、9045241bytes、3224作品/48主题。现场单次样本，含自动化开销，非P95。正式服务已由其他发布更新，本任务没有重启它。

沿用上方 UI 表面、状态和验收矩阵及已读取的 AGENTS/UI执行合同/LIDS/主题手册/HTTP合同；统计和来源资格必须完整，unknown不变0。最小范围是主题读取及直接依赖的共享材料/来源查询和相关回归；不改变布局、调度、研究方法、数据权限。先在候选做任务专属聚合分段计时，再优化被证实的慢段，删除临时计时，验证稳定范围投影等价和限制负例。生成物沿已登记 `/tmp/topic-map-v41-*`，构建缓存复用已登记 `/tmp/linggan-topic-map-workflow-target`；不保存真实正文、凭据、dump。共享材料语义变更/并行冲突停止升级。


### 2026-10-13 后续候选完成回执

实施：共享媒体当前下载/合格副本/较新处置副本改为集合选择，消除逐槽位遍历整批副本；完整范围按500篇且最多两批并行，原顺序合并；窗口计数使用实际窗口的同一分组遍历，跳过完整输入克隆/组装/hash；主题评论分页在自有只读可重复读事务内复用作品/父评论上下文。调用方自有事务继续逐页资格检查，备选保存/恢复不扩大缓存边界。主题材料/评论读取的JIT和custom plan仅事务LOCAL。没有跨请求许可缓存、统计截断、字段删除或UI语义修改。

最终同一稳定3224作品/48主题/141附研究作品的正式→候选→正式对照：8489.0→4590.9→8638.3ms，9.05MB。去除asOf并按引用集合归一化后，scope/statistics/topics/works/journey/sources/candidates/changes/alternatives/ownCreators/methodVersion/sourceBoundary/domains全部相等，包括完整媒体投影与来源指纹；稳定作品差异0，独立重算统计差异0。未用API成功取代资格证明。正式版本9e45e01c，PID16192 cwd/txt在runtime-main；最终候选PID50325 cwd为本worktree、txt为登记的临时Cargo缓存，数据库强制只读、不启动worker。

| 同一浏览器动作与完成条件 | 正式3000 | 候选3001 |
|---|---:|---:|
| 新标签语料→概览完整统计及材料可见 | 11015ms | 5950ms |
| 刷新已有概览 | 11482ms | 5528/5761/5575ms，中位5575ms |
| 切换用户生命旅程、地图可见 | 11551ms | 6076ms |
| 单主题下钻、1篇匹配可见 | 未重测 | 5056ms |

最终浏览器统一采用locator click后AX就绪观察；较早native click测量16924/14175ms包含不同导航等待，保留为诊断样本，不用于最终收益计算。HTML壳正式199ms、候选147ms；浏览器console error=0。候选仍有约5–6秒等待，API仍返回9MB，不宣称秒开/P95/规模SLA。完整统计和来源资格是保留的成本。

验证：主题全套隔离PostgreSQL96项通过、0失败，含新增跨作品副本截止/旧副本回退/slot处置负例；最后调整后统计5项、研究原9项、备选5项定向通过；新增132回复跨128页、父评论更新和页尾撤回测试通过，输入账本不增。新测试最初把合成setup已有调用误设为0，断言失败，核验后改为保存setup基线并比较不增，再隔离复跑通过，未改业务代码掩盖失败。最终160项主题lib单元（含计数/父文/小预算边界）通过，29组JS回归通过。API最终build通过；governance/手册/diff及变更Rust格式检查通过（creator_discovery.rs保留main已有格式，仅局部追加LOCAL设置）。临时targeted入口首跑因/tmp相对项目根错误提前停止，未创建容器，修正固定根后继续；全部实际测试容器/卷cleanup verified。

证据仅聚合：`/tmp/topic-map-v41-followup-final-comparison.json`、`followup-browser-final.json`、`followup-postgres.log`、`followup-targeted-postgres.log`、`followup-context-postgres.log`、`followup-final-unit.log`、`followup-ui.log`、`followup-final-build.log`、`followup-governance.log`（均同topic-map-v41-前缀）。早期并行隔离测试期间对照约20s→8.3s，业务投影也相等，但最终速度使用测试退出后的8489/4591/8638ms样本。真实HTTP正文只在内存对比，未保存。临时分段计时代码已删除。

完成层级：独立候选源码、自动验证、真实只读浏览器/API实测；没有提交/push/PR/merge/3000部署/Mog页面业务验收。正式3000由其他发布保持9e45e01c，本包未重启/迁移/修改其研究策略，不沿用旧PR的发布授权。

收尾核验：已终止本次候选PID50325，3001无监听；正式3000仍为PID16192。本次自建浏览器标签8/9/10/11已全部关闭，用户原标签未修改。候选代码与聚合证据保留用于后续提交复核。

### 2026-10-13 首屏继续优化 Claim（Mog 已授权实施与实测）

继续同一执行者/分支/Issue；不增加并发、不提交发布。目标：进入/刷新/证据详情的独立实测。沿用已读 UI 执行合同、LIDS data/language、当前 compact 表面与上方验收矩阵；布局/业务动作不改变。新增首屏统计先就绪、列表独立读取状态，不把尚未载入数量当成完整统计。

候选实现采用进程内可重建读取投影，不在共享库安装迁移/触发器。复用前校验当前 schema 的完整 MVCC 行版本/删除/DDL 指纹；只缓存构建前后指纹一致的完整 canonical 结果，来源/定义/评论/媒体任何变化都失效。数据仍注明原 asOf，保留完整范围和 unknown；读取投影有界且只存在 RAM，不保存正文到磁盘。页首统计与材料传输分开，证据打开保留现有实时作品/评论核验，并改为单作品研究资格响应（完整冻结比较依赖仍参与资格检查）。

源码生命周期：crates/intelligence/src/topic_map/read_model.rs 为本模块可重建投影所有者，由 topic_map.rs 索引；API local_web/topic_map.rs 与 local_web.rs 仅组合进程生命周期和读接口；既有 JS/VM/隔离 PostgreSQL 证明扩展失效、冷/热、完整统计、来源撤回、分页及乱序关闭。使用既有 /tmp/topic-map-v41-* 输出聚合测量/截图/日志，禁止原文/凭据/dump。构建沿既有注册缓存。真实候选只读且无worker，最终关进程/自建标签。冷构建耗时与已预构建首屏耗时必须分列，不用热命中伪装冷性能。

真实库版本诊断补充：8秒只读前后比较仅 creator_discovery_policy 与 model_workspace 行版本变化。当前写入所有者分别更新扫描游标/updated_at、worker_last_seen_at/state/error；已核对主题canonical及材料/评论资格读取，这些运行字段不参与其投影/许可。计划仅对这两表按去除上述明确运行字段后的完整业务行内容做数据库内摘要，其余表仍按MVCC版本；配置引用、阈值、授权/额度、领域与身份等全部保留，新增业务列自动进入摘要。必须增加“运行字段变化复用，业务配置变化失效”隔离负例，再重测；之前409和非法31天请求不计通过。

第二次冲突诊断：构建期发生作者分析作业控制、模型调用账本及目标巡检时间变化。已沿canonical读取链核对：模型调用账本只被独立research progress用于费用/诊断，不被图谱Snapshot或来源资格读取；该状态接口仍每次实时读取。目标last_scheduler_considered_at仅决定巡检公平次序；作者分析的job_state/attempt_count/next_attempt_at/lease_token/lease_expires_at/last_error_code/updated_at不进入主题Snapshot或评论/原文许可。仅排除这张独立调用账本，并对后两表去掉已审计的运行字段做完整内容摘要；作者结果/指纹/手工裁定、目标身份/生命周期/名称/监控资格及所有新增业务列保留。增加真实schema合成调用账本、作者作业控制/结果变更、目标时钟/生命周期反例。没有暂停正式worker或修改真实运行数据。

### 2026-10-13 首屏后续最终回执（候选源码，未发布）

实现为RAM完整投影、只读启动预构建/完成后间隔20秒刷新、最多4范围且60秒后禁止复用；读取或成功构建会清理过期条目。既有完整canonical读合同保留。summary先传全部统计/主题与首批12作品；overview随后传全部匹配索引，省略sourceManifest及非封面media传输字段；证据研究资格只传目标作品，后台仍核对全部冻结比较依赖，正文/评论仍各自实时GET。材料阶段成功整份替换，失败不伪装空库，旧领域迟到响应不能覆盖当前领域。没有持久化/触发器、游标分页、按作品增量统计或数据库迁移。

资格版本检查覆盖schema表身份/权限/定义及行版本；仅排除四个调度运行控制账本和仅服务独立费用/进度的model_invocation行。四张混合表以去掉已审计运行字段的完整业务行摘要检查：model_workspace心跳，creator_discovery_policy扫描游标/updated_at，creator_discovery_author_analysis作业状态/尝试/租约/排队/错误/updated_at，以及collection_observation_target的last_scheduler_considered_at。其他业务字段/新增列、目标生命周期与身份、作者结果/指纹/手工裁定、配置/阈值/权限、许可与来源处置仍参与；该账本的research progress仍独立实时读取。来源变化不能命中旧许可投影，构建前后不一致最多重试一次再409，不将不稳定结果宣称完整稳定投影。

最终稳定正式→候选完整canonical→正式：10748/6811/10341ms，约9.02MB。除asOf并按引用集合归一化后，13组顶级业务投影及3224篇完整作品（包括媒体/来源指纹）全部相等；稳定作品差异0，全范围与每个主题统计独立重算差异0。新overview的完整3224作品/48主题与canonical在明确传输省略后相等；summary的全量非作品字段及首批12内容相等、totalWorkCount仍3224；单证据完整作品相等。当前合格附研究作品为137，与正式前后读取相同；此前141为较早现场状态，不能以本次候选保留旧的失效结果。正式research policy读取为active，本包不恢复/暂停/扩批/调用模型或平台。

已准备读取API：summary349/368ms（1.37MB），完整overview665ms（6.78MB），单证据278ms（9785bytes）。最终冷请求：进程启动默认领域5398ms；新的7天参考范围5283ms。背景定期刷新仍是完整构建，有CPU/数据库成本；源码计时结果不是持久化增量维护的证明。

| 同一浏览器动作/就绪条件 | 正式3000 | 候选首屏 | 候选完整材料/详情 |
|---|---:|---:|---:|
| 语料侧栏进入概览，各1次 | 11140ms | 542ms | 1268ms |
| 刷新已有结果，3次中位 | 11385ms | 563ms | 1148ms |
| 浏览器整页刷新，各1次 | 16905ms | 2288ms | 3777ms |
| 同一作品原文/评论/研究资格全部读完，3次中位 | 10621ms | — | 440ms |

正式基线测量时候选关闭。刷新原始样本：正式11714/11385/10371；候选首屏7598/509/563、全材料8103/1136/1148ms。因此不宣称每次刷新亚秒：一次失效重建仍等待7.6秒。详情正式11101/10621/10445、候选474/365/440ms，6次均没有读取错误。含浏览器动作及AX就绪观察开销；小样本，不是P95/SLA。浏览器console error=0。原测量标签会话清理导致整页正式补测使用独立新标签5，候选已关闭；补测完成显式关闭5，没有操作用户原标签。

验证：完整隔离PG98通过/0失败，后续版本边界修正后最终8项定向复跑（6统计/资格、1调用账本反例、1HTTP）通过并cleanup verified；作者作业/巡检时间变动复用，但作者结果/目标生命周期/业务阈值/工作区身份变化失效。来源撤回/定义修订/同数行更新/删除/DDL仍使投影失效。160主题lib、3非ignored API、31 JS组通过，包含两阶段失败和旧领域迟到响应；最终build、变更Rust格式、JS语法、diff、治理与设计手册检查通过。早先非法31天请求400、两次连续源变化409、磁盘耗尽/中断未完成的运行不计通过，均有诊断或重新验证。临时诊断代码已删除；没有为计时暂停真实worker。

正式仍是runtime-main@9e45e01c/PID16192，cwd/txt核验一致。本次最终候选PID87846，cwd为本专属worktree，txt为已登记冻结副本（sha256 6fa278cf03f2c7bbb3b1c88bae0be4a0fff90c652747d2e52c62c0518aeba812），服务JS与当前源码逐字节一致，DSN强制default_transaction_read_only=on，无worker。测试结束已停止候选，3001无监听；冻结副本按注册规则删除。原文/凭据未存盘，真实HTTP响应只在内存比较。聚合证据：/tmp/topic-map-v41-first-screen-comparison.json、cold-final.json、browser-final.json、full-postgres.log、final-targeted-postgres.log、unit.log、api-unit.log、ui.log、build.log、governance.log、handbook.log（后续均同topic-map-v41-first-screen-前缀）。

完成层级：独立候选源码、回归、真实只读API/浏览器实测。尚未提交/push/PR/merge/3000部署/Mog前端业务验收，不沿用旧包发布授权。首屏常态与详情改善明显，冷重建仍约5–8秒；若要消除该等待，后续需要按真实源变化维护可持久化的投影/增量统计，不能仅增加RAM过期时间或放宽来源许可。


### 2026-10-13 当前后续包发布授权

Mog 已明确要求“提交，推送合并后部署上线刷新 3000”。授权当前已实测的后续包，单执行者，不增加 Agent 或审查并发；沿既有 Issue #382 与专属分支完成 PR、合并及本机 runtime-main 更新。先同步最新 main 的登录页更新并验证集成，不更改该页面。保留完整统计、来源资格与冷重建限制。无新增 migration，不执行共享库迁移、研究策略/预算变更、模型或采集触发、插件及远端生产发布。最终 SHA、检查、drain、三服务身份及正式浏览器计时在发布回执记录；Mog 业务验收仍独立。


发布前集成复核：主线登录页 2f186861 合入，本包唯一共享壳 diff 仍是 topic_map 路由注入数据库。月度记录冲突保留双方有效记录。compact 浏览器合成 fixture 原来只模拟旧 GET，发布前同步 summary/overview/evidence 合同和首批→完整就绪等待；参数断言改为新 overview 请求。该调整只修测试，不改运行界面/语义，待远端 CI 证明。当前已自审缓存资格/失败/范围及媒体历史处置；未安排额外 Agent 或冒充独立复审。
