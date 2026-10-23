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
- `POST /api/local/ops/native/accounts/{accountRef}/responsibility` 的请求含 `requestId`、
  `nextMemberRef`（管理员可为 `null` 以标记待接管）、`expectedVersion` 与原因。相同请求只回放
  原回执；版本陈旧、跨组、停用接任人和无权限均不写责任历史。
- 管理员可在有效账号间转交或标记待接管；组长只能从当前同组负责人转给同组的有效成员；运营没有
  交接权限。每次真实变化同时写 `linggan_ops_native_write_request` 与责任变更回执。

## 3. 当前边界

`0153_ops_account_responsibility_command` 已把账号责任交接接进受会话保护的原生命令边界，并以隔离
Axum → PostgreSQL 用例验证同组组长转交、管理员待接管、幂等重放、跨组拒绝、停用接任人拒绝和版本冲突。
账号读模型现已同时返回当前负责人稳定 ID、显示名和责任版本；账号台账页面把它与历史迁入运营人分开，
并通过受控成员候选列表提供交接入口。候选列表只是便利读取，服务端仍重新判定权限和组边界。
`0154_ops_member_deactivation` 已提供管理员发起的版本化、幂等停用回执：停用会撤销目标成员
全部浏览器会话、待接受邀请及待用重设令牌，并将其当前负责账号置为待接管、追加责任变更历史；它不改写
历史迁入运营人、发布人或业务归属。最后有效管理员受事务锁保护，停用后的交接失败不会恢复访问。
它仍没有成员/分组管理、停用 UI、离职交接预览、在途同步资格裁决或任何生产处置；这些仍需独立的服务和
证明。因此它不单独证明 H01–H07 全部通过，也不授权写入历史迁入值、同步请求或生产数据库。

隔离 PostgreSQL 证明已验证完整 schema 可应用、组成员事件及责任回执可写但不可改删，以及当前负责人写入不会覆盖历史来源字段。后续服务必须补充并发交接、跨组拒绝、停用成员拒绝和历史不漂移的行为证据。
