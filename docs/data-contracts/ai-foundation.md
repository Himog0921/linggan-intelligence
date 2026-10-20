# 第一代 AI 底座合同

> 状态: 代码事实优先
> 最后核对: 2026-10-20
> 适用范围: #454 / PR-1B，调用/控制/能力/skill/执行器；新解读业务由PR-1C接续
> 事实来源: Mog已确认设计和持续目标、DEC-0011、现有MODEL-PI-001及本包实现/隔离证明
> 冲突时以谁为准: Mog最新要求、来源/权限与版本化合同；未实现/未验证必须可见

## 集中设置的读取与写入边界

ai-settings.v1投影同时提供只读资源/协议元数据。connection.revision用于连接CAS，不以最后一个connection_version.revision替代；同一地址换密钥时selectionModelRef仅用于稳定模型选择，modelRef/probe始终表示当前密钥版本，后者可以为空，不能继承旧调用资格。写作试跑选择端点只扫描最近50份仍指向当前作品版本的预览，并按冻结配置hash与当前来源资格过滤；只返回引用/标题/动作/时间，不复制正文或产生调用。

React四区的保存、开关、外发、测试、导入、试跑和启用各自接独立后端回执；没有确认的网络/解析结果保持未知并沿用同一命令身份，明确CAS拒绝保留草稿。保存后刷新某一能力版本不得销毁其他表单的未保存草稿。初始关闭不是自动刹车，未启动不是正常，未知/零/大整数不能相互替代。UI和隔离证明现状见唯一PR-1B计划，当前不代表共享运行/真实供应商可用。

## 旧入口与历史保真

旧model-settings根读、内联probe/default config和评论/创作者/旧主题的旧启用/启动/恢复链返回410；旧执行器入口在租约、凭据和子进程之前永久拒绝，不由旧开关恢复。连接/模型元数据save仍服务统一设置。业务页只读历史和状态，并通过统一task CAS暂停；人工点赞标准与改帖不授予AI许可。

0141要求0140写作桥接已存在，只删除workspace.default_config_ref/worker_last_seen_at/worker_state/worker_last_error，不删除调用账或旧业务结果。ai-invocation-history.v1每页50条、稳定(created_at,invocation_ref)游标及任务筛选，精确字符串/NULL保真，不返回prompt/result；额度占用包含已实测消耗和未知保守预留，不能当作仅未结算数。

工作循环与持有独立会话的心跳/排空监督并发轮询，不能因等待实例行写锁而暂停释放该锁的队列事务。成功收条仍要求数据库先draining再drained，之后原子文件；强杀没有排空收条。结算COMMIT回执不明只回读同一invocation/item/token，不重发模型；已尝试而失去进程的调用按unknown_reserved保守关闭并保留占用。

## 身份和默认边界

稳定模型为connection_ref × model_id，连接版本与密钥版本不冒充新模型。地址/协议/本地模式变化生成新endpoint epoch；A→B→A仍需重新批准，单换密钥保持同一epoch。外发批准绑定endpoint epoch × task × material class；每次dispatch重核，frontmatter不授予工具或外发权限。

任务注册包含note_interpretation、direction_list_draft、direction_proposal、author_org_identity、writing_assistant、writing_legacy、model_probe；comment_study/comment_embedding冻结，legacy_creator_discovery/legacy_topic_map退役。尚未接入任务不可因有注册行就显示可运行。全局/执行器/任务三层默认off，全部开启且配置/来源/能力/外发/预算合格才放行。开关命令CAS且幂等；系统只能关，人明确恢复。

## 用量、预算与刹车

input/output是供应商实测总量，缓存输入/缓存写入包含在input中，思考包含在output中，不能再加一次。缺失分项保持NULL；实测合计、预留占用、未派发分别表达。可能已派发且用量未知继续占用预留，不能写成实际消耗或0；明确未派发才释放。价格未知与0价格不同，不能拿SDK默认0当真实价格；成本计算使用精确整数/数据库numeric。

预算日只由统一数据库函数定义，时区Asia/Shanghai。预算模式区分未配置/当前不设上限/有上限；note_interpretation按Mog决定当前不设每日上限，其他缺批准不猜配额度。实际配置和当前开关不授权对真实库回填或调用。

每50篇完成一轮，失败严格超过30%自动暂停；连续两轮尝试过且无可用产出也暂停。0篇尝试不递增/不清空该连续计数；写作这种交互任务不自动刹车。批次冻结配置/skill/模型/栏目，当前权限撤回仍能阻止旧批次发出请求。

历史回填只补可证明的分类，不能猜finished_at、实测用量或失败语义；来源不足legacy_unclassified显式保留。迁移遇到违反终态/完成时间不变量的历史行应拒绝并给预检依据，不用now伪造历史完成。

## 能力、上限和执行

绝对资源上限共用版本化合同`crates/contracts/src/ai_limits.json`，不是供应商能力。实际input+output必须在所选能力context内，output/timeout/思考等级也受能力约束。pi目录只是有版本的预填来源，不因模型名字/官方域名自动认定兼容；允许手填和合成探测证明，未知保持未知。新任务不沿用旧默认配置绕过能力/外发准入。

独立执行器持会话级单例锁与service_instance心跳，4–8并发；任务领取SKIP LOCKED且有lease/代次围栏。排空只停止新领取，等待在途终态/用量/回执，DB与ack文件一致；installer等待从冻结最大超时和收尾裕量推导，不能硬编码90秒或手写成功收条。新服务初始off；巡查不再启动模型循环，非AI维护归属单独写清。冻结评论研究不被新执行器恢复。

## Skill 和页面

导入SKILL.md及引用文件，第一代拒绝scripts/、路径穿越、符号链接和失控大小；目录名/名称、description长度与锁定pi加载器一致。固定路径排序、NFC与LF，保留版本来源哈希。frontmatter只存档，任务工具权限由受信配置给出。批量调用用pi的formatSkillInvocation内联固定版本；写作AgentHarness在PR-3。

导入协议只接受具名字节载荷（UTF-8或base64），不接受符号链接/服务端文件路径，也不按上传路径打开本机文件。文本按UTF-8路径顺序、NFC/LF规范化，单文件256KiB、总计1MiB、最多64文件；分别记录规范化包哈希与原始来源哈希/清单。非文本附件保留字节/哈希，查看时显示类型，不伪装文字。固定Node入口仅在内存中调用pi-agent-core@0.85.1的loadSkills；元数据警告也拒绝，加载/导入无工具或供应商调用。formatSkillInvocation包含固定正文与排序后的文字参考文件，二进制附件不内联。

版本/文件/导入事实不可变；文件只准在创建该版本的同一事务插入，延迟约束在提交前检查SKILL.md、文件数与总字节，防止空版本或事后加文件。新命令携带预期最新版本，CAS冲突不覆盖；相同规范化包复用版本，另留来源导入回执。命令重放只承认历史回执，读页面须重取当前状态，不能把旧的“批准成功”回执当成撤回后的当前许可。

版本试跑、比较与启用分别留事实；非首版需完成关联候选的试跑，v1首次启用仅允许显式理由豁免。内容在本地改后重新导入，没有在线编辑器。

Mog已选择A确认[具体设置页G1规格](../plans/active/ai-gen1-pr1b-foundation.md#page-ai-settings-001--g1-已确认规格)，沿唯一React应用落地；仅本页实现获准，其他页面G1仍到对应阶段。旧界面/worker_*、default_config_ref在writing_legacy已接任务配置并验证后同PR移除；不能留下两套当前配置。旧数据与调用账保留，旧消费者冻结合同继续可读。所有验证分层，当前未有本包运行或真实供应商证明。

## 2026-10-20 · 配置与试跑存储边界

任务配置不可变、当前指针CAS；候选做法不同于当前绑定时只保存候选。批次固定输入引用/顺序/哈希及配置，最多10000项，试跑最多50项；旧/新做法比较必须同一任务、相同输入。模型探测不能绑定做法或用于试跑资格。完整试跑资格要求候选与基线均完成且有匹配配置的尝试/回执，首次v1理由豁免与已批准版本回退分别记录。受理回执不是运行完成。

轮次按50项、已尝试项和可用产出计算；严格大于30%失败或跨批连续两轮无产出在同事务系统暂停任务。零尝试保留连续计数，写作/连接探测不自动刹车，系统无法恢复。当前隔离存储测试构造了部分尝试记录；实际执行链证明仍待补，不能据此宣称供应商或共享运行通过。

## 类型化调用账与历史升级

0138为调用账增加任务/冻结配置/批次/轮次、quota_bucket、budget_day及预算归属budget_domain_ref/budget_run_ref；预算归属列与业务表的run_ref/domain_ref分名，避免冻结消费者现有JOIN USING被新列改变。四个当前写入者显式写任务与预算归属；旧评论/主题/创作者查询按类型列和统一预算日读账，写作仍遵守其单次批准预算，并返回类型化用量。仅预算读取改变，不恢复冻结或退役业务。

迁移仅按实际存在的消费者关系/已知旧producer标记分类，冲突或未知保留legacy_unclassified。原始result、创建/完成时间保留，旧charged_tokens另存legacy_charged_tokens；当前charged_tokens统一为额度占用：完整实测为input+output，未派发为0，其余为预留与可证明部分用量的较高值。NULL缓存/思考/价格不是0；缓存包含在input，思考包含在output。成本是固定价格回执计算的精确pico-USD报价，缺价格/必要分项保持NULL，不冒充供应商实际扣费。

终态与完成时间不一致时迁移拒绝，并提示按invocation_ref/state/created_at/finished_at预检；不得用当前时间伪造历史。终态不可再写/删除，运行中身份、预留和已记录用量不能改写；消费者预派发释放并入首次结算，旧失败恢复只处置其业务指针，不改旧失败调用。合成夹具不能通过重开失败调用或改预留模拟新尝试，须创建新回执。

失败码有词表外键，当前新执行器码有明确语义；无法证明的历史/既有消费者码保留原值，retryable/counts_as_failure为NULL，并标明未核实，不能凭名称转为可重试或已计刹车。历史“57种”仍是旧审计快照，未读取共享库刷新；真实迁移预检/分类与权限窗口仍待，当前编码不以该未知扩展真实库访问。

## 能力驱动的固定适配合同（实施中）

新派发携带冻结capability_ref、能力上限、单次input预算、所选思考等级、pi兼容参数/等级映射、结构化格式/strict及业务响应schema；模型名、域名、材料中的命令不决定能力。单次input+output受模型context限制，Node/Rust都核对全局资源合同；业务输出仍由Rust校验，供应商格式不替代它。旧冻结消费者仅保留原有有界输入合同；缺能力记录不能因使用官方地址/已知模型名自动取得结构化或思考能力。

资源合同同时给出请求/响应/最终文字/流量字节上限，独立于token估算。原有小型冻结调用边界以明确legacy字段保留，不能成为新任务的能力默认值。新调用不再被旧60s/8192/256KiB边界截断；未知用量保持预留。pi SDK会产生内部零价格和默认用量，本系统只采纳原始协议实测，金额由冻结价格回执报价。思考内容只允许在受控SDK内部处理，业务结果只返回最终text；工具调用仍不获授权。

离线目录来自固定SDK元数据文件，来源指纹覆盖SDK/协议合同/记录；查询与预填不代表当前账号可用列表、实时价格、等待上限或原生格式证明。pi_catalog来源写入必须逐值匹配，修改后需改用人工/合成探测来源。目录零价先记NULL，明确人工0价可保留0；价格用十进制字符串传输以免JS丢失精度。实际安装的两个pi包版本都核验为0.85.1，helper子进程清空环境且只允许固定脚本。

B6实施接缝：会话单例锁由独占数据库连接持有，数据库instance代次围栏与item租约共同约束领取、续约、派发和发布。会话丢失/超时不自动重发可能已外发的调用；派发前已接纳不等于已发送，只有可证明未派发才释放预留。暂停保留待领项，取消阻止新领取，在途仍结算用量但禁止被取消输出发布。writing试跑使用不可变preview与逐项输出，不复用一个正常写作请求做基线/候选。逐次材料和外发资格在实际调用前重新核对，完成回执不能绕过当前材料读取资格。此段是待实现合同，不是执行器通过证明。

## 执行器探测与运行读数（0139增量）

独占连接持会话锁，当前instance/generation和item租约代次/令牌共同围栏。instance心跳30秒过期；活会话的明确排空可续约和收尾，排空态不领新项。只有零尝试失联项可重新待领，可能已外发的尝试保守终结且保持预留；晚到最小用量另有不可变证据，不复活item、不写业务输出、不自动调账。instance与item的数据库排空不是文件ack/launchd/installer握手的完成证明。

探测输入可固定capability_ref及单次limits profile；该profile与材料ref同为不可变批次输入。probe来源能力只可复制该次冻结的声明、兼容、等级和格式，原生格式还需合成响应匹配；当前key版本必须匹配。该记录表示某个选定profile通过，不证明最大容量或所有思考等级实测通过；原始容量来源与当次testedProfile分开展示。缺profile的合成探测只证明本次可调用，不替它添加原生格式或能力。

固定Node诊断显式给出requestSent；旧回执缺值为未知。false必须与无HTTP/响应/实测等证据一致，才能将调用记未派发、释放预留并排除已尝试轮次；Rust child异常或超时缺该证据继续未知预留。日预算互斥核对后固定同一数据库admitted_at作为created_at/预算日，price_ref和item轮号同时固定。

设置execution读数提供service.observedState（未启动/运行/排空/已排空/丢失/过期）、最近7日dailyUsage、最近100条脱敏失败及晚到证据汇总。用量/成本/大计数使用十进制字符串；无实测合计保持NULL，未知占用另列，knownQuotePicoUsd不是含未知项的实际账单。lateUsage.quotaReconciled=false明确尚未调账，不能再加进已结算用量。该读口已接四区React、写作处理与独立进程源码/隔离证明；真实常驻安装及供应商/业务验收仍未执行。

## 写作桥接（0140候选，未部署）

writing_legacy从统一当前配置准备预览，冻结task_config_ref、原命令与材料hash。confirmed请求原子写command/batch/item/request为queued，不创建调用账；只有执行器在当前准入后才预留和派发。普通request关联唯一item与invocation；trial只用同一个WritingPreview，基线/候选独立输出，不反复写同一普通请求。来源、取消、实例/item围栏在返回时重核；正式建议只在创作表保存，普通item保存引用。读取历史建议和对照也重核来源资格，未接入的业务不从设置输出正文。历史试跑事实与当前输出可读性分开。

预留使用冻结input额度+output额度；estimatedInputTokens仅为共享输入规划估算，不当作精确消耗或实测上下文上界。实测/部分/未知用量仍按0138合同，已知超预留照实计量，未知不得释放。单次写作仍maxCalls=1，不自动重试收费。queued的invocationRef/chargedTokens为NULL，usage.state=not_reserved，与已建账的not_dispatched/measured/unknown_reserved区分。客户端闭集字段同步；未预留显示尚未建立账，不显示0消耗。

0140拒绝旧running/cancel_requested写作请求，需部署前核对并由旧执行者或获准的恢复窗口明确收尾；迁移不推断是否外发，不改历史result/created_at/finished_at。历史预览task_config_ref可为NULL并继续读取，不能作为新执行配置回退。调用前取消无账，在途取消照实结算且不发布，失效恢复同步终结公开请求，晚到只留最小用量证据。

当前7项真实隔离写作/迁移证明、5项Axum HTTP（其中写作HTTP的callable前置是显式合成行）与旧4项本机fake合同通过。真正SDK写作证明先经过owned loopback probe，没有用数据库假前置替代该步骤。独立第四进程/installer双收条与正式设置、评论历史浏览器已另有隔离证明，详见PR-1B最新计划；真实常驻安装、真实供应商和部署未执行。

## 批次控制与读取（PR-1B候选）

全局/执行器/任务暂停仍是三层CAS开关，只影响后续派发。单批取消使用独立commandRef、expectedRevision和理由，事务内记录命令并取消未派发项；在途只结算原调用、保留用量、禁发产出。取消不可恢复，恢复开关不能复活取消批次，重做需新批次。控制冲突返回409并保留调用方草稿；相同命令重试返回原回执，变更同ID载荷拒绝。近期批次读数及按位置读取的条目只提供状态、waitReason、版本、invocation引用与轮次，不暴露模型输出正文或代替材料读取资格。终结、取消和恢复统一先锁批次再锁条目，避免并发回执在轮次时升级共享锁。

## 独立执行进程与回执（PR-1B候选）

`executor/service`驱动真实4–8并发，实例心跳不等待队列或child；可运行的处理器仍只有model_probe与writing_legacy。进程启动要求0131–0141的实际迁移账SHA匹配，保存设置或启动进程不隐式开启控制。每条终结、取消与恢复按批次→item锁顺序；会话/条目代次、有效租约和当前材料资格在写入时重核。

Provider调用与租约续期并行；续租失败不重发已经开始的有界调用，晚到回执进入仅用量账。原结算失败只重试同一冻结invocation的事务；terminal result的executorReceipt与item lease token/代次同时提交，支持提交结果不明确后的原件回读。owned PG协议代理已在实际COMMIT成功后丢失回复，证明冻结receipt原件回读、单次外发与单次结算，不靠再次调用供应商重试。持续失败在15秒收尾窗口内优先尝试保留最小实际用量；若整个DB仍不可达，不能保证凭空取得回执，也不能报成功或释放预留。

排空DB成功与文件成功必须同时成立。成功收条绑定instanceRef、generation字符串、PID和源码身份；失败无成功receipt。当前已有独立binary、installer/sync消费、第四label与保留禁用候选；替身命令和owned PG/真实进程绑定已证明，不代表Mac launchd或本机常驻服务已更新。

## 安装许可与本地维护归属（PR-1B候选）

更新许可为linggan-runtime-update.v1的闭集JSON，绑定from/target完整SHA、patrol收条结果、AI原件及cluster/database/schema身份和disabled快照。原子写入；sync持锁后对同一数据库重新核对实例/代次/PID/源码、无running ledger/item、无对应会话锁，且旧binary/3000均已退出，才允许替换代码。许可不宣称外部供应商已经停止未知的历史远端计算，也不把未知费用清零。

新AI label初始disabled；既有disabled不因重装而复活。发布identity写full SHA，拒绝旧flat permit和未提交安装入口。开发态local_drained只证明停止自己的子进程，不可授权发布替换。巡检只做采集调度和确定性评论目录维护；embedding/模型不得从该process发出。旧模型循环的进程证明随owner退役；新独立AI binary已由owned PG/loopback证明SIGKILL后四条未知调用不重发、仅未尝试条目继续。Mac安装、共享运行和Mog验收仍未完成。
