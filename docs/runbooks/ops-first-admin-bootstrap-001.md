# OPS-FIRST-ADMIN-BOOTSTRAP-001 · 首位运营管理员受控初始化

> 状态: 权威当前
> 最后核对: 2026-10-23
> 适用范围: 已迁移 `0152_ops_first_admin_bootstrap` 的本地 Linggan 运行库中，建立唯一首位 Ops 管理员的控制台动作。
> 事实来源: `0152_ops_first_admin_bootstrap.sql`、`scripts/local-runtime.sh`、OPS-AUTH-001、OPS-LAUNCH-001 与隔离 runtime proof。
> 冲突时以谁为准: Mog 对精确环境和名单的授权、实际 migration ledger、OPS-AUTH-001 与运行时命令的 fail-closed 行为。

## 1. 何时可以执行

这不是公共注册、HTTP 接口或部署后自动步骤。只有在已获该环境的首位管理员名单授权、服务已按受控迁移流程停止、`0152` 已由显式迁移写入同一数据库 ledger 时，才可由本地运行库操作者执行一次。

真实邮箱、显示名和令牌不写入 Git、工单、日志或运行回执。没有名单/环境授权时，保留未初始化状态；不得用合成姓名替代真实成员，也不得通过 SQL 手工插行绕开此命令。

## 2. 一次性输入与动作

操作者在受控终端生成 32 字节十六进制令牌，并把令牌、邮箱、显示名依次通过三行标准输入交给命令；三项都不会作为环境变量、命令参数或明文持久化。确认值必须精确为 `CREATE_FIRST_OPS_ADMIN`。

```bash
IFS= read -r -s -p 'Bootstrap token (64 hex): ' bootstrap_token; printf '\n'
IFS= read -r -p 'Authorized administrator email: ' bootstrap_email
IFS= read -r -p 'Authorized administrator display name: ' bootstrap_display_name
printf '%s\n%s\n%s\n' "$bootstrap_token" "$bootstrap_email" "$bootstrap_display_name" | \
  LINGGAN_OPS_BOOTSTRAP_CONFIRM=CREATE_FIRST_OPS_ADMIN \
  ./scripts/local-runtime.sh bootstrap-ops-admin
unset bootstrap_token bootstrap_email bootstrap_display_name
```

成功只创建一名 `invited` 状态的 `admin`、一张 7 天有效的标准邀请和一条不可改删的 bootstrap 回执；受邀人仍必须用该令牌经过普通邀请签收接口自行设定密码。命令不发送邮件、不建立浏览器会话、不打印令牌，也不完成邮箱持有验证。

## 3. 拒绝条件与核对

命令在下列任一情况失败且不写入：确认值/输入格式不正确、服务仍运行、数据库凭据或本机端口不可信、`0152` relation 或 ledger checksum 不匹配、任意 Ops 成员已存在，或 bootstrap 回执已存在。事务以 advisory lock 串行化并同时检查“无成员、无 bootstrap 回执”；第二次调用不能创建第二身份。

执行后仅核对受控数据库中的 bootstrap 回执、关联成员为 `admin/invited` 和邀请摘要存在。不要查询或记录原始令牌。接受邀请、真实邮件送达、HTTPS、实际团队使用、推送、合并和部署均是独立阶段，不能由此命令或隔离 proof 推断。
