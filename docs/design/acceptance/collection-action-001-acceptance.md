# ACC-COLLECTION-ACTION-001 · Keyword Drawer 主操作一致性验收

> 状态: 一次性报告
> 最后核对: 2026-09-15
> 适用范围: Issue #283 / PR #284 的 `/collection/targets` keyword Drawer
> 事实来源: 当前交付分支、PAGE-COLLECTION-001、LIDS、focused render test 与本次复审
> 冲突时以谁为准: 真实代码/测试/运行结果与用户最新确认

## 对象与场景

- 页面/组件: keyword Drawer 概览、列表行的同一主操作判据。
- 环境: PR #284 专属 worktree；不连接共享数据库、runtime、插件或平台。
- 测试输入是明确的 `KeywordArchiveRead` / `TargetInspectorView` 状态，只证明渲染合同，不是现实观察事实。

| 场景 | 用户任务 | 预期状态含义 | 视觉检查重点 | 真实后果/回执 | 结果 |
|---|---|---|---|---|---|
| inspector 可读 + 未建档 | 找到下一步 | 显示“需要建立档案” | 既有 Drawer 决策区显示一个主按钮 | POST 仍由后端重算 | 既有测试覆盖 |
| inspector 可读 + 待补详情 | 继续补齐 | 显示“有详情缺口需要补采” | 不写成已完成或无需处理 | 同上 | 既有测试覆盖 |
| inspector 不可读 + 未建档 | 不因巡查读失败失去建档入口 | 巡查/最近结果未知；建档态已独立读到 | 不可读提示与“建立档案”同屏 | 同上 | SOURCE VERIFIED：focused render test |
| inspector 不可读 + 待补详情 | 不因巡查读失败失去补采入口 | 巡查/最近结果未知；详情缺口已独立读到 | 不可读提示与“补采缺口”同屏 | 同上 | SOURCE VERIFIED：focused render test |
| 建档态不可读 | 不伪造下一步 | 建档态未知，不提供 archive 写操作 | 中文未知边界可见 | N/A | 既有分支保留 |

## 分层结论

| 完成层 | VERIFIED / NOT VERIFIED / N/A | 证据 | 仍有限制 |
|---|---|---|---|
| 设计规格一致 | SOURCE VERIFIED | PAGE-COLLECTION-001 + manifest；未知提示与独立建档事实同屏 | 实页视觉待验 |
| 前端/组件实现 | SOURCE VERIFIED | focused render test | 不证明路由数据读取 |
| 自动检查 | VERIFIED | focused render test、API 220 项、Evidence 70 项、crate check、invariant / governance / UI handbook / diff check | 隔离 PostgreSQL 与浏览器另列 |
| 真实链路/回执 | NOT VERIFIED | 本次不发起 POST | 未验证真实 receipt |
| 部署 | NOT VERIFIED | 未切 runtime | main 合并不等于部署 |
| Mog / 业务验收 | NOT VERIFIED | 待前端验收 | 不由自动测试替代 |

本记录不证明 inspector 读取已经恢复、关键词档案完整、平台采集成功、按钮提交成功、shared runtime 已更新，或 Mog 已完成业务验收。
