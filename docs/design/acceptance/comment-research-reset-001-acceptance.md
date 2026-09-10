# ACC-COMMENT-RESEARCH-RESET-001 · 评论研究 V1 验收记录

> 状态: 一次性报告
> 最后核对: 2026-09-10
> 适用范围: COMMENT-RESEARCH-RESET-001 的隔离数据库、API、worker、浏览器与本机运行时验收
> 事实来源: 当前交付分支代码与后续命令回执
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
| HTTP / UI | 部分 PASS（自动） | 独立 endpoint、无旧 route、本地 API guard 已验证；真实浏览器点击和 tab P95 尚待 runtime |
| shared 开发库清理 | 未完成 | explicit table deletion receipt；raw/comment attribution/model ledger 留存核验 |
| localhost :3000 切换 | 未完成 | runtime exact head、migration、健康检查和浏览器验收 |
| Mog 业务验收 | 未完成 | 用户亲自验证一轮已配置模型的评论研究结果 |

本页不将代码完成、部署成功或 HTTP 200 写成用户验收或真实模型语义质量。
