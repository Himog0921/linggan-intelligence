# CREATOR-DISCOVERY-001 实施与验收

> 状态: 一次性报告
> 最后核对: 2026-10-09
> 适用范围: CREATOR-DISCOVERY-001 候选验证、授权发布和性能收口
> 事实来源: 当前分支代码、隔离 PostgreSQL、合成 Pi adapter、HTTP 与独立代码审查
> 冲突时以谁为准: 用户最新授权、当前代码/运行与可复现测试；本报告不授予部署或真实调用权限

## 交付对象

基线 main `5f31b8588a0d4e2d6b8b1a7b55656b311516b6ff`。开发工作树为 `<repo>/.worktrees/creator-discovery-001`，未写入共享 checkout。需求、UI 依据和冲突裁定见[执行规格](../plans/active/creator-discovery-001.md)。

页面 `/corpus/creators` 从当前领域的材料用途读取作品、稳定作者和来源分析；通过现有语料路径读代表作与完整命中集合。迁移新增用途策略、作品/作者派生结果与有界租约状态，并统一作者归属的共享 owner。具体实现由 GPT-6 Sol subagent 执行；主代理负责范围、架构和集成审查，另有独立 Sol reviewer。

## 初次候选验证账本

单元、合成数据和本地候选运行都不能代替共享环境部署或 Mog 验收。临时日志按产物登记规则保存在 `/tmp/creator-discovery-*.log`，不提交业务数据或构建产物。

| 层级 | 结果 | 证据及限制 |
|---|---|---|
| API / worker 编译 | VERIFIED | `cargo check -p linggan-api -p linggan-worker --locked`；最终日志 `creator-discovery-final-check.log` |
| 单元与二进制测试 | VERIFIED | API 304、contracts 21、evidence 96、intelligence 134，共 555 通过；42 项按原测试标注 ignored，未计为通过。日志 `creator-discovery-final-unit.log` |
| 8 作者 / 12 作品基准及反例 | VERIFIED | evidence PostgreSQL 2/2，日志 `creator-discovery-pg-final.log`；作者8、作品12、未知身份1、相关作者7、垂类2、亲历4、名单外6、名单外爆款3；组合亲历爆款命中2。包含精确回链、同篇交集、关系查询失败、人工支持片段失效、角色保留、其他暂停领域主角色冲突与并发登记 |
| worker 合成模型路径 | VERIFIED | PostgreSQL 1/1；真实本地 Pi 子进程与 invocation 账本，预算、资格、三次有界失败、租约/孤儿回执恢复、指标变更不重跑、人工结果保留。日志 `creator-discovery-worker-dispatch-admission.log` |
| 共享作品读取回归 | VERIFIED | `material_projection_postgres` 串行 17/17；日志 `creator-discovery-shared-regression-serial.log` |
| 真实本地 HTTP | VERIFIED | 候选 API + 一次性 PostgreSQL，37/37；页面与资产、列表/代表作、域隔离、语料精确回链、非法参数、跨域写拦截、登记角色/状态保留、策略 CAS、人工覆盖/恢复、重试边界与暂停域写保护。日志 `creator-discovery-http.log` |
| 静态与治理检查 | VERIFIED | 三项 JS 语法、运行脚本语法、diff 空白检查、项目治理及 UI 手册治理检查；这些不证明视觉体验 |
| 浏览器视觉及交互 | NOT VERIFIED | IAB 与 Chrome 对专用 3107 地址返回 `net::ERR_BLOCKED_BY_CLIENT`；没有截图、完整页面点击或 Mog 验收 |
| main / origin 集成 | NOT VERIFIED | 当前候选分支；提交或 Draft PR 不等于合并 |
| 共享 migration / 3000 服务 | NOT VERIFIED | 本轮没有应用到业务库或更新常驻服务 |
| 真实模型语义与平台采集 | NOT VERIFIED | 未调用真实模型端点，没有发起平台采集 |

共享作品 PG 首次并行执行触发测试容器 PostgreSQL 锁内存上限，串行重跑 17/17 通过。worker 的来源变更用例最终使用子进程 ready/release 屏障，避免以 sleep 猜测模型已经发出。屏障还证明：调用已经准入时禁用策略可以提交；旧调用的实际合成用量仍结算，但不能覆盖已改变的任务快照。

## 隔离证明的再生方法

使用任务专有的 PostgreSQL 16 + pgvector 数据库，`LOCAL_001_PROOF_DATABASE_URL` 必须指向可丢弃的测试实例。fixture helper 会重建命名 schema，不得给它共享业务 DSN。以下数据库用例必须串行执行。

```sh
cargo test -p linggan-api -p linggan-contracts -p linggan-evidence -p linggan-intelligence --lib --bins --locked
cargo check -p linggan-api -p linggan-worker --locked
cargo test -p linggan-evidence --test creator_discovery_postgres --locked -- --ignored --nocapture --test-threads=1
cargo test -p linggan-intelligence --test creator_discovery_worker_postgres --locked -- --ignored --nocapture --test-threads=1
cargo test -p linggan-evidence --test material_projection_postgres --locked -- --ignored --nocapture --test-threads=1
./scripts/check-project-governance.sh
./scripts/verify-ui-design-handbook.sh
```

worker 测试另需 `CREATOR_PROOF_NODE` 指向本机 Node 可执行文件；子进程为已检入的 `tests/support/creator_discovery_adapter.mjs`，只解析真实 Pi 请求并返回合成 JSON，不连接模型端点。预占、完成和故障回执经过现有 model invocation owner。

临时 HTTP 检查脚本为 `/tmp/creator-discovery-http-check.py`，运行方式 `python3 /tmp/creator-discovery-http-check.py`；它依赖上述 fixture 与候选 API。HTTP 核对使用候选 API 的独立 3107 监听，数据库仅指向上述 fixture。测试覆盖读取和必要的合成写命令，实际业务库、常驻 API、worker、插件均未参与。HTTP 验收后、fixture 重建前的快照：WorkOrder 0、model invocation 0、作品人工覆盖非空0，领域已恢复 active，跨域参照登记保持原 dismissed 状态。最后一项前端弹窗上下文保护只做 JS 与窄代码复验，未将之前 HTTP 结果冒充该交互的浏览器证明。

## 审查结论与运行语义

独立审查发现的历史额度字段失配、focus 引用不足、简介更新缺少重新排队、调用资格、失败恢复、机构身份线索、媒体显示、分页回链及关系未知态均已修正。最后复验没有剩余已确认 P1/P2。主代理另修正切换领域时旧行可操作、保存阈值意外改模型选择及前端自行格式化指标时间的问题。

人工积极判断须绑定当前支持片段；依据失效时读为 unknown 并保留原记录。已有目标的角色、暂停/忽略与规则原样保留；跨领域新增主角色冲突由用户显式改选参照处理，登记不创建 WorkOrder。

模型调用在短事务持久记录派发准入后执行；网络等待期间不持有数据库事务。禁用用途阻止后续准入，已准入调用可完成结算。返回结果必须同时匹配 lease 和语义指纹，指标变化本身不触发重新语义分析。

## 部署前置及验收责任

迁移 0115 是新源码的共享作者归属与分析状态前置；代码提交、测试通过或 PR 不意味着已迁移或已部署。共享迁移、合并与更新本机 3000 需要针对最终版本明确授权。正式来源分析用途默认关闭，由用户明确选择模型与预算后才运行。

Mog 仍需在可访问的前端验收真实目录、筛选、依据、返回和纳入观察的产品效果。真实模型效果只能由已授权的小范围真实调用另行证明。

## 授权发布阶段的补充检查

Mog 随后授权合并 main 与刷新3000。#368 已合并为 `b182ba83`，实际发布前额外执行 `scripts/test-local-001-discovery-postgres.sh`。它暴露此前定向证明未覆盖的两个来源强度消费者：生命周期把主页推得作者计为详情确认，语料 collection context 把它计为 MATCHED。修复从唯一归属 owner 透传 attribution_source；详情确认与目录关联继续分责，不改公共 JSON 或0115字节。原失败断言保持，生命周期补充合格目录作品点反例。

另外两个 control-only 裁剪测试夹具误加0115但不具备模型依赖，已移除这两处无关 include；完整 material/API fixtures 仍覆盖迁移。最终项目入口 `scripts/test-local-001-discovery-postgres.sh` 已完整通过：26次Rust测试调用累计283项通过、Node 3项通过，零失败；worker默认轮忽略的5项在后续数据库轮全部执行通过。关键生命周期5/5、采集控制21/21及运行时15/15、派发37/37、API39/39。日志 `/tmp/creator-discovery-release-local001-final3.log`，退出0且数据库、容器、卷清理已核验。脚本依据原有边界明确排除两条要求 `P1_BROWSER_PROOF=1` 的独立浏览器用例，这两项不计为通过。


## 0115 授权迁移与本机发布（2026-10-08）

本节取代上表中 main、共享 migration、3000 及浏览器访问限制的历史状态；隔离测试和真实模型边界保持。Mog 已在当前对话明确授权提交推送合并及刷新3000，另明确授权0115迁移和必要备份。

- #368 与 #370 已合并，部署目标 `54cccc3dd238124378dcaff31b58b8a57719a918`。加密备份96620304字节，权限0600；完整解密流及69684条归档目录校验通过，未落地明文。
- 0115 已应用；台账 checksum 与源文件 `b935e46868c90819235981749a680ff1a5ae9eb0d5ba69cb9d51e5521c4aed7c` 一致。原模型配置、研究策略、Run控制和调用计数核对不变；创作者policy/job均为0，未启用来源分析。
- canonical `runtime-main` 的 API 16703、worker 16709、media worker 16716 已分别核验cwd/可执行文件；health为ready，数据库READY。它们是本次点时PID，不承诺后续不变。
- 浏览器3000可访问；考研自习目录1位作者、24篇作品，点击待判断24进入精确语料，再返回同领域创作者。截图为系统临时目录的 `creator-discovery-release-page.jpg`。
- ADHD领域1657篇作品请求超过120秒未返回；不能把小领域成功当作大领域验收通过。性能修复在同一交付包继续，结果后续登记。

私有备份与发布回执：`database/backups/creator-discovery-0115-20261008T145542Z/release-receipt.json`（已按生成物登记忽略，不入Git）。真实模型质量、平台采集和Mog业务验收仍未验证。


## 大领域读取性能修复验证

原实现对每篇派生候选分别查询；本次由 `material_media_read` 统一按批读取，在批内复用当前撤回集合，每篇独立计数和截断。创作者列表每100篇一批，仍对全领域集合计算统计和指纹。相同创建时间新增 `job_ref`／`derivative_ref` 稳定并列序；原先未定义并列顺序，现在单篇与批量共用这一规则。

- 隔离 PG `creator_discovery_postgres` 最终3/3通过，日志 `/tmp/creator-discovery-batch-proof5.log`。固定预期验证非空 OCR、ASR、三键撤回、OCR退役、未来job、重复origin去重、每篇256+1与稳定并列序；数据容器、数据库和卷已清理。
- 3107候选使用业务库的只读事务连接，不启动worker；读取1658篇作品、953位作者。首页/第二页/末页返回50/50/3行，耗时1.214/1.066/1.033秒，作者键不重叠、统计一致。摘要回执 `/tmp/creator-discovery-performance-candidate.json` 不含作者身份或作品正文。此为有界点时测量，不是长期P95承诺；原超时盘点后系统新增1篇，不能伪称数据快照完全相同。任务专用3107 PID27120已核验并停止。
- 独立 GPT-6 Sol reviewer 对冻结差异、共享调用点和最终PG日志复审，未发现已确认P1/P2；主代理核对撤回三键join与原bool_or语义、全量集合及分页，结论一致。API build和diff检查通过。
- `cargo fmt --check --package linggan-evidence` 因既有未改文件格式差异未通过；未格式化无关源码。实际编译与数据库证明单独记录，不将该检查报为通过。

- 性能修复最终完整 `scripts/test-local-001-discovery-postgres.sh` 退出0：Rust283、Node3通过，零失败，隔离数据库/容器/卷清理已核验（`/tmp/creator-discovery-performance-local001.log`）。两条独立P1浏览器用例按原入口边界排除，未计为通过；新增媒体资格测试另为3/3。项目治理检查通过。


## 领域切换页面上下文收口

性能修复 #371 合并并部署为 `65beecce` 后，3000 实测1658作品/953作者约1.258秒返回，API/worker/media分别PID51009/51015/51023且cwd/executable均来自canonical runtime；配置、预算、模型调用计数与发布前一致。

浏览器发现正文选择另一领域后，SSR页头和侧栏未刷新。窄修改为领域切换完整导航，让服务器统一渲染壳和正文上下文；普通筛选继续AJAX。加载清coverage/pager；导航取消防抖、废弃旧读/弹窗序号，阻止焦点加载及在途注册回执更新旧页；跨领域历史导航重载壳。仅JS改动，未改API、数据库、worker和迁移，node语法与diff检查通过。正式浏览器复验记入本机发布回执，不将之前性能证明当作这一交互的验证。

- 独立复审提出BFCache恢复可能保留导航中空表状态，已确认并以`pageshow.persisted`重载修复；普通加载不触发。语法、diff、项目治理和UI手册治理均通过。旧coverage残留发现已在最终diff中消除，不列为未解决问题。

- 最终历史恢复修复采用pageshow/popstate合并的setTimeout(0)，首次读取与历史表单恢复共用URL入口；focus在初次恢复/待恢复时不读取，pagehide废弃旧请求。node语法、diff与API build通过。3107真实只读候选已走通ADHD953→考研1/24→Back ADHD→Forward考研，并逐一核对下拉、SSR页头、3侧栏链接及作品回链；待判断24进入实际24行语料并返回考研。独立Sol复审中提出的focus抢跑反例已修正。无Rust、API/schema或迁移变化。

## 2026-10-09 · 长列表滚动、分页与信息层级候选验收

当前正式 `:3000@fc2b9b2c` 的 1280×720 浏览器复现：`body` overflow hidden、创作者主区高 8413px 且 overflow visible，50 行表尾分页位于视口下方约 8.4k px；PageDown 后窗口和主区滚动均为 0。这是用户无法继续向下看作者的直接页面原因。正式 3000 仍是修复前版本，此段不把候选当成已发布。

候选仅替换工作树 `creators.html/css/js`，通过拒绝 POST 的本机 3107 临时只读代理读取正式 3000 的 GET 数据；未写共享库、未改 API、模型、worker、迁移或全站壳层。浏览器以 ADHD 当前 966 位作者核对：桌面主区高 592px、内容高 6643px，PageDown 将主区滚动到 572px；页面纵向可达。上/下分页均显示 20 页，进入第 2 页为 50 行，输入第 20 页为 16 行且下一页禁用；输入 99 时就地提示“请输入 1 至 20 页”，URL 留在原页；浏览器 Back/Forward 重新取得对应第 2/20 页。换页后定位到结果导航，URL 保留筛选与 `page/pageSize`。

390×844 视口下，页面滚动总宽 390px 无整页横向溢出，表格 1024px 宽在自己的容器横向滚动；PageDown 使文档纵向前进。主视图把“在讲什么/来源特征”合为“内容线索”，没有线索的行仅出现一次“待分析”；来源分析未启用在统计区底部统一说明，垂类/亲历数仅统计已有判断，已确认相关 0 位也在摘要中限定为已有判断。长平台账号不再占据列表行，打开作者依据抽屉仍可见带中文标签的“平台账号”用于回查。候选页面真实截图已由浏览器现场查看；未把合成数据或纯 DOM 断言当视觉验收。

独立提交前审查在候选浏览器证实一条 P2：直达合法的 `page=999` 时，API 返回 `total=966/items=[]`，旧候选仍显示“第1000/20页”。实施者随后按服务端总数夹取最后有效页，替换 URL 并重新读取。主代理在同一只读代理的真实浏览器复验：`page=999` 自动改为 `page=19`，返回16行、显示第20/20页且下一页禁用；此前输入99的表单校验仍保持当前页。`total=0` 分支的静态执行检查显示回到 `page=0` 且不发第二次请求；独立窄复审仍待回执。

这轮候选的 JS 语法与空白差异、LIDS 手册和项目治理检查已通过；独立提交前审查确认 P2 已修复，最后统计提示收口亦经窄复审，无未解决的已证实缺陷。PR exact-head CI、main 合并、正式 3000 刷新与 Mog 业务验收分别以后续回执为准。无 LIDS Token、Primitive、共享 CMP、静态参考页或材料迁移，本次不生成新的视觉规则/例外；现有 L1 Corpus Explorer 与运行时 Token 原样复用。
