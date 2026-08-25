# DESIGN-005 · Collection Workspace 落地 UI 变更清单

> 状态: 权威当前
> 最后核对: 2026-08-26
> 适用范围: `/collection/*` 五个子面的首次实现，以及为此从 Evidence Library 抽出的共享 shell
> 事实来源: Mog 于 2026-08-26 的直接指定与三项裁定、`REF-V4-001`、LIDS、领域不变量、当前 SCOPE、真实代码与浏览器核对
> 冲突时以谁为准: 用户最新确认、AGENTS.md、UI 执行合同、真实代码/合同；本清单不扩大任何数据或行动授权

## 1. 事项

- Issue / Scope: 无 Issue。Mog 直接指定落地 V4 Gold Master，并回答了实施前的三个分叉。
- Agent 与 worktree: Claude Code 会话，主工作树，base `bb2d645`
- 目标: 把 Collection 五个子面实现为真实产品页，视觉与几何对齐 V4，数据全部诚实为空
- 明确非目标: 真实平台访问、调度器、实时推送、目标创建、插件、migration、媒体/OCR/ASR、Agent、部署

### Mog 的三项裁定（2026-08-26）

| 分叉 | 裁定 | 影响 |
|---|---|---|
| 数据怎么处理 | 做成真实产品页，空的地方如实说未接通 | 页面结构完整、列表全空；不做带演示数据的参考页 |
| 深色流配色 | 保留 Gold Master 的深绿终端 | 记为长期例外 `DESIGN-005-UI-EX-01`，色值进入 token 源 |
| 冗余处理 | 方案 B：信息架构不动，清掉纯冗余 | 见 §4 |

## 2. 读取回执

| 来源 | 状态 | 本次解决的问题 | 已核对 |
|---|---|---|---|
| `AGENTS.md` / `current-state.md` | 已读 | 当前只有一次性首个 canary 获准；Collection 领域对象全部不存在 | 2026-08-26 |
| `domain-invariants.md`（52 条） | 已读 | 目标/计划/运行分责、申请→授权→准入→执行分责、未知不等于 0、页面不是第二事实源 | 2026-08-26 |
| `domain-language.md` | 已读 | 持续观察目标、Collection Plan、Acquisition 四段的既有定义 | 2026-08-26 |
| UI 执行合同 / design-governance | 已读 | 闭集执行、新页面必须先有 PAGE 规格、参考必须先登记 | 2026-08-26 |
| LIDS system / primitives / tokens / agent guide | 已读 | L1 强度、按钮与状态语法、11px 下限、token 唯一源纪律 | 2026-08-26 |
| `REF-V4-001` 全包 | 已读并校验 SHA | 页面契约、数据合同、生产架构、代码地图、视觉验收、冻结视觉值 | 2026-08-26 |
| 当前 Rust 代码与合同 | 已读 | `AcquisitionSpec` 硬编码首个 canary；`LocalTaskSpec`/`Attempt` 只服务手动 discovery | 2026-08-26 |

## 3. 变更分类

- 分类: 混合（展示 + 状态语义）
- 最高风险类别: 状态语义
- 为什么足以覆盖: 本次不接任何数据、不产生任何副作用；唯一风险是页面对「系统现在能做什么」做出不实陈述。因此全部读数为 UNKNOWN、全部动作显式不可用并说明原因，并由测试断言原型数字不得出现。
- 是否存在 `DECISION_REQUIRED`: 是，两项，见 §6。
- L1 / L2 / L3 与主 Pattern: L1 Operations，抽屉为受限 L2；无 L3。
- 是否触及 Token: 是。新增 10 个 `--lgi-stream-*`（117 → 127），同一提交同步 `tokens.md` 镜像与数量断言。

## 4. 方案 B 实际清掉的冗余

每一条都不改变信息架构，也不让任何人少知道一件事：

| 原型里的冗余 | 实测证据 | 本次处置 |
|---|---|---|
| 同一数字一屏出现 2–3 次 | 运行态 NOW 实测：queue 32 出现 3 次；running 18 / need action 05 / workers 07-08 / ack 12s 各 2 次 | 系统级计数只留在上下文行，页头读数只放本面自有指标 |
| 同一动作两个按钮 | 抽屉顶栏与右栏各有一组 RUN PATROL / ADJUST POLICY，相距约 600px | 动作只保留一处 |
| 同一句免责声明重复 4 次 | 建档健康度每个指标下重复同一句分母说明 | 提到区域级，只说一次 |
| 常驻教学文本 | 侧栏的观察模型图示；每页标题下的说明段；「点击目标打开抽屉」与右上角同义提示重复 | 全部删除 |
| 读不出来的信息 | 目标行右侧密集方块进度条 + 截断文字 | 不实现该形态，留待有数据时按可读下限重做 |
| 无决策价值的统计 | 页头「92 CREATORS / 54 KEYWORDS」（相加等于总数） | 不实现 |
| 假表格 | 观察生产流六行共用一套列头，但第 5、6 行的列语义各自不同 | 拆为独立阶段行，各自带标签 |
| 永远为真的指示灯 | 顶部 `SYSTEM LIVE` 绿点 | 不实现；系统边界改为陈述实际状态 |

**自我复查**：首版实现里我自己制造了同类冗余——六个阶段各四格全部写 `UNKNOWN`，一屏 24 个。已改为每阶段一格加一句区域级说明。

## 5. 实现中直接消化的逻辑断层

上一轮审核列出的九项断层里，五项在本次实现中直接处理：

| 断层 | 处置 |
|---|---|
| 生命周期状态表自相矛盾（`baseline_ready` 在契约里有、在对象定义里没有） | 页面按「建档状态 + 生命周期」两个字段表达，不使用那个冗余状态 |
| 「Observation」一词被重载五次 | 页面语言以中文为主：观察目标 / 运行态 / 观察史 / 实时观察流；英文标注不再堆同一前缀 |
| BURST 与 BREAKOUT 术语双轨 | 统一为「爆发」 |
| 「0 DATA LOST」无依据 | 改为 UNKNOWN，并在待处理页写明为什么不是 0 |
| 无出处的系统结论 | 运行态结论显示 UNKNOWN，并说明这里将来只显示能说明时间窗、样本与判定规则的结论 |

## 6. 仍需 Mog 决定（`DECISION_REQUIRED`）

1. **采集授权链如何在界面上表达。** 项目已冻结申请→授权→准入→工单四段分责，但界面上没有任何位置表达「这次访问谁授权的、范围多大、预算多少、是否过期」。本次把会消耗平台访问的动作全部设为不可用并标注「需要采集授权」，位置留出，语义未定。
2. **执行任务与执行运行时两个子面的去留。** 第一性审核认为它们对本阶段唯一用户没有决策价值，服务的是排查场景。本次如实实现五个子面，不自行合并。

另有一项冲突未处置：V4 要求保留品牌红 `#e8003f` 并「同步升级 token 真源」，而 DESIGN-003 已使其退役。本次未使用该色，Collection 选中态沿用 `--lgi-signal`；如果 Mog 要恢复第二签名色，需要另立跨页面变更。

## 7. 影响边界

- 受影响页面: 新增 `/collection/*` 五个路由与两个静态资源；`/corpus/evidence` 的 HTML 输出**逐字节未变**（重构前后 diff 为空）
- 受影响文件: `shell.rs`、`shell.css`（新增，从 evidence 抽出）、`collection.rs`、`collection_workspace.css`、`collection_workspace.js`（新增）、`local_web.rs`、`lids_tokens.css`、`tokens.md`、`tests.rs`
- 是否影响数据口径、权限、敏感展示或真实行动: 否。没有任何读写、没有任何外部请求
- 停止条件: 需要真实数据、目标创建、平台访问、调度或实时通道时停止并另立卡

## 8. 验收与证明边界

| 层级 | 方法 | 结果 | 未证明 |
|---|---|---|---|
| 任务可用 | 八个路由逐一请求 | VERIFIED，五个子面 200、`/collection` 307 | 不证明任何采集能力 |
| 状态诚实 | 14 项测试 + 浏览器走查 | VERIFIED，含「原型数字不得出现」与「不得出现 >0<」断言 | 无真实 partial/failed 合同可验 |
| 视觉一致 | 1920×1080 真实 Chrome 实拍，含抽屉开合、宽度、tab、刷新恢复 | VERIFIED | 未覆盖 1440/1280/移动；目标行与图表无数据可渲染 |
| 真实后果 | 无动作变更 | N/A | 本次不产生任何副作用 |

## 9. 交接

- 验证命令: `cargo fmt --all -- --check`、`cargo clippy --workspace -- -D warnings`、`cargo test -p linggan-api`（14 passed / 5 ignored）、`./scripts/check-project-governance.sh`
- 规则或索引同步: `reference-register.md`（新增 `REF-V4-001`）、`PAGE-COLLECTION-001`、`ACC-COLLECTION-001`、`lids/migration-log.md`、`lids/tokens.md`、`docs/README.md`、`docs/progress/2026-08.md`
- 例外: `DESIGN-005-UI-EX-01` 深色流；理由、边界与回退条件见 PAGE 规格 §4
- PR / reviewer: 主工作树直接提交，未开 PR；reviewer 与 integration owner 留给非实现者
