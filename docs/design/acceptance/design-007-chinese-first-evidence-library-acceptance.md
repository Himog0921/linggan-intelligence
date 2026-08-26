# ACC-DESIGN-007 · Evidence Library 中文优先验收记录

> 状态: 一次性报告
> 最后核对: 2026-08-26
> 适用范围: Issue #65 的 `LIDS-LANG-001` 与 `/corpus/evidence` 文案本地化切片
> 事实来源: LIDS-LANG-001、DESIGN-007 change manifest、PAGE-EVIDENCE-001、真实代码与本次验证输出
> 冲突时以谁为准: 用户最新确认、真实运行/代码/合同；本报告不替代数据或业务验收

## 1. 验收对象

- Issue / Scope: Issue #65；`DESIGN-007 / LIDS-LANG-001`
- 页面: `/corpus/evidence`
- 页面任务: 检索、核验并追溯 Linggan 已接纳的 discovery 卡片；中文清楚说明它不会重搜平台、不会虚构详情或媒体。
- 关联规则: `LIDS-LANG-001`、`PAGE-EVIDENCE-001`、`LIDS-PRI-001`、`LIDS-PAT-001`。

## 2. 场景矩阵

| 场景 | 用户应理解的中文主表达 | 英文旁注 | 验证结果 |
|---|---|---|---|
| 默认已接纳发现 | 已接纳的发现卡片；当前显示最新已接纳的发现卡片，其中部分卡片的发布时间仍可能未知 | `ACCEPTED DISCOVERY`、`LATEST ACCEPTED DISCOVERY · PUBLISHED_AT UNKNOWN` | 通过：render test + loopback PostgreSQL proof |
| 发布时间未知 | 发布时间未知，未使用首次发现/观察/接收时间替代 | `PUBLISHED_AT UNKNOWN` | 通过：render test + strict window proof |
| 媒体尚未取得 | 媒体尚未采集，非远程封面、非封面不存在 | `MEDIA NOT ACQUIRED` | 通过：render test；未取得不显示远程封面 |
| 严格发布时间窗口 | 仅按来源可知发布时间过滤；未知发布时间对象被排除并计数 | `WINDOW / PUBLISHED_AT` | 通过：7D loopback API/页面同源 proof |
| 无卡片 / 未接通 | 当前无可展示材料或本地读投影未接通，不代表平台无内容 | `SOURCE INCOMPLETE`、`NOT CONNECTED` | 通过：loopback DOM 检查 |
| Discovery 停止原因 | 已达到配额、页面结束、风险控制、用户手动停止或原因未知；不把部分结果说成完整 | 原始 `stopped_reason` 代码 | 通过：focused render test；未知代码显示「未归类」 |
| 无效查询 / 读取暂不可用 | 两条路径各自显示准确中文主状态；未读取材料不冒充来源不完整、空库、已读取或已采集 | `LOCAL_QUERY_INVALID`、`READ_PROJECTION_UNAVAILABLE` | 通过：focused render test；不证明数据库、平台或真实 producer 可用 |
| 页面内剩余英文整句 | 结果表头、本机呈现／未读材料、默认视角均以中文独立说明 | `ACCEPTED DISCOVERY`、`LOCAL PRESENTATION`、`NO MATERIAL READ`、`LATEST ACCEPTED DISCOVERY · PUBLISHED_AT UNKNOWN` | 通过：focused render regression；不涉及共享 Shell |
| 英文技术注释层级 | 中文主表达独立可读，英文技术键保持可见但相对更小 | `.v7-tech-key` | 通过：页面局部样式断言；不涉及共享 Shell |
| 顶栏服务状态与时区 | 成功读投影显示「本机服务 / 已接纳发现材料」；时区显示「本机时区」 | `LOCAL HOST / ACCEPTED DISCOVERY`、`UTC+08` | 通过：focused render regression；不改状态、时间事实或共享 Shell |
| 未接通读模型顶栏 | 无本机读模型时中文独立显示「本机服务 / 读投影未接通」；不把英文整句当作状态主文案 | `LOCAL HOST / NO READ MODEL` | 通过：base DOM regression；不改 Shell、状态判定或读模型 |
| 嵌入式技术旁注层级 | Fact strap、空态指标与事实读数内的技术键，仍低于相邻中文标签；视图、筛选及区块标题的英文注释同样不得放大 | `.v7-tech-key`、`SYSTEM VIEWS`、`FILTER`、`RAW CONTENT` | 通过：focused CSS selector / typography regression；不改结构或数据 |

## 3. 验收边界

- 本项不改、也不证明真实采集、插件交付、平台访问、媒体取得、OCR/ASR、详情/评论、趋势或部署。
- 原始用户标题、创作者名和其它来源材料保持原样；本项不翻译或改写它们。
- 视觉截图仅用于本机检查，不提交为 Git 生成物，也不得包含真实原始 XHS 内容或 URL。
- 共享 Shell 的孤立英文不属于本事项：它会影响其它页面，须以独立 UI 事项确认；本次没有修改 `shell.rs` 或 `shell.css`。

## 4. 分层结论

| 完成层 | 状态 | 证据 | 限制 |
|---|---|---|---|
| 设计规格一致 | 通过 | `LIDS-LANG-001`、DESIGN-007、LIDS/项目索引和迁移记录 | 不证明其它页面已迁移 |
| 页面实现 | 通过 | Rust template / projection renderer / page CSS；中文主表达与技术键组合均有 focused test | 不改行为 |
| 自动检查 | 通过 | `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo test --workspace`、governance check | 不证明真实平台或用户验收 |
| DOM / 数据边界 | 通过 | 独立 loopback `127.0.0.1:3100` DOM 读取；隔离 PostgreSQL proof 的 9+5 数据测试和 6 条 API/页面测试 | 本机 loopback 无真实平台材料，不输出或保存真实 XHS 内容 |
| 视觉 / Mog 验收 | 待用户确认 | 本项未改几何、Token、页面结构或按钮状态；CSS 仅降低英文技术键的语义角色 | 未重新提交真实材料截图；用户仍须在合并后确认整体阅读感受 |
| 真实链路/回执 | 不适用 | 本事项为文案与展示治理切片 | 不证明采集或媒体 |
| Mog / 业务验收 | 待用户确认 | 本机页面视觉检查 | 用户决定最终可读性 |

## 5. 交付质量门与后续整合

- 最终质量门实际执行 `cargo clippy --workspace --all-targets --locked -- -D warnings`。此前仅记录的非 `--all-targets` 命令不能代表该更严格门已通过；本次以最小测试 helper 拆分 loopback 证明函数，保留全部原有状态与媒体未采集断言。
- 最终 CSS / 文案补正：base 无读模型顶栏以「本机服务 / 读投影未接通」独立表达，`LOCAL HOST / NO READ MODEL` 收回相邻 `.v7-tech-key`；同时为会被 descendant label selector 覆盖的 Fact strap、空态指标与事实读数技术键增加更高特异性的页面局部规则，并把 `SYSTEM VIEWS`、`FILTER`、`RAW CONTENT` 的英文注释降为相对中文的 `.82em`。本补正不触及共享 Shell、API、数据库、查询、采集或运行时。
- 已知后续依赖为 Draft PR #69 的共享 shell 中文化。#67 只触及 Evidence Library 的页面局部 renderer/CSS，#69 才拥有共享 `shell.rs` / `shell.css`；两者运行时文件边界不重叠，不能在本事项内合并。
- 集成顺序固定为：先合并 #67；随后将 #69 rebase 到新的 `main`，由 integration owner 处理可能重叠的 focused tests / 文档行，再对 #69 的新 exact head 重新审查和整合。任何旧 head 的审查结论均不得复用。
