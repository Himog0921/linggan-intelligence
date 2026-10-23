# 运营原生写入数据合同（OPS-NATIVE-WRITE-001）

> 状态: 权威当前
> 最后核对: 2026-10-23
> 适用范围: OPS-LAUNCH-001 阶段 2 的原生账号、发布、手工阅读量与日报；请求幂等、并发版本与历史来源边界
> 事实来源: `01_IMPLEMENTATION_MANUAL.md` §4–§6、验收 D01–D11；迁移 `0149_ops_native_write_model.sql`；既有 `OPS-LEDGER-001`
> 冲突时以谁为准: Mog 最新确认、OPS-LAUNCH-001、真实 schema/API、[OPS-LEDGER-001](ops-ledger-001.md)

## 1. 统一账本，不伪造迁入

`linggan_ops_account`、`linggan_ops_publication`、`linggan_ops_view_report` 和 `linggan_ops_daily_report` 是唯一的运营业务读写表。每行只有一种来源：

| 来源 | `source_system` / `source_kind` | 能表达什么 | 禁止事项 |
| --- | --- | --- | --- |
| 历史迁入 | `content_workbench` / `workbench_import` | 旧工作台在导出时记录的值 | 不能让原生写入生成批次、来源记录或迁入操作者 |
| 原生运营 | `linggan_native` / `linggan_ops_manual` | 已认证成员在 Linggan 中明确提交的当前运营事实 | 不能回填或改写历史导入语义 |

原生主记录的 `source_record_id` 必须等于自身 UUID 文本；它不是外部系统 ID。原生行的导入批次、旧来源时间和旧回流字段必须为 `NULL`。迁入行不能带任何 `native_*` 字段。数据库约束负责阻止两种来源混写。

## 2. 对象、历史与空值

- 账号：稳定 `account_ref`；`native_record_state` 为 `active` 或 `archived`。改名、负责人、业务或健康不会更换 ID。归档不删除历史发布。
- 发布：稳定 `publication_ref`；记录发布人、实际登记人和发布当时的业务 `native_business_domain_ref`。`active` / `revoked` 是可恢复的生命周期，不等于从平台删除。账号平台与发布平台必须一致。
- 手工阅读量：每一次保存向 `linggan_ops_view_report` 追加一行；`value=0` 合法，`cleared` 表示未知。当前值由最新追加报告决定。小红书最新有效值 `>=5000` 才判爆款；抖音不判爆款。
- 日报：原生唯一性是稳定 `native_member_ref + business_date`。`links_opened_count` 和 `followers_gained` 中 `NULL` 表示未填，`0` 表示明确填零；不能由 GET 补写。每次成功保存另追加 `linggan_ops_daily_report_revision`，使实际保存人和版本可追溯。
- 已迁入日报保留旧的必填 `0`，因为无法倒推出它当时是未填还是零。

## 3. 写入请求、重试与并发

每个原生写命令有调用方生成的 UUID `request_ref`、经服务端规范化后计算的 `payload_sha256`、服务器认证的 `actor_member_ref` 和一次不可变 `linggan_ops_native_write_request` 回执。

| 情况 | 结果 |
| --- | --- |
| 同一请求号、同一载荷 | 返回首次回执；不重复创建或追加 |
| 同一请求号、不同载荷 | 拒绝为请求冲突；不写新事实 |
| 版本不匹配 | `stale` 回执，返回最新版本；不静默覆盖 |
| 校验或权限失败 | `rejected` 回执，不产生目标记录/修订 |

业务代码必须在同一事务内：锁定/比较当前版本 → 写主记录或追加报告 → 写请求回执和必要修订 → 提交。浏览器提交的人员、岗位、业务或账号归属均不是授权依据；服务端按当时会话、角色和当前责任重新判断。

## 4. 当前切片的明确边界

迁移 `0149` 只建立可验证的数据边界和审计容器；它**不**自行开启 HTTP 写路由、旧工作台停写、正式 schema 迁移、真实团队数据、邮件或平台采集。后续 API 必须按本合同实现逐行回执、权限和读取投影；采集同步还必须接入既有 Request → Admission → WorkOrder → Lease → Task/Attempt → Package/Receipt 链，不能把此表当作第二个任务队列。

## 5. 验收覆盖映射

| 验收 | 本合同/迁移现在提供 | 后续仍需证明 |
| --- | --- | --- |
| D01 | 原生稳定 ID、来源隔离、账号状态和版本字段 | 实际新增/编辑 API、全局平台身份冲突处理和 UI |
| D02 / D09 | 请求回执、载荷指纹、版本字段 | 多行逐项 HTTP 回执、超时重放、冲突恢复 UI |
| D04 / D10 / D11 | 发布时业务、撤销/归档状态、审计字段 | 管理员更正动作、统计和概览联动 |
| D05 / D06 / D07 | 阅读量追加模型、日报原生空值和修订 | 批量填报、月累计/待填投影和页面 |
| D08 | 来源互斥与旧语义保留 | 原生和历史共同的正式读模型与切换对账 |

因此本合同不是“数据模块已交付”声明；任何真实上线、旧入口停写或业务验收仍需独立授权与回执。
