# COMMENT-STUDY-P2-CLOSEOUT-001 · UI 变更清单

> 状态: 草案
> 最后核对: 2026-09-30
> 适用范围: `/corpus/comments` 的研究选择、目标详情、研究结果与运行汇总
> 事实来源: 用户确认的 P2 交付范围、COMMENT-STUDY-PRODUCTIZATION-001、当前 API/数据合同及 2026-09-29 Run `4685462d-96e3-40c5-8496-e4805421414e`
> 冲突时以谁为准: 用户最新确认、`AGENTS.md`、当前数据库事实、当前代码与合同

## 1. 事项

- Issue / SCOPE: COMMENT-STUDY-P2-CLOSEOUT-001；不替代已合并的 PR #338/#348。
- 本轮修复 worktree: `codex/comment-study-p2-integrity-001`；从已合并的 P2 主线继续。
- 本轮基线: `origin/main@7f5a0796f06e76e6ad489a358bfaf7b4ea56bbaf`。
- 目标: 完成 P2 的输入上下文、继续可恢复目标、过大响应恢复、研究结果透明度与重复/零合格作品排除。
- 用户可见结果: 普通启动默认只研究新评论；已结束 Run 可显式补跑未完成评论，并从源 Run 精确范围中重算资格。每条结果显示原声、实际冻结的作品/父评论语境、模型原因或故障原因和下一步；合法 Signal 可直接查看。
- 明确非目标: 不推进 P3/P4 新状态机或 Problem 归并、不调用真实模型、不改共享数据库/迁移、不删除合法研究历史。本轮修复的 exact-head 合并与运行时切换仍待 Mog 决定；业务验收由 Mog 完成。

## 2. 读取回执

| 来源 | 状态 | 本次解决的问题 | 已核对 |
|---|---|---|---|
| `AGENTS.md` / current-state | 已读 | 受保护交付、worktree、真实副作用边界 | 2026-09-29 |
| UI execution contract / LIDS | 已读 | 页面状态、展示限制与验收证据范围 | 2026-09-29 |
| COMMENT-STUDY-PRODUCTIZATION-001 总体计划/页面规格 | 已读 | P2 顺序、输入合同与现有用户路径 | 2026-09-29 |
| HTTP / 数据合同 | 已读 | Run 模式、目标投影与状态名称 | 2026-09-29 |
| 当前实现与运行数据 | 已读 | latest Run、target 输入、模型 reason、实际父评论关系 | 2026-09-29 |

## 3. 变更分类

- 分类: 混合；最高风险为状态语义与冻结输入真实性。
- 对应来源: P2、LIDS Data Truth、`comment-study.read.v1/v2`、`comment-study.run-selection.v2`。
- Pattern: L1 Corpus Explorer；复用当前表格、详情 dialog、Tab 与状态组件；不新增 Token、CMP、Scene 或 Motion。
- `DECISION_REQUIRED`: 无。直接父评论只解释指代，不能替代目标评论证据；Problem 建立仍不在本范围。

## 4. 表面、状态、依赖与边界

| 表面 | 必须表达 | 禁止表达 |
|---|---|---|
| 研究作品目录 | 每个 workRef/role 一行；仅列至少一条当前可研究评论的作品 | 重复采集记录变成重复研究作品；零合格作品仍可勾选 |
| 启动默认值 | 普通启动仅选新评论；源 Run 的“补跑未完成”显式选择精确评论集合，沿用原预算，通过唯一启动事务重算资格 | 默认扩大模型调用范围；把已成功、在途、受限或仍不可恢复项带入新 Run |
| 目标详情 | 原声 → Run 冻结工作语境 → Run 冻结父评论 → Signal 或模型 reason/失败原因 → 可恢复动作 | 把当前新读到的语境说成历史 Run 输入；把上下文写成评论证据 |
| 研究结果 | 选中 Run 的所有合格 Signals，包括已归并、待归并与其他合法状态 | 只显示“待归并”而隐藏已处理 Signal |
| 运行汇总 | 成功信号、无信号、等待语境、失败、排除分别计数 | 将所有终态压成单一“已完成”计数 |

页面按权威 PAGE 规格使用四个一级视图：概览、用户评论、研究批次、用户问题。研究批次内通过目标评论／研究结果子面板先展示冻结目标和输入诊断，再展示该 Run 全部合法 Signal；“待归并”只表示 Problem 归并状态，不作为全部研究结果的筛选条件。

本轮表面与状态矩阵：普通创建对话框的默认模式为 `new_only`，普通预览/启动接口拒绝 `continue_ready`；研究批次行对已结束且含未完成目标的 Run 提供显式恢复动作；目标详情分别显示目标原声、冻结语境、模型原因与受限态；Run 控制动作必须先按服务端回执刷新列表再显示下一操作；切换 Run 时迟到的旧响应不得进入新 Run 缓存。恢复动作调用原有 `start_study_run` 事务，源 Run 及目标的不可变历史只读，页面不建立第二套选择或预算状态。验收分别覆盖正常/空/失败/受限/处理中/确认回执，以及源 Run 已被后续成功、仍在途、仍无上下文和用户主动停止的反例。

父评论原文当前受限时必须隐藏；技术 batch/call 细节折叠，不展示 prompt、密钥或 provider 原始流。

## 5. 验收与证明边界

| 层级 | 验收方法 | 实际结果 | 未证明边界 |
|---|---|---|---|
| 目录 | 隔离 PostgreSQL 重复 association/零 eligible 反例 | 既有 `comment_study_work_catalog_postgres` 4/4 通过 | 当前生产/本机数据之外的覆盖规模 |
| 上下文与续做 | root、reply、parent missing、context fingerprint 改变及不变；源 Run 精确补跑 | 隔离 PostgreSQL 的 `missing_parent_reaches_model_and_only_changed_context_requeues_after_needs_context` 与 `source_run_recovery_uses_exact_failed_keys_and_keeps_original_history` 通过 | 人工 Gold Set 与语义质量 |
| 响应恢复 | 传输/文本上限定位；多目标拆为单目标；凭证缺失只失败一次 | adapter 合成 31/31；隔离 PostgreSQL 拆分和凭证缺失用例通过 | 真实 provider 端容量与成本 |
| 页面与结果 | Run 回执、默认范围、显式补跑、冻结输入与受限态 | synthetic Playwright 通过；真实 Axum/PG 浏览器 1/1，命令 API 11/11；补跑 HTTP 精确范围反例另单项通过 | 共享运行时 `:3000` 和 Mog 人工验收 |
| 页面 URL 路由 | 页面入口接受页面规格登记的导航字段（domain、view、q、workRef、runRef、problemRef、commentExternalId、state、cursor、detail、panel），API 查询仍各自严格 | `origin/main` 与 `runtime-main` 已同步到 `08d995fc2e695bdda589a6591c5a7d2e881992c9`；真实 Run 的结果入口可切换 URL、滚动并聚焦详情 | 浏览器键盘激活原生结果按钮已验证；Mog 对实际鼠标点击与业务内容的人工验收仍待完成 |

## 6. 交接

- 预期文件: selection/context、batch failure handling、read projection/API、`comment_study.html/js/css`、HTTP 合同、总体计划、progress 与本清单。
- 验证命令：`npm test`（Pi adapter 31/31）、`scripts/test-comment-study-productization-ui.py`（synthetic Playwright）、`scripts/test-comment-study-productization-postgres.sh`（一次性 PostgreSQL 中 Intelligence 122/122、Axum 命令 11/11、worker 2/2）。补跑 HTTP 的第三条新评论排除反例在最终小幅增强后另用一次性 PostgreSQL 单项 1/1 通过。PR exact-head CI 待建立后核验。
- 不申请共享迁移或真实 provider。PR 提交后由 Mog 决定合并与 `:3000` 运行时切换；Mog 的实际页面验收仍待完成。

## 7. 结果查看的可见反馈（2026-09-30）

- 复现：真实 Run 行的“查看结果”会切换 URL 并加载“目标评论与上下文”，但结果区在运行列表下方；页面仍停留在列表顶部，仅按钮文案变为“当前查看”，所以用户看起来像没有反应。
- 候选：仅当当前渲染已提交且用户仍在研究批次页、所选 Run 未变化时，才滚动到详情并把键盘焦点移到详情标题。桌面详情使用滚动边距避开 sticky 工具栏；用户偏好减少动态效果时使用即时滚动。详情标题可被程序化聚焦，避免按钮随 DOM 更新消失后键盘/读屏用户失去位置。
- 状态：首轮独立复审发现标题可能被 sticky 工具栏遮挡、JS 平滑滚动未响应减少动态效果偏好；两项已修正。二次独立复审无阻断；`git diff --check`、项目治理和 UI 手册检查通过。提交 `08d995fc` 已直接快进合入 `main`；官方 `runtime-main/scripts/runtime/install.sh` 确认 worker drain、迁移台账最新、无模型调用，三个服务已重启，health 为 ready。浏览器用真实 Run 验证：原生按钮 Enter 激活后 URL 切换、详情标题获得焦点并在桌面视口中避开 sticky toolbar。鼠标点击与 Mog 业务验收仍待人工确认；不得将 P2 整体验收表述为完成。
