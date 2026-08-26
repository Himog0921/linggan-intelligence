# LOCAL-001D · 默认呈现发布时间未知的已接纳 Discovery

> 状态: 活跃计划
> 最后核对: 2026-08-26
> 适用范围: Issue #62 的 Evidence Library retrieval、默认读取视角、未知发布时间标签、合成测试与受控文档同步
> 事实来源: Mog 的最新确认、REAL-CANARY #52 的聚合结果、`AGENTS.md`、`LOCAL-001`、`LOCAL-001C0-DISCOVERY-BOUNDARY-V1`、PAGE-EVIDENCE-001 与当前代码
> 冲突时以谁为准: 用户最新确认、`AGENTS.md`、真实代码/运行/合同、ACCEPTED 决定；本计划不改变真实 Canary 数据或扩大采集范围

## 目标与用户结果

当本地 Linggan 已接纳 discovery 卡片、但平台卡片没有提供可验证来源发布时间时，用户打开 Evidence Library 的默认日常读取入口仍能看见这些材料，并明确知道其 `PUBLISHED_AT UNKNOWN`。系统不得把首次发现、观察、接收或重放时间伪装成发布时间。

显式的 `PUBLISHED: 7D` / `PUBLISHED: 30D` 读取仍保持严格：只有来源发布时间已知且落在窗口中的内容可显示；未知发布时间的匹配对象被排除，并报告排除数量。

## 已冻结语义

```text
URL 未携带 window
  -> latest_accepted_discovery（显式内部读取视角，不是发布时间窗口）
  -> 已接纳内容按最新 discovery 读取；known + PUBLISHED_AT UNKNOWN 均可显示

URL 携带 window=last_7_days 或 last_30_days
  -> 严格 PublishedWindow
  -> 只接受已知来源 published_at；unknown 排除并计数
```

`latest_accepted_discovery` 不增加可点击过滤器、保存视图、采集、补采或任何写操作。它只解决默认读取语义与显示边界。

## 范围

- `EvidenceQuery` 内部时间视角与 URL 缺省值；
- 现有 runtime/fallback Evidence Library read projection；
- `/corpus/evidence` 的时间视角/未知发布时间文字和页面局部状态样式；
- 合成 contracts、PostgreSQL proof、loopback/API 和页面渲染测试；
- 数据合同、页面规格、运行手册、LIDS 记录、索引与进度记录。

## 明确不做

- 不改插件、ingress、XHS 页面读取、真实 Package、数据库 migration、Topic/analysis、媒体、OCR/ASR；
- 不回填、修复、改写或重新接纳真实 Canary 数据；
- 不用 `first_seen`、`observed_at` 或任何接收时间填充 `published_at`；
- 不增加交互控件、采集能力、写入、远程封面、额外页面或视觉系统。

## 实施与验证

1. 将 URL 缺省值从一个隐式 30 天发布时间窗口改为命名的内部读取视角；验证：contracts 区分 default 与 explicit published windows。
2. 修改两条既有 read projection，使默认视角保留已接纳、发布时间未知的卡片；验证：runtime 与 legacy fallback 的隔离 PostgreSQL 合成测试。
3. 在页面将未知来源发布时间显式标出且不显示替代日期；验证：page render 与 loopback 合成测试。
4. 保留显式 7/30 天窗口的严格筛选与未知排除数；验证：API/projection 合成测试。
5. 完成 LIDS/产品/运行文档同步、governance 检查、Draft PR 和独立 Spec/Standards 审查；不合并、不关闭 Issue。

## 风险、停止与回退

- 风险：把默认“最新已接纳”误写成“最近发布”，或让未知时间默认为零/现在。防线：独立 `EvidenceTimeView`、否定测试和明确 UI 标签。
- 停止条件：若实现需要扩展真实数据、修改 ingress/schema、增加 UI 动作或推断发布时间，停止并升级新决策。
- 回退：该改变是 read-model/UI 语义；若验证失败，回退本事项提交即可，不修改已接纳 Package 或真实事实。
