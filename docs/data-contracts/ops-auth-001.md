# OPS-AUTH-001 · 内部团队身份与会话合同

> 状态: 权威当前
> 最后核对: 2026-10-23
> 适用范围: 内部邀请制成员身份、角色、会话、登录、退出、停用即时失效，以及浏览器与机器调用边界。
> 事实来源: Mog 提供的 `Intelligence_Ops_Launch_2026-10-23` 实施包；[OPS-LAUNCH-001](../plans/active/ops-launch-001.md)；现有 React/Axum/producer 路由核对。
> 冲突时以谁为准: Mog 最新确认、真实安全行为、`AGENTS.md`、OPS-LAUNCH-001；旧 localhost 守卫和假登录页不构成身份合同。

## 1. 不变量

- 不提供公共注册、首位注册者升级或浏览器自报角色/负责人；首位负责人由受控初始化建立，真实名单与初始化输入均不进 Git。
- 角色只有 `admin`、`lead`、`operator`；“运营助理”映射为 `operator`，职位名称不赋权。全体有效成员可读全队运营数据，写命令在服务端逐条再判角色、当前责任、组关系和对象状态。
- 成员停用立即撤销其全部会话；交接未完成不恢复访问。历史贡献记录稳定成员引用和当时归属，绝不因改名、交接或停用而漂移。
- 浏览器会话与 plugin/producer/station 的安装身份、租约和机器路由完全分离；给浏览器加身份不得改变机器协议。

## 2. 持久化与机密边界

`linggan_ops_member` 保存稳定 `member_ref`、规范化电子邮箱、显示名、角色、状态、密码哈希、创建/停用时间和乐观版本。密码只保存 Argon2id 哈希；日志、JSON 响应、审计字段和 Git 禁止出现明文密码。

`linggan_ops_session` 保存稳定会话引用、仅令牌 SHA-256 摘要、成员引用、签发/最近使用/过期/撤销时间。原始令牌只在成功登录的响应 `Set-Cookie` 出现；不能从数据库、响应正文或日志读回。邀请和重设令牌沿同一“摘要、单次、限时、可撤销”规则，在后续成员管理切片加入。

会话默认有效期为 7 天，cookie 名为 `linggan_ops_session`，`HttpOnly; SameSite=Strict; Path=/`；HTTPS 运行必须额外 `Secure`。服务以受信反向代理的 `X-Forwarded-Proto: https` 或 `LINGGAN_OPS_COOKIE_SECURE=1` 选择该属性；本机 `http://localhost`/`127.0.0.1` 开发链允许省略 `Secure`，但这不是生产豁免。所有会话响应是 `Cache-Control: no-store` 并设 `X-Content-Type-Options: nosniff`。

## 3. HTTP 表面与失败词典

| 接口 | 行为 | 成功 | 不泄露的失败 |
| --- | --- | --- | --- |
| `POST /api/local/auth/login` | 以 email + password 建立会话 | `200`，只返回成员公开身份；cookie 写入 | `401 ops_auth_invalid_credentials`，不存在、停用、密码错误使用同一结果 |
| `GET /api/local/auth/session` | 读取当前浏览器成员 | `200`，公开身份与角色 | `401 ops_auth_session_required`；过期/撤销/停用不区分给浏览器 |
| `POST /api/local/auth/logout` | 撤销当前会话并清 cookie | `204`，重放亦安全 | 无有效 cookie 同样 `204` |

写接口必须是同源请求：服务端验证本机受信 Host、`Sec-Fetch-Site` 和（存在时）`Origin`；不接受跨站来源。登录、退出或会话失败不会说明账户是否存在。密码重设、邀请签收、成员/组管理和资源责任判定将在其实现切片增加明确接口，不能借用登录接口的输入或响应字段。

## 4. 授权接入顺序

认证切片先建立数据模型、三条会话接口与隔离 PostgreSQL 证明；随后将业务 React 页面、运营 JSON/媒体路由接入会话中间件，再为管理员、组长、运营分别收紧写命令。任何被保护的读写均不得回退到 localhost 判断、隐藏按钮或客户端 actor。

在尚未接入会话中间件的过渡期，现有页面仍是旧本机开发面；这只能说明切片尚未完成，不能被标为 A01–A08 通过。

## 5. 最小可证伪验收

- 用隔离 PostgreSQL 写入已启用成员的 Argon2id 哈希：正确凭据得到仅 HttpOnly cookie；错误凭据、停用成员和损坏 hash 不泄露存在性。
- 以该 cookie 读取 session 成功；撤销、过期或停用后均被拒绝；退出后 cookie 失效且同一会话不能复用。
- 不带 cookie、伪造 cookie、跨站 `Origin` 均不能获得成员身份；producer 路由在同一测试中不因浏览器 cookie 逻辑而改变。

这些证明是认证数据与 HTTP 切片的完成条件，不等于真实邮件、首位负责人、真实团队名单、全站授权接入或团队试用完成。
