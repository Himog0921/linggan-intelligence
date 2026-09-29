# CREATOR-GAP-REQUEST-20260929 · 补采请求与失败回执变更清单

> 状态: 草案
> 最后核对: 2026-09-29
> 适用范围: `/collection/targets` 创作者目标的“补采缺口”提交与反馈
> 事实来源: Mog 对补采范围的当次确认、南瓜哒哒只读账本、当前 Rust/SQL 合同、`PAGE-COLLECTION-001`、LIDS 中文语言规则
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实代码/数据库/测试、已接受领域决定

> 实施状态: 候选实现；仅独立分支，未集成或运行验收。

## 用户结果与范围

Mog 确认“补采缺口”指补采未取得完整笔记内容的作品，包括详情、标题、正文、媒体、评论等。本候选先修复已经证实的请求失败：同一目录作品有多条 Domain usage 时，候选查询不能把一篇作品重复冻结。已入选作品仍走详情、评论、回复、媒体及 OCR/ASR 的既有受控合同；结果以实际回执与材料为准。

目前“哪些作品入选”的既有判据仍是**缺合格详情**。有合格详情但单独欠标题、正文、媒体或评论的作品，尚不能由当前按钮识别为候选；这与 Mog 确认的完整补采范围有缺口。`UNKNOWN` 或零条评论、零个媒体对象不能自动推断平台存在可采内容。此边界必须在逐项缺口判据与授权范围确定后再扩展，不把这次去重证明写成完整语义交付。

## 表面、状态、依赖与验证

| 表面 / 状态 | 当前候选后果 | 依赖与验证 |
|---|---|---|
| `/collection/targets` 列表与抽屉的“补采缺口” | 仍为同一入口；请求按当前目录唯一 Work 冻结，至多 200 篇 | `archive_ledger` 的目录与合格详情判据、Domain usage、准入与授权；隔离 PostgreSQL 真实工单回归通过 |
| 已提交且准入 | 显示“已排入缺口补采”，说明冻结范围和待执行；不声称已取得材料 | 原 Request → WorkOrder → Lease/Task → Package/Receipt 链；UI 单测通过，常驻 runtime 未验证 |
| 已有在途工作、无待补详情、生命周期不允许、缺领域 | 分别显示原因，不把数据库/合同失败冒充重复任务 | handler 错误分类单测通过 |
| SQL、schema、未知就绪原因 | 显示“提交结果待核对”及服务端日志诊断编号；先刷新目标与工单。连接可在提交后、回执前断开，因此不声称一定没有新工单 | UUID 格式负例单测通过；实际服务日志/浏览器未验证 |
| 根建档已完成、缺口暂不能排入、原建档用途冲突 | 分别说清**本轮根基线**已收口、当前不可排和冻结用途不一致；不冒充整个当前目录所有材料齐全或服务端故障 | 错误分类与反馈文案单测通过；真实浏览器未验证 |
| 有详情但其它内容缺失 | 本候选仍可能不入选，标记 `DECISION_REQUIRED` | 需区分“未观察”“观察失败”“平台确实没有”及评论/媒体部分覆盖；不能从当前 51/60 详情覆盖推断全内容完成 |

变更类别为状态/语义与权限/行动的混合修复。LIDS 层级是 Collection L1 Operations、目标抽屉受限 L2；不新增 Token、组件、布局或第二采集入口。共享依赖为 `crates/evidence/src/acquisition_chain.rs`、`archive_ledger`、`qualified_detail`、`apps/api/src/local_web.rs` 与反馈文案。没有改变授权上限、平台访问或材料接纳规则。

## 证明与未证明

- VERIFIED：共享开发库只读复算南瓜哒哒 9 个缺合格详情 Work，在原 join 下展开为 23 行；5 个 Work 各有 3–5 条 Domain usage。该 helper 进入 `ensure_material_targets_belong_to_platform`，唯一内容行数小于含重复的候选长度，返回 `InvalidMaterialTargets` 并回滚；不经过公开入口的 `validate_material_targets`。隔离 PostgreSQL 测试建立重复 usage 后，候选冻结每篇一次，保留 30 条评论、两层回复、媒体与 OCR/ASR 参数。
- VERIFIED：两个 API/UI 定向单测覆盖数据库错误与已知业务状态的区分、诊断编号格式。
- VERIFIED：对该目标 60 篇已发现作品的共享库只读汇总，51 篇有详情；其中标题 `UNKNOWN` 为 0、正文 `UNKNOWN` 为 40，媒体来源缺行为 0。两篇当前评论数低于详情里已知计数与 30 条上限的较小值，但最近评论包都标 `partial / comment_area_end`；计数差不能直接断言仍有可见评论可采。该快照不证明每一媒体字节已取得，也不把正文 `UNKNOWN` 自动归因为失败。
- NOT VERIFIED：`origin/main`、常驻 `:3000`、真实浏览器点击、共享库新增 WorkOrder、插件实际采集、材料回执与 Mog 业务验收。
- `DECISION_REQUIRED`：逐项内容缺口的资格与停止条件，尤其标题/正文为空、评论/媒体为零时如何区分来源不存在和采集遗漏；该决定影响候选选择与可重复采集范围。

文件边界：候选改 `acquisition_chain.rs`、`observation_target_dossier_postgres.rs`、`local_web.rs`、`collection_targets_view.rs`、`creator_lifecycle_tests.rs`。不修改旧 Package、Receipt、材料或共享数据库，不触发真实补采。本清单与当月进度同步；合并、运行刷新及真实采集属于后续独立验收层。
