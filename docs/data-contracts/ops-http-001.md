# 运营页面只读接口合同（OPS-HTTP-001）

> 状态: 权威当前
> 最后核对: 2026-10-13
> 适用范围: OPS-MIGRATION-001 第 D2 步（Issue #405）：一级页面「运营」四个二级页面读取的 JSON 接口、导出、历史封面图，以及业务选择的作用范围
> 事实来源: [PAGE-OPS-001](../design/pages/ops-page.md)、[OPS-LEDGER-001](ops-ledger-001.md)、Mog 2026-10-13 的确认
> 冲突时以谁为准: Mog 最新确认、AGENTS.md、OPS-LEDGER-001、真实 API/schema/代码
> 当前实现状态: 分支 `feat/ops-migration-d2`；未合并

全部接口只读（GET），沿用本机读取守卫：只接受本机访问，响应 `no-store`、`nosniff`。D2 不新增任何写接口；写入随 D4b。

## 1. 业务选择

参数 `domain`：省略或取 `all` 表示「全部业务」；取某个领域编号时只看该业务。

| 数据 | 「全部业务」 | 某个业务 |
|---|---|---|
| 账号 | 全部在台账的账号 | 归属该领域的账号 |
| 发布记录 | 全部 | 所属账号归属该领域的；没有账号的记录不出现 |
| 每日数据、本月加微 | 全部 | **不分业务**：照样返回全部，并带 `scope: "all_businesses"` 说明 |

可选业务只列有我方账号的领域。传入的领域编号不存在，或没有我方账号，返回 404 `ops_business_not_found`。

## 2. 通用约定

- 只看在最新导出中的记录（`missing_since_batch_ref` 为空）；被合并的重复登记只出现在主记录的 `duplicates` 里。
- 日期一律是北京时间的业务日，格式 `YYYY-MM-DD`；「本月」是今天所在的北京时间月份。
- 未知或没有值返回 `null`，不返回 0；0 只表示已确认为零。
- 每个响应带 `asOf`：最近一次导入批次的导出时间与批次号，`{ "exportedAt"（UTC 时刻）, "exportedAtBeijing"（北京时间 `YYYY-MM-DD HH:MM`）, "batchRef" }`。从未导入时为 `null`。
- 失败统一返回 `{ "operation": "local_read", "outcome": "unavailable", "code": "<代码>" }`：参数不合法 400 `ops_parameter_invalid`，业务不存在 404 `ops_business_not_found`，读取不可用 503 `ops_read_unavailable`，本机服务没有连数据库 503 `read_model_not_connected`。页面把代码翻成中文原因显示，不直接显示代码。

## 3. 接口

### `GET /api/local/ops/businesses`

可选业务：`[{ "domainRef", "name", "accountCount" }]`，按领域创建时间排序。`accountCount` 只数在台账的账号（不含已在工作台删除的账号），与账号管理的「总账号」一致。

### `GET /api/local/ops/summary?domain=`

发布记录页读数条：

| 字段 | 含义 |
|---|---|
| `month` | 本月，`YYYY-MM` |
| `activeAccounts` / `ledgerAccounts` | 使用中与养号中的账号数 / 在台账的账号数 |
| `monthPublished` | 本月发布的主记录条数 |
| `effectivePublication` | 固定 `{ "state": "not_connected" }`，自动发现（D5）上线前不给数值 |
| `monthFollowersGained` | `{ "value", "scope": "all_businesses" }`，本月每日数据加微之和 |
| `monthHits` | 本月发布的小红书主记录中，最新阅读量 ≥ 5000 的条数 |

### `GET /api/local/ops/publications`

参数：`domain`、`operator`（发布人名，精确匹配）、`platform`（`xhs` / `douyin`）、`account`（账号名包含匹配）、`from` / `to`（业务日，含首尾）、`views=pending`、`page`（从 1 开始）。

- 排序：发布时间从新到旧；`views=pending` 时只返回小红书、阅读量未填的主记录，按发布时间从旧到新，最该补的排在前面。
- 每页 50 条。返回 `page`、`pageSize`、`total`、`totalPages`、`operators`（当前业务下全部发布人名，供页签使用）与 `items`。

每条 `item`：

| 字段 | 含义 |
|---|---|
| `ref` | 发布登记编号 |
| `businessDate`、`publishedAt`、`dateState` | 业务日、原始发布时间、`as_recorded` / `suspect_year` |
| `platform`、`title`、`body`、`url` | 原值；`url` 是发布链接 |
| `account` | `{ "name", "ledgerState" }`；没有账号为 `null` |
| `publisher` | 发布人名或 `null` |
| `views` | `{ "state", "value", "asOf", "overdue", "hit" }`。`state` 取值：`value` 已填；`pending` 小红书未填；`optional` 抖音未填。`overdue` 表示小红书未填且发布已满 7 天，`hit` 表示小红书且 `value` ≥ 5000 |
| `likes`、`collects`、`comments`、`coverClickRate` | 迁入旧值或 `null` |
| `systemCheck` | 固定 `not_checked`，D5 前不给其他值 |
| `duplicates` | 被合并进来的重复登记：`[{ "businessDate" }]` |
| `cover` | `{ "available": true/false }`；为真时可读 `/ops/covers/{ref}` |

### `GET /api/local/ops/accounts?domain=&operator=`

只列在台账的账号，按负责运营、平台、账号名升序。返回 `total`、`active`、`operators`、`items`。

每条 `item`：`ref`、`platform`、`name`、`profileUrl`、`deviceCode`、`followerCount`、`ipLocation`、`operator`、`status`、`isRetained`、`notes`，以及固定为 `{ "state": "not_filled" }` 的 `health`。账号健康改为运营手动填写，D4b 前没有值。

### `GET /api/local/ops/daily?domain=&operator=`

最近 200 行，按业务日从新到旧、同一天按运营人。返回 `scope`（固定 `all_businesses`）、`limit`、`operators`、`items`。

每条 `item`：

| 字段 | 含义 |
|---|---|
| `ref`、`businessDate`、`weekday`、`operator` | 原值；星期按业务日计算 |
| `published` | 该运营人当天的主记录条数（按发布人名） |
| `linksOpened`、`followersGained` | 原值 |
| `addRate` | `followersGained ÷ linksOpened`；打开为 0 时为 `null` |
| `hits` | 该运营人当天发布的小红书主记录中，最新阅读量 ≥ 5000 的条数 |
| `monthFollowersGained`、`monthPublished` | 该运营人当月截至当天的累计 |

### `GET /api/local/ops/export?kind=&format=&domain=`

`kind` 取 `accounts` / `publications` / `daily`，`format` 取 `csv` / `json`。导出当前业务下的全部记录，不分页；列与页面一致，另附来源记录号和数据截至时间。CSV 用 UTF-8 并带 BOM，便于表格软件直接打开；CSV 的「数据截至（北京时间）」列写 `YYYY-MM-DD HH:MM` 北京时间，JSON 的 `asOf` 保留 UTC 时刻。

### `GET /ops/covers/{ref}`

返回这条发布登记已迁入的历史封面原图。只能读封面表登记过的文件：文件放在本机媒体目录下，读取时核对大小与类型；没有封面返回 404。

## 4. 不证明什么

- 接口只说明 Linggan 镜像里现在记着什么，不说明平台上的现状。
- `effectivePublication` 与 `systemCheck` 的固定取值表示能力未接入，不表示结果为零或未通过。
