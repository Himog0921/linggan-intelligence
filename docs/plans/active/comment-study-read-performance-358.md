# COMMENT-STUDY-READ-PERFORMANCE-358 · 评论研究读取性能

> 状态: 活跃计划
> 最后核对: 2026-10-10
> 适用范围: Issue #358 的评论目录、统计、作品选择器与情报总览读取
> 事实来源: Mog 当前对话授权、main 3830d4b6、正式 3000 的有界只读测量与当前代码合同
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实代码与数据库证明

用户要求优化评论研究模块。Claim 已登记于 Issue #358，单一执行者在 `.worktrees/comment-study-read-performance` / `codex/comment-study-read-performance` 实施；代码提交前由只读 commit-reviewer 复核。

三次 HTTP GET 中位数：overview 2011.4ms，comments 1670.4ms，catalog-summary 1564.2ms，works 1323.9ms，problems 71.2ms，runs 605.8ms。评论页同时调用 comments 与 catalog-summary，二者重复计算同一目录事实。先用只读 EXPLAIN 确认主耗时，优化共享查询并验证输出等价；不以并行请求掩盖单次成本。

保持 canonical current、有效成功 head、来源限制、unknown 身份、历史状态、分页 scope/asOf 和筛选先于分页。GET 不准备缓存、不启动研究、不调用模型。禁止 shared DB 写入、migration/reset、模型/预算设置修改、采集或视觉重设计。测试只使用合成材料与隔离 PostgreSQL。性能回执只含汇总数值，不含真实评论正文或凭据。

验收：读取结果等价、受限和正文变化即时反映、分页与跨领域语义保持；有意义的隔离 PostgreSQL 回归与计划成本防护；相关前端测试、编译/格式与治理检查。最终候选 PR、exact head、验证与剩余边界供用户审查；本轮新候选的合并/本机发布另按最新授权执行。

## 表面、状态、依赖与验收地图

| 表面 | 保持的状态/含义 | 依赖与验收 |
|---|---|---|
| 用户评论列表/筛选/翻页 | 正常、空、受限、身份未知、索引部分、最新失败与历史有效结果不混用 | 同一目录 SQL 同时返回页与未分页统计，统计使用该页 asOf；隔离 PG 检查搜索/分页/资格与限制 |
| 情报总览 | 目录事实、研究事实、知识事实各自独立，读取失败不当作零 | 复用目录统计事务；有效 head/Problem 回归与只读 HTTP 对比 |
| 作品选择器与评论详情 | 先筛选后分页；历史/父语境/限制保持 | 相同只读事务局部关闭 JIT；作品目录与详情 PG 回归 |
| 页面加载 | 读取失败保留错误，无新研究/预算或设置动作 | comments 返回附加 summary，去除重复 catalog-summary 请求；现有前端路径及 HTTP 统计语义验证 |

SQL 实测：同一目录 EXPLAIN 总执行 1574.454ms，JIT 1387.538ms；事务局部 jit=off 后 190.167ms。此为单次 SQL 描述性结果，尚未证明候选 HTTP 或正式 3000 已改善。保留 catalog-summary 独立 API；新增 summary 是兼容字段，其未分页统计与返回页共享范围和快照。

## 稳定性与研究批次后续定位

目录过滤 SQL 复用预编译通用计划时，jit=off 仍出现 3691.443ms 的搜索计划；因此只对目录/单条同源投影与作品目录使用匿名、不持久化的参数计划，避免跨筛选/连接次数的性能退化。确定性守门会在全部4个后端强制 JIT 成本阈值为0，验证4次公共读取 JIT functions=0、连接默认恢复on，并在12次混合搜索后检查目录计划未缓存。维护清洗 SQL不属于该检查范围。

研究批次附加计数原 SQL逐 Run反复展开3次有效依据，单次计划624.898ms。候选先从权威有效Signal视图选择页内Run集合，一次materialized展开，再分别汇总pending Resolution和同一Run的pending Pair；局部事务jit=off后7.751ms。正式库同一只读快照内17行双向EXCEPT ALL差异0。新增隔离负例覆盖跨Run Pair不计入任一Run、新no_signal head失效、来源限制即时失效。

## 候选验证与部署边界

正式3000 baseline和强制只读连接的独立3001候选，各3次GET中位数（ms；验证期间有隔离测试活动，不代表P95或冷启动保证）：

| 接口 | baseline | candidate | 原有字段（忽略asOf/cursor编码） |
|---|---|---|---|
| overview | 2442.6 | 640.1 | 相同 |
| comments | 2707.8 | 373.5 | 相同 |
| catalog-summary | 2385.9 | 230.1 | 相同 |
| works | 1492.7 | 174.7 | 相同 |
| runs | 611.9 | 79.8 | 相同 |
| problems | 77.9 | 62.2 | 相同 |
| search | 1969.5 | 253.3 | 相同 |

12次交替搜索/列表235–270ms；第二页1964.0→245.8ms，原有字段相同；目录附加summary与独立统计相同。真实库响应仅在内存比对，不保留正文。合成Playwright UI回归通过并禁止额外catalog-summary请求；实际候选浏览器显示50行，完整7526条统计，第二页50行且统计不变。当前main/runtime仍3830d4b6，本次仅候选，不代表已刷新3000或Mog业务验收；未创建研究、改动模型/预算/后台策略或发起provider。既有后台任务可独立继续运行。


最终验证回执：84项隔离PostgreSQL（1+5+4+5+3+66）全通过，JIT guard catalog_calls=4/jit_functions=0、4个后端默认on且重复搜索后目录prepared语句0；新增批次负例通过。43项相关API单元通过（12项明确ignored），18项相关lib单元通过；build、变更Rust格式、JS/bash语法、diff和项目治理通过。合成UI通过；真实候选浏览器已验证列表、第二页、ADHD搜索与研究批次入口。commit-reviewer只读核验完整diff与实际日志，结论PASS；两次测试准备/旧断言失败均已修复，不计为通过。隔离数据库、容器、卷cleanup verified。无关main全仓格式差异未改写；本次没有宣称全仓格式通过。PR CI与合并/部署回执在后续另记。

## 2026-10-10 概览进入等待的后续修复

PR #380 已合并为 main 732bedef；其接口时延不代表页面进入时延。Mog 反馈进入概览仍近7秒，当前代码在初始化时串行等待 loadSetup → 作品/方法 → runs → overview。只读测量 setup 4197.1ms，overview 572.5ms；弹窗准备扫描不属于概览读取依赖。继续 Issue #358，单一实施者在 codex/comment-study-entry-performance，提交前独立 reviewer。

分类为既有加载交互与状态修复（L2 页面局部）；读取 UI execution contract、design/README、design-governance、LIDS README/data-boundaries、comment-study-productization-001 与现行页面/HTTP 合同。Token/Primitive/Component/Pattern 和页面布局均沿用，不新增视觉值或业务能力。

| 表面 | 状态与依赖 | 验收 |
|---|---|---|
| 概览/评论/问题入口 | 直接读取当前视图；不等待研究弹窗 setup、works、policies 或无关 runs | 合成慢/悬挂准备接口时概览仍可展示；初始请求只含当前视图 |
| 研究批次和 URL 返回 | 需要 run 列表才加载；显式 runRef 保留；render token 保留防止旧响应覆盖 | 四视图、深链、Back/Forward、批次分页现有回归 |
| 发起研究/所选评论弹窗 | 点击即展示已有弹窗，准备中不允许预览/启动/保存方法；同一准备请求合并，关闭再开不重复；失败明确并可再次打开重试 | 悬挂 setup、快速开关、失败恢复、评论选择保留；所有动作只走合成 API |
| 现有研究方法/模式/预算 | 成功准备后沿用现有资格与预览签名规则；GET 不创建 Run | 完整产品化合成与隔离 API 浏览器回归，无真实 provider |

只修改页面 JS、必要初始按钮状态、相关测试与已索引文档；无 migration/后端语义/模型/预算/自动排程改变。真实页面仅只读检查；不保存真实正文。最终需报告从导航到可用概览的实测或测量边界，不能再将接口中位数写成页面进入证明。

后续候选验证：完整合成UI、相关API单元43+1（12 ignored）、build、JS/Python语法、diff与治理通过。新悬挂setup负例在正式基线JS上预期失败，候选通过。独立review的P2首次方法提前可保存反例成立，修正为完整准备后打开编辑器并加入入口守卫；新增空方法目录+悬挂works用例验证保存/编辑禁用、完成后首次创建仍可用、全过程无POST，reviewer独立真实函数反例复跑及复审PASS。真实只读浏览器初轮正式4637/4500/4401ms、候选668/657/656ms；最终构建候选重复刷新883/783/592ms（中位783ms），包括自动化往返开销，仅代表该本机样本，不是P95或Mog验收。真实弹窗准备中预览/启动/保存禁用，关闭后概览保留；未执行研究命令。PR/精确head CI、合并与正式刷新仍待各自回执。
