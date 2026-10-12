# TOPIC-MAP-READ-PERFORMANCE-382：主题图谱进入性能

> 状态: 活跃计划
> 实施进度: 候选实现与本机只读实测通过；待发布
> 最后核对: 2026-10-12
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
