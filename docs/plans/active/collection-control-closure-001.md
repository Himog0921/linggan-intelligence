# COLLECTION-CONTROL-CLOSURE-001 · 平台账号准入与巡检规则闭环

> 状态: 活跃计划
> 最后核对: 2026-09-10
> 适用范围: Issue #149 / Package 2；只支持 1440 CSS px 桌面全屏
> 事实来源: Issue #149 Claim、`origin/main@c5158b14f5fc2313dbd3dc94670083500e767ced` 的 migration/Rust/Collection/Browser Producer、PAGE-COLLECTION-001 与 LIDS v7
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实代码/数据库/测试、ACCEPTED 决定与 Issue #149 冻结合同

## 0. 2026-09-04 范围修正 · 认领即自动接活、工位名是服务端真相

本段按 Mog 最新产品决定，修正本计划旧 Package 2 中“新工位默认关闭”的交付语义。实施任务为 `collection-control-closure-001-p2-auto-acceptance-station-name`，exact base 为 `origin/main@e5c5ec5f9bab62f25bea9798fd9abb127df03956`，专属分支为 `codex/collection-station-auto-acceptance-name`；详细授权留在 [Issue #149 comment](https://github.com/Himog0921/linggan-intelligence/issues/149#issuecomment-5541694126)。

- 人在 Runtime 注册工位后，它仍是“待认领”的资源，不表示有插件可执行；成功 `claim` 的有效安装会把该工位默认切为**自动接活**，并追加 `installation_claimed_auto_enabled` 审计。
- 自动接活不是绕过门禁。只有心跳新鲜、安装凭证/最低版本/能力合格、账号已绑定且资格新鲜、风险/并发/预算允许，并且队列存在合格任务时，才会获得 Lease 并执行。
- 人在 Runtime 显式暂停后，`person_disabled` 是持久安全覆盖：后续心跳、重装、替换安装和 claim window 都不得自行重开；只有人显式恢复自动接活才写新的转换。
- `execution_station.display_name` 是唯一的可读工位名真相。Runtime 新建工位预填“本机 Chrome”，且可由人改名；check-in 只回显该服务端名称和当前接活状态。插件不保存/编辑本地别名，不以显示名做身份或配对键。
- additive `0035_claimed_station_auto_acceptance` 只把旧 `migration_closed` 或 `registered_closed` 默认态且已认领的历史工位迁为自动接活；人工暂停的历史工位保持暂停。

这项源码/隔离 proof 不授权共享 migration、`:3000` runtime 切换、Chrome reload、真实平台访问或实际采集；当前现场暂停状态不会因为本修正被自动解除。此处“账号资格新鲜”的旧表述由 §0.2 取代。

## 0.1 2026-09-05 范围修正 · 建档不再是观察门槛，工位只认领统一队列

本段由 Mog 对“自动监测基线设计”的确认产生，覆盖本计划旧 §4.2 的 `baseline_required`、dynamic cadence 与“scheduler 直接派发到工位”的语义。实施任务为 `monitor-scheduling-clarity-001`，exact base `origin/main@33e6623b2d5baa070ee18ebd0813178a57c6c8ca`，分支 `codex/monitor-scheduling-clarity-001`，worktree 为 `/Users/moglenny/proma/linggan-intelligence/.worktrees/monitor-scheduling-clarity-001`；范围修正与文件 claim 记录在 Issue #149。

- `UNKNOWN`、`PARTIAL`、历史目录缺失和未建立深度档案是结果/覆盖事实，不是创建或启用观察规则的门槛。一个有效 Target + Rule 必须能持续产生未来观察；`monitoring + no active automatic rule` 是数据库拒绝、迁移修复的非法状态。
- rule revision、active pointer、target monitoring/lifecycle、anchor/next schedule 同一事务写入。首版只提供固定全天间隔（6h / 12h / 24h / 2d / 7d）；调度只读 `next_run_at`，一次 missed schedule 只补一张 Work Order。
- Scheduler 只创建 `queued` Work Order，**不**选择 station 或宣称派发。所有浏览器需求进入同一 durable pool，并按 `immediate`、`scheduled`、`batch` 三个技术 lane 排程；station heartbeat/claim 在资格、账号、额度、风险、能力和授权 scope 合格后用原子 Lease 认领。
- `creator_patrol` 固定为作者主页资格核验加最近 30 条作品观察。`deep_archive` 是 batch lane，既不阻断也不暂停有效 patrol rule；OCR/ASR/媒体不占浏览器队列。
- `0036_monitor_scheduling_clarity` 为 additive migration；本实现只允许 disposable PostgreSQL 证明，**不**授权共享 migration、`:3000` Runtime 切换、插件重载、真实平台访问、部署、push 或 merge。

**阅读规则：** 下文 §3–§9 保留的是 2026-09-04 Package 2 的交付快照，用于追溯当时 PR 的边界；其中出现的 `baseline_required`、dynamic cadence、窗口/fallback 和 scheduler 预先创建 Lease 均已被本节取代，不能作为当前实现或验收依据。

## 0.2 2026-09-10 范围修正 · 账号观察是诊断事实，不是短租约门槛

本段按 Mog 最新产品决定，取代本计划 §0、§4.1 和既有 Runtime 文案中“正向账号资格必须在短窗口内新鲜，否则拒绝接活”的语义。实施任务为 `collection-account-observation-soft-gate-001`，exact base 为 `origin/main@7c4375e78e96fdf7411a5dec74304e9e822e3b49`，专属分支为 `codex/collection-account-observation-soft-gate-001`；范围修订与 Claim 记录在 [Issue #149 comment](https://github.com/Himog0921/linggan-intelligence/issues/149#issuecomment-5613757602)。

- `account_last_observed_at` 只保留为诊断与排障元数据；`0064` 将旧 observation `expires_at` 改为可空历史字段，新的观察不再产生任何到期时间。**时间流逝本身绝不拒绝新 Lease**。固定浏览器里 20 分钟、6 小时或更久未读取全局导航，不是账号退出的证据。
- 新 Lease 的硬门仍为：工位心跳、接活开关、凭据、最低版本、能力、人工确认的账号绑定、风险/并发/预算，以及没有明确账号负面事实。最新页面观察到的账号与当前人工绑定不一致时也是 `account_binding_changed`，必须由人确认新绑定；`needs_login`、`restricted`、`cooling` 和从未形成账号事实的 `UNKNOWN` 仍按关闭处理。**人工绑定没有按日期自动失效：时间不是账号变更的证据。**
- 页面暂时读不到“我/我的”导航标记属于**不完整观察**，不得写成新的 `UNKNOWN` 覆盖既有 `usable` 或既有明确负面状态；它不是可上报的 eligibility event。容量投影也忽略历史 `unknown/signal_incomplete` 行，回退到最后一个确定事实；首次从未形成任何确定事实的账号仍为 `UNKNOWN`。
- Browser Producer 只在 XHS content script 因用户页面或任务本来要打开的任务页自然启动时做一次已渲染 DOM 的最小观察。它不再由 alarm、队列等待或 `account_eligibility_stale` 扫描任何已打开标签、重试 claim、打开/刷新/滚动/切换平台页面或调用平台接口。
- 长时间空闲后的保护复用既有**每账号至多一个 live Lease**：同一 station/account 的第一个正常任务可领取并在其本来打开的任务页顺便形成新事实；该 Lease 未释放前不会再给同账号第二单。不新增 Account Lease 表、后台巡检或定期探测。

### 0.2.1 2026-09-10 实施收敛 · 观察事件、持久绑定和单一容量读取

本轮落实不以继续叠加条件为方式，而是替换旧的交叉输入和重复容量 SQL。它只覆盖源码、append-only migration、隔离 PostgreSQL proof、Browser Producer source/release 与文档；不授权 shared DB migration、Runtime/Chrome reload、真实平台访问、push、merge 或部署。

- Producer→API 的 observation body 是严格闭集：`authenticated_observed` 必须携带瞬时账号标识；`cooldown_observed`、`login_required`、`access_restricted` 是不携带账号标识的明确负面事件。不存在 `signal_incomplete + raw account id` 这类可产生幽灵身份的交叉组合；不完整 DOM 读取不发送请求。
- 人工账号 binding 是持续有效的服务端事实，直到人明确结束/替换它。新增 additive `0064_account_observation_normalization` 删除旧 `confirmed_until` deadline，并取消新 observation 的 expiry 写入；历史 `account_binding_expired` reason 只用于读取既有历史，不再由当前 evaluator 产生。
- 账号观察只在用户自然打开的 XHS 页，或已领取任务自然打开的同一 XHS 页发生。负面信号只能来自平台状态组件，绝不从页面标题、笔记或评论全文猜测；无法得出结论、或本机回传暂时失败时都放行该首单。只有服务端已确认的负面事实或身份不一致才停止该 Lease。禁止 alarm、标签枚举、打开/刷新/滚动/点击或平台 API 探测。
- server minimum plugin version 与 Producer contract 同步升级到 `0.8.46`，旧版本不能绕过任务页观察路径领取新 Lease。`evaluate`、frozen revalidation 和 ready-batch slots 使用同一命名 `CapacityCandidate` loader / policy，不再维护三份 SQL 或 positional tuple。
- 验收必须分别证明：普通 capacity 读取、具体 claimant revalidation/dispatch 及 batch-ready slots 都会在账号身份变化或明确负面观察后关闭；HTTP 拒绝旧平铺 body 和畸形组合；发布 zip 可重现且只保留当前受控版本。

**阅读规则补充：** 此后各节出现 `account_eligibility_stale`、账号 positive freshness、20 分钟/6 小时资格续期或“先重报再 retry claim”时，均为历史快照或历史行兼容，不得恢复为当前执行门槛。

## 1. 用户结果与串行门

Package 1 已合并并核验到本计划的 exact base。Package 2 要让同一份服务端 durable truth 同时回答：哪台工位、哪个安装、哪个平台观察账号现在可以接活；某个目标按哪一版规则自动巡检；一次保存、跳过、准入、租约、派发或失败到底发生了什么。Package 3 / Creator Dossier 在本包合并复核前继续等待。

不能接受的结果：插件自行宣布账号可用；UNKNOWN 被当成健康；安装密钥、平台账号原始 ID、Cookie、HTML 或自由错误文本进入数据库/页面/日志；保存规则被写成采集成功；Collection 出现 Evidence 正文、监控价值、机会评分或 Creator Dossier。

## 2. Claim

- Issue: `#149`
- Task ID: `collection-control-closure-001-p2`
- Claim: `COLLECTION-CONTROL-CLOSURE-001-P2`
- Exact base: `c5158b14f5fc2313dbd3dc94670083500e767ced`
- Branch: `codex/collection-control-closure-001`
- Worktree: `/Users/moglenny/proma/linggan-intelligence/.worktrees/collection-control-closure-001`
- 实现方式: 一个 Package 2 PR；不 merge、不切换共享 runtime、不应用共享 migration、不操作 Chrome/真实平台。

## 3. Reality Matrix

| 责任 | 实施前 | 当前证据 | 本包动作 |
|---|---|---|---|
| target identity/lifecycle | EVIDENCED | `0005` 唯一键与 append-only transition | 保留并把控制命令与 lifecycle/transition 原子化 |
| Request→Admission→WorkOrder→Lease→Task | PARTIAL | `0006`–`0010` 已存在；关键步分事务 | 冻结 install/account/rule refs；失败不留下可误读成功 |
| station quota/risk/capability | PARTIAL | station 200/day、risk pause 与 capabilities 已执行 | 统一 capacity evaluator，增加 accepting/freshness/min-version/account/credential |
| platform account | ABSENT | 没有身份、binding、eligibility | keyed digest only；append-only observation/current；五态 fail closed |
| installation credential | ABSENT | `install_key` 被当身份，不是秘密 | 一次返回、只存 hash、轮换/撤销、claim/report 验证 |
| monitoring rule | PARTIAL | target bool + fixed seconds，无 revision/receipt | append-only rule revision + active pointer + expected revision/idempotency receipt |
| dynamic cadence | HISTORICAL / SUPERSEDED | 旧快照中的两轮 comparable qualification | 当前首版不提供动态频率或 fallback；见 §0.1 |
| scheduler | PARTIAL | 每 tick `LIMIT 50` 且无 target receipt | cursor/fair scan + 每目标 durable decision receipt/backoff |
| Collection read surfaces | PARTIAL | Tasks 真实；Operations/Attention 仍多为空壳 | 只投影 scheduler/decision/WorkOrder/Lease/Task/Receipt/account capacity 事实 |
| target baseline | HISTORICAL / SUPERSEDED | 旧快照把 Coverage 用作规则门槛 | 当前 UNKNOWN/PARTIAL 只限制覆盖结论，不阻断规则；见 §0.1 |
| shared DB/runtime/Chrome/真实平台 | NOT VERIFIED | 本 Claim 禁止 | 只用 disposable PostgreSQL 与隔离非 3000 browser |

## 4. 冻结合同

### 4.1 统一容量判定

历史快照曾把 installation/account positive freshness `20m` 写入这一 policy；当前仅 installation heartbeat freshness 保持硬门。账号正向观察的时间窗口已经由 §0.2 降为诊断元数据。当前判定顺序为 risk → station active/accepting → active installation → credential → version → installation heartbeat → capability → bound account → explicit eligibility state → live-Lease busy → station budget。每次返回闭集 reason code；从未形成账号事实的 UNKNOWN 一律阻断。`busy` 只从 live Lease 派生，不接受 Producer 上报。

账号 eligibility 闭集为 `usable | cooling | needs_login | restricted | unknown`。Producer 只提交版本化 signal 与 raw account identity 的瞬时 loopback body；服务端使用部署密钥计算 versioned keyed digest，raw identity 绝不持久化或返回。账号 binding 与 eligibility history 只追加；current 是重算读取责任。

### 4.2 巡检规则 v1（已由 §0.1 修正）

每个 target 的规则 revision 只追加；active pointer、`monitoring_enabled`、lifecycle 和 `anchor/next_run_at` 同事务变化。命令携带 expected revision、idempotency key、payload digest、actor/source；相同 key+payload replay，不同 payload conflict，stale revision 拒绝。

当前规则只有固定、全天间隔（6h / 12h / 24h / 2d / 7d）。没有 dynamic、窗口、工作日/周末选择或 fallback 这套第二调度语言。暂停只停止未来自动调度，人工检查仍可申请；dismissed 目标禁止新工作，历史保留。`UNKNOWN` 或 `PARTIAL` archive coverage 不再产生 `baseline_required`，保存规则不创建 Work/Lease/Attempt。

### 4.3 页面与支持范围

Collection 仍为五个子面；目标页新增一个轻量“监控规则”按钮与 LIDS modal，不建独立设置页。Modal 首焦点、Tab trap、Esc、关闭 focus return、服务端错误后输入保留必须成立。Targets/Operations/Attention/Tasks/Runtime 使用相同 reason/revision/frozen refs；无数据库或写冲突不能 redirect 假成功。

验收只覆盖 1440 CSS px 桌面全屏；1280、390、手机、小于 13 寸屏幕与 responsive parity 均不进入本包。

## 5. UI Change Manifest

- 分类: 展示 + 交互 + 状态/语义 + 权限/行动，按权限/行动管理。
- 强度/Pattern: Collection L1；目标抽屉内 modal 为受限 L2 overlay；唯一主 Pattern `Collection Control`。
- 三秒答案: 当前最先阻断接活的是哪个可审计 reason；该目标当前规则 revision/模式/下一窗口是什么。
- 五秒动作: 在目标行打开“监控规则”，保存一版规则并看见 durable receipt；失败时留在原表单修复。
- 视觉 thesis: LIDS 白场研究仪器面；规则表单为一个连续 surface，状态靠中文+枚举+位置，不用装饰图案。
- CSS 策略: 复用现有 page-local vanilla CSS 与唯一 `--lgi-*` token；不改全站 token，不建第二套 Button/Status/Surface。
- 非目标: 新页面、Dossier、Evidence 内容、监控价值、机会、动态曲线、真实平台操作、移动端。

## 6. 表面、状态、依赖、验收矩阵

| 表面 | 常用态 | 失败/受限/部分态 | 验收 |
|---|---|---|---|
| Targets 行与规则 modal | current revision、自动观察、固定间隔、下一计划点、save receipt | DB unavailable、stale、conflict、validation | Rust render/HTTP + 1440 keyboard/browser |
| Runtime | station/install/account/capacity reason 与恢复责任 | not accepting、stale、version、unbound、unknown/cooling/login/restricted、quota/risk/busy | 同一 evaluator PostgreSQL tests + page equality |
| Operations | scheduler run/target decision、WorkOrder/Lease/Task/Receipt | skipped/deferred/rejected/retrying | durable rows only，刷新后不消失 |
| Attention | 只列有恢复动作的真实阻断 | reason code + owner/action | 不把 PARTIAL+VALID 或 DYNAMIC_UNAVAILABLE 当失败 |
| Tasks | frozen station/install/account/rule refs 与 timeline | Lease/dispatch invalidation/cancel/recovery | append-only history，不覆盖旧 Attempt |
| Producer check-in/report/claim | credential 持有、eligibility facts report、server decision | credential invalid/revoked、account stale/unknown | unit/contract/release tests；不访问平台 |

直接依赖是 additive `0034`、Rust evidence/control owner、local API/五个 Collection 读面、Browser Producer authoritative source/release、migration runner/schema fixture、PAGE/产品/LIDS change log/索引/progress。Package 1 的 Work Current/Corpus 仅做负向隔离，不修改其产品语义。

## 7. 实施顺序

1. RED→GREEN：`0034` 空库 migration、privacy/uniqueness/history/credential/rule receipt 约束。
2. RED→GREEN：单一 capacity evaluator；Authorization purpose/max_targets；Admission frozen refs；Lease/dispatch 时点重查。
3. RED→GREEN：rule command、固定 next-run schedule、scheduler queue/target receipt、失败/cancel/recovery。
4. HTTP/UI：规则 modal 与 durable receipt；Operations/Attention/Tasks/Runtime 共用读模型。
5. Producer：0.8.46 嵌套 observation/任务页复核合同，只上报最小的确定事实；build/package/verify/reproducibility。
6. disposable PostgreSQL、Rust workspace、1440 browser、plugin 与治理验证。

## 8. 停止条件

需要保存/展示 secret、raw account ID、Cookie、HTML 或平台自由错误；Producer 被要求成为账号/规则/quota/Admission authority；需要真实平台、共享 DB、`:3000`/launchd、Chrome reload、部署/merge；需要修改 0001–0033、Package 3 或 Evidence/monitoring value 域时，停止对应 lane 并报告。

## 9. 分层完成

| 层 | 当前状态 |
|---|---|
| Plan/Claim/Reality Matrix | VERIFIED |
| Code/schema/UI/plugin source | 已有 Package 2 来源已验证；本次 `0036` 源码只在 `codex/monitor-scheduling-clarity-001` 独立 worktree，尚未 merge |
| Automated/disposable PostgreSQL/1440 browser/release | VERIFIED（隔离范围；本轮追加 station-close/dynamic-round 回归；独立 headless Chrome 以 1440 CSS px 直接复核五面与规则失败 modal） |
| commit/push/PR/merge | VERIFIED：PR [#153](https://github.com/Himog0921/linggan-intelligence/pull/153) 已合入 `origin/main@84fd9498e18164011621ffabc81a2a421a52f7a1` |
| shared migration/runtime/Chrome/真实平台/deploy/Mog acceptance | 不由本次 V4 UI 扩展重新声明；须按各自实际证据单独核对 |

## 10. COLLECTION-FIVE-PAGE-V4-UI-001 · UI 验收反馈扩展

> 状态: 独立分支第二轮复审修正中；已获 commit/push/merge 与本机 `:3000` 刷新授权
> 最后核对: 2026-09-04
> 用户输入: `/Users/moglenny/Downloads/linggan-collection-field-workspace-v4-ia-cn.html#targets`

### 10.1 Claim

- Issue: `#149` 的五面 UI 验收反馈；不改变 Package 2 已冻结的数据与控制合同。
- Task ID / Claim: `collection-five-page-v4-ui-001` / `COLLECTION-FIVE-PAGE-V4-UI-001`
- Exact base: `84fd9498e18164011621ffabc81a2a421a52f7a1`
- Branch: `codex/collection-five-page-v4-ui`
- Worktree: `/Users/moglenny/proma/linggan-intelligence/.worktrees/collection-five-page-v4-ui`
- 允许: Collection 五页 Rust render、page-local CSS/JS、直接测试与合同文档；隔离非 `:3000` loopback 和 1440 CSS px 浏览器验证；Mog 于 2026-09-04 追加授权当前 exact head 的 commit/push/PR/merge，以及合并后本机 `:3000` API 刷新。
- 禁止: migration/schema/共享数据库、巡检与媒体 worker 重启、插件/真实平台、外部部署、Package 3、Evidence/Corpus 内容语义、监控价值与窄屏适配。

### 10.2 用户结果与设计方向

- 五页继续回答各自真实问题，并采用 V4 的桌面密度、ledger/Inspector 关系、页内控制条和主工作区。页面名只保留读屏 `h1`，页面读数只进共享 Context Bar；V4 的可见大标题与第二条五格读数不继承。
- 观察目标点击后仍打开右侧宽幅抽屉，默认核心继续是已交付的创作者生命周期；不恢复参考稿里的 Evidence tab、代表证据、监控价值或假档案分。
- 视觉 thesis: LIDS 白场研究仪器面，以硬边界、高密度表格、克制 signal 和右侧事实检查区形成采集运营工作站。
- 内容 thesis: Context Bar 中的可验证读数 → 筛选/模式 → 主列表或生产流 → 选中对象的解释/恢复区。
- 交互 thesis: URL 持有路由、筛选、抽屉与规则 modal；键盘/点击等价，动作只使用 80–160ms transform/opacity 反馈。
- CSS 策略: 页面几何只维护现有 `collection_workspace.css`、`target_drawer.css` 与 `collection_workspace.js`；共享 `shell.css` 仅由 Context KPI owner 增加可见 scope/source 子行。全部消费现有 `--lgi-*` / 兼容 `--v7-*` token，不引入 Tailwind、CSS-in-JS、字体、图标库或第二套 token。

### 10.3 Reality Matrix

| Claim / surface | 实施前状态 | 当前证据 | 本扩展动作 |
|---|---|---|---|
| 五个真实 Collection route | VERIFIED | `/collection/attention|targets|operations|tasks|runtime` 均由当前 Rust binary 服务 | 保留 route/URL，不改成 hash-only prototype |
| V4 全局 header / context / rail | LIDS OVERRIDES | 共享 shell 已是唯一 owner；LIDS 要求隐藏 `h1`、读数仅在 Context Bar | 只采用 Collection page scope 几何/密度，不复制或局部重写 shell，不采用参考的可见标题/第二读数条 |
| 待处理 ledger + inspector | ABSENT | durable recovery rows 已接通，但当前是整行平铺 | 用同一 projection 渲染左 ledger 与右事实 inspector；无选择时也不伪造默认事实 |
| 观察目标目录 + 右抽屉 | PARTIAL | 真实列表、规则 modal、生命周期抽屉已接通；页面缺 V4 标题/读数层级 | 重排真实字段与操作，保留 lifecycle、provenance、UNKNOWN 与 Coverage |
| 生产流 + 实时观测面 | PARTIAL | scheduler run/decision/Work/Lease 真实投影已接通；视觉仍是普通列表 | 映射为 V4 flow + dark instrument，但不制造 live event 或趋势 |
| 采集任务 ledger + inspector | PARTIAL | Task/Attempt/Package/Receipt timeline 和 frozen refs 已接通 | 将真实 task timeline 与冻结资源组合成 V4 双栏工作区 |
| 执行工位判断 + 关系/登记/认领 | VERIFIED / VISUAL PARTIAL | capacity、station、installation/account control 与本地动作已接通 | 只重排为 V4 判定、instrument、roster、inbound 层级 |
| Evidence / 监控价值 /假健康分 | FORBIDDEN | Issue #149、Package 1/2 合同与用户再次确认 | 对参考稿这些内容做负向回归，不渲染 |
| 数据库/API/控制语义 | NO CHANGE REQUIRED | 五面现有 projection 足以支撑本轮视觉复刻 | 若必须新增事实或 schema 则停止并报告 |
| 手机/小于 13 寸/1280/390 | OUT OF SCOPE | 用户已明确排除 | 只验收 1440 CSS px 桌面全屏 |
| 共享 runtime / DB / Mog 验收 | NOT VERIFIED | 本扩展尚未部署 | 隔离验证后分层报告，等待另行授权 |

### 10.4 验收矩阵

| 表面 | 自动证明 | 1440 隔离浏览器 | 人工验收 |
|---|---|---|---|
| 共享页层级 | 五页均有唯一读屏 h1、两个有范围 Context KPI 与真实 route；无可见标题或第二 header | 1440 无横向溢出，工具条/rail/主工作区清晰 | Mog 对 V4 视觉接近度判断 |
| Attention | durable reason/owner/action 与 inspector 一致 | 行选择、键盘焦点、未知/空态可读 | 恢复责任是否直观 |
| Targets / drawer / rule modal | 真实计数、UNKNOWN/0 分离、lifecycle 与 modal 合同回归 | 点击行开抽屉、Esc/focus return、生命周期首屏可见 | 生命周期是否成为抽屉核心 |
| Operations | run/decision/Work/Lease 来源不变，无模拟事件 | flow 与 dark instrument 同屏，无假实时滚动 | 生产状态是否一眼可判 |
| Tasks | Task/Attempt/Package/Receipt 与 frozen refs 不丢失 | ledger + inspector 信息无遮挡 | 任务结果与恢复是否可理解 |
| Runtime | capacity reason、账号隐私、local-only action 不变 | verdict/instrument/roster/inbound 层级成立 | 工位可用性是否一眼可判 |

停止条件：复刻需要引入模拟事实、Evidence 内容、监控价值、第二套 token/header、数据库或运行时变更；或发现现有 projection 无法如实支撑一个关键可见结论。

### 10.5 实施与验证回执

- 五个 Collection route 均保留唯一读屏 h1，并在共享 Context Bar 中只显示两个带可见 scope/source 的 KPI；可见大标题与第二五格读数已按 LIDS 复审结论删除。共享 `shell.css` 只由既有 KPI owner 承担子行样式，全局 token、header/rail 与 Corpus 页面未被重写。
- Attention 以 durable reason/owner/action 组成 ledger + Inspector；Tasks 以 Task/Attempt/Package/Receipt 与 frozen Work refs 组成 ledger + tabs；Operations 真实替换 body slot，并以 scheduler decision / Work / Lease 投影组成 flow + dark instrument。
- Targets 保留 URL-owned drawer、monitor-rule modal、focus return 与 creator lifecycle；合法 `archived | monitoring | paused | dismissed` 状态不再错误显示为 UNKNOWN。
- Runtime 继续使用同一 capacity evaluator，并以真实 station/install/account/claim-window 读数替换 provisional readout。
- 第二轮复审修正继续固定：Context scope/source 必须可见；新增 V4 spacing 只走 4/8/12/16/24/32px LIDS ladder；global LIDS focus 是新增控件的唯一 owner；生命周期辅助文案不使用字符图标。
- Task Inspector 的状态文案与语义 class 随选择同时更新；零任务、控制 schema 不可用与读取失败均进入明确终态，不保留“正在读取”。Frozen Work 先选最近 Work / Lease，再展开该 Lease 的全部 Task，因此同一 Work 的两个 Task 各有精确 frozen tab；Operations 的 Work/Lease 阶段仍由 decision 计数，不被 Task 展开重复。
- 自动检查覆盖五页隐藏 h1 + 2 个带可见 scope/source 的 Context KPI、control body slot、Operations 模式隔离/恢复计数、ledger/Inspector、Task-scoped Frozen Work、tab ARIA/键盘、kind-aware lifecycle、spacing/focus owner 与 Evidence/监控价值模块负向；页面 JS 通过语法检查。
- 首版一次性 PostgreSQL 16 / 隔离 `:3311` / 1440×900 浏览器证明只属于后来被 exact-head 复审拒绝的首版，不是当前修正版证明。当前修正版已并入 `origin/main@e5c5ec5`，workspace 全量、完整 Work/Material/Corpus/Topic/API PostgreSQL 69 项、dispatch 8 项、Collection Control `8 + 11` 项通过；每套临时数据库、container、volume 均已清理。
- 当前修正版无数据库隔离 API `:3311` + Chrome 152 强制 1440×900，五页均 `clientWidth=scrollWidth=1440`，每页 2 个带直接 scope/source 的 KPI、1 个 1×1 absolute 读屏 h1、0 个第二 readout strip、0 条 loading 残留，应用页面 console warning/error/exception 为 0。它只证明当前 head 的桌面几何与终态；有数据交互由 exact PostgreSQL/source tests 证明，不沿用已拒绝首版的浏览器结果。
- 未证明：真实数据密度、共享数据库、Browser Producer/真实平台与 Mog 业务验收。本扩展不修改 migration/schema，也不处理窄屏；第三轮双轴 exact-head 复审、commit/push/merge 与本机 `:3000` 刷新的实际证据仍须按动作补记。
