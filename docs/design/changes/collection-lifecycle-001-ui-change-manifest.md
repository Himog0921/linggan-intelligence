# COLLECTION-LIFECYCLE-001 · Creator 生命周期抽屉 UI 变更清单

> 状态: 权威当前
> 最后核对: 2026-09-03
> 适用范围: Issue #148 的 `/collection/targets` creator drawer、lifecycle JSON API 与 `/corpus/evidence?work=` 精确定位
> 事实来源: 用户最新决定、Issue #148 Claim、`PAGE-COLLECTION-001`、LIDS v7、当前 Work Resource/Collection 代码与隔离 PostgreSQL 测试
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实代码/数据库/测试、ACCEPTED 决定与 Issue #148 Claim

## 1. 变更分类与用户结果

| 项 | 判定 |
|---|---|
| 变更类型 | 展示 + 交互 + 状态/语义；以状态/语义为最高风险 |
| LIDS 强度 / Pattern | Collection L1 Operations；目标抽屉为受限 L2；Collection Control |
| 用户结果 | 点击 creator target 后，默认概览直接判断已接纳作品在真实发布时间轴上的表现、可绘制范围和排除原因 |
| 第一视觉锚点 | 作品散点 + 服务端 5 点滚动中位线；不复制旧内容工作台外观 |
| 停止条件 | 需展示名猜作者、观察时间补发布时间、UNKNOWN 补 0、新事实表/索引 migration、Evidence 复制或外部平台访问 |

## 2. 来源回执与方向锁

- 采用 `PAGE-COLLECTION-001` 的五个 Collection 工作面和受限右抽屉，不建第六个页面。
- 采用 LIDS v7 白场、结构线、Signal 选择、中文主表达、40px 控件、400/600/700 字重、0/4/8 圆角、390px 与 reduced-motion。
- 历史内容工作台只提供行为对标；旧服务端的 `trailing-5-work-median-v1` / `window=5` 是冻结领域口径。旧前端 15 点重复计算不继承，也不新造 7 点口径。
- 近 90 天固定为 `Asia/Shanghai` 90 个含首尾日历日；不是 `as_of - interval '90 days'` 的滚动 2160 小时。
- `life_work` 只是 URL/UI 选择；Rust lifecycle query 与 HTTP query 不接受 `selected_work`。

## 3. 表面、状态与依赖

| 表面 | 本项实现 | 明确不做 |
|---|---|---|
| creator overview | 默认生命周期、覆盖摘要、排除回执、可访问 SVG、最小选中作品摘要 | 正文、评论、媒体、Evidence fragment、监控价值、趋势预测 |
| keyword overview | 明示 `NOT_APPLICABLE`，不读 2000 点生命周期 | 伪造空曲线 |
| baseline / patrol / trace | 保留既有职责与 URL 状态；不执行生命周期扫描 | 生命周期重复读取 |
| drawer tabs | `overview / baseline / patrol / trace` 四个职责 | Evidence tab |
| lifecycle API | target、as-of、window/metric、summary/exclusions/receipt/analysis/points | `selected_work`、Corpus Inspector 或敏感材料 |
| Corpus deep link | 首批列表外的稳定 Work 仍直读 detail seam | 回退第一条或复制详情到 Collection |

关键状态：`READY`、`INSUFFICIENT_OBSERVATION`、`NOT_APPLICABLE`、`READ_UNAVAILABLE`、`SCAN_LIMITED`。真实 `KNOWN 0` 是点；UNKNOWN 是排除原因。

## 4. 实现边界

- 新增 `target_drawer.css`，只消费已有 `--lgi-*`；无 token 声明、字面颜色、渐变、玻璃或图标库。
- SVG 每个作品点都是有 `aria-label` 的真实链接；鼠标和键盘走同一 URL。选中点只显示标题、发布时间、当前指标、创作者内分位与 Corpus 链接。
- 图表使用 `log(1 + metric)` 视觉纵轴，原始值在可访问名和摘要中保留；一篇、零值、并列与极值不会因对数轴消失。
- 服务端返回并渲染 `creator-percentile-v1` 与 `trailing-5-work-median-v1`；浏览器不重算分析值。
- 页面仅在 creator + overview 读取 lifecycle。API 保持独立可读，窗口/指标闭集验证，未知参数被拒绝。

## 5. 证明边界

| 层 | 证据 | 当前结论 |
|---|---|---|
| 领域/数据库 | Rust unit + isolated PostgreSQL 16 | stable author、qualified time、KNOWN、上海 90 日、2000+1 receipt 已覆盖 |
| API | Axum route tests + isolated PostgreSQL | 闭集、404/503、最小响应、敏感字段负向断言已覆盖 |
| UI/交互 | Rust render/source tests | 四 tab、默认图、SVG a11y、无监控价值、server-owned median、深链已覆盖 |
| CSS | page-local source guard + 隔离视口 | 以 `ACC-COLLECTION-LIFECYCLE-001` 最终记录为准 |
| 现实世界 | 未执行 shared runtime、平台访问或部署 | NOT VERIFIED；不由自动检查替代 |
| Mog 业务验收 | Draft PR 后待用户检查 exact head | NOT VERIFIED |

## 6. 文件与交接

- 主要实现：`crates/evidence/src/creator_lifecycle.rs`、`apps/api/src/local_web/creator_lifecycle_api.rs`、`apps/api/src/local_web/target_drawer.rs`、`apps/api/src/local_web/target_drawer.css`、`apps/api/src/local_web/evidence_library.js`。
- 测试：`crates/evidence/tests/creator_lifecycle_postgres.rs`、`apps/api/src/local_web/creator_lifecycle_tests.rs`。
- 规格与验收：`PAGE-COLLECTION-001`、本清单、`ACC-COLLECTION-LIFECYCLE-001`、LIDS migration log。
- 无设计例外、无新 CMP、无 schema/migration、无外部副作用。
