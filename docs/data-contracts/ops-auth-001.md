# OPS-AUTH-001 · 内部团队身份与会话合同

> 状态: 权威当前
> 最后核对: 2026-10-23
> 适用范围: 内部邀请制成员身份、角色、会话、登录、退出、停用即时失效，以及浏览器与机器调用边界。
> 事实来源: Mog 提供的 `Intelligence_Ops_Launch_2026-10-23` 实施包；[OPS-LAUNCH-001](../plans/active/ops-launch-001.md)；现有 React/Axum/producer 路由核对。
> 冲突时以谁为准: Mog 最新确认、真实安全行为、`AGENTS.md`、OPS-LAUNCH-001；旧 localhost 守卫和假登录页不构成身份合同。

## 1. 不变量

- 不提供公共注册、首位注册者升级或浏览器自报角色/负责人；首位负责人由受控初始化建立，真实名单与初始化输入均不进 Git。
- 角色只有 `admin`、`lead`、`operator`；“运营助理”映射为 `operator`，职位名称不赋权。运营台账的读写按角色、当前责任、组关系和对象状态校验。范围见[团队责任合同](ops-team-responsibility-001.md)。
- 成员停用立即撤销其全部会话；交接未完成不恢复访问。历史贡献记录稳定成员引用和当时归属，绝不因改名、交接或停用而漂移。
- 浏览器会话与 plugin/producer/station 的安装身份、租约和机器路由完全分离；给浏览器加身份不得改变机器协议。

## 2. 持久化与机密边界

`linggan_ops_member` 保存稳定 `member_ref`、规范化电子邮箱、显示名、角色、状态、密码哈希、创建/停用时间和乐观版本。密码只保存 Argon2id 哈希；日志、JSON 响应、审计字段和 Git 禁止出现明文密码。

`linggan_ops_session` 保存稳定会话引用、仅令牌 SHA-256 摘要、成员引用、签发/最近使用/过期/撤销时间。原始会话令牌只在成功登录的响应 `Set-Cookie` 出现；不能从数据库、响应正文或日志读回。邀请和重设令牌沿同一“摘要、单次、限时、可撤销”规则。邀请原始令牌仅会返回给当前已认证管理员以便复制到受控投递通道；响应禁止缓存，数据库、日志和后续读取接口均不可读回该原文。`0153_ops_first_admin_bootstrap` 额外保存唯一的受控控制台初始化回执；它的 singleton 不能因成员停用或删除尝试而重新打开首位管理员路径，详见 [OPS-FIRST-ADMIN-BOOTSTRAP-001](../runbooks/ops-first-admin-bootstrap-001.md)。

会话默认有效期为 7 天，cookie 名为 `linggan_ops_session`，`HttpOnly; SameSite=Strict; Path=/`；HTTPS 运行必须额外 `Secure`。服务以受信反向代理的 `X-Forwarded-Proto: https` 或 `LINGGAN_OPS_COOKIE_SECURE=1` 选择该属性；本机 `http://localhost`/`127.0.0.1` 开发链允许省略 `Secure`，但这不是生产豁免。个人改密在锁定成员、核验当前密码、更新 Argon2id 哈希、撤销全部旧会话和签发新会话的同一事务中完成；登录也会在同一事务中以与改密冲突的成员行锁核验密码并写入会话，因此先完成的登录会被随后的改密撤销，先完成的改密会使旧密码登录失败。所有会话响应是 `Cache-Control: no-store` 并设 `X-Content-Type-Options: nosniff`。

## 3. HTTP 表面与失败词典

| 接口 | 行为 | 成功 | 不泄露的失败 |
| --- | --- | --- | --- |
| `POST /api/local/auth/login` | 以 email + password 建立会话 | `200`，只返回成员公开身份；cookie 写入 | `401 ops_auth_invalid_credentials`，不存在、停用、密码错误使用同一结果 |
| `GET /api/local/auth/session` | 读取当前浏览器成员 | `200`，公开身份与角色 | `401 ops_auth_session_required`；过期/撤销/停用不区分给浏览器 |
| `POST /api/local/auth/logout` | 撤销当前会话并清 cookie | `204`，重放亦安全 | 无有效 cookie 同样 `204` |
| `POST /api/local/auth/password/change` | 已登录成员以当前密码设置新密码 | `200`，撤销该成员所有旧会话并签发一枚新 cookie | `401 ops_auth_invalid_credentials` 当前密码不匹配；`422` 空或超长输入 |
| `POST /api/local/auth/invitations` | 管理员创建或重邀待加入成员 | `201`，只向该管理员一次性返回可复制令牌；7 天有效 | `401` 未登录、`403` 非管理员、`409` 已启用/已停用成员、`422` 无效输入 |
| `POST /api/local/auth/invitations/accept` | 以有效邀请首次设密、激活成员并建立会话 | `200` 与 HttpOnly 会话 cookie | `410` 伪造/过期/撤销链接，`409` 已消费链接，`422` 格式错误 |
| `GET /api/local/auth/members` | 管理员读取成员、邀请状态、账号数和最近已接受停用的引用 | `200`，仅管理字段 | `401` 未登录、`403` 非管理员 |
| `POST /api/local/auth/invitations/{inviteRef}/revoke` | 管理员撤销仍可签收的邀请 | `200`，返回撤销结果 | `403` 无权、`409` 已签收或已失效 |
| `GET /api/local/auth/members/{memberRef}/offboarding-preview` | 管理员读取计划交接账号及其责任版本 | `200`，只读预览 | `403` 无权、`404` 无成员 |
| `POST /api/local/auth/members/{memberRef}/deactivate` | 管理员提交计划交接并停用，或紧急停用 | `200`，持久回执 | `403` 无权、`409` 计划变化或请求冲突、`422` 无效输入 |
| `GET /api/local/auth/deactivations/{requestRef}` | 管理员按请求 ID 查询该次回执 | `200`，原操作结果 | `403` 无权、`404` 尚无此回执 |

写接口必须是同源请求：服务端验证本机受信 Host、`Sec-Fetch-Site` 和（存在时）`Origin`；不接受跨站来源。登录、退出或会话失败不会说明账户是否存在。邀请只绑定服务器上创建的邮箱、显示名和角色，签收请求不能自行提交或提升角色；重邀在同一事务中撤销旧未消费令牌且不创建第二个成员身份。首位管理员不走 HTTP：受控本地命令仅在空团队和已验证 `0153` schema 下创建一次 `invited/admin` 身份及标准邀请，令牌从 stdin 接收、只存摘要，后续仍走同一签收路径。当前仅实现受控复制令牌，真实邮件投递与邮箱持有验证仍须在测试接收器/获授权的正式邮件环境独立证明。个人改密不等于忘记密码：后者的 1 小时令牌、受控投递和签收仍未实现。邀请状态和撤销属于管理员名册。受邀成员签收与撤销通过同一邀请行互斥。

已接入会话的业务表面是 `/ops` 入口、`/ops/overview`、`/ops/publications`、`/ops/accounts`、`/ops/daily`、`/ops/io`，以及既有 `/api/local/ops/*`、`/ops/covers/{publication_ref}` 路由；它们先经过同源守卫，再由服务端查询活动成员会话。`/ops/creation` 和 `/ops/api/creation/*` 仍是既有内容创作工作台，故意不被本切片接管。当前目标保存仅增加 active-session 身份边界，最终由哪一类角色可设置目标及实际操作者留痕，必须随原生运营写模型一并实现，不能把本次读取/会话接入表述为完整写权限。

## 4. 停用与写入

停用提交按稳定成员身份判断。事务中验证管理员的当前会话、目标成员及已有回执。紧急停用只写身份、会话、待用令牌和停用回执。计划交接增加完整账号清单及接任资格检查，并在同一事务提交账号责任变更。账号接管、组长代管和迁入发布范围见[团队责任合同](ops-team-responsibility-001.md)。

运营账号、发布、阅读量、日报及新同步请求的写事务会重新核对发起会话。停用提交后，旧会话的新写入失败。已经接纳的机器同步按[精确发布同步合同](ops-publication-sync-001.md)执行。

## 5. 最小可证伪验收

- 用隔离 PostgreSQL 写入已启用成员的 Argon2id 哈希：正确凭据得到仅 HttpOnly cookie；错误凭据、停用成员和损坏 hash 不泄露存在性。
- 管理员创建邀请、重邀、受邀成员签收与首次设密处于同一隔离 PostgreSQL + Axum 场景：非管理员不可发邀，旧链接失效，同一邮箱只保留一个成员引用，令牌仅存摘要，签收后才有会话。
- 以该 cookie 读取 session 成功；撤销、过期或停用后均被拒绝；退出后 cookie 失效且同一会话不能复用。
- 用两个同一成员的有效会话改密：错误当前密码不改变两者；成功后两枚旧 cookie 都被拒绝、仅新 cookie 可用，旧密码登录失败而新密码登录成功。
- 不带 cookie、伪造 cookie、跨站 `Origin` 均不能获得成员身份；producer 路由在同一测试中不因浏览器 cookie 逻辑而改变。
- 对 Ops 入口、五个台账 React 页、运营 JSON/目标/封面路由，不带 cookie 均为 `401`；外部 Host 在会话检查前为 `403`；有效成员可走 `/ops` 的既有重定向；创作工作台入口保持原边界。

这些证明是认证数据与 HTTP 切片的完成条件，不等于真实邮件、首位负责人、真实团队名单、全站授权接入或团队试用完成。
