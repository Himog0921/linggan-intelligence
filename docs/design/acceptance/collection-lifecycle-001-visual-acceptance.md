# ACC-COLLECTION-LIFECYCLE-001 · Creator 生命周期抽屉验收记录

> 状态: 一次性报告
> 最后核对: 2026-09-03
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
| 选中作品 | 只显示最小摘要并跳 Corpus | VERIFIED | 390 DOM：point link `tabIndex=0`、具名、精确 deep link |
| Work 不在首批列表 | 直读目标 detail，不回退第一项 | VERIFIED | JS source contract VERIFIED |
| 读取失败 | 说读不到，不说没有作品 | VERIFIED | server-rendered fallback VERIFIED |

## 3. 视觉工作条件

- 设计方向: LIDS v7 白场研究仪器；生命周期是唯一视觉核心；无渐变/玻璃/永久动画。
- 响应式: 隔离 loopback 在 1440/1280 截图走查通过；CDP 强制 390×844 后 `document/body/drawer` 均为 `scrollWidth=clientWidth=390`，drawer body 为 `390/390`，panel 为 `358/358`，figure 为 `356/356`，receipt/controls 为 `358/358`。
- 可访问性: 四个真实 tab link；40px lifecycle controls；SVG `role=img`、title/desc；每点为具名链接；focus-visible；reduced-motion 关闭过渡。
- Data Truth: `READY / INSUFFICIENT_OBSERVATION / NOT_APPLICABLE / READ_UNAVAILABLE / SCAN_LIMITED` 分开；UNKNOWN 不显示成 0。
- 截图/录屏: 仅保存在系统临时目录，不提交 Git。
- 视口说明: Chrome headless 的 `--window-size=390` 实际 `innerWidth` 下限为 500，不能作为 390 证明；最终窄屏证据来自 CDP `Emulation.setDeviceMetricsOverride(390×844)` 的 DOM 几何。首次窄屏走查发现并修复 grid/flex min-content 与 receipt 长串换行问题。

## 4. 分层结论

| 完成层 | 状态 | 证据 | 限制 |
|---|---|---|---|
| 设计规格一致 | VERIFIED（源码/测试） | 四 tab、page-local token CSS、无禁用模块 | 仍待 Mog 视觉判断 |
| 前端/组件实现 | VERIFIED（branch） | server-rendered SVG + Corpus deep link | 不等于 shared runtime |
| 自动检查 | VERIFIED（branch） | `cargo test/check --workspace --all-targets --locked`、fmt、JS syntax、两项 governance 全通过 | PostgreSQL ignore suites 另以本卡隔离运行 |
| 隔离真实页面 | VERIFIED（脱敏 fixture） | PostgreSQL 16 + loopback `:3108`；1440/1280 视觉、CDP 390 DOM/overflow/a11y/deep-link | 不是当前真实数据或 shared runtime 证明；实例已清理 |
| shared DB/runtime/deploy | NOT VERIFIED / 未授权 | 未执行 | Claim 明确排除 |
| Mog / 业务验收 | NOT VERIFIED | Draft PR 后由 Mog 验收 exact head | 不由本报告替代 |

## 5. 发现与后续

- 无 DECISION_REQUIRED；用户已明确 Evidence 只在 Corpus、排除监控价值。
- 旧服务端为 5 点窗口，旧前端为 15 点重复计算；实现以服务端 `trailing-5-work-median-v1` 为准且不在前端重算。
- 生命周期查询最多扫描 2000 + 1 探针，因此页面只在 creator overview 调用；其它 tab/keyword 不放大读取。
- 隔离 PostgreSQL 16：lifecycle 4/4、API 1/1 通过；固定上海边界同时验证投影展示为 `2026-06-07` 与 `2026-09-04`。临时容器和 `:3108/:9223` 监听均已清理。
- Workspace：`cargo test --workspace --all-targets --locked`、`cargo check --workspace --all-targets --locked`、`cargo fmt --all -- --check`、`git diff --check`、`node --check apps/api/src/local_web/evidence_library.js`、`scripts/check-project-governance.sh`、`scripts/verify-ui-design-handbook.sh` 全通过；既有 API dead-code warning 保留。
- 不得由本报告推断真实平台历史完整、共享运行时已切换、Issue 已关闭或 Mog 已验收。
