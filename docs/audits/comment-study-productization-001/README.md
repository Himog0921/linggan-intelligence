# 评论研究产品化 · 来源和验收账本

> 状态: 实施记录；不作为已通过验证的声明
> 最后核对: 2026-09-23
> 适用范围: COMMENT-STUDY-PRODUCTIZATION-001
> 事实来源: 用户提供的手册v1.0与本分支的实际提交
> 冲突时以谁为准: 实际执行证据与对应版本合同

- `source-lock.json`：成文时固定源码和LIDS附件的来源回执，不是当前全量部署状态。
- `document-check.original.json`：2026-09-22文档交付时的静态检查原回执。保留其“未写GitHub”等历史描述，不以它报告本次状态。
- `acceptance-status.json`：T01–T54实际业务验收台账；每条由对应层级的执行证据才能改为PASS，文档或Schema检查不能替代Rust/数据库/浏览器证明。
- [P0来源与现场边界](p0-execution.md)：源码部分已核对，本机数据库与在途调用未核验。
- [P1前序回看](p1-read-counts.md)：批次计数、schema-phase候选、清洗缓存、评论目录/详情/历史和各次验证边界。
- [P1作品目录与共享标题](p1-work-catalog.md)：`/works` 后端、Evidence标题复用、迁移编号对齐，以及隔离PG/合成浏览器回执。

## 当前剩余工作：沿用原 P0–P5，不新增任务层

原手册的P1–P5五个主交付阶段仍未结项；其中P1已有多项代码候选，P2–P4尚未形成完整能力，P5尚未实际验收。另有P0本机现场证明未完成。阶段数不等于工作量百分比，commit/文件/测试用例的数量不能转换成已完成比例。

| 手册阶段 | 已有事实 | 剩余事项 | 结项状态 |
|---|---|---|---|
| P0 基线与保护 | 手册入库、源码基线和主线差异已核对 | 用户环境的schema/历史保护/在途请求只读盘点，迁移前drain证明 | 源码部分完成；现场NOT_RUN |
| P1 材料可查 | 清洗缓存、评论目录/详情/历史、批次计数、作品分页、用户评论/作品选择UI；隔离PG与合成浏览器子集通过 | 完整作品语境、原冻结来源资格统一、剩余错误/竞争/性能边界及T01–T54完整验收 | 核心路径有证据；整阶段未验收 |
| P2 启动与方法 | 已有旧Run/policy可复用；schema仅候选 | 不可变方法版本与可视化、显式预算、preview/start共用选择、稳定身份/fingerprint、幂等及并发排重 | 主要功能待实现 |
| P3 执行与恢复 | 现有batch/ledger/Worker可复用 | 三阶段真实请求快照/共享预算/dispatch fence、暂停恢复停止、缓存接纳顺序、超时及公平调度 | 主要功能待实现 |
| P4 有效知识与四Tab | 现有Signal/Problem/membership及概览上半区 | 有效head/限制传播/重研不增证、问题证据页、概览下半区、四Tab和统一前端初始化 | 主要功能待实现 |
| P5 集成与发布包 | 最新main已本地整合；exact-head CI、编译、单元、22个隔离PG proof、合成数据浏览器子集已通过 | 并发/历史保留全量证明、迁移/性能、T01–T54剩余项、文档治理与发布前核验 | 部分L1–L3通过；整阶段未完成 |

数据库的constraints/views、历史身份回填和所有新写入约束是P1–P4配套工作，不额外算成一个独立系统；完整同版验收后才能注册/运行迁移。原型全文入库、月报正文追加和大型文件格式检查同样不能漏在集成之外。

下一阶段自动化计划（计划表/UI、到期扫描、时间槽防重、每日额度、启用运行）不计入本轮这五个阶段；本轮先做可供它复用的统一启动、预算和恢复。自动化只有设计稿，未启用。

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
- 候选 `0104_comment_study_policy_constraints.sql` 只承担 constraints 阶段的 policy 部分：UPDATE/DELETE/TRUNCATE 不可变、新插入完整性、父方法同领域。后续 Target/Run 约束独立补齐，不修改已交付 0103；均不注册到共享升级入口。
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
