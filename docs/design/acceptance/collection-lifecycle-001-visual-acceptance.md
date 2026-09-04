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

## 2. 场景矩阵

| 场景 | 预期含义 | 自动/隔离结果 | 视觉/交互结果 |
|---|---|---|---|
| creator 有合格点 | 默认看见真实发布时间散点、原值与 5 点中位线 | VERIFIED | 隔离脱敏 3 点 fixture 在 1440/1280 VERIFIED |
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
| 选中作品 | 只显示最小摘要并跳 Corpus | VERIFIED | 390 DOM：point link `tabIndex=0`、具名、精确 deep link |
| Work 不在首批列表 | 直读目标 detail，不回退第一项；390px 自动打开 Inspector，不新增 history | VERIFIED | 63-Work fixture 的第 51–63 范围对象：direct/refresh/Back/Forward VERIFIED |
| 读取失败 | 说读不到，不说没有作品 | VERIFIED | server-rendered fallback VERIFIED |
| Escape / 返回焦点 | 保留 filter 与既有 `sort=last`，关闭后回原 target | VERIFIED | 390 浏览器键盘 VERIFIED |

## 3. 视觉工作条件

- 设计方向: LIDS v7 白场研究仪器；生命周期是唯一视觉核心；无渐变/玻璃/永久动画。
- 响应式: 隔离 loopback 在 1440/1280 截图走查通过；CDP 强制 390×844 后 `document/body/drawer` 均为 `scrollWidth=clientWidth=390`，drawer body 为 `390/390`，panel 为 `358/358`，figure 为 `356/356`，receipt/controls 为 `358/358`。
- 可访问性: 四个真实 tab link；40px lifecycle controls；SVG `role=img`、title/desc；每点为具名链接。Corpus/Collection 代表 link、button、input、Work row 与 SVG point 在 1440/390 实际键盘遍历中均得到共享 `#335e72`、2px outline、2px offset。SVG 透明 hit circle 为 `r=6 + 24px non-scaling stroke`，有效外径 36px；可见点另有 4px stroke 与 scale，不以颜色为唯一指示；reduced-motion 关闭过渡。
- Data Truth: `READY / INSUFFICIENT_OBSERVATION / NOT_APPLICABLE / READ_UNAVAILABLE / SCAN_LIMITED` 分开；UNKNOWN 不显示成 0。
- 截图/录屏: 仅保存在系统临时目录，不提交 Git。
- 视口说明: Chrome headless 的 `--window-size=390` 实际 `innerWidth` 下限为 500，不能作为 390 证明；最终窄屏证据来自 CDP `Emulation.setDeviceMetricsOverride(390×844)` 的 DOM 几何。首次窄屏走查发现并修复 grid/flex min-content 与 receipt 长串换行问题。

## 4. 分层结论

| 完成层 | 状态 | 证据 | 限制 |
|---|---|---|---|
| 设计规格一致 | VERIFIED（源码/测试） | 四 tab、page-local token CSS、无禁用模块 | 仍待 Mog 视觉判断 |
| 前端/组件实现 | VERIFIED（branch） | server-rendered SVG + Corpus deep link | 不等于 shared runtime |
| 自动检查 | VERIFIED（branch） | `cargo test/check --workspace --all-targets --locked`、fmt、双 JS syntax、diff 与两项 governance 全通过 | PostgreSQL ignore suites 另以本卡隔离运行 |
| 隔离真实页面 | VERIFIED（脱敏 fixture） | PostgreSQL 16 + loopback `:3108/:3116`；1440/1280/390 DOM/overflow、direct/refresh/history、Tab/focus、query-invalid 与 lifecycle hit target | 不是当前真实数据或 shared runtime 证明；两个 runtime、browser tab/viewport 与专用数据库均已清理 |
| shared DB/runtime/deploy | NOT VERIFIED / 未授权 | 未执行 | Claim 明确排除 |
| Mog / 业务验收 | NOT VERIFIED | Draft PR 后由 Mog 验收 exact head | 不由本报告替代 |

## 5. 发现与后续

- 无 DECISION_REQUIRED；用户已明确 Evidence 只在 Corpus、排除监控价值。
- 旧服务端为 5 点窗口，旧前端为 15 点重复计算；实现以服务端 `trailing-5-work-median-v1` 为准且不在前端重算。
- 生命周期查询最多扫描 2000 + 1 探针，因此页面只在 creator overview 调用；其它 tab/keyword 不放大读取。
- 隔离 PostgreSQL 16：lifecycle 5/5、API full-path 1/1 通过；固定上海边界同时验证投影展示为 `2026-06-07` 与 `2026-09-04`，共享 Current parity 覆盖相同 `observed_at` tie。
- Remediation 390 实测：document/body/drawer `390/390`、panel `358/358`、figure `356/356`；Escape 返回 `?filter=creator&sort=last#target-…` 并恢复 opener 焦点；非法 query 无 active 默认项；`dtab=evidence` 显示 Overview 且有点；console warn/error 0。
- Remediation 3 以 63 个脱敏 Work 构造首批外 detail：`30000000-0000-4000-8000-000000000048` 在 390 direct/refresh 后均显示「补充作品 48」，drawer open、Inspector rect `23.41–390px`、document `390/390`；从 Collection 标记进入后单次 Back 立即返回，Forward 精确恢复。Corpus/Collection desktop+390 computed focus 全部为 2px solid `rgb(51,94,114)` / offset 2；`QUERY_INVALID` 非焦点状态样式未变；console logs `[]`。
- Workspace：`cargo test --workspace --all-targets --locked`、`cargo check --workspace --all-targets --locked`、`cargo fmt --all -- --check`、`git diff --check`、两个 JS syntax、`scripts/check-project-governance.sh`、`scripts/verify-ui-design-handbook.sh` 全通过；既有 API 16 项 dead-code warning 保留。
- `:3108/:3116`、浏览器 tab/viewport、专用 browser DB 与 isolated PostgreSQL container/volume 已清理。
- 不得由本报告推断真实平台历史完整、共享运行时已切换、Issue 已关闭或 Mog 已验收。
