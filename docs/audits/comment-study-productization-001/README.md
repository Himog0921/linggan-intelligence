# 评论研究产品化 · 来源和验收账本

> 状态: 权威当前
> 最后核对: 2026-09-28
> 适用范围: COMMENT-STUDY-PRODUCTIZATION-001
> 事实来源: 用户提供的手册v1.0与本分支的实际提交
> 冲突时以谁为准: 实际执行证据与对应版本合同

- `source-lock.json`：成文时固定源码和LIDS附件的来源回执，不是当前全量部署状态。
- `document-check.original.json`：2026-09-22文档交付时的静态检查原回执。保留其“未写GitHub”等历史描述，不以它报告本次状态。
- `acceptance-status.json`：T01–T54实际业务验收台账；每条由对应层级的执行证据才能改为PASS，文档或Schema检查不能替代Rust/数据库/浏览器证明。
- [P0来源与现场边界](p0-execution.md)：源码部分已核对，本机数据库与在途调用未核验。
- [P1前序回看](p1-read-counts.md)：批次计数、schema-phase候选、清洗缓存、评论目录/详情/历史和各次验证边界。
- [P1作品目录与共享标题](p1-work-catalog.md)：`/works` 后端、Evidence标题复用、迁移编号对齐，以及隔离PG/合成浏览器回执。

## 当前剩余工作：严格沿用原 P0–P8

本表是 2026-09-28 的新快照，替代上方历史增量在当时记录的旧状态。PR #338 仍 OPEN/Draft，最新含代码的实现 head=`f92e13c93e1065ad66fd21fbc434a50638dc64d4`，base=`main@2587fba486434acf2c94a8e7f64ce245b87a38f6`；exact-head run [36373080844](https://github.com/Himog0921/linggan-intelligence/actions/runs/36373080844) 成功。该 head 含 SIGKILL 后 worker 重启恢复用例；本轮本机隔离验证为 Intelligence PG 66/66、Axum/PG 9/9、SIGTERM 1/1、SIGKILL recovery 1/1。各次 `integration-source-export` 按 push-only 条件跳过，工作流中的前端行为脚本不存在，因此没有实际运行 UI 自动化。PR 历史中 #323–#327、#329、#335、#336 已各自合并 main；这些并非 PR #338 的合并，#338 自身尚未合并，也未进入当前 `main`/本机 `runtime-main`。本地隔离证明不代表共享迁移、生产运行时、人工标注或业务验收；工作树已整合当前 main，测试数量不折算成阶段百分比。

| 手册阶段 | 当前已有证据 | 未完成/下一关 | 状态 |
|---|---|---|---|
| P0 外围接线与隔离启动 | 2026-09-27 只读确认 `:3000` health ready、`runtime-identity=2587fba4`、共享开发库 migration head 0104；聚合快照为 4,880 material comments、7 Study Runs、945 Targets、1,412 semantic attempts、764 Signals、0 Problems；prepared/leased batches 0/0，running invocations 0，其他 active SQL sessions 0。Request ledger 不存在（0105 未应用） | 数据行数/采样 hash 与来源资格仍可补作 P8 恢复记录；P0 不作为当前代码推进的前置关卡 | 运行身份和在途计数级状态已核；共享 migrations 未应用 |
| P1 原声、清洗与语境 | 前序 #323–#336 已并入 main 的材料/评论目录、详情历史、清洗和选择能力；本轮保留 main 的统一 domain usage 并重跑既有回归 | T01–T54 全量、剩余边界及用户指定 exact-head 的完整产品验收 | 已有交付在 main；整阶段验收未结 |
| P2 选择、预算、启动与取消/重试 | 方法版本/读取复制、按领域默认 CAS、显式预算、共享 preview/start、请求幂等与并发隔离、正式 router 和方法/Run UI；v2 Run→semantic worker、冻结请求快照、调用前 fence、Run 级预算及 usage 累计；Run 取消、deadline 到期拒绝迟到结果、按剩余 deadline 限制 provider timeout、部分响应停止收口和新 Run 显式 `retry_failed`。本地隔离 PG 57 项 + Axum/PG 8 项，Intelligence unit 123/123、API 290 passed/37 ignored；exact-head CI 36342688035 通过 | 真实浏览器逐动作/逐数字与 Mog 接受；估算与 provider 实测用量差异仍需完整账本承接 | 代码候选与精确 head 自动检查通过；用户页面/业务接受未完成，P2 整体未结 |
| P3 有界执行、质量与恢复 | scheduler 公平轮转/有界恢复（`891a76f6`）、T23–T26、M4 Rust adapter→WeMM 合成探针、SIGTERM drain、同作品双已知作者 cold-start 事务测试均已推送并有 exact-head CI 成功。SIGKILL 后 worker restart 恢复测试随 `f92e13c9` 推送，exact-head run 36373080844 成功。本地隔离套件 Intelligence PG 66/66、Axum/PG 9/9、SIGTERM 1/1、SIGKILL/restart recovery 1/1。 | 完整 semantic/resolution/pair 故障与部分接纳矩阵仍需逐项对照；实际 `runtime/sync.sh` 发布/restart演练、Gold Set/离线 Recall@K、模型真实端到端语义与HTTP页面路径仍未验收。M4 探针未采集 system memory pressure/swap；真实浏览器及 Mog 验收另列 P7。 | 有界调度、T23–T26、本机 M4、优雅关停/强制终止恢复和冷启动事务合同有本地与 exact-head 证据；P3 阶段出口未达成。Recall@K 等独立人工标注（`BLOCKED_ON_HUMAN_ANNOTATION`），不是验证失败 |
| P4 长期归并与纠偏 | 既有 Problem、membership、撤销/重研等候选路径可复用 | 有效 head、跨下游限制传播、合并/撤销及支持版本历史的同版 PG 证明 | 部分候选；整阶段 NOT_VERIFIED |
| P5 指标、语言与情报接口 | Intelligence 已有材料和事实读取基础 | 六视角、窗口/cohort/distinct 对账、durable event 与下游消费 | 未形成完整验收 |
| P6 每日增量与资源 | 自动化衔接仅有设计 | dirty 合并、时区/计划开关、预算续办、退避及资源争用证明 | 未启用、未验收 |
| P7 前端完整验收 | P1 有历史合成浏览器回执；本轮 P2 UI 代码已接通 | 真实接口逐动作/逐数字核对、空错旧态、键盘与多视口；Mog 业务确认 | NOT_RUN |
| P8 小流量发布与恢复 | 当前没有本轮生产调用或发布 | 备份/隔离恢复、锁定依赖、合规小流量、发布/回退回执与完整差异表 | NOT_RUN |

数据库的constraints/views、历史身份回填和所有新写入约束是P1–P4配套工作，不额外算成一个独立系统；完整同版验收后才能注册/运行迁移。原型全文入库、月报正文追加和大型文件格式检查同样不能漏在集成之外。

每日自动化（计划表/UI、到期扫描、时间槽防重、每日额度、启用运行）属于 P6；当前只有设计，未启用。P2 的启动与预算为后续自动化提供基础，但不证明每日调度已经落地。

机器合同位于 [data-contracts](../../data-contracts/comment-study-productization-001/README.md)，完整技术入口位于 [手册包](../../runbooks/comment-study-productization-package.md)。新执行记录按阶段追加，并引用手册章节及测试ID；不得用新设计默默覆盖过去已经批准的字段。


## 当前推进点：P2 方法合同（2026-09-24）

Mog 已在当前对话报告 P1 手动产品验收合格并授权 P2。用户未提供手动验收所用 exact head；记录时 #338 head 为 `3c124697b1154cd6141d7d1b72e4de97991ed4ac`，不将两者等同。以上阶段表保留为前序实施时点；P0 共享库核验、全包 T01–T54、合并和部署未因此通过。

### 本次对应手册与文件

开发手册 §5、数据库合同 §3.1/§5.1/§6、HTTP 合同 §3。第一内部增量在 `comment_study_policy.rs` 及其私有 templates/tests 中实现闭集请求、三阶段固定规则和严格 Schema 生成、方法快照与阶段哈希。所有字段沿用批准手册；没有新服务/表/供应商依赖。保留已合入主线和迁移号 0103，不退回原来的 0102。

### 实施边界

- 本轮编译器是无副作用纯函数；不写 policy、不更新活动指针、不调用模型，不接入 `/policy`、`/runs` 或 Worker。现行提示词原文用固定 SHA 校验；新候选严格 Schema 尚未切到现行外发，避免只改一部分合同就破坏旧流程。
- 客户端不能传 hash/schema/origin；三阶段说明必齐、每项最多 8000 Unicode scalar；默认预算沿用 1–3000、1–20000。说明保留原字符，不以 UTF-16 单元计数，不接受 NUL。
- 完整模型身份和参数必须由后续受控数据库读取器提供；当前只验证字段范围和配置一致性，不冒充模型已启用、父方法同域或真实可调用。
- Hash 采用明确 UTF-8 键排序、整数 JSON、原数组/字符串。只改一阶段说明，只改变该阶段 hash 及 method hash；实际模型身份、输入/输出上限、timeout 参与 hash。不填不存在的 temperature。
- 历史未记录不倒填；完整旧快照按保存内容校验，不从当前说明重建。未知 builder/safety/cleaner 不允许执行。Hash 是内容校验，不是授权或签名。

### 验证与未完成

新增 17 个 Rust 单元场景：请求闭集/必填、Unicode 上下界、非法模型参数、三个批准 Schema 全量一致和 6144 字节上限、对象闭集、nullable 合同、确定性、阶段隔离、真实参数敏感性、篡改/未知版本、跨 Python/Rust canonical golden、现行提示词 SHA。P2 数据库存储/跨域父引用/CAS/不可变性和启动并发没有在本提交实现，T01–T54 不勾整体 PASS。

本地批准手册完整性检查通过，源码差异和固定 hash 校验通过；Rust 编译与新测试以 exact-head CI 后续回执为准。全仓治理基线实测 71 项既有错误；本轮 72 项，其中新增 1 项为月报正文尚未追加。不能把仅手册检查通过当作全仓通过；不为解决该检查改写批准手册或截断巨大月报。进度索引链接本记录，月报安全追加留在集成阶段，当前不具备完整治理通过结论。独立 commit-reviewer 工具不可用，已自审但不冒充独立复审，最终交 Codex。

下一步沿手册接方法创建/读取/复制及不可变约束，再完成同次切换的原子启动、选择/排重与方法 UI。当前 P2 是已开始，不是已全部交付；共享库 migration、外部模型、自动计划、合并/部署均未执行。


## P2 第二增量 · 方法持久化与只读目录（实施前边界）

2026-09-24 / Issue #295 / PR #338；基线 `40434fc0`，按 Mog“继续推进下一步”执行。只复用 policy 四个新列与三阶段编译器，增加持久化/读取及复制血缘；不新增方法表、不改变默认指针、不切换旧 `/policy` 或 `/runs`，不外发模型。

- 源码归属：`comment_study_policy/store.rs`、`read.rs` 为现有 policy 私有实现；共享既有 cursor，增加 policies 资源。
- 候选 `0106_comment_study_policy_constraints.sql` 只承担 constraints 阶段的 policy 部分：UPDATE/DELETE/TRUNCATE 不可变、新插入完整性、父方法同领域。后续 Target/Run 约束独立补齐，不修改已交付 0103；均不注册到共享升级入口。
- 新 GET/POST `/policies`、GET `/policies/{policyRef}` 复用 loopback Host/Origin 与 no-store；错误返回闭集安全码。复制必须提供全部已编辑字段，完整加载校验父记录，不借活动版本补填未知历史。
- 保存时读取并锁住实际模型配置/连接启用事实；保存不是模型调用或语义资格测试。读取已保存版本不因连接停用而抹掉历史。
- 验收：保存/复制不改父行、不切活动指针、不建 Run/调用；数据库直接修改/删除及不完整新写被拒；不存在/未知/停用/跨域分别拒绝；超过100版本连续分页；读取损坏 hash 不默默重建。T29/T34/T50/T51 的方法子集，不覆盖尚未实现的启动并发。
- 用户前端与激活 CAS 在新启动合同一起切换，避免旧 Run 使用新 policy 却仍发旧 prompt。

验证结果待本轮真实执行回执；不把已有22项P1数据库回归当新增方法测试。

实现文件已形成：三条新方法 API、policy 保存/只读模块、policy-only 0104约束和6个隔离PG/3个API测试。当前本地没有Cargo且无法解析Rust下载域名；本地文档校验与bash语法/diff检查通过，Rust/PG以本增量GitHub CI为准。全仓治理仍有72项（71项基线，加1项月报正文未追加）；不以进度索引冒充月报检查通过。


## P2 第三增量 · 共用选样规则与输入等价（实施前边界）

2026-09-24 / Issue #295 / PR #338，基线 `39f3f534`。本轮按 HTTP 合同 §4–5、数据库合同 §5.2/§6 先固化 preview/start 共用的确定性内核：闭集开始与预览命令、稳定身份集合规范化、四模式真值表、作品轮转、内容 fingerprint 与 v2 Target 输入。放置在 `comment_study_selection.rs` 及私有 input/tests；仅复用当前 cleaner、context manifest、source gate 和 canonical JSON，不加表、服务、调度器或模型依赖。

本增量不接通 `/runs`/`selection-preview`，不宣称已经具有数据库幂等或并发防重。后续事务读取器必须在 domain lock 后用单条冻结 SELECT 提供最新材料、所有在途事实、最后尝试和已保留语境；本纯函数不能证明输入来自哪个数据库快照。旧 Worker 与默认方法不切换，避免有完整方法的新 Run 仍被旧外发合同消费。整体 P2 和 T01–T54 仍未完成。

验证目标：688/100 选样交集、四模式/未知历史/在途优先级、完整十项排除计数、作品公平轮转、显式三预算与请求 hash、材料换 ID 不误判输入变化、raw 标点和真实父语境变化、未入保留语境的 OCR 不触发重研；所有用例均为合成确定性证明，不冒充数据库或真实模型证明。独立复审仍交 Codex；本工具环境不能调用仓库约定的 commit-reviewer，不以自审替代。

实现候选：新增共用命令/选择器和输入准备器、28个合成单元测试。当前纯函数只消费快照，不是冻结查询或收费入口：没有幂等回执 SQL、实际 domain lock、Run/Target 插入或 dispatch 切换，T11–T16 等不得标为通过。Python 独立生成 fingerprint 与 signed big-endian lock key golden；批准手册字节校验与diff检查已过；Rust/PG以exact-head CI回执为准。本环境无Cargo且Rust下载域名不能解析，未运行fmt/clippy。治理检查实测仍72项既有/未解决问题，未修改批准正文以凑通过。

## P2 第四增量 · 单快照读取与事务回执（实施前边界）

2026-09-24 / Issue #295 / PR #338，基线 `7a062315`，main 仍为 `a42315eb`。复用批准的 selection/policy/source 合同。本轮实现一个事务内冻结 SELECT（只读非 HOLD 游标分段消费同一快照）、实际 domain advisory lock、方法核对、准确 Run/Work/Target 写入及 requestRef 回执；不在锁内外发、不新建表或服务。游标仅活于本次短事务，每次 FETCH 128 行；不将全库原文 fetch_all 留在 Rust，至多保留预算内输入和每作品一份语境。整个扫描另设 15 秒 deadline，FETCH 不重取另一份语境。PostgreSQL DECLARE 的 insensitive snapshot 语义须以并发 PG 用例验证。

新写所需约束候选单独编号；不改 0103/0104 和共享迁移注册。原 HTTP `/runs`、页面与三阶段 P3 dispatcher 不在本增量切换。新内部启动的 v2 Run 不得进入旧 v1 batch，必须有明确隔离并测试；完整外发/成本 gates 和新 HTTP 启动随后接通。预览只读，无 request 占位或 Run；重放从既有回执返回，不重新选择。独立复审仍交 Codex，不用自审冒充仓库 commit-reviewer。

实现候选已形成：`selection/snapshot.rs`/`snapshot.sql` 只读冻结；`run/start.rs`/`write.rs` 处理预览与带重放的原子保存；现有 policy 数据库方法仅扩大 crate 内可见性，既有 batch 只增加 v1 输入隔离；`build.rs` 记录真实编译 Git revision，无法确定时拒绝新 Run，不写死基线。0105 约束只补有证据的稳定身份，保留历史 fingerprint/时间为未知；新输入/预算冻结、活动评论唯一性和回执不可变均为候选，未共享应用。HTTP 启动、默认版本切换、P3 外发尚未启用，不能把内部函数证明当页面已上线。

新增9项隔离PG候选：688条两次选择100、同请求并发重放与不同载荷冲突、不同请求无交叉占用、no_work/index_pending回执固定、父语境补齐与同文重采、写回执失败整事务回滚、冻结/不可变/旧dispatcher隔离、锁等待后读取已提交新材料、缺范围/缺guard拒绝。脚本必须显式运行本组，不以ignore算PASS。批准手册静态校验、bash语法及diff检查通过；Rust/PG/并发与新schema运行结论待exact-head CI，未完成大型库性能、浏览器、全包T01–T54或独立审查。前序文档治理问题未靠改写手册消除。


## P2 第五增量 · 默认方法 CAS 与命令 HTTP 验收

2026-09-24 / Issue #295 / PR #338，基线 `c2a90945`，main 核对为 `a42315eb`。对应开发手册 §5、HTTP 合同 §1/§3–5/§12；继续使用已有 policy、selection 与 start 事务，不重复实现选样和请求回执。前序第四增量已有 exact-head CI run `35950332361`：编译、单元及37项隔离PG通过，含9项实际启动事务证明；不再把该事务记为待开发。

- `comment_study_policy/activate.rs` 复用原 singleton 默认指针。expectedActivePolicyRef 必须出现，可明确为 NULL，禁止省略／nil／额外字段；domain lock、已记录方法和实际启用模型行锁、锁后再次验证方法，最后条件 UPDATE 或 INSERT。并发默认变化返回 control_version_conflict；不修改方法正文、既有Run或模型工作。当前合同仍只支持现有单领域，不凭空扩展领域默认表。
- `comment_study_command_api.rs` 是受测的下一版路由组合：selection-preview、runs、policies/{policyRef}/activate 和旧policy的410返回。复用现有 Host/Origin 判定，统一安全错误、requestRef、no-store/nosniff；HTTP只映射现有DTO，origin由服务器固定Manual。新Run201，重放/no_work/index_pending为200；错误不泄漏SQL、DSN、原声或供应商正文，不承诺提交阶段网络失败一定无副作用。
- 此路由本轮只编译并由真实HTTP测试调用，**未merge到生产Router**，无第二公开路径、环境开关或影子写通道。现有页面、旧/policy与/runs、默认指针和Worker在用户环境没有切换。激活函数不是已经替用户选择了默认方法；必须与页面及执行边界对齐后再启用。
- `comment_study_command_tests.rs` 新增9项无DB边界测试；`comment_study_command_postgres.rs` 新增6项真实Axum＋隔离PG测试。脚本显式运行该ignored组，不以编译成功或ignored代替运行。对应T09、T11–T17、T29/T34等的本层子集，不给全部T编号自动勾PASS。
- PG范围包括同请求并发201/200与预算绑定、不同请求不重复占用、无工作/等待索引回执固定、默认CAS保护历史方法与Run、首次默认并发及无效/停用方法拒绝、缺范围/部分schema没有部分写入；统一断言模型调用数为0。旧生产入口保持原样由路由测试单独检查。

没有新增或修改migration、注册共享升级、执行真实模型、重置历史、合并或部署。批准手册正文保持原字节；阶段记录是实施状态，不改写原字段和目标。局部diff与shell语法检查可在当前环境完成，Rust/PG真实结论以本次exact-head CI回执为准；fmt/clippy、全仓治理（前序月报正文尚未追加）及浏览器不冒充已通过。最终CI回执写PR描述，避免为了记结果再次改变已验证代码head。

下一执行点：受控弹窗的方法查看／复制／默认选择与显式三预算、持久requestRef；旧入口退役和新路由挂载须与执行路径安全边界一同切换。P2整体仍未完成，P3三阶段真实请求快照／共享额度／恢复仍待做，自动日计划不在本轮启用。

## P2 第六增量 · 最新 main 对齐与正式命令入口（2026-09-27）

基线为 PR #338 分支 `07a46a531aa9aa309dbcd2f71de0b713dbe54397` 与 `origin/main@2587fba486434acf2c94a8e7f64ce245b87a38f6`。按最新主线合并统一 Domain/Material usage 合同：main 的 0103/0104 保持原号，本分支 Comment Study 迁移排到 0105/0106/0107；六个共享文件冲突已按两侧当前职责合并。P2 查询/写入改用 append-only domain usage，活动方法指针以 `(domain_ref, policy_ref)` 做 CAS，preview/start 的作品角色随请求与 Run 快照冻结。

命令 API 已纳入实际 Axum router：`/selection-preview`、`POST /runs`、方法激活及旧保存入口退役。页面可创建/复制方法、查看版本、设置激活方法、填写三阶段提示与预算、预览并以持久 requestRef 启动；不在 UI/PG proof 中调用模型。迁移未注册到共享升级器，没有修改共享数据库或发布本机 runtime。

验证：`./scripts/test-comment-study-productization-postgres.sh` 在脚本创建并清理的 disposable PostgreSQL 中全通过：Intelligence 8 个测试目标共 37 项（包括 9 项启动事务用例）以及真实 Axum 路由 6 项，总计 43 项。证明覆盖当前迁移链、领域 usage、方法保存/复制/约束、按域 CAS、启动幂等/并发、terminal receipts 和 HTTP 边界；不覆盖完整 T01–T54、真实模型质量、生产规模性能或恢复。此前同版本 `cargo check -p linggan-intelligence -p linggan-api`、两个单元套件与 `node --check` 通过；最终 exact-head CI 尚待推送后回执。

P2 下一关为 exact-head CI 与 Mog 对真实页面/数字/动作的验收；执行阶段的真实模型调用账本、Schema 校验/部分接纳、取消/重启还没有项目级闭环。之后按 P3 质量闸门执行：目前唯一前置阻塞是独立人工 Gold Set 标注，不能由 Agent 自标。P4–P8 按上方矩阵续办，环境级迁移/生产调用/部署/恢复各自保留授权与证明边界。

## P2 第七增量 · 新 Run 接入 semantic worker（2026-09-27）

本增量从最新整合树 `main@2587fba486434acf2c94a8e7f64ce245b87a38f6` 开始，PR #338 远端前一 head 为 `d10b94563c63934d737c2195a9ffc7a67ccf4e0c`。核对开发手册 §5–6 与 HTTP 合同 §7–8 后，修复新 `run-selection.v2` Run 已写成 queued、却被旧 batch selector 限定为 v1 的断路：selector 和 batch preparer 现在只接收 dispatch 已启用且无停止理由的 v2 Run，继续兼容历史 v1 Run。

每个新 semantic invocation 在事务中验证 Run 冻结的 method hash，读取不可变方法中的 instruction/Schema，序列化准确 provider payload，将 request manifest/hash 与通用 invocation hash 对齐保存。获得 live batch lease 后、紧邻 adapter 调用前，CAS 写入 `dispatch_started_at` 并标记通用 invocation 的 callStarted。Run 行锁串行累计 request 的保守输入估算与模型 max output reservation；预算不足时不创建 invocation，停止未外发 Run，并给取消的 Target 写 terminal reason/timestamp；仍有其他调用时先将该 batch 返回 prepared。合成结果证明合法 no_signal 可接纳、缺失的兄弟 Target 独立重试。新增 0108 migration 为 model request snapshot 提供 UPDATE/DELETE/TRUNCATE 不可变保护，避免改写已发布的 0107 checksum；启动 schema guard 要求对应 triggers。

验证：`./scripts/test-comment-study-productization-postgres.sh` 的 8 个 Intelligence PostgreSQL 目标共 38/38、Axum+PostgreSQL 6/6；`cargo test -p linggan-intelligence --lib --locked` 123/123；`cargo test -p linggan-api --bin linggan-api --locked` 287 passed、35 ignored。以上为本机 disposable/synthetic 验证，不发真实模型请求。此前精确 head `d10b945` 的 Actions run [36301233256](https://github.com/Himog0921/linggan-intelligence/actions/runs/36301233256) 中 synthetic-proof 成功，integration-source-export 因条件跳过；它不覆盖本增量。当前增量尚需推送后的 exact-head CI。

### 提交前复审修正与增量复验（2026-09-27）

第一轮独立审查发现输入限额没有计入完整 v2 方法指令/Schema/封装、已 checkpoint 的实测用量仍被预算累计当作预留、调用前估值误计为 charged，以及接受/预算停止间的 Run/Target 锁序反转。后续复审确认这些修正闭合，最终复审无 blocker；实现统一用冻结方法和完整 semantic request 做预打包，批次过大时顺序缩小，单目标仍超限则无 invocation 地终止并记 `input_limit_exceeded`；已知完整 usage 用于后续累计，未知仍保留 reservation，尚未开始外发的 invocation 计费为 0；准备、claim、dispatch、accept、reject 和 lease recovery 统一按 Run → Batch → Target 锁序。新增本地 PiAdapter synthetic-child 用例从真实 worker 函数传递 v2 冻结方法和输入，不访问 provider。

复验：隔离脚本 8 个 Intelligence PostgreSQL 目标共 45/45，Axum/PostgreSQL 6/6；Intelligence 单元 123/123；API 单元 287 passed、35 ignored；定点 rustfmt 与 `git diff --check` 通过。隔离脚本会清理临时数据库/容器/卷。独立 reviewer 已无 blocker，exact-head run 36309022127 成功；工作流没有运行浏览器 UI 自动化。证据不包括完整 T01–T54、人工浏览器验收、真实 provider、共享 migration 或部署；仍待用户逐页核验真实页面回执/数字/动作。

后续独立复审补出两项边界：开始 provider dispatch 后、usage checkpoint 前崩溃时，过期恢复必须继续按 reservation 计入预算；以及已发布 0107 不得因增加 snapshot trigger 而改变 checksum。候选现以未注册的 0108 独立承载 request snapshot trigger，并分别为“已 dispatch、usage 未知”与“尚未 dispatch”的过期调用增加保守结算/释放 reservation 回归。0107 与父提交 SHA-256 同为 `da0fc26c23949cb52137ecfbbd9f82698c10e244b52656e34d75f91365ec1dd7`。最新隔离复跑 Intelligence PostgreSQL 45/45、Axum/PG 6/6；单元 123/123 与 287 passed/35 ignored；治理、UI handbook、定点 rustfmt、shell 语法和 diff 检查均通过。独立最终复审未发现 blocker。PR head `4b177bdeb484581b95c892b8e6a331f1640d02e9` 的 exact-head run [36309022127](https://github.com/Himog0921/linggan-intelligence/actions/runs/36309022127) 成功；push-only `integration-source-export` 按条件跳过，工作流的前端行为脚本当前不存在，因此未执行浏览器 UI 自动化；Mog 页面验收仍待完成。

未完成仍按原手册分层：P2 需 exact-head CI 与真实页面逐动作/逐数字验收；P3 需补 pause/resume/stop control CAS、公平调度、失败/部分接纳与运行态 drain 的完整证明。P3 Recall@K 需独立人工 Gold Set；Agent 不自行标注。后续顺序为 P4 有效结果 head 与限制传播 → P5 六视角与 durable events → P6 每日增量/额度/退避 → P7 真实 API、空错旧态、键盘/1440/1024/390/200% zoom 与 Mog 验收 → P8 隔离恢复、有限授权发布与回退。P0 用户环境历史、在途任务与数据保护/恢复证明继续并列跟踪。未修改 T01–T54 台账状态，未共享迁移、调用 provider、部署或合并。

## P2 第八增量 · 停止、迟到回执与显式重试（2026-09-28）

对照 GREENFIELD 手册第 10 章 P2 顺序及 AC062/AC072，并沿用 HTTP 合同取消/重启边界。这里实现并证明一个候选切片，不把页面代码或合成数据库证明等同于用户验收。

- `POST /api/local/comment-study/runs/{id}/cancel` 严格接收 `{domain_ref}`，拒绝未知字段、错误 UUID、nil 领域和 query 参数；领域不匹配或非 v2 Run 不暴露资源。事务先锁 Run，与 model dispatch fence 共用锁序，control_version 只在首次状态转换递增，重复请求幂等。
- 停止阻断后续 dispatch；已预留但尚未越过 fence 的调用按零费用失败收口、目标取消。已越过 fence 的调用保持在途与结算机会；合法且租约/期限仍有效的响应可接纳，缺失的兄弟目标记一条失败尝试并按 `user_stopped` 取消，不会重新入队。已有 Signals 保留。重试通过新 Run 的 `retry_failed` 明确选择，不重置旧 Run 或重放已接纳结果。
- Run 列表显示服务端派发状态；仅对仍活动的 v2 Run 提供 Danger 停止按钮。采用页面规格要求的自定义确认对话框，说明在途费用与结果边界；提交后核验回执身份并重新读取页面，不乐观改写状态。兼容 main 0104 的只读 Run 查询使用 `to_jsonb(run)` 读取候选字段，不直接引用尚未共享的列。
- 代码影响：`comment_study_run/start.rs`、`comment_study_batch_acceptance.rs`、`comment_study_read/runs.sql` 与投影、local command API/测试和 comment-study HTML/JS/CSS。无新 migration、无共享库写入、无真实 provider、无部署或 merge。
- 验证：`./scripts/test-comment-study-productization-postgres.sh` disposable PostgreSQL：Intelligence 8 组 **56/56**、Axum+PostgreSQL **8/8**；单元：Intelligence **123/123**、API **290 passed / 37 ignored**；取消 UI 的静态回执断言 **1/1**，`cargo check -p linggan-intelligence -p linggan-api --locked`、node syntax、定点 rustfmt、UI handbook 与 diff 检查通过。全仓 `cargo fmt --all -- --check` 仍因未改文件的既有漂移失败；项目治理检查待补本月记录后重跑。首轮隔离运行暴露新增 SQL 的字符串续行带入反斜杠，六个既有 P3/usage 用例失败；修正续行后从头重跑，全套通过。隔离容器与卷由脚本清理。
- **未验证**：真实浏览器交互、部署后的 API、Mog 页面/业务验收；此提交不改变 T01–T54 状态。此次只证明隔离合成路径，不证明 shared migration、线上计费或 provider 行为。精确 head CI 在推送后记录。

下一关仍按手册顺序：先由 exact-head CI 核对本增量；P2 页面逐动作/数字与 Mog 验收仍需在人机界面完成。之后接 P3 剩余 pause/resume/stop 的 CAS 控制、公平调度、失败/部分接纳全矩阵与 drain；Recall@K 必须等待独立 Gold Set。再进入 P4→P5→P6→P7→P8，P0 历史/在途与备份恢复保护并行跟踪。
