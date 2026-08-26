# LOCAL-001C0-DISCOVERY-BOUNDARY-V1 · Discovery 与 Evidence Library 跨边界合同

> 状态: 权威当前
> 最后核对: 2026-08-25
> 适用范围: `LOCAL-001 / 001C-0` 的 Rust 合同、后续 001B 读投影与 001C-1 小红书 discovery ingress 的共同边界
> 事实来源: Mog 的 2026-08-25 明确确认、Issue #31、[LOCAL-001 活跃计划](../plans/active/local-001-local-product-evidence-library.md)、`AGENTS.md` 与 `domain-invariants.md`
> 冲突时以谁为准: 用户最新确认、`AGENTS.md`、真实运行/代码/合同、ACCEPTED 决定；本合同不授予真实平台访问或媒体取得

## 1. 这张卡固定的边界

本合同只解决下一张实现卡不能自由猜测的五件事：平台观察指令与本地库检索的区别、首批 discovery 能带回什么、部分结果如何诚实保留、搜索位置属于什么，以及封面候选何时才能显示。

001C-0 本身不定义数据库表、HTTP API、插件消息、页面运行时代码或真实接入。未来唯一的运行时 producer 名称为 **Linggan-owned browser producer package（Linggan Plugin）**；它将与 Linggan local host/API 组成同一系统。旧内容工作台及其插件只可作为历史能力与风险参考，不是 producer、endpoint、fallback、数据源或验证中转。

`xhs.discovery.visible-card.v1` 通过结构校验只说明“输入符合本卡的发现面形状”；它不表示 Linggan Plugin 已发送、服务端已接收、Evidence 已接纳、Source Object 已解析、Observation 已形成或页面已经可读。

## 2. 两类不能互相调用的 Query

| 合同对象 | 回答的问题 | 可以包含 | 明确不能包含或触发 |
|---|---|---|---|
| `AcquisitionSpec` | 要去平台观察什么 | 平台、搜索词、排序、目标单位与上限 | Evidence Library 的本地检索、页面显示、自动详情/评论/媒体深化 |
| `EvidenceQuery` | 要从 Linggan 已接纳材料中找什么 | 关键词、已接纳材料搜索范围、`WINDOW`、本地排序 | 平台、插件、采集命令、媒体/OCR/ASR 请求 |

两者是不同 Rust 类型，并分别采用 `deny_unknown_fields` 的输入形状。任何未来 UI 输入“搜索”只构造 `EvidenceQuery`；没有新的 `Acquisition Authorization`、Admission、Work Order 与 Linggan Plugin 执行，绝不能被解释为平台搜索。

### Evidence Library V1 检索语义

- 浏览单位是 `ContentItem`；若未来发生文本匹配，命中单位是 `EvidenceFragment`。本卡没有建立这两个持久化对象或读投影。
- URL 未携带 `window` 时，Evidence Library 使用显式内部时间视角 `latest_accepted_discovery`：仅读取已接纳的 discovery 卡片，已知和未知 `published_at` 都可显示；未知卡必须标为 `PUBLISHED_AT UNKNOWN`，且不得用 `first_discovered_at`、`observed_at`、接收/重放时间或任何其他时间代替来源发布时间。它不是 `WINDOW`，也不构成“最近发布”的主张。
- 只有 URL 显式指定 `window=last_7_days` 或 `window=last_30_days` 时，`WINDOW` 才只指 `ContentItem.published_at`，并以 read projection 求值时的 Linggan PostgreSQL `scope_001_now()` 为唯一时间参照。显式窗口只保留 `published_at` 落在闭区间 `[scope_001_now() - window, scope_001_now()]` 的已知值；未来发布时间不被称为最近，也不进入窗口，但其已接纳发现记录仍保留。`published_at = UNKNOWN` 必须排除，并报告排除数。排除数按当前 `EvidenceQuery` 匹配的 `ContentItem` 身份统计：只有该身份没有任何同一查询下、已知且位于当前窗口的 occurrence 时才计入。若同一 ContentItem 同时有窗口内已知发布时间和未知发布时间 occurrence，它显示一次且不增加排除数。文本不匹配的本地对象不能被计入页面的未知发布时间排除数。API 的 `window` 使用实际读取的 `last_7_days`/`last_30_days` 值；页面必须将同一值显示为 `7D`/`30D`，不能推定或硬编码一个发布窗口为默认值。
- `Latest Discovery` 只使用同一稳定内容身份首次被 Linggan **接受**的时间；它不是发布时间、页面实际观察时间或最近接收/重放时间。
- 标题、作者名、正文、评论、OCR、ASR 的实际召回和排序实现留给具有已接纳材料的 001B；缺少的材料必须呈现为 `NOT_ACQUIRED`/`UNKNOWN`，不能被当作“不匹配”。

## 3. 001C-1 首批发现面

第一张真实 Canary 后续必须保持以下冻结值，任何扩大都需新授权：

```text
platform        = xhs
query           = ADHD
sort            = comprehensive
target basis    = maximum_quota
target unit     = visible_search_card
maximum quota   = 20
```

`maximum_quota` 是最多观察 20 个实际可见位置，不是一个预先知道的 20 个对象集合。因此 `20 - actual visible card count` 不能创造“缺失笔记”、平台总量或完成百分比。

每张 `DiscoveryCard` 只允许保存页面实际可见卡片的最小信息：稳定平台内容标识、实际可见标题/创作者展示名、来源发表时间原文（若页面真有）、可见封面候选，以及一个 `DiscoveryOccurrence`。

`DiscoveryOccurrence` 必须保存：查询、排序、页面实际观察时间和结果位置。位置不是 ContentItem、Content Observation 或“这篇笔记当前排名”的字段；同一内容从不同查询或排序被再次发现时，保留不同 occurrence。

下列内容在 discovery 合同中一律拒绝：详情正文、评论或回复、作者历史、媒体字节、OCR 文本、ASR 转录、嵌入、Topic、Signal、Insight 或 Claim。

## 4. 部分结果与 Coverage

页面/插件因为风险控制、页面结束、人工停止或未知原因只实际看见一部分卡片时：

1. 每张实际可见卡片仍有独立价值，不能因总目标 20 未达到而丢弃；
2. `DiscoveryCoverage.visibleCards` 必须等于实际交付卡片数；
3. 停止原因必须保留；`unknown` 仍是未知而不是零或“无更多结果”；
4. `quota_reached` 只能在实际可见卡片数等于本次 `maximumQuota` 时使用；未达到配额的部分结果必须保留一个如 `risk_control`、`surface_ended`、`manual_stop` 或 `unknown` 的真实停止原因，不能把 `20 - visibleCards` 制造成缺失对象；
5. 该材料可以在未来接纳后作为发现面材料使用，但 Coverage 不自动给趋势、代表性、平台总量或“没有看到”的 Claim 资格；
6. 之后补采必须是新的授权/Attempt/Package，绝不能修改本次 discovery 包。

## 5. 封面、媒体与页面显示

搜索卡片上的外部封面地址是 `MediaCandidate`：它只记录某个卡片/slot 在特定观察中显示过的来源线索。

```text
MediaCandidate / observed external URI
        ≠ page display URL
        ≠ MediaBlob
        ≠ local replica
        ≠ downloaded or verified media
```

Evidence Library 的任何 `img` 或背景图在未来只能使用 Linggan 本地媒体端点；该端点的前提是独立 001C-2 取得并验证了对应 `MediaBlob`，且存在被允许读取的本地副本。未完成时页面必须明确显示真实状态，例如：

| 页面状态 | 可以表达 | 不能表达 |
|---|---|---|
| `MEDIA_NOT_ACQUIRED` | 只观察到候选，尚未取得本地字节 | 空白图片、远程 CDN 回退或“封面不存在” |
| `MEDIA_ACQUISITION_FAILED` | 已尝试但未取得；仍保留原因与 slot Coverage | 已删除、不存在或其他 slot 失败 |
| `MEDIA_RESTRICTED` | 已有材料但当前用途不可预览 | 没有媒体或访问已永久丢失 |
| `MEDIA_UNAVAILABLE` | 合同明确的本地副本当前不可读取 | 用原平台 URL 临时回退 |

本卡不实现以上端点、下载、存储、缩略图、哈希、MIME、保留期、删除传播或访问策略；这些属于 001C-2 的独立媒体生命周期合同。

## 6. Evidence Library 按钮合同（未来接通时）

| 按钮/控件 | 当前 001A 页面 | 已冻结的未来含义 | 禁止行为 |
|---|---|---|---|
| 搜索 | 禁用 | 只提交或恢复本地 `EvidenceQuery` | 触发平台/插件搜索、补采或任何写入 |
| 复制查询 | 禁用 | 只复制本地 URL/查询状态 | 复制外部平台 URL、创建任务 |
| 保存当前视图 | 禁用，`DEFINITION_PENDING` | 未决定，不实施 | 把它偷偷做成监控/采集计划 |
| 发起研究 | 禁用，`DEFINITION_PENDING` | 未决定，不实施 | 创建 Research/Claim/Agent 行动 |
| 补采 | 禁用，`SELECTION_AND_AUTHORIZATION_REQUIRED` | 将来须先选择明确的缺口与新授权 | 把一个泛化按钮解释为详情、评论、指标或媒体的一键采集 |

本合同不改变 `PAGE-EVIDENCE-001` 目前“没有可用动作”的运行时事实；它只是为下一张受控读投影和真实 discovery 卡定义不可越过的边界。

## 7. Issue #34 的 localhost 接入绑定

Issue #34 不改变 discovery payload 合同；它只把当前 versioned payload 绑定到 Linggan 自有的受控 loopback 接口，供未来 Linggan Plugin 健康检查和提交使用。这个接口存在不证明插件已安装、页面已被真实观察或平台访问发生过。

```text
GET  http://localhost:3000/health
POST http://localhost:3000/api/local/discovery-packages
GET  http://localhost:3000/api/local/evidence-library
GET  http://localhost:3000/corpus/evidence
```

| 接口/情况 | 允许的结果 | 禁止解释 |
|---|---|---|
| `GET /health`，未配置 Linggan DB | `SOURCE_INCOMPLETE` / `NOT_CONNECTED` | localhost 已接纳任何 Package 或平台可访问 |
| `GET /health`，已配置 Linggan DB | `LOCAL_DISCOVERY_READ_PROJECTION` / `DISCOVERY_ONLY` | 真实 discovery、详情、评论、媒体或趋势已完成 |
| `POST /api/local/discovery-packages`，合格新 payload | `200`，`admission=accepted`，有 delivery/package/receipt 引用、实际 `visibleCards` 和停止原因 | package 完整、平台只有 N 条或内容详情已取得 |
| `POST`，字节完全相同的已接纳 payload | `200`，`admission=replay`，复用原 package/receipt 并有新 delivery 引用 | 覆盖、更新或重新解释旧 package |
| `POST`，不合格 payload | `422 discovery_contract_invalid` | 已接纳任何卡片或 Coverage |
| `POST`，未配置/不可用 DB | `503 ingress_not_connected` 或 `503 ingress_not_committed` | 失败等于无可用历史材料或应重试平台 |
| `GET /api/local/evidence-library` / 页面 Search | 只读已接纳数据；省略 `window` 时为 `latest_accepted_discovery`，显式 `window=last_7_days|last_30_days` 才按来源发布时间严格过滤；标题/创作者名 V1 文本匹配 | 触发 `AcquisitionSpec`、平台搜索、补采或详情文本召回 |

Ingress body 是 UTF-8 JSON 的 `xhs.discovery.visible-card.v1`；不接受拼接的插件命令、旧工作台 envelope 或任意“额外字段”。页面响应和 JSON 读取投影不返回外部封面 URL；未有本地副本前只返回/展示 `MEDIA_NOT_ACQUIRED`。

## 8. 合同测试与未证明边界

`crates/contracts/tests/discovery_boundary_contract.rs` 覆盖：两类 Query 的字段隔离、partial visible-card 保留、非空稳定内容身份和有效 observation time、详情/评论/媒体字节/OCR/ASR 的拒绝、position 只能在 occurrence 且同包唯一、remote cover 不能成为 display URL，以及来源发布时间缺失在 Discovery 合同内保持 unknown。

它没有证明：插件已加载、localhost ingress 存在、任何真实平台访问、数据库写入、Evidence Acceptance、页面读投影或 `WINDOW` 结果过滤、媒体副本、OCR/ASR、隐私处理、趋势或业务价值。
