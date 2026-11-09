# Linggan 状态机清单（第一批 5 台）

> 状态: 权威当前
> 最后核对: 2026-11-09
> 适用范围: 采集任务、工单、AI 批次、观察目标、研究底稿这 5 个对象的"状态全集、谁能改、合法转移、写入点"；第 7 节另记三个容易读错的名字（同样只描述现状）；不覆盖第一批之外的其它状态机，不定义新规则
> 事实来源: `database/migrations/` 0005–0193 的 CHECK 约束与触发器（含被后续迁移 DROP 替换的旧定义）、`crates/evidence` 与 `crates/intelligence` 当前代码、`apps/web` 当前展示逻辑；逐条证据标在每节末尾
> 冲突时以谁为准: 数据库约束与当前代码（二者本身冲突时，数据库为准，因为它是最后一道闸）；本文只描述现状，不改变现状

## 0. 这份文档回答什么

一个"状态机"就是一件东西从开始到结束能经过的几种状态。本项目几乎所有状态都只写在数据库的 `CHECK` 里和代码的 `WHERE` 守卫里——**值域是明确的，但"什么情况允许从 A 变到 B"没有写在任何一处**，散在 2–9 个文件里。

本文对每台机器只写 5 件事：

1. 这个对象是什么（一句话）
2. 有哪几个状态（状态集合表）
3. 谁允许改（哪个命令 / 哪个后台模块）
4. 合法转移（A→B 可以，A→C 不行）
5. 写入点在哪几个文件

本文**不是**数据库设计文档（DDL 在 `database/migrations/`），**不是**新规则（不改变任何行为），**不是**一次性报告。

第 7 节单独收集三个"名字看似一个意思、实际是另一个意思"的地方。它们不是新的状态机，而是读状态时最容易踩的坑。

自动检查：`node scripts/check-state-machines.mjs` 把本文"状态集合"表与数据库约束、代码枚举逐项比对，任何一边多出或缺少一个值都会报错。检查范围与盲区见第 6 节。

## 1. 采集任务 collection_run

**是什么**：用户点一次"抓这个领域 / 这个作者"产生的任务。它有两个互不替代的轴——用户意图（`control_intent`）和系统进度（`phase`）。

### 状态集合

| 表 | 列 | 允许状态 |
| --- | --- | --- |
| collection_run | control_intent | `running`, `paused`, `stopped` |
| collection_run | phase | `discovery`, `materials`, `ended`, `blocked` |
| collection_run_order | stage | `discovery`, `detail`, `comments`, `supplement` |

### 合法转移

| 从 | 到 | 条件 | 写入点 |
| --- | --- | --- | --- |
| `running` | `paused` | 用户点暂停 | `collection_run.rs:471` `command_run` |
| `paused` | `running` | 用户点继续 | 同上 |
| `running` / `paused` | `stopped` | 用户点停止 | 同上 |
| `stopped` | 任何 | **数据库拒绝**：`stopped collection run cannot restart` | `0125_collection_single_run.sql:72-74` 触发器 |
| `discovery` | `materials` | 发现工单完成，且不是"作者预览" | `collection_run_progress.rs:92` |
| `discovery` | `ended` | 发现工单完成，且来源是 `creator_preview` | 同上 |
| `discovery` | `blocked` | 发现工单被取消 | `collection_run_progress.rs:68` |
| `materials` | `ended` | 所有作品都已收完或已放弃 | `collection_run_progress.rs:239` |
| `blocked` | 任何 | **无写入点**——`blocked` 目前是终点 | 生产 Rust 代码里写 `phase='blocked'` 仅 1 处 |

### 谁允许改

- 用户命令：`command_run`（暂停 / 继续 / 停止 / 调整篇数上限）
- 后台推进：`advance_runs` → `freeze_discovery` / `queue_materials`（`collection_run_progress.rs`）。两个驱动：API 进程里每 15 秒一次的推进循环（`collection_run_api.rs:80-99`），以及 worker 的关键词补采步骤（`apps/worker/src/keyword_details.rs:17`）。两边靠一把咨询锁互斥，同时跑也安全。
- 页面显示是**再算一遍**，不是直接读：`ended = (control_intent='stopped' OR phase='ended')`、`needs_attention = (phase='blocked' OR 任一作品有 gap_reason)`（`0125:191-192`），前端 `tasksApi.ts:61-68` 又按自己的顺序重排成中文标签。

### 写入点

`crates/evidence/src/collection_run.rs`、`crates/evidence/src/collection_run_progress.rs`（共 2 个文件）

### 已知缺口

暂停后阶段标签仍可能前进一次：`advance_runs` 的筛选条件（`collection_run_progress.rs:16`）为"`running` 且处于 discovery/materials"**或**"discovery 且发现工单已完成"，后者没有检查暂停。影响限于：暂停期间仍可能完成一次"发现→材料"的冻结（`scope_frozen_at` 会被写上）。材料阶段的后续排队有 `control_intent='running'` 守卫（`:32-36`），所以不会继续派发工作。

## 2. 工单 collection_work_order

**是什么**：授权"去做一件具体采集"的凭据。一张工单从排队到结束是一条队列状态；授权本身由"租约"承载，租约到期就失效；租约下面挂着真正要执行的任务；详情页另有一张会话表防重复导航。

这一节涉及 4 张表，它们是一层套一层的关系，不是同一层状态。

### 状态集合

| 表 | 列 | 允许状态 |
| --- | --- | --- |
| collection_work_order | queue_state | `legacy`, `queued`, `leased`, `completed`, `cancelled` |
| collection_work_order_lease | release_reason | `completed`, `partial`, `expired`, `revoked`, `station_unavailable`, `dispatch_start_failed`, `execution_locator_unavailable`, `input_blocked` |
| collection_work_order_lease_task | execution_state | `pending`, `in_progress`, `completed`, `unavailable`, `blocked`, `input_blocked` |
| collection_detail_page_session | state | `authorized`, `navigation_started`, `navigation_committed`, `delivery_pending`, `finished`, `stopped` |
| collection_detail_page_session | stop_reason | `navigation_state_unknown`, `owner_unavailable`, `page_unavailable`, `risk_stop`, `delivery_terminal`, `detail_page_url_invalid` |

租约表没有 `state` 列：它用 `released_at` 是否为空表示"还活着"，`release_reason` 表示"为什么结束"，并且数据库强制"未结束的租约一张工单最多一份"（`0010_work_order_lease.sql:33-34` 唯一索引）。`release_reason` 从 4 个值长到 8 个值；迁移 0097 加的 `input_blocked` 明确是"停"，不是"可重试的冷却"（`0097:172-173` 注释）。

### 合法转移（queue_state）

| 从 | 到 | 条件 | 写入点 |
| --- | --- | --- | --- |
| 新建 | `queued` | 工单入队 | `work_order_lease.rs` / `acquisition_chain.rs` |
| `queued` | `leased` | 某台工位领到租约 | `work_order_lease.rs:356`、`acquisition_chain.rs:2541` |
| `leased` | `queued` | 租约到期、释放或工位下线 | `dispatch.rs:1661`、`dispatch.rs:2177`、`work_order_lease.rs:639` |
| `leased` | `completed` | 任务全部完成 | `dispatch.rs:1497`、`dispatch.rs:1565`、`work_order_lease.rs:459` |
| `queued` | `cancelled` | 工单过期、任务被停止或调度失败 | `dispatch.rs:2202`、`acquisition_chain.rs:3516`、`collection_run.rs:474` |
| `leased` | `cancelled` | 租约期间的止损 | `dispatch.rs:1576`、`dispatch.rs:2222` |
| `completed` | `cancelled` | **唯一反直觉的一条**：发现"没有任何执行任务真的完成"时，把已完成降级为取消 | `dispatch.rs:1425-1428` |
| `legacy` | 任何 | 不新增，只是 0036 迁移之前历史行的默认值 | `0036_monitor_scheduling_clarity.sql:123` |

`lease_task.execution_state` 是 6 个值，不是"只有进行中和完成"：`0019` 最初写成 `IN ('pending','in_progress','completed')`，`0045` 换成 OR 等式形式并加入 `unavailable`（页面被确认不存在），`0047` 加入 `blocked`（有界重试后仍读不到页面），`0097` 加入 `input_blocked`（缺必需执行输入，从未打开过页面）。后三次都是"先 DROP 同名约束再 ADD"，所以约束名一直是 `collection_work_order_lease_task_execution_state_check`，值集在增长——只看第一次定义会得到过时的三值域。失败明细另记在 `collection_work_order_lease_task_dispatch_failure`（`0033`）。它也不是严格单向——派发失败但租约仍有效时，`in_progress` 会退回 `pending`（`dispatch.rs:1202`），前提是这条任务还没被包回执结束。

`detail_page_session.state` 是"授权→导航→交付"五步链，只有 `dispatch.rs` 一个写入点。

### 谁允许改

- 用户命令：停止采集任务（连带取消其名下排队中的工单）、撤销目标
- 后台调度：`dispatch.rs`（派发 / 回收 / 到期 / 止损）、`work_order_lease.rs`（发租约 / 释放）
- 插件回执：`acquisition_chain.rs`（领取、提交结果）

### 写入点

`queue_state` 有 7 个文件可写：`dispatch.rs`、`work_order_lease.rs`、`acquisition_chain.rs`、`collection_run.rs`、`topic_map_collection_search.rs`、`topic_map_research_collection.rs`、`apps/api/src/local_web/collection_control_surface_view.rs`（建单时直接插入状态）。另有 `collection_control.rs`、`keyword_archive_detail.rs`、`patrol_scheduler.rs` 三个文件会 `UPDATE collection_work_order` 的其它列，并把 `queue_state=...` 当条件用，但不写这一列。`detail_page_session.state` 只有 `dispatch.rs` 1 个。

### 已知缺口

`queue_state` 没有单一闸门。上面那张"合法转移"表是从 9 个文件各自的 `WHERE queue_state=...` 守卫里反推出来的——**没有任何一层能拒绝一张表里没写的转移**。新增一条转移路径时，只要写对 `WHERE` 就能生效，本文不会自动拦住。

## 3. AI 批次 linggan_ai_batch / item

**是什么**：一批 AI 调用任务（默认 50 条一轮），由执行器从队列里领取、调用供应商、回写结果。这是全库约束最严的一台机器。

### 状态集合

| 表 | 列 | 允许状态 |
| --- | --- | --- |
| linggan_ai_batch_control | state | `open`, `completed`, `cancelled`, `failed` |
| linggan_ai_batch_item_control | state | `pending`, `running`, `succeeded`, `failed`, `skipped`, `cancelled` |
| linggan_ai_executor_instance | state | `running`, `draining`, `drained`, `lost` |

### 合法转移（item）

| 从 | 到 | 条件 | 写入点 |
| --- | --- | --- | --- |
| `pending` | `running` | 执行器原子领取，租约 30 秒 | `executor/queue.rs:105` |
| `running` | `succeeded` | 供应商返回可用结果 | 各执行分支（见下方写入点） |
| `running` | `failed` | 供应商明确失败；或租约过期且**已经调用过**（记 `ai_dispatch_unknown`） | `executor/queue.rs:143` |
| `running` | `pending` | 租约过期但**从未调用过**供应商 | 同上 |
| `pending` | `cancelled` / `skipped` | 批次取消；且必须从未调用过（`attempt_count=0`） | `executor/queue.rs:73` |
| 任何 | 任何 | 数据库拒绝 11 类非法组合，例如"`running` 必须有租约"、"`succeeded` 必须有输出"、"`skipped` 必须 `attempt_count=0`" | `0135_ai_gen1_batch.sql:56-66` |

批次自身的 `state` 不是写出来的，是**触发器算出来的**：`round_no*50>=total` 就 `completed`（`0137_ai_gen1_round.sql:67`）。另有熔断：失败率超过 30% 记 `failure_ratio`，连续 2 轮无可用输出记 `no_usable_output`（`:45-58`），并拒绝在还有条目 `pending`/`running` 时关闭批次（`:85-94`）。

### 谁允许改

只有 AI 执行器（`ai-executor` 常驻进程）。用户侧能做的只有"取消批次"，最终也由执行器写。

### 写入点

`linggan_ai_batch_control` 1 个文件（`batch_store.rs`）；`linggan_ai_batch_item_control` 8 个文件（`batch_store.rs`、`executor/queue.rs`、`writing.rs`、`probe.rs`、`ocr_recognition.rs`、`receipt.rs`、`service/work_comparison.rs`、`service/research_call.rs`）。

### 已知缺口

- **`state='lost'` 名字误导**：执行器"还活着没有"的判定靠心跳（30 秒内）+ 数据库锁，不看这个列（`0139_ai_gen1_executor.sql:33-45`）。`lost` 只在"新执行器接管时"给上一任贴上，是历史标记，不能当"死活"读。
- **`failed` + `ai_dispatch_unknown` 不等于"确定失败"**：它表示"发过请求但不知道对方有没有执行"，重试可能重复调用、重复计费。这一条只写在代码注释里（`executor/queue.rs:143` 附近），没有进入任何约束。

## 4. 观察目标 collection_observation_target

**是什么**：一个要长期盯住的对象（作者 / 关键词 / 领域）。它有一套**明确设计过**的状态机：当前状态是一行摘要，每一步变化另存一条转移记录。

### 状态集合

| 表 | 列 | 允许状态 |
| --- | --- | --- |
| collection_observation_target | lifecycle_state | `pending_decision`, `archiving`, `archived`, `monitoring`, `paused`, `dismissed` |

### 合法转移

| 从 | 到 | 条件 | 写入点 |
| --- | --- | --- | --- |
| `pending_decision` | `archiving` | 用户决定建档 | `acquisition_chain.rs:2736` |
| `pending_decision` | `dismissed` | 用户忽略 | `collection_target.rs:1153` |
| `archiving` | `archived` | 建档完成 | `work_order_lease.rs:521` |
| `archiving` | `monitoring` | 建档转监控 | 同上 |
| `monitoring` | `paused` | 全域暂停 | `patrol_scheduler.rs:591`、`:666` |
| `paused` | `monitoring` | 恢复监控 | `patrol_scheduler.rs:579`、`:649` |
| `monitoring` | `dismissed` | 用户忽略 | `collection_target.rs:1153` |

### 谁允许改

- 用户命令：`transition_target`（`collection_target.rs:762`）、建档 / 忽略入口、监控规则命令（`collection_control.rs:1987`）
- 后台：巡查调度器自动暂停/恢复（`patrol_scheduler.rs`）、建档完成回写（`work_order_lease.rs`）

### 写入点

5 个文件：`collection_target.rs`、`acquisition_chain.rs`、`work_order_lease.rs`、`patrol_scheduler.rs`、`collection_control.rs`。

### 已知缺口

- **没有一层校验 `from → to`**：数据库只约束了"值合法"（严格说，转移表自己的 `from_state` / `to_state` 连值域约束都没有），没有约束"这一步转移合法"。上表是从代码里反推的，不是被强制的。
- **转移日志可追加、可整目标删除，但不能改写**：`0195_observation_transition_immutable.sql` 起，对该表的 `UPDATE` 会被数据库拒绝。`DELETE` 仍然允许，因为产品删除整条观察目标时会连带删除它的历史（`collection_target.rs:1147`，从叶子往根删）。`0005` 注释里"只追加"的措辞也因此不再准确：准确说法是"只能追加，或随目标整体删除"。

## 5. 研究底稿 linggan_material_unit / reading

**是什么**：一篇作品的可读材料（正文、图片、音频、视频帧）以及由它生成的标准化底稿。它同时存在**两套"这一列能不能用"的说法**，这是本文最需要记住的一条。

### 状态集合

| 表 | 列 | 允许状态 |
| --- | --- | --- |
| linggan_material_unit | source_state | `present`, `known_empty`, `not_acquired`, `not_applicable`, `restricted` |
| linggan_material_unit | adoption | `automatic`, `human_adopted`, `human_edited` |
| linggan_material_unit | verification | `unchecked`, `verified` |
| linggan_material_reading | publication | `automatic`, `human_edit` |
| linggan_material_reading_selection | event | `initialization`, `automatic`, `adopt`, `edit`, `keep`, `source_changed` |

代码里还有两套不在数据库里的值域：

| 代码 | 枚举 | 允许状态 |
| --- | --- | --- |
| crates/evidence/src/dossier_input.rs | ColumnState | `available`, `not_provided`, `not_acquired`, `waiting`, `known_empty`, `terminal_failure`, `restricted`, `not_adopted`, `not_connected`, `needs_relayer`, `invalid_source` |

第二套是底稿读取页的"整篇就绪"口径，**7 个值来自 `research_dossier/coverage.rs:47-63`，未纳入自动检查，需人工核对**：`not_applicable`、`restricted`、`known_empty`、`not_acquired`、`available`、`partial`、`not_processed`。

两套口径的关系：`ColumnState` 有 11 个值，但**只有 3 个**会挡住整篇就绪（`waiting` / `restricted` / `not_acquired`，`dossier_input/read.rs:70-97`）。其余 8 个（含 `terminal_failure`、`invalid_source`）不挡。这是 2026-10-25 设计审查后**有意为之**的决定：部分列不可用不应该抹掉已有的合格内容，`ready` 只等于"没有未就绪理由"（`dossier_input/identity.rs:12-14`）。

### 合法转移

| 对象 | 变化 | 条件 | 写入点 |
| --- | --- | --- | --- |
| material_unit | `source_state` 改变 | 来源版本变化（重新采集 / 来源失效） | `material_reading.rs:173`，与 `source_version` 同一条语句写入 |
| material_reading | 新增一条读法 | **只插入，不更新**：同一材料可以有多份读法，由"采用"选择哪一份 | `material_reading/write.rs:164`、`material_reading/review.rs:250` |
| reading_selection | 6 种事件 | 初始化 / 自动 / 采用 / 编辑 / 保留 / 来源变化 | `0146_research_dossier_v2.sql` |

### 谁允许改

- 自动流程：材料入库、AI 识别、自动读法生成
- 用户：采用（adopt）/ 编辑（edit）/ 保留（keep）三种人工动作

### 写入点

`crates/evidence/src/material_reading.rs`、`material_reading/write.rs`、`material_reading/review.rs`（unit 1 个文件，reading 2 个文件）。

### 已知缺口

两套口径并存：`ColumnState`（11 值，描述"这一列"）和 coverage（7 值，描述"这一类来源"），页面在不同区域各用一套。二者不冲突，但同一个"没有"在不同地方叫不同名字。

另一个实例：`publication` 在 0146 建表时有 3 个值（含 `candidate`），0168 迁移把它收窄成 2 个值。0168 的整体注释说明这是一次"一份现行读法 + 可选人工编辑"的模型调整，但没有单独记录"`candidate` 这个状态消失了"——如果只看页面或接口，不会知道它已经不在了。这正是本文第 6 节自动检查要盯住的那类漂移。

## 6. 自动检查做什么、不做什么

检查脚本：`node scripts/check-state-machines.mjs`（可用 `--list` 打印它认到的全部条目）。

**做**：

1. 本文每张"状态集合"表 ↔ 数据库 `CHECK` 的实际值集，逐项比对（多一个、少一个都报错）。两种写法都认：`col IN (...)` 与 `col = ANY (ARRAY[...])`，以及"状态机式"的 `(col = 'a' AND …) OR (col = 'b' AND …)`——后者是 `0045`/`0047`/`0097` 改 `execution_state` 时用的写法，只认 `IN` 会得到过时的三值域。
2. 后一次迁移改了同一列时，判据跟着迁移的写法走：`DROP`（含 `DROP CONSTRAINT %I` 循环这种动态写法）之后有可读新定义，就取新定义；只 ADD 了一个更宽的值集而没 DROP 旧约束，报"实际生效的是两者交集"；DROP 之后没有可读新定义，报"无法验证"。
3. "代码"表 ↔ Rust 枚举的实际变体，逐项比对。
4. 扫描生产 Rust 代码里"更新本文所列数据表"的 SQL，凡在 `SET` 或条件里用等号写出、比较了不属于本文该列状态集合的字面量，报错。按表归属，不会把别的表的同名列混进来；字符串按词法切分（跳过注释、字符字面量、裸字符串），不会因为某行里的一个引号而漏掉后面的语句。

**不做**（刻意不做，因为静态看不出来，硬做只会产生误报）：

- 不检查"合法转移"表：某个文件是否只在允许的时机改状态，需要运行期信息，静态扫描给不出可信结论。
- 不覆盖本文五节之外的其它状态机（媒体作业、评论研究、主题图谱、运维发布等）。
- 不检查 coverage 的 7 值口径（第 5 节已标注人工核对）。
- 不检查绑定参数（`$2` 之类）里的状态值——这类值最终仍会被数据库 `CHECK` 拒绝，风险由数据库兜底。
- 不检查 `INSERT … VALUES ('x')` 与 `CASE … THEN 'x'` 里的状态值：前者要按列位置对表，后者无法判断是不是在写状态列，硬做只会产生误报。
- 只用一个等式表达的值域（`CHECK (col = 'x')`）不当成完整值集：它可能是更宽条件的一支。这种列若被登记进本文，脚本会以"找不到 CHECK"报错，而不是猜。

**验证方法**：在任意一处写一个假状态，运行脚本必须变红。已实测四类漂移各自变红、还原后变绿：迁移新增值（`9999` 临时迁移加 `'stuck'`）、代码字面量写错（`queue_state='complet'`）、文档多写值（`'ghost'`）、枚举新增变体（`GhostState`）。`scripts/check-state-machines.test.mjs` 里另有 18 条针对解析边界的用例（单值收窄、`ANY(ARRAY)`、动态 DROP、无 DROP 的放宽、同一条语句里 DROP+ADD、带载荷的枚举变体、注释里的引号等）。

## 7. 三个容易读错的名字

这一节不是新的状态机，而是三处"名字像一件事、实际是另一件事"的地方。读代码、看接口返回、数统计时先对照这里，可以省掉一次误判。

### 7.1 AI 执行器的 `state = 'lost'` 不是"它挂了"

- 写入只有一处：新执行器接管时，把上一任从 `running`/`draining` 改成 `lost`（`crates/intelligence/src/ai_foundation/executor.rs:167`）。
- "谁算当前的主人"不看 `state`：`0139_ai_gen1_executor.sql` 的 `linggan_ai_executor_is_owner` 同时要求 `state='running'`、心跳在 30 秒内、持有会话锁。心跳停了的 `running` 行同样不是主人。
- 所以：`lost` 只说明"它被接管过"，是历史标记；反过来，看到 `running` 也不能当成活着。判断存活只看心跳和锁。

### 7.2 任务接口里的 `queue_state` 不是工单的 `queue_state`

- 读取模型写的是 `lease_task.execution_state AS queue_state`（`crates/evidence/src/collection_task_read.rs:275`），同一文件的 `:107` 与 `:119` 用这个别名算"进行中 / 租约过期"。
- 这个字段的值域是租约任务的六个值（`pending`、`in_progress`、`completed`、`unavailable`、`blocked`、`input_blocked`），**不是**工单的五个值（`legacy`、`queued`、`leased`、`completed`、`cancelled`）。
- 两个字段同名、值域不同，不能互相比较，也不能拿一处的取值去套另一处的转移规则。

### 7.3 三层各有一个 `completed`，只有最里层代表"这件活干完了"

| 层 | 字段 | 写 `completed` 的实际含义 |
| --- | --- | --- |
| 工单 | `collection_work_order.queue_state` | "这一轮派发结束了"，**不一定全成**。三个写入点都写它：全部任务完成时（`crates/evidence/src/work_order_lease.rs:459`）、租约记为 `partial` 时（`crates/evidence/src/dispatch.rs:1497`）、租约记为 `input_blocked` 但有任务完成过时（`dispatch.rs:1565`）。此外在"没有任何任务被接收"的收尾里，它还可以被从 `completed` 降级为 `cancelled`（`dispatch.rs:1425-1428`）。 |
| 租约 | `collection_work_order_lease.release_reason` | "所有任务都是完成态"，区别于 `partial`（有任务缺输入/不可用）、`input_blocked`、`expired`（`work_order_lease.rs:445-457`、`dispatch.rs:1489-1495`、`dispatch.rs:1555-1562`）。 |
| 租约任务 | `collection_work_order_lease_task.execution_state` | 这一件活真的做完了（`work_order_lease.rs:409`）。 |

三层同名不同义，统计"完成了多少"时选错一层，得到的数字就不同：按工单数会混入 `partial`/`input_blocked` 收尾的轮次，按租约数只算整轮全成，按租约任务数才是逐件的口径。写任何"完成率"之前先明确用的是哪一层。
