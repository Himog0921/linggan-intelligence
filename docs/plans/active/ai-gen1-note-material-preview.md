# AI-GEN1 · 每篇解读材料只读预览

> 状态: 活跃计划
> 最后核对: 2026-10-20
> 适用范围: #454 / #420；独立只读 UI 增量
> 事实来源: Mog 2026-10-20 对交接10选择A；main616ac30f、note-material-input.v1 Rust合同、现有React底稿
> 冲突时以谁为准: Mog最新确认、实际代码/合同/回执；本计划不授予真实材料补齐、模型/媒体/采集或合并部署权限

## 范围与实施步骤

在已有研究底稿完整度后、旧任务预览前增加「每篇解读材料」。按需GET现有note-material接口，不修改API查询、数据库或运行时。默认标题、封面大字、正文、话题标签、图片文字五列，转写折叠为未纳入默认输入的底稿上下文。旧三任务预览仅更正文案为旧配置/当前执行停用，不替换消费者。

先核对并登记Claim/文件责任；完成解码、独立读取状态和栏目/引用；补定向测试与一个正式React→Axum只读主路径；一次收尾审查、处理发现、提交Draft与CI。真实状态看Issue/PR/CI，不逐修同步多套文档。

## PAGE-NOTE-MATERIAL-001 · G1已确认 / UI变更清单

Mog原话选择A「按最小只读增量实现」。分类：展示、交互与状态表达。读取回执：AGENTS、docs/README/current-state、governance、UI执行合同、design/README/design-governance、LIDS README/system/tokens/primitives/patterns/agent-guide/data-boundaries/language-policy，以及[既有底稿规格](../../design/changes/research-dossier-001-ui-change-manifest.md)。L1 Corpus Explorer内L2 Inspector；Token→既有原生控件/Section→本地材料组件→既有折叠阅读Pattern→研究底稿，复用Dossier样式，不新增跨页CMP/Token/壳层。

| 表面地图 | 行为与事实边界 |
|---|---|
| 证据库作品→研究底稿 | 完整度后新Section，首次展开才GET；作品/领域身份严格对应 |
| 新区准备摘要 | ready为后端准备条件，保留所有原因（包括转写），不等于完整、分析或外发许可 |
| 五栏目与引用 | 原片段原文、短引用、Unicode字符范围，来源/版本/位置/hash折叠；无验证同源映射就只显示来源详情 |
| 转写上下文 | 独立折叠，未纳入默认五列 |
| 技术身份/处理范围 | 三类hash、读取身份（未保存快照）、cutoff/层版本/处理范围；处理范围不是发送许可 |
| 旧任务预览 | 旧配置/执行停用的标签与说明；现有显式写操作边界不变 |

| 状态词典 | 表达/禁止替代 |
|---|---|
| 未展开/读取中 | 不读/隐藏缓存原文；aria-busy和只读提示 |
| available / not_provided / not_acquired | 已取得/来源未提供/尚未取得，不能互换 |
| waiting / known_empty / terminal_failure | 等待/本次来源明确空/本次处理终止，不能压成无材料 |
| restricted / not_adopted / not_connected | 受限/未纳入/本期未接入，受限无正文 |
| needs_relayer / invalid_source | 需要重新分层/来源异常，只提示不执行 |
| 未知合同/原因/范围 | 读取合同待核对，不猜资格、不显示旧正文，诊断保留 |
| 404/读取失败 | 领域或作品范围不可读/读取未成功；不得推断现实不存在或保留缓存正文 |

| 依赖地图 | 所有权与处理 |
|---|---|
| 自有FE文件 | evidence/NoteMaterial*、noteMaterial*；独立状态、解码/展示/测试 |
| 必要接缝 | DossierPanel接入，TaskInput旧链标签，既有E2E选择器；不动#452/#453评论页 |
| 隔离证明 | 现有dossier fixture/runner里增加只读浏览器入口，不改生产数据库查询或迁移 |
| 共享文档/CI | README/design索引、generated registry、progress单条；原有web CI接入一步，C0同区块以后按两包追加集成 |
| 历史#420 Claim | 保留原责任与G4；本包仅新只读区域，无活跃同路径并改证据 |

| 验收矩阵 | 证明方法 |
|---|---|
| 六栏目闭集、原因、身份、引用 | 解码与Unicode/未知状态负例，不重新计算后端ready |
| 懒读取/刷新/切作品域/失序响应 | 组件测试实际延迟响应，旧正文在pending/error/restricted均不可见 |
| 正式页面主路径 | 自有合成PG→实际Axum GET→正式React；缺失/受限/身份/引用/刷新和1440/390可读性，前后副作用账一致 |
| 旧任务标签与相关既有行为 | 既有E2E选择器定向更新、相关测试/构建/治理 |
| 收尾 | 一次独立审查，同范围修复定向复核；最终head/CI核对，不自动合并 |

原因闭集以read/readiness.rs为准，位置前缀保留，未知码不猜译。短引用范围为Unicode scalar；snapshotIdentity是读取身份，不叫持久化快照。刷新整份替换；不将query缓存或local/session storage作为第二材料来源。GET不派任务、不写快照。

停止点：新语义/权限、生产API或数据库修复、实际并行所有权冲突升级Mog；常规实现选择自行解决。无DB查询变更便不新增迁移证明；不增加故障/缩放/恢复的默认发布门槛。真实质量样本、G4与C1仍待明确范围和授权。

## 验证与未证明边界

候选已实现。定向8项通过；完整前端88文件995项通过后，对合同诊断及领域单独切换补测8项通过，类型/构建与lint已运行。正式React→Axum→自有PG只读主路径通过，含缺失/受限、字面引用、身份、刷新和1440/390，9类副作用计数前后一致，owned容器/卷清理通过。源码只新增本地区域/解码/测试，无生产API、查询、migration或运行时改动。最终CI与一次收尾审查结论看PR，不在此逐次复制。

未合并部署；未读真实材料、未补数据或运行模型/媒体/采集；不代签#420 G4。

## 主线集成接缝

C0 #459已另获Mog授权合并发布为main2dc1f520；本包从原616ac30f重放到该基线。四份追加型文档冲突保留两包意图，Web CI保留C0与材料预览两步；本包生产源码/测试逐文件与原收尾审查哈希相同。仅更正C0计划的发布未证项与私有证据迁移/清理生命周期，不改产品语义，不重新开展整轮审查。最终提交与CI以#460为准。
