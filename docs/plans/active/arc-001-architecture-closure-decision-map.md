# ARC-001 代码前产品与系统架构收口决策图

> 状态: 活跃计划
> 最后核对: 2026-08-21
> 适用范围: F01 之后、首个真实 producer 与首个用户可见产品切片之前的产品和系统架构收口
> 事实来源: 用户最新 course correction、当前开发跟踪表、产品/页面草案、采集与媒体历史经验、`ADV-AUDIT-001` 最终处置
> 冲突时以谁为准: 用户最新确认、`AGENTS.md`、ACCEPTED ADR、当前活跃 SCOPE 与可复现代码/数据库事实；本图不自动授权真实访问或实现

## 当前执行护栏

F01 可以继续且只可以继续到：

```text
synthetic Package
→ PostgreSQL atomic ingress
→ two independent Records
→ Observation / Current
→ loopback API
→ minimal CLI
```

完成即 hard stop。ARC-001 未收口前，不开始 F02–F10、真实 producer、插件升级、媒体、Agent crew、Web、生产或旧系统迁移。当前 F01 内不改变产品含义的合同、migration、来源追踪和命名修正由工程团队负责，不建立 Mog 决策票。

审计衍生的 `ADV-M-01` 最高权威冲突规则仍为单独的 `DECISION_REQUIRED`；它不改变本图票序，也不授权低层实现或文档静默改写产品含义。

## Notes

Domain: Linggan Intelligence 第一阶段产品与系统架构收口

Consult: `decision-mapping`、`grilling`、`domain-modeling`、`prototype`、`product-manager`、`codebase-design`

Standing preferences:

- 一次只解决一个 ticket；
- Mog 只决定产品形态、真实资源、隐私、成本和对外后果；
- 技术选型、表结构、crate、状态管理和测试工具由工程团队负责；
- 整体逻辑架构面向全项目，物理架构只冻结第一阶段和已确认的扩展接缝；
- 不因现有首页草案丰富就默认五 Agent 编队、等距地形或七个导航已进入 P0；
- 历史能力只继承用户任务、业务价值、关键不变量和已验证交互认知；旧实现必须先经过 capability inheritance audit，不能因“保留能力”自动复制；
- ticket 产物只链接到本图，不把专题设计复制进本图。

## product-shell: 第一阶段以什么产品形态存在？

Blocked by:
Status: resolved
Type: Grilling
Decision owner: Mog

### Question

Linggan 第一阶段以什么独立产品形态存在，旧内容工作台与它是什么关系，哪些历史能力应当继承而不复制旧实现？

### Answer

Mog 已确认：

1. **Linggan Intelligence 是独立且完整的 Web 产品。** 它不嵌入内容工作台，也不以与内容工作台长期分工或交接作为产品定义。这里确认的是产品身份，不是 Web 实现已经获得授权。
2. **内容工作台是 Linggan 的来时路。** 它是历史原型、最小 MVP、需求来源和经过实践的能力样本，不是 Linggan 的运行时依赖、下游执行系统或未来架构边界。
3. **已证明有价值的用户能力不能因重做而丢失。** 当前至少包括采集能力、任务调度模式、Media V2 的领域思想，以及市场洞察中的主题地图和相关成功页面、交互。
4. **保留能力不等于复制旧实现。** 保留的是用户任务、业务价值、关键不变量和经过实践验证的交互认知；本票不授权复制旧代码、旧数据库表、旧接口、旧页面层级或旧技术债。
5. 每项历史能力进入 Linggan 前都必须经过 **capability inheritance audit**：

   ```text
   旧能力解决什么用户问题
   → 哪些部分有真实成功证据
   → 旧实现有哪些限制或事故
   → Linggan 中更高维的新定义
   → 采用 / 重构 / 重新设计 / 退役
   → 新的验收标准
   ```

6. 与旧内容工作台的数据导入、迁移、兼容、导出或“情报弹药包”只是未来可选接缝，不定义 Linggan 的产品身份，本票不授权实现。
7. 本票只解决 `product-shell`。它不决定首页或页面数量、每日主任务、首个 producer、媒体是否进入 Canary，也不授权旧系统迁移。

## primary-daily-job: 用户每天首先要完成什么？

Blocked by: product-shell
Status: resolved
Type: Grilling
Decision owner: Mog

### Question

Mog 每天进入 Linggan 的 5–10 分钟内，系统必须帮助他完成的那一个核心判断或结果是什么？

### Answer

本票直接引用已确认的 `DEC-02 / USER-DEC-06`，不重新发明日常任务：

- Mog 每天先看到**少量值得关注事项**，理解为什么现在出现、系统实际观察到了什么、哪些解释或缺口仍不确定，然后决定：暂不处理、继续观察、进入 Topic 深入，或提出行动。
- Topic 是深入工作区，承载用户原声、内容叙事、长期历史、近期观察以及对应的 Coverage / Evidence；首页负责把人带到值得深入的地方，不替代 Topic 研究。
- “今天没有合格事项”与“今天的观察不完整”是两种不同结果，必须分别说明，不能都显示成空白或“暂无”。

该答案只确认用户每天首先完成的认知与决策任务。它不决定五个 Agent 运行实例、等距地形、卡片数量、通知、夜间调度或其他页面实现；这些仍由后续 `p0-surface-prototype` 与 `first-phase-runtime` 验证和收口。

## p0-surface-prototype: 第一版准确需要哪些可见界面？

Blocked by: primary-daily-job
Status: open
Type: Prototype
Decision owner: 产品团队制作，Mog 走查确认

### Question

为了完成已确认的第一条日常工作流，P0 应准确包含几个一级入口、几个详情工作区，以及哪些能力只应当是 Tab、抽屉或后台状态？

### Answer


Asset target: `docs/product/first-phase-interface-prototype.md`

验收必须包含三条真实脱敏样本走查：早晨判断、Topic 深入、采集不完整；同时明确 P0 不做清单。

## first-producer-canary: 首批真实 Canary 是否包含媒体？

Blocked by: primary-daily-job
Status: open
Type: Grilling
Decision owner: Mog

### Question

首批获得授权的真实 producer Canary，是只验证内容身份、标题、正文、作者、时间、有限评论和 Coverage，还是同时包含图片/视频下载与转录？

### Answer


## capture-control-contract: 真实插件前必须冻结哪些调度责任？

Blocked by: first-producer-canary
Status: open
Type: Research
Decision owner: 工程架构团队；只有真实账号、风险或资源边界扩大时再交 Mog

### Question

结合旧内容工作台事故与新系统 Acquisition Admission，新 Linggan 在真实插件开工前最少必须冻结哪些 Demand、Work、Attempt、Lease、Reconcile、Package、ACK、Coverage 和恢复责任？

### Answer


Asset target: `docs/architecture/capture-control-contract.md`

产物必须包含“旧事故 → 新不变量 → 新责任 → 可证伪测试”映射，不复制旧 `CollectionTask`、pending fallback 或旧表结构。

## media-lifecycle-contract: Linggan 的媒体后继架构是什么？

Blocked by: first-producer-canary
Status: open
Type: Research
Decision owner: 工程架构团队；隐私、外部模型和保留策略交 Mog

### Question

Linggan 怎样用最小模型表达媒体身份、来源代次、下载、字节、存储副本、用途、转码/OCR/转录、撤回和派生失效，而不退化成 URL 字段或复制旧 `MediaAsset`？

### Answer


Asset target: `docs/architecture/media-lifecycle-contract.md`

若首批 Canary 不含媒体，本票不阻塞纯文本真实 producer 探查，但必须在任何媒体 Lane 开工前 resolved。

## first-phase-runtime: 第一阶段怎样形成完整可运行系统？

Blocked by: p0-surface-prototype, capture-control-contract, media-lifecycle-contract
Status: open
Type: Research
Decision owner: 工程架构团队

### Question

用什么最小前端、后端、数据库、worker、插件接缝、AI 接缝和部署拓扑，才能实现已确认的 P0 工作流，同时不预建尚无真实调用者的未来模块？

### Answer


Asset target: `docs/architecture/first-phase-implementation-baseline.md`

产物必须明确前端框架/路由/状态边界、后端模块/API-worker 分责、当前物理数据库模型、真实 producer/媒体/Agent 接缝，以及开发、测试、部署和可观测性的最小闭环；不得一次性物理实现概念地图中的全部实体。

## first-user-visible-scope: 首个真正产品切片是否获准开工？

Blocked by: first-phase-runtime
Status: open
Type: Grilling
Decision owner: Mog

### Question

基于页面原型、真实 Canary 边界和第一阶段技术基线，哪一条完整用户工作流被正式批准为 F01 之后的首个用户可见 SCOPE？

### Answer


Resolved 后才允许创建新的活跃 SCOPE、对应 GitHub Issues，并更新开发阶段跟踪和当前状态；F01 只在取得 SCOPE-001 约定的可复现技术证据后转为已完成 technical tracer。该证据必须包含真实 PostgreSQL 副作用，但不要求真实 producer、真实插件或真实平台数据。

## Next steps

`product-shell` 与 `primary-daily-job` 已 resolved。当前有两个互不替代、均已解锁但尚未回答的票：`p0-surface-prototype` 与 `first-producer-canary`。

一次只推进一个 ticket 时：

```text
Invoke /decision-mapping with the map at docs/plans/active/arc-001-architecture-closure-decision-map.md, ticket p0-surface-prototype.
```

若由两个独立会话并行推进，分别使用：

```text
Session A: Invoke /decision-mapping with the map at docs/plans/active/arc-001-architecture-closure-decision-map.md, ticket p0-surface-prototype.
Session B: Invoke /decision-mapping with the map at docs/plans/active/arc-001-architecture-closure-decision-map.md, ticket first-producer-canary.
```
