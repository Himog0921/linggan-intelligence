# COLLECTION-CONTROL-CLOSURE-001 · 平台账号准入与巡检规则闭环

> 状态: 活跃计划
> 最后核对: 2026-09-04
> 适用范围: Issue #149 / Package 2；只支持 1440 CSS px 桌面全屏
> 事实来源: Issue #149 Claim、`origin/main@c5158b14f5fc2313dbd3dc94670083500e767ced` 的 migration/Rust/Collection/Browser Producer、PAGE-COLLECTION-001 与 LIDS v7
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实代码/数据库/测试、ACCEPTED 决定与 Issue #149 冻结合同

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
| dynamic cadence | ABSENT | 无两轮 comparable qualification | `<2` 或 lens/Coverage/time 不合格均 `DYNAMIC_UNAVAILABLE`，显式 24h fallback |
| scheduler | PARTIAL | 每 tick `LIMIT 50` 且无 target receipt | cursor/fair scan + 每目标 durable decision receipt/backoff |
| Collection read surfaces | PARTIAL | Tasks 真实；Operations/Attention 仍多为空壳 | 只投影 scheduler/decision/WorkOrder/Lease/Task/Receipt/account capacity 事实 |
| target baseline | PARTIAL | 零 record/空 Coverage 仍可能推进 | 空 Coverage 不得 baseline ready |
| shared DB/runtime/Chrome/真实平台 | NOT VERIFIED | 本 Claim 禁止 | 只用 disposable PostgreSQL 与隔离非 3000 browser |

## 4. 冻结合同

### 4.1 统一容量判定

服务端单一 policy 固定 `minimum_plugin_version=0.8.34`、installation/account positive freshness `20m`、station daily accepted-note budget `200/day`（Asia/Shanghai，仍以 station 为唯一主体）。判定顺序为 risk → station active/accepting → active installation → credential → version → freshness → capability → bound account → eligibility current/fresh → live-Lease busy → station budget。每次返回闭集 reason code；UNKNOWN 一律阻断。`busy` 只从 live Lease 派生，不接受 Producer 上报。

账号 eligibility 闭集为 `usable | cooling | needs_login | restricted | unknown`。Producer 只提交版本化 signal 与 raw account identity 的瞬时 loopback body；服务端使用部署密钥计算 versioned keyed digest，raw identity 绝不持久化或返回。账号 binding 与 eligibility history 只追加；current 是重算读取责任。

### 4.2 巡检规则 v1

每个 target 的规则 revision 只追加；active pointer 与当前 monitoring/lifecycle 同事务变化。命令携带 expected revision、idempotency key、payload digest、actor/source；相同 key+payload replay，不同 payload conflict，stale revision 拒绝。模式闭集 `manual_only | fixed | dynamic`；时区只接受 `Asia/Shanghai`；fixed/fallback `6h–7d`、默认 24h；v1 拒绝跨午夜窗口；默认全天并包含周末。

暂停只停止未来自动调度，人工检查仍可申请；dismissed/停止目标禁止新工作，历史保留。creator 无合格 baseline 时返回 `baseline_required`；keyword 不套 creator baseline。保存规则不创建 Work/Lease/Attempt。

dynamic 只消费同 target/surface/ranking/task contract/rule revision/account lens 下至少两轮 accepted、Coverage 合格且 exact publication time 可比的事实。任何相对/未知时间、scan limit、Coverage 不足或 lens 变化都为 `DYNAMIC_UNAVAILABLE`，调度按规则中显式 fixed fallback 运行；首版不从监控价值、Dossier、Evidence 或 Opportunity 读取任何输入。

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
| Targets 行与规则 modal | current revision、模式、窗口、fallback、save receipt | DB unavailable、stale、conflict、validation、baseline required、dynamic unavailable | Rust render/HTTP + 1440 keyboard/browser |
| Runtime | station/install/account/capacity reason 与恢复责任 | not accepting、stale、version、unbound、unknown/cooling/login/restricted、quota/risk/busy | 同一 evaluator PostgreSQL tests + page equality |
| Operations | scheduler run/target decision、WorkOrder/Lease/Task/Receipt | skipped/deferred/rejected/retrying | durable rows only，刷新后不消失 |
| Attention | 只列有恢复动作的真实阻断 | reason code + owner/action | 不把 PARTIAL+VALID 或 DYNAMIC_UNAVAILABLE 当失败 |
| Tasks | frozen station/install/account/rule refs 与 timeline | Lease/dispatch invalidation/cancel/recovery | append-only history，不覆盖旧 Attempt |
| Producer check-in/report/claim | credential 持有、eligibility facts report、server decision | credential invalid/revoked、account stale/unknown | unit/contract/release tests；不访问平台 |

直接依赖是 additive `0034`、Rust evidence/control owner、local API/五个 Collection 读面、Browser Producer authoritative source/release、migration runner/schema fixture、PAGE/产品/LIDS change log/索引/progress。Package 1 的 Work Current/Corpus 仅做负向隔离，不修改其产品语义。

## 7. 实施顺序

1. RED→GREEN：`0034` 空库 migration、privacy/uniqueness/history/credential/rule receipt 约束。
2. RED→GREEN：单一 capacity evaluator；Authorization purpose/max_targets；Admission frozen refs；Lease/dispatch 时点重查。
3. RED→GREEN：rule command、baseline Coverage、scheduler cursor/target receipt、失败/cancel/recovery。
4. HTTP/UI：规则 modal 与 durable receipt；Operations/Attention/Tasks/Runtime 共用读模型。
5. Producer：0.8.34 credential/report/claim contract，只上报最小 signal；build/package/verify/reproducibility。
6. disposable PostgreSQL、Rust workspace、1440 browser、plugin 与治理验证。

## 8. 停止条件

需要保存/展示 secret、raw account ID、Cookie、HTML 或平台自由错误；Producer 被要求成为账号/规则/quota/Admission authority；需要真实平台、共享 DB、`:3000`/launchd、Chrome reload、部署/merge；需要修改 0001–0033、Package 3 或 Evidence/monitoring value 域时，停止对应 lane 并报告。

## 9. 分层完成

| 层 | 当前状态 |
|---|---|
| Plan/Claim/Reality Matrix | VERIFIED |
| Code/schema/UI/plugin source | VERIFIED |
| Automated/disposable PostgreSQL/1440 browser/release | VERIFIED（隔离范围） |
| commit/push/Draft PR | NOT VERIFIED（本实现 agent 不执行） |
| shared migration/runtime/Chrome/真实平台/deploy/merge/Mog acceptance | NOT AUTHORIZED / NOT VERIFIED |
