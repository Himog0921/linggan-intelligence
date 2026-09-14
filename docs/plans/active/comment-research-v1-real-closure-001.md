# COMMENT-RESEARCH-V1-REAL-CLOSURE-001 · 开发期派生重置与首轮发布

> 状态: 活跃计划
> 最后核对: 2026-09-14
> 适用范围: Issue #254；Comment Research V1 的开发期派生数据重置、失败隔离与恢复、首轮真实发布闭环
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

## 4. 实施步骤与可证伪验收

1. **迁移和重置接口**：新增终态 schema migration，并以独立、可重复的本地 reset 命令按外键顺序仅删除研究派生表数据；隔离 PostgreSQL 验证原始语料、作者归属、模型配置、embedding profile 和 ModelInvocation 均保持不变。
2. **研究指纹与选择**：替换“存在任意历史 RunItem 即排除”的谓词；用 Fingerprint + 有效结论 + 累积尝试次数决定候选。覆盖取消、临时失败、合同失败、成功、无信号和输入变更六种场景。
3. **失败隔离**：删除整批取消传播；调整 Run 完成和暂停语义，让无关 pending Item 不被改为终态取消。
4. **自动推进**：用户在页面确认“准备本轮研究”后，服务端自动选择符合当前输入的范围并由 Comment Worker 推进 Run；用户不勾选或凑批。连续自动排程继续关闭，直到首轮真实发布和质量判断完成。
5. **首次真实闭环**：重置后先执行有界首批，检查结构合同、调用账本、Atom、embedding、membership 与 ResultRevision。只有 published ResultRevision 后才扩展到后续自动批次。
6. **用户表面**：运行记录显示可恢复、合同需要更新和暂停原因；概览/问题/变化显示已发布版本。不得以空数字或历史失败伪装新结论。

## 5. 非目标

不复用旧 Problem/Atom；不做自动 Topic merge/split；不创建新的 Agent、Skill Center、队列页面或人工审核流；不扩大采集；不发送原始评论到 Git、普通日志或未授权第三方；不把一次结果变成趋势或市场结论。

## 6. 完成分层

只有以下层分别有证据时才能声称完成：代码与迁移、隔离 PostgreSQL、真实开发库派生重置、真实首批发布、localhost runtime、真实结果页面、Mog 业务验收。模型质量与研究业务判断不由编译或单批成功替代。
