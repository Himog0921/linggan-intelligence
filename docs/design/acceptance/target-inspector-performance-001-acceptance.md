# ACC-TARGET-INSPECTOR-PERFORMANCE-001 · 观察目标检查器验收

> 状态: 一次性报告
> 最后核对: 2026-09-13
> 适用范围: Issue #158 / branch `codex/target-inspector-performance-001`
> 事实来源: 当前交付分支、专属测试、disposable PostgreSQL 与隔离浏览器预览
> 冲突时以谁为准: 真实代码/数据库/浏览器结果、用户最新确认与 Issue Claim

## 验收对象

- 页面：`/collection/targets`、creator target Drawer、作品列表/表现、巡查。
- 视口：1440 CSS px 桌面全屏；窄屏不在本包支持范围，但不得破坏 Drawer 的既有边界。
- 数据前提：只读隔离库；合成来源必须标明不是 Evidence；不接触共享库或外部平台。

## 场景矩阵

| 场景 | 预期 | 结果 |
|---|---|---|
| 六目标选择 | 独立勾选、顶部数量、分组全选、勾选不打开 Drawer | BROWSER + SOURCE VERIFIED；浏览器勾选 2 项后按钮显示 `批量编辑 2`，弹窗显示 `已选择 2 个目标`；专属 UI 合同 8/8 |
| 正常/排队/执行 | queued 与 running 分开；自动处理中无人工按钮 | SOURCE + POSTGRES VERIFIED |
| 部分/阻塞 | 已取得事实保留，缺口与唯一救援动作可见 | SOURCE + POSTGRES VERIFIED |
| Known zero / Unknown | 零可见；未知不入图、不伪造零 | SOURCE + POSTGRES VERIFIED |
| 作品列表/表现 | URL 可恢复；列表查证、表现看分布 | BROWSER + SOURCE VERIFIED；`wview=performance` 可恢复，既有 lifecycle 回归 16/16 |
| 无分类 | 明示尚未建立内容分类 | BROWSER + SOURCE VERIFIED；空样本没有生成主题结论 |
| 1440/a11y | 主体列与操作同时可见；控件、focus、reduced motion、图表替代成立 | VERIFIED；1440×900 三张实拍；AX table/link/button 语义、关闭后目标 focus hash 与 0 browser error 已核对 |

## 分层结论

| 完成层 | 状态 | 证据/限制 |
|---|---|---|
| 设计规格一致 | VERIFIED ON MAIN + LIVE LOCAL PAGE | 1440×900 列表、概览、作品表现实拍；发布后 Chrome 实页再次核对新三 Tab、页头动作和已删除旧文案 |
| 源码实现 | VERIFIED ON MAIN | PR #205 exact head `e067bfb` 已合并为 `f0ed68d` |
| 自动检查 | VERIFIED | focused、workspace、格式、Node、diff、项目治理与 UI handbook 均通过 |
| 隔离 PostgreSQL | TARGET INSPECTOR VERIFIED | PostgreSQL 16 的新 inspector proof 1/1 证明 queued/running、Known zero/Unknown 与只读无副作用；完整旧脚本随后在未修改的 `collection_control_postgres` 两项基线断言处失败，不把整条脚本写成全绿 |
| 本机 API 发布 | VERIFIED | PR #205 合并为 `f0ed68d`；PID `54442` 实际来自 `runtime-main@f0ed68d`，health READY，Targets 200，Chrome 实页 6 目标与 0 console issue |
| Mog / 业务验收 | NOT VERIFIED | 需后续前端验收 |

## 不得推断

源码或截图通过不证明 shared runtime 已替换、采集成功、作品完整、分类已建立、跨账号价值成立或业务验收完成。

本次后续授权只刷新了本机 loopback API；没有新 migration，也没有重启 worker、重载插件、访问平台或触发真实采集。这里的 runtime 证明不扩张为那些层面的完成声明。

## 2026-09-13 · 趋势／分布复核补充

- **用户反馈与实现**：用户指出作品数据必须同时承担「趋势」和「分布」两种阅读逻辑，且折线需要受控色阶。逐篇散点因此从折叠详情提升为趋势后的常驻同级面；阅读次序为「趋势 → 分布 → 逐篇证据」，散点仍保留到每篇作品的可访问链接。趋势 SVG 与其图例仅在数据线中使用既有 `body / signal-ink / signal` 的低饱和墨色至信号橙过渡；它不代表另一条指标、状态或预测。
- **本次证据**：`cargo fmt --all -- --check`、`cargo build -p linggan-api` 及表现页 focused tests 已通过；本地 `:3001` source preview 对真实只读目标确认两张图同时可见，散点的逐点链接和 Unknown 排除说明仍在。
- **仍未证明**：这次 source preview 本身不证明 Git 提交、`main` 合并或 `:3000` 刷新；不改变此报告先前的 runtime 记录，更不证明共享数据库、worker、插件、外部平台、采集或 Mog 最终视觉验收。

## 2026-09-13 · 同画布切换修订（源码证据，未发布）

- **替代范围**：该修订替代上节“两张图同时常驻”和“数据线渐变”的表达。新的 `趋势｜分布` 共享同一 qualified / KNOWN 作品集与同一画布位置；趋势独有按周／按月，分布没有粒度控制。趋势的单一 Ink 中位线、淡色同序列填充和密度柱不形成第二指标；分布的 Y 轴始终跟随当前指标。
- **数据边界**：高讨论率只在评论、点赞均 Known 且点赞大于零时作为 `评论／点赞 >20%` 的点外圈；Known zero 与未知都不被推断为低讨论。所有点仍链接精确作品，当前指标 Known zero 仍纳入，当前指标 Unknown 仍列为 exclusion。
- **源码验证**：本轮 focused render / lifecycle 单测、格式、API check、UI handbook、governance 和 diff 检查的完整结果以本次工作回执为准。此前 `:3001` 的“真实目标图表可见”不适用于这一修订：本轮只读 source preview 的 health 为 READY，但 Targets 页读取为“观察目标当前未知”，没有可渲染 chart 的 target。因此浏览器视觉与端到端交互为 `NOT VERIFIED`。
- **发布边界**：当前修订仍在工作区，未提交、推送、合并、刷新 `:3000` 或改变共享数据库、worker、插件、外部平台和采集；Mog 视觉／业务验收继续为 `NOT VERIFIED`。

### 提交前独立复审返修（仍为源码证据）

- **已修正**：独立复审指出 `PAGE-COLLECTION-001` 与当前语义冲突（归属点填充、禁用所有分位数及禁止渐变）。页面规格已随本包同步：归属在表中查证，分布的墨点不承担确认程度；P25–P75 仅为当前纳入作品的阅读辅助；唯一渐变只允许趋势 SVG 的同序列面积填充。
- **数值边界**：此前新图的 `k` 缩写会让 `9,439` 失去精确读法。现改为 `<10,000` 直接带千分位显示、`≥10,000` 使用中文“万”，并为 SVG 轴、基准、趋势中位数和分布典型区间提供精确值 title / accessible label。趋势主线和新增环描边收敛为 LIDS 批准的 2px。
- **复核状态**：以上返修已通过格式、目标页 focused tests 与数值单测；需要在最终 exact commit 上由独立 reviewer 重新给出结论。浏览器图表、main、`:3000`、共享 DB 及 Mog 验收仍未发生。

## 完整 PostgreSQL 脚本保留失败

`./scripts/test-local-001-discovery-postgres.sh` 已实际运行。新增 `target_inspector_postgres` 1/1 通过，临时 database/container/volume 已清理；脚本后续在本分支未修改的 `collection_control_postgres` 留下两项失败：一项仍把迁移边界写死在 `0042`，而当前证明集合早已继续增长；另一项为按时点变化的 station daily-limit 预期与当前 available 结果不一致。它们不改变本包 inspector 专项证明，但阻止我们宣称完整 PostgreSQL harness 全绿。
