# COLLECTION-READ-PERFORMANCE-001 · 采集读取性能

> 状态: 活跃计划
> 最后核对: 2026-10-14
> 适用范围: Mog 当前授权的采集读取性能交付包，关联 #398，独立分支 codex/collection-read-performance
> 事实来源: 当前用户授权、main 90ca30a75ebe2ba1b86739e98f5a54fb67f02fd8、正式只读实测及既有采集合同
> 冲突时以谁为准: 用户最新确认、真实源码/运行/测试、既有领域合同

## 本地执行声明与边界

执行者 /root；稳定 task-id COLLECTION-READ-PERFORMANCE-001。独立 worktree 为 /Users/moglenny/proma/linggan-intelligence/.worktrees/collection-read-performance；基线 90ca30a75ebe2ba1b86739e98f5a54fb67f02fd8。初始授权为实现与实测；Mog 随后明确授权本候选提交、推送、合并并部署/刷新本机3000。仅恢复API/recovery读口，两个worker继续disabled；不包含迁移、后台恢复、插件或真实平台/模型调用。Mog已另行允许向私有Issue #398发布声明并设当前账号责任人；已发布声明见 https://github.com/Himog0921/linggan-intelligence/issues/398#issuecomment-6158957433。本地声明不能冒充 GitHub Claim 或独立审查。

代码范围：apps/api/src/local_web.rs、collection_control_surface_view.rs、collection_workspace.js，crates/evidence/Cargo.toml（复用工作区 Tokio join 宏）、crates/evidence/src/archive_completeness.rs（目标读取的直接共享依赖；仅并行原有三份独立 SQL，不改统计或资格条件），以及直接验证其读取/交互合同的测试。文档范围为本计划、docs/README.md、docs/current-state.md、docs/progress/2026-10.md 和既有 Collection 页面性能说明。其他代码、schema/migration、插件、worker、准入/Lease 命令、模型和平台动作禁止修改或触发。共享 checkout 脏改动不吸收；主线再次变更时重新核对，不擅自集成。

## 表面、状态、依赖、验收

| 表面 | 状态与不可替代的含义 | 依赖 | 验收 |
| --- | --- | --- | --- |
| 任务进入/刷新与执行诊断 | 全部统计、最新活动排序、未知与零、历史与 live Lease 均保留 | #414 已合并任务 SQL；冻结 Work 只读投影 | 验证当前改进已生效；只读任务投影与原 works 等价；详情/失败回归 |
| 目标列表、领域与筛选 | 原统计完整，读取失败独立保留；不将单次来源加入名单 | target counts、domains、archive/observation/catalog | 有数据完整 HTML 等价；多领域/空/未知原测试；进入/刷新实测 |
| 目标抽屉、作品、巡查、关闭/返回 | 获取完成后显示服务器真实投影；未读到不伪造零；失败正常整页导航；不缓存资格/Lease | 原 drawer renderer、来源资格与 inspector；同源 GET 片段 | 片段与整页抽屉等价；不读列表；链接/键盘/焦点/宽度/刷新/back；并发过时响应不可覆盖 |
| 原动作/规则/批量/删除/管理 | 原 POST/准入/回执，不走局部读取机制 | 现有命令链与管理脚本 | 原回归，不触发正式命令 |

## 实施顺序与真值边界

1. #414 已替换逐 Task 全扫 Attempt，本轮不重复修复；保留全活动排序与完整诊断。
2. 任务详情只读取并使用冻结 Work，免读未显示的调度和工位能力；不改真正准入校验。
3. 目标各独立读取并行；抽屉同源只读片段免读列表，前端保留列表节点与选择、URL、焦点和整页无 JS 路径。每次打开仍重新读取抽屉事实，不缓存来源资格或 live Lease。
4. 隔离 PostgreSQL 与行为测试；候选 API 仅 loopback/recovery，数据库连接强制 default_transaction_read_only，不启动 worker、不调用提供商或平台。正式 3000 不重启。候选实测与主线区分报告。

所有命令及数据口径仍由正式来源负责；浏览器 DOM、HTTP 200、代码通过和局部查询对比不冒充真实采集或 Mog 业务验收。原 2026-10-13 任务 16–23 秒基线是旧版本；当前 #414 任务 HTML 初测 661ms，目标 HTML 1307ms，不能将已发布改进记成本分支成果。

## 本轮候选验证回执

实现已写入本独立分支，尚未提交/推送/合并或部署。候选 API 在本机3001以 recovery 与 default_transaction_read_only=on 连接现有数据，只启动API；正式3000 PID53237及其main90ca30a7未修改，巡检和媒体worker仍为disabled。本轮临时API和浏览器已在验证后关闭。

原采集隔离PG 149项＋新增片段/冻结Work 2项通过；补充档案38项、检查器1项和两项片段/Work复跑通过（第二组41次执行）。137项Collection单元测试通过，15项ignored不计通过；5项纯JS事件/传输合同测试通过。首次新增无DB断言误用大写DOCTYPE而失败，改为大小写无关后重新通过；未将失败轮计入通过。新档案并行第一次编译缺少生产Tokio宏依赖，已显式复用工作区依赖并重新构建成功，未改变Cargo.lock。

当前主线/候选六个有数据完整HTML逐字相同（全部领域、具体领域、关键词筛选、任务、目标概览/作品）；两份抽屉片段与各自完整页中的抽屉相同且不含列表。档案SQL字符串与资格宏未更改，三份读取并行、原合并顺序保留。隔离证明还验证工位凭据读取不可用时任务冻结Work仍可完整读取，未把该缺失变成0或新执行权限。

真实浏览器操作至就绪（含自动化开销）：目标进入1192/976ms；刷新966/1326/1081ms，中位1081；抽屉打开1152/1097/969ms，中位1097；作品切换946ms；关闭289/329/350ms，中位329；任务进入824ms，刷新716/1090/617ms，中位716；用户任务详情372/375/363ms，中位372。作品深链接刷新1379ms恢复正确页签；Back、Escape、焦点和列表已选项保留验证通过；console error 0。两次使用不存在的就绪选择器的观测错误已排除出耗时样本，不是页面加载失败。

HTTP候选稳态目标完整响应约780ms，任务约513ms，目标片段约719ms。任务首要瓶颈已由此前#414修复，本轮不将其16–23秒降至约1秒的收益记成本分支成果；目标刷新仍约1秒，单次SQL仍有全历史聚合，不宣称所有入口秒开或大规模持久投影已建立。

临时证据：/tmp/collection-read-performance-http.json、browser.json、postgres.log、archive-proof.log、final-unit.log、governance.log、handbook.log（均带collection-read-performance前缀）。未持久化业务正文或凭据；这些是点时本机读证据，不证明真实采集/模型/平台链路或Mog业务验收。外部Issue Claim、独立review、PR、提交推送、main集成与正式部署仍未执行，不能用本地候选验证替代。


## 发布阶段

2026-10-14：Mog授权提交推送、合并部署并刷新3000。代码提交前按AGENTS要求由commit-reviewer独立只读审核；结果待实际回执。目标main90ca30a7未漂移；沿用现有API-only/recovery发布保护，绝不调用会恢复三个服务的通用install入口。提交/推送/PR/CI/合并/正式浏览器的分层结果将在实际发生后记录，候选段中的“未发生”为该阶段历史事实。


### 提交前独立审核与修正

commit-reviewer独立只读审核发现P2：Ajax挂载只初始化drawer本体，没有重新增强作品的data-drawn-select。已改为幂等初始化，mounted时增强新抽屉，不重复加控件；原生select仍是唯一值与change源。审核员执行复现、API/evidence cargo check、无DB片段测试及最终6项JS并给出PASS；基线与候选三份档案SQL全部字符串/资格宏序列也已独立比较一致。主代理真实CUA确认Ajax作品抽屉只有一个自绘筛选，选择“待采详情”后nativeValue=pending、label一致、菜单关闭；console error 0。此前5JS是候选阶段的历史次数，最终6JS通过。其他Rust/SQL未改，不将自审冒充独立审核。

当前发布授权只覆盖这一完整差异；Issue #398的整体业务验收和真实采集仍不随本PR关闭。远端流程按实际运行/跳过报告；完整正式发布结果以本机运行身份和 /tmp/collection-read-performance-release-receipt.md 为准，不能从提交推送或PR推断已部署。
