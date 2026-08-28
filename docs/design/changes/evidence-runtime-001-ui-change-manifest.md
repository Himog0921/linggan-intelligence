# EVIDENCE-RUNTIME-001 · 多材料证据库运行页面变更清单

> 状态: 权威当前
> 最后核对: 2026-08-29
> 适用范围: Issue #90 将多材料 Evidence Library 产品手册与 Material Projection 只读合同落到 `/corpus/evidence`
> 事实来源: Issue #90、PR #87 exact head `bac3a7148229843c43bdedf7659e758c70191076`、PR #88 exact head `510b5fa9c9c2e38aedf1e785ea7acecbeded324a`、PAGE-EVIDENCE-001、LIDS 与当前 Rust 代码
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实运行/代码/API 合同；本清单不扩大 Material Projection 或媒体生命周期语义

## 1. 事项

- Issue / Work Package：Issue #90 / `EVIDENCE-RUNTIME-001`，五卡 Evidence Library 垂直交付的卡 4。
- Agent / worktree：`/root/evidence_runtime_agent`；`codex/issue-90-evidence-runtime`；独立 worktree `evidence-runtime`。
- Stacked base：PR #88 exact head `510b5fa9c9c2e38aedf1e785ea7acecbeded324a`；本分支已合入 PR #87 exact head 作为设计输入。
- 目标：让 `/corpus/evidence` 默认消费 `/api/local/evidence-library` 的 Material Projection，以来源作品为顶层对象显示九条材料 lane，并按 `detailUrl` 读取右侧 Inspector。
- 用户可见结果：用户可以检索当前已接纳材料、选择作品、核验详情/评论/媒体/派生/来源，并在有界通道中继续读取下一页。
- 明确非目标：修改数据库、接纳、媒体资格/处置语义、插件、采集调度、OCR/ASR provider、部署、真实平台、Issue #89 或 legacy 默认混读。

## 2. 读取回执

| 来源 | 状态 | 本次解决的问题 | 已核对 |
|---|---|---|---|
| `AGENTS.md`、`docs/README.md`、`docs/current-state.md`、`docs/governance/*` | 权威当前 | 文件治理、事实层级、协作边界与唯一验收轮次 | 2026-08-29 |
| UI execution contract / design governance | 权威当前 | 表面、状态、依赖、验收矩阵与停止条件 | 2026-08-29 |
| `PAGE-EVIDENCE-001`、PR #87 reference/verifier/acceptance | 权威当前 / 一次性报告 | 主对象、信息架构、lane、视觉与交互 | 2026-08-29 |
| LIDS SYS/TOK/PRI/PAT/AGENT/LANG | 权威当前 | 原生 CSS、共享 Shell、L1 Corpus Explorer + L2 Inspector、中文优先、a11y | 2026-08-29 |
| `material-projection-data-map.md` 与 PR #88 Rust/API/tests | 代码事实优先 | 列表、`detailUrl`、详情、评论研究通道、有界 receipt、本地媒体句柄 | 2026-08-29 |
| 当前 Evidence Library HTML/CSS/render/tests | 代码事实优先 | Discovery-only 旧运行页需要由 Material Projection 客户端取代 | 2026-08-29 |

## 3. 方向锁与变更分类

- Visual thesis：高密度但克制的冷白证据台面；九条材料 lane 的状态带是唯一视觉锚点，Signal 只表达当前选择。
- Content plan：本地查询与读取回执 → 作品材料集合 → 当前作品详情 → 通道 receipt 与来源限制；无营销 Hero。
- Interaction thesis：列表行键盘选择并按 `detailUrl` 更新 Inspector；Tab 使用 roving tabindex；有 `nextCursor` 时只对当前通道继续加载，窄屏选择后进入详情并可返回。
- CSS strategy：原生 CSS only；只消费 `lids_tokens.css + shell.css`，页面样式不重定义共享 Shell。
- Radius：沿用 LIDS `0 / 2 / 4 / 8px`；不引入 Pill 或第二套容器语言。
- 分类：展示 + 交互 + 状态/语义；最高风险为敏感材料的状态/语义呈现，但不新增权限或写动作。
- 对应来源：`PAGE-EVIDENCE-001`、`LIDS-SYS-001`、`LIDS-PRI-001`、`LIDS-PAT-001`、`LIDS-LANG-001`、`MATERIAL-PROJECTION-001`。
- DECISION_REQUIRED：无。API 缺少产品手册目标字段时统一显示 `SOURCE_INCOMPLETE`，不扩后端合同。
- LIDS：L1 Corpus Explorer + 受限 L2 Split Evidence Inspector；触及 Page、Motion 与 Data Truth，不改 Token/Primitive/CMP/Scene。

## 4. 表面地图

| 表面 | 责任 | 数据/状态来源 | 禁止替代 |
|---|---|---|---|
| 共享页头 / context / 二级 rail | 产品方位、结果数、选择与读取状态 | `shell.rs` + 页面客户端状态 | 页面 CSS 重定义 Shell、复制标题块 |
| 查询与筛选 | 构造本地 Material Projection 查询 | URL 查询参数 + 列表 receipt | 平台搜索、保存视图、采集动作 |
| 中央连续列表 | 作品级身份、预览、九条 lane、限制 | `/api/local/evidence-library` | legacy cards、Package/Blob/Slot 顶层对象 |
| 右侧 Inspector | 概览、评论、媒体/派生、来源/限制 | `detailUrl` 与 channel receipts | raw Package 拼装、远程 CDN、AI 结论 |
| Inline feedback | 加载、空、失败、查询无效、部分、受限 | `local_read` error / Material fields | toast-only、把失败写为空库 |
| 375px 顺序流 | 查询 → 列表 → Inspector → 返回 | 本地视图状态 | 横向压缩三栏、隐藏关键状态 |

## 5. 状态词典

- Lane 原样消费：`NOT_REQUESTED / QUEUED / NOT_OBSERVED / OBSERVED / PARTIAL / ACQUIRED / PROCESSING / NOT_ENABLED / SEARCHABLE / FAILED / RISK_CONTROL / BYTES_CLEANED / WITHDRAWN_OR_RESTRICTED / UNKNOWN`。
- `UNKNOWN` 不显示为 0；`NOT_OBSERVED` 不显示为来源不存在；媒体槽位 `OBSERVED` 不显示为字节已取得；Package ACK 不显示作品完整。
- 评论普通列表只显示 lane/计数状态；本机授权详情通道才显示匿名上下文原文，且不渲染平台用户标识。
- 媒体只渲染非空 `localAssetUrl`；`deliveryState != INLINE_SAFE`、不安全/未知类型、受限或已清理只显示状态，不内联。
- API 未提供但页面手册要求的信息统一显示 `SOURCE_INCOMPLETE`，不由 UI 推断。

## 6. 依赖与文件边界

| 依赖 | owner | 本卡用法 | 本卡不得做 |
|---|---|---|---|
| PR #87 PAGE / reference | 卡 2 | 运行页结构与视觉锚 | 修改产品主对象或状态词典 |
| PR #88 Material Projection | 卡 3 | 唯一读取 API | 修改 DB/admission/media semantics |
| `shell.rs` / `shell.css` / LIDS tokens | shared UI | 原样复用 | 页面内重定义共享类或 Token |
| 卡 5 真实垂直证明 | 后续 | 留出真实验证入口 | 本卡访问真实平台或声称业务验收 |

允许：Evidence Library HTML/CSS/最小 JS、只读路由组合、聚焦 UI tests、直接相关 Page/manifest/acceptance/current-state/progress/index。禁止：`database/`、`plugins/`、Material Projection 领域/SQL/媒体资格语义、部署和真实数据。停止条件：必须新增 API 字段、扩大敏感权限、使用远程媒体或改变媒体处置资格。

## 7. 验收矩阵

| 层级 | 一次性验收方法 | 通过条件 | 未证明边界 |
|---|---|---|---|
| 任务可用 | 聚焦 UI tests + 浏览器 DOM/键盘走查 | 列表 → `detailUrl` → Tab/通道下一页 → 窄屏返回闭环成立 | 真实平台内容质量 |
| 状态诚实 | fixture/DOM 断言 | unknown、partial、not observed、restricted、cleaned、read error 不互相冒充 | 全量生产状态分布 |
| 视觉一致 | reference verifier + handbook + 桌面/375px 截图/几何 | 无横向越界，lane 为视觉核心，Shell/LIDS/40px/focus/reduced motion 成立 | Mog 最终审美验收 |
| 真实后果 | 网络请求与 diff 审计 | 只调用 loopback Material Projection / controlled asset；无写请求、CDN fallback | 部署、采集、OCR/ASR、业务验收 |

## 8. 交接

- 实施冻结后只运行 Issue #90 约定的一次正式验证与一次桌面/375px 浏览器检查。
- 不邀请独立 reviewer，不新增验收门槛，不修非本卡基线问题。
- PR 为 Draft、`Refs #90`，base 指向 `codex/issue-86-material-projection`；Mog 决定合并。
