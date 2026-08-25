# PAGE-EVIDENCE-001 · Evidence Library 本地页面

> 状态: 权威当前
> 最后核对: 2026-08-25
> 适用范围: `http://localhost:3000/corpus/evidence` 的第一个 Linggan 本地产品页面
> 事实来源: Mog 的 Local-001 产品确认、[LOCAL-001 活跃计划](../../plans/active/local-001-local-product-evidence-library.md)、Issue #25、Issue #29、LIDS 与当前 Rust 实现
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实运行/代码/合同、ACCEPTED 决定；V7 为本页受限的精确视觉与骨架 Gold Master

## 1. 身份与授权

- 页面规格 ID: `PAGE-EVIDENCE-001`
- 关联 Issue / Scope: `LOCAL-001 / 001A`（host）、Issue #29（V7 精确视觉/骨架）与 Issue #34（受控 discovery 接纳/只读投影）
- 当前状态: host 与 V7 骨架已存在；当且仅当服务显式配置 Linggan 本地 PostgreSQL 时，可接收并读取 `xhs.discovery.visible-card.v1` 的已接纳 discovery 卡片。没有配置数据库时，页面保持诚实空态。
- LIDS 视觉强度: L1 Corpus Explorer，嵌入受限 L2 Split Evidence Inspector
- LIDS 主 Pattern: `LIDS-PAT-001 / Corpus Explorer`，右侧使用同一工作面的 Inspector，不建立第二首页
- 产品页面来源: LOCAL-001 的 Evidence Library 用户任务；`REF-V7-001` 的页面 Gold Master
- 用户任务: 在此检索、核验并追溯 Linggan 已接纳的本地 discovery 卡片；没有数据或没有可用发布时间时，清楚看到它们各自的限制。
- 三秒答案: “这是本地 Evidence Library；它只显示已接纳的 discovery 卡片，不会重新搜索平台。”
- 五秒主动作: 输入文本只检索标题与创作者名，并且只检索本地已接纳卡片；窗口固定按来源可知的 `published_at` 过滤，并以读取时的 Linggan PostgreSQL `scope_001_now()` 作为唯一时间参照，只显示过去 7/30 天至当前的卡片。
- 明确非目标: 真实平台采集、详情、评论、作者主页、媒体下载/展示、OCR/ASR、保存查询、Topic/Insight/Agent 行动、线上部署。
- 当前可用数据/权限合同: `xhs.discovery.visible-card.v1` 的 Package admission、visible-card Coverage 和本地 `EvidenceQuery` 读取合同；没有详情 Evidence 或媒体访问授权。
- 决策 owner: Mog；实施范围由 LOCAL-001 活跃计划和 Issue #25 限定

## 2. 页面边界

- 入口: `GET /` 以临时重定向进入 `GET /corpus/evidence`；后者是同一 local host 的唯一页面路由
- 退出与返回: 当前无可用的相邻产品路由；V7 顶层、二级导航与控件保持原方位和结构，但均为 disabled/`aria-disabled`，不是未接通能力的假链接
- 本页负责的核心任务: 从 Linggan PostgreSQL 中读取已接纳的 discovery 卡片、显示其卡片级来源时间与 package 级 Coverage，并说明这不是详情或市场判断。
- 本页明确不负责: 触发/调度平台采集、接纳详情/评论/作者资料、读取原文、下载媒体、创建 Observation、计算趋势、修改设置或建立任何第二事实源。
- 第二事实源边界: HTML 只是数据库读取投影；它不保存搜索结果、计数、趋势或动作回执。`EvidenceQuery` 绝不等于 `AcquisitionSpec`。
- Inspector 边界: 本卡没有“选择材料”或详情读取；右栏继续显示 `UNKNOWN`，不能从 discovery 卡片补造正文、作者画像、评论、Capture 或 Coverage 以外的信息。

## 3. 信息、状态与行动

| 状态 ID | 触发/数据来源 | 用户应理解什么 | 禁止暗示什么 | 可用下一步 | 对应合同 |
|---|---|---|---|---|---|
| `SOURCE_INCOMPLETE` / `NOT_CONNECTED` | 服务未配置 `LINGGAN_LOCAL_DATABASE_URL` | 页面没有读取任何材料 | Linggan 库为 0、平台没有内容、旧内容工作台或插件已被读取 | 显式配置 Linggan 本地数据库后重启 host | LOCAL-001 / 001A |
| `ACCEPTED_DISCOVERY_ONLY` | 服务读到 #34 接纳的 discovery Package | 此页只显示搜索面实际可见的卡片 | 已有详情、评论、作者画像、媒体或市场趋势 | 只读检索本地材料 | `xhs.discovery.visible-card.v1` |
| `UNKNOWN_PUBLISHED_TIME_EXCLUDED` | 当前 `EvidenceQuery` 匹配的 ContentItem 没有任何位于 `WINDOW` 内的已知来源发布时间 | 该对象不进入 `WINDOW` 结果；同一对象若另有窗口内已知 occurrence，则显示一次且不计入排除数 | 发布时间为 0、旧内容、平台没有内容或采集失败 | 等待未来独立事实补充；本卡不猜测 | EvidenceQuery / WINDOW |
| `READ_PROJECTION_UNAVAILABLE` | 已配置数据库但本地读取失败 | 页面没有显示旧系统或远程回退数据 | 所有已接纳材料均丢失或平台不可用 | 修复本地数据库连接后重试读取 | #34 read projection |
| `LOCAL_QUERY_INVALID` | URL 查询不是当前受限 EvidenceQuery | 系统没有执行读取或平台搜索 | 查询被转成采集命令 | 使用受限文本 + 7/30 天窗口 | EvidenceQuery |
| `UNKNOWN` | 没有选择 ContentItem，且没有可用 Observation/Capture 输入 | 来源、观察、Capture 和 Coverage 当前未知 | unknown 等于 0、正常、失败或完整 | 无 | AGENTS.md 领域不变量；LIDS-PRI-001 |
| `NO_ACCEPTED_MATERIAL_AVAILABLE` | 当前 local query 没有已知发布时间且落在窗口内的卡片 | 页面现在不能展示卡片 | 系统永久没有材料、平台无内容或库为 0 | 调整本地查询或等待未来独立补证 | #34 read projection |

本卡不显示 `success`、`live`、`fresh`、平台总量、完整百分比或趋势。Package admission、卡片处理、Coverage 与页面投影是独立状态，不能被压缩成一个“完成”。

## 4. 已批准的设计组合

| 页面区域/场景 | PAGE / DS / PAT / CMP 来源 ID | 使用目的 | 明确不允许的替代 |
|---|---|---|---|
| 顶层产品方位 | `REF-V7-001` + `LOCAL-001-UI-EX-01` + LIDS Token | 延续 V7 两层导航的工作空间方向 | 未接通页面路由、假链接或伪造 active task |
| 左侧语料 rail | `LIDS-PAT-001 / Corpus Explorer` | 给 Evidence Library 一个稳定的 L1 定位 | 卡片瀑布流、全局第二侧栏 |
| 中央 Evidence Workspace | `LIDS-PAT-001 / Corpus Explorer` | 连续材料工作面；001A 只显示边界空态 | 示例材料、KPI 卡、趋势图或人工造数 |
| 右侧 Provenance Inspector | `LIDS-PAT-001 / Split Evidence Inspector` | 为真实对象预留来源/观察/Capture 边界位置 | 第二套筛选器、第二主页或 AI 结论 |
| Truth / Read Model / Coverage readout | `LIDS-PRI-001` 五轴状态原则 | 分开表达来源不足、读模型未接通和未知 Coverage | 一个万能“正常”或“失败”标签 |

### LIDS 采用清单

- Token 基线与唯一运行时真源: `LIDS-TOK-001`。`apps/api/src/local_web/lids_tokens.css` 仍是唯一的全局 `--lgi-*` token 值编辑源，完整承载当前 117 个 token（DESIGN-003 由 107 增至 117）；`docs/design/lids/tokens.md` 是其版本化规范与校验镜像。Issue #29 不修改它。`evidence_library.css` 不声明 `--lgi-*` token。
- Primitive / 正式 CMP 查重结果: 没有新增跨页面 CMP；沿用 LIDS 的语义分责，但本页由 V7 精确视觉/骨架例外控制具体排版和页面色值。
- 页面唯一视觉核心: V7 的 128px 两层页头、白底硬线三栏工作面、FACT LAYER 与右侧 Inspector；中央区只用诚实空行替代 V7 的模拟材料。
- 原声、来源、样本、窗口、Coverage/Validity/冲突/复核边界: 001A 没有材料、原声或样本；页面直接说明对应字段未知而非显示占位数字。
- ASCII / 场景 / 动效: 不使用等距场景与环境动效。ASCII/像素语言仅出现在一处——rail 选中项右边缘的离散像素纹理（DESIGN-004），承担 `system.md` 中约 5% 的终端语言权重，不构成场景。运动限于状态反馈：hover/选中的背景与色彩过渡（DESIGN-003）、以及品牌标识与 rail 选中项的粗野位移。`prefers-reduced-motion:reduce` 下关闭全部过渡，并把 rail 选中项的常驻位移置为 `none`。
- 页面级例外 `LOCAL-001-UI-EX-01 / Issue #29 修订`: 用户直接授权本页依 `REF-V7-001` 精确复刻视觉值和 HTML 骨架：桌面为78px + 50px = 128px 页头、216px rail、440px inspector（≤1500px 为420px、≤1180px 为380px）、全白背景、黑色硬线、全局 `#E8003F` 与 Evidence `#EF4F25`。桌面必须是 `100vh` 固定工作台，document 不向下延展，Results 和 Inspector 在各自栏内滚动；≤900px 改为自动首行高度，以容纳可能换行的顶层导航和 context row，随后才开始顺序折叠和页面原生滚动。这些值只以明确命名的 `--v7-*` 页面局部变量存在于 `evidence_library.css`；不改写、不复制为 LIDS 全局 token，也不授权其他页面继承。本条中的「全白背景」与两个色值已被后续用户确认部分替代，原文保留以便追溯：DESIGN-003 使第二签名色 `#E8003F` 退役、签名色收敛为 `--lgi-signal`，并把上下文行改为 `--lgi-canvas-low` + 双层点阵；DESIGN-004 按同一依据把左侧 rail 一并改为该测量场底。两次替代都不改变 128px 页头、216px rail、440px inspector 的几何与断点。
- 明确禁止: 继承 V7 的模拟运行状态、计数、示例帖子/评论/转录、时间、引用或成功回执；任何 V7 位置上的未接通控件不得产生写入、采集、保存、研究或连接副作用。

## 5. 交互与真实后果

| 用户动作 | 前置条件 | 请求/回执来源 | 页面如何区分接纳、处理中、完成、部分、失败 | 不得宣称 |
|---|---|---|---|---|
| 搜索本地卡片 | 服务已显式连接 Linggan PostgreSQL；输入可选文本，窗口为 7 或 30 天 | `GET /corpus/evidence` 或 `GET /api/local/evidence-library`；仅读取已接纳 discovery 数据 | 无数据库为 `NOT_CONNECTED`；无窗口内卡片为 `NO_ACCEPTED_MATERIAL_AVAILABLE`；当前查询候选中发布时间未知的对象单独计数且被排除 | 触发小红书搜索、补采、详情/评论读取或“世界中没有内容” |
| 其余 V7 控件 | 无 | 不适用 | 继续 disabled；没有写入或采集回执 | 保存视图、研究、补采、原文、选择材料或真实运行状态 |

### 5.1 001C-0 已冻结、但尚未接通的后续控件语义

`LOCAL-001 / 001C-0` 已定义跨边界合同；Issue #34 只接通其中的本地搜索读取。除下列明确说明的 Search 外，V7 位置继续保持禁用；它们不是正在等待的隐式动作。

| V7 控件位置 | 未来获准含义 | 当前状态 | 禁止退化 |
|---|---|---|---|
| Search | 只查询 Linggan 已接纳 discovery 卡片的 `EvidenceQuery`；V1 仅标题/创作者名，`WINDOW = known published_at` | 已接通时 enabled；无数据库时 `NOT_CONNECTED`；JSON 使用实际 `last_7_days`/`last_30_days`，页面同源显示 `7D`/`30D` | 把输入框当作小红书搜索、插件命令或正文/评论/OCR/ASR 检索 |
| Copy query | 只复制本地 URL 与读取查询状态 | disabled / `NOT_CONNECTED` | 创建采集、研究或外部链接任务 |
| Save current view | 未定义 | disabled / `DEFINITION_PENDING` | 偷偷保存为监控或持续采集 |
| Start research | 未定义 | disabled / `DEFINITION_PENDING` | 创建 Research、Claim、Agent 或任何写入 |
| Reacquire / 补采 | 先选择明确数据缺口、经新授权的未来动作 | disabled / `SELECTION_AND_AUTHORIZATION_REQUIRED` | 用一个含糊按钮自动补详情、评论、媒体或指标 |

封面位置同样没有本卡可用图片。搜索页中将来观察到的外部地址只是 `MediaCandidate`；在独立 001C-2 成功取得、验证并保存本地副本前，页面必须使用 `MEDIA_NOT_ACQUIRED` 等真实状态，绝不能把小红书 CDN 地址设为 `img src` 或 CSS 背景回退。

## 6. 验收与未证明边界

- 任务验收场景: 已配置本地数据库时，提交一份合格的受控 discovery Package 后可在页面读取卡片；未配置数据库时，仍能区分 host 已运行与读投影未接通。
- 状态/语义验收场景: `SOURCE_INCOMPLETE`、`NOT_CONNECTED`、`ACCEPTED_DISCOVERY_ONLY`、未知发布时间排除、`UNKNOWN` 和“当前窗口没有可展示卡片”分别出现；页面不出现 0、LIVE、FRESH、平台总量或趋势语言。
- 视觉验收场景与声明的工作条件: 1280×800、1440×900、1920×1080、390×844 的真实 local route 检查，见 `ACC-EVIDENCE-001`。
- 真实后果验收场景: 仅当 isolated PostgreSQL proof 已真实通过时，才证明受控 Package → localhost ingress → 本地读取投影；它不证明真实插件、平台、账号或浏览器。
- 自动检查: API route tests、discovery integration tests、workspace format/clippy/test、governance check。
- 本次证明: 以 PR 的精确命令与数据库 proof 记录为准；Docker daemon 不可用时不得以编译/fixture 代替 PostgreSQL proof。
- 本次未证明: 真实 001C-1 discovery、详情/评论/作者资料、任何媒体 bytes/本地 cover、OCR/ASR、Observation/Topic/Research/Insight、权限、部署和业务验收。
