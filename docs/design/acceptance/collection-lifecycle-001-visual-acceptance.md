# ACC-COLLECTION-LIFECYCLE-001 · Creator 生命周期抽屉验收记录

> 状态: 一次性报告
> 最后核对: 2026-09-04
> 适用范围: Issue #148 当前 branch 的 creator target lifecycle、四职责抽屉与 Corpus Work 深链
> 事实来源: 当前 branch 自动检查、隔离 PostgreSQL 16、隔离 loopback 页面与视口检查
> 冲突时以谁为准: 真实代码/数据库/测试、Issue #148 Claim 与用户最新确认；本报告不替代共享运行时或 Mog 验收

## 1. 验收对象

- Issue / SCOPE: `#148 / COLLECTION-READ-MODEL-CLOSURE-001`
- 页面: `/collection/targets?drawer=<target-ref>` 与 `/corpus/evidence?work=<public-ref>`
- 关联: `PAGE-COLLECTION-001`、`COLLECTION-LIFECYCLE-001`、LIDS v7、`creator-percentile-v1`、`trailing-5-work-median-v1`
- 数据前提: 仅既有 append-only target/Task/Package/Work/detail/engagement 事实；无新表、无外部采集
- 当前支持基线: **1440 CSS px 桌面全屏**。Mog 的“不在手机或小于 13 寸屏幕运行”是使用场景，不是 CSS breakpoint；1280/390 不属于本 Package 验收。
- 风险接受: 既有 1280/390 结果仅保留为诊断证据。一次隔离 1280 测量中 Inspector 右缘超出 viewport 约 49.83 CSS px 的裁切移出当前范围；页面局部 focus 声明也作为已接受技术债保留，已验证表面的 computed focus 仍正确。本记录不把原 reviewer FAIL 改写成 PASS。

## 2. 场景矩阵

| 场景 | 预期含义 | 自动/隔离结果 | 视觉/交互结果 |
|---|---|---|---|
| creator 有合格点 | 默认看见真实发布时间散点、原值与 5 点中位线 | VERIFIED | 1440 验收基线 VERIFIED；1280 仅为历史诊断 |
| KNOWN 0 / UNKNOWN | 0 可绘制；UNKNOWN 排除且说明原因 | VERIFIED | 文案与摘要 seam VERIFIED |
| 同名不同稳定 ID | 只纳入 exact author identity | VERIFIED | N/A |
| 相对时间/作者不符 | 不进入点，分项回执 | VERIFIED | 回执 seam VERIFIED |
| 上海 90 日边界 | 90 个含首尾日历日；起点前 1 秒排除 | VERIFIED | 控件口径可见 |
| keyword | 不读 lifecycle，显示不适用 | VERIFIED | 无 SVG 伪曲线 |
| scan 2000+1 | 截断可见，不冒充完整历史 | VERIFIED | receipt seam VERIFIED |
| Work Resource Current parity | 生命周期与列表/单品/Inspector 共享 title/author/published/engagement 裁定，含同 `observed_at` tie | VERIFIED | N/A |
| 非法 window/metric | HTML 为 `QUERY_INVALID`；API 422；不伪报默认值 | VERIFIED | 隔离页面 VERIFIED |
| `all` 窗口 | caption 明示全部合格历史 | VERIFIED | render seam VERIFIED |
| 退役/未知 `dtab` | 同一 parser 归一 Overview 并实际读取 | VERIFIED | `dtab=evidence` 隔离页面 VERIFIED |
| 选中作品 | 只显示最小摘要并跳 Corpus | VERIFIED | 1440 支持基线通过；390 DOM 键盘/深链结果仅作诊断证据 |
| Work 不在首批列表 | 直读目标 detail，不回退第一项 | VERIFIED | 63-Work fixture 的第 51–63 范围对象已做 390 direct/refresh/Back/Forward 诊断，不构成本包窄屏验收 |
| 读取失败 | 说读不到，不说没有作品 | VERIFIED | server-rendered fallback VERIFIED |
| Escape / 返回焦点 | 保留 filter 与既有 `sort=last`，关闭后回原 target | VERIFIED | 1440 验收基线 VERIFIED；390 键盘结果仅作诊断证据 |

## 3. 视觉工作条件

- 设计方向: LIDS v7 白场研究仪器；生命周期是唯一视觉核心；无渐变/玻璃/永久动画。
- 支持与响应式边界: 当前验收只以 1440 CSS px 桌面全屏为基线。既有隔离 loopback 曾覆盖 1440/1280，并用 CDP 强制 390×844 得到 `document/body/drawer=390/390`、drawer body `390/390`、panel `358/358`、figure `356/356`、receipt/controls `358/358`；这些 1280/390 数据现仅是诊断记录。一次 1280 测量发现 Inspector 右缘约超出 viewport 49.83 CSS px，Mog 已接受并移出本 Package。
- 可访问性: 四个真实 tab link；40px lifecycle controls；SVG `role=img`、title/desc；每点为具名链接。Corpus/Collection 代表 link、button、input、Work row 与 SVG point 在既有 1440/390 实际键盘遍历中均得到共享 `#335e72`、2px outline、2px offset，其中只有 1440 属当前验收基线。SVG 透明 hit circle 为 `r=6 + 24px non-scaling stroke`，有效外径 36px；可见点另有 4px stroke 与 scale，不以颜色为唯一指示；reduced-motion 关闭过渡。后加载页面仍有局部 focus 声明，但已验证表面的 computed focus 正确；Mog 接受该局部声明为后续技术债。
- Data Truth: `READY / INSUFFICIENT_OBSERVATION / NOT_APPLICABLE / READ_UNAVAILABLE / SCAN_LIMITED` 分开；UNKNOWN 不显示成 0。
- 截图/录屏: 仅保存在系统临时目录，不提交 Git。
- 历史诊断说明: Chrome headless 的 `--window-size=390` 实际 `innerWidth` 下限为 500；当时的 390 诊断证据因此来自 CDP `Emulation.setDeviceMetricsOverride(390×844)` 的 DOM 几何，并曾发现、修复 grid/flex min-content 与 receipt 长串换行问题。该记录不改变当前 1440 CSS px 支持基线。

## 4. 分层结论

| 完成层 | 状态 | 证据 | 限制 |
|---|---|---|---|
| 设计规格一致 | VERIFIED（源码/测试） | 四 tab、page-local token CSS、无禁用模块 | 原 reviewer FAIL 保留；本轮是 Mog 的范围/风险接受，不是独立 reviewer PASS |
| 前端/组件实现 | VERIFIED（branch） | server-rendered SVG + Corpus deep link | 不等于 shared runtime |
| 自动检查 | VERIFIED（branch） | `cargo test/check --workspace --all-targets --locked`、fmt、双 JS syntax、diff 与两项 governance 全通过 | PostgreSQL ignore suites 另以本卡隔离运行 |
| 隔离真实页面 | VERIFIED（脱敏 fixture） | PostgreSQL 16 + loopback `:3108/:3116`；1440 支持基线的 DOM/交互通过；1280/390 历史诊断另覆盖 direct/refresh/history、Tab/focus、query-invalid 与 lifecycle hit target | 1280/390 不属于本 Package 验收；不是当前真实数据或 shared runtime 证明；两个 runtime、browser tab/viewport 与专用数据库均已清理 |
| local `:3000` runtime refresh | AUTHORIZED / NOT VERIFIED | 未执行；待 root 在 merge 后刷新 | 不由本报告代替运行证明 |
| shared DB / migration / external deploy | NOT VERIFIED / 未授权 | 未执行 | Claim 明确排除 |
| Mog / 业务验收 | NOT VERIFIED | Draft PR 后由 Mog 验收 exact head | 不由本报告替代 |

## 5. 发现与后续

- 无 DECISION_REQUIRED；用户已明确 Evidence 只在 Corpus、排除监控价值。
- 旧服务端为 5 点窗口，旧前端为 15 点重复计算；实现以服务端 `trailing-5-work-median-v1` 为准且不在前端重算。
- 生命周期查询最多扫描 2000 + 1 探针，因此页面只在 creator overview 调用；其它 tab/keyword 不放大读取。
- 隔离 PostgreSQL 16：lifecycle 5/5、API full-path 1/1 通过；固定上海边界同时验证投影展示为 `2026-06-07` 与 `2026-09-04`，共享 Current parity 覆盖相同 `observed_at` tie。
- Remediation 390 实测：document/body/drawer `390/390`、panel `358/358`、figure `356/356`；Escape 返回 `?filter=creator&sort=last#target-…` 并恢复 opener 焦点；非法 query 无 active 默认项；`dtab=evidence` 显示 Overview 且有点；console warn/error 0。
- Remediation 3 以 63 个脱敏 Work 构造首批外 detail：`30000000-0000-4000-8000-000000000048` 在 390 direct/refresh 后均显示「补充作品 48」，drawer open、Inspector rect `23.41–390px`、document `390/390`；从 Collection 标记进入后单次 Back 立即返回，Forward 精确恢复。Corpus/Collection desktop+390 computed focus 全部为 2px solid `rgb(51,94,114)` / offset 2；`QUERY_INVALID` 非焦点状态样式未变；console logs `[]`。
- 支持边界不追溯改写以上证据：1280/390 现在只作诊断记录；1280 Inspector 约 49.83 CSS px 右侧裁切和页面局部 focus 声明由 Mog 明确接受并移出本包。核心事实、数据库读模型与代码 checks 已过，但 merge、`:3000` runtime 刷新与 Mog 实际使用验收仍待 root 后续证明。
- Workspace：`cargo test --workspace --all-targets --locked`、`cargo check --workspace --all-targets --locked`、`cargo fmt --all -- --check`、`git diff --check`、两个 JS syntax、`scripts/check-project-governance.sh`、`scripts/verify-ui-design-handbook.sh` 全通过；既有 API 16 项 dead-code warning 保留。
- `:3108/:3116`、浏览器 tab/viewport、专用 browser DB 与 isolated PostgreSQL container/volume 已清理。
- 不得由本报告推断真实平台历史完整、共享运行时已切换、Issue 已关闭或 Mog 已验收。
