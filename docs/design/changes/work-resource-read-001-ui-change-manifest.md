# WORK-RESOURCE-READ-001 · UI Change Manifest

> 状态: 权威当前
> 最后核对: 2026-08-31
> 适用范围: Issue #110 对 `/corpus/evidence` 与 Intelligence 共享 Work Resource Read 的 UI 变更
> 事实来源: Issue #110、PAGE-EVIDENCE-001、LIDS、Media V2、WORK-RESOURCE-READ-001 草案与当前分支代码
> 冲突时以谁为准: 用户最新确认、AGENTS.md、当前代码/API 合同与真实运行证据
> Issue: #110

## 事项与读取回执

- Agent / worktree：Codex；`codex/issue-110-work-resource-read`；`/Users/moglenny/proma/worktrees/linggan-work-resource-read-001`
- 目标：一个共享作品资源合同，修正作者/目标与发布时间表达，并提供三种排版。
- 明确非目标：不采集新正文/评论/媒体，不回填历史 Package，不改 Media V2 身份，不迁移共享库，不发布运行时，不合并。

| 来源 | 状态 | 本次用途 | 已核对 |
|---|---|---|---|
| `AGENTS.md` / current-state | 权威当前 | Issue、worktree、Unknown、时间与 Media 边界 | 是 |
| UI execution contract / design governance | 权威当前 | 变更分类、表面/状态/依赖/验收要求 | 是 |
| `PAGE-EVIDENCE-001` | 权威当前 | 页面任务、Corpus Explorer、Inspector 与状态 | 是 |
| LIDS system/tokens/primitives/patterns/agent guide/language | 权威当前 | Token → Primitive → Component → Pattern → Page | 是 |
| Media lifecycle / Material projection data map | 权威当前 | 媒体 owner、lane 与受控预览 | 是 |
| 当前 Rust/SQL/JS/plugin tests | 当前事实 | 共享 seam、时间丢失断点、目标血缘与布局入口 | 是 |

## 分类、表面与停止条件

- 分类：展示 + 交互 + 状态语义的混合变更；最高风险为状态语义。
- 依据：用户本轮确认、`PAGE-EVIDENCE-001`、`WORK-RESOURCE-READ-001`、Media V2 与时间不变量。
- LIDS：L1 Corpus Explorer + embedded L2 Split Evidence Inspector；主 Pattern 不变。
- 受影响表面：`/corpus/evidence` 的结果头、作品列表、Inspector overview/provenance、窄屏列表。
- 受影响状态：`KNOWN`、`SOURCE_TEXT_ONLY`、`UNKNOWN`、`OBSERVED_ON_TARGET_SURFACE`、`MATCHED`、`NOT_VERIFIED`、`MISMATCH`。
- 依赖：共享 Work Resource API、typed Material Projection、Media V2、Target/WorkOrder/Task/Attempt/Package/Receipt。
- 禁止：页面私有 SQL/API fallback、目标名覆盖作者、相对时间晋升精确时间、远程媒体绕过、平台访问或数据写回。
- 停止条件：真实签名详情请求未获当次确认；共享 migration/deploy/merge 未获授权；任何不能回链来源的显示值。

## 变更

- Evidence Library 改为消费中立的 `/api/local/work-resources`，不再把页面名当资源合同。
- 结果头增加 `研读 / 表格 / 封面` 三种排版。`layout` 只控制前端排版，`view` 继续控制状态筛选。
- 列表同时呈现“作品作者”与“监控目标”；没有平台作者 ID 证明时，不用目标显示名填补作品作者。
- 发布时间区分精确 `KNOWN`、仅来源文本 `SOURCE_TEXT_ONLY` 和 `UNKNOWN`；Inspector 展示来源字段、类型、精度和 parser version。
- 来源血缘增加 Target 与 Work Order，并继续保留 Task、Attempt、Package、Receipt。

## LIDS 影响

| 层 | 影响 |
|---|---|
| Token | 无；继续只消费现有 token |
| Primitive | 增加安静的三段 layout selector，40px 可点击目标、focus-visible、160ms 颜色/按压反馈 |
| Component | Work Resource row 增加 table/cover 变体；不建立页面私有数据组件 |
| Pattern | Corpus Explorer 保持；三种布局共享同一选择与 Inspector |
| Page | `PAGE-EVIDENCE-001` 更新共享入口、监控上下文和时间资格 |
| Data Truth | 新增 `SOURCE_TEXT_ONLY`；作者与目标关系显式状态化 |

## 响应式与无障碍

- 桌面 Table 使用四列比较，窄屏自动堆叠；Cover 在 640px 为双列、420px 以下单列。
- layout selector 使用 `role=group` 与 `aria-pressed`；Table header 的 `hidden/aria-hidden` 同步。
- 列表三种布局继续使用同一 `role=option`、键盘方向键、Enter/Space 选择与右侧 Inspector。
- Reduced Motion 沿用页面全局规则；未新增自动播放、悬浮必需信息或仅颜色状态。

## 未证明

当前已有静态代码、JS 语法、Rust 编译/测试构建、插件聚焦测试与隔离 PostgreSQL proof；尚未证明已发布运行页、真实 Chrome 三视口、真实签名详情字段、真实数据回填或 Mog 视觉/业务验收。

## 验收矩阵

| 层 | 方法 | 当前结果 | 未证明 |
|---|---|---|---|
| 任务可用 | Rust UI source test、三按钮与 `layout/view` 分离检查 | 自动检查通过 | 真实 Chrome 点击与长列表效率 |
| 状态诚实 | PostgreSQL exact epoch / relative text / target lineage 正负 Oracle | 合成 proof 通过 | 真实目标作品详情的 author/time 字段 |
| 视觉一致 | LIDS token/selector/responsive/source audit | source 与静态门禁通过 | 桌面、900、375px 实际渲染 |
| 真实后果 | layout 切换仅前端 rerender；API 为只读 GET | 代码和测试证明无写 route | 未部署，未对共享库产生副作用 |

## 交接

- 规则/索引同步：PAGE、LIDS migration log、架构合同、current-state、月度 progress、docs index。
- 例外：无新 Token/CMP/权限；旧 `/api/local/evidence-library/legacy` 仅保留显式兼容，不作为 fallback。
- PR / reviewer / integration：按 Issue #110 建 PR；reviewer、合并和部署等待 Mog 指定。
