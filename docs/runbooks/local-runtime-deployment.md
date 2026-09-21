# 本机常驻服务部署手册

> 状态: 权威当前
> 最后核对: 2026-09-21
> 适用范围: 本机三个 launchd 常驻服务（API / 巡检 worker / 媒体 worker）的运行来源、更新方式、故障处置与运行诊断
> 事实来源: Mog 于 2026-09-03 的明确要求「本地以 `/Users/moglenny/proma/linggan-intelligence` 为准，跟远端同步，不要到处复制」、当前 launchd 配置与实际运行验证、2026-09-17 更新入口用旧副本失败的实测（见 §3）、2026-09-21 按 COLLECTION-UPGRADE-001 S4 的实现与隔离库用例写成 §7（**样例为手工构造**，不是运行抓取；其余各节未重新实测，最后核对的口径见各节标注的日期）
> 冲突时以谁为准: 实际运行输出与 launchd 当前加载的配置；本手册不授予平台访问或迁移执行权限

## 1. 唯一的两个目录

```
/Users/moglenny/proma/linggan-intelligence          ← 开发。就是本地 main，与 origin/main 同步
~/Library/Application Support/Linggan Intelligence/
    runtime-main/                                   ← 运行。detached，跟随 origin/main
```

**不再有第三个副本。** 2026-09-03 之前的做法是每次部署新建一个按 commit 命名的冻结快照
（`runtime-source/linggan-intelligence-origin-main-<sha>`），并把启动脚本里的路径改过去。
它累积到 28 份共 21GB，而且三个服务各自指向自己那次部署的快照——当天实测 API 在 `ccf7cca`、
两个 worker 还停在 `7a792c9`，同一台机器上跑着两个版本的代码。该模式已整体废弃。

开发目录之所以与运行目录分开，只有一个理由：**开发时的未提交改动不应该影响常驻服务**。
运行目录永远不接受手工编辑，每次启动都会被 `git reset --hard` 覆盖。

## 2. 安装（重装机器或换机器）

部署脚本住在仓库 `scripts/runtime/`，不是散在 `~/Library` 里的手写副本。全新机器上：

```bash
git clone <repo> && cd linggan-intelligence
cp env.example .env && $EDITOR .env      # 填数据库口令
./scripts/dev-db.sh up                   # 起 PostgreSQL
./scripts/local-runtime.sh migrate       # 应用迁移
./scripts/runtime/install.sh             # 建运行目录、写 launchd、启动三个服务
```

`install.sh` 幂等：已装好时重跑只刷新 plist 并重启。它**不**跑迁移、不装依赖、不碰远端环境。

## 3. 更新方式

CI-AUTO-004 源码将版本切换改为先等待评论调用落账。以下是本包发布后的更新合同，不表示当前常驻进程已经使用这版协议。

```bash
./scripts/runtime/install.sh
```

`install.sh` 先请求巡检 worker 停止领取新模型任务，等待它将当前调用和用量落账，并核对与 PID 对应的退出回执。成功后才生成绑定起止 revision 的更新许可，再同步、构建并重新加载三个服务。未确认退出时停止更新，不把未知调用费用当作零。

**入口脚本必须来自与 `origin/main` 同步的副本。** 2026-09-17 实测踩过：开发目录停在 `c94a3ec`，那份 `install.sh` 里还留着一段「兼容仍会构建已退休二进制」的补丁（`install.sh:168-177`），`cargo build --bin linggan-comment-worker` 当场报 `no bin target named ...` 并以 101 退出。**失败点在 drain 之后、重启服务之前**——巡检 worker 已被 `bootout` 且没有装回去（`launchctl list | grep linggan` 只剩两个 label），另外两个服务仍是旧进程。它不是「什么都没发生」的失败，而是「停了一个、旧了两个」。

所以更新前先确认入口副本不落后，或者直接用运行目录里那份：

```bash
git -C ~/Library/Application\ Support/Linggan\ Intelligence/runtime-main log -1 --date=short --format='%h %ad %s'
~/Library/Application\ Support/Linggan\ Intelligence/runtime-main/scripts/runtime/install.sh
```

`install.sh:152-159` 对「安装入口就在运行 worktree 里」的幂等场景有显式处理，`install.sh:161-166` 也写明「旧运行目录可能含已退休的二进制，因此从源 checkout 跑目标 revision 的同步器」。无论用哪份入口，跑完必须核对三件事：`launchctl list | grep linggan` 是**三个** label、`git -C $runtime_dir log -1` 等于目标 revision、三个进程的启动时间是刚才而不是上一次部署。

启动时 `scripts/runtime/sync.sh` 依次做：

1. 取同步锁（三个服务同时启动时串行化）；
2. `git fetch origin main`；需要改变 revision 时核对并消费匹配的 worker 更新许可，之后才切换；
3. 检查迁移台账，有未应用的迁移就**拒绝启动**（见下）；
4. 在锁内构建三个二进制；
5. 释放锁，各服务 `exec` 自己的二进制。

拿不到网络时不会让服务起不来：fetch 失败会记录一行并继续用当前 revision。

## 4. 迁移仍然不由服务执行

这条约束从旧模式沿用，理由没变：**一个开机自启的服务不该顺手改数据库结构**。

但代码现在会自动跟到 main，所以同步脚本必须主动检查——否则新代码撞上没跑的迁移，
报出来的是一堆 SQL 错误，而不是一句「该迁移了」。检查到有未应用迁移时服务拒绝启动，
日志里会列出是哪几个，处置方式固定：

```bash
cd /Users/moglenny/proma/linggan-intelligence
./scripts/local-runtime.sh migrate
```

**已知边界**：开机时 API 可能比 Docker PostgreSQL 先起来。此时读不到台账，脚本会记录
「读不到迁移台账」并**跳过检查继续启动**——硬拦会让服务在数据库没起来时永远起不来。
因此开机后第一次启动不保证迁移检查生效；手工重启一次即可获得完整检查。

## 5. 三个服务

| 服务 | launchd label | 二进制 |
|---|---|---|
| API（3000 端口） | `com.linggan-intelligence.local-runtime` | `linggan-api` |
| 巡检调度 | `com.linggan-intelligence.patrol-worker` | `linggan-worker` |
| 媒体处理 | `com.linggan-intelligence.media-worker` | `linggan-media-worker` |

三者由同一个 `scripts/runtime/launch.sh <binary>` 启动、共用同一份 `sync.sh`。更新后仍须分别核对三个 PID 和构建 revision；共用脚本不证明当前存活进程已经同步。

日志在 `runtime-logs/{api,worker,media-worker}.{out,err}.log`。

## 6. 处置

### 服务起不来，且日志里一行都没有

先看 launchd 自己的报错，不要看服务日志——服务还没跑到写日志就死了：

```bash
launchctl print gui/$(id -u)/com.linggan-intelligence.local-runtime | grep -iE 'working directory|last exit'
```

`last exit code = 78: EX_CONFIG` 且 `working directory` 指向一个不存在的目录，就是这个原因。
2026-09-03 清理快照时真实发生过：launchd 内存里加载的 job 仍带着指向已删快照的
`WorkingDirectory`，于是它在执行脚本前就失败。**服务日志里那时显示的报错是几天前的旧内容**，
按它排查会跑偏。

处置是重新加载 plist：

```bash
launchctl bootout gui/$(id -u)/com.linggan-intelligence.local-runtime
launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/com.linggan-intelligence.local-runtime.plist
```

### 不要用 `pkill` 停服务

`pkill -f linggan-api` 会同时打到开发用的临时实例和常驻服务。版本更新使用上述受控 install 入口，不以 `kickstart -k` 绕过模型落账。开发用 `local-runtime.sh serve` 同样先等待自己启动的 worker，成功才结束其他子进程；失败保留回执并说明未完成停止。

### 同步锁卡住

`runtime-main/.sync.lock` 是目录锁。超过 5 分钟的锁会被下次启动自动清理；要手工清就
`rmdir` 它。

## 7. 运行诊断：一个 tick 号串一条链

巡检 worker 每轮 tick（60 秒扫一次）有一个号，落在 `collection_scheduler_run.scheduler_run_ref`。
同一个号写在它的步骤行、这一步排出的目标级决定、以及那些工单上；日志里的事件行用 `tickRef`
带着它。**一条链断在哪里，从这一个号出发就能查出来，不需要累计任何日志。**

三个查询（`psql`，把 `<run>` 换成 run 号；工单号由上一条给出）：

```sql
-- ① 这一轮四步各自的结果。失败与「没轮到」在这里分开，失败带的是受限类别，不是报文。
SELECT step_key, outcome, error_class, skipped_reason,
       considered_count, produced_count, skipped_count
  FROM collection_scheduler_run_step
 WHERE scheduler_run_ref = '<run>'
 ORDER BY started_at;

-- ② 巡查这一步把哪些目标排成了活、各自给了什么理由。
SELECT target_ref, outcome, reason_code, work_order_ref
  FROM collection_scheduler_target_decision
 WHERE scheduler_run_ref = '<run>';

-- ③ 排出的工单此刻在哪条队列、什么时候该跑。
SELECT work_order_ref, target_ref, queue_state, dispatch_lane, scheduled_for
  FROM collection_work_order
 WHERE work_order_ref = '<工单号>';
```

同一轮在 `runtime-logs/worker.out.log` 里的三行（一行一个 JSON 对象）：

```json
{"ts":"2026-09-21T09:00:00Z","service":"worker","revision":"9a1c2f4","event":"tick","tickRef":"<run>","outcome":"failed"}
{"ts":"2026-09-21T09:00:04Z","service":"worker","revision":"9a1c2f4","event":"tick_step","tickRef":"<run>","stepKey":"media_acquisition","outcome":"failed","errorClass":"sqlstate_42703","durationMs":12}
{"ts":"2026-09-21T09:00:05Z","service":"worker","revision":"9a1c2f4","event":"tick_step","tickRef":"<run>","stepKey":"patrol","outcome":"ok","durationMs":41,"considered":1,"produced":1,"skipped":0}
```

**这三行是手工构造的脱敏样例，不是从运行日志里抓的**：形状按实现写（`<run>` 是省略的号），
用来对照字段，不构成任何一次真实故障的证据。同一条链在隔离 PostgreSQL 上有可复现的用例
（`crates/evidence/tests/scheduler_tick_postgres.rs` 的
`one_tick_ref_strings_the_failure_and_what_it_queued_without_reading_the_logs`）。

读法：`outcome` 是「这一步跑完没有」，`failed` 与 `skipped` 是两件事——`failed` 是跑了但坏了
（带 `errorClass`），`skipped` 是这一步自己的表不在、**没轮到**（带 `reason`，如
`schema_unavailable`）。一步失败不连坐其余三步：上面例子里媒体投影坏了，巡查照样把活排了出去。
计数只在 `outcome="ok"` 的行上出现；失败、没轮到、以及**开始了却没写完结（进程被杀）**的行
一律没有计数——「数过了是零」与「没数过」不能互相冒充。库里 `outcome IS NULL` 的那一行就是
最后这种情况，它既不写成 `ok` 也不写成 0。

### 7.1 字段白名单

事件只能带这 13 个字段（`crates/evidence/src/runtime_event.rs` 的 `EVENT_FIELD_WHITELIST`，
结构体里没有「随手塞一个键」的入口）：

| 字段 | 含义 | 边界 |
|---|---|---|
| `ts` | 事件发生的时刻 | **进程时钟**（UTC、秒），不是数据库时钟 |
| `service` | 谁在说话 | `worker` / `media-worker` |
| `revision` | 这份进程是哪个部署 | 读不到写 `unknown`，不拿别的东西冒充 |
| `event` | 事件类型 | `startup` / `readiness` / `tick` / `tick_step` |
| `tickRef` | 这一轮 tick 的号 | 与 `scheduler_run_ref` 是同一个值，串证的入口 |
| `stepKey` | 哪一步 | 四步之一；`media_acquisition` 这类受限词 |
| `outcome` | 这一步（或这一轮）的结局 | `ok` / `failed` / `skipped` |
| `reason` | 没轮到的原因 | 受限码 `[a-z0-9_]{1,32}`，如 `schema_unavailable` |
| `errorClass` | 失败的类别 | 受限码，如 `sqlstate_42703`；**不是报文** |
| `durationMs` | 这一步花了多久 | 进程时钟测出来的毫秒数 |
| `considered` / `produced` / `skipped` | 这一步数过的三个计数 | 只在跑完的步骤上出现 |

两条纪律：**引用只到 tick 号为止**——目标、工单、租约、任务、提交这些引用住在数据库里，
把副本抄进日志只会多出一份会漂移的东西，也会让「什么算敏感」在第二个地方重新判一遍；
**理由是受限码**——不规则形状（空格、斜杠、问号、中文、连接串）一律记 `unclassified`，
不把散文按字符挑成一个「看起来像原因码」的词。

页面结构自检的快照另有自己的一份字段表（`crates/evidence/src/selector_health.rs` 的
`SELECTOR_HEALTH_SNAPSHOT_FIELDS`，与插件 `src/shared/selectorHealth.js` 里的同名常量是同一份
合同的两侧），一条平台记录九个键：`platform`、`pageType`、`capability`、`checkedAt`、
`verifiedAt`、`checkedCategories`、`missingCategories`、`staleCategories`、`failureCounts`
（服务端比插件多最后一项：连续次数由本机留存层补上，页面看到的那一次里没有「连续几次」）。

快照里**没有**这两样，也不要去找：`pluginVersion`（报到自述里本来就有，存进
`plugin_installation.plugin_version`——同一件事不能有两个答案）、`ruleVersion`（今日它就是
`verifiedAt` 的别名，另起一个名字会让人以为存在一套独立的规则版本）。

白名单不是承诺，是测试判据：Rust 侧 `an_event_carries_only_whitelisted_fields` 与
`every_field_the_event_can_carry_is_whitelisted_and_used` 断言事件的字段**逐字等于**那张表
（多一个、少一个都变红），串证用例再从真实 tick 路径上验一遍；
`nothing_outside_the_field_whitelist_survives_into_the_snapshot` 断言一份夹带了选择器串、
DOM 文本、带签名的地址的载荷收下来之后那些键一个不剩。插件侧另有 node 用例断言出门前
只带快照字段。

### 7.2 这一节不证明什么

- 样例是**描出来的**，不是抓来的：它证明字段表长得对、查得到，不证明任何一次真实故障被这样
  串起来过（那要有一次真实提交，属真实平台访问，未授权）；
- 只覆盖本机三个常驻服务，不含部署机（Mac mini）与任何远端环境；
- 日志只带引用，**不带目标与工单的内容**：要回答「排出给谁、排的是什么」必须真的查库；
- 库里的步骤、决定与工单是采集链的事实，日志只是进入那条链的入口。两者不一致时以库为准，
  并把不一致本身当作一次待查的缺陷。


## 8. 本手册不证明什么

- 不证明部署机（Mac mini）或任何远端环境采用同一模型——这里只描述本机；
- 不授权服务自行执行迁移、访问平台或修改数据库；
- 不保证开机首次启动时迁移检查一定生效（见第 4 节的已知边界）。


## 9. COMMENT-RESEARCH-RESET-001 V1 的发布前条件

此段规定当前唯一评论研究路径；CI-AUTO-004 的 daily/Task B/P4/本地 Python 聚类实现是历史记录，不是 runtime 依赖。

1. 先在交付 head 执行 `bash scripts/test-comment-research-postgres.sh`、`npm test --prefix apps/pi-adapter` 与 `cargo check -p linggan-intelligence -p linggan-api -p linggan-worker`。前者会在随机隔离 PostgreSQL 中从历史 migration 完整应用到 `0085`，验证旧研究派生被显式退役、V2 Atom→Stable Problem 的受限归并、归属修正会生成新 derivation head、冻结 Run 仍可完成而整版旧结果撤出可读投影；它绝不访问开发库。
2. V1 不需要 Python/HDBSCAN/Leiden runtime。Pi adapter 只需在实际 runtime checkout 执行 `bash scripts/runtime/prepare-pi-adapter.sh --install`，再用 `--check` 验证固定 Node/npm 依赖。
3. 在运行目录更新前，通过本节既有受控 `install.sh` 取得 worker drain 回执。旧 worker 的在途调用不能在删除旧表期间继续写入；未结束调用必须保留 invocation ledger 的未知用量事实，不能计作零。
4. 更新后的代码仍不能自行迁移。取得 drain 后，从 canonical main 使用第 4 节的 `./scripts/local-runtime.sh migrate` 显式应用 `0068`–`0085`。`0069` 是终态删除 migration：清除旧研究派生和可能已存在的早期 V1 派生结果，不使用 `CASCADE`，并保留 Raw Comment/Evidence、作者归属、来源资格、通用模型连接/配置、embedding 设置和 invocation ledger；`0070` 固化 V1 derivation identity/read boundary：新 Run 选当前 head，冻结 Run 保留原输入可读性，不迁移或恢复任何旧研究结果；`0084`/`0085` 只增添 V2 Frame、closed-world Resolution 和可恢复 pair checkpoint，绝不重写既有 Problem 或原始评论。
5. migration 后安装并刷新服务，逐项核对 exact Git revision、migration ledger、三个 PID、`/health` 和 `:3000` 的 V1 页面。旧 route/page 不应再可访问。
6. 发布、保存连接或保存 policy 均不会调用真实模型；连续排程没有入口且保持关闭。只有用户在页面主动点击“开始研究”才会冻结 V1 Run 并发送已获许可的评论。

真实 provider 的语义质量与 Mog 业务验收不由上述基础设施步骤推断。


## 8. COLLECTION-UPGRADE-001 分阶段刷新与停止新访问

先应用 0096–0101，再从已合并 main 的最新脚本执行部署。部署源 `.env` 明确设
`LINGGAN_COLLECTION_UPGRADE_PHASE=recovery`；缺失/未知值同样为 recovery。`install.sh`
会同步该配置并受控 drain/restart。核对 `/health.collectionUpgradePhase`、readiness、
导航准备契约和部署 revision 后，才将部署源配置改为 `governance`，再次受控刷新。
recovery 下旧包的 Attempt 重放与 Submission 保持开放，不应清空浏览器数据库。

插件从本次 0.8.55 release-manifest 指定 ZIP/源码 dist 切换，保留同一个扩展 ID 与用户数据。
核验加载路径、版本和新的工位报到，不能把 ZIP 已生成写成实际插件已升级。

有问题时使用**新版二进制切回 recovery**：它能识别新台账、准备身份与 input_blocked。
不得重置远端 main，也不执行 down migration。旧二进制会将 input_blocked 误读为排队；
未经新库兼容实证不得切回旧二进制。恢复阶段的 PostgreSQL 用例证明禁止新 claim 时旧包仍可接纳。

历史处置工具默认只读（命令运行环境需设置正确的 `LINGGAN_LOCAL_DATABASE_URL`）：

```bash
cargo run --locked -p linggan-worker --bin linggan-collection-repair > /tmp/collection-repair-preview.json
# 默认只展示最近 100 个对象；truncated=true 时可按原 taskId 精确查看：
cargo run --locked -p linggan-worker --bin linggan-collection-repair -- --task-id <UUID>
# 经逐项批准后的显式应用；UUID/hash 必须来自同一原始预览：
cargo run --locked -p linggan-worker --bin linggan-collection-repair -- --apply /tmp/collection-repair-preview.json --batch-id <UUID> --preview-hash <SHA256>
```

默认预览只 SELECT；应用逐项重新核对并输出批次/哈希/结果。每批最多 100 个对象，
每项锁等待 2 秒、语句 5 秒。冲突跳过而不强制重试，需重新预览；已成功项重放无额外事件。
仅停止有明确作用域、缺输入且尚无任何在途材料的 pending 通道。旧 released/in_progress、
已开始或有准备会话的对象不改写；本地 terminal/缺历史身份材料是 NOT_OBSERVED，需在原浏览器
另作逐项核验，不得据服务端预览批量复活。输入历史未知的失败预算不追溯猜造。
保存预览与应用 JSON 作为运维回执，原 Package、Receipt、失败历史均保留。
