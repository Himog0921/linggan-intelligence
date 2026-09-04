# OBSERVATION-TARGET-DOSSIER-UI-001 · UI Change Manifest

> 状态: 草案
> 最后核对: 2026-09-05
> 适用范围: `/collection/targets` creator/keyword 目录、creator 档案工作区、生命周期散点与“建立档案”真实行动
> 事实来源: Mog 最新确认、Issue #158 与 Claim、PAGE-COLLECTION-001、采集监控产品规则、LIDS v7、当前代码和隔离证明
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实代码/数据库/回执、Issue #158 Claim；本清单不授予共享运行或外部平台权限

## 1. 事项与用户结果

- Issue / SCOPE: `#158 / OBSERVATION-TARGET-DOSSIER-UI-001`。
- Agent / worktree: `codex/observation-target-dossier-ui-001`，由 exact `origin/main@bfe5d7e623b70e314cc6697269e291c3d5f5b3ef` 建立。
- 目标: 观察目标不再是一张工程控制表，而是创作者作品档案的日常入口。
- 用户可见结果: 首层一眼判断观察谁、档案掌握多少、巡查是否有效、上次/下次时间、最近变化和下一步；打开 creator 后直接看作品生命周期分布与档案缺口。
- 明确非目标: Corpus 正文/评论/媒体复制、监控价值/机会评分、AI Creator Dossier 研究报告、手机与 1280/390 适配、schema/migration、共享数据库、`:3000`、插件、真实平台、部署与 merge。

## 2. 读取回执与设计方向

| 来源 | 本次采用 | 本次不继承 |
|---|---|---|
| Mog 最新确认 | 创作者档案、前 200 篇链接上限、渐进详情、巡查持续回流、散点分布 | 工程字段优先或空白大图 |
| 旧内容工作台 | 账号生命周期曲线的任务意义、深档案的分层完成观念 | 旧技术栈、数据库、监控价值、旧 UI 外观 |
| V4 synthetic reference | 内容数据库式密度、稳定列、白场和行级主动作 | 模拟数字、第二套壳层、健康百分比 |
| PAGE-COLLECTION-001 | 五个 Collection 子面、目标抽屉与 Corpus 唯一材料入口 | Evidence tab、技术回执作为默认内容 |
| LIDS v7 | 白色连续阅读场、硬结构线、单一 Signal、40px 控件、中文独立成立、全局 focus | 新颜色、渐变、玻璃、字符图标、另一套 token |
| 当前事实合同 | stable Work 去重、Work Current、UNKNOWN 与 0 分开、Request→Authorization→Admission→WorkOrder→Lease | 根据点击、状态名或示例数字猜执行成功 |

本次使用 `design` skill 将视觉方向锁为“内容数据库 / 编辑部桌面”：白底黑灰结构、真实头像与散点为内容，橙色只标记主要行动、选中和最近新增。信息顺序冻结为 `身份 → 档案 → 巡查 → 上/下次 → 变化 → 下一步`；工作区顺序为 `身份 → 最近变化 → 生命周期 → 档案缺口`。

## 3. 变更分类与风险

- 分类: 展示 + 交互 + 状态语义 + 权限行动的混合变更。
- 最高风险: “建立档案”不能只换按钮文案；必须有可审计、有限、可恢复的真实行动。
- LIDS 强度 / Pattern: Collection L1 Operations；creator drawer 为受限 L2；主 Pattern 为 Collection Control + dossier reading surface。
- Data Truth: 档案 Coverage、巡查 Operation、时间 Freshness 和动作 Admission 分开。`UNKNOWN` 不是 `0`，`PARTIAL` 不是失败，WorkOrder/Task 完成不是档案完整。
- DECISION_REQUIRED: 无。用户已明确 200 上限、渐进补齐、巡查回流、creator/keyword 分离与桌面支持边界。

## 4. 首层目录合同

### Creator

固定列为：

`编号｜创作者｜平台｜分组｜档案状态｜作品目录｜详情进度｜巡查状态｜最近变化｜上次巡查｜下次巡查｜操作`

- 普通数据格单行；昵称、账号、分组超长省略；数字和时间使用 tabular numerals。
- 档案状态只来自去重目录、详情、隔离问题和活动 deep-archive Lease；不从按钮点击或 lifecycle 名称猜在途。
- 上次巡查只使用 `last_patrol_succeeded_at`；`last_patrol_dispatched_at` 不冒充成功结果。
- 详情显示 `已取得 / 已发现 · 缺口`，不显示总健康分或假精确百分比。
- 默认不显示复选框或批量管理区；每行最多一个主要动作。

### Keyword

固定列为：

`编号｜关键词｜平台｜分组｜巡查状态｜最近命中｜数据更新｜上次巡查｜下次巡查｜操作`

关键词不拥有 creator 档案、详情进度或生命周期散点；对象工作区只保留 `概览｜巡查`。当前没有目标级差分读模型时显示“尚未取得”，不以 `0` 冒充已确认无变化。

### 主动作

| durable 状态 | 主要动作 | 后果 |
|---|---|---|
| 未建立且无在途 | 建立档案 | 发起 200 篇上限的受控目录建档 |
| 有 live deep-archive Lease | 查看进度 | 打开档案 Tab，不重复提交 |
| 目录存在、详情有缺口 | 继续完善 | 复用同一受控入口，生成版本化渐进计划 |
| 有 quarantined 记录 | 查看档案问题 | 打开真实问题/缺口，不声称已修复 |
| 档案可用、巡查未开启 | 开启巡查 | 打开既有版本化巡查规则 modal |
| 档案与巡查正常 | 查看档案 | 打开 creator 概览 |
| keyword 未设置/暂停/运行 | 设置巡查 / 恢复巡查 / 查看观察 | 分别进入真实规则或关键词概览 |

## 5. 建档与持续更新合同

1. 可见“建立档案”要求 exact purpose 的有效 creator `deep_archive` Authorization 允许至少 200 Works；较小授权明确拒绝，不静默缩小承诺。
2. Request、Admission、根 WorkOrder、`progressiveArchive.version=1` marker 与 Lease 在同一事务形成；根 `maximumQuota=200`。
3. 第一阶段扫描主页当前可见作品链接，上限 200；结果按 stable Work 去重，200 不是平台总量或完成率。
4. worker 只读取已经接受且未隔离的 target-scoped directory facts；每个 target 每轮冻结最多 3 篇缺详情 Work。
5. 子 WorkOrder 沿用根 Authorization 与 purpose；每篇固定详情、最多 30 条评论、回复展开最多 2 次、媒体及已授权 OCR/ASR。
6. target row lock、live material-scope 排除、子 WorkOrder 与 Lease 在同一事务，避免并发 tick 重复深化。
7. 没有 marker 的历史 WorkOrder 不会被自动扩大权限。授权过期、目标停止、产能不足或 live batch 存在时，不冒充已推进。
8. 巡查新作品只有经精确 `Package → LeaseTask → Lease → WorkOrder → Target` 链、有效回执且目标仍允许持续观察，才进入后续自动补齐。

## 6. Creator 档案工作区

- Tabs: `概览｜档案｜巡查`；旧 `baseline` URL 仅作兼容别名，Evidence/trace 退回概览。
- 概览: 最近变化在前，生命周期为主视觉，档案缺口在后。
- 档案: 作品目录、详情进度、当前可分析、问题/深层材料分开，不合成总分。
- 巡查: 当前启停、上次成功、下次时间、最近结果和真实规则入口；没有差分读模型时不造事件或零值。
- 正文、评论、外部评论身份和媒体详情仍只在 Corpus；选中散点通过稳定 Work public ref 深链进入。

生命周期规则：

- 默认全部周期 + 点赞；可切近 90 天、点赞/评论/收藏/转发。
- 横轴为合格发布时间；纵轴压缩互动量差距，但悬浮/选中与 ARIA 使用原值。
- 空心点 = target-scoped 主页目录关联、详情作者尚未确认；实心点 = 作者 ID 精确匹配；橙色外圈 = 最近一次已接纳巡查首次关联。
- 作者冲突、发布时间不合格、所选指标 UNKNOWN 不入图；KNOWN `0` 可入图。
- 一个 stable Work 一个点；后续巡查的最新 KNOWN 互动观察更新原点，不制造重复点。
- 删除复合指标、分位数、滚动中位线、算法版本、扫描工程回执与监控价值。

## 7. 影响、文件与回退

- Rust read/action: `collection_target.rs`、`archive_completeness.rs`、`creator_lifecycle.rs`、`acquisition_chain.rs`、`work_order_lease.rs`、worker tick 与 local-web routes。
- UI: `collection_targets_view.rs`、`target_drawer.rs`、`target_drawer.css`、lifecycle API DTO。
- 文档: 产品规则、PAGE-COLLECTION-001、本 manifest、验收记录、current-state、progress 与 LIDS migration log。
- 无 migration、无新表、无新 token/CMP/framework/font/icon library。
- 回退时必须同时撤回 UI 承诺、progressive marker/tick 和两级 lifecycle projection；不得只隐藏按钮而留下自动深化，或只删除 worker 而保留“自动补齐”文案。

## 8. 验收与未证明边界

| 层级 | 方法 | 交付要求 | 未证明边界 |
|---|---|---|---|
| 任务可用 | SSR/source tests + 1440 browser | 字段可扫读、上/下次独立、动作唯一、creator/keyword 分开 | Mog 实际使用验收 |
| 状态诚实 | unit + isolated PostgreSQL | stable Work 去重、live Lease、两级关联、UNKNOWN/0、成功时间 | 真实平台总量 |
| 视觉一致 | 1440 CSS px desktop | 无文档级横向溢出，散点为唯一主视觉，LIDS/focus 成立 | 1280/390/手机 |
| 真实后果 | isolated PostgreSQL | 200 授权边界、marker、3 篇 batch、原授权、真实 Lease | shared DB/runtime、插件与平台执行 |

本 Package 只进行一次 exact-head 产品/数据/代码联合审查，汇总后最多一次批量修正，再做最终验证；不以连续 reviewer 循环延长范围。
