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

### Issue tracker

本项目使用私有仓库 `Himog0921/linggan-intelligence` 的 GitHub Issues 追踪任务、问题、阻塞和 Agent 工作单；Issue 不是产品、架构或事实的第二权威来源。具体规则见 `docs/agents/issue-tracker.md`。

任何仓库写入必须先有已分配 Issue 和 Claim 评论，并在 Issue 专属 branch/worktree 中通过 draft PR 交付；编码 Agent 禁止直接在共享 root checkout 或 `main` 工作。共享文件由指定 integration owner 统一，最终结论需要非实现者 independent review。

### Triage labels

任务分流使用 `needs-triage`、`needs-info`、`ready-for-agent`、`ready-for-human`、`wontfix` 五个角色标签；标签只表达下一步责任，不替代事项编号、正式授权、文档状态或验收结果。具体映射见 `docs/agents/triage-labels.md`。

### Domain docs

本项目采用单一领域上下文，但继续使用现有的 `docs/context/domain-language.md`、`docs/product/domain-invariants.md` 和 `docs/decisions/`，不复制根目录 `CONTEXT.md` 或 `docs/adr/`。Agent 的读取和冲突处理规则见 `docs/agents/domain.md`。

### SCOPE-001 execution

任何 Agent 在修改 SCOPE-001 的 fixture、migration、Rust、PostgreSQL、API、worker、CLI、测试或完成声明前，必须阅读 `docs/agents/scope-001-execution-contract.md` 和当前 SCOPE。当前切片采用 Closed World：未列能力不获授权，独立状态不压成总 `completed`，unknown 不变成默认值，低层不越权生成高层 Claim，任何通过声明必须携带证明与未证明范围。

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
