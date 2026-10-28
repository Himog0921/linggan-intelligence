# CREATION-ASSETS-001 · 创作资产（S1a）合同：账号身份、产品事实卡、衔接句库、检查与复制

> 状态: 代码事实优先
> 最后核对: 2026-10-28
> 适用范围: 创作模块资产版第一片 S1a 的后端候选：账号身份、产品事实卡、衔接句库、稿件检查（规则集 `creation-checks.v1`）、复制到小红书的文本与回执；不含经历与人设事实（S1b）、选题框与 AI（S2）、样板（S3）、标题与回收（S4）
> 事实来源: [DEC-0012](../decisions/0012-creation-asset-layer.md)；[创作流程规格](../product/creation-asset-workflow.md) 附录 A；`crates/intelligence/src/creation_assets/`；迁移 `0181_creation_assets_s1a.sql`；隔离 PostgreSQL 与 HTTP 证明
> 冲突时以谁为准: Mog 最新确认；可执行代码与测试；本合同随候选合并后生效，未合并前不代表线上行为

## 责任与身份

资产属于本机个人创作空间，actor 为 `local_machine`（与 [CONTENT-WORKBENCH-001](content-workbench-001.md) 一致）；表上的 `saved_by` 等列用检查约束限定为该值，成员登录（S0）上线时必须先放宽这些约束。复制不是发布：回执只说明哪一版稿件在哪套规则下检查过，确认只说明文字到达了剪贴板；“已发布”仍只由运营发布记录证明。接口全部走创作路由已有的同源守卫，不新增外部入口。

## 对象与版本

所有表追加式（更新与删除被触发器拒绝），更正以新修订追加。每个命令带 `requestId`：同一个 id 属于一种命令，原样重试得到原结果（`replayed: true`），同 id 不同内容返回 409，`linggan_creation_assets_request` 记录命令种类与指纹（不含 requestId）。

| 对象 | 身份与版本 | 要点 |
|---|---|---|
| 账号身份 | `accountRef` + `revision`（从 1 起） | `orgRelation`：`parent`/`coach`/`none`/`unknown`；`bioDisclosesRelation`、`hasRealPerson`、`canProvideExperience` 为三态（`null` 即未知，不是“否”）；`backingPersonRole` 只写角色，需 `hasRealPerson=true`；简介核对时间在答案不变时沿用。只列小红书、台账在册、状态为在用/养号中的账号 |
| 产品事实卡 | `CP-0001` 起的编码 + `revision` | 名称、可用叫法、禁用叫法（两者不得重叠）、是什么/适合谁/能讲/不能讲/必带提示。修订只含内容；**确认**是另一条追加记录，只有已确认的修订参与检查，新修订确认前沿用上一个已确认的修订 |
| 衔接句 | `XJ-0001` 起的编码 + `revision` | 身份分组 `parent`/`coach`、一行文字（≤500 字）、来源 `manual`/`saved_back`；入库时跑入库检查并保存结果；状态 `review`/`ok`/`off`/`fix` 是追加的状态事件，无事件即待审；任何改写生成新修订并回到待审；停用后重新启用回到待审 |
| 复制回执 | `receiptRef` | 稿件修订、规则集版本、所用事实卡修订、提醒与提示的规则计数、文本指纹；**不保存文本**。确认记录单独追加 |

稿件文档上新增的 `bridge`、`coverAi` 见 CONTENT-WORKBENCH-001。

## 接口（均在 `/ops/api/creation/` 下）

| 方法与路径 | 作用 | 备注 |
|---|---|---|
| `GET accounts?domainRef=` | 可写稿的账号及其最新身份修订 | |
| `PUT accounts/{id}/identity` | 追加身份修订 | 请求带 `expectedRevision`（无身份为 0），过期 409 `creation_assets_stale` |
| `GET accounts/{id}/candidates` | 该账号可用的已审核衔接句，按编码排序 | 只有身份已确认且简介已写明时才有；`state` 说明原因 |
| `GET fact-cards` / `POST fact-cards` / `PUT fact-cards/{code}` | 列表（最新与已确认修订）/ 新建 / 追加修订 | 新建 `expectedRevision=0` |
| `POST fact-cards/{code}/confirm` | 确认最新修订 | 已确认再确认为 400 |
| `GET bridge-sentences?group=&status=` | 每句的最新修订及状态、使用次数 | 使用次数 = 当前修订里带该编码的稿件数 |
| `POST bridge-sentences` / `PUT bridge-sentences/{code}` | 入库 / 改写 | 含疗效承诺的被拒，409 `creation_assets_blocked` 并附 `findings` |
| `POST bridge-sentences/{code}/review` | `approve`/`request_fix`/`disable`/`enable` | `request_fix` 必须有说明；越过流转 400 |
| `POST bridge-sentences/check` | 输入时的入库检查 | 不写任何东西 |
| `POST checks` | 对编辑器当前稿件（`{document}`）跑全部稿件规则 | 不写任何东西；文档先过同一套校验 |
| `POST works/{id}/copy` | 检查**已保存的当前修订**，通过则返回文本并写回执 | `expectedRevision` 须是当前修订，否则 409 stale；有必改 409 blocked 并附 `findings`；正文为空 400 |
| `POST works/{id}/copy/{receipt}/confirm` | 剪贴板成功后写确认 | 重复确认返回第一次 |
| `GET works/{id}/copy-status` | 最近一次已确认的复制 | 编辑器比较其 `revisionId` 与当前修订，区分“已复制”与“草稿中” |

错误形状沿用创作路由：顶层 `code`；blocked 另带 `findings`。过期、冲突、被拒、读不到分别是 409、409、409、503，不混用。

## 检查（规则集 `creation-checks.v1`）

结果 `findings[]`：`rule`、`level`（`must`/`warn`/`hint`）、`message`、`location`、`fixes[]`。`location` 的偏移是 UTF-16 码元（即 JavaScript 字符串下标）。同一规则每次最多 30 条。`notRun[]` 列出因条件不具备而没有运行的规则（没有已确认的事实卡时 R-W3、R-I2 不运行），**沉默不等于通过**。

- **必改（挡住复制）只有结构性规则**：R-M1 账号未选或身份未确认；R-M2 简介没写明；R-M3 衔接句的身份分组与账号不符；R-M4 还没选衔接句。选了“这篇不植入”、或账号确认与机构无关，则没有衔接句检查，衔接句也不会输出。
- **提醒（不挡复制）**：疗效承诺 R-W10（正文与衔接句）、R-W11（标题与封面大字）、讲效果 R-W1、对所有孩子下结论 R-W2、产品叫法 R-W3、写法不统一 R-W6、话题重复 R-W7、封面大字超过 10 字 R-W8、标题超过 20 字 R-W9。词表**没有用真实笔记校准**（规格附录 A；Mog 未授权），所以词表类规则只提醒，不能拦截。
- **提示**：R-I1 封面是 AI 作图；R-I2 提到了产品。
- **衔接句入库**：R-L1 疗效承诺挡住入库，R-L2–R-L7 只提醒或提示。

## 复制格式

正文各段之间是只有 U+3164 的一行；衔接句在锚定段落之后，没有可用锚点则在最后一段之前（只有一段则在其后）；空段落丢弃，段尾空白去掉；话题另起一行，`#` 开头、去重、保持顺序；标题单独返回。真机上是否保留 U+3164、话题能否识别、标题怎么计数**未验证**（Mog 暂不做），先按此上线，以使用反馈调整。

## 未证明

真实输入法与浏览器内的行为；真机复制；词表的准确率；供应商与外发（本片不涉及模型）；多人使用与权限（S0）。证明范围：规则与格式的单元测试（含对规则的故意破坏验证）、隔离 PostgreSQL 的 5 条流程证明（身份、事实卡、衔接句、复制、追加式与请求归属）、HTTP 层 2 条证明。
