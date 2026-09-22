# 下一阶段：评论研究自动化衔接合同

> 状态：技术设计定稿，待用户批准开发；不是已实现或已验收声明
> 交付包：COMMENT-STUDY-PRODUCTIZATION-001 · 文档版 1.0
> 核对日期：2026-09-22
> 源码基线：`main@c74d72e3d17b9d5ecfb9953de025713c47e4560e`
> 责任：本包设计由 ChatGPT 整理；开发、共享库操作、真实模型调用与发布由 Mog 单独授权
> 事实与设计：标为“现状”的内容来自固定版本源码；“规定／新增／必须”为本次目标合同

## 1. 状态与边界

本文件是**下一阶段设计，不是当前实施项或开启授权**。本轮只证明计划发起者可以调用同一个内部开始函数。不得因为本文件已经给出 schedule 字段，就在本轮顺手建表、开启定时模型调用或读取旧自动计划进行恢复。

自动化不是定时打开页面，也不是让 Agent 每天判断要不要研究；它是受控范围＋固定方法＋时间槽＋额度，在后台生成普通 Run。后面的清洗、合批、提取、向量、归并、证据和四Tab全部沿用本轮路径。

## 2. 本轮必须已经成立的接点

`start_study_run` 不依赖 browser session 或“用户刚保存设置”；它接收明确 command 和可信 origin。request 回执可表示 no_work/index_pending；稳定身份排重和三阶段预算必须在数据库里真实生效。暂停／停止／deadline、缓存先行、来源失效传播已经完成。

新方法不自动覆盖旧批次，语境改变不自动重跑所有数据，缺向量不挡提取。每个阶段有确定错误和下一动作；日计划不通过制造新 Run 来绕过已耗尽的同上下文重试次数。

## 3. 下一阶段只新增一张计划表

拟定 `linggan_comment_study_schedule`，属于下一阶段增量 migration。

| 字段 | 类型／约束 | 定义 |
|---|---|---|
| `schedule_ref` | uuid PK | 本模块计划身份 |
| `domain_ref` | uuid NOT NULL UNIQUE FK observation_domain | 第一版每 domain 一个日计划；当前仍仅 ADHD |
| `policy_ref` | uuid NOT NULL FK study_policy | 显式固定完整方法，不动态追踪“最新” |
| `enabled` | boolean NOT NULL DEFAULT false | 只控制未来日时间槽发起，不取消已开始 Run |
| `timezone` | text NOT NULL CHECK='Asia/Shanghai' | 第一版只支持北京时间，不引入 cron／通用时区配置框架 |
| `local_run_time` | time NOT NULL | 用户明确保存的每天运行时间；本手册不替用户选时间 |
| `comment_budget` | integer NOT NULL 1–3000 | 每个自动 Run 的最大评论数 |
| `context_character_budget` | integer NOT NULL 1–20000 | 每作品冻结语境预算 |
| `daily_token_limit` | bigint NOT NULL 1024–10000000 | 该计划全部自动 Run 在授权日期的总额度 |
| `scope_manifest` | jsonb NOT NULL object | kind=domain_backlog 或 works＋明确 workRefs，最多100个显式作品 |
| `selection_mode` | text NOT NULL DEFAULT 'new_only' | 第一版只允许 new_only；后续恢复语境另行扩展合同，不隐式失败重跑 |
| `next_due_at` | timestamptz，enabled 时非NULL | 下一次名义到期时间，不代表进程已运行 |
| `last_slot_at` | timestamptz，NULL允许 | 最近已提交开始回执的日时间槽，包括 no_work |
| `last_work_ref` | uuid，NULL允许 | 全域 backlog 的跨作品轮转位置；不是数据成功水位 |
| `revision` | bigint NOT NULL DEFAULT 1，>0 | 设置 CAS；更新加1 |
| `created_at`,`updated_at` | timestamptz NOT NULL | 本地持久记录时间 |

INDEX `cs_schedule_due_idx(next_due_at,schedule_ref)` WHERE enabled=true。新方法成为全局默认不改变已有计划 policy_ref；用户在计划设置里明确选择新版本才更新。禁用保留 next_due_at/last_slot_at 记录，重新启用重新计算未来到期时间，不自动补做所有漏掉日子。

为 current start_request.origin_ref 增加到 schedule 的 FK；先验证 scheduled 历史全部能关联（本轮不产生真实 scheduled 行）。所有 JSON 字段闭集校验，不允许用户插入 SQL 或任意 Agent 指令。

## 4. 每日时间槽算法

每天最多自动发起一次普通 Run；材料超过3000或覆盖作品超过100，留在 backlog，不拆成数十个自动 Run 无声加费用。下一天继续轮转。需要更高频率时修改下一阶段计划合同，而不是 Worker 绕过。

到期扫描附在现有服务端 tick 中，只做有界数据库工作：

1. 查 enabled 且 next_due_at≤now 的计划；每 tick 最多5个。普通只读查询确定 domain，不提前拿反向行锁。
2. 开始 READ COMMITTED；先 domain advisory lock，再 schedule 行锁；重读 enabled/revision/next_due_at。
3. 若设备睡眠错过多天，将名义时间槽合并为**最近一个已到期日**，计算 missedSlotCount，仅执行一次，不逐日回放。
4. 按 `(origin_ref=schedule_ref,scheduled_for=slot)` 查持久开始回执。已有则复用，不生成第二次研究；配置改变也不重做已提交时间槽。
5. 无回执时，在同事务生成一个 UUID request_ref，固定 plan revision、policy、范围和额度，以 TrustedStudyOrigin::Scheduled 调用 `start_study_run_in_transaction`。随机 UUID 不承担防重，时间槽 UNIQUE 和锁承担防重。
6. created/no_work/index_pending 都写回执；同时更新 last_slot_at、轮转位置、next_due_at=该名义日的下一天。提交后模型 Worker 才看到新 Run。
7. 提交前崩溃全部回滚；提交后响应丢失，下次查时间槽回执。数据库失败不 advance next_due_at，下一tick重试；禁止先推进指针再创建Run。

新内部 scope `DomainBacklog{after_work_ref}` 由计划构建，不向浏览器开放。按作品UUID环形轮转最多100个当前有待处理材料的作品；按本轮方式每作品逐条取样。轮转位置只表示扫描公平性，**不能用它排除所有小于该值的新评论**；到末尾必须 wrap，原材料与历史 anti-join 才决定未研究集合。

## 5. 每日额度必须在外发时执行

只限制每天“创建了多少 Run”不够：昨天的 Run 今天仍可能产生归并调用。每日额度必须放在本轮的统一 model-request 预留函数里，所有三个阶段执行同样检查。

下一阶段在 request_manifest 增加可选审计字段 budgetWindow：scheduleRef、localDate、tokenLimit；并升级 snapshot contract 的读取兼容规则。它不是新任务或预算账本。manual 请求没有该字段。

预留在 domain lock 下进行：按当前北京时间日，查询本计划最早一笔已经记录的当日 budgetWindow；有则使用其中冻结的 tokenLimit，无则取 schedule.daily_token_limit 并由第一笔请求固定。当日更改额度设置只影响下一天，不让同一天不同请求偷偷采用不同上限。

计算当日已记账额：model_request JOIN start_request（通过唯一 run_ref、origin=scheduled、origin_ref=本计划）JOIN invocation，按 budgetWindow.localDate 归属 SUM(charged_tokens)。每 invocation 只计一次。先同时满足 Run.token_limit 和计划当日剩余额度，才预留下一请求。配对仍只计 ownerRun 一次。

明确未外发的请求不占消费；未知外发保留预留；迟到 usage 回到当时的授权日期，不计成今天的新消费。跨午夜而未领取外发的请求须在新日期重新预留、旧请求明确未外发结算；不能昨晚预留一批而今天绕过当日额度。已在午夜前领取外发的长请求归属昨晚授权窗口；这是系统授权口径，不保证上游账单日期相同。

日额度不足时暂停该计划的**后续外发**，保留本轮队列与已得结果；设置自动 Run 的 dispatch_state=paused、dispatch_reason=daily_budget_exhausted。下一日仅对可证明是“日额度暂停且Run额度仍够”的自动Run恢复。`daily_budget_exhausted` 是下一阶段才加入 CHECK 的专用原因，本轮不加入、不复用 budget_exhausted。Run总额度用尽仍 stopped，不自动加额。

以上读取先使用 start_request 的 origin 索引与 request 的 run 索引；实测需要时为 schedule/day JSON 字段加受控索引，不预先新增预算窗口服务或表。缓存命中不创建 invocation，也不虚报一笔模型节省费用。

**范围限制：**这里限制的是本计划的自动调用，手动研究仍有自己的 Run 限额并单列显示；不能把两个加起来宣称已有全站每日总消费硬上限。

## 6. 增量与失败规则

新增采集版本先确定稳定评论 head，清洗后台只处理缺失缓存。日计划默认 new_only，已成功／无信号／失败／缺语境／取消的旧 Target 不每日重复进队。输入补齐恢复可在以后显式启用 input_changed，但必须以真实 fingerprint 变化和明确依赖解除为依据，不能用“昨天失败”作为无限重试理由。

向量暂缺、retrieval_incomplete 的恢复继续使用原 resolution 身份与请求历史；前提改变后可以重新比较，不创建第二次语义提取。新出现 Problem 后老 deferred_novel 的再召回复用已有 pool_cursor，每tick有界，不全量all-pairs。模型升级只改变未来明确绑定版本的任务，不能在跑到一半时换方法。

没有需要处理的材料时没有模型调用；no_work/index_pending 依然有开始回执，可在研究批次页“自动发起记录”子区查看，不用空 Run 冒充工作。

## 7. 控制与中文表达

计划开关的明确文案是“每天自动开始研究”；关闭后新时间槽不发起，已开始 Run 按其授权继续。旁边提供“暂停当前自动批次”，使用本轮 Run 控制，不隐含停止已经发送的远端请求。

页面显示：下一次计划时间、固定研究方法、每日额度、当天已记账／费用未知、最近回执、backlog、最近运行结果。计划enabled、时间到期、Run创建、模型外发、结果接纳、用户问题新增分别显示，不能以绿色开关代表“自动研究成功”。

## 8. 开启前验证与阶段退出

下一阶段必须增加：双scheduler并发同slot、设备睡眠3日只一次补执行、关闭／修改计划竞争、UTC/北京时间日边界、跨日未外发预留、昨天Run今日调用、两个自动Run争最后额度、manual与scheduled区分、0工作不调用模型、始终失败不跨日无限重复。

同时复跑本轮T01–T54；自动化不能绕开已经正确的人工启动合同。先用合成provider验证所有副作用，再在 Mog 明确额度和数据授权下进行有限真实样本试运行，记录结构拒绝、错误提取、错误归并和召回遗漏；用已有独立标注或经授权建立的小型基准，不用模型自评冒充真实质量。

日常运行不加入人工逐条确认。一次性的质量校准和用户业务验收是对系统是否值得持续自动运行的判断，不是插在每个批次里的机械审批。没有语义质量证据时可以继续开发／验证自动化安全，但不能宣称研究质量成熟或无监督结论可靠。
