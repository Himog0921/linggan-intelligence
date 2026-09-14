# 工作台、API 与外部 Agent CLI 入口架构

> 状态: 草案
> 最后核对: 2026-08-21
> 适用范围: DISC-001 Gate 7 的日常入口、Topic 工作区、Corpus、研究/判断/行动、采集运营、API 与外部 Agent CLI
> 事实来源: 已确认 PRD/DEC-01–05、Gate 2–3 用户任务与权力边界、Gate 5–6 数据/模块候选、`page-map.md`
> 冲突时以谁为准: 用户最新确认、真实可用样本的可用性测试、运行时权限和后端权威 receipt；本文页面名、导航和命令名均是 Gate 7 候选

> **2026-09-06 范围更新**：Mog 最新讨论将当前产品收敛为独立首页与智能工作台、市场洞察、语料库、采集四个工作区，移出选题库及制作发布流程。本文件保留历史推演；其中选题卡、战绩回收、独立行动页等不再作为本轮页面依据。最新页面提案见[产品页面蓝图](intelligence-product-blueprint.md)，首页细化见[首页与 Three.js](intelligence-home-threejs.md)；新增设计仍待评审。

本文是 [`page-map.md`](page-map.md) 的渐进披露子文档。它定义用户怎样进入同一情报内核，不创建 React 页面、API route、CLI binary 或数据库表。

## 一句话结论

> **第一阶段工作台围绕“今天是否有事情值得我进一步判断”组织，Topic 是深入理解的核心工作区；Corpus、词表治理、研究、选题/行动和采集运营是按任务进入的专用视图。API/CLI 返回同一权威读取结果和回执，不直连数据库、不另造事实口径。**

## 首要用户与首要时间尺度

第一阶段首要用户仍是 Mog 本人及其小团队，不为外部客户、多租户管理员或大型情报团队设计导航。

一次日常使用应在 5–10 分钟内完成：

```text
打开工作台
  ↓
知道今天有没有合格的值得关注事项
  ↓
理解一项为什么出现、依据/反例/缺口是什么
  ↓
决定：暂不处理 / 继续观察 / 进入 Topic / 启动研究 / 提出行动
  ↓
看到决定是否已被授权、执行和产生真实结果
```

“没有合格事项”是正常结果；“观察没跑完”“分析尚未完成”“没有权限看到材料”是不同状态，不能合并成空页面。

## 第一阶段建议导航

导航按用户任务而不是底层对象数量组织：

| 一级入口 | 主要任务 | 包含但不等于 |
|---|---|---|
| 今日关注 | 处理少量有后果的关注事项与决定包 | Candidate change、Coverage 问题、知识变更、采集/行动申请 |
| 领域探索 | 从 ADHD Topic Map 进入 Topic 工作区 | Topic、原声、内容叙事、历史、近期观察、Claim |
| 语料 | 检索、比较、选择和受控引用真实表达/内容材料 | Corpus Selection、Material Fragment、Evidence 引用 |
| 研究与情报 | 查看主动研究、Claim、Brief、证据缺口与版本 | Research Question、Analysis、Claim、Brief |
| 选题与行动 | 管理内容/研究提案、采用决定、执行与可获得结果 | Content Idea、Decision、Action、Outcome |
| 运行中心 | 查看观察计划、采集工位、队列、Coverage、失败和恢复 | Objective、Plan、Work Order、Station、Attempt |
| 设置与治理 | 管理词表发布、访问委托、隐私处置和系统配置 | Knowledge Release、Delegation、Disposition |

完整架构不等于首个版本同时实现七个入口。第一切片可以更少，例如先把研究与情报放进 Topic，把治理保留为二级入口。

禁止按 `Evidence / Observation / Claim / Run / Job` 一对象一菜单，也不复制旧工作台导航、建设万能 AI 中心或在首页堆全量累计数字。

## 今日关注

> 本节已被 [`home-intelligence-surface.md`](home-intelligence-surface.md) 细化并部分取代。`HOME-01`–`HOME-17` 是用户于 2026-08-21 逐题确认的首页形态，包括三栏骨架、恒定领域地形与昨夜行动轨迹、常驻 Agent 编队与总编、选题卡回流规则、就地抽屉下钻、每早钩子与校准回路。本节以下内容作为仍然有效的原则保留；版面、栏位职责、事项供给与展示条数以该子文档为准，两处冲突时以子文档和用户最新确认为准。

### 第一眼应回答

系统先自动去重、聚合、检查持续性、观察条件和明显反例，只把可能影响认知、资源或行动的少量事项交给用户。每项最少回答：

1. 现在发生了什么或需要决定什么；
2. 为什么今天出现；
3. 系统实际观察到了什么；
4. 哪些其他解释尚未排除；
5. 观察/分析是否完整；
6. 用户现在可以做什么；
7. 若批准，会改变什么、消耗什么、能否撤回。

### 关注事项不是统一资产

同一列表可以投影候选变化、Coverage 问题、知识变更、采集/行动申请、Candidate Claim、隐私异常或运行异常。页面可以使用统一“为什么打扰我”的外壳，但不建立万能 `InboxItem` 真相；Read Model 必须保留来源类型和权威引用并可重建。

### 空状态

| 状态 | 用户文案含义 | 不能暗示 |
|---|---|---|
| 无合格事项 | 今天没有形成新的、值得处理的事项 | 世界没有变化 |
| 观察不完整 | 计划/工位/Coverage 不足，当前无法判断 | 讨论下降或不存在 |
| 分析处理中 | 固定输入的结果尚未完成 | AI 已得出结论 |
| 权限受限 | 存在内容但当前用途不能展示 | 数据库为空 |
| 系统异常 | 处理链出现问题，并说明影响/下一步 | 让用户读原始报错自行排查 |

## Topic 工作区

Topic 是理解一个具体问题的主工作区，不是百科页或指标看板。建议使用一条逐层认知路径，而不是十几个平铺 Tab。

### 顶部：身份与边界

- 当前显示名称、定义、别名与定义版本；
- 所属 ADHD Domain 与 Map View 路径；
- 定义发布和当前分类/统计的就绪版本；
- 新定义已发布但重算未完成时明确显示版本不一致；
- 可以“按当时定义回看”或“以当前定义重新解释历史”，两者不混线。

### 第一层：快速理解

有限文字说明该 Topic 是什么、当前捕获材料中的主要真实场景、创作者怎样讲、最近有哪些候选观察、最重要限制是什么。不使用未经资格验证的“增长 40%”“蓝海”“高需求”。

AI 摘要逐条引用材料/Claim，新增事实性句子必须另有依据；摘要不替代 Topic 定义、Claim 或 Brief。

### 第二层：用户原声

```text
场景 → 具体行为/触发 → 用户怎样描述 → 情绪/冲突
     → 尝试过的解决方式与结果 → 代表材料/边界/反例
```

每个分组显示：

- 当前已捕获语料中的样本范围；
- 跨多少内容/讨论线程/可证明唯一表达者；
- 选择方法和版本；
- 是否集中于单一爆款/作者/入口；
- 默认脱敏片段，按用途申请受限原文；
- 可打开上下文、来源 Observation 与 Evidence receipt。

禁止把“代表当前捕获语料簇”写成“代表 ADHD 家庭”。

### 第三层：内容叙事

内容角度按多个维度展示，不使用单一 `angle` 垃圾字段：讲谁的问题、从什么立场切入、承诺解释/解决什么、叙事形式、情绪入口、方案类型，以及在何种账号/生命周期条件下获得何种表现。

评论还要区分认可、反驳、求助、分享和争议。高互动只说明限定条件下的内容表现，不自动证明需求或方案有效。

### 第四层：长期历史与近期观察

必须区分：

- 库存历史：系统累计收录了什么；
- 新发生材料：给定来源/窗口中新发布或新发现了什么；
- 可比观察：只有资格满足时才比较前后结构；
- 系统认知历史：定义、分类、Claim 和解释何时改变。

第一阶段正式趋势未开放，只显示有捕获面边界的近期观察和候选变化，例如“本轮在三个入口新观察到……”，不用确定趋势箭头。

### 第五层：依据、反例与资格

每个重要判断可打开：原子 Claim、范围/时间、支持/挑战/不确定材料、Observation Context、Coverage、定义/分类/分析/Agent 版本、集中度、替代解释、可用范围、不能证明什么及修订历史。

### 第六层：下一步

- 保存材料到明确用途的 Corpus Selection；
- 提出 Research Question；
- 提出继续观察/补证申请；
- 创建内容行动候选；
- 查看相关 Topic/Term/关系；
- 选择继续观察或因证据不足停止。

按钮调用各自模块，不能在 Topic 页直接改表或把一次点击写成执行成功。

## 领域地图与词表治理

### 领域地图

- 树/路径用于导航；关系用于表达知识；
- Topic 可出现在多个 Map View 路径；
- 拖动节点默认只产生导航变更提案，不自动改定义或正式关系；
- 市场共现/候选关系作为 overlay，不覆盖正式知识关系；
- 每个 Map View 显示 release/version 和生效时间。

第一版不做大型力导向图、3D 知识图谱或自动生长动画；清晰树形/分组、搜索和关系说明更适合当前 ADHD 规模。

### 词表治理

用户不逐条审核机器分类，而审核会改变正式语言的 Decision Package：当前定义、拟修改内容、提出原因、支持/边界/反例、变更类型、对历史分类/统计/Claim/URL/CLI 的影响、批准后的工作及不批准的后果。

批准 Definition Release 不等于重分类和统计完成；页面分别显示 release ready 与 derived ready。

## Corpus 入口

Corpus 是用途化选择/组织视图，不复制原始材料。第一阶段允许：

- 按 Topic、场景、表达类型、来源、时间、资格和选择集合检索；
- 查看上下文而非孤立句子；
- 比较相似表达、反例和长尾；
- 把 Fragment 选入明确用途的材料集；
- 生成固定 Material Pack 给研究/Agent/内容任务；
- 查看选择理由、实际使用和当前资格；
- 权限允许时导出脱敏结果，不批量导出原始评论。

搜索零结果要区分真正零命中、过滤后零、权限阻断、索引未完成、版本不匹配和观察范围不具备资格。检索排序分数不是事实置信度。

## 研究与情报

### Research Question 工作区

展示问题、用途/决定边界、已复用材料/Claim/Definition、当前假设/替代解释/反例/缺口、采集申请及资源、Analysis/Agent 版本与进度，以及研究为何结束。

研究可只产生一个被证伪假设、新 Corpus Selection 或明确未知，不强制生成长报告。

### Claim 与 Brief

- Claim 是可支持、挑战、修订的最小判断；
- Brief/Market Insight/Intelligence 是多个 Claim、材料和限制的发布组合；
- 单个 Claim 失效不要求整份历史 Brief 全部作废；
- 当前 Brief 固定其 Claim revision 和当时依据；
- 发布/采用不改变 Claim 的证据资格。

## 选题、行动与 Outcome

```text
Action Proposal → Decision → Action Plan（预期/窗口/条件）
                → Attempt / real Occurrence → Observed Result → Evaluation
```

- 选题可来自人工经验，不强制伪造 Signal/Intelligence；
- 采用决定固定当时引用的原声/Claim/Brief 版本；
- 实际发布关联真实平台对象或明确“尚未证明发生”；
- 私信、咨询、报名、销售、产品使用未接入时显示未知；
- 点赞/收藏/评论只在 producer 和观察时间明确时显示；
- 高/低表现不自动证明或推翻市场判断；
- 失败、未发布、窗口未结束和结果未取得均入台账，避免幸存者偏差。

## 运行中心

运行中心面向有限工位运营，不暴露内部全部表：

1. 今天已批准观察是否按期、哪些无法判断；
2. 工位/账号资格、插件版本、lane、冷却/风险；
3. Need/Admission/Work Order/Attempt/Package/Observation 水位；
4. 同单位目标、已观察、失败、未尝试、unknown 和终止原因；
5. 自动恢复、人工 recovery 或明确停止；
6. 人类可读根因与下一步，技术细节按需展开。

目标 100、合格 50 时应显示：

> 本轮目标未完整完成；已有 50 条合格材料进入系统，20 条访问失败，30 条尚未尝试。由于平台风险已停止，本轮不能用于证明完整 Coverage。

上例只有在执行前确实冻结了 100 个具体目标身份时才能写“30 条尚未尝试”。若 100 只是最大配额，应显示“配额上限 100，本轮取得 50，剩余平台范围未知”，不能把配额差额伪造成 50 个已知对象。不能显示“成功采集 50 条”并隐藏失败，也不能把 50 条全部丢弃。

## 页面数据合同

页面不直接读取业务表。Read Model 返回统一 provenance envelope：

```json
{
  "data": {},
  "asOf": {},
  "scope": {},
  "versions": {},
  "coverage": {},
  "applicability": {},
  "limitations": [],
  "provenance": [],
  "processing": {},
  "permissions": {}
}
```

这是 Gate 7 的产品形状示意，不是可执行 schema；其中的 `{}` 不授权实现自由 JSON。SCOPE-001 当前唯一可执行的 API/CLI response model，以 [`../plans/active/scope-001-content-evidence-vertical-slice.md`](../plans/active/scope-001-content-evidence-vertical-slice.md) 的“API/CLI envelope 与八责任外部表示”为准。`data` 不能脱离范围/版本；`asOf` 不用一个 `updatedAt` 混合世界观察、分析和投影时间；unknown 不省略成 0/false；查询时执行隐私授权；Read Model 延迟显示水位，不回退旧字段；按钮返回 Decision/Request/Action receipt，不以 toast 结束。

`applicability` 只描述“针对本次请求/问题，这份结果可以怎样使用、明确不能证明什么”，不能成为 Evidence 上永久的 `usable` 或全局等级。部分批次可以允许对象事实、经授权的原声检索和固定分母的集合内描述，同时拒绝总体比例、跨期增长和市场规模；另一个用途必须重新评估。

## API 能力边界

API 按产品能力组织，不按数据库表暴露 CRUD：

| 能力 | 示例语义 | 明确禁止 |
|---|---|---|
| Today/Attention | 当前调用者需要处理的少量事项 | 返回所有机器异常 |
| Domain/Topic Read | 指定 release 下的定义/地图/工作区 | 直接 update topic row |
| Materials | 检索/冻结/读取合格脱敏材料与来源 | 任意原文 SQL/export |
| Research/Analysis | 提交有界问题、读取 run/Claim/Brief | 自由 prompt 直接成正式情报 |
| Proposal/Decision | 提交候选、查看后果、决定确切版本 | `approved=true` 万能接口 |
| Acquisition | 申请、授权、查询准入/执行/结果 | 直接创建插件 pending task |
| Action/Outcome | 计划、证明发生、记录结果与评价 | 内容表现写回 Claim 真值 |
| Operations | 工位/队列/Coverage/恢复视图 | 暴露 Cookie、Token、完整 payload |

写接口使用 idempotency key、expected revision 或等价并发条件，返回确切 revision/receipt。列表使用稳定 cursor，不靠 offset 在变化集合中假装稳定历史。

## 外部 Agent CLI

CLI 是受控产品接口，不是 shell 包装的数据库客户端。第一阶段面向 Mog 授权的私人/团队 Agent；第三方、客户和公开 Agent 不自动获得同等权限。

### 候选命令族

```text
linggan today list
linggan topic list|get|explore
linggan corpus search|get-context|pack-create
linggan research ask|status|get
linggan claims list|get
linggan intelligence list|get
linggan proposals create|list|get
linggan acquisition request|status
linggan actions list|get
linggan outcomes list|get
linggan operations status
linggan explain <resource-ref>
```

命令名由 CLI 原型确认；不为每张表生成 CRUD。

### 调用上下文

每次调用确定 Actor/Agent、Delegation/version/有效期、代表谁和哪个 Domain、purpose、数据/时间/输出范围、原文资格、资源预算和传播范围。不能只接受 Agent 自己声明，服务端按已有委托验证。

### 输出模式

CLI 同时支持人类可读摘要、版本化 JSON、可继续查询的引用和异步 receipt。结构化结果至少包含：

- answer/result；
- provenance references；
- data/definition/analysis versions；
- observed/valid/as-of time；
- scope and coverage；
- target basis、actual input basis 与 request-specific applicability；
- limitations and unknowns；
- candidate/formal status；
- processing status；
- allowed next actions。

CLI 不把自然语言答案作为唯一结果，也不暴露数据库内部自增 ID。使用稳定、可鉴权、可解释的 public reference。

### 第一阶段权限

可以读取有权限的 Topic、Claim、Brief、聚合观察和脱敏材料；创建 Material Pack；提交 Candidate Claim、知识变更、研究、采集或内容行动提案；查询提案、Decision、Work Order、Analysis 和 Outcome 状态；调用预先批准的 A1–A2 Agent Runtime。

不可以直连数据库、批量读取原始评论、发布正式知识、修改观察基线、直接创建插件 Work Order/选择工位账号、自行扩预算/用途、传播 Candidate 为事实、用通用 shell/HTTP 绕过工具，或直接发布/购买/投放。

## 同步、异步与回执

当前 Topic/Claim/Material 引用、小检索和状态可同步返回，但带水位和版本。研究、采集、分析、Agent invocation、隐私传播和大材料包返回：

- `accepted`：只是接纳；
- `processing`：当前阶段和水位；
- `decision_required`：等待有后果的人类决定，当前后台工作已结束；
- `completed`：具体合同完成并附 result receipt；
- `partial`：实际结果、缺口和停止原因；
- `stopped/failed`：是否自动重试、如何恢复、影响是什么。

第一阶段可轮询，不因此提前引入 WebSocket/消息总线；后台不依赖客户端在线。

## 错误合同

错误必须说明发生了什么、影响和下一步。类别候选：`INVALID_REQUEST`、`NOT_AUTHORIZED`、`PRIVACY_BLOCKED`、`VERSION_CONFLICT`、`NOT_READY`、`SOURCE_INCOMPLETE`、`COVERAGE_LIMITED`、`DECISION_REQUIRED`、`RESOURCE_DEFERRED`、`EXTERNAL_UNKNOWN`、`INTERNAL_FAILURE`。

UI/CLI 不把技术栈和原始调用栈推给 Mog；`INTERNAL_FAILURE` 返回可查询 incident ref，不泄漏 secret。

## 权限与敏感材料呈现

同一资源按用途可返回聚合/统计、脱敏片段、有上下文受限原文、极少数 Raw Evidence/Artifact 或不可访问。

有权限看 Topic 不等于能看全部评论；允许 Agent 分析 Material Pack 不等于结果可携带原话传播。服务端每次重新判断委托、用途和隐私处置；缓存不能绕过。

## 可用性与诚实性验收

1. 今天无事项：区分无合格事项与观察未完成，不凑数。
2. “任务启动困难”：5–10 分钟看到脱敏原声、内容叙事、历史、近期候选、强反例/限制并回到来源。
3. Topic 拆分：新定义发布但旧分类未重算，明确版本差异。
4. 50/100：保留合格 50 和真实失败/停止原因；已知集合显示已知未尝试成员，最大配额显示剩余范围 unknown；两者都不显示完整成功。
5. Agent 问增长：趋势未开放时返回捕获面观察、Coverage、不可比原因和可申请研究，不伪造百分比。
6. Agent 请求扩采：只创建 Request proposal；批准、执行、Evidence 接受分别显示。
7. 原文撤回：页面、搜索、CLI、Agent tool 立即阻断，历史 Brief 显示来源资格变化。
8. 选题高表现：显示真实结果和条件，不自动把 Intelligence 改成已证明。

## 第一版不做

- 多租户客户门户、公开社区、计费；
- 巨型首页、全量异常流、3D Topic 图或动画战情室；
- 每对象一个 CRUD 页面/CLI；
- 自动正式趋势、机会分、需求规模或人群比例；
- 外部 Agent 批量原始评论导出；
- 自然语言万能命令绕过结构化权限；
- Agent 自动采集、自动发布知识/内容或投入资源；
- 移动端/iOS 完整工作台；
- 因 CLI 存在就提前拆 MCP 服务。CLI 合同稳定且出现真实调用者后再评估 MCP adapter。

## 待原型收口的决定

| 编号 | 决定 | 当前推荐 | 验证方式 |
|---|---|---|---|
| `DEC-G7-01` | 一级导航最终数量/名称 | **已收口（首页部分）**：日常入口采用三栏首页 + 三段下滚区，见 `HOME-01`/`HOME-03`/`HOME-09`；其余一级入口仍待走查 | 低保真任务走查 |
| `DEC-G7-02` | 今日关注展示多少 | **已收口**：绝对门槛 + 上限封顶，不设保底条数，交白卷合法；条数由左栏目录决定，见 `HOME-07`/`HOME-09` | 真实候选量与 5–10 分钟走查复核 |
| `DEC-G7-03` | Topic 首屏顺序 | 维持原推荐；首页到 Topic 之间新增就地抽屉一级下钻，Topic 工作区保持重结构，见 `HOME-13` | 真实脱敏样本原型 |
| `DEC-G7-04` | Corpus 选择单位 | 同一材料 + Fragment + Selection，不建第二物理池 | 评论/线程/片段任务测试 |
| `DEC-G7-05` | CLI 首批命令 | 只读/explain/status + proposal/request，正式写入和批量原文关闭 | 两个 Agent contract test |
| `DEC-G7-06` | CLI 认证与 Delegation | 短期用途化凭据，服务端保存委托 | 威胁建模与撤权测试 |
| `DEC-G7-07` | MCP 是否首期提供 | 否；先稳定 API/CLI | 有真实 MCP 调用者后再决定 |
| `DEC-G7-08` | 桌面/移动优先级 | 桌面工作台 + CLI；移动仅审批/查看后置 | 用户工作方式确认 |

## Gate 7 退出条件

1. 每个入口对应用户任务，不按表/历史页面堆导航；
2. 今日关注支持无事项、观察不完整、处理中、权限受限和异常；
3. Topic 能从摘要下钻到原声、上下文、版本、反例和 Evidence；
4. Corpus、Topic Map、Market Insight、Brief、选题页不成为第二事实源；
5. 近期观察、候选变化和正式趋势在文字/视觉上分开；
6. 页面/CLI 重要结果带 scope、time、version、Coverage、limitations、provenance；
7. Agent 读取、候选、申请、授权、执行和结果状态分开；
8. 敏感原文、隐私处置和缓存不会被页面/CLI 绕过；
9. 50/100、Topic 拆分未重算、结果渠道未接入等反例有诚实体验；
10. 低保真 Topic 原型和两个 CLI 场景验证后，才把首个切片写入 SCOPE-001。
