# ACC-PLUGIN-002 · PLUGIN-MIGRATION-001 Popup 验收

> 状态: 一次性报告
> 最后核对: 2026-08-25
> 适用范围: Issue #37 popup 的静态 source、adapter、mock loopback receipt 与 release 验收；不替代浏览器或真实采集验证
> 事实来源: `PAGE-PLUGIN-001`、PLUGIN-MIGRATION-001 UI Change Manifest、实际 build/static checks 与 Issue #37
> 冲突时以谁为准: 真实插件加载/运行、当前合同和用户确认；本记录不把模拟检查说成真实链路

| 层级 | 实际结果 | 未证明边界 |
|---|---|---|
| 任务可用 | `npm run check` 的 adapter 与 mock ingress 测试覆盖可见卡顺序、20 条限额、条件不符不提交和 receipt 分支 | 浏览器/XHS 当前页面、用户点击 |
| 状态诚实 | source/static audit 将 local readiness、`NOT_READY`、receipt 与 Evidence/页面结果分离 | 实际 host 或数据库接纳 |
| 视觉一致 | popup CSS 只消费打包 `--lgi-*` token；单一 L1 readout/action 结构通过静态检查 | 浏览器截图、响应式与辅助功能走查 |
| 真实后果 | 固定 loopback、最小权限、无旧工作台/禁止能力的 source/release audit 通过 | 真实 XHS、真实 ingestion、Evidence Library 展示 |

该验收只证明静态包与 mock 路径；不证明安装、真实平台页面、账号状态、持久化、媒体或研究链路。
