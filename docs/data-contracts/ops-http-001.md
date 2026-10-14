# 运营页面接口合同（OPS-HTTP-001）

> 状态: 权威当前
> 最后核对: 2026-10-14
> 适用范围: OPS-MIGRATION-001 第 D2 步（Issue #405）：一级页面「运营」四个二级页面读取的 JSON 接口、导出、历史封面图，以及业务选择的作用范围；第 D3 步（Issue #409）：总览读取与周、月加微目标的设定（本合同唯一的写接口）
> 事实来源: [PAGE-OPS-001](../design/pages/ops-page.md)、[PAGE-OPS-002](../design/pages/ops-overview-page.md)、[OPS-LEDGER-001](ops-ledger-001.md)、Mog 2026-10-13 的确认
> 冲突时以谁为准: Mog 最新确认、AGENTS.md、OPS-LEDGER-001、真实 API/schema/代码
> 当前实现状态: D2、D3 已随 PR #407 / #415 上线；D3 合并后修复由 Issue #417 交付，未合并、未部署

除 §4 设定目标外，接口只读（GET），沿用本机读取守卫：只接受本机访问，响应 `no-store`、`nosniff`。D2 不新增写接口；发布记录、账号与每日数据写入随 D4b。

## 1. 业务选择

参数 `domain`：省略或取 `all` 表示「全部业务」；取某个领域编号时只看该业务。

| 数据 | 「全部业务」 | 某个业务 |
|---|---|---|
| 账号 | 全部在台账的账号 | 归属该领域的账号 |
| 发布记录 | 全部 | 所属账号归属该领域的；没有账号的记录不出现 |
| 每日数据、本月加微 | 全部 | **不分业务**：照样返回全部，并带 `scope: "all_businesses"` 说明 |

可选业务只列有在台账账号的领域；账号都已在工作台删除的领域也不列出。传入的领域编号不存在，或没有在台账的账号，返回 404 `ops_business_not_found`。

## 2. 通用约定

- 除 §4 的设定目标外，都是只读的 GET。
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

## 4. 总览与目标（D3）

口径见 [PAGE-OPS-002](../design/pages/ops-overview-page.md)。

- 时间段参数：`slice=yesterday|week|month|custom`；`custom` 时必须带 `from`、`to`（`YYYY-MM-DD`，北京时间的业务日，含首尾，最长 366 天）。参数不合法返回 400。
- 上期与本期天数相同，算法见 PAGE-OPS-002 §2。
- **只算已迁入的完整日子**：`completeThrough` 是最近一次迁入的导出日（北京时间）的前一天。本期只算到这一天（`covered`），上期按 `covered` 的天数对齐；本期一天都没迁入时 `covered`、`previous` 为 `null`，本期数字为 `null`（未知），不出下降提醒和自动结论。规则见 PAGE-OPS-002 §2.1。
- 「不分业务」的数字带 `dailyScope: "all_businesses"`。
- 接口只返回数字、名称和状态，结论文字由页面拼出。
- 未知一律为 `null`，不写 0。

### `GET /api/local/ops/overview?domain=&slice=&from=&to=`

返回所选时段的总览：

| 字段 | 内容 |
|---|---|
| `asOf` | 最近一次迁入的批次与导出时间（国际时间和北京时间）；还没迁入过时为 `null` |
| `businessSelected` | 是否选了某个业务 |
| `window` | `slice`、`today`、本周与本月的周期写法（`thisWeek`、`thisMonth`，供设定目标用）、`completeThrough`、`current`（日历上的本期）、`covered`（本期中已迁入的部分）、`previous`（与 `covered` 同样天数的上期）；后三者都是 `{from, to, days}` |
| `kpi` | 本期、上期：发布、链接打开、加微、转化率、爆款；有效发布为 `{"state": "not_connected"}`；各项环比（任一边未知或上期为 0 时为 `null`）；`dailyScope` |
| `funnel` | 发布 → 打开、打开 → 加微的转化率，本期与上期；选了业务时 `publishToOpenAvailable` 为 `false`（链接打开不分业务） |
| `target` | `state`：`set`（周期起止、目标值、按已迁入天数算的本期已完成、完成率、时间进度、每日还需、预计周期末、节奏 `achieved` / `on_track` / `behind`、最新修改记录）、`not_set`、`not_applicable`（自定义时段）；昨日与本周都按 `window.today` 所在 ISO 周读取，本月按今天所在月份，与对话框的 `thisWeek` / `thisMonth` 一致；目标周期没有已迁入完整日时完成情况为 `null` |
| `insights` | 自动结论的材料（类别 `pace` / `drag` / `review` 与相应数字、名单），最多 3 条 |
| `attention` | 今日必须处理：`items`（类别 `drop` / `zero_report` / `pending_views` / `owner_mismatch`、程度、运营人、账号、指标、本期与上期值、降幅、`count`、最早日期、`zeroMetrics`），只含排在前面的 5 项；`rest` 是其余各项（同样排序，页面折叠、点开在原处显示）；`hiddenCount` 等于 `rest` 的条数；`totalCount`。`count` 对待补阅读量是篇数，对负责人不一致是登记条数，对 `zero_report` 是本期有每日数据的天数；`zeroMetrics` 只在 `zero_report` 里有内容：本期每天都记 0 的指标及其上期值 |
| `ranking` / `rankingCovered` | 运营人排行各列；`rankingCovered` 表示本期是否有已迁入完整日，与 `window.covered !== null` 一致。未覆盖时排行为空但表示未知，页面写「这段时间还没有迁入的数据」；已覆盖且无人时才写「当期没有运营数据」。近 7 天加微为 7 个值（没有每日数据的那天为 `null`）；`risk`、`riskKind`、`riskMetric` 取自这个运营人在 `attention` 里最严重的一项 |
| `dailyOutput` | `days`（本期每一天）、`coveredTo`、`operators`、每天每人按平台的发布数、每天每人的加微 |
| `platformShare` | 按登记平台统计的发布数 `{xhs, douyin}`；本期未覆盖时整个字段为 `null`，不写 0/0 |
| `matrix` | `covered` 表示本期是否有已迁入完整日；运营人 × 账号的发布数与爆款数；账号带平台与是否已在工作台删除。未覆盖时数组为空、`covered: false`，页面写「这段时间还没有迁入的数据」；已覆盖且无登记时才写「当期无矩阵数据」 |

### `GET /api/local/ops/overview/trend?domain=&granularity=day|week|month`

长趋势：按日近 90 天、按周近 12 周（ISO 周）、按月近 6 月，保留排到今天的时间桶；数字只累计到今天与 `completeThrough` 中较早的一天。回包另带 `asOf`、`completeThrough`、`dailyScope`。每点包括：

- `start` / `label`：桶起始业务日与日期标签。
- `coverage`：`full`（截至今天应有的桶内日子均已迁入）、`partial`（只迁入部分）、`not_migrated`（桶内一天都没迁入，包括从未导入）。
- `coveredTo`：实际计算截至日；未覆盖为 `null`。周/月部分覆盖的数值只说明这一部分，页面可见说明「按 YYYY-MM-DD 及以前计算」，不外推整周/整月；今天之后的日期不参与覆盖判断。
- `followersGained`：不分业务；未覆盖或这一部分没有每日数据时为 `null`。
- `hits`：按发布日；未覆盖为 `null`，已覆盖且没有爆款才为 0。页面把未覆盖桶画成「还没迁入」，不画成 0。

### `GET /api/local/ops/targets?periodType=week|month&periodKey=`

这个周期的当前目标与全部修改记录，新的在前。周期写法为 `2026-W42` 或 `2026-10`。

### `POST /api/local/ops/targets`

设定目标，是本合同唯一的写接口。

- **请求限制**：只接受本机页面发出的请求（Host 为本机、同源），请求体必须是 JSON：`{ "periodType", "periodKey", "targetValue", "expectedRevision", "requestId" }`。
  - `targetValue`：1 到 1000000 的整数；
  - `expectedRevision`：页面看到的当前记录号，没有记录时为 `null`；
  - `requestId`：页面为这次保存生成的编号（UUID）。
- **回执**：每个请求都留回执，`outcome` 取下列之一：

  | `outcome` | 含义 |
  |---|---|
  | `accepted` | 追加了新的修改记录 |
  | `unchanged` | 目标值与当前相同，不追加修改记录 |
  | `stale` | `expectedRevision` 不是当前记录，未写入；回执带回当前记录 |

  回执里都带当前记录。
- **重复与冲突**：同一 `requestId` 再次提交，返回第一次的回执并标 `replayed: true`；同一 `requestId` 但内容不同，返回 409 `ops_target_request_conflict`。
- **失败**：参数不合法 400 `ops_parameter_invalid`；库不可用 503。

## 5. 不证明什么

- 接口只说明 Linggan 镜像里现在记着什么，不说明平台上的现状。
- `effectivePublication` 与 `systemCheck` 的固定取值表示能力未接入，不表示结果为零或未通过。
- 目标的修改记录只说明谁在何时把目标设成多少；登录（D4）之前「谁」只记到本机。
