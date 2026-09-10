# ACC-COMMENT-RESEARCH-RESET-001 · 评论研究 V1 验收记录

> 状态: 一次性报告
> 最后核对: 2026-09-10
> 适用范围: COMMENT-RESEARCH-RESET-001 的隔离数据库、API、worker、浏览器与本机运行时验收
> 事实来源: main、共享开发库、loopback runtime 与浏览器命令回执
> 冲突时以谁为准: 真实运行/数据库副作用、用户最新确认、AGENTS.md

## 已实现与自动证明

- V1 已提供 derivation、Atom、候选召回、受限归并、冻结结果和独立读取端点。
- 作者回复的 `作者` 徽标仅在身份已确认时从研究派生正文移除，原始正文保持不变。
- worker 只推进 V1 queue，不再从 legacy recovery、daily、Task B 或 P4 获取工作。
- 页面和模型设置页面已经替换旧用户路径；连续自动排程没有入口。
- `cargo check`、V1 Rust library test、Pi adapter protocol test、isolated PostgreSQL terminal migration、20 项 V1 PostgreSQL proof 与 2 项 API proof 均已通过。
- 终态 migration 明确移除了旧研究派生关系，保留 RawComment、作者归属、来源资格、通用模型配置与调用账本；没有使用 `CASCADE`。

## 尚未验收的层次

| 层次 | 状态 | 所需证据 |
|---|---|---|
| Rust 编译 / 静态 JS | PASS（自动） | `cargo check`、format、Pi adapter protocol test |
| isolated PostgreSQL 终态迁移 | PASS（自动） | full-history migration 至 0070、20 项 V1 PostgreSQL proof、2 项 API proof、资源清理 |
| Worker synthetic end-to-end | PASS（受控 fixture） | 逐阶段 Atom、向量候选、归并、失败隔离与 publish gate proof；不等于真实供应商语义质量 |
| HTTP / UI | PASS（初始运行态） | 浏览器实际打开 `/corpus/comments?view=overview`，看到五个 V1 tab、正确空态，控制台无 error；尚未有结果态和 tab P95 测量 |
| shared 开发库清理 | PASS（终态重置） | 0064–0070 已入台账；旧 Task B/P4/daily/replay/recovery relation 已不存在；Raw Comment 3,102 与 invocation ledger 129 均留存；V1 派生结果为 0 |
| localhost :3000 切换 | PASS（本机） | `runtime-main` 与 `origin/main` 同步，API/worker/media 均运行，`/health` database/schema `READY` |
| 向量模型配置 | BLOCKED（真实运行前置） | 当前只有 DeepSeek 文本模型，没有 qualified+enabled embedding；guarded revision 已由 isolated proof 覆盖页面禁用与服务端无向量 Run 拒绝，仍须在本次 runtime 切换后核验实际页面与 POST 回执，并先配置、测试实际 embedding provider |
| Mog 业务验收 | 未完成 | 用户亲自配置 embedding、保存策略并验证一轮真实评论研究结果 |

本页不将代码完成、部署成功或 HTTP 200 写成用户验收或真实模型语义质量。
