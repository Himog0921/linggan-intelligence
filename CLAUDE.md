# Linggan Intelligence Agent Contract

## 最高约束：文件治理与变更留痕

本仓库按“图书馆”而不是“临时工作台”管理。任何 Agent 在创建、生成、移动、重命名、归档或删除文件前，必须先阅读 `docs/README.md`、`docs/current-state.md` 和 `docs/governance/`。

- 不得把报告、计划、截图、导出物、生成代码、构建产物或临时文件随意放在根目录或任意业务目录。
- 新文件必须先确定生命周期、归属目录、稳定名称、责任来源和索引入口；无法分类时停止入库，记录为 `DECISION_REQUIRED`。
- 所有重要 Markdown 必须带状态头，并登记到 `docs/README.md`；未被索引的重要文档视为未完成交付。
- 生成型文件必须登记在 `docs/governance/generated-artifacts-registry.md`，说明生成来源、固定位置、是否入 Git、再生方式和清理规则。未登记的生成型文件不得提交。
- `references/` 是校验和保护的历史证据区。不得为了修正文档冲突而改写其中原件。
- 新文档与旧文档、代码、合同或测试冲突时，不得并存两个“最高版本”。必须按 `docs/governance/agent-collaboration.md` 完成冲突裁定、旧文档降级或替代标记，并写入当月变更记录。
- 修改代码、合同、配置、数据库描述或权威文档时，必须同步更新相关说明和 `docs/progress/YYYY-MM.md`，记录事项编号、原因、影响文件、冲突处理和验证结果。不要用无意义代码注释代替变更记录。
- 文件删除、覆盖和归档必须可解释、可追溯；默认保留并降级旧材料，不得静默覆盖或随意清理。
- 提交前运行 `./scripts/check-project-governance.sh`。检查失败时，文件治理任务不算完成。

以上规则高于 `references/`、历史 handoff、一次性报告及任务对话中的临时建议。具体分类和协作流程以 `docs/governance/` 为准。

## Agent skills

### 插件与工位的权威改动位置

为避免把同一能力写进历史副本、插件和 Linggan 三处，以下边界是当前仓库的硬约束：

- **工位、插件安装身份、认领窗口、授权、额度与第 5 问准入**只能修改 Linggan 的 Rust/domain、数据库、local API 与 `/collection` 页面；浏览器插件只能上报一次安装和执行受控任务，不能成为工位或授权的权威来源。
- **浏览器插件的唯一权威源码**是当前受控 branch/worktree 中的 `plugins/linggan-intelligence-browser/`。发布包必须从这一目录的当前 `main` 或已派定的专属 worktree 生成，并与 `releases/release-manifest.json` 校验一致。
- 名为 `plugin-retrofit-*`、旧版本插件副本、迁移快照和历史 worktree 只可用于只读对照、审计或受控迁移；不得在其中开始新功能、生成发布包或将其结果宣称为当前插件版本。
- 新插件事项仍必须使用已派定的 Issue、专属 branch/worktree、独立复审与合并后核验；不得直接在共享 `main` 或历史副本写入。

### Issue tracker

本项目使用私有仓库 `Himog0921/linggan-intelligence` 的 GitHub Issues 追踪任务、问题、阻塞和 Agent 工作单；Issue 不是产品、架构或事实的第二权威来源。具体规则见 `docs/agents/issue-tracker.md`。

仓库写入按风险分级，不再把所有改动强制塞进同一套流程：

- **受保护交付**必须有 Mog 明确派定的 Issue/交付包，并使用 Claim、专属 branch/worktree 和 PR。它包括代码、migration/schema、运行合同、插件与发布包、部署/生产、真实外部动作、敏感数据/权限、不可逆处置，以及存在并行冲突或较大权威面改写的事项。编码 Agent 仍禁止直接在共享 `main` 上实施这类改动。
- **低风险直接维护**在 Mog 当前对话已明确授权、工作区干净且无并行冲突时，可以不预建 Issue、Claim、独立 worktree 或 PR。它只包括文档勘误、索引/链接、已确认决定的状态同步、进度记录、对已被取代卡片的说明与关闭，以及不改变产品/领域/运行/权限/数据语义的小型治理整理。必须检查 diff、运行适用治理检查、留下可追溯记录，并分别报告修改、验证、提交、推送和部署；未经明确授权不得借此扩展到实现、push、deploy 或真实外部动作。

#### worktree 只能建在一个地方

受保护交付要求专属 worktree，但此前从未规定它建在哪。结果是 2026-09-03 清理时发现 31 个
worktree 散在六个位置（各 Agent 工具的默认值：`~/.codex/worktrees/`、`~/.proma/agent-workspaces/`、
`~/proma/.worktrees/`、`~/proma/worktrees/`，以及直接建的同级目录），共 33GB，其中多个分支
只存在于本地。散落本身不是洁癖问题：它让「哪些工作还没推」这个问题没人答得上来。

**唯一允许的位置**：

```
<仓库>/.worktrees/<branch-slug>/
```

- 不使用任何 Agent 工具的默认 worktree 路径。工具若有默认值，在建之前显式覆盖为上述路径。
- 该目录已在 `.gitignore` 中，不进版本库，也不进 Cargo workspace。
- **PR 合并后立即 `git worktree remove`**，不留到"以后再清"。删除前确认该分支已推送到 origin。
- 由 `scripts/check-project-governance.sh` 自动执行：出现在约定路径之外的 worktree 会使检查失败。

例外只有一个：本机常驻服务的运行目录
`~/Library/Application Support/Linggan Intelligence/runtime-main`。它不是交付用的 worktree，
生命周期与部署绑定，见 [`docs/runbooks/local-runtime-deployment.md`](docs/runbooks/local-runtime-deployment.md)。

Mog 决定受保护交付的执行、并行、审查、集成与 exact-head 合并授权；当前交付包可以明确授权连续处理一组低风险治理项，不需要为每个状态修正重复建卡。具体分级与升级条件见 `docs/agents/issue-tracker.md` 和 [`docs/governance/agent-collaboration.md`](docs/governance/agent-collaboration.md)。

### 协作权力边界：Mog 指挥协作，Harness 在任务内执行

Mog 负责定义**想得到的用户结果**、**明确不能接受的结果**、协作派单与并行边界，以及会改变产品语义、真实世界权限、敏感数据、资源消耗或不可逆后果的决定。Harness 只在 Mog 已派定的交付包或 Work Package 内，选择满足既有约束的最小实现和验证方法。

- Mog 决定谁派单、哪些工作同时进行、是否拆分、何时暂停/恢复、是否需要独立审查，以及何时对哪个 PR head 授权合并。除非 Mog 在当前事项中明确委托，Agent 不得替代这些决定，也不得自行创建/领取衍生任务、启动额外 Agent 或扩大并发。
- 用户提出的“不要做什么”是硬约束；用户也可以直接指定协作方式、执行者、并发上限和合并时机。Agent 必须如实执行，不能以“更优的 Harness 编排”为由覆盖。
- 任何需要用户确认的问题，除了产品语义、风险接受度、权限、真实世界动作、敏感数据和不可逆后果外，也包括跨任务派单、并发、审查和合并决定。技术排查与当前已派任务内部的最小实现仍由执行 Agent 负责，不得转嫁给 Mog。
- 当前交付包才是唯一的**用户结果授权**；Issue、任务卡、文件边界、PR 和测试只是交付包内部的执行与审计单位，不能替代用户结果。Mog 可以要求逐卡验收，也可以授权连续推进；Agent 不得自行假定任一模式。
- Agent 不得用“严格治理”掩盖不完整盘点：凡被派定的可见结果，必须识别其涉及的表面、状态、共享依赖和验收路径；发现遗漏时，报告给 Mog 由其决定当前任务扩展、另开任务或停止，不得自行重编排。

### Triage labels

任务分流使用 `needs-triage`、`needs-info`、`ready-for-agent`、`ready-for-human`、`wontfix` 五个角色标签；标签只表达下一步责任，不替代事项编号、正式授权、文档状态或验收结果。具体映射见 `docs/agents/triage-labels.md`。

### Domain docs

本项目采用单一领域上下文，但继续使用现有的 `docs/context/domain-language.md`、`docs/product/domain-invariants.md` 和 `docs/decisions/`，不复制根目录 `CONTEXT.md` 或 `docs/adr/`。Agent 的读取和冲突处理规则见 `docs/agents/domain.md`。

### SCOPE-001 execution

任何 Agent 在修改 SCOPE-001 的 fixture、migration、Rust、PostgreSQL、API、worker、CLI、测试或完成声明前，必须阅读 `docs/agents/scope-001-execution-contract.md` 和当前 SCOPE。当前切片采用 Closed World：未列能力不获授权，独立状态不压成总 `completed`，unknown 不变成默认值，低层不越权生成高层 Claim，任何通过声明必须携带证明与未证明范围。

### UI design handbook

任何 Agent 在创建、修改、审查或验收用户可见 Web UI、前端组件、页面交互、状态文案、视觉资产或 UI 自动验证前，必须先阅读 docs/agents/ui-execution-contract.md、docs/design/README.md 和 docs/design/lids/README.md，再按具体事项进入相关产品页面、设计规则与数据合同。

LIDS 是全项目唯一设计表达标准：Token → Primitive → Component → Pattern → Page，Motion / Scene / Data Truth 为横向约束。设计手册只规定已获批准的界面表达与协作流程，不替代用户确认、运行时事实、产品语义、权限边界、API/数据合同或当前 SCOPE。LIDS 的建议技术路径、原型、模拟数值和历史状态不是实现授权；没有状态为“权威当前”的明确依据时，Agent 必须停止受影响部分并标记 DECISION_REQUIRED，不得借助参考图、旧系统、截图、通用设计习惯或模型推断自行补全。

## Design System (LIDS) Constraints

手册入口 `docs/design/lids/README.md`；Token 基线 `docs/design/lids/tokens.md`，运行时镜像 `apps/api/src/local_web/lids_tokens.css`。

- No hardcoded colors, font-sizes, or spacing values — use the defined tokens only.
- No hover-lift transforms; primary buttons use the specified fill (see LIDS manual).
- Propose visual/texture changes (background patterns, accent weight, shadow thickness) as a small isolated diff and ask before applying site-wide.
- Write/refresh governance docs BEFORE implementation, not after.

第一条目前**没有自动检查兜底**：`scripts/verify-ui-design-handbook.sh` 只校验手册文档本身的完整性，不扫代码里的写死值。在补上这道检查之前，这一条靠改动者自觉与 review 守，不要因为"检查通过了"就认为没有违规。

## 目标

构建能够持续观察世界、保存可追溯来源材料与版本化观察、发现变化并形成有证据边界的可行动情报系统。

## 事实优先级

1. 真实运行结果、真实数据库副作用和可复现测试证据。
2. 新仓库实际代码、数据库 migration、跨边界合同和 producer fixture。
3. `docs/decisions/` 中标为 ACCEPTED 的决定。
4. `docs/README.md` 标记为“权威当前”的文档。
5. 活跃计划、一次性报告和历史归档。
6. `references/` 中的历史代码和讨论资料。

`references/` 永远不是可直接执行的现行需求。旧文档中的“已完成”只描述当时状态。

## 不可违反的规则

- 禁止把旧 Prisma schema 或旧 migration 整体移入新数据库。
- 禁止从旧数据推断不存在、不可用、活跃或可信等事实。
- 禁止 V1/V2 双写、字段级 fallback 和静默降级。
- 所有外部输入、事件、数据库 JSON 都必须运行时校验。
- 下游副作用失败时禁止返回完整成功；需要原子性的链路必须同事务完成。
- mock、类型断言和编译通过不能证明真实链路成功。
- Evidence 不可静默覆盖；更正以新版本追加。
- Evidence 只证明系统在特定 producer、时间和观察条件下接纳了什么来源材料，不证明材料内容真实、完整、有代表性或适合任意判断；任何关于存在、趋势、需求、机会或因果的主张都必须重新说明适用范围。
- 不得从数据库计数为零、任务结束或未采集到推断现实不存在；必须同时检查观察资格、Coverage、失败和分析版本。
- 批次未达到目标不得连坐已安全取得的原料。Package 接入资格、逐 Record 处理、Source Observation、Corpus/材料使用和 Claim 推断必须分别判断；Coverage 限制解释范围，不自动否定材料价值，也不自动授予代表性、趋势或市场外推资格。
- “目标数量”必须保留目标语义。只有执行前已冻结的已知对象集合，才允许记录具体失败或未尝试成员；最大配额、来源穷尽、时间预算、风险预算和探针目的不得通过 `target - emitted` 伪造剩余对象、完成百分比或平台总量。补采必须形成新的 Attempt、Capture Identity 与不可变 Package，不修改旧包。
- Corpus、Topic Map、市场洞察页面、报告、缓存和 CLI 都不是第二事实源；应用层不得复制一套与记录、定义和判断来源脱节的真相。
- 观察目的只说明为什么观察和授权范围，不进入 Source Object、Evidence 或 Observation 的身份；同一次合格 Observation 可以服务多个持续观察或主动研究目的，禁止按研究问题复制对象事实。
- Topic/Term/知识性关系定义当前采用的领域语言，不证明现实现象普遍存在；共现、趋势、需求、机会、因果和行动价值必须作为有证据边界的派生分析或可争论判断单独治理。人工确认授予使用资格，不制造真理。
- Topic 长期身份、定义版本、正式 Domain Definition Release、实验分析词表、分类运行、人工裁定与 Map View 必须分责。正式发布由明确决定产生；新定义未完成重分类/统计时必须标明未就绪或旧版本，禁止聚类标签自动发布、歧义词全局等价、导航移动改知识、拆分/合并自动继承旧 Claim/分类/统计。
- Corpus 只记录同一来源材料在具体用途下的选择与使用血缘，不得复制第二份原文真相。精确片段、脱敏/翻译/摘要等转换、语义标注、历史选择和当前使用资格必须分责；代表性只能相对于明确输入集合表达，不能以永久 `allowedUse`、`featured` 或“正式 Corpus”单值替代用途资格。
- Signal 是有来源、有范围、有分析版本的派生发现；注意力处理、Claim 资格和趋势判断必须分开。进入首页、人工查看、重复出现或停止关注都不能把 Signal 变成市场事实，也不能让不同运行按标题相似累加成一个全局置信度。
- Market Insight 与 Intelligence Brief 是固定当时 Claim revision、分析、材料选择、限制和正文的版本化发布物，不是第二套 Claim。后续判断修订或来源资格变化必须追加影响说明；发布物生成完成不等于判断正式、建议获批或现实行动授权，重要新断言不得藏在散文中绕过 Claim 资格。
- Content Idea 是内容工作资产，不是 Domain Topic 或强制情报流水线阶段。人工来源可以如实存在；机器建议、保存版本、采用决定、成稿、材料实际使用、Publication/Action 和 Outcome 必须分开。Agent 或生成内容的输入血缘不替新增事实、诊断、承诺或因果背书。
- 不冻结完整 Agent 回答或检索结果，不等于可以跳过安全访问审计。外部 Agent 的调用者、委托、受限数据访问、策略判断、资源消耗和回执必须可追溯；审计本身也不得借机永久复制全部敏感原文上下文。
- 分析发布物与外部内容发布行动必须分责。Market Insight/Intelligence Brief 被保存或发布只表示分析版本可引用，不证明内容已在外部平台发布；平台发布成功也不能反向证明分析正确，禁止建立把两者混成一种 `Publication` 的万能状态机。
- Decision、Action、Outcome Observation 和 Evaluation 必须分开；结果不能直接回写 Topic、Claim 或观察计划，只能通过新的判断或有记录修订校准后续认知。
- 后续领域、producer、数据、架构、页面、CLI 和实现设计必须持续通过 `docs/product/domain-invariants.md` 的对抗性案例；类型、编译和普通成功路径不能替代这些不变量证明。
- Agent 输出必须包含证据引用、反例或缺失说明、不确定性和模型/规则版本。
- 不把密码、DSN、Cookie、授权令牌、生产 dump 或个人原始数据提交到 Git。
- 平台时间不得被无条件压成精确事实。相对时间、推算时间和未知精度必须保留原始表达、来源位置、参照观察时间、解析规则版本与精度边界；producer 推算不能伪装成平台已证明的绝对发布时间。
- 对象身份错误不得合并成一个笼统的“采集冲突”。执行目标错、来源记录内部身份错、同一采集包身份/hash 冲突、媒体主体错必须在 Gate 4 的真实合同中分别审计，并产生与各自后果匹配的隔离、重试和 Coverage 语义。
- 公开可见不等于可以无限制地长期保存、逐字展示、批量导出、训练模型或提供给外部 Agent。ADHD、儿童、家庭、医疗和可反向识别材料必须遵循最小必要用途；原始 Evidence 默认只在内部受限区保存，普通工作台默认展示脱敏片段，外部 Agent 默认只能取得聚合结果、脱敏片段和受控 Evidence 引用，不得批量读取原文。
- 使用真实敏感材料做实验或让第三方模型处理前，必须有明确目的、最小样本、访问范围、输出边界和结束后的处置计划；不得把原文写入 Git、普通日志、长期提示记录、无边界缓存或训练集。具体第三方处理合同未确认前，默认不发送原文。
- 获准的删除、撤回、脱敏或访问限制必须沿血缘传播到 Corpus 选择、全文索引、embedding/feature、摘要、引用、缓存和 Agent 输出。历史 Claim/Intelligence 不静默消失，而是标记来源资格变化并重新评估；不得继续返回已失效原文，也不得抹除不含敏感内容的最小处置审计痕迹。
- “内部受限长期保存”不等于永久保存。具体保留期限、加密、角色、审计和法律适用性留待 Gate 5–7 与专业审查确认；来源在平台消失也不自动等于已收到隐私撤回请求，两种事件必须分开记录和处理。
- 第一阶段每日入口先帮助用户处理少量“值得关注并需要决定是否深入”的事项，再进入 Topic 深入研究。任何事项必须说明为什么现在出现、观察范围、证据与反例/缺失、当前资格和决定后果；没有合格事项时显示“没有形成新的合格事项”或“观察不完整”，不得为了填满首页制造变化、Signal 或 Intelligence。
- 外部 Agent 不是系统超级用户。第一阶段只能在可追溯的调用者、用途、数据、时间、输出和资源委托范围内读取聚合结果/脱敏片段/受控引用、运行不改变正式世界的分析、提交候选建议或申请有边界动作。申请不是授权，授权不是执行成功；未经人的明确授权，不得改变正式 Topic/Claim、长期观察方法、真实采集范围或现实行动。即使获准，后续仍必须经过正常采集准入、执行、Evidence 验证或行动结果链，外部 Agent 不得直连数据库、producer 或批量原文。
- 第一阶段不得把尚未接通的深度评论、私信、咨询、销售、访谈或产品行为包装成自动 Outcome 闭环。Decision、Action、结果渠道可获得性、实际取得的 Outcome Observation 和 Evaluation 必须分开；未接入、未观察、观察窗口未结束或来源不合格只能是未知，不能记为 0、失败或没有结果。旧 schema 存在指标字段不证明真实 producer、行动关联或反馈链已经成立。
- 第一阶段不得承诺“市场正在增长/下降”的正式趋势判断。系统可以在明确时间和捕获范围内陈述近期观察，也可以标记仍受账号、工位、入口、Coverage、时间精度、平台排序或分析版本影响的候选变化；只有 Gate 4–5 的真实可比性实验和统计资格通过后，才能把相应判断升级为趋势。库内累计增长、扩采、新模型或搜索结果变化均不得冒充市场趋势。

## Bootstrap Rust 与 SCOPE-001 放置护栏

下列名称不是全产品永久 bounded context、部署单元或数据库所有权；但 `SCOPE-001` 已按当前活跃计划冻结本切片允许使用的 crate、业务文件和依赖方向。实施 Agent 不得自行合并、改名或重新分配；确有必要时先修订当前 SCOPE，并重新通过 G1–G5。SCOPE 未列的名称仍不得因为未来可能需要就提前创建业务模块、表或大规模移植代码。

- `contracts`：跨边界版本化合同与运行时校验。
- `domain`：纯领域语言和不变量，不依赖数据库或 Web 框架。
- `evidence`：Evidence 接入、完整性和不可变规则。
- `observation`：从 Evidence 形成可确认观察及时间序列。
- `intelligence`：Feature、Cluster、Signal、Intelligence Event、Outcome。
- `storage-postgres`：PostgreSQL 实现，不向领域层泄露 ORM/SQL 类型。
- `apps/api`、`apps/worker`：组合入口，不承载领域规则。

## 交付纪律

每项能力必须同时交付：来源合同、正常路径、攻击性负例、真实 PostgreSQL 集成证明、文档同步。发现来源不足时冻结相关 lane，继续不依赖它的工作，不得猜造事实。

### 提交前独立审核

**一次交付（一个 PR / 一次要提交合并的改动）在收尾提交前跑一次 `commit-reviewer` 子代理，看完它的结论再提交。**

碰代码指改动了 `.rs`／`.sql`／`.sh`／`.ts`／`.mjs`／`.css`／`.html` 或任何会被执行的文件。纯文档、进度记录、改错别字不需要审——为它们派一个子代理，只是在制造噪音。

**按交付收尾审，不按每一步改动审。** 审核跑一次的开销不小（一次可能几十万 token），同一次交付里因为审核意见、自查或临时调试又改了代码，直接改完继续走原有的提交流程，不必为这一处修正再单独发起一轮审核；把多处相关的小改动攒成一次交付一起审，比拆成好几次交付各审一遍更省。只有当改动明显超出了原交付的范围（比如中途又顺手做了另一块无关功能）才值得为新增的部分再审一轮。

审核员定义在 `~/.claude/agents/commit-reviewer.md`（`model: sonnet`、`effort: high`）。它只审不改。

**审核结论不是圣旨，要自己核实。** 2026-09-10 的实例：审核指控新的 CSS 断言认可了一个必然横向滚动的布局，算出 1268px 下限——它把 `minmax(0,N)` 轨道的最大值当成了最小值求和，而那类轨道的基准尺寸是 0，真实下限只有 520px。同一轮里它另外 9 条是对的，其中一条（迁移是 append-only，只检查创建约束的文件挡不住后来的 `DROP INDEX`）避免了一个会长期假绿的检查。

所以：**每条都要判断成不成立，成立的修，不成立的驳回并写明理由**，两种处理都要留痕。整轮照单全收和整轮无视，一样是没做审核。
