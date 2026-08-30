# WORK-RESOURCE-READ-001 · Intelligence 共享作品资源读取合同

> 状态: 草案
> 最后核对: 2026-08-31
> 适用范围: Intelligence 中需要展示作品封面、标题、作者、发布时间、互动、材料状态与来源血缘的页面
> 事实来源: Issue #110、Media V2、当前 Material Projection、Browser Producer detail collector、additive migration `0025_work_resource_read.sql`
> 冲突时以谁为准: 用户最新确认、不可变 Capture Package、类型化材料事实、Media V2、当前代码与数据库约束

## 1. 唯一公共入口

Intelligence 页面只能消费共享 `Work Resource Read` Interface：

- 列表：`GET /api/local/work-resources`
- 单品：`GET /api/local/work-resources/{publicRef}`
- 授权评论通道：`GET /api/local/work-resources/{publicRef}/comments`
- Rust Interface：`read_work_resources`、`read_work_resource`、`work_resource_schema_is_ready`

Evidence Library 是首个消费者，不是接口 owner。后续选题、创作者、观察、研究或其他页面不得另写 SQL、另读 Package JSON、另建封面/作者/时间拼接规则，亦不得以页面私有 endpoint 形成第二份事实。

内部的 typed Material Projection、Media V2 slot/origin/blob/materialization、评论与作者版本化读取可以继续拆模块；它们对页面只通过这个小 Interface 暴露。显式 `/api/local/evidence-library/legacy` 仅为旧发现卡兼容读取，不是共享资源入口，不得成为新页面 fallback。

## 2. 一个作品资源的最小合同

```text
WorkResource
├─ identity: platform / contentExternalId / publicRef
├─ display
│  ├─ title + state
│  ├─ creatorDisplayName + state
│  ├─ publishedAt + publishedAtState
│  ├─ publishedAtSourceText / sourceField / sourceKind / precision
│  ├─ publishedAtReferenceObservedAt / parserVersion
│  └─ engagement + per-field state
├─ collectionContext
│  ├─ relationshipState
│  ├─ targetRef / targetKind / targetDisplayName + state
│  ├─ authorIdentityMatchState
│  └─ workOrderRef
├─ preview: controlled local asset handle or explicit state; never a remote source URL
├─ laneSummaries[] / summary / matchedFields
└─ detail inspector: field sources, Target/WorkOrder/Task/Attempt/Package/Receipt, media and limitations
```

列表与详情必须使用同一字段语义。三种布局只改变排版，不改变查询、字段资格、状态或血缘。

## 3. 作者与监控目标不是同一字段

来自创作者主页监控的作品可以证明“这条作品是在该监控目标表面被观察到”，但只有作品详情中的平台作者 ID 与目标稳定 ID 一致，才能证明“该监控目标就是作品作者”。因此：

| 情况 | `relationshipState` | `authorIdentityMatchState` | UI |
|---|---|---|---|
| 创作者主页发现，详情尚无作者 ID | `OBSERVED_ON_TARGET_SURFACE` | `NOT_VERIFIED` | 显示“监控目标：木可可同学”；作品作者仍是“当前未知” |
| 详情作者 ID 与目标稳定 ID 相等 | `OBSERVED_ON_TARGET_SURFACE` | `MATCHED` | 分别显示作者和监控目标，并写明平台 ID 已证明一致 |
| 详情作者 ID 不同 | `OBSERVED_ON_TARGET_SURFACE` | `MISMATCH` | 保留两者并突出不一致，禁止覆盖作者 |
| 关键词发现 | `DISCOVERED_FOR_TARGET` | `NOT_APPLICABLE` | 显示关键词目标，不把关键词当作者 |

血缘优先使用 `Task → LeaseTask → Lease → WorkOrder → Target`；旧的手动 profile discovery 没有 lease 关系时，只允许用 TaskSpec 的稳定 `authorExternalId` 与同平台 creator target 精确匹配。显示名只作 UI 文案，不参与身份匹配。

## 4. 发布时间资格

Browser Producer 的详情解析先返回一个有来源资格的时间断言：

- `publishedAt`：归一化毫秒值；
- `publishedAtText`：来源原值文本；
- `publishedAtSourceField`：实际命中的 payload 字段，如 `publishTime`；
- `publishedAtSourceKind`：`platform_epoch | visible_text | unknown`；
- `publishedAtPrecision`：`millisecond | second | minute | day | relative | unknown`；
- `publishedAtReferenceObservedAt`：相对文本的观察参照时点；
- `publishedAtParserVersion`：当前为 `xhs-detail-time-v2`。

服务端只有在 `sourceKind=platform_epoch`、精度为秒/毫秒、字段名命中已支持的详情字段清单，且 parser version 精确为 `xhs-detail-time-v2` 时，才把值写入 `published_at`。可见的“3 小时前”“4 月 17 日”仍保留为 `SOURCE_TEXT_ONLY`；即使插件为了当前页面体验算出了一个毫秒值，服务端也不得把该派生值晋升成历史精确发布时间。

迁移只新增列，不回写、覆盖或重新解释旧 Package。旧行没有精确来源时继续是 `UNKNOWN` 或 `SOURCE_TEXT_ONLY`，不能用 `observedAt/acceptedAt` 代替发布时间。

## 5. Media V2 继承

封面与正文媒体继续由 Media V2 拥有：slot → source observation/generation → candidate assertion → download attempt → blob → materialization/derivative/disposition。Work Resource Read 只返回当前有资格显示的同源受控 `localAssetUrl` 或明确状态；列表和详情合同都不暴露原始 CDN URI。它不新建 Asset 表、不复制远程 URI、不把 discovery URL 当永久资源，也不允许页面绕过处置读取字节。

## 6. 页面与验证边界

- `layout=research|table|cover` 是本地展示状态；`view=` 仍是材料状态筛选，两者不得复用。
- 切换布局不重新请求 API，不修改数据，不改变当前选择与 Inspector。
- `Research` 展示完整 lane 带；`Table` 用于横向比较；`Cover` 用于视觉扫描。
- 真实签名详情字段探针只允许一条、只读发布时间字段名/类型/值，不保存正文或媒体；未获当次外部请求确认前保持未执行。
- 自动测试或 PostgreSQL 合成 proof 不等于真实 XHS、真实浏览器、部署或 Mog 业务验收。
