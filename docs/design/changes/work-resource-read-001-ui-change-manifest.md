# WORK-RESOURCE-READ-001 · UI Change Manifest

> 状态: 权威当前
> 最后核对: 2026-08-31
> 适用范围: Issue #110 对 `/corpus/evidence` 与 Intelligence 共享 Work Resource Read 的 UI 变更
> 事实来源: Issue #110、PAGE-EVIDENCE-001、LIDS、Media V2、WORK-RESOURCE-READ-001 草案与当前分支代码
> 冲突时以谁为准: 用户最新确认、AGENTS.md、当前代码/API 合同与真实运行证据
> Issue: #110

## 事项与读取回执

- Agent / worktree：Codex；`codex/issue-110-work-resource-read`；`/Users/moglenny/proma/worktrees/linggan-work-resource-read-001`
- 目标：一个共享作品资源合同，修正作者/目标与发布时间表达，提供三种排版，并按 Mog 对当前运行页的反馈收敛首屏层级。
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
- 受影响表面：`/corpus/evidence` 的查询条、读取回执、系统/我的视图工作台、排版选择、作品列表、Inspector overview/provenance、窄屏列表。
- 受影响状态：`KNOWN`、`SOURCE_TEXT_ONLY`、`UNKNOWN`、`OBSERVED_ON_TARGET_SURFACE`、`MATCHED`、`NOT_VERIFIED`、`MISMATCH`。
- 依赖：共享 Work Resource API、typed Material Projection、Media V2、Target/WorkOrder/Task/Attempt/Package/Receipt。
- 禁止：页面私有 SQL/API fallback、目标名覆盖作者、相对时间晋升精确时间、远程媒体绕过、平台访问或数据写回。
- 停止条件：真实签名详情请求未获当次确认；共享 migration/deploy/merge 未获授权；任何不能回链来源的显示值。

## 变更

- Evidence Library 改为消费中立的 `/api/local/work-resources`，不再把页面名当资源合同。
- 控制工作台提供 `研读 / 表格 / 封面` 三种排版。`layout` 只控制前端排版，`view` 继续控制状态筛选。
- 删除主工作面重复的“语料 / 证据审查”“以作品为顶层的多材料证据库”“作品级材料集合 / 多通道真实状态”；页面方位继续由共享 shell、rail 和无障碍标题承担。
- 原左侧 `STATE VIEWS` 不是页面导航，而是 `view` 预设查询；恢复参考稿的 `SYSTEM VIEWS / 系统视图` 横向策略。全部材料、部分取得、风险停止、媒体已清理、撤回或受限继续映射原有查询条件并重新读取，未改变状态语义。
- 同时恢复 `MY VIEWS / 我的视图` 的结构位置，但保存合同未接通，因此只显示 `暂无已保存视图 / SAVED VIEWS NOT CONNECTED` 禁用空态，不生成假收藏项或保存动作。
- 删除左栏重复的当前结果、当前选择与读取状态：结果数保留在控制工作台底栏，读取状态保留在 inline receipt，选择由选中行与 Inspector 直接表达。结果区因此只保留连续作品列表 + 右侧 Inspector。
- Table 可读性升级：标题 12→14px、作者上下文 9→12px、辅助正文 9→11px、状态 8→10px，桌面行高 76→92px；窄屏堆叠保持同一字号。
- `研读 / 表格 / 封面` 不再使用三个独立描边方格或黑底选中块；改为参考稿中的单条 tab rail，上下黑色细线维持编辑网格，当前排版只以 signal 下划线标记。语义、URL 参数和点击目标不变。
- 列表同时呈现“作品作者”与“监控目标”；没有平台作者 ID 证明时，不用目标显示名填补作品作者。
- 发布时间区分精确 `KNOWN`、仅来源文本 `SOURCE_TEXT_ONLY` 和 `UNKNOWN`；Inspector 展示来源字段、类型、精度和 parser version。
- 来源血缘增加 Target 与 Work Order，并继续保留 Task、Attempt、Package、Receipt。
- 2026-08-31 封面比例修订：作品行增加只用于排版的 `data-platform`；小红书在研读与封面排版中使用 `3:4` 竖版容器。改动不改变 Work Resource 字段、媒体读取资格、当前选择或 Inspector。

## LIDS 影响

| 层 | 影响 |
|---|---|
| Token | 无；继续只消费现有 token |
| Primitive | 三段 layout selector 与五项系统视图均使用 40–44px 的紧凑点击目标、focus-visible、160ms 颜色/按压反馈 |
| Component | Work Resource row 增加 table/cover 变体；不建立页面私有数据组件 |
| Pattern | Corpus Explorer 保持；系统/我的视图策略回到查询工作台，我的视图诚实禁用，三种布局共享同一选择与 Inspector |
| Page | `PAGE-EVIDENCE-001` 更新共享入口、监控上下文、时间资格和无重复标题/状态侧栏的两列工作面 |
| Data Truth | 新增 `SOURCE_TEXT_ONLY`；作者与目标关系显式状态化 |

## 响应式与无障碍

- 桌面 Table 使用四列比较，窄屏自动堆叠；Cover 在 640px 为双列、420px 以下单列。
- 桌面主工作面为 flexible results + 440px Inspector；系统/我的视图在控制工作台内分区，不再占第三列。≤640px 时系统视图可在自身区域横向滚动，不造成 document 溢出；Table 改为单列堆叠。
- layout selector 使用 `role=group` 与 `aria-pressed`；Table header 的 `hidden/aria-hidden` 同步。
- 列表三种布局继续使用同一 `role=option`、键盘方向键、Enter/Space 选择与右侧 Inspector。
- Reduced Motion 沿用页面全局规则；未新增自动播放、悬浮必需信息或仅颜色状态。

## 未证明

当前已有静态代码、JS 语法、Rust 编译/测试构建、插件聚焦测试与隔离 PostgreSQL proof；另在隔离的 `127.0.0.1:3300` 只读实例中，以本机 13 个作品集合完成 1440×900 与 375×812 的 in-app Browser 几何/截图检查、双视图区/表格字号核验及系统视图点击检查。尚未证明已发布到 `:3000`、真实 Chrome 多视口、真实签名详情字段、真实数据回填或 Mog 对本轮结果的最终视觉/业务验收。隔离实例未接当前运行快照的媒体根，图片字节不纳入本轮视觉结论。

## 验收矩阵

| 层 | 方法 | 当前结果 | 未证明 |
|---|---|---|---|
| 任务可用 | Rust UI source test、五项状态预设、三按钮与 `layout/view` 分离检查 | 自动检查与 in-app Browser 真实点击通过；同一选择跨 layout 保留 | 真实 Chrome 长列表效率 |
| 状态诚实 | PostgreSQL exact epoch / relative text / target lineage 正负 Oracle | 合成 proof 通过 | 真实目标作品详情的 author/time 字段 |
| 视觉一致 | LIDS token/selector/responsive/source audit + 1440×900 / 375×812 实际渲染 | 两视口无 document 横向溢出；双视图区层级清晰；Table computed font/row height 达到 14/12/11/10px 与 92px | 900px、真实 Chrome、Mog 最终审美验收 |
| 真实后果 | layout 切换仅前端 rerender；API 为只读 GET | 代码和测试证明无写 route | 未部署，未对共享库产生副作用 |

## 交接

- 规则/索引同步：PAGE、LIDS migration log、架构合同、current-state、月度 progress、docs index。
- 例外：无新 Token/CMP/权限；旧 `/api/local/evidence-library/legacy` 仅保留显式兼容，不作为 fallback。
- PR / reviewer / integration：按 Issue #110 建 PR；reviewer、合并和部署等待 Mog 指定。
