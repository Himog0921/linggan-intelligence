# LOCAL-001C0-DISCOVERY-BOUNDARY-V1 · Discovery 与 Evidence Library 跨边界合同

> 状态: 权威当前
> 最后核对: 2026-08-25
> 适用范围: `LOCAL-001 / 001C-0` 的 Rust 合同、后续 001B 读投影与 001C-1 小红书 discovery ingress 的共同边界
> 事实来源: Mog 的 2026-08-25 明确确认、Issue #31、[LOCAL-001 活跃计划](../plans/active/local-001-local-product-evidence-library.md)、`AGENTS.md` 与 `domain-invariants.md`
> 冲突时以谁为准: 用户最新确认、`AGENTS.md`、真实运行/代码/合同、ACCEPTED 决定；本合同不授予真实平台访问或媒体取得

## 1. 这张卡固定的边界

本合同只解决下一张实现卡不能自由猜测的五件事：平台观察指令与本地库检索的区别、首批 discovery 能带回什么、部分结果如何诚实保留、搜索位置属于什么，以及封面候选何时才能显示。

它不定义数据库表、HTTP API、插件消息、页面运行时代码或真实接入。`xhs.discovery.visible-card.v1` 通过结构校验只说明“输入符合本卡的发现面形状”；它不表示插件已发送、服务端已接收、Evidence 已接纳、Source Object 已解析、Observation 已形成或页面已经可读。

## 2. 两类不能互相调用的 Query

| 合同对象 | 回答的问题 | 可以包含 | 明确不能包含或触发 |
|---|---|---|---|
| `AcquisitionSpec` | 要去平台观察什么 | 平台、搜索词、排序、目标单位与上限 | Evidence Library 的本地检索、页面显示、自动详情/评论/媒体深化 |
| `EvidenceQuery` | 要从 Linggan 已接纳材料中找什么 | 关键词、已接纳材料搜索范围、`WINDOW`、本地排序 | 平台、插件、采集命令、媒体/OCR/ASR 请求 |

两者是不同 Rust 类型，并分别采用 `deny_unknown_fields` 的输入形状。任何未来 UI 输入“搜索”只构造 `EvidenceQuery`；没有新的 `Acquisition Authorization`、Admission 与 Work Order，绝不能被解释为平台搜索。

### Evidence Library V1 检索语义

- 浏览单位是 `ContentItem`；若未来发生文本匹配，命中单位是 `EvidenceFragment`。本卡没有建立这两个持久化对象或读投影。
- `WINDOW` 只指 `ContentItem.published_at`。来源发布时间未知的内容不自动进入 7 天或 30 天窗口。
- `Latest Discovery` 只使用首次 Discovery 时间；它不是发布时间、最近观察时间或接收时间。
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
4. 该材料可以在未来接纳后作为发现面材料使用，但 Coverage 不自动给趋势、代表性、平台总量或“没有看到”的 Claim 资格；
5. 之后补采必须是新的授权/Attempt/Package，绝不能修改本次 discovery 包。

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

## 7. 合同测试与未证明边界

`crates/contracts/tests/discovery_boundary_contract.rs` 覆盖：两类 Query 的字段隔离、partial visible-card 保留、详情/评论/媒体字节/OCR/ASR 的拒绝、position 只能在 occurrence、remote cover 不能成为 display URL、来源发布时间未知不进入时间窗口。

它没有证明：插件已加载、localhost ingress 存在、任何真实平台访问、数据库写入、Evidence Acceptance、页面读投影、媒体副本、OCR/ASR、隐私处理、趋势或业务价值。
