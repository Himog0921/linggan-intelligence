# OPS-TEAM-RESPONSIBILITY-001 · 团队组与账号当前责任

> 状态: 权威当前
> 最后核对: 2026-10-23
> 适用范围: `0147_ops_team_responsibility` 的团队组、成员组事件、账号当前责任及责任变更回执。
> 事实来源: OPS-LAUNCH-001 §3.1/§4；迁移 `0147_ops_team_responsibility.sql`；隔离 PostgreSQL 证明。
> 冲突时以谁为准: Mog 最新确认、OPS-LAUNCH-001、真实 schema 与后续已验证的授权服务。

## 1. 分离的事实

`linggan_ops_member` 是可登录的稳定团队身份；`linggan_ops_group` 是 Linggan 自己的运营组；
`linggan_ops_group_membership_event` 是成员加入/离开的追加式历史。当前归属由该成员最新事件重建，
不能以历史工作台中的姓名或负责人编号推断。

`linggan_ops_account.current_responsible_member_ref` 是账号**现在**由谁负责；它明确不同于
`responsible_user_source_id`，后者仅是迁入工作台的来源证据。每个首次分配、交接或取消分配都必须在
`linggan_ops_account_responsibility_change` 留下追加式回执，包含前后成员、实际操作者、理由、请求号、结果版本和时间。

## 2. 不变量

- 组名、状态和版本属于 Linggan，不重写历史导入来源；禁用组保留记录，不删除。
- 组成员事件和账号责任回执只追加，数据库拒绝更新或删除。
- 账号当前负责人可为空，表示待接管；这不等于把历史发布人、历史业务归属或导入来源清空。
- 责任命令必须在一个事务里比对账号 `responsibility_version`、验证当前会话/角色/当前组关系、写回执并推进版本。
  客户端 actor、组或版本不得成为授权事实。

## 3. 当前边界

本迁移只建立可审计的结构；尚未提供成员组管理、账号交接、停用保护或原生账号/发布/日报写 HTTP。
因此它不单独证明管理员/组长/运营的完整权限矩阵，也不授权写入历史迁入值、同步请求或生产数据库。

隔离 PostgreSQL 证明已验证完整 schema 可应用、组成员事件及责任回执可写但不可改删，以及当前负责人写入不会覆盖历史来源字段。后续服务必须补充并发交接、跨组拒绝、停用成员拒绝和历史不漂移的行为证据。
