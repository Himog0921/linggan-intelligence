# COMMENT-RESEARCH-V1-REAL-CLOSURE-001 · 开发期派生重置与首轮发布

> 状态: 活跃计划
> 最后核对: 2026-09-14
> 适用范围: Issue #254；Comment Research V1 的开发期派生数据重置、失败隔离与恢复、语义上下文输入与首轮真实发布闭环
> 事实来源: Mog 2026-09-14 当前授权、COMMENT-RESEARCH-RESET-001、当前 main/localhost Run 读取、真实数据库副作用
> 冲突时以谁为准: 用户最新决定、AGENTS.md、真实代码/数据库/运行证据

## 1. 用户结果

评论进入当前可读语料后，系统自动从普通用户原声中选择可研究项。用户不挑选、凑批或修复历史批次；系统按当前研究指纹复用有效结论、恢复临时失败、隔离单项合同失败，并在一轮完整处理后发布可回溯的研究版本。

页面保持五个用户视图：概览、用户原声、用户问题、变化观察、运行记录。原声独立可读；其余三个研究视图只读取完整发布的 ResultRevision。

## 2. 开发期重置边界

### 必须清空

- ResearchDerivation 及其 current/readable 研究投影；
- ResearchRun、RunItem、Atom、AtomEmbedding、EmbeddingSpace；
- Problem、Definition、Membership、ProblemResolution；
- ResultRevision、ProblemWindowStat、ChangeObservation；
- 仅由以上对象派生的索引/队列状态。

### 必须保留

- RawComment、已接纳 CapturePackage、作品与父评论事实；
- 内容作者归属、评论使用限制和当前 source/readable 投影；
- 模型连接、连接版本、模型条目、默认模型配置、秘密引用；
- WeMM embedding profile、资格与本机运行配置；
- 当前已保存的 ResearchPolicy revision/active 指针。它是下一轮研究的配置，不是历史研究结果；
- 通用 ModelInvocation 账本。历史调用仍是事实，但不再作为新研究资格或新结果输入。

重置不通过 SQL 回写“成功”或手工挽救历史 Run；它只删除可再生成的评论研究派生对象。它是显式、可重复的本地开发期运行操作，不修改已入账 migration。完成后，系统从保留的当前语料和策略重新派生。

## 3. 资格与恢复合同

一个评论是否进入新 Run，由当前 Research Fingerprint 决定。Fingerprint 包含当前 derivation 输入、研究合同、模型配置版本及必要上下文；它不是用户可见对象。

| 当前 Fingerprint 下的最新有效结论 | 新 Run 行为 |
|---|---|
| `succeeded` / `no_signal` | 复用，不再调用模型 |
| `cancelled` | 可恢复 |
| provider timeout / connection / worker interruption | 有界自动恢复；尝试次数跨 Run 累计 |
| JSON/结构/证据合同失败 | 当前 fingerprint 不重试；模型或合同改变后可研究 |
| source 不可读、身份不合格、明确限制 | 不进入 |

单个模型调用或输入包失败只改变对应 Item。错误率达到受控暂停阈值时，不再外发后续包，但未处理 Item 保持可恢复，不写为 `cancelled` 或伪成功。

### 3.1 语义上下文与审计上下文（COMMENT-RESEARCH-CONTEXT-INPUT-001）

回复评论不能只向模型提供当前短句和父评论的引用/hash。新的 `comment-research.derivation.v2` 将下列两类内容在同一不可变 Derivation 中明确分责：

- **语义上下文**：当前 `researchText`，以及父评论可读时经同一清洗规则得到的 `parentResearchText`。它们是模型可理解的研究材料；父评论最多一层，绝不递归展开回复链。
- **审计上下文**：作品、父评论的 stable ref、父文本 hash、请求/可用性状态与合同版本。它用于来源追溯、输入变化检测和冻结复现，不能用 hash 代替模型语义输入。

若一条回复声明有父评论、但父评论在当前可读来源中不存在或清洗后没有可研究正文，新的 Item 不发送任何模型调用，而以既有 `incompatible` 终态和精确代码 `context_insufficient_parent_unavailable` 保留“语境不足”。它既不是 `no_signal`，也不把评论不存在或没有讨论伪装成事实。之后父评论变为可读时，Derivation 的语义/审计上下文与输入 fingerprint 改变，系统只为新的输入创建新的候选；历史 Run 和 ResultRevision 绝不回写。

本包不实现作品级全文、OCR、ASR 或任意摘要的模型上下文。只有将来存在稳定、用途合格且版本化的 `postResearchText` 合同时，作品级上下文才可以作为独立包进入输入合同；不能把当前材料投影或整篇正文静默外发。

### 3.2 页面状态文案变更清单（UI-CR-CONTEXT-001）

| 项 | 本包约定 |
|---|---|
| Issue / worktree | #254；`codex/comment-research-context-input-001` |
| 分类 | 状态语义；不改变页面结构、权限或行动 |
| 用户可见结果 | 运行记录/原声行将 `context_insufficient_parent_unavailable` 显示为“需要父评论语境、未发送给模型、并非没有研究信号”；旧策略遇到 v2 合同时明确提示重新保存策略 |
| 非目标 | 不新增页面、字段、按钮、筛选、模型调用、原文/模型原始输出展示或设计 token |
| 表面与依赖 | `/corpus/comments` 的既有运行记录、原声状态和研究设置反馈；依赖 RunItem `incompatible + failure_code` 与 API `409`，不以本地状态替代回执 |
| 状态边界 | `no_signal` 仍只表示模型在足够输入下确认无信号；`incompatible/context_insufficient_parent_unavailable` 表示输入语境不足；`model_failed` 仍表示模型/合同执行失败，三者不可互换 |

读取回执：`AGENTS.md`、`docs/README.md`、`docs/current-state.md`、`docs/agents/ui-execution-contract.md`、`docs/design/README.md`、`docs/design/design-governance.md`、`docs/design/lids/README.md`、`docs/design/lids/language-policy.md`、`docs/design/lids/data-boundaries.md`、`docs/design/lids/decisions.md`、`docs/design/pages/comment-research-v1-page.md` 与现有 local API / JS 已核对。页面保持既有 L1 Corpus shell 与既有表格 Pattern；没有新增 Token、Primitive、CMP、Scene 或 Motion。LANG-05 的中文主表达和数据边界的“未知不等于失败”适用；本包不宣称现有运行页已完成 LIDS v7 迁移。

| 验收层 | 方法 | 本包应证明 | 未证明边界 |
|---|---|---|---|
| 任务可用 | JS syntax + local API 测试 | 旧策略会停止并提示重存；受控失败码有中文说明 | 真实 3000 未切换前不证明用户现场流程 |
| 状态诚实 | 隔离 PostgreSQL | 缺父语境不建模型调用、终态不是 `no_signal` | 不以 fixture 代替真实评论质量判断 |
| 视觉一致 | 复用既有状态行/表格和中文文案 | 不新增视觉值、组件或布局 | 未进行浏览器/屏幕验收前不称视觉验收完成 |
| 真实后果 | 禁止实际外发 | 无真实模型调用、共享库或 runtime 写入 | 部署、重新保存策略和真实 Run 需另行授权 |

## 4. 实施步骤与可证伪验收

1. **迁移和重置接口**：新增终态 schema migration，并以独立、可重复的本地 reset 命令按外键顺序仅删除研究派生表数据；隔离 PostgreSQL 验证原始语料、作者归属、模型配置、embedding profile 和 ModelInvocation 均保持不变。
2. **研究指纹与选择**：替换“存在任意历史 RunItem 即排除”的谓词；用 Fingerprint + 有效结论 + 累积尝试次数决定候选。覆盖取消、临时失败、合同失败、成功、无信号、父语境不足和输入变更七种场景。
3. **失败隔离**：删除整批取消传播；调整 Run 完成和暂停语义，让无关 pending Item 不被改为终态取消。
4. **自动推进**：用户在页面确认“准备本轮研究”后，服务端自动选择符合当前输入的范围并由 Comment Worker 推进 Run；用户不勾选或凑批。连续自动排程继续关闭，直到首轮真实发布和质量判断完成。
5. **首次真实闭环**：重置后先执行有界首批，检查结构合同、调用账本、Atom、embedding、membership 与 ResultRevision。只有 published ResultRevision 后才扩展到后续自动批次。
6. **用户表面**：运行记录显示可恢复、合同需要更新、父语境不足和暂停原因；概览/问题/变化显示已发布版本。不得以空数字或历史失败伪装新结论。

## 5. 非目标

不复用旧 Problem/Atom；不做自动 Topic merge/split；不创建新的 Agent、Skill Center、队列页面或人工审核流；不扩大采集；不发送原始评论到 Git、普通日志或未授权第三方；不持久化完整 prompt、模型回答或思考；不把一次结果变成趋势或市场结论。

## 6. 完成分层

只有以下层分别有证据时才能声称完成：代码与迁移、隔离 PostgreSQL、真实开发库派生重置、真实首批发布、localhost runtime、真实结果页面、Mog 业务验收。模型质量与研究业务判断不由编译或单批成功替代。
