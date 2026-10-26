# 当前状态与事项队列

> 状态: 权威当前
> 最后核对: 2026-10-26（STANDARD-DOSSIER-001 发布；其余各节保留原核对日）
> 适用范围: 当前阶段、事项顺序、阻塞与下一步
> 事实来源: 用户要求、2026-10-20 AI-GEN1候选源码/owned隔离证明与GitHub只读核对；其他模块及本机运行事实按各节原核对日保留
> 冲突时以谁为准: 真实运行结果、ACCEPTED ADR 与用户最新确认

## 2026-10-26 · 统一研究底稿合并与本机发布

Mog 明确授权“远端新增 PR 一起处理，然后合并部署上线”。[#496](https://github.com/Himog0921/linggan-intelligence/pull/496) 已合并为 `cbfff0cc92a37558caf741f878d95281890580b9`，包含已合并的 #497/#498/#499；实际 merge tree 与独立审查预测完全一致。底稿候选 `67d8ba7d` 的八任务 Delivery、#499 精确 head 的所选 Delivery、合并树 18 项 runtime 回归及预检通过；最终增量独立审查无 P1/P2 阻断。固定 100 篇隔离回归、实际截图和问题边界见[实施审计](audits/standard-dossier-001-2026-10-25.md)。

2026-10-26 14:49（Asia/Shanghai）通过官方 installer 实测，canonical runtime、前端与后端为 `cbfff0cc`，Intelligence 迁至 `127.0.0.1:3001`；既有 PostgreSQL READY，账本 167 条/0166，无 migration。巡检、媒体、AI 执行器保持禁用且未加载，旧心跳 stale 不等于执行器运行。正式同一 100 篇 JSON/Markdown 全部读取成功，仍为生产原有 58 块，未将隔离准备后 2073 块冒称生产回填；6 类页面桌面/手机 12 场景无溢出、控制台无警告/错误，核对入口真实可达。

现有本机 Chrome 插件原 ID 保留，0.8.64 原位更新并实际重载，配置为 localhost:3001；服务端已收到该版本工位报到。旧加载目录名保留以维持无 manifest key 的扩展身份，Chrome 缓存未迁移或清空；未发起采集。旧 Chrome-1 的历史安装未升级，不称所有浏览器已更新。发布前后模型、材料读本、识别任务、采集包/轮次及账本计数无变化。

本段是上述点时实测；后续文档提交、部署精确版本与最终收尾以 [PR #496 持续发布回执](https://github.com/Himog0921/linggan-intelligence/pull/496) 为准。私有原证据已完整移至主仓库 `artifacts/private/standard-dossier-001/`，不随工作树清理删除。此次没有生产历史材料回填、新模型外发或公网配置变更；Mog 业务验收独立。

## 2026-10-25 · 检查器与单次评论深采已合入主线

PR [#485](https://github.com/Himog0921/linggan-intelligence/pull/485) 与 [#486](https://github.com/Himog0921/linggan-intelligence/pull/486) 已按序合并；图组去重、评论直接展示、采集记录重排、指定条数及 API 15 秒推进单次采集进入源码主线。两项未知请求身份 P1 与推进窗口饥饿 P2 已修复，最终 head 独立审阅和所选完整 Delivery 通过；合并 SHA、失败修正及隔离证明见[当月记录](progress/2026-10.md)。

Mog 已明确授权加密备份、整库恢复验证、0150 迁移和本机发布；备份/隔离恢复完成。实际迁移、最终运行版本、PID/目录、3000 页面及副作用以 [PR #486 发布回执](https://github.com/Himog0921/linggan-intelligence/pull/486) 为持续更新入口。源码合并和合成证明不等于真实插件采集或 Mog 业务验收，不恢复巡查、媒体或 AI worker。下文保留此前各节的点时记录。

OCR 的动态交付事实以统一 Issue #454 及关联 PR/CI 为准。原本地规则与开发回归见[既有 OCR 计划](plans/active/ocr-title-quality-454.md)；当前获准的本篇手动云 OCR、动效百分比、检查器结果与模型选择见[手动识别计划](plans/active/ocr-manual-recognition-454.md)。两者保留来源和采用边界，候选实现、隔离证明不代表主线/部署或 Mog 生产业务验收；本包不新增真实模型调用、不操作共享数据库或现有服务。

## 2026-10-23 · 研究底稿 v2 集成与 CI 接缝本地复验完成

Mog 已确认完整底稿标准并授权实现；唯一交付为 [PR #481](https://github.com/Himog0921/linggan-intelligence/pull/481)。已集成 main `b471d9f1`（#477/#479）：新增人工采用行为接入唯一 v2 命令/面板，旧 overlay/API 退役，保留主线 0145、本包迁移顺延为 0146–0148。集成提交 `4955f4b0` 的必要本地 PostgreSQL、HTTP、实际浏览器与独立审阅通过；实际 CI 完整 Web 1180/1180、主题/底稿 UI、OCR adoption 和 preflight 通过，但最终 gate 因 OCR 筛选名、AI/浏览器旧来源假设失败。

同包修正保留冻结引用语义：新观察后仍读原完整材料，实际当前 restriction 才阻断调用与隐藏产出。Writing 完整 9/9、模块整理后的工作台受影响场景 1/1、三阶段真实浏览器、补回的 OCR scope 与 selector 回归已通过；本轮九个源码路径独立审阅无剩余实质阻断。最后预检、提交与远端 CI 以 PR 的实际新 head 为准。旧失败和复验分开记录，不以本地结果代替 CI。尚未合并、共享迁移、部署或进行真实模型/G4 验收。详见 [实施包](plans/active/research-dossier-v2.md)。

## AI 设置左栏导航候选（2026-10-21）

`/settings/models?view=capabilities|models|runs|advanced` 的四个正常 AI 设置视图已在候选工作树迁入共享左栏，主区不再显示重复 Hero、说明或刷新按钮。聚焦测试、类型/lint/LIDS/生产构建与隔离浏览器的八个视图场景通过；390px 使用共享横向轨道的四个直接入口。完整 settings 实链的做法导入 `400` 并等待回执超时，可由未修改 `5997b189` 基线复现，未在本包掩盖。候选未提交、未合并、未部署或刷新本机 `:3000`；无迁移、共享库写入、真实模型/供应商或研究调用。详见 [变更清单](design/changes/ai-settings-left-nav-001-ui-change-manifest.md) 和 [验收记录](design/acceptance/ai-settings-left-nav-001-acceptance.md)。

## 2026-10-20 08:38 · Web CI 取消 BEGIN 的连接复用缺口

be83ecdd 的 AI 底座 CI 39344829080 全部成功，包含原 COMMIT 丢回复用例；Web 39344829039 的设置/材料/评论/创作导出已通过，最后采集合成浏览器失败于 PG 设置事务隔离级别，原日志/产物保留，不计整链通过。本机原采集 14PG+5 路由+正式浏览器/管理合同通过，普通定时取消未复现。随后 owned PG 线代理精确延后 BEGIN 成功回复并取消 future，再进行池读取，确定性复现同样错误：SQLx 0.9 的客户端事务深度未登记，但后端 BEGIN 已生效，连接被复用。协议实验不冒称记录了原 CI 全部帧。

当前补齐唯一共享连接池的归还清理：同一简单查询 BEGIN; ROLLBACK 保证后端回到事务外；正常归还不产生“没有事务”警告，遗留失败事务导致清理失败则不回池。只回滚未提交事务，不提交或重试业务命令，保留会话时区/search_path/预备语句/会话锁；增加 owned 故障回归并纳入既有 AI CI。已加入9行池归还清理与精确BEGIN取消测试，新增测试验证同一backend PID复用、上海时区/自定义schema/预备语句保留、未提交不泄漏/已提交保留、失败事务连接关闭。移除清理时同用例确定性报原隔离级别错误，修后通过；清理/恢复源码字节已核实。完整基础49项（含新增取消边界1项）、6探测+9服务+7实际binary共22项、采集14PG+5路由+正式浏览器/11目标/6管理合同18个隔离命令均通过，所有owned资源清理核实；治理/设计/定向格式/bash/diff通过。原reviewer仅对本次池/测试/runner/CI增量复核PASS，无未解决P1/P2；会话锁保留来自源码核对，不冒称新增用例直接覆盖。新提交CI仍待，不能以本机成功替代GitHub。原一轮独立审查保留，后补共享接缝已单列增量复核；不重审其余 B。无共享 DB/服务、真实模型/媒体/采集或合并部署动作。

## 2026-10-20 07:54 · COMMIT 丢回复用例的 CI 失败与代理修正

9a923386 的 Web 检查已通过；AI 底座在实际 service 组 8 通过/1 失败，丢 COMMIT 回复用例在最终 service 结果返回 FenceLost，后续安装/历史只读未执行，不计通过。原日志为 private/settings-ci-foundation-9a923386-failed.log。本机同组及定向三次均通过，尚未直接复现 CI 的持有权失败，不能归因已证或重跑碰绿。

静态检查发现测试 PG 线代理每帧三次 TCP 写入、未设置 TCP_NODELAY。独立自有 Linux 回环实验用同样分帧方式，60 帧默认耗时 2.591 秒，启用 NODELAY 为 0.0049 秒，单次整帧写为 0.0038 秒；无数据库/宿主 socket/真实材料，容器已回收。该实验证明代理会额外引入平台相关延迟，但不单独证明 CI FenceLost 根因。

修正仅限测试代理两端 TCP_NODELAY，保留在实际 COMMIT 成功回复到达后仅丢一次回复的故障。原 service 用例补充到模型请求/释放/完成的计时与失败时的最小实例/租约/账状态，不记录 SQL、报文、凭据或原文。生产 Rust、SQL、租约/超时、一次模型调用/一个 attempt/无迟到账/一致 drained 的断言均不变。定向三次均通过（Mac约1.45–1.56秒，与原代理约1.43–1.48秒均正常）；完整6探测+9服务+7实际binary共22项通过，实际COMMIT丢回复/一次调用/一个attempt/无迟到账与drained断言保持，自有资源清理核实；治理/设计/定向Rust格式/diff通过。新head CI仍待，同范围测试修正不重开整轮审查。

本轮另补实际故障UI五步：owned过期实例行经真实API显示stale、实际CAS并发返回409且草稿保留、无效SKILL元数据返回400且未新增版本、实际撤回外发显示已撤回，以及显式浏览器读取断网后提示并恢复。五图原生200%均核对720 CSS视口/main不溢出，未发现新产品缺陷；调用/采集工单前后不增，自有DB/卷/profile清理核实。证据为settings-fault-ui/{report.json,*.png}和settings-fault-ui-server.log。不是共享运行、真实供应商或Mog签收，也不声称遍历所有故障组合。

## 2026-10-20 07:40 · AI 设置长字段修复与原生缩放证明

G1 A 按 Mog 已确认规格执行。补证据时发现长服务地址/模型标识撑宽模型区，原断言只检查根元素而漏检；新增真实浏览器断言在旧 CSS 下复现 main 5234 > 1280。现仅对 AI 设置区的 grid 轨道、长文本和选择链接施加宽度/换行约束，不改变壳层、Token、状态、权限或操作。

修后完整 Web 86 文件 988 项及类型/lint/build 通过。设置真实 React→Axum→一次性 PG 的 7 步/16 图与登录 36 图通过，覆盖 92 字符连接名、632 字符地址、190 字符模型 ID 和 150 段 SKILL；保存/导入/首版启用后调用与批次仍为 0。独立私有浏览器的原生 tabs.setZoom=2 在每个分区实测为 720 CSS 像素、dpr=2，CSS zoom=1、pinch scale=1；四区及长 ID/长地址/SKILL 末段共 7 图已看图核对，mainClient=mainScroll=720。仅离线技能/目录解析子进程可启动，模型进程拒绝；该环境调用/采集工单计数前后相同，不把合成种子已有调用记为 0。两套环境均已核实回收自有资源。

证据固定在 private/ai-gen1-pr1b/settings-long-{red,green}.log、settings-long-web-verify.log 和 settings-native-zoom/；原失败、禁用加载器和导航重置缩放的无效记录保留，不计通过。完整故障状态的逐浏览器人工复核仍未完成，不能由本次成功态长文替代。共享预检/备份迁移、Mac 实装/完整 serve、真实供应商及 #420/G4/Mog 验收继续未证明。

本次为同范围收尾修正，不重开整轮独立审查。此前 26d248a1 的四项 CI SUCCESS 保留为该旧提交事实；修复提交须以新 CI 核实，当前尚未完成。#455 仍依赖在先；无合并部署、共享 DB/服务、真实模型/媒体/采集、实施代理或新任务。PR-1C 仍按原 WP2/WP3 上线及 G4 前置推进。

## 2026-10-20 · PR-1B Draft #456 已建立

[PR #456](https://github.com/Himog0921/linggan-intelligence/pull/456)已推送为Draft，实现提交`c3d040385625c36c9ef39cf28f7c57b1a6dcfdd0`，增量基线#455@38379f26，目标main612dcbea，显式依赖#455。root已逐文件核对244路径与已审/已测工作区一致，工作树在实现提交后干净；本补记仅同步交付状态，不改变已测代码或重开审查。远端CI以最终head实际结果为准，当前不宣称通过或具备无条件合并条件。

一次独立审查PASS、1条P2已关闭；完整本地证据和未证明边界见下一节及[PR-1B实施包](plans/active/ai-gen1-pr1b-foundation.md)。没有合并部署、共享DB/服务、真实模型/媒体/采集或实施代理。后续先跟进#456 CI、处理同范围失败并核最终head；PR-1C仍等原WP2/WP3上线及G4，不把Draft当该前置已完成。

## 2026-10-19 · WEB-REACT-001 完整收尾包（当前）

#446 已按 c0abdb9e 合并为 main@d635718e，原 head 的3项实际执行CI成功、4项条件跳过，独立GPT复核覆盖该精确提交。本机 runtime/3000仍为362dad3f（16个React路由模式，登录旧实现），尚未部署本包。最终合并核对发现新增的保留副本标签/双页缩放P2，但门控shell缺少失败即停止，断言失败后仍执行了merge；此为root执行错误，已停止发布，原实施树按规定移除、私有证据移交，改在同一#430收尾包的codex/web-react-001-recovery专属树修复，不改共享脏目录。

当前修正同时覆盖历史恢复、采用改稿、冲突保留三种副本生成路径，保留未保存标签；恢复副本同步标签草稿/ref，阻止未确认替换命令期间修改副本；阅读书页缩放后落到最后双页的起点。有效旧代码反例4失败/63通过，修后67项定向通过；完整前端85文件983项、strict类型/lint/build通过。新增正式浏览器标签保存旅程23步通过并清理，同一获准GPT复核待完成；后续PR/新CI通过后才部署。完整证据和业务验收缺口见[收尾矩阵](design/acceptance/web-react-001-closeout.md)。

## 2026-10-18 · WEB-REACT-001 D3 评论研究四视图候选验收

当前 main／正式 3000 同为 `acdfbb87`；D2 #443、E #439（后续 #444）、F #441、G #437 已合入并部署。F 两项旧内核回归已修复，正式截图与候选回执独立留证；历史段落不再代表当前阶段。C 后续 #434 仍 Draft，H 未开始。Mog 派定 D3：先同步过期计划／验收，保留全部功能、数据口径、当前 UI，完成实际页面截图核对。专属工作树 `.worktrees/web-react-001-d3-comments`、分支 `codex/web-react-001-d3-comments`、Issue #433 Claim；D3候选已接入React，原六资源/三原生UI脚本和旧DOM集成断言同包删除；四视图及既有抽屉/命令完整迁移。全阶段隔离浏览器与D3回库证明、like-library专项通过，173组实际输入全滚动截图已核对（56组零差异，其余小量文字像素差异据实记录）。原productization的未改start锁等待用例期望1实际0，串行隔离3次复现，不能称全部专项通过；独立审查/CI及未部署边界见[D3验收](design/acceptance/web-react-001-d3-acceptance.md)。B 冷启动之外的断网重启／整版回退、C 成品与各模块 Mog 验收继续保留。该段为原候选时点；Mog后续已授权PR #445修复启动锁回归，通过后合并部署3000与四视图截图，见[D3验收§7](design/acceptance/web-react-001-d3-acceptance.md#7-pr-445-启动锁修复与发布范围2026-10-18)。共享库迁移和真实研究／平台动作仍不在范围内，最终发布状态以PR #445实际回执为准。


### 2026-10-15 · 阶段 C V3 重新落地（源码候选，待 Mog 验收）

PR #432 已合并至 `main@36f3fdf9`，此前阶段 C 的功能证明未达到 Mog 认可的 V3 体验。Mog 已明确退回当前前端，授权 GPT-6.1-Sol 子代理重做、root 逐页对照审核；沿用 React 和后端，验收后再合并部署。专属候选 `codex/workbench-v3-fidelity` 恢复选题墙、方案详情、完整材料书页/纸条/矩阵、画布、沉浸正文/并排改稿、独立封面/内页/导出，并补齐选题相关笔记与评论的预备、检索加入本篇和阅读集合持久化。

首篇题目为“孩子迟迟不开始写作业，把第一步变小”，Mog 已选定“驭A低手裴妈”的小红书账号。真实库仅只读取得最小材料样本，在私有隔离候选复制原文与父语境完成验证，不改共享稿件、不调用真实模型。自动化与 root 视觉审核结果见[阶段 C 验收](design/acceptance/web-react-001-c-acceptance.md)的本轮章节。每日自动十方案、模型创作质量和 Mog 成品签收仍需独立证明。本轮未合并部署，3000 仍为已发布基线。

### 2026-10-15 · WEB-REACT-001 阶段 C 首轮派定历史（#430 / #425）

Mog 明确要求恢复 V3 正式实现，并按 React 技术栈重新分派 GPT-6.1-Sol 子代理，由 root 审核。已核实 #430 为总 Issue、PR #431 于 2026-10-15 15:47 CST 合并，主线为 `0a90fd4a`；[#430 合并后只读回执](https://github.com/Himog0921/linggan-intelligence/issues/430#issuecomment-6187465728)确认 3000 同版本、Web ready、launchd 冷启动构建和线上设置读取通过；线上写操作、断网重启与回退未验收。本轮未重启服务。专属工作树前移至同一主线并恢复 #425 后端和 V3 文档。唯一实施入口为 [WEB-REACT-001 阶段 C](plans/active/web-react-001.md)，原专项 [§10](archive/plans/uiux-content-workbench-001.md#10-首个落地包一篇真实选题到成品导出) 保留为设计与验收来源。

本轮实施：React 今日选题/研读取材/创作画布/正文、独立 TypeScript 图页渲染与导出、#425 后端缺口修复，以及统一客户端和路由集成。原生候选仍暂停且不合入。首篇选题为作业启动，账号稍后选；A01–A13 沿用。当前候选已完成121项前端测试、17项隔离PostgreSQL/HTTP、10项Axum路由、1项精确范围匹配，以及正式React浏览器17场景与真实五页PNG/ZIP。浏览器仅用合成资料；真实账号/材料/模型及Mog成品签收仍未证明。详情见[阶段C验收](design/acceptance/web-react-001-c-acceptance.md)。阶段 B 线上写入、离线重启与整版回退仍按其回执留待验证，不冒称已完成；本轮授权阶段 C 实施不包含共享库迁移、真实模型外发、合并或部署。


### 2026-10-14 · 研究底稿第一阶段历史（#420，已由 v2 取代）

Mog 已派定[完整模块计划](plans/active/research-dossier-001.md)。PR #423（32780fae）已合并为 93dc826e 并更新本机 API/3000；研究暂停、两个 worker 停用。真实前端验收发现的分层漏行、重复图片、计数标题及引用冗长问题在同一交付包续修，候选位于独立 3108，尚未合并替换 3000。

### 2026-10-14 · UI/UX 首个真实创作落地包：实施计划待 Mog 确认

Mog 要求先拆计划，确认后再开始业务代码。详细实施入口为[专项计划 §10](archive/plans/uiux-content-workbench-001.md#10-首个落地包一篇真实选题到成品导出)：真实选题→材料/原声精确摘录→画布→正文→封面和内页→保存恢复→成品导出；包含复用/适配/新增清单、S0–S6 顺序、数据责任、状态/表面/共享依赖及 A01–A13 验收。建议首包纳入受控文本 AI、可编辑模板与自有图片，AI 图片生成和每日十方案留在后续范围。

本次代码依据是专属工作树及本机 `origin/main@5511d75e`，未 fetch、未查询运行库；前轮所读主目录 `edca6dee` 为较早的其他分支，已在提案补充适用边界。仅更新专项文档，原型与业务代码保持；无任务派发、模型/采集、提交推送、迁移或部署。下一步由 Mog 评审首包范围、账号/样稿及成品规格；目前实施尚未开始。

### 2026-10-14 · UI/UX 专项 V3：方向获认可，精简说明与工具栏

新增 [V3 独立原型](design/pages/content-workbench-prototype/v3/index.html)：分组纸条放大、筛选 / 原文 / 分布联动、书页翻阅与精确摘录，画布片段可一次放入正文及指定内页。实际从一篇选题走到 5 张 PNG 与正文 ZIP 导出，4 段素材 / 8 处关联，内页排序、撤销及源文回查已走通。[验收记录 §7](design/acceptance/uiux-content-workbench-001-prototype-acceptance.md)说明证据与限制。全部为本地合成原型，未改业务、未接入真实语料/AI，V1/V2 及草稿保留。Mog 已明确认可 V3 方向；本轮按截图删除重复说明、收紧工具栏，工作区得到更多高度，见[验收记录 §8](design/acceptance/uiux-content-workbench-001-prototype-acceptance.md)。落地挑战和建议顺序已按当前主仓库源码核对并写入设计提案 §13；业务实施未派定。

### 2026-10-14 · UI/UX 专项 V2：研读取材、创作画布与改稿对照

根据 Mog 的截图、视频及研读反馈，新增 [V2 独立原型](design/pages/content-workbench-prototype/v2/index.html)。多篇原笔记 / 原声 / 相似选题 → 精确片段 → 来源回查 → 结构画布 → 正文与真实差异对照 → 图文导出已完成本机交互验证。18 份材料全为合成，正式数据、模型和业务代码未接入。入口本机 :4177/v2/，V1 与其草稿保留；[验收记录 §6](design/acceptance/uiux-content-workbench-001-prototype-acceptance.md)区分实际证明与能力缺口。仍待 Mog 审美和写作体验验收。

### 2026-10-14 · UI/UX 专项阶段二：独立原型已交付，待 Mog 验收

Mog 已明确派定今日选题、材料研读、图文创作三页可交互原型，并串起正文、封面、内页到导出。交付位于同一专属文档工作树的 `docs/design/pages/content-workbench-prototype/`，预览使用独立 loopback 静态服务。三种今日布局、素材摘录入稿、本地保存恢复、图文编辑、排序与 PNG/ZIP 导出已做真实浏览器核对；[提案](design/changes/uiux-content-workbench-001-prototype.md)与[验收记录](design/acceptance/uiux-content-workbench-001-prototype-acceptance.md)分开记录实现边界。

这是合成数据原型，等待 Mog 验收整体审美与使用体验；正式产品能力缺口仍在。没有修改业务代码、生产 token 或真实数据，没有模型 / 采集 / 部署 / 提交推送。

### 2026-10-14 · UI/UX 内容工作台专项：计划与第一阶段只读基线已形成

Mog 明确要求先整理专项文档，再走查当前真实页面与代码，不修改业务代码。目标为个人每日 ≥10 个含角度/依据/提纲/素材的完整方案，并在 Intelligence 内完成正文、封面、内页与成品导出，重点看阅读量。范围及七阶段顺序见 [UIUX-CONTENT-WORKBENCH-001](archive/plans/uiux-content-workbench-001.md)。

本次运行基线为 `5511d75e`，监听 PID 67489 的 cwd/txt 与 runtime-main 一致；在同 revision 的 `codex/uiux-content-workbench-001` 专属工作树只写文档。覆盖当前主要页面及代表性交互，交付[审计报告](audits/uiux-content-workbench-baseline-2026-10-14.md)及[证据附录](audits/uiux-content-workbench-evidence-2026-10-14.md)：11 项能力现状、21 个问题、51 组有效截图状态。发现主题读取冲突、建档统计与行状态口径不一、缺数据仍给稳定漏斗结论等具体问题；完整选题供给与图文创作导出仍是产品缺口。

下一步为今日选题、材料研读、图文创作的完整原型与统一 LIDS 提案。本轮未改业务代码、数据或配置，未发起采集/模型调用，未提交、推送、合并或部署；原有后台继续运行，不能据此声称系统零调用。阶段一基线不等于修复或 Mog 业务验收。

### 2026-10-14 · 运营模块迁入 D3：总览已上线，三处小修已上线，业务验收与读屏跟进待办（Issue #409 / #417）

Mog 派定的 D3 已随 PR #415 合并上线，原 D3 上线时 `origin/main` 与 `runtime-main` 为 `6feefefe`，当前修复后同为 `14ee80b1`；共享库 10-14 10:08 执行 `0126_ops_target`。3000 本月发布 109、链接打开 100、加微 48、转化率 48%、爆款 2；今日必须处理 5 项，另有 11 项可展开。最近迁入为 10-12 22:43 的导出，只算到 10-11，默认本周仍未知。部署与只读链路见 [ACC-OPS-PAGE-002](design/acceptance/ops-page-002-visual-acceptance.md) §9；原部署是 API-only，当前复核三服务已运行，不能混写。Mog 业务验收、共享库目标写入仍待办，#405/#409 保持打开。

本轮交接派单由 [Issue #417](https://github.com/Himog0921/linggan-intelligence/issues/417) 修复未覆盖空态、趋势爆款未知、周一昨日目标，并补齐上线回执；修复已随 PR #418 合并部署为 14ee80b1；三服务与真实浏览器已核验，无新迁移、导入或目标保存。合并后返回的读屏意见已有候选修复，未合并部署，详见验收记录 §11–12。Mog 已确认每天上午 9 点检查新导出，每次 apply 仍另行授权，流程见[迁移计划](plans/active/ops-module-migration-001.md)；D4、D4b、D5、D6 未派定。

### 2026-10-13 · 运营模块迁入 D2：一级页面「运营」已上线，历史封面已迁入（Issue #405）

Mog 派定 D2 并确认：做成一级页面「运营」，页头、面包屑与其他一级页面同一套逻辑，左侧栏放二级页面；先只读，写入等登录之后作为 D4b 接入；历史封面由他从工作台电脑拷来。页面规格 [PAGE-OPS-001](design/pages/ops-page.md) 已经 Mog 确认：「运营」放一级导航最后一个；账号健康改为运营手动填写；每日数据的执行状态列 D3 前不显示。

PR #407 合并为 `main@9e45e01c` 并部署：四个只读二级页、只读接口、迁移 `0124`（封面登记表，共享库已执行）和历史封面迁入命令。走查修正的 13 处见 [ACC-OPS-PAGE-001](design/acceptance/ops-page-001-visual-acceptance.md)，其中一处是共享页头放不下第 6 项。Mog 把工作台硬盘挂到本机后，共享库迁入 510 条历史封面（批次 `dc5ce2e0`），逐文件校验，3000 上缩略图正常。合并后 Codex 审查的 3 条问题在小修 PR 中修正；待 Mog 业务验收后关闭 Issue #405。下一步是 D3 总览。

### 2026-10-13 · 运营模块迁入 D1：数据底座与历史迁入（已上线）

Mog 派定 D1（Issue #402），并确认爆款规则只管小红书：抖音播放量选填、不判爆款。分支 `feat/ops-migration-d1` 新增迁移 `0123_ops_ledger`（五张运营台账表）、导入模块与命令 `linggan-ops-history-import`（先预览、再带指纹执行）。合成数据的隔离证明全部通过，逐一改坏 11 条规则都有测试变红；用 Mog 10-12 下载的三份导出在一次性数据库上演练，40 项新旧核对全部一致，9 月发布数差异已查明是旧算法重复计数。数据含义见 [OPS-LEDGER-001](data-contracts/ops-ledger-001.md)，回执见[迁移计划](plans/active/ops-module-migration-001.md)。

PR #403 经 Mog 另派 agent 审查后合并上线（`main@3029efff`，共享库已执行 0123）。同日经 Mog 授权完成共享库历史导入：先加密备份，40 项新旧核对一致；并新建空的卢叔数学领域。页面要到 D2 才看得到。目标与人员分组无法从工作台导出，表随 D3/D4 建。

### 2026-10-13 · 运营模块迁入：能力继承审计完成，交付待排期

DEC-0009 第②项的迁移前审计已完成：只读走查内容工作台运营管理代码，并对 Mog 下载的三份导出做聚合统计。Mog 逐题确认了六项去留，并确认了整份迁移清单。详见[能力继承审计](audits/ops-module-inheritance-audit-2026-10-13.md)与[迁移计划](plans/active/ops-module-migration-001.md)。

- **迁入**：账号、发布记录（页面原样）、每日数据、目标、总览（改 4 处）、导入导出、角色与分组。
- **规则改变**：帖子改为系统每天从我方账号主页自动发现；阅读量每篇必填，≥5000 自动记爆款；有效发布由系统确认。
- **随选题池重做**：团队执行、管理周报。
- **Linggan 新建**：登录与对外访问。

计划拆为 D1–D6 六步，都还没有派定，也没有授权任何实现、迁移、部署或对内容工作台的改动。

### 2026-10-12 · 采集日常两页与管理入口收敛

Mog 已确认 JTBD Demo 的入口逻辑并授权真实代码实施，要求完整保留观察目标/采集任务当前 UI 与字段、工位管理细节；删除独立待处理/生产流。Issue #398 的候选已在 [PR #400](https://github.com/Himog0921/linggan-intelligence/pull/400) 实现两项日常导航、领域菜单与工位上下文入口、任务页既有目标的人工观察一次及旧址迁移；原目标/任务/工位正文渲染与样式保持原样，并同步主线 be4b485d。已取得编译、233 项非忽略页面测试、2 项隔离 PostgreSQL 投影与浏览器导航证明；最终快照检查及范围见 [COLLECTION-JTBD-001](plans/active/collection-jtbd-001.md) 和 PR 回执。等待 Mog 审阅，不代表合并、部署或真实采集。

### 2026-10-12 · 主题图谱完整工作区候选

Mog 已授权 [TOPIC-MAP-WORKFLOW-382](plans/active/topic-map-workflow-382.md) 完整调整：左侧主题概览/用户生命旅程、材料优先、统一检索与范围恢复、研究状态与空态修复、无方向人工备选的来源闭环。基于 main/runtime d94e72c7 的专属 worktree，保留旧概览能力；不改变方向研究规则或运营选题池职责。0121 仅获一次性隔离测试迁移授权，正式库/3000 未变，真实模型未调用。候选与隔离验证结果以活跃计划为准，不能视为运行验收。

### 2026-10-12 · 第一阶段主 Job 确认（DEC-0009）

Mog 在首次治理复盘的 JTBD 逐题访谈中确认第一阶段主线为小红书选题池经营，见 [DEC-0009](decisions/0009-stage-one-job-topic-pool.md)。事项顺序：①高赞原话库；②运营模块整体迁入（含卢叔数学，迁移前做能力继承审计）与选题池"试新候选"并行；③迁入完成后接入复用、停用建议与命中率统计；"归并成用户问题"继续调整但不阻塞。待 Mog 裁定：Issue #382 进行中的主题图谱工作是否按方向层重新对齐（Mog 已表示另找时间专门处理）；"雷达""洞察"入口去留。本条只记录决定，不代表任何代码实现、迁移或部署。

同日经 Mog 授权的采集配置变更（本机共享库）：新增 9 个家长向/跨领域关键词与 7 个家长向博主为观察目标并全部发起建档，启用既有"考研自习"建档；博主巡查规则（每日、新作品自动补详情）已生效。**关键词巡查规则尚未保存**：服务端要求建档（基线与全部详情）完成后才接受，届时按 DEC-0009 设置保存。博主巡查授权已续签至 2027-10-12。

### 2026-10-12 · 主题研究语义业务内核集中修复

PR #391 已合并并部署为 main/runtime `535b1fdf`，schema0120。三作品真实试跑表明结构接纳仍不能证明语义正确：引用未覆盖完整主张、他人支持误归本人边界、相关关系误作父子包含。研究已再次暂停，自动扩批及补采关闭；模型、16000输入/2000输出及每日每轮300万预算沿用。

同 Issue #382 的专属 `codex/topic-map-semantic-kernel-382` 候选采用 v5 短引句精确唯一定位与独立、逐调用计费的提炼/归并语义复核；待审提案不算接纳或覆盖，旧v4不升级质量。独立复审发现的目录版本混用导致重复提案及目录变化提前终止均已修正，完整新树87项隔离原生回归通过并核验资源清理；145 Rust、22组JS、41 Pi及治理通过。候选尚未提交、发布或真实试跑；下一步按三作品小批逐条原文验证同义、障碍差异及父包含，详见[活跃计划](plans/active/topic-map-core-001.md)。

### 2026-10-12 · 主题研究真实规则反馈修复

PR #390 已合并并在本机3000加载c64b8318；沿用原模型与预算的五窗口真实试跑无截断但全部校验失败，0接纳，研究再次暂停，自动扩批关闭。新同Issue专属 `codex/topic-map-rule-feedback-382` 修复首轮约束与有界静态规则反馈，保持未知/来源守门和失败费用；真实归并与父子质量尚未通过。

### 2026-10-12 · 主题研究输出可靠性修复

Mog 已要求先暂停，再修复输出截断、归并规则诊断与批次规模，最后恢复真实验证。领域 policy 已暂停，无在途主题调用；同一 Issue #382 专属 `codex/topic-map-reliability-382` 基于 main `eaa0cfc6`。保留原模型与 token 配置/预算，不把任务退役或失败算作完成；采用完整语义的紧凑运输、更小窗口与 20 窗口有界批次、精确规则码，真实结果仍待修复后验证。计划：[topic-map-core-001](plans/active/topic-map-core-001.md)。

### 2026-10-12 · 主题图谱读取性能发布 / PR #389

主题图谱性能候选已完成本地实现、70项隔离PostgreSQL、108项lib单元、2项API单元、19组JS回归和只读实测。首次侧栏进入29230→5174ms，刷新中位5423ms，单主题4628ms；同一稳定1924作品/38主题/80附研究作品的API正式约14666ms→候选4700.8ms，响应21.62MB→5.86MB。除明确缩减的media详情外业务投影/来源指纹一致，完整统计独立重算差异0。分支基线已跟进main@ffcdb1eb保留PR #388修复；该包已随后合并为 main `eaa0cfc6` 并部署3000（PR #389）；12日可靠性修复再次核对正式 listener PID4607 的 cwd/txt 均来自该 runtime-main。性能执行时研究为active；后续可靠性修复已按 Mog 指示暂停领域研究。详见[性能回执](plans/active/topic-map-read-performance-382.md)。GitHub Claim外发被自动审批拒绝，保留本地留痕。


### 2026-10-11 · 创作者已确认设计发布收口 / PR #386

Mog指定合并PR #386并刷新3000，采用后续确认的八列表格、筛选草稿弹窗与三分区抽屉；#384调度候选不纳入。发布前独立复核发现并修复搜索防抖关闭筛选草稿，39离线场景及11真实GET候选路径通过，治理缺项已补；无migration或模型策略变更。合并、canonical runtime安装和Mog业务验收以下续实际回执为准；详见[创作者验收](audits/creator-discovery-001-acceptance.md)2026-10-11节。

### 2026-10-10 · 评论研究进入性能后续候选

PR #380 已按 Mog 授权合并为 main `732bedef` 并刷新本机3000，接口优化已部署；其接口耗时不能证明整页进入耗时。Mog 随后反馈概览进入仍近7秒。只读定位确认初始化等待研究弹窗准备，setup单次4197.1ms，而overview572.5ms；此前漏测该串行依赖。

Issue #358 的独立 `codex/comment-study-entry-performance` 候选延后弹窗准备，只加载当前视图；打开弹窗后保留正在准备/失败重试与预览启动守卫，研究批次按需读取，深链/历史不变。真实库只读浏览器各3次导航/刷新到概览标题可见（含自动化开销）：正式4401/4500/4637ms、候选656/657/668ms。合成悬挂准备接口回归对已部署JS为预期失败，对候选通过；完整合成UI与相关API单元43+1通过，12项明确ignored。独立review发现并修复首次方法在works等待时提前开放保存，新增准备前后负例后复审PASS；最终候选重复刷新883/783/592ms，中位783ms。没有migration或真实模型命令；此候选尚未合并、刷新或获得Mog验收。详见[实施计划](plans/active/comment-study-read-performance-358.md)。

### 2026-10-10 · 评论研究读取性能候选（PR #380 形成时记录）

Mog 要求继续优化评论研究；Issue #358 的独立 `codex/comment-study-read-performance` 候选基于 `3830d4b6`。事务局部关闭目录 JIT、避免可选筛选通用预编译计划退化，评论页复用同一快照的完整统计；批次列表一次汇总页内当前有效 Signal，保留跨 Run Pair、no_signal head 和限制语义。各3次GET对照中，目录2707.8→373.5ms、搜索1969.5→253.3ms、总览2442.6→640.1ms、批次611.9→79.8ms；原有字段相同，12次交替搜索未复现秒级退化。详见[实施与验证计划](plans/active/comment-study-read-performance-358.md)。

本次仅独立3001强制只读候选与隔离合成证明；正式main/runtime仍为已交付Corpus性能PR #379的3830d4b6。本次新候选的PR、合并、3000刷新与Mog验收按各自回执记录；没有数据库migration、共享库写入、新Run或主动provider调用。旧段落描述各自当时状态，不代表本日runtime身份。

### 2026-10-08 · 评论研究真实运行解释与九类标签候选

只读核对 `origin/main` 与本机 `runtime-main` 均为 `5f31b8588a0d4e2d6b8b1a7b55656b311516b6ff`（#365 已合并并刷新）；下方 #364／窄屏“候选”的措辞只描述当时阶段。正式 `:3000` 仍运行此版本。Mog 的两次手动 Run 可从服务端开始请求、冻结清单和 Target 表交叉核对：`1cc7d809…` 选中 4 篇／候选 91 条，排除作者本人 26、已有研究记录 17、文本不具研究条件 2，实际入包 3 篇／46 条；`a21dc6c8…` 选中 5 篇／候选 142 条，排除 41、76、2，实际入包 1 篇／23 条。两次均为“只研究新评论”，100 条上限未截断目标；无法从库内还原提交前每一次页面勾选。

Issue #358 的独立分支 `codex/comment-study-run-clarity-358` 已完成候选：运行页明确分开展示选中、实际入包、候选、排除与目标；发起弹窗先选择再预览，并在输入变化后作废旧预览；用户评论目录新增现行九类当前有效 Signal 的“研究标签”列和全目录服务端筛选。旧六视角按 Mog 最新决定弃用。候选本机 `:3001` 接真实共享库只读走查了两次 Run、29 条“需求”筛选结果、评论详情与两篇作品预览，以及 1440／1024／390 CSS px 和 720px 等效放大重排；未创建 Run、改写共享库或外发模型。此候选尚未合并／部署，正式 `:3000` 与 Mog 的业务签收仍待后续回执；[浏览器验收记录](design/acceptance/comment-study-release-polish-358-acceptance.md)记载细节。

### 2026-10-08 · 评论研究 Pair 性能修复已发布；窄屏表格候选

Mog 对 [#364](https://github.com/Himog0921/linggan-intelligence/pull/364) 的精确 head `e20462f` 授权合并及刷新本机 `:3000`。PR 已合并为 `main@c81c68a7`；受控安装确认旧 worker drain、迁移台账无需更新，API／巡检 worker／媒体 worker 均从干净的 `runtime-main@c81c68a7` 启动。`:3000/health` 为 ready、数据库 READY；真实浏览器可读概览、评论目录、真实 Run 的四条 Signal 及用户问题页，部署后没有新评论研究模型请求。原失败 Pair 已达尝试上限，发布本身不会自动重试；Problem 仍为 0。

约 495 CSS px 的真实浏览器走查发现“用户评论”五列表格将中文逐字挤成竖排；通用表格最小宽声明在浏览器里实际计算为 0。Issue #358 的本分支修正这一声明并为评论表补足列宽，保留横向滚动，不修改数据／模型合同；390／520／1440px 候选样式已用只读代理接真实数据验收，尚未合并或刷新运行。原两条评论可由现有显式重研预览再次选中，但新 Run 可能产生多于预期的模型请求；此前三次预留授权已用尽，未获新授权前不得执行真实重研。

### 2026-10-07 · 评论研究真实双评论验收与 Pair 瓶颈

[#361](https://github.com/Himog0921/linggan-intelligence/pull/361) 已合并为 `main@abd750b1` 并刷新本机 `:3000`；运行目录与三个服务曾核对为该提交，四视图真实浏览器链路已走查。Mog 单独授权把已预览的两条 ADHD 评论清洗文本及必要作品、父评论语境发往 `api.deepseek.com` 的 `deepseek-v4-flash`，仅一次 Run、54,000 词元预算、最多预留三次模型请求。

该 Run `e5bf72a5-b311-40a1-96bb-dd10d6d889bb` 的语义模型请求真实成功：两条 Target 成功，当前有效 Signal 为 belief、experience、need、problem 各一条，用量 2,691 词元。后续 Pair `672832b6-081d-40f7-9bc6-f394420cc936` 的第一次分发资格 SQL 耗时约 380 秒，超过 45 秒期限；#361 截止保护将其回滚为未分发、零计费。第二次重试仍卡在同一查询，已精确取消该数据库语句并按原释放路径结算为未分发、零计费。三次请求预留位已用完，实际外发只有语义请求一次；真实 Problem 仍为 0，不能宣称正向业务验收通过。

Issue #358 的独立候选 `codex/comment-study-pair-fence-performance` 基于 `origin/main@abd750b1`，把 Pair 栅栏改为只核对两条冻结 Signal 对应的最新 Target 与当前评论。真实库只读对照中，该 Pair 的新旧资格结论一致，新查询执行约 0.47 毫秒；隔离 PostgreSQL 与本地假适配器的定向用例通过 Resolution/Pair 分发和来源限制路径。候选尚未 PR、合并或部署；新的真实 DeepSeek 请求不在本次三次预留授权内。

### 2026-10-06 · Collection 关键词采样与恢复候选

Issue [#362](https://github.com/Himog0921/linggan-intelligence/issues/362) 的独立分支基于 `origin/main@b6415315` 修正关键词巡查的三个数量：下拉预算、候选上限与最终保留数；最多评论等排序不再一律按点赞重排。候选回执记录真实下拉数，服务端只在下拉预算已执行且取得冻结的样本数、达到候选上限或确认到底时认定本轮完整；未完成轮次已接纳的材料仍可用于详情追采。普通发现开页失败按真实阶段上报并有单工单失败上限；目标页区分最近巡查未完成与当前受阻，工位页把 `station_busy` 解释为容量等待。

这仍是未合并、未部署的源代码与隔离测试候选。当前 `:3000` 和已安装插件不因此改变，旧 CapturePackage、Receipt 与共享数据库未改写；真实平台回采与 Mog 页面验收尚无新回执。状态词义与验收边界见[变更清单](design/changes/collection-keyword-sampling-recovery-001-ui-change-manifest.md)。

### 2026-10-06 · 评论研究四视图已合并并在本机运行

[#338](https://github.com/Himog0921/linggan-intelligence/pull/338) 于 09-28 合并；[#359](https://github.com/Himog0921/linggan-intelligence/pull/359) 的精确 head 为 `fe90f290`，于 10-02 合并为 `main@8ec92f3d`。其 [exact-head CI](https://github.com/Himog0921/linggan-intelligence/actions/runs/37012606180) 的编译、单元、隔离 PostgreSQL 与前端步骤通过；日志确认两条真实 Axum＋隔离 PostgreSQL＋Playwright 路径均实际运行并通过，分别覆盖方法选择／预览／启动／停止，以及评论→Run→请求→Signal→Problem→证据→返回。`integration-source-export` 按仅 push 触发的条件跳过。

2026-10-06 只读核对：干净的本机 `runtime-main@8ec92f3d` 与远端 `main` 一致；`runtime-identity.json` 记录构建于 10-03、migration head 为 `0114_comment_study_effective_head`，共享数据库最新迁移账本的 0114 hash 与仓库迁移文件一致。`:3000/health` 返回 ready、数据库 READY、scheduler running；当前 `/api/local/comment-study/overview` 返回 HTTP 200、`comment-study.read.v2` 和三类服务端汇总，`/corpus/comments` 返回 HTTP 200 与四个视图入口。真实浏览器在正式 `:3000` 打开四视图及评论详情／Run 链路；后续页面修正又通过只替换 JS 的只读 `:3001` 代理验证跨领域、无领域与评论→Signal→返回路径。后者是候选脚本验收，尚未合并部署。[页面验收记录](design/acceptance/comment-study-release-polish-358-acceptance.md)列明边界。

这次核查未创建新的 Run 或发起模型调用。部署后的新真实 Run、Problem 正向质量和 Mog 对当前页面的业务验收均无本次回执；[正式验收账本](audits/comment-study-productization-001/acceptance-status.json)仍为 5 `PASS`、49 `NOT_RUN`，不能按隔离测试数量批量提升。Issue [#358](https://github.com/Himog0921/linggan-intelligence/issues/358) 在 #359 合并后重新打开，继续承载运行与业务验收收口。每日自动计划仍属下一阶段。

2026-10-06 对 `Problem=0` 的只读复算确认：当前 87 条合格 Problem/Need Signal 均有合格向量，Problem 与 membership 表确为 0；历史 153 次 Pair 全部拒绝，但只有其中一部分有结构化的不同题理由，不能把其余拒绝也解释成已证不同题。当前仍可调度的 6 条未比较 Signal 中，5 条各有同 Run、不同作者且未配对的候选；最靠前的合格候选在旧全域向量池分别排第 19、38、46、54、59，旧代码先截前 16 再筛配对资格，使它们无法进入比较。Issue #358 的后续候选仅把确定性资格过滤前移到前 16 的排序截断之前，保留有界预算与双独立作者、同题判断、一次首比较约束；具体语义是否同题、能否建 Problem 和新模型消耗仍待运行回执。

上述五条分属两个派发仍为 `enabled` 的既有 v2 Run，按只读账本复算均有至少一次 Pair 的剩余预留额度。worker 每约十秒轮转 Pair 阶段；因此发布召回修复后，后台**可能自动发起新的真实模型调用**，即使没有人点击启动新的 Run。准备合并与本机发布时必须把这一现实后果和预期预算说清，不把源代码部署授权偷换成新模型调用授权。

Issue #358 的下一只读页面收口候选从 #360 精确提交 `c7ca7195` 独立分出，不改变待批准的 #360 head。设计手册逐项回看发现概览办法／经历只有三条预览、旧 Problem 修订只见编号、Run 列表缺方法与归并待办、评论详情缺直接关联信号等真实可用性缺口。候选增加当前有效 Signal 和评论关联的跨 Run 分页、安全的历史定义与来源分布读取，并把这些入口接入四视图；现有未闭合表达的分页已证实存在，不重复实现。隔离数据库与脚本验证分别记录在本月进度；候选尚未提交 PR、合并、部署，真实 Problem 正向路径和 Mog 业务验收仍未证明。

### 2026-10-02 · 评论研究整包收口开发时快照

Mog 要求以附带手册的 P0–P5 功能与四视图为收口目标，降低开发期形式测试门槛，以真实接口浏览器端到端路径验收；允许继续开发 P3/P4，不将独立 Gold Set、完整 T01–T54、历史数据迁移或备份恢复演练作为代码推进前置。质量、共享库、发布与 Mog 业务验收仍分别记证。自动每日计划属于下阶段，保持关闭。实时 GitHub 核对 #338 已于 09-28 合并，`origin/main@74f1a486`；本机三个服务进程及 runtime identity 均为该 SHA，`:3000/health` READY，migration head 0113。只读数据库与浏览器看到现有真实 Run/Signal 和 provider 用量回执，Problem 为 0；这证明模型调用已发生，不证明语义质量或 Problem 正向路径。

当时已部署页面四 Tab 可打开，但评论详情没有到作品、Run、Signal 的可点击路径，Run 选择器对已结束状态显示“未知状态”；Problem 当前支持与后台召回尚未统一最近成功/无信号 Target head。受保护分支 `codex/comment-study-complete-001` 已实现有效依据、来源资格、服务端概览汇总及页面可回证导航。独立 PostgreSQL 上两条真实 Axum／浏览器端到端路径和关键后端定向场景已通过；截至该日分支尚未合并、部署到共享运行，也未由 Mog 验收。此段只记录 10-02 发布前状态。

### 2026-09-30 · Comment Study 历史合并与验收状态

PR #338、#350 与 [#351](https://github.com/Himog0921/linggan-intelligence/pull/351) 均已合并；#351 的代码 merge commit 是 `c6cf2d9c`，当时 main 包含该提交。#351 最终 head `fd15b056` 的 [exact-head CI](https://github.com/Himog0921/linggan-intelligence/actions/runs/36658996689) 已通过 compile、unit、隔离 PostgreSQL 和前端行为验证。Mog 后续明确要求刷新本机 3000；当时 `runtime-main@f05b0655`、三个 launchd 进程与 `:3000/health` 均已核对，数据库 READY。合并、部署、模型调用与 Mog 业务验收是不同事实。

[正式验收账本](audits/comment-study-productization-001/README.md)的 2026-09-30 逐项核验确认机读状态仍为 T23–T26、T43 共 **5 PASS**，其余 **49 NOT_RUN**；隔离测试子场景不自动提升完整 T 编号。T06 旧文字要求未知评论作者不进入研究，而现有 AC043 代码与隔离测试允许其形成 Target/Signal、仅禁止作为新 Problem 的独立已知作者，待合同版本裁定。approved-v1 文档校验仍有八份 SHA 不符。P2 代码已合并，真实页面/数字、真实模型路径与 Mog 业务验收未完成；P3 质量出口仍受独立 Gold Set、分层 Recall@K、获批真实模型和 M4 资源实测约束，P4 尚不放行。

### CREATOR-GAP-REQUEST-20260929（交付分支候选；未进入运行）

南瓜哒哒“补采缺口”失败的只读复算显示：9 篇缺合格详情的作品被 Domain usage 多行 join 展开为 23 行，`ensure_material_targets_belong_to_platform` 比较唯一内容行数与含重复的候选长度后返回 `InvalidMaterialTargets`，事务回滚。独立分支 `codex/creator-gap-error-20260929` 已用存在性查询消除重复，并把 creator 请求的数据库/合同失败从“当前不能重复提交”改为可追踪的真实失败提示；隔离 PostgreSQL 全套与 API/UI 回归通过。分支已推送供 PR 审查；本修复尚未合并、部署或触发真实补采，常驻 `:3000` 和共享库未改变。

Mog 已明确“补采缺口”包括未取得完整详情、标题、正文、媒体、评论等，并确认差评论时可重新打开一次详情页，采集最多 30 条评论。新增候选将可证实的评论缺口纳入人工补采；已接纳巡查新增且缺详情的作品，仅在创作者规则显式开启并另有精确用途授权时自动跟进，每次最多 3 篇，默认关闭。新插件不再把未知平台评论数误写成 0，作品目录可按已接纳发现卡的原始类型标记显示视频/图文/未标记分布。视频正文 `UNKNOWN` 和旧原始 `normal` 类型缺少足够来源，不能单凭它们扩张补采或声称图文/视频类型已入 canonical 材料。分层边界见[变更清单](design/changes/creator-gap-request-20260929-ui-change-manifest.md)，验证状态见[进度记录](progress/2026-09.md)。

### 2026-10-01 · 模型密钥单路径修复已合并并部署

9 月 29 日 18:18、9 月 30 日 10:26 和 12:18 三个 Comment Study Run 大量目标失败；模型回执均为 `model_secret_unavailable`、`callStarted=false`、计费 token 0。旧 Keychain 条目可在用户一次授权后交互式读取，但常驻 worker 不能稳定读取；具体底层错误类别仍未知。[MODEL-SECRET-DEV-001](plans/active/model-secret-dev-001.md) 的 [PR #356](https://github.com/Himog0921/linggan-intelligence/pull/356) 已合并（merge commit `f05b0655`），其 [exact-head CI](https://github.com/Himog0921/linggan-intelligence/actions/runs/36686765817) 和独立代码审查通过。

本机数据库引用的两项 DeepSeek 密钥已逐字迁入单一文件 SecretStore；第三项本机 WeMM Runtime 原无 Keychain 条目，按空凭据合同建立空文件。三个文件经新版 Rust SecretStore 在实际 support dir 下 3/3 读取通过，目录 0700、文件 0600；旧 Keychain 工作区 service 已清空。受控 install 收到旧 worker drain 回执，三个 launchd 进程运行 `runtime-main@f05b0655`，`:3000/health` ready、数据库 READY，模型设置 API 报告 `LOCAL_PRIVATE_FILE`。部署后尚无新的模型调用回执，故真实 provider 路径和评论研究业务验收仍未证明；旧失败回执保持原样。

### 2026-09-28 · GREENFIELD Comment Research 历史快照（当时 PR #338 为 Draft）

2026-09-28 最新远端核对：PR #338 仍 OPEN/Draft，head=`47de9bf04cd6aee0afa9beed9147635f1260265f`，base=`main@f54778562fbaaff56f482cb3d9fa46b0bbf51965`，`mergeable=MERGEABLE`、check 状态 `CLEAN`。修复了方法保存成功提示早于方法目录刷新完成的竞态；真实 Axum/隔离 PostgreSQL 浏览器回归核对刷新后的选中 `policyRef` 等于 HTTP 201 回执且不同于原方法。精确 head Actions [36394260704](https://github.com/Himog0921/linggan-intelligence/actions/runs/36394260704) 全绿：compile、unit、隔离 PostgreSQL proofs、frontend behavior tests 均通过；push-only `integration-source-export` 按条件跳过。

PR 合并事实单独记录：#338 的 `mergedAt` 为空，PR issue timeline 没有 merge/close/reopen 事件；以当前 base 比较，分支有 60 个独有提交，`git cherry` 未发现 patch-equivalent upstream 提交。分支中的 `1c43495`、`c2af487`、`d10b945`、`c7e23f6` 是把当时的 main 合回 PR 分支。已有相关能力确已上线，来源是独立合并的 #323–#327、#329、#335、#336；#347 是 Collection 改动。当前本机 `runtime-main` 正是干净的 `main@f54778562fbaaff56f482cb3d9fa46b0bbf51965`，包含这些独立 PR 的结果，但不包含 #338 候选增量。

P0 本机只读快照：`:3000/health` 为 ready、loopback-only；`runtime-identity.json` 的 migration head=`0106_reply_disposition_quota_requalification`。以 `BEGIN READ ONLY` 查询本机 `linggan_intelligence_dev`（PostgreSQL 16.14），ledger 共 107 条，最新 migration SHA=`184f91bff325cf335424df212874a53a621169fa388d6b29290c70a5260e0773`，与运行身份一致。#338 候选 `0105`–`0109` 五个迁移 ID 均未登记；方法版本列、Run 预算列、Target 稳定评论 ID/终态列及 model request 表也未出现。Comment Study 19 张表有 41 个已验证 FK、99 个已验证 CHECK、41 个有效索引、0 个禁用用户触发器。原声表计数：Material Content 1,459、Material Comment 6,257、Media Blob 3,009、Media Slot 3,064。

同一只读快照中已有历史 Comment Study Run 7 个（均 `completed_with_failures`）、Work 22、Target 945（succeeded 613、failed 158、no_signal 70、needs_context 104）、semantic attempt 1,412、Signal 764、Resolution 79、Problem Pair 145、Problem/Revision/Membership 均 0；219 个 batch 都是终态。通用 model invocation 共 2,379、当前 `running` 为 0；除盘点连接外 active SQL 为 0。以上是历史账本和当时状态，不把 completed_with_failures 报成在途故障。此次 P0 只读事务没有修改共享数据；任何候选 migration 前仍需临近执行时复核并保存受保护行基线，P8 备份/恢复另行完成。

P2 方法版本、preview/start、预算绑定、幂等、v2 执行、取消/显式重试已有源码和隔离 PG/Axum 证明。本地提交 `677e38a8` 又覆盖真实页面经 Axum 路由和 disposable PostgreSQL 的方法新建（HTTP 201）、CAS 激活（200）、预览、启动及停止；直接数据库断言 Run/Target/start receipt/model invocation=`1/2/1/0`，Run=`cancelled`，controlVersion=`1`。夹具先设活动 `legacy_unrecorded` 方法，证明浏览器会选取已记录版本；额外合成浏览器场景模拟目录第一页 100 条、活动 legacy 在下一页，确认激活请求仍携带正确 CAS ref。修复后完整本机隔离套件复跑通过：Intelligence/PostgreSQL 67/67、Problem rebuild 48/48、Axum/PostgreSQL 10/10、worker SIGTERM 1/1、SIGKILL/restart 1/1；合成分页 Playwright 及 `node --check`、Python AST、`git diff --check` 通过。不证明部署页面、真实 provider 或 Mog 业务验收。

仍需 Mog 对真实页面动作、范围、预算数字和回执进行业务验收；真实模型质量样本须在明确样本与预算后单独授权。P3 自动化执行/恢复已有隔离证明，但质量出口仍需负责人独立标注 Gold Set、分别核验既有 Problem 与未归并池的分层 Recall@K、获批真实模型语义/HTTP 质量路径，并补 M4 system memory pressure/swap 实测。Gold Set 未通过前不进入 P4；其后严格按 P4→P5→P6→P7→P8。P0 本机 main 运行库已完成一次只读现场盘点；P8 备份/恢复仍待单独建立与演练，不从隔离测试推断。T01–T54 仍为 5 `PASS`（T23–T26、T43）和 49 `NOT_RUN`，不按测试数量折算阶段完成度。

本增量未改动 `:3000` 运行时、共享数据库或 migration，未调用真实 provider，未部署或合并；PG 证明使用一次性 disposable database/container/volume 和 synthetic adapter。原工作区 `AGENTS.md` 改动未纳入候选提交。

### DOMAIN-UNIFICATION-001（#343/#344 已合并并刷新本机运行；作品读取修复候选待集成）

Mog 已确认正式 Domain 在基础采集、详情、评论、媒体与研究能力上平权，Domain 只定义研究上下文；Collection 需要新增“领域管理”。当前唯一推进入口是 [DOMAIN-UNIFICATION-001](plans/active/domain-unification-001.md)：它冻结 Domain–Target 的 `primary/reference` 配置、相容多用途共享一次 WorkOrder、统一 canonical Material/Comment/Media、Domain-scoped Corpus 与 Comment Study、pause 真语义，以及开发期旧 `cross_industry` 路径的直接清理。

本计划明确不为旧开发数据建设双写、长期 fallback、逐行审批、down migration 或专用保全系统。WP0–WP5 已实施并经完整隔离 PostgreSQL 套件与双轴独立复审；#343 修复了旧 Evidence 兼容读取跨 Domain、任务冻结用途不可见、暂停拒绝原因折叠和 CSS token/窄屏问题，合并时为 `main@46e10837`。后续 #344 语料入口与评论研究领域菜单修复已合并，2026-09-26 核对 `origin/main` 与本机 `runtime-main` 均为 `30fe3063`。WP4 领域管理在隔离 PostgreSQL 与浏览器完成配置写入、pause/resume、多 Domain role、冲突反馈和键盘焦点验收；部署后本机浏览器确认页面可读、任务列表与详情均显示冻结领域用途，390/1085/1440 视口均无 DOM 横向溢出。

此前的当前 schema 隔离副本升级已证明 0102→0103→0104 对旧数据可行（当时源库 1,352 Content）。随后对共享 `linggan_intelligence_dev` 制作迁移前有效快照，并在受控停止 API、巡检 worker、媒体 worker 后应用 0103/0104：升级前实际为 1,362 Content、24 条旧 cross sample/detail、245 条旧评论；升级后 1,362 Content 均有 `legacy_domain_migration` usage、缺失 0，旧 cross 表与 canonical 单值 Domain 列在 `public` 下均为 0。24 个孤儿 session 与 72 条旧 preparation 退役，212 条 grant attempt 保留、其中 24 条解除旧 session 引用。快照只用于可恢复性，不建立长期兼容层。当时记录的“旧考研样本/评论没有 canonical Package 血缘”现已被 2026-09-26 的原始包走查证伪：它们没有**已投影的 canonical Material**，但原始 accepted Package、WorkOrder/Domain usage 与 record disposition 仍在同一共享库。当前运行库考研正式领域仍为 0 篇/0 评论；隔离修复候选可从原包补投影，尚未写共享库。

2026-09-26 候选 `codex/all-domain-works-130` 修复 Collection“全部领域”目标抽屉与关键词列表跳过标准作品目录的问题；目录仍只从已接纳的 canonical Material 及冻结 Domain usage 读取，不恢复旧 `cross_industry` 表或运行时 fallback。针对“全都给姐上岸”的一次性限定目标/领域补投影工具默认预览，仅复用现有接纳函数。隔离真实数据副本首次补出 24 篇作品、24 条详情、245 条评论，再跑新增为 0；166 条 quarantined 回复不进入材料。此为候选和隔离证明，不代表共享库修复、`main` 合并、`:3000` 刷新或 Mog 验收。

Issue #130 保持开启并标记 `ready-for-human`，等待 Mog 对实际业务页面和数据后果验收。PR #343/#344 已合并；共享迁移及本机 `runtime-main@2587fba4` 的基本 API/浏览器 smoke 已执行，`:3000` health 为 200，runtime identity 的 migration head 为 0104。PR #338 仍为 OPEN/Draft；当前候选未合入、未应用共享 migration、未发布。真实平台重采、插件重载、非空媒体与本次作品修复的共享库补投影及 Mog 验收均未发生；不从健康页推断完整业务交付。隔离和本机浏览器分层回执见 `docs/design/acceptance/domain-unification-001-acceptance.md` 与本月进度。

### OCR-CONTENT-LAYERING-001 / Issue #296（候选源码；未进入共享运行）

- 用户已基于 50 张本机封面盲测确认：PaddleOCR 的识字质量显著优于 Tesseract，但原始识别仍会混入小红书水印、截图界面和场景偶发文字。候选把图片 OCR 切为 PaddleOCR v4，并保留 raw 文本、逐行归一化坐标、置信度与版面 JSON；本地规则只移除确定的平台边缘水印/UI，复杂图保持 `PARTIAL`/`NEEDS_REVIEW`，不会补写或删猜正文。
- `cover_headline` 与 `image_substantive_text` 是不同的可追溯派生视图。平台标题为空时，Evidence Library 仅可从前 3 张图中合格的 `cover_headline` 补显示标题，并标记“封面 OCR”；平台原始标题不被覆盖。历史 Tesseract OCR 将在共享 migration 后退役为不可读/不可检索，原始媒体、Evidence、事件和 ASR 不删除。
- 已完成源码编译、媒体 worker 规则测试、B39 原图 bridge 实跑，以及完整一次性 PostgreSQL LOCAL-001 契约证明；尚未执行共享 migration、历史重排、Paddle runtime 安装、视觉模型真实调用、runtime/3000 切换、PR 合并或业务验收。

### DETAIL-PAGE-SESSION-REPLAY-SAFETY-001（`0.8.52` 单轨候选已完成隔离验证；待进入共享运行）

- 针对同一 XHS 笔记被 `in_progress` claim 重放后反复打开详情页的缺口，候选新增一张**非任务账本**会话表：证据材料按 `(work_order_ref, content_public_ref)` 唯一，跨行业样本按同构键唯一；每个会话绑定首个安装、持久 `grant_request_id`、冻结采集计划及可空的 Chrome 导航观测。Task → Attempt → CapturePackage → Receipt 仍是每个 lane 的唯一交付事实。
- Producer 先把 request id 与“导航已消费”写入同一个 IndexedDB 事务并等待提交，再能创建窗口。同 request id 可取回原 grant；新 request id、换安装、缓存缺失、已消费而 tab 丢失均停止自动导航。正文先进入 durable outbox；页面 payload 不随原 Lease 到期删除、又不能被新 Lease 接管，只有全部冻结 lane 已入 outbox 才可清理。
- 当前候选移除了活跃插件内的旧 Workbench 协议、轮询、同步、旧本地表和专属测试；`references/current-v2` 仅作受校验的历史证据，不参与构建或运行。对同一详情内容，缓存损坏、已消费许可对应的 tab 不确定、窗口/页面就绪失败会报告 `detail_page_session_recovery_required`，服务端终结该 lease 的同内容冻结 lane 为 `unavailable`；只有已保存 payload 的 outbox 交付失败可安全重试，且不会重新导航。`0092_detail_page_session_recovery_boundary.sql` 扩展失败码闭集并登记至本地运行与完整 fixture。候选包为 `0.8.52` / SHA-256 `7d9e0bd969b1424d06f9cfb149a81d8371b66d921233b06ce6d75e8bc7d6d270`。插件核心 258/258、抖音/共享运行时 69/69、Rust Evidence 70/70 与完整隔离 PostgreSQL LOCAL-001 proof 均通过；该 proof 的容器、数据库、卷已清理。尚未执行共享 migration、runtime 切换、Chrome 插件重载、真实平台访问或 Mog 业务验收。

2026-09-20，Mog 授权在隔离分支补全授权恢复与页面风险保护：`0090` 追加 grant 结果审计、30 分钟内两次明确风险拦截页触发的安装级 12 小时 cooldown，以及服务端容量复核；浏览器 `prepared → grant_accepted → consumed` 使授权传输不可达能以专用 pre-Attempt 失败回队而不导航。后续 `0092` 将不可恢复的会话状态与可重试的授权/交付错误分开；它仍不代表共享 migration、runtime 切换、Chrome 重载、真实平台访问或业务验收。

### TARGET-DELETION-AUTHOR-001 / Issue #214（候选已完成隔离验证；未进入共享运行）

- 本包把“删除观察目标”收束为删除控制决定及其申请、准入、工单、租约、详情会话、授权尝试、lane preparation、执行资格和调度历史；已接纳的作品、详情、评论与采集包不随之删除。人工作品失效结论只对该目标目录生效，因此在逐字确认删除整个目标时随控制面一并删除，并在预览中计数披露；跨行业样本已经进入参照语料，安装级明确风险页信号仍可能支撑 cooldown，两者都会阻止删除。
- `0063_content_author_attribution.sql` 从已接纳的内容详情优先、同次 `profile_discovery` 受限回退地建立作品作者归属读取面。它支持控制目标删除后保留作者归属；它**不是**评论研究 V1 的作者回复清洗或语义归并实现。
- 本轮隔离 PostgreSQL 已通过作者归属、删除控制历史、阻止删除保护事实及名称确认等新增证明，并修正了四处旧测试夹具/断言，使其符合现有的 200 篇渐进建档、工单重排、租约历史与人可读北京时间合同。完整 LOCAL-001 脚本已通过，且其临时数据库、容器和卷均已清理；该结论仍只是候选源码证明，不代表共享迁移、PR 合并、运行时切换或真实采集。


### COMMENT-STUDY-REBUILD-001 / Issue #295（已发布；作者声音排除修复候选中）

**后续范围修订**：本节后文 ADHD 专属 Comment Study 范围记录的是当时的部署状态。Mog 于 2026-09-23 通过 [DEC-0008](decisions/0008-peer-domains-and-unified-material-pipeline.md) 将其修订为显式 Domain；本 worktree 的 Comment Study Domain/role 隔离目前只有候选源码与隔离 PostgreSQL 证明，尚未合并、发布或浏览器验收。

Mog 于 2026-09-16 明确决定不迁移任何旧评论研究派生结果，并要求不保留旧 V1/V2/V3 代码路径、合同或兼容层。当前唯一权威是 [DEC-0006](decisions/0006-comment-research-clean-rebuild.md) 和 [COMMENT-STUDY-REBUILD-001](plans/active/comment-research-rebuild-001.md)：领域 ADHD 是组织范围，用户选择笔记，评论是研究目标与唯一 Signal 原声证据，标题／正文／OCR／ASR 和确知父评论只作为可追溯语境。开发期本机 reset 仅删除评论研究派生层，保留原始 Evidence、资格事实与媒体 disposition；随后评论研究已合并并刷新到本机 3000，Mog 已开始首次真实手动验收。真实 provider 端到端成功、P3 Recall@K 与业务结论仍分别 `NOT VERIFIED`。

2026-09-17 新建独立候选 `codex/comment-study-layout-001`（base `origin/main@0a7c940`）收束用户指出的初始输入页空白 Hero 与卡片选择器：现为紧凑受控启动工具栏、可筛选的连续候选表、运行读数。临时 3001 真实读取到 100 篇 ADHD 作品；桌面和 390px 宽度浏览器走查、筛选后保留选择、页面静态测试均已通过。该候选不改 API、schema、worker、模型或 shell，且尚未提交、合并、刷新 3000 或取得 Mog 业务验收；细节见 [布局验收记录](design/acceptance/comment-study-layout-001-acceptance.md)。

已在 `codex/comment-research-rebuild-001` 的专属 worktree 建立新 schema/reset guard、ADHD/as-of/restriction/withdrawn-media source gate、用户选笔记的 repeatable-read Run 冻结，以及评论原声 span/Frame basis 的原子 semantic acceptance。同篇评论现在可冻结为 batch：输入 manifest 固定 work context 和每个 target，输出必须按 targetRef 返回；局部合法结果已可独立写入 Signal，遗漏 target 生成有界 rejected attempt 并回到 queued，不能被伪装为 no_signal 或拖垮同批合法结果。batch 先以短事务领取带 token 的 lease、在发送前重核来源资格；来源冻结后被限制则取消 batch 而不外发正文。每个已租 batch 至多预留一个不含评论正文/提示词的通用调用账本收据；重复预留复用同一收据，接纳的 semantic attempt 指回它，正常接纳、合同拒绝和 lease 过期都会终态写回账本。长期 Problem 现以服务端封存候选集、逐候选固定维度比较、单一明确匹配才建立 membership 的方式落地；无匹配保持 `deferred_novel`，只有不同作者的两条独立评论均已无匹配并提出带 include/exclude 边界的共同定义时才可建档。真实 Pi adapter 已接入 semantic、resolution 和 pair 三种受限输出合同；候选召回使用结构化 PostgreSQL FTS；新 API/UI 已提供受控 policy 保存、ADHD 作品选择与不外发的 StudyRun 创建。旧评论研究 Rust/API/UI/worker 和旧向量设置入口均已从活跃源码移除。隔离 PostgreSQL proof 已通过；真实模型调用和运行时/3000 发布仍待后续授权。

2026-09-20，Mog 已授权 Issue #316 在 `codex/comment-study-p3-quality-gate-010` 专属 worktree 推进 P3 质量闸门与本地整合准备。候选 `feat/comment-study-p3-vectors` 已从 `129c9e3` 本地 rebase 至 `origin/main@8348096`，只裁定并保留了 `docs/progress/2026-09.md` 的双方记录；这不是 PR、merge 或最新基线 PostgreSQL 证明。随后在该 worktree 仅导入 P3 的十个提交，并以隔离 PostgreSQL fixture 修复 revision 读写、Problem core embedding 待办、旧 lexical candidate recall、probe 错误类别及 OCR→StudyContextSnapshot 资格断裂；前四项已通过双向复核，OCR 项已通过 source 定点复核，当前隔离 proof 为 41/41。#316 已制定 Gold Set/Recall@K、OCR 语境契约和未比较状态的无真实数据验证协议；其中 `retrieval_incomplete` 与 `budget_stopped` 现以中文保留在既有待归并读取面，API 静态测试为 243 passed、21 ignored（264 total）。现已在共享库只读范围冻结 100 条 FileVault 私有样本，并完成一次本地 WeMM MPS 预检；真实 P3 质量验证暂待 Mog 指定独立人工标注者，不能以 agent/模型自标代替 Gold Set。

**2026-09-21 发布收口授权**：Mog 已明确授权审核本评论研究分支、提交推送、合并和刷新本机上线环境，并由 Mog 手动验收。发布前审核新增修复了 current revision 读取与 provider I/O 前 claim 永久卡住两项缺陷，中文 UI/提示词也已收束；相应隔离 PostgreSQL proof 已通过。一次三条私有评论的 provider 回放发现并修正 outcome/reason 合同遗漏，但后续回放未在工具会话时限内形成可用闭环。因此真实语义模型成功、P3 Recall@K 和业务验收仍是 `NOT VERIFIED`，发布不应被表述为 P3 出口通过。

**2026-09-21 作者声音污染处置（候选修复，未合并／未部署）**：Mog 在发布后的首次真实运行 `21d1f1e7-674e-4ec3-992c-1020b7ba589b` 发现作品作者回复被列为评论研究目标。审计确认 source gate 只检查领域和可读性，未将评论作者与既有 `linggan_material_content_author` 的稳定作品作者身份比较；100 个目标中 40 个来自作品作者，67 条 Signal 中 20 条来自该身份。Mog 已授权删除该一次运行及其所有评论研究派生结果（100 target、13 batch、86 attempt、67 Signal、14 resolution、5 work、1 run），原始 Evidence 与通用模型调用账本保留。候选改为只接纳双方身份已知且不同的评论；作者根评论／回复仍可作为父语境，身份缺失 fail closed。下一次真实运行必须在该修复合并、部署后重新创建并由 Mog 验收。

**2026-09-21 COMMENT-STUDY-PAIR-BOUNDS-001（候选实施中，未合并／未部署）**：已发布的干净运行曾令 13 条 `deferred_novel` Signal 产生接近完整两两组合的自动 Pair；这证明后台自动搜索需要止损，但不证明候选质量或任何阈值。当前候选将后续自动流程收束为“每个 Signal 仅一次首个合格候选比较”，有效未建题仍保留 `deferred_novel`；新 Pair 记录实际 profile／候选位次和明确结论代码，并在 `/corpus/comments` 用中文解释。已有 Pair 不删除、不重算、不回填历史排名或理由。本地隔离 PostgreSQL 已验证每个 Signal 的自动配对度最多为 1、有效非同题不建 Problem、读取投影不暴露原始 provider 输出。真实 provider、Recall@K、历史 Pair 细分类、PR、合并、runtime/3000 和 Mog 业务验收均仍 `NOT VERIFIED`。

**下列 COMMENT-RESEARCH-V1/V2 段落仅记录历史运行和设计沿革，不再定义当前评论研究实现、运行或验收范围。**

### COMMENT-RESEARCH-V1-REAL-CLOSURE-001 / Issue #254（历史运行记录；已被 COMMENT-STUDY-REBUILD-001 取代）

Mog 已授权只重置评论研究派生层：Run、RunItem、Atom、向量空间/向量、Problem/Definition/Membership/Resolution、ResultRevision 和变化投影可以清空；Raw Comment、作品/父评论上下文、作者归属、来源限制、模型连接/配置、WeMM profile、active policy 与通用 ModelInvocation 账本必须保留。本项新增同一研究指纹下的有效结论复用、取消项恢复和局部 Atom 接纳，避免历史 Run 存在即永久阻断新研究。2026-09-14 的输入合同核查确认：父评论曾只以 ref/hash 进入审计 manifest、未进入模型语义输入；P0 已将清洗后的可读父正文冻结为一层 `parentResearchText`，并把父评论应存在却不可读的情况以不调用模型的 `context_insufficient_parent_unavailable` 保留，不能压成 `no_signal`。PR #277 已合并为 `af2ad164f8fb01de2e3b13abc241bfa0a4c5d221`；受控安装已完成 worker drain、v2 派生预热与三个服务刷新，v1/v2 均为 1,643 条 Derivation、1,248 条可读普通用户原声，`:3000/health` READY、原声 API total=1,248、运行记录页 HTTP 200。预热不保存 policy、不建 Run、不调用 provider；连续自动排程仍关闭。作品级全文/摘要尚无稳定且用途合格的合同，未纳入本次实现；受控首轮 Run、published ResultRevision、浏览器人工验收与 Mog 业务判断仍分别待证。

2026-09-14 已建立 Issue #279 / `codex/aud-code-quality-20260914` 专属 worktree，用于修复代码质量审查确认的执行所有权、历史详情可达、退役材料读取、Targets 问题入口与静态检查信号。计划为 [`plans/active/aud-code-quality-20260914.md`](plans/active/aud-code-quality-20260914.md)。该事项不执行共享数据库 migration、runtime 刷新、插件重载、真实采集、部署或合并；上述运行态事实不能从 branch 测试推导。

### COMMENT-RESEARCH-SEMANTIC-OUTPUT-001 / Issue #256（源码实施中；受控首批暴露合同失败）

2026-09-14 已在共享开发库执行仅派生层的实际 reset：删除 1,643 个 Derivation、4 个 Run、1,248 个 RunItem、58 个 Atom、3 个 Problem 与 0 个 ResultRevision；随后再次派生出当前 1,643 条研究输入。Raw Comment 为 3,102，模型调用账本、模型配置、向量 profile 与 active policy 均保留。20 条受控真实首批的 Run `4cfd518c-a1df-4046-88ce-1bc5190fd088` 完成为 `completed_with_failures`，没有发布 ResultRevision：语义提取有 3 次不可解析 JSON 和 1 次合同拒绝，问题归并有 2 次结构不合格。该事实证明当前发布门槛生效，也证明不能把现有结果展示为正式研究版本。Issue #256 只修复 V2 输出合同与安全传输边界，部署后仍只允许新的受控首批；全量和连续自动研究继续关闭。

### COMMENT-RESEARCH-OUTPUT-DIAGNOSTICS-001 / Issue #258、#260、#262（源码实施中；V6 待受控验证）

V4 受控 Run `64a9f4f8-b5d9-43c7-be3e-4214ab0f82fc` 证明候选绑定修复消除了固定 UUID 导致的接纳拒绝；V5 改为由模型逐字复制唯一 evidence 短句、程序映射不可变 source offsets，且不读取或保存模型全文。用户随后以当前父评论输入合同运行的 20 条 Run `7025f656-8ecc-48bc-9bbb-c8b33f66ff22` 仍为 `completed_with_failures`：5 个语义调用为 `semantic_json_schema_rejected`、10 个问题归并调用为 `problem_resolution_json_schema_rejected`。安全账本均显示完整 `direct_json`，因此不是空输出、截断或 Markdown 包装。确定性源码缺口是 provider JSON Schema 仅要求 tagged enum 的根字段、未约束每个分支所需字段和互斥字段；V6 将该 Schema 与 Rust variant 收束，并以新的 extraction/membership rule hash 使旧 V5 policy 在开始新 Run 前明确要求重新保存。未重置派生层、未触发模型、未调整发布阈值；连续排程继续关闭。原始评论、作品上下文、模型/向量配置和通用调用账本不受影响。


### COMMENT-RESEARCH-PUBLISH-001 / Issue #264（已合并、runtime 已切换；真实部分版本已发布）

V5 受控 Run `af79c247-104e-4618-b12c-3b0d1dba6bdb` 已终态：冻结 20 条，14 条提取研究信号、5 条确认无信号、1 条 `semantic_json_schema_rejected`；19 个 problem/need Atom 中 17 个已归入 Problem，另有 2 个 `problem_resolution_json_schema_rejected`。PR #266 合并后，`runtime-main@5dfe70a71fcf60d675b4a835af9b63f55c378243` 已在不新增模型调用的条件下发布 immutable ResultRevision `6dbf52b8-2c09-41f6-8b05-3e5654f37998`：冻结事实为纳入 19 / 20、未纳入 1、问题归并 17 / 19，`publicationCoverage=partial`，终态失败计数为 RunItem 1、Problem Resolution 2。Run 保持 `completed_with_failures`，未纳入评论与未归并 Atom 不进入问题、变化或分母；localhost 概览、用户问题、变化观察、运行记录和独立用户原声读取均可访问，调用账本维持 58 次。当前样本的 19 条都位于比较窗口基线期，当前期分母为 0，故 Issue #267 已使概览与用户问题显示“本版基线样本”而不是当前 0，变化观察仍不可比较；连续自动排程仍关闭。页面已如实暴露出部分问题定义过宽或偏离 ADHD 域，模型语义质量、领域边界、可比趋势和 Mog 业务验收仍待独立确认。

### COMMENT-RESEARCH-CUMULATIVE-STATE-001 / Issue #281（候选源码实施中；不改变统计门槛）

Mog 已明确确认：有效 Problem membership 与单轮 Run 的统计完整度是两件事。任一 `problem`/`need` Atom 只要经过既有输出、候选、定义与 membership 接纳合同，就应即时累计为可读事实；同 Run 的其他未归并或失败 Atom 不能反向隐藏它。ResultRevision 的研究/组织覆盖仍只负责占比、排行、窗口比较与趋势资格，低覆盖 Run 不得被表述为完整分布。本候选将概览与用户问题改读 current membership 的累计绝对计数，并让运行记录明示研究信号、problem/need、已归并、待归并和覆盖率；变化观察继续只读已发布统计版本。2026-09-14 的补充决定已把“启动研究”定义为当前范围内未完成资产的持续处理授权：下一次 Run 会有界带入符合止损条件的历史 resolution backlog，独立记录 execution Run，不复制 Atom、不改写原 Run 冻结窗口；成功后只补足原窗口组织覆盖，续办 Run 不生成 ResultRevision。JSON/Schema 失败仅在模型配置或 membership contract 改变后续办，语义接纳失败最多三次，已知瞬时服务失败可在后续已启动 Run 再试。候选隔离 PostgreSQL/API proof 已通过；尚未执行共享 migration、runtime 切换、真实模型调用或 3000 人工验收。

### COMMENT-RESEARCH-PROBLEM-RESOLUTION-V2 / Issue #285（已合并 main、共享 migration 与 :3000 runtime 已切换；真实模型语义与业务验收待执行）

Mog 已确认把 Problem 从单条 Atom 的标题生成改为可审计的长期归并合同。DEC-0004 规定：模型只能输出有来源的 Problem Frame 与固定语义维度比较；程序验证候选/引用/版本并决定 assigned、deferred、not-user-problem 或 out-of-scope。无候选匹配的单条信号作为可读待归并保留；只有两个独立、eligible、等价的信号与一个受其共同支持的最小定义才可创建 Problem。`5983e16` 经合并提交 `3b8e6a2` 与 runtime migration 入口修复 `af3be61` 进入 `origin/main`。当前 V2 固定 ADHD scope、V7 Frame/closed-world candidate 合同、`deferred_*` 可读面、pair checkpoint、受限 pair worker、catalog guard、账本关联和 lease recovery；同作者、同来源或原文完全重复的信号不能构成新建依据，语义改写 repost 尚无已验证实现。隔离 proof 已确认两条独立 `deferred_novel` Atom 会在一个事务中生成一个稳定 Problem、两条 membership 并推进 catalog revision，并拒绝单 Atom 直建、同作者、缺失 Frame 与域外 Frame；规则内核 61/61、完整 PostgreSQL 42/42、API 3/3 与隔离 API 1/1 均通过，随机容器和卷已清理。2026-09-15 已在共享本机数据库登记 `0084` / `0085` 的仓库 SHA-256；受控 runtime worker drain 后，三个 launchd 服务均在 `runtime-main@af3be61` 重启，`:3000/health` 为数据库 `READY` 且 Problems 页 HTTP 200。该回执不证明真实模型语义准确性、语义 repost 判断、真实模型调用、浏览器人工验收或 Mog 业务验收；连续自动排程仍未因本卡启用。

### COMMENT-RESEARCH-RESET-001 / Issue #213（V1 已合并、开发库已切换、3000 已上线；首轮模型配置与业务验收待执行）

Mog 已决定评论研究仍处开发期：既有研究派生结果可以删除，不做 Task B、P4 与新系统之间的数据迁移。新交付只保留 Raw Comment/Evidence、来源资格、作者事实、清洗定位、调用账本、预算与 lease/recovery 等可独立复用基础；旧 candidate/relation/vector、P4 HDBSCAN/Leiden/adapter/group、旧超级查询和页面读取将全部退役。唯一未来内核由 [COMMENT-RESEARCH-RESET-001](plans/active/comment-research-reset-001.md) 与 [DEC-0003](decisions/0003-comment-research-single-semantic-kernel.md) 裁定；其中当前 Derivation 输入合同以 [COMMENT-RESEARCH-V1-REAL-CLOSURE-001](plans/active/comment-research-v1-real-closure-001.md#31-语义上下文与审计上下文comment-research-context-input-001) 的 `comment-research.derivation.v2` 为准，前者不再规定输入版本。

本轮已完成旧路径依赖图与代码替换。`0063_content_author_attribution` 是作品作者归属唯一投影；`0064`–`0068` 建立 versioned ResearchDerivation、保存策略、冻结 Run、四类失败语义、RunItem/归并 lease、四类 Atom、版本化 embedding、exact cosine Top-K 候选、受限同一/新问题决定、immutable ResultRevision 和独立变化信号。确认作品作者回复才从研究正文移除 `作者 ` 徽标，Raw Comment 不改写；`author_identity_unknown` 不进入研究分母。模型输出必须通过 Unicode span、kind、来源与定义候选校验，才可写入 Atom/Membership；`solution`/`experience` 仍是原声证据。生产 `linggan-comment-worker --execute [--once]` 只推进 V1 的语义、向量、归并与发布步骤，通用 invocation ledger 保存预算、调用、失败与用量；短暂 adapter/provider 问题有界重试，永久不兼容记录到具体 Item，旧 recovery 不能阻断新 Run。

`0069_comment_research_v1_cutover` 在完整历史 migration fixture 上通过：它不用 `CASCADE`，显式删除旧 Task B/P4/daily/replay/recovery 的表、视图、触发器、函数和 identity index（含无 FK 的 `linggan_ci_source_revision`），并清空可能由早期开发分支写入的 V1 policy/run/derivation/Atom/vector/Problem/result 派生行；Raw Comment/Evidence、作品作者归属、来源资格、通用模型连接/配置、embedding 设置和 invocation ledger 保留。`0070_comment_research_v1_derivation_head` 使归属/父回复上下文变化产生新的 immutable derivation：冻结 Run 继续读取其原始可读输入，新 Run 才选择当前 head；任一冻结输入不再当前时，已发布整版结果不可读。旧 API/UI、旧 Python 语义计算 runtime、旧 worker 入口和专属验证脚本均已从候选源码删除；五个 V1 读取端点与页面不再让变化观察复用概览，也不向原声显示“语义向量准备”。isolated PostgreSQL 的 20 项 V1 proof、2 项 API proof、Pi adapter SDK proof、Rust 编译及 API route/page guard 已通过。

Mog 已于 2026-09-10 授权 source merge、共享开发库执行此 terminal migration、localhost runtime 切换和上线测试。PR #217 的 V1 交付已合并，PR #219 补足受控 runtime 在 worker 明确不存在时的 drain 判定；`runtime-main` 与当时的 `origin/main` 同步，`/health` 返回数据库与 schema `READY`，API、巡检 worker 和媒体 worker 均在 loopback 本机服务中运行。共享开发库已记录 `0064_comment_research_kernel` 至 `0070_comment_research_v1_derivation_head`；旧 Task B/P4/daily/replay/recovery relation 不存在，V1 policy/run/Atom/vector/Problem/result 派生计数均为 0，而 Raw Comment 为 3,102、通用 invocation ledger 为 129，说明 terminal reset 没有删除应保留的证据与账本。

浏览器已实际打开 `http://127.0.0.1:3000/corpus/comments?view=overview`：页面标题、五个 V1 tab、空态和“作品作者回复/身份未知不进入用户问题与变化统计”均正确出现，控制台无 error；旧评论研究页面不存在。当前空态不是研究失败：开发期结果已按决定清空，尚未保存策略或手动开始首轮研究。连续排程仍关闭，部署没有创建真实模型 invocation。

**2026-09-12 首轮就绪只读核对与隔离候选**：live `/api/local/comment-research/setup` 已返回默认 `deepseek-v4-flash` 配置、已启用且 qualified 的 `Tencent/WeMM-Embedding-2B` profile，故“没有 embedding”不再是当前现场事实；但 `/api/local/model-settings` 返回 `503 model_database_unavailable`，当前运行中的模型语义资格不能由该读取面确认。精确源码根因为 model settings projection 的外层 alias 与通用 callability SQL 不一致。候选分支 `codex/comment-research-v1-first-run-repair` 已将 V1 研究资格收束为同一个事务内 predicate：精确 config/model/connection version、连接启用和最新成功 `semanticQualified` probe；默认配置保存、策略保存、Run 启动和每次 generation reservation 都复用它。候选还让 JSON 解析、字段/证据合同和 Unicode offset 映射失败分别以安全代码/计数出现在运行读取面，不保存评论或模型原文。该候选尚未提交/合并、未切换 `runtime-main`、未写共享数据库且未调用模型；隔离 PostgreSQL/API proof 与实际页面/POST 回执仍是进入首轮前的必要条件。

### CI-AUTO-004 · 本机发布回执（2026-09-09；历史实现事实）

- Mog授权提交、推送、合并及刷新3000。PR #210已合并，代码revision为 `23bce3833cb3198934afa117fd40f634bda2b83c`；合入main新增采集/证据库修复后Rust单元84、API/worker编译、治理检查通过。
- 已生成0053迁移前的加密备份并校验解密与archive目录；0053–0060八项迁移应用成功，实际台账SHA256全部与仓库文件一致。未修改研究开关或额度。
- 旧worker无drain协议，本次按明确维护授权走一次bootstrap：停止前后running invocation均0，许可为no_inflight_bootstrap，未伪称取得旧worker drain回执。新worker已装入PID绑定drain与90秒退出窗口。
- 首次发布三个PID为API41214、worker41224、media41235，cwd及executable均指向runtime-main，revision23bce38；3000为loopback监听，health database/schema READY、scheduler running。后续文档回执同步仍会更换PID，以现场核验为准。
- runtime-main内CPython3.14.5与带哈希锁定wheel已安装，comment-semantics runtime自检通过；没有用开发工作树venv替代运行依赖。
- Chrome原有3000证据库已刷新，再进入评论研究，真实加载四视图/研究设置/自动研究覆盖；当前范围1613条原声、144篇作品。自动研究仍关闭、revision0；模型调用保持76（成功25、失败51），无running及部署新建调用。页面加载证明不等于真实语义准确率。
- 原声10万合成样本P95591ms未过500ms的已知验收限制仍保留；不以发布替代性能/模型质量/Mog业务验收。
- 合并后的交付工作树已移除，合成容量报告按登记路径保留在canonical的artifacts/private/comment-semantics。源码在main；本回执替代先前“未提交/未迁移/3000未更新”的阶段描述。

### CI-AUTO-004 · 评论自动研究（代码与功能验证完成，性能部分未达标，未发布）

Mog 曾要求完整落实《重构评论研究流程》最后决策，由 subagent 实施、root 先审查并落盘完整计划。基线 `829fe22`，独立 `codex/comment-auto-research-v1` / `.worktrees/comment-auto-research-v1`。当前已读取24节及7场景、完成运行/语义/UI三路源码审查；其历史入口为[主计划](archive/ci-auto-004-comment-auto-research.md)、[执行合同](archive/ci-auto-004-execution-contracts.md)、[UI清单](design/changes/ci-auto-004-ui-change-manifest.md)，未来架构由 COMMENT-RESEARCH-RESET-001 规定。

P0–P5代码已实现：自动资格/日预算、不可变规则、成对回放与健康回退、原子聚类、统一设置和双轴状态已接入。Rust单元84、SDK23、UI45、评论任务PG45、3000新增准入1、回放6及聚类/来源/拆并专项1通过；来源边界7、定向预检2、旧问题关系7、规则1、drain1与查询功能3通过。实际合成浏览器已核对1440/1280/900/375、跨页20+8、设置取消零写入；所有本包临时服务/数据库已清理。

性能门槛尚未全通过：最新10万合成数据独立测量概览P95 585ms达标，原声591ms未过500ms。共享主机资源干扰已记录，但不将其推定为唯一原因，也不使用早先447ms结果覆盖最新失败。完整证据和下一步性能条件见主计划第8节。源码尚未提交/推送/合并，0053–0060仅在隔离库验证，3000与共享DB未更新；真实模型语义质量和Mog业务验收仍未验证。

### CI-20260907-V1 本机发布回执（2026-09-08）

- Mog本轮授权提交、推送、合并main和刷新3000。代码PR #192已合并为`cce3f60855fae0143452136a9b96daa9ac243e4a`；独立release_review对head`8d35138`给出PASS。集成后评论PG38、模型兼容50（18重叠）、新API3、治理通过。
- 发布时保留main已有0047采集迁移，评论迁移顺延0048并注册local-runtime。共享台账0048 SHA256=`2ed806e50dbc85fa3d6ceaaffcd3041136a303e9532237953355043161d57b88`与文件一致；应用前已创建加密pg_dump并校验archive目录，未重置数据库。
- 首次发布核验：API/worker/media PID为91654/91657/91659，cwd与executable均属于`~/Library/Application Support/Linggan Intelligence/runtime-main`，revision为cce3f60；health数据库/schema READY、scheduler running。后续文档回执提交会同步runtime，PID可能更新，不能把这些点时PID当永久事实。
- 四视角真实GET均成功：1054评论、15作品、1050可研究、0分析、0问题。每次单次HTTP约982–1043ms，不是P95证明，真实规模/并发性能门槛仍未完成。API的高级观察资格false；daily enabled=false；模型调用数发布前后均5，没有新模型外发。
- 已在用户现有Chrome `localhost:3000`标签刷新，确认概览/原声/用户问题/每日观察导航和原声1054条加载。本次解除的是实际3000基本访问验证，不是此前3307阻断的绕过；完整视口/缩放/键盘/写操作、真实语义质量/embedding与Mog验收仍未完成。
- 旧“未提交/未迁移/3000未切换”及0047评论编号是实施阶段历史快照，由本回执替代。此发布不将84项全部验收或T7质量门槛改成通过；Issue #190保持打开承接剩余项。发布证据另见PR #192与Issue #190回执。

### CI-20260907-V1（当前交付分支；编码与隔离验证）

Mog 曾指定 `comment-intelligence-v1` 完整实施包；基线 `origin/main@61e8c35`，实施位于 `codex/comment-intelligence-v1` 的专属 worktree。四视角、统一来源身份与范围、多维语义、增量问题／词频、人工纠正、23:00 日批与预算恢复已接入代码。来源合同、复用映射和具体证明见[历史执行计划](archive/comment-intelligence-v1.md)、[84 项验收](design/acceptance/comment-intelligence-v1-acceptance.md)。高级观察默认不自动发布，真实质量须独立评测。

本轮没有共享迁移、push／merge、3000 切换或真实评论外发。Chrome 对隔离预览返回 `ERR_BLOCKED_BY_CLIENT`，没有以原包截图代替实际视觉验收。下方 COMMENT-DAILY-001 是历史交付时点记录，不能据此推断当前运行版本；本轮代码与运行状态分别记录。

### COMMENT-DAILY-001（交付分支；尚未进入共享运行）

Mog 批准“评论语料与每日研究 V1”并要求原声参考证据库紧凑表格。单代理在 codex/comment-daily-v1、基线 73f5996 实现四视图、异步保守清洗、按作品分包、23:00 日批、预算与显式失败重试，承接下方 #167/#169 历史条目。新 0043 仅在隔离数据库验证，未应用共享库；3000、真实评论外发、merge 和 Mog 业务验收未发生。本次合同及实际证明见 [COMMENT-DAILY-001](plans/active/comment-daily-001.md) 和 [评论验收](design/acceptance/comment-research-001-acceptance.md)。




## 2026-10-17 · 主题概览 B「材料优先」正式候选（#440 / Draft #441）

Mog 选定 B，正式 React 候选保留全部功能/数据口径并调整为左材料右分布，双视图导航回左侧；以 [阶段F计划 §7](plans/active/web-react-001-f-topics.md#7-2026-10-17--mog-选定-b材料优先正式改版) 与 [F修订验收](design/acceptance/web-react-001-f-acceptance.md) 为准。已同步证据库主线24e3ec23，834项前端回归及正式隔离全阶段、最终F10步通过；实际四宽度212张最终截图已核对、源码59项指纹已独立复核。历史5,184像素一致不覆盖B改版；3000仍82d48a72，未合并部署、未恢复研究或真实模型/平台动作。

## 2026-10-13 主题图谱紧凑工作区候选

TOPIC-MAP-COMPACT-382 根据 Mog 最新确认重排 `/topics`：领域旁切换、左树固定、四组真实统计前置、旅程地图/列表与统计口径独立、场景检索局部化。代码在专属分支修改，使用既有 HTTP 与来源资格，不变更研究内核、数据库或全站 token。27 组 JS、10 组多尺寸 Chromium、治理及设计检查通过（合成数据，0 页面异常）；PR #404，未合并、部署或启动研究。详见[变更清单](design/changes/topic-map-compact-382.md)。

## 当前阶段

### ACCOUNT-OBSERVATION-BOOTSTRAP-002 / Issue #218（源码候选；尚未进入共享运行）

新装 Browser Producer 显示“无观察账号”的根因被拆开处理：安装记录与服务端凭据可以已经有效，但运行时未配置账号身份摘要键时，健康接口会明确标示 `identity_key_missing`；认证身份 observation 不会上报，但明确的登录墙、冷却或访问限制仍可上报并阻断后续接活。即使账号身份尚未由人工绑定，也不应因“未观察”或“未绑定”而拒绝第一张任务。源码候选新增 `0065_account_observation_bootstrap.sql`，将账号观察改为按安装追加记录、以数据库生成序列裁决同一时间点的先后、允许无身份的显式负面状态；容量选择仅拒绝明确的登录/限制/冷却、身份变更、账号并发或工位并发。

第一张任务仍由既有 WorkOrder/Lease 串行保护，并只在任务本来访问的页面被动确认账号；不打开、刷新、滚动小红书页面，不额外制造平台访问。运行时与插件会明确呈现 `identity_key_missing`，不泄露任何摘要键或账号身份。Rust 单元、API 单元、插件 262 项和完整隔离 PostgreSQL LOCAL-001 已通过，临时数据库、容器和卷已清理。分支为 `codex/account-observation-bootstrap-002`，基线为 `origin/main@9c84f3506a1ff3ee7a3357daba83130dd5ee438b`；尚未提交、推送、合并、迁移共享库、刷新 3000、重载插件或访问外部平台。

### TARGET-INSPECTOR-PERFORMANCE-001 / Issue #158（已合并 main；本机 3000 已刷新）

Mog 于 2026-09-08 确认推进观察目标列表和右侧检查器收口，并要求由 subagent 分工实施。PR #205 已由 exact head `e067bfbf7f60d80fc45502ac8cc10f686c8c5efe` squash 合并为 `f0ed68dcf94c139be67f70bd76c09623d3c87b7d`：Targets 顶部删除重复 scheduler/patrol/timezone prose，六目标勾选后顶部批量编辑显示数量；1440 宽表压缩为主体字段与操作同时可见，避免 sticky 操作列覆盖内容。检查器收敛为 `概览｜作品｜巡查`，creator 的作品内为 `列表｜表现`。

新增 `TargetInspectorProjection` 在一个 repeatable-read、read-only 事务中分开读取 archive、execution、patrol、coverage 与 required action；queued 不冒充 running，`Known(0)` 不等于 `Unknown`。概览只给一个行动判断，系统可自行继续时不显示按钮。作品表现复用 target-scoped lifecycle facts；当前没有作品内容分类合同，因此不生成主题分布或运营价值结论。

源码、focused/workspace 测试、新 inspector PostgreSQL 16 证明、项目治理和 UI handbook 已通过；1440×900 浏览器实拍及选择、批量入口、三 Tab、作品双视图和关闭焦点返回均已通过。完整 PostgreSQL 脚本仍被未修改的 collection-control 两项基线断言阻断，不能写成全绿。合并后本机 API LaunchAgent 已刷新：live PID `54442` 的 cwd/executable 均来自 `runtime-main@f0ed68d`，`/health` 为 database `READY`、scheduler `running`、loopback-only，`/collection/targets?domain=all` 为 200；现有 Chrome 页面确认 6 个目标、新检查器文案和 0 console issue。没有新 migration，未重启 worker、重载插件、访问外部平台或触发真实采集；Mog 业务验收仍需单独确认。

### COLLECTION-DOSSIER-RELIABILITY-002 / Issue #158（源码修复中；未进入共享运行）

2026-09-08 针对两项实测问题开始独立源码修复：观察目标抽屉与列表曾维护不同的目标字段投影，领域字段加入后抽屉查询失败并把有效目标误报为“当前未知”；一条 `content_detail` 的 `page_read_failed` 则会无限重排同一 WorkOrder，阻塞同批后续作品。修复把目标列表/精确抽屉读取收束到一份静态列合同；详情读取失败则以同一 `WorkOrder + contentExternalId` 计数，两次有界冷却后第三次写入 `blocked` 终态和失败处置，保留失败账本、零 Attempt/Package/Receipt/Evidence，并让后续作品继续领取。档案和任务页以“详情读取受阻”呈现该事实，不写成页面不存在或采集完成。

该事项的新增 `0047_collection_detail_failure_boundary.sql`、Runtime 切换、共享数据库迁移、插件重载、重派历史任务、真实平台访问、真实采集与 merge 均尚未执行或授权。隔离 PostgreSQL 的 focused dispatch proof 已通过；完整发布/共享运行和 Mog 业务验收仍待后续单独授权与证据。

### MODEL-PI-001 / Issue #169（共享基础运行已接通；DeepSeek 测试返修）

#168、#170、#172 已合并。Mog 于 2026-09-06 明确授权后，0039/0040 已应用到本机共享库，3000 模型设置 API 从 503 恢复为 200，评论 worker 已有有效心跳。部署回执见 [PR #172](https://github.com/Himog0921/linggan-intelligence/pull/172)。单页+单供应商弹窗是当前交互，不再采用旧四区常驻布局。

2026-09-07 Mog 的 DeepSeek Responses 测试两次达到 1024 token 上限。当前同包返修明确关闭已知官方 V4 模型的默认思考，落实原纯文本合同，区分模型已响应和评论输出资格；保持预算/超时/历史回执。SDK 11 项、隔离 PostgreSQL 32 项及合成浏览器验证通过，真实供应商修复后复测尚未执行。具体范围见 [MODEL-PI-001 历史合同](archive/model-pi-001.md) 和 [验收记录](design/acceptance/model-pi-001-acceptance.md)。

### COMMENT-RESEARCH-001 / Issue #167（已合并 main；模型执行已由 #169 接入）

Mog 曾授权一个子代理实施、根代理集中审核，先推进 P00 直接依赖与 P01 评论研究，尚不启动后续产品包。实施基线 `be1e6fb83650b376ee22e6fdec07241b8f604614`，分支 `codex/comment-research-001`；自包含合同见[历史合同](archive/comment-research-001.md)。源码接入原声浏览、问题分组、语料资产与服务端已存查询，人工收存不再依赖 AI。资产理由、单集合归属与撤销、查询名称/条件/删除通过追加修订维护，旧请求不能覆盖新版本或复活已撤销项。`0039_comment_research.sql` 已在 2026-09-06 按 Mog 授权应用共享库，见 #172 部署回执。

评论分析有持久队列、租约恢复、最多 60 秒本地等待及严格候选接纳接口，独立 `linggan-comment-worker --queue-only [--once]` 只入队；#170 已为其接入受控 Pi 消费器。真实模型/费用/敏感材料处理许可未收口；没有语义聚类或真实质量证明，P01-C 和全包不能因此标为完成。共享 migration、3000 与评论 worker 已完成 #172 运行更新；真实采集与 Mog 对评论分析质量的验收不由该部署证明。


### COLLECTION-SCHEDULER-SCALE-001（当前源码实施项；尚未进入共享运行态）

Mog 已明确本轮目标不是临时证明一条采集链，而是把已有的自动观察能力收束为能在大量目标、多个工位和多种任务来源下持续运行的统一调度系统。实施位于独立 worktree 的
`codex/collection-scheduler-scale-001`，基于 `origin/main@4864edd466d379dc7c9d0e7000e4e5cb3ec3d111`；冻结合同、边界和验收条件见
[`COLLECTION-SCHEDULER-SCALE-001`](plans/active/collection-scheduler-scale-001.md)。

本包只改变源码和隔离测试：Rule 使用持久稳定相位避免集中到期；所有浏览器工作继续共用 WorkOrder 队列，并按 immediate/scheduled/batch 三个技术 lane、公平领取和批量滚动蓄水推进；工位、账号、平台三级并发及可恢复失败冷却均由 PostgreSQL 事实约束；`/collection/runtime` 只读呈现 lane、平台、Lease 与 Rule 排程事实。历史档案 `UNKNOWN/PARTIAL` 不再作为持续观察门禁，不新增 baseline/readiness/recovery 生命周期。

源码已提交为 `ded6e6f9cf574b8ea846d932cedb1337ab517fa8` 并推送为 PR #162，仍未合并。`0037_collection_scheduler_scale.sql` 仍只在源码中，尚未应用到共享数据库；本轮完整隔离 PostgreSQL harness 已通过 116 个 Rust/PostgreSQL/API proof 与 2 个 Node proof，并清理随机数据库、container 和 volume。`:3000` Runtime、worker revision、插件加载、观察 Rule、真实 WorkOrder、外部平台访问、真实 Attempt/Package/Receipt/Evidence、源码 merge 与 Mog 业务验收均未执行或验证；隔离 proof 不能被写成这些真实运行结果。

### OBSERVATION-TARGET-DOSSIER-UI-001 / Issue #158（独立 UI 实施项）

Mog 已把观察目标重新定义为“持续观察的创作者及其不断生长的作品档案”，并明确要求首层按语料库式宽表回答对象、档案、巡查、变化和下一步；点击 creator 后进入以作品生命周期散点为核心的宽幅档案工作区。当前工作位于 exact `origin/main@bfe5d7e623b70e314cc6697269e291c3d5f5b3ef` 建立的 `codex/observation-target-dossier-ui-001`，Issue #158 已建 Claim；只验收 1440 CSS px 桌面全屏。

“建立档案”的冻结合同是：要求覆盖 200 篇上限的有效 deep-archive 授权，先以 `maximumQuota=200` 扫描目标主页当前可见作品链接并按 stable Work 去重，再由版本化 marker 驱动每批 3 篇的详情、最多 30 条评论、媒体及已授权 OCR/ASR。后续巡查发现的新作品继续进入同一渐进补齐链；生命周期以目录归属空心点、作者确认实心点和最近有效巡查新增外圈表达，互动字段继续读取共享 Work Current。上次巡查只读成功结果时间，不用派出时间冒充。

当前实现不新增表或 migration，不复制 Corpus 的正文、评论身份或媒体详情，也不加入监控价值、复合指标、分位数或滚动中位线。共享数据库、`:3000`、插件、真实平台、部署与 merge 均未获本 Issue 授权；隔离 PostgreSQL、1440 浏览器、workspace 和一次 exact-head 联合审查完成后才可提交 Draft PR，Mog 业务验收仍为 `NOT VERIFIED`。

### COLLECTION-FIVE-PAGE-V4-UI-001 / Issue #149（已由 PR #154 合入 main）

五个 Collection 页面已由 `fc17d70` 合入主线。V4 文件只作为 synthetic 几何、密度与交互对标；共享壳层、事实、权限与状态继续由 Intelligence 合同所有。其 1440 桌面验收、PostgreSQL 与历史复审证据见 `ACC-COLLECTION-FIVE-PAGE-V4-UI-001`；窄屏不在支持范围。该事项不再是当前实施项，后续观察目标重做由 Issue #158 单独承接。

### TOPIC-WORKSPACE-REAL-001 / Issue #112（已部署；待 Mog 业务验收）

Mog 已在 #133 的前端验收后授权推进下一大阶段。Topic 已由 PR #115 合入 `main@fa80d8a`；新的 additive migration 固定为 `0031_topic_workspace.sql`，因为 `0027` 已由统一媒体占用。Topic 只保存暂定 Definition Version、人工 Classification Run、冻结 Material Pack、精确 Work Resource 引用和 Import Receipt；不复制正文/评论/媒体，不形成正式 Release、Claim、趋势或行动。自动 proof、一次隔离浏览器验收和一次最终严格审查均已完成；其中 shared Shell 在 390px 裁断第五个一级入口的真实缺口已改为自动换行网格并复验，Topic 写入也已收束 canonical/idempotency 的并发唯一身份。Mog 随后明确授权 shared migration 与 `:3000` runtime 切换：`0031` 已以 checksum `cd7e6a33c5c9203d2fcddc88d246aaa16b2a9de4bebfda21123978315ee6297b` 入账，API、巡检 worker 与媒体 worker 均从 `linggan-intelligence-origin-main-fa80d8a` snapshot 运行，`/topics` 已返回真实页面。当前共享库尚无 Topic/Definition/Material Pack（均为 0），因此默认 Topic API 如实返回 `not_found`；真实材料导入与 Mog 业务验收仍待单独授权。

AGENT-CANDIDATE-ANALYSIS-001 / #113 只能在 #112 的 exact Material Pack 合同已稳定后读取冻结引用，输出仍限于带引用的候选解释、反例、unknown 与 gap；本卡期间不并发写 Agent runtime。

### 2026-09-02 · 已完成计划与历史 Issue 收口

Mog 已明确：媒体生命周期合同决策完成；`LOCAL-001` 不再作为当前路线图总编号；`PLUGIN-REHOME-001` 和 `GOV-005` 的合并后文档收口完成；旧式页面探针停止，#76 按源码升级完成关闭，#92 不重跑系统切换而以既有分层证据收口。相应计划已归档至 `docs/plans/completed/`，当前工作只由具体的运行、材料、页面、采集与复观测交付包承接。本次状态同步不把真实媒体字节、OCR/ASR、保留/撤回、真实采集、部署或业务验收写成已完成。下文中早于本记录的 `LOCAL-001`、Issue #41、#72、#74 或 #76 “活跃／待收口”表述仅保留历史上下文，已由本段与归档计划取代。

同次清理还处置了旧式合成 F01 / 旧运行发布 / 静态设计 / 远期自主性卡：Issue #3 只保留为 #133 标准详情窗口的历史验收输入，不创建或重用其字面 `0002_scope_001_content_observation.sql`；Issue #4 只描述已退役的 frozen-Work 模型，不以未实现的第三份 migration 继续阻塞当前入口。Issue #64 的 PR #63 特定 loopback 发布已被更晚的 `0.8.28` 已验收运行时快照取代，不重放旧发布。Issue #85 的 Evidence Library 产品规格、静态参考和静态验收资产已完成，运行页由 #90 与当前事实链另行负责。Issue #102 的自主观察控制设计不进入当前路线图；将来如有真实业务需要，必须以新的、当时可验证的 Decision Package 重新提出。以上均不等于修复或证明一条未运行的旧链路。Issue #10 的 `p0-surface-prototype` 仍是当前 ARC-001 未回答的产品问题；Issue #89 仍是量化的有界 N+1 性能债务，均不因本次清理关闭。

### Browser Producer `0.8.28` 当前权威快照（已合并、已对齐运行时、已真实验收）

- 已验收 Browser Producer 运行时代码快照为 `d7e722018f4f4cfa217c9cf5c0cac6fbcdcaacb3`（PR #132 已合并）；当前 `main` 在其上只有后续的文档／治理状态提交，不改变运行时工件或行为。
- 当前发行包为 `plugins/linggan-intelligence-browser/releases/linggan-intelligence-browser-v0.8.28.zip`，SHA-256 为 `015a3de775a55d6ac2be8dac5d6ca833f7d88f3d772d641a73c7bbe91f51184e`。
- 工位 `1` 已通过正式 API 认领 active installation `9179e6cf-3316-493a-abee-2e7eb162f824` / `0.8.28`，旧 `0.8.23` 被取代。
- API、巡检 worker 与媒体 worker 当时从 `linggan-intelligence-origin-main-d7e7220` 快照运行。**该部署模型已于 2026-09-03 废弃**：三个服务现在统一从跟随 `origin/main` 的 `runtime-main` 运行，见 [`runbooks/local-runtime-deployment.md`](runbooks/local-runtime-deployment.md)。此处保留当时的 `/health` 结论（`LINGGAN_BROWSER_PRODUCER_RUNTIME`、`PLUGIN_RUNTIME_002_SCHEMA_READY / READY`、scheduler running），但运行来源的描述不再成立。
- 真实样本“智商131的A娃。”（`contentExternalId=6a2047060000000022027f0e`，Work `b681991d-525a-4332-8fea-e937e7684c15`）在 `0.8.28` 一次有效标准详情执行中接纳详情 1、媒体槽位 6、顶层评论 15、回复 15；详情窗口为 `requested=30 / unique=30 / page=233 / detail_window complete`。
- 同轮 6 个逻辑媒体槽位、7 个字节组件全部 acquired/materialized：封面 1、正文图 3、Live Photo still+motion、作者头像 1。Evidence UI 已显示本地封面、本地头像与单一“作者”字段（2026-09-04 DEC-作者合并）。
- 当前完成证明为 231 项插件测试、Rust workspace、49 项隔离 PostgreSQL proof、production build、content runtime、运行隔离、release verify/reproducibility；production npm audit 为 0。全 dev 依赖的既有 9 项 audit 提示和 Webpack bundle 体积警告继续保留。

尚未被上述完成声明覆盖的边界：

1. 本轮只证明标准详情附带 `30/30` 评论窗口，没有重新深采全部 233 条。`0.8.21` 的 `233/233` 是独立 Attempt 证据，不与本轮数量相加。
2. `comment.image` 已有代码与隔离 PostgreSQL 非空正样本，但本轮真实样本没有非空评论图片，真实层仍为 `NOT_OBSERVED`。
3. Live Photo 原始字节已完整取得；音频提取/ASR 因处理环境失败而保持 `FAILED`，不写成插件媒体缺失。
4. Evidence 详情 lane 仍因 content-detail Coverage `unknown=3` 显示 `PARTIAL`；标题、正文、作者、发布时间、互动、媒体与评论均可读。这是状态语义差异，不是数据丢失。

### PLUGIN-XHS-FINALIZATION-001 演进记录（`0.8.20–0.8.28`，历史过程不覆盖上述当前快照）

0.8.20 已在同一授权作品上真实完成标准详情：详情 1 条、媒体槽位 9 个、顶层评论 16 条和回复
14 条分别被 Linggan 接纳；评论窗口为 `30 / 30 COMPLETE`，页面公开数 713，没有把标准窗口冒充
全部评论。随后全回复 Attempt 从头取得 `396 / 713`，以 `PARTIAL / no_progress` 接纳 20 条顶层和
376 条回复。页面实测位于 `.note-scroller` 当前底部、无结束标记、无剩余回复展开控件；只执行一次
回拉再下滑，顶层评论节点即从 20 增至 30，确认平台无限加载需要底部重触发。

0.8.21 在同一终包内关闭两个断点：无人值守媒体字节由持久 outbox 交给 offscreen 文档完成下载、
哈希、续传和 finalize，避免 MV3 Service Worker 领取后被回收；评论未达页面公开数且当前底部无
结束标记时，最多三次回拉后继续下滑，取得新评论即重置预算。动作仍串行、最低冷却 1.2 秒，并在
每次外部动作前后检查暂停、停止和风险状态。人工媒体下载窗口继续保留。

0.8.21 重载后已改用较小的真实样本“智商131的A娃。”完成全评论正向证明：本次 Attempt 从头
取得页面显示的 233 条唯一评论，其中顶层 70、回复 163；Linggan 回执为
`all_public_comments / complete / 233 of 233`。同次标准详情暴露 SSR 在图片和作者头像水合前已
足以被判为可用，导致正文与互动数被接纳、`media_slots` 却为零。0.8.22 保留 SSR 权威字段，只在
结构化媒体或作者身份字段缺失时使用当前详情 DOM 补齐，详情图片按 URL 去重，并明确读取详情作者
头像和主页身份；不得从评论作者或推荐流误取媒体。0.8.22 真实复验进一步发现 SSR 会用非字符串
占位对象承载尚未水合的媒体字段；0.8.23 因此只有非空字符串 URL 才算“结构化媒体已存在”，
占位对象不能再阻断当前详情 DOM 的真实 URL 补齐。

0.8.23 重载后，同一作品已真实形成并接纳 6 条媒体槽位：正文图 3、封面 1、Live Photo 1、
作者头像 1；详情、标准评论和回复也各自形成 Package。此前整包 `submission_invalid` 的根因不是
页面漏读，而是 Live Photo 同时观察到 still/motion 候选时，Rust 错把尚未取得字节的复合槽位写成
`COMPLETE`，违反数据库只允许观察阶段 `PARTIAL/UNKNOWN` 的约束，导致整个事务回滚。修复后同一
真实包已被 `ACCEPTED`；隔离 PostgreSQL 也固定 `PARTIAL / OBSERVED / OBSERVED` 回归。

0.8.24 进一步统一两类使用者：目标驱动后台媒体任务与页面“采集当前笔记到 Linggan”都会在提交
槽位后立即进入可靠字节队列；旧“人工采集并下载媒体”窗口继续保留，供真人单独操作。真实重载
确认新包 6/6 槽位接纳，作者头像也完成 Materialization；正文图、封面和 Live Photo 未落地的根因
是平台在受信 XHS CDN 上返回 `http://` 候选，而下载安全边界只允许 HTTPS。0.8.25 只把无凭据、
无端口、命中平台白名单的 HTTP 候选规范化为 HTTPS，随后仍执行严格主机与重定向校验；普通 HTTP
和任意第三方域名继续拒绝。0.8.25 真实标准详情新包再次接纳 6 个媒体槽位，并已物化作者头像和
3 张正文图；统一 Work Resource 与 Evidence UI 已显示首图封面回退、作者头像以及单一作者字段。终验同时证明首轮离屏唤醒只处理 2 条普通媒体、Live Photo 仍等待全局工作队列，不能
保证同一标准详情及时带回全部媒体。0.8.26 因此把 Live Photo 拆为 still/motion 两个直接字节单元，
一次离屏唤醒可串行排空最多 12 条有界任务；最终 7 个组件仍须重载后真实验收。PR 合并、main /
runtime exact-head 对齐在结果后分别核对。

0.8.26 重载后的单次干净标准详情又暴露出更底层的回执阻塞：页面完成读取后，提交
`lingganSubmitCapturePackage` 等待 5 秒超时，数据库没有出现新详情、媒体、评论或回复包。根因范围
收敛到新增 offscreen 媒体上下文与 Service Worker 共用 one-shot runtime message 总线的接缝。
0.8.27 将离屏媒体改到 `linggan-media-worker-v1` 命名 Port，页面采集回执只由 Service Worker
接收；可靠队列冷启动窗口统一为 15 秒，标准详情任一 lane 的队列错误会显式回执且不再静默阻断
后续 lane。重载后的唯一标准详情终验已对“智商131的A娃。”接纳新的 `content_detail`、
`media_slots`、`comments`、`replies` 四包：详情 1、媒体逻辑槽位 6、顶层评论 15、回复 15；评论
Coverage 为 `DETAIL_WINDOW / complete / 30 of 30`，页面公开数为 233。6 个槽位的 7 个字节组件
（作者头像、封面、3 张正文图、Live Photo still/motion）均已完成 Download、Blob 与
Materialization；Evidence 实际读取本地封面和作者头像，并以单一作者字段呈现。该 Live Photo
的 OCR/缩略图链已完成；音频抽取与 ASR 因本机音频/Whisper 处理环境失败而明确显示 `FAILED`，不把
无有效音轨或转录伪装成成功。

最终 Spec Review 发现并在 0.8.28 同包关闭两个阻断：标准详情此前没有把已采评论中的图片交给统一
媒体链；回复包入队失败会让外层把已经入队的评论包也报成失败。现在评论图片以评论为主体、作品为
读取上下文进入 `Slot → Candidate → Download → Blob → Materialization → Work Resource`，并返回
`comment.image + subjectExternalId`；comments/replies 回执完全分栏。additive `0030` 只扩展现有
媒体 purpose，不建立第二套资产。0.8.28 的最终自动、隔离 PostgreSQL、发行包、提交、PR 合并与
main/runtime exact-head 对齐按本事项最后一轮完成；0.8.27 已通过的真实详情/评论/媒体链不被重写成
0.8.28 的评论图片非空真实证明。最终门禁为 231 项插件测试、Rust workspace、49 项隔离 PostgreSQL
proof、production build、content runtime、运行隔离、发行校验与可复现重建全部通过；发行 ZIP
SHA-256 为 `015a3de775a55d6ac2be8dac5d6ca833f7d88f3d772d641a73c7bbe91f51184e`。生产依赖审计为
0 项；完整开发依赖树仍有既有 9 项 audit 提示，Webpack 仍有既有 content bundle 体积警告。

## 历史项目快照（仅用于演进追溯）

以下事项保留其各自记录时点的分支、Draft、待部署和未验证结论，不再代表 `2026-09-01 / 0.8.28`
的当前主线、运行时或真实样本状态。发生冲突时，一律以上方“Browser Producer `0.8.28` 当前权威快照”
和对应当前架构合同为准；这里降级的只是各事项旧的实施状态与完成声明，不降级其中引用的 ACCEPTED
决定、领域不变量或仍然有效的合同。后者继续以其各自权威文件为准，这里也不因本轮状态同步而重写历史证据。

### XHS-MEDIA-AUTHOR-EVIDENCE-001（`0.8.19` 历史候选快照，已被上述 `0.8.28` 真实链覆盖）

用户已确认“ADHD的尽头是成瘾”仍在平台推流，页面公开评论数随时间增长属于真实进量，不按重复或错误分母处理。本后续只对该已授权作品重新执行详情、评论和媒体链；最新 Attempt 数量、页面公开数与历史累计唯一评论继续分别表达。

0.8.19 已让详情中同时存在的 `authorId + authorAvatar` 形成独立作者媒体槽位。Rust 以当前作品作为观察上下文校验作者主体，写入 `author.avatar` 权威关系，再复用 Slot → Candidate → Download → Blob → Materialization；头像不进入 OCR/ASR。Work Resource 的 `media.avatar` 只返回 `INLINE_SAFE` 本地句柄。Evidence Library 以单一作者事实区呈现并支持本地头像，媒体 Inspector 连续列出头像、封面、正文图、视频及派生资源。

插件合同与 209 项聚焦测试、Rust workspace test/check/format、JS 语法、隔离 PostgreSQL author-avatar 真链、0.8.19 production build、发行校验、可复现性和运行隔离已通过。发行 ZIP SHA-256 为 `c8410c08cd981e9cad93945515613db77918adea3589794232ed623d5ba70338`。仍存在既有 Webpack content bundle 体积警告和 npm audit 9 项依赖风险。本机持久库 `0029`、`:3000` Runtime、Chrome 0.8.19 重载、目标作品新 Package/Receipt/Materialization 和 Mog 页面验收尚未执行，当前不得写成真实链完成。

### PLUGIN-XHS-FINALIZATION-001（`0.8.16` 历史候选快照，已被上述 `0.8.28` 真实链覆盖）

在 `origin/main@62a60404` 上完成一次有界终包，不再按真实页面每暴露一个症状就增加一套旁路。执行面现在对 `TaskSpec → 页面动作回执 → Attempt/Package → outbox` fail-closed：同一详情 lane 以稳定幂等键复用单一提交，包含并发唤醒冲突回读；页面没有携带精确 `taskId/action/capability` 成功回执时不得报执行成功。暂停、停止、restart 和批量单篇 timeout 都等待当前 collector 退出后才允许下一执行者操作页面；scheduled 不限评论保留 `0 = unlimited`，批量数量允许明确输入 1–50 且不再静默缩成 50。

媒体继续使用既有 Slot → Observation → Download → Blob → Materialization → Derivative 事实链，additive `0027_unified_media_resource.sql` 只补一层七类权威关系和统一 Work Resource `media` 读模型，不建立第二套资产。Evidence 列表、Inspector 与 Collection Target 禁止各自拼远程 URL；封面统一按“显式封面 → 首张正文图 → 视频 poster → 无”选择，并只返回 `INLINE_SAFE` 本地句柄。`requestedLimit` 与页面公开数、当前唯一取得数分开贯穿，避免再次产生 `130/53`。

插件终包门禁 207 项、搜索筛选/下拉联想/探针 13 项聚焦测试、合同检查、Rust workspace 测试/检查、隔离 PostgreSQL 全迁移与 47 项相关 proof、production build、发行校验和可复现重建已通过；新增并发 outbox 与远程头像禁止渲染回归也已通过。`v0.8.16` ZIP 当前 SHA-256 为 `b7b57375a20d83145ceabfea59a7ba71b2c313560d04fef006698ffd2ce8b05f`。Webpack 仍报告既有 bundle 体积警告，`npm audit` 仍报告 9 项既有依赖风险，Rust API 测试仍有既有 dead-code warnings。严格 `cargo clippy -D warnings` 在主线基线与终包分支均被同组 11 项既有 lint 阻断；本卡最初新增的 2 项已拆除，没有把全仓历史重构混入终包。

真实边界没有被自动检查替代：现有运行工位最后上报 0.8.14，0.8.16 尚未被 Chrome 重载；“ADHD的尽头是成瘾”当前只证明跨 Attempt 的部分材料增长，最近一次为 `121/533 PARTIAL / manual_stop`，当前投影为 167 条稳定唯一评论。该作品 8 个媒体槽位仍为 0 个 Materialization，所以 Evidence 无封面仍是当前真实事实。共享持久数据库尚未应用 0027，本机 `:3000` 尚未替换，真实 XHS 全评论、媒体物化、Linggan 接纳和 Mog 业务验收都待精确包重载后的单次验证。

### PLUGIN-XHS-COMMENT-EXECUTION-MEDIA-001（`0.8.8–0.8.15` 历史演进记录）

真实长评论样本“ADHD的尽头是成瘾”已证明标准详情的 30 条评论材料被 Linggan 接纳：当前投影为 16 条顶层评论、14 条回复，详情公开评论数为 452。暂停中的单篇深采在浏览器内已取得 130 条，但旧 0.8.7 只在 collector 返回时提交 Package，因此新增 100 条尚未进入 Intelligence；控制面把首轮 53 锁成总数，形成 `130/53`。同一作品已有 8 个媒体槽位和可访问的 XHS 封面候选，但没有本地 Materialization；媒体工作被领取后只留下 `lease_expired`，所以 Evidence Library 不展示远程 CDN 封面是符合边界的，缺陷在媒体执行没有完成而不是页面漏用远程 URL。

Chrome 已真实重载 0.8.8 并报到；目标页当前公开评论数为 492。首个全回复 Attempt 正确显示 `17/492`，以 `PARTIAL / no_progress` 接纳 10 条顶层和 7 条回复，使当前唯一评论从 30 增至 47；第二个 Attempt 在 `15/492` 人工暂停后，以 `PARTIAL / manual_pause` 接纳 10 条顶层和 5 条回复，使当前唯一评论增至 58。由此已证明错误分母、暂停丢材料和 Intelligence 未收到评论三项修复有效。真实链同时发现不限数量被当成无目标，API 首屏后没有进入 DOM 补采；媒体侧仍未产生封面 Materialization，代码审查进一步发现新领取媒体行可能被历史本地队列和无超时请求阻塞，这与租约持续过期的运行事实一致，但须由 0.8.9 复验确认根因已经关闭。

0.8.9 候选让不限评论任务以页面公开数作为当前集合目标，API 首屏不足时进入共用 DOM 补采器；每个新 Attempt 先删除同页上一轮主评论/楼中楼快照，再从空游标重拉第一页，防止把旧页面缓存冒充本轮重采。新媒体代次优先于历史本地行执行，候选下载 20 秒超时。单动作、1.2 秒最低冷却、暂停/停止边界、真实 PARTIAL 交付和人工媒体窗口保持不变。完整 492 条评论树与封面本地物化仍须在 0.8.9 发布重载后证明，当前不能宣称整卡完成。

0.8.9 重载后的真实页面已确认公开数为 508，但首屏仍在 `18/508 / no_progress` 停止。根因已收敛为终态续采判定只读容器标题和入参，没有使用同一详情上下文已观察的 508。0.8.10 直接补齐该传递断点，不改其他执行策略。

0.8.10 重载后目标页公开数增至 514，仍在 `20/514 / no_progress` 停止，无风控。这证明 514 确实在进度轮次被读到，但虚拟列表终态选中的容器不再带该元数据。0.8.11 因此在 Attempt 内持久任一轮已观察公开数，不再要求终态 DOM 重复提供同一值。

0.8.11 复验仍在 `18/532 / no_progress` 停止，无风控。真实滚动探针确认 `.note-scroller` 为正确容器：从 `scrollTop=560` 移动到 1960 时节点仍为 18，到底加载后节点增至 38，`scrollHeight` 从 2989 增至 5958。0.8.12 因此把滚动位置和高度的真实前进纳入进展判定，不再在尚未到底时累计 no-progress。

0.8.12 重载后的真实两级评论 Attempt 已从旧停点 18 条推进到 `121/533`，滚动到底、无验证码或访问异常，并以 `PARTIAL / no_progress` 形成真实回执，证明自适应滚动与部分材料交付有效。继续验证“全部楼层回复”时，设置弹窗已明确显示选中，但单篇任务控制器只保存 `commentDepthMode`、没有把它传入 collector，运行日志仍显示两级模式的 `1/3` 停止预算，页面保留 27 个未展开入口。0.8.13 只补齐该参数透传及回归测试；完整 `533/533` 仍须重载后从新 Attempt 证明。

0.8.14 将另一会话已合并的 Work Resource Read / Evidence Library 平台改动、0.8.13 评论执行修复和一条用户授权的 XHS 详情时间字段回归收口为同一发行快照。真实探针确认现有 `xhs-detail-time-v2` 映射正确，因此不新增另一套时间解析。197 项插件测试、TypeScript 合同、production build、content runtime / Linggan isolation、release 校验与全新依赖可复现重建已通过，ZIP SHA-256 为 `28acbf796468ba416dcffeea801ba34fea7686c6b532e2bc54913b20d999376a`。浏览器重载和新 Attempt 的全回复真实链验证仍待执行。

0.8.14 重载后工位已真实上报新版本。新的不限数量/全部楼层回复 Attempt 正确重置为 `0/552`，页面从 19 个评论节点加载到 29 个且无验证码或访问受限，但约 70 秒后仍停在 0。根因是首轮 API 获取失败后，`allReplies` 分支在 DOM 评论解析前反复尝试同一展开入口并直接 `continue`。0.8.15 改为首轮 API 失败即转交共享 DOM 采集器，由同一条路径完成解析、去重、1.2 秒动作冷却、回复展开、滚动和部分快照交付。本轮 0.8.14 Attempt 已人工暂停为 `0/552`，不宣称已取得评论。198 项插件测试、TypeScript 合同、production build、content runtime / Linggan isolation、release 校验与全新依赖可复现重建已通过，0.8.15 ZIP SHA-256 为 `37fe88bdcf7cf948eb490cd76454d5d93134b3669d4cb34a856ff8a9bbf38e3b`；浏览器重载后的真实链复验仍需执行。

### WORK-RESOURCE-READ-001 / Issue #110（实现与集成验证完成，部署待完成）

为避免 Intelligence 各页各自拼封面、标题、作者与时间，当前交付分支新增中立 `Work Resource Read` Interface 与 `/api/local/work-resources`；Evidence Library 只是首个消费者。Media V2 继续拥有媒体事实，页面不能绕过共享接口另读 SQL/Package。profile discovery 的监控目标作为 collection context 单独返回。**2026-09-04 DEC-作者合并**：所有表面只显示一个作者字段——详情 `creatorDisplayName` 优先，其次 `targetKind = creator` 且非 `MISMATCH` 的目标显示名，皆无则“当前未知”；关键词目标永不填补作者。采集来源移入 Inspector「来源与溯源」。

详情发布时间链修复了“插件算出 `publishedAt`、服务接纳却只存文本”的断点：additive `0026_work_resource_read.sql` 保存实际字段、source kind、precision、reference 与 parser version，只有合格 platform epoch 晋升为精确 `published_at`；相对/日历文本为 `SOURCE_TEXT_ONLY`。资料库增加 `layout=research|table|cover` 三种只改变排版的视图，继续共享查询、字段资格、当前选择和 Inspector。与评论接纳升级合并后，Rust workspace、插件聚焦测试、发布包校验/可复现性、51 项隔离 PostgreSQL proof、调度序列与完整 local-runtime proof 均通过。2026-08-31 的一条用户授权 XHS 签名详情探针已确认 SSR 顶层 `time` 为 `number` 型 13 位毫秒 epoch，精确命中现有 `xhs-detail-time-v2`；未保存正文、作者资料、评论或媒体。探针不产生 content_detail Package/Receipt，不证明共享投影已收到该真实时间；运行发布与 Mog 最终业务验收仍未由本次探针完成。

Mog 对当前 Evidence Library 首屏指出重复定位文案、左侧 `STATE VIEWS` 用途不清和主内容失焦，并确认参考稿红框内的 `SYSTEM VIEWS / 系统视图`、`MY VIEWS / 我的视图` 是希望保留的设计语言。交付分支已删除重复宣言和第三栏，把五项真实查询预设放入系统视图；我的视图因保存合同未接通，只显示诚实禁用空态。Table 标题/上下文/辅助正文/状态提升到 14/12/11/10px，桌面行高 92px。隔离 `127.0.0.1:3300` 只读实例以本机 13 个作品集合完成 1440×900 与 375×812 的 in-app Browser 几何、截图、computed style 和系统视图实际点击检查；没有改 `:3000`、数据库、媒体根、插件或采集，Mog 对本轮最终视觉仍待确认。

### MATERIAL-DEEPENING-001 / Issue #103（真实 canary 执行中）

当前实施把已发现的固定 12 个作品作为明确 WorkOrder 范围，而不是让普通巡检对全部发现结果自动扩张。新增 additive `0022`–`0024`：固定作品及评论/回复/媒体/OCR/ASR策略、详情互动观察时间线、实况图片 still/motion 组件、有界媒体处理租约和可检索派生文本。发租时每个作品顺序展开 `content_detail → media_slots → comments → replies`，每个 Task 只请求一个能力；媒体和处理重试均最多 3 次。

Browser Producer 已沿真实 canary 升至 `0.8.4` 并进入 `main` 与本机发布：安装、启动或 alarm 唤醒后自动签到与领活，能按作品标识自动打开详情并分别回传详情、媒体、评论、回复 Package；不再依赖人工点击“领取”。服务端派发最新已接纳发现材料中的短期 XHS 签名来源链接，插件验证作品身份与 `xsec_token` 后执行；插件升级后同一工位可原位承接有效租约中的未完成 Task。普通图片、封面、视频与 Live Photo 共用媒体槽位，Live Photo 的 still/motion 候选和取得状态分开。服务端会自动取得短期 URL 的字节并保存在独立本地媒体根；本机媒体 worker 使用 PaddleOCR/FFmpeg/Whisper 形成缩略图、OCR、音频、ASR 和视频抽帧 OCR，空文本按 `KNOWN_EMPTY` 保留。Evidence Library 已接入互动时间线、评论/回复与派生文本检索，并只使用受控本地资产。

共享数据库已应用 additive `0022`–`0024`，API、调度 worker 和媒体 worker 已切到 `origin/main@8fe531e` 冻结快照。Chrome 已加载并认领 0.8.4；首个 `content_detail` Task 自动打开了带 `xsec_token` 的目标详情页，但页面水合后全局 `__INITIAL_STATE__` 已被删除、当前 DOM 又没有旧详情容器，导致详情采集在浏览器内失败，任务保持 `in_progress` 且仍为 0 Attempt / 0 Package，服务端接纳尚未发生。该现象与内容工作台 2.0.93 的真实热修根因一致。0.8.5 候选补回安全 SSR `noteDetailMap` 解析，并让详情就绪、完整度判断、正式单篇采集和启动探针共用这一路径；只解析有界 JSON 对象，不执行页面脚本。123 项 Browser Producer 测试、合同检查、生产构建、发布包校验、可复现性与旧工作台隔离已一次通过；发行 SHA-256 为 `62d17c53e0fd6153e3c28aaaa66351002b1de05a576942fd3f687483534f8676`。数据库、WorkOrder/Task、签名 URL、12 条固定范围和原 canary 均不改变。真实 12 作品穿过详情/评论/媒体/OCR/ASR 与 Evidence Library 的最终验收仍未完成，下一步是 0.8.5 精确 head 合并、发布、重载后继续同一 canary。

### COMMENT-COLLECTION-RECEIPT-001（v0.8.7 已合入，Chrome 实际执行版本仍待切换）

已按 ADR-0002 完成代码级收口：标准详情回执与全量深采回执分开；深采以“页面显示数 = 本次唯一采回数”判定 `COMPLETE`，`200 / 300` 等短采为 `PARTIAL` 但保留 `usable`；新的深采从笔记评论入口重新开始，而非续用旧页码。`0025_comment_current_projection.sql` 使不可变 Attempt 评论历史与按稳定评论 ID 去重的当前检索投影并存；本机 runtime 已切到 `main@b8bf0f0`，持久数据库已备份并应用 `0026_work_resource_read`，API、调度 worker、媒体 worker、health 和 Work Resource 路由均已核对。

Chrome 原已加载 0.8.6，并在一个获准真实 XHS 详情样本读到 3 条顶层评论和 3 条回复；页面显示 6、本 Attempt 唯一采回 6、自然结束，详情与媒体 Package 已接纳。评论材料没有进入语料：旧插件把顶层自指 root 误分为回复、把页面的 parent 与 reply-to 双字段原样出包，且手工任务错误带 `maximumQuota=1`，服务端因此按合同隔离，没有伪装为成功。0.8.7 在出包前规范评论关系，有限任务保留事前配额，自然结束深采使用 `maximumQuota: null` 与人工/时间/风险/自然结束条件，Package 与 TaskSpec 共享同一 target；167 项插件测试、构建、隔离、ZIP 校验与可复现性通过，ZIP SHA-256 为 `4cc80ac66762c82bb09f2e729a42afd4c6c5690d0ad0e03cf892fa7fdfe836b7`，旧 0.8.6 ZIP 仍保持 `4ff1ed8ff05ce380c946edd048c6d569d974b7ec143450b2bf17c67fd1406040`。修复已由 PR #117 合入 `main@ebcac83`，磁盘上的 Chrome 加载目录也与 0.8.7 `dist` 完全一致；但重载后一次真实点击的最新工位报到仍明确上报 `plugin_version=0.8.6`，新 TaskSpec 仍是 `maximumQuota=1`，说明 Chrome 后台 Service Worker 实际没有切到新版本。该次回复包继续按旧合同隔离，不能拿它否定 0.8.7，也不能宣称 0.8.7 已真实通过。**修复后 comments/replies 接纳、Work Resource 评论树、单篇深采、批量评论和业务验收仍未证明**。

### COLLECTION-SURFACE-TRUTH-001（真实状态硬化候选，待合并与发布）

`/collection/targets` 已有 2 个真实目标、常驻 worker 也持续写入 scheduler heartbeat，但已部署页面仍同时显示“采集运行时未接通、调度器未接通、暂无观察目标”；`/collection/runtime` 也继续显示 `SCHEDULER HEARTBEAT UNREADABLE`。根因不是数据库缺事实，而是 DESIGN-009 当年只给执行工位页传入部分容量状态，其余四个子面和语料页一级导航保留了静态占位文案；后来新增的 scheduler heartbeat 从未接回页面。

当前交付分支只复用现有 `count_targets`、`read_runtime_capacity`、`read_station_overview` 与 `read_scheduler_heartbeat`，把目标数、巡检数、建档数、空缺工位、未归位安装和调度状态按字段独立显示；任何一项读不到只让该项变成未知，不连坐其余已知事实。调度明确区分运行、心跳过期和读不到；任务、待处理、观察历史、周期复盘与语义事件读模型尚未接入时，页面无论目标数是多少都明确写“当前未知”，不再伪造空列表或零。Corpus 一级导航同样从真实目标计数显示“观察中 / 无观察目标 / 状态未知”。`cargo test -p linggan-api` 为 59 passed、14 个需隔离 PostgreSQL 的测试按设计 ignored、0 failed；隔离 PostgreSQL proof harness 另有 46 passed、0 failed。候选 API 在 3101 端口连接本机持久数据库只读实测显示 2 个目标、1 个巡检、1 个建档、0 个空缺工位、1 个未归位安装和 scheduler running，且目标页、生产流与 Corpus 不再出现相反状态。**该候选尚未合并，也未替换 3000 端口冻结运行快照；浏览器视觉和 Mog 业务验收仍待发布后完成。**

### OBSERVATION-RUNTIME-001 / Issue #94 与 MEDIA-ACQUISITION-001 / Issue #98（已完成并运行）

观察目标与观察规则现在进入同一条有限运行链：常驻 worker 记录调度心跳，扫描已启用且授权有效的目标，基线建档、持续巡检和关键词发现分别生成有界 WorkOrder/Step；过期租约可恢复，单目标基线最多创建 3 个 WorkOrder，并优先避开最近失败工位。Browser Producer `v0.7.0` 使用 MV3 alarm、安装/启动和 Service Worker 唤醒自动签到并单飞领取，不再要求用户点击“领取任务”；发现封面另走有界媒体工作、SHA-256、分块上传与本地 Materialization，不阻塞已接纳的文本/发现材料。迟到的真实 Package 不再因旧租约丢失而丢弃：它不能推进当前 Step，但可按独立材料接纳结果保留。

Material Projection 已接纳搜索/主页发现面带回的真实标题、作者、封面来源和点赞/评论/收藏/分享读数；Evidence Library 优先展示本地受控资产，在没有本地副本时只允许 `https://*.xhscdn.com` 的来源封面并明确标为“未物化”，不会把来源 URL 伪装成本地媒体。数据库通过 additive `0020_observation_runtime_automation.sql` 增加调度心跳、发现面媒体/互动字段及 Receipt 的执行影响/材料接纳双轴。

观察运行与媒体取得已由 PR #95/#96/#99/#100/#101 合入 `main@fe60b30`；共享数据库在备份后应用 `0020` 与 additive `0021`，API/worker LaunchAgent 已切到同一冻结快照，Chrome 已真实签到 Browser Producer `v0.7.0`。已有创作者目标在没有人工调用领取接口的情况下完成 `author_profile → profile_discovery`：两份 Receipt 均为 `COMPLETED_LIVE_STEP + ACCEPTED`，Lease 正常完成，目标由 `archiving` 转为 `monitoring`。来源实际保留 12 条发现材料；媒体 alarm 自动完成 12 个封面取得工作，形成 12 个 `acquired` Download Attempt、12 个内容寻址 Blob 和 12 个 Materialization。媒体根已固定为独立的 Application Support 目录，不随代码快照切换；12 个物理文件 SHA-256 全部与内容地址一致，12 个受控本地资产 URL 均返回 `200 image/webp`。Evidence Library 浏览器实测渲染 12 个作品集合、12 张 640px 本地封面，并将媒体字节标记为 `ACQUIRED`。来源未提供作品作者名、点赞、评论、收藏和分享数，因此这些字段仍保持 `UNKNOWN`；详情、评论、OCR/ASR 未执行，不能把本次基线说成完整作品建档。

### MATERIAL-PROJECTION-001 / Issue #86（Draft stacked 实现）

基于 `MEDIA-RECON-001`，新的 accepted Package 已有作品级类型化材料投影：发现、详情、评论、回复、作者、媒体槽位、媒体字节状态及 OCR/ASR 生命周期共用一个 `items` 读取 envelope。`/api/local/work-resources` 返回共享作品资源投影；DOMAIN-UNIFICATION-001 的待合并分支已移除旧 `cards` HTTP 兼容入口，避免它跨 Domain 读取。逐字段未知不补值；评论/回复保留稳定身份、根/父关系与各自 Coverage；作者资料按观察版本追加；媒体保留 Producer 顺序与未知展示顺序、多候选来源、generation、Live Photo partial、Blob/本地 Materialization、处理事件/派生和处置状态。普通 API 不返回远程候选 URI、storage key 或临时上传状态，`batch_checkpoint` 不生成材料或整体完成声明。插件到页面的映射见 [`architecture/material-projection-data-map.md`](architecture/material-projection-data-map.md)；运行版本是否已切换仍以 PID 与迁移台账为准。

独立审查后的隔离 PostgreSQL 16 proof 已覆盖 Task/Package/Record/Coverage 的平台、能力与目标绑定，逐 Record 隔离且健康 sibling 不连坐，评论/回复数据库关系约束，媒体 `observationRef` 历史冲突与同包重复隔离，首次同槽位并发 generation，作品级查询，以及固定 `asOf` 的 50 项 keyset cursor。读取会先把 lane/media-kind 存在性下推，再在单次最多扫描 200 个作品的预算内补页；预算触发时返回 `scanLimited + cursor`，后续从最后扫描键继续，不重复扫描已排除对象。这个预算只关闭了无界事务风险，没有关闭 enrichment 的有界 N+1：当前最坏仍可达到每个候选 7 次、单响应最多 1,404 次 enrichment/base 查询，批量化债务由 Issue #89 单独承接。

原件读取不再使用裸 SHA 作为资格：普通投影只返回 `/api/local/media/<materializationRef>/<sha256>`，服务端验证 Materialization 与 Blob 关系，并按 Blob 全局、Slot 和具体 Materialization 三层处置决定可读性；共享 Blob 的另一份合格 Materialization 不会被错误连坐。同一 Slot 有多个副本时选择最新仍合格的 Materialization；最新副本被局部或 Blob 处置后会回退旧健康副本，同时用 limitation 保留“较新副本已处置”事实，Slot 级处置仍阻断整个集合。Derivative 使用独立的 `/api/local/derivative/<derivativeRef>` 受控句柄，`inputScope` 不再冒充位置。两类可撤销资产都返回 `private, no-store, max-age=0` 与 `nosniff`。上传 finalize 在写入时流式核对声明 hash/size 并原子提升到内容寻址路径；GET 重新检查当前治理资格和 size，以可配置上限流式交付，不把每次全量 hash 作为读取前提。未知或新媒体声明类型仍可保存；只有安全类型 inline，其他类型以 attachment 交付并标明 `UNSUPPORTED_MEDIA_TYPE`。接纳拒绝绝对路径、空组件和 `.`/`..`，读取 canonicalize 后拒绝根外路径。404/503 均使用 `local_read` envelope。旧 schema 遇到合法但无法履行的 Material query 返回 503，不伪装 200 空页。这仍不是 OCR provider 或真实平台媒体证明。该 Draft 未回填历史 Package，未访问真实平台、未取得真实平台媒体字节、未运行 OCR/ASR provider、未改 Evidence Library HTML/CSS，也未部署或完成用户验收。

### EVIDENCE-RUNTIME-001 / Issue #90（Draft stacked 实现）

`/corpus/evidence` 已从 server-rendered discovery cards 切换为客户端只读 Work Resource Read：默认只请求 `/api/local/work-resources`，以作品集合为结果行显示 discovery/detail/comments/replies/author/media slots/media bytes/OCR/ASR 九条 lane；选择作品后只沿列表给出的同源 `detailUrl` 更新右侧 Inspector。评论原文只在本机授权详情通道按页读取并显示匿名上下文；普通列表不显示评论原文、平台用户标识或 `contentExternalId`。媒体只在详情同时具备受控同源 `localAssetUrl` 与 `INLINE_SAFE` delivery 时内联，未知类型、受限、已清理或不安全 delivery 不回退 CDN。作品列表和评论通道支持 `nextCursor`；媒体、派生、来源回执若已截断但 API 未给通道 URL，页面明确显示 `SOURCE_INCOMPLETE`，不猜路由或扩大卡 3 合同。

本卡没有修改数据库、接纳、Material Projection 字段/SQL、媒体资格/处置、插件、采集调度或 OCR/ASR provider。它也不证明历史 Package 已回填、真实平台材料/媒体、部署、长期稳定性或 Mog 业务验收；这些边界仍由后续真实垂直证明承担。

### GOV-006 / Issue #82（决策治理收敛）

Mog 已明确取消“任何仓库写入都必须 Issue + Claim + 独立 worktree + Draft PR”的统一门禁。后续按风险分级：代码、数据库、运行合同、插件/release、部署/生产、真实外部动作、敏感数据/权限、不可逆处置和并行冲突继续走受保护交付；当前对话已明确授权的低风险文档勘误、索引/状态同步、进度记录和被取代卡片收口可以直接维护，但仍须核对工作区、检查 diff、运行适用治理检查并分层报告，不能借机扩成实现或自动 push/deploy。

ARC-001 已同步为“已回答问题不再重复提问”：Capture Control Contract 已由 PR #17 合入；媒体卡已从当前主线完成 V2 语义、PR #15 草案、产品规则、插件 `v0.5.0` 通道、Rust/PostgreSQL 实现和 Evidence Library 消费边界的校准，唯一当前入口为 [`architecture/media-lifecycle-contract.md`](architecture/media-lifecycle-contract.md)，决策卡现为 `resolved`。这只证明合同收口；真实媒体字节、OCR/ASR/抽帧/embedding、保留期清理、撤回传播、多材料读模型、页面实现和用户验收仍未证明。第一阶段运行时只剩当前实现缺口基线，首个用户可见范围只剩正式 SCOPE 冻结。Issue #14 已按“媒体合同决策完成”归档关闭；Issue #10 继续只承接其自身当前交付。旧 Issue #11/#16/#18 与 PR #12/#13/#15/#19 已按“已吸收/已被取代”关闭。

`PATROL-LEASE-SEQUENCE-001` 已把一次 creator 基线的 `author_profile → profile_discovery` 固定为同一 lease 下的顺序任务关系：新 lease 不再双写旧 `task_id` 列；派发以数据库原子 claim 标记领取安装与 `in_progress`，同安装重试会取回同一 live task，后一步必须等待前一步形成已接纳回执；内容页允许 `DISCOVER_SURFACE` runtime 消息，并沿后台下发的 scheduled TaskSpec 与 `linggan_dispatched_task` 来源原样创建 Attempt，不再重建 manual task；Package 接纳时再次锁定 live lease 与领取安装，并把 Package、Receipt、task/必要的 lease completion 放进同一事务。只有全部步骤完成才释放 lease 并记录本轮 patrol 成功。持续博主巡检只执行 `profile_discovery`，关键词目标执行 `discovery_search`；手动页面采集仍走 manual TaskSpec。自动领取与发布版本由 `OBSERVATION-RUNTIME-001` 统一升级至 `v0.6.0`。

Issue #85 / `EVIDENCE-PAGE-002` 已完成多材料 Evidence Library 的产品手册、技术呈现要求和合成静态高保真参考：主对象为“一个稳定来源作品在当前 Linggan 中可核验的材料集合”，并覆盖 discovery、详情、评论/回复、作者、媒体槽位/字节与 OCR/ASR 派生 lane。该设计明确区分部分可用、风险控制、访问受限、处理中、字节已清理、未知、空结果和读取错误，并静态演示一次 Package 的槽位级来源观察组如何包含 declared Bundle、still/motion 组件、逐地址 candidate assertions 以及绑定精确 `candidateRef` 的下载尝试；checkpoint 只进入来源核验，不成为材料卡。Issue #90 已把现行 API 能承担的部分落到运行页；静态参考仍不证明真实平台、媒体、OCR/ASR 或用户验收。

### AUD-XHS-001 / Issue #74（历史受限页面探针，已治理终止）

Mog 曾于 2026-08-26 明确授权在已连接 Chrome 中执行最小小红书页面探针，用于回答当前搜索上下文/下拉联想、搜索列表、笔记详情（详情、媒体与当前顺序下最多 30 条评论）和作者页究竟有哪些可见字段、插件能否交付、何时停止。该授权严格限于匿名化的字段存在性、数量、页面状态、顺序和缺口记录：不向 Linggan 提交或持久化真实笔记、评论、作者资料、媒体或原始页面/API 输出；不读取 Cookie、账号秘密或隐藏账户资料；不下载媒体、不调用 OCR/ASR、不绕过验证码或安全限制。它只是对 `DEV-03` 的受限预检，不表示 DEV-03、真实 Evidence 接入、插件发布、Canary、平台兼容、分析、部署或用户验收已经通过。2026-09-02，Mog 决定停止继续此类旧式探针；既有登记中的 `UNKNOWN`、`PARTIAL` 与 `SOURCE_INCOMPLETE` 保留为未知，不以推断填补。唯一登记入口继续是 [`platforms/xiaohongshu/capture-capability-registry.md`](platforms/xiaohongshu/capture-capability-registry.md)，历史执行计划已归档为 [`AUD-XHS-001`](plans/completed/aud-xhs-001-xiaohongshu-capture-probe-refresh.md)。

首轮实测已确认搜索筛选、下拉结构、搜索卡片和作者页 DOM 的一部分当前事实，但同时暴露详情面漂移：当前详情使用 `.note-detail-mask / .note-container / .note-content / .comments-el`，本次页面未出现旧 `noteDetailMap`。Issue #76 的源码升级（PR #77）已实现搜索页面事实回执、详情 DOM 回退和统一的“详情 + 媒体观察 + 最多 30 条评论”逻辑结果；其中 `30` 是标准详情窗口硬上限，空状态、部分结果和停止原因分开记录。#76 以源码层完成归档，不重跑历史系统切换；真实兼容性、运行时、实际接纳与媒体链仍由 #103、#133 或具体 Bug 承接。Issue #78 `PLUGIN-XHS-ACTIVE-COLLECTION-001` 已获 Mog 授权，负责把搜索/主页发现、单篇评论深采和批量评论升级为目标驱动、主动加载、可暂停恢复的执行能力：`30` 不限制独立深采或批量评论，但每项任务必须保留目标、实际、停止原因和恢复状态。

Issue #80 `PLUGIN-XHS-ADAPTIVE-SCROLL-AND-DETAIL-RECEIPT-001` 已经由 PR #81 合并到 `main`：它把“采集当前笔记到 Linggan”与“人工采集并下载媒体”明确拆分，前者不得弹下载窗口，后者保留旧的人工选择和下载；详情回执按笔记详情、媒体观察、评论与回复三条 lane 如实显示。页面加载收敛为有限、可解释的步骤，`no_progress` 不伪装成页面结束；安全验证或访问受限会结束当前 Attempt。此前在一次明确确认下提交的最小详情样本，仅证明一条当前笔记记录进入 Linggan 当前投影；未下载媒体，未证明三条 lane 各自接纳。**真实新版本安装、不同筛选、非空评论/楼中楼、实际 lane 接纳、部署和用户验收仍为 `SOURCE_INCOMPLETE` / 未验证**；代码检查和合并不替代这些真实链路结论。

### PLUGIN-RUNTIME-001 / Issue #50（执行中）

用户已明确拒绝“按搜索、详情、评论、媒体逐张卡重新开放”的路线，改为将完整灵感爆爆爆浏览器插件一次性 retrofit 为 Linggan Intelligence 自有 Browser Producer Runtime。实现必须保留成熟 XHS/抖音页面采集、批量、恢复和原交互，只切断内容工作台运行时；Linggan 负责 TaskSpec、接入、Coverage、receipt、媒体真相与本地读取边界。媒体不再是后续附录：URL 只是 Observation，UI 只能使用 Linggan 本地 asset URL，OCR/ASR 为独立异步派生队列。2026-08-26 的同 Issue #50 / Draft PR #51 最终修订已形成可审查 head：合成“搜索前 20 条”可经 shared package、loopback ingress 到 Evidence Library；无类型接纳的 raw/quarantine 记录不显示为 Evidence 卡片；媒体保留独立上传/失败/派生队列血缘；Popup 已删除旧授权、工位、Cookie 和账号管理处理，并且暂停/继续/停止只在页面控制器实际确认状态转换后显示成功；无可控任务返回 `no_active_task` 且不修改可见进度。隔离且自动清理的 PostgreSQL proof 已通过，但未发生真实平台、账号、媒体、OCR/ASR 或用户验收。**但 v0.4.0 已被用户实际点击验证为工具栏 popup 空白，不能用于 REAL-CANARY-001；Issue #53 正在以 v0.4.2 只修复 popup 启动与诚实 fallback，未合并、未重新加载、未进行真实平台访问。** 本机持久运行库迁移仍受 `.env` 与 Docker 应用角色密码不一致阻塞，未自行运行要求明确授权的密码修复。

### LOCAL-001D / Issue #62（Draft PR 前实现）

REAL-CANARY #52 的聚合结果已确认：本地库存在已接纳的当前可见 discovery 记录，但其来源发布时间均未知；此前 Evidence Library 在 URL 未带窗口时仍隐式使用 `PUBLISHED:30D`，因此会把这些有效材料全部隐藏。Issue #62 只修复这一个读取/表达缺口：缺省 URL 改用显式 `latest_accepted_discovery` 视角，已知与 `PUBLISHED_AT UNKNOWN` 的已接纳卡片都可显示，且不以首次发现、观察、接收或重放时间替代发布时间；显式 7/30 天发布时间窗口继续严格排除未知值并报告排除数。该事项不改真实 Canary、插件、ingress、schema、媒体或分析；当前仍须完成隔离 proof、独立审查、合并后本机 release 和 Mog 验收，不能把代码改变说成真实链路已修复。

项目已完成 `DISC-001` 的产品定义、领域语言、不变量与责任边界基线；这不等于第一版产品形态、准确页面集合或完整前端/后端/数据库/插件/媒体/Agent 运行架构已经收口。`SCOPE-001` 的合成切片范围和实施授权已经确认；代码前语义冻结基线已经在 commit `9801fdf5deb55e1b3fc5b8ac2c43234be295a42d` 推送到本地记录的 `origin/main`。语义冻结随后经过四轮独立只读攻击，历史结论均如实保留为当时的 G1–G4 FAIL/G5 PASS；第四轮新增的 payload owner、pre-routing audit union、动态 snapshot Oracle 与跨 Attempt Satisfaction 四项 P1 已完成最小收口。用户已明确要求停止重复复核循环并尽快进入代码阶段，因此 SCOPE-001 代码门现为 **CONTROLLED OPEN FOR TDD**。2026-08-21 用户进一步明确确认 F01 采用“可运行、可查询的合成主链优先”：F01 只继续到 synthetic Package → PostgreSQL atomic ingress → two Records → Observation/Current → loopback API → minimal CLI，完成后 hard stop。F01 已完成 Package hash、Record hash、duplicate-key、unsafe-integer、unpaired-surrogate 与 RFC 8785 JCS golden tracer，并已有仅含 F01 的手工静态 manifest Oracle；其余 canonicalization 负例仍未开始且不阻塞 F01 主链。`ARC-001` 已建立为并行的产品与系统架构决策图；其旧版“ARC 未收口前一律不得 Web/插件/真实 producer/媒体”的总禁令，已由 Mog 在 2026-08-24 的 `LOCAL-001` 取代其中关于**独立本地 Linggan**的部分：本地 Web、插件 Local 模式、local data presentation 与 preflight 后的受控 Canary 可分别申请和验证。SCOPE-001 仍保持 synthetic-only；F02–F10、生产、旧系统迁移、Agent crew、外部/线上 host、无界真实采集和无合同敏感材料处理仍未获授权。真实平台仍必须先通过 Local host、插件实际加载版本、localhost contract、隐私/日志、Coverage 与对应 Media/OCR/ASR 处理合同，不因 Local-001 或搜索前 20 条而自动开放。

[`plans/active/arc-001-architecture-closure-decision-map.md`](plans/active/arc-001-architecture-closure-decision-map.md) 的 `product-shell`、`primary-daily-job` 与 `first-producer-canary` 已 resolved：Linggan Intelligence 是独立且完整的 Web 产品；每天先让 Mog 处理少量值得关注事项，理解来源、出现原因与不确定性，再决定暂不处理、继续观察、进入 Topic 深入或提出行动；首批 Canary 必须连续覆盖发现面、受控详情/媒体 acquisition 与异步 OCR/ASR，而不能让 discovery 冒充媒体完成。内容工作台只是历史原型和能力样本，不是运行时依赖；首页五个名称是产品职责，不是五个已授权运行实例。这里确认的是产品身份和日常用户任务，不表示 Web、P0 或 Agent 已获实现授权。ARC-001 当前已解锁但未回答的页面形态票是 `p0-surface-prototype`。

`SCOPE-001 / F01` 仍是当前进行中的合成事实内核实施；它不因本地页面存在而扩大。`LOCAL-001A` 已经独立审查、合并到 `main`：本机已有一个仅绑定 loopback 的 Rust host，根入口临时跳转到 `/corpus/evidence`，并存在一个诚实空态的 Evidence Library 页面及唯一运行时 Token 值源。它只证明本地入口和页面代码，不证明 Materials read model、真实数据、Evidence/Observation/Capture、数据库、插件/平台、媒体、OCR/ASR、Agent、部署或 Mog/业务验收。`LOCAL-001 / 001B-001C1` 已在 GitHub [Issue #34](https://github.com/Himog0921/linggan-intelligence/issues/34) 的独立 branch/worktree 与 Draft [PR #36](https://github.com/Himog0921/linggan-intelligence/pull/36) 实现受控 discovery 接纳/本地读投影，并在随机、一次性的隔离 PostgreSQL proof database/container/volume 中通过 admission/replay/append-only、已知发布时间的闭区间 WINDOW（未知与未来发布时间均不冒充最近）及 loopback ingress → storage → Evidence Library API/页面 E2E 验证；当前仍未合并，且这不等于真实插件或真实平台完成。001C-2/001C-3 仍未开始，不能从该 Draft 自动开启。Mog 已另行确认 `DESIGN-002`：以「任务启动困难」为例制作一个明确标为 `SYNTHETIC / NOT LIVE` 的 Topic Intelligence Reference Page，并把 [LIDS v2.0](design/lids/README.md) 纳入全项目唯一设计表达标准。LIDS 现在约束未来 UI 的 Token → Primitive → Component → Pattern → Page、L1/L2/L3、状态诚实与 Agent 工作方式，但成熟度仍为 Proposed；LOCAL-001A 的受限 loopback 页面与运行时 Token 源不表示已有通用组件、真实数据、L2/L3、3D/技术栈、权限或动作。DESIGN-002 仍只是 `p0-surface-prototype` 的一个可审查输入，不回答完整 P0，也不扩大 F01 或任何真实数据/权限/行动授权。`PLUGIN-REHOME-001` 的源码隔离与自有 release 基线已由 [PR #42](https://github.com/Himog0921/linggan-intelligence/pull/42) 合并到 `main`（merge commit `3ea210dd2bd426750a30c510e233564a9c35a6f9`）；其 Issue #41 的正式文档收口仍由独立事项处理。它只证明源码归属、build/release 与旧工作台运行切断，不证明真实采集、媒体、OCR/ASR 或页面数据互通。其后 `PLUGIN-RETROFIT-LOCAL-TRUSTED-001` 已由 [PR #46](https://github.com/Himog0921/linggan-intelligence/pull/46) 合并到 `main`（merge commit `123311c8dd98aa5c449b4ea508468a074a88db75`）：在测试期为**合成 manual Discovery**建立 `TaskSpec → Attempt → durable browser outbox → loopback receipt`，保留 partial Coverage、replay 与 attempt terminal conflict 的独立语义；`scheduler` 仍为 `NOT_CONNECTED`。它不证明真实浏览器/UI 触发、XHS/Douyin、账号/Cookie、详情/评论/媒体、OCR/ASR、Evidence Library 真实数据、研究/AI、部署或 Mog/业务验收。Issue #43 在本计划的 Draft 文档审查、合并后重新核验与分层记录完成前保持 OPEN，不能由 PR 合并自动关闭。`GOV-003` 是并行进行的低风险单 Issue/单 PR 治理工作单，不是第二个业务 SCOPE：GitHub [Issue #1](https://github.com/Himog0921/linggan-intelligence/issues/1) 与 draft [PR #2](https://github.com/Himog0921/linggan-intelligence/pull/2) 已建立，但 independent review 对旧 head `a7b63c365c7d3befccc54fd42586c48fe0f22542` 的结论为 **FAIL**，当前只允许在同一 branch/worktree 内修订并请求原 reviewer 复核；不得 merge、close 或把 GOV-003 记为完成。

全项目已正式按“开发阶段”跟踪；阶段路线、每阶段验收证据与需要 Mog 决定的关口统一见 [`development-stage-tracker.md`](development-stage-tracker.md)。本文继续只维护当前快照和唯一下一步，不复制长期路线。

已确认事实：

- GOV-001 已在 commit `73a6dd928a3be01d642efa9aa4c6c5e293c944cf` 推送至 `origin/main`。
- 代码前语义冻结基线 `9801fdf5deb55e1b3fc5b8ac2c43234be295a42d` 之后的 6 个提交（截至 `64a2141`），以及 F01 ingress 实现 `a45cc61` 和证明记录 `d88fd87`，均已使用 GitHub noreply 身份推送至 `origin/main`。启动 GOV-003 前重新核实 root checkout 为 clean `main @ d88fd8715051aa15da9eeb6860770a03eaad67f6`；这只证明该时点 Git 同步状态，不证明 Issue → worktree → PR → independent review 协作链已跑通。GOV-003 只有在对应 PR 合并并重新核验后才进入当前完成记录。
- 当前机器已有 Git、Rust 和 Cargo。
- Docker PostgreSQL 16.14 已完成真实验证；日常是否正在运行以 `./scripts/dev-db.sh status` 为准。
- ENV-001 采用本机 Rust + Docker PostgreSQL 16，容器内 `psql` 与 `pg_restore` 已通过真实验证。
- ENV-001 验收时，开发库 `linggan_intelligence_dev` 有 0 张业务表，临时 proof 数据库已确认删除；日后需要声称“当前仍为 0”时必须在 Docker 可用状态下重新查询，不能把历史验收当实时状态。
- F01 已完成最小 PostgreSQL foundation 的真实 RED→GREEN：独立一次性 PostgreSQL 16 container 与 test-only volume 内的随机 proof database 从零应用 `0001_scope_001_capture_evidence.sql`，预置一个 Work、一个 Attempt 和两个冻结 target，接受最小 Delivery/Package/Record/target result/Coverage/processing work 关系，并拒绝跨父对象 Delivery；成功或失败均核验删除该 proof database、container 和 volume，未复用开发 PostgreSQL。该 foundation 结果本身不证明 ingress 原子回滚、运行角色权限、worker、Source/Content/Observation/Current、API/CLI 或完整 F01。
- F01 的 Package ingress 已完成真实 RED→GREEN：`crates/evidence` 的接入按判定顺序锁定并核对 Work/Attempt/Capture 同属、验证 epoch/固定合同/冻结 target/authority，然后在一个事务内写入 accepted delivery、Package、两条 Record、target result、Coverage 与两条 processing work 并冻结 Attempt 终态。5 项真实 PostgreSQL 16 测试（各自独立 schema）证明：accepted 行数逐表对齐 `manifest.json`、六个事务故障注入点零半写、同 hash replay 复用原 acceptedReceiptRef、不同 hash conflict 不覆盖既有 package_hash、绕过 Rust 的串父 SQL 仍被拒绝。该结果不证明 Record processing、Observation/Current、worker、API、CLI、F02–F10、运行角色权限负例或任何真实 producer。
- GOV-001 文件治理基线已通过验证并获用户确认，交付见 [`plans/completed/gov-001-project-file-governance.md`](plans/completed/gov-001-project-file-governance.md)。
- GOV-002 已确认使用私有仓库 GitHub Issues、默认五类 triage 角色和单一领域上下文适配；工程技能读取现有 `docs/context/domain-language.md`、`docs/product/domain-invariants.md` 与 `docs/decisions/`，不建立第二套 `CONTEXT.md` 或 `docs/adr/`。
- DISC-001 已确认必须覆盖完整产品，而非只讨论情报核心：语料库、主题词表、主题地图、市场洞察、选题库和外部 Agent CLI 均已进入设计范围。
- DISC-001 已确认 Linggan Intelligence 的稳定核心定义是面向垂直领域的持续情报研究系统；语料、主题地图、市场洞察、内容增长和 Agent 调用是共同内核的应用出口，不能因其中一个应用深入而重新定义项目内核。
- DISC-001 在 2026-08 确认了首个领域边界：当时一级 Domain 为 ADHD，ADHD 数学、家庭干预等为 Topic。该单 Domain 范围已由 2026-09-23 的 [DEC-0008](decisions/0008-peer-domains-and-unified-material-pipeline.md) 修订；当前 Domain 与 Topic 定义见 [`context/domain-language.md`](context/domain-language.md)。
- DISC-001 已确认第一阶段首要用户是产品负责人本人及其小团队，围绕 ADHD 持续进行市场研究、内容判断和选题决策；外部客户与多租户产品能力不进入首期。
- DISC-001 已确认两种产品发动方式：持续观察是默认日常主循环，主动研究由人的问题或经人确认的候选变化触发；两者使用同一资产底座。目标全量必须由实际 Coverage 证明。
- DISC-001 已确认五类长期积累视角：世界事实、语料与表达、领域知识、市场与情报认知、行动与学习。它们用于盘点长期价值，不等于五种平级真相、五张表或五个应用模块；Outcome 只校准未来，不改写过去事实或伪造因果。
- DISC-001 Gate 1 已完成：成功分为观察、资产、认知、决策与学习四层，不要求每次使用都产生 Intelligence 或 Outcome；任务完成、数据量、聚类、评分、报告或 AI 输出均不能单独证明成功。
- DISC-001 Gate 2 已确认六项子决定：受控渐进式采集责任链；有限资源下的统一采集准入与插件执行边界；持续观察中“比较性观察负责测量变化、探索与调查观察负责理解和发现”的解释纪律；主动研究的问题驱动有限闭环；以 Topic 为共同探索入口的多路径资产流转与市场理解边界；真相与派生责任边界。普通采集仍必须满足最低 Evidence 来源与验证底线。
- DISC-001 已建立 [`product/domain-invariants.md`](product/domain-invariants.md) 作为后续 Gate 和实现阶段的长期对抗性验收基线；它冻结系统不能越过的认识、历史、权限和因果边界，不预设物理模型。
- 中期对抗审查已核实：现役小红书 producer 会把相对时间按采集时刻和近似规则推算为 `publishedAt`，且归一化记录不总能保留实际被解析字段的来源；因此新项目必须把原始时间表达、来源、参照时间、解析版本和精度作为后续 Gate 3–5 的硬问题，不能把推算值当精确平台事实。
- 中期对抗审查已核实：Coverage 目前来自 `slotReports`、计数、cursor/`has_more`、终止和失败等多种 producer 事实，没有一个已证明的通用 Coverage 对象；来源事实与针对具体用途的 Coverage 评估必须分开设计。
- 中期对抗审查已确认：跨账号、跨时间的搜索结果可比性仍为 `SOURCE_INCOMPLETE`；它阻塞“近期趋势”承诺，但不阻塞只证明 Capture → Evidence → Observation 的基础切片。
- 中期对抗审查已修正：执行目标错、来源记录内部身份错、采集包身份/hash 冲突和媒体主体错不是同一种错误；Gate 4 必须按真实 producer/fixture 分别确认。
- DISC-001 Gate 2-6A 已在真相分层压力测试后确认：发生记录、Evidence、Source Object/Observation、Derived Analysis、Domain Definition、Claim、Decision/Action/Outcome Observation/Evaluation 分责；观察意图不复制对象事实；Corpus、Topic Map、页面和 CLI 是选择、视图或服务接口，不是线性终点或第二事实源。对抗基线持续按新确认边界增加案例，不用固定数量充当完成证明。
- DISC-001 `DEC-01` 已确认真实材料的最低隐私与传播边界：受限原始 Evidence 默认只在内部按明确用途保存，工作台默认脱敏，外部 Agent 不批量读取原文；获准删除、撤回、脱敏或访问限制必须传播到 Corpus、索引、embedding/feature、摘要、引用、缓存和 Agent 输出，历史判断标记证据资格变化并重新评估。具体保存期限、算法、角色和法律适用性仍留给 Gate 5–7 与专业审查。
- DISC-001 `DEC-02` 已确认第一阶段每日主任务：先从少量值得关注事项判断是否继续观察、进入 Topic 深入研究或提出行动；Topic 是点击后的核心工作区。每日展示不提升事项证据资格，没有合格事项与观察不完整必须分开表达，不能制造变化填满首页。
- DISC-001 `DEC-03` 已确认第一阶段外部 Agent 只能在明确调用者、目的、数据、时间、输出与资源委托内读取、分析、建议和申请。申请不是授权，授权不是执行成功；改变正式知识、长期观察、真实采集、敏感传播或现实行动必须由人确认，确认后仍经过各自正式责任链。
- DISC-001 `DEC-04` 已确认第一阶段不承诺尚未接通的深度评论、私信、咨询、销售等自动 Outcome 闭环。最低边界是 Decision/Action 可追溯、结果渠道可获得性可见，只记录来源明确的实际结果；未接入、未观察、窗口未结束或来源不合格保持未知。旧 schema 指标字段不证明真实结果链已成立。
- DISC-001 `DEC-05` 已确认第一阶段不承诺市场增长/下降等正式趋势，只展示有时间与实际捕获面边界的近期观察和保留替代解释的候选变化。正式趋势等待 Gate 4 真实可比性实验和 Gate 5 统计资格设计通过后按被证明范围开放；当前 `SOURCE_INCOMPLETE` 不阻塞基础观察切片。
- DISC-001 Gate 3 第一项已确认：搜索/作者页发现与逐篇详情/评论采集分责；发现面 Evidence、发现记录、来源身份解析、Source Object 和详情 Observation 不混用。API 返回与页面实际可见是不同来源陈述；发现 N 条不自动产生 N 次深采。真实字段、保留方式、顺序/终止、Coverage、批量和风险策略进入 Gate 4 producer 探查。
- DISC-001 Gate 3 第二项已确认：采集发生记录、Capture Package、Evidence 接入/接受、Record/Source Object 下游隔离、Observation 和 Derived Analysis 分责。包级接入原子且区分重放与 identity/hash 冲突；只有接纳后才暴露的单条对象归属问题进入下游隔离。无效交付留下安全失败与 Coverage 记录但不自动成为 Evidence 或永久原始档案；缺少稳定身份不创建 `Expression Object` 兜底；正常复观测、来源差异、解析更正、不可访问、来源删除和隐私处置不得混成一次覆盖更新。
- DISC-001 Gate 3 第三项已确认：Observation 只追加；迟到材料保留观察、接收和接纳时间而不倒退 Current；parser/结构化修正属于有血缘的解释修订，不制造新世界 Observation；API/页面来源差异未经审计不静默裁定。Current 是带来源、规则和理由的可重算读取责任，允许未知；执行、观察、接收、接纳、来源时间陈述和派生时间不得互相代替。最终字段、来源优先、持久关系和事务仍进入 Gate 4–5。
- DISC-001 Gate 3-4 已确认正式领域语言：Topic Identity 与 Definition Version 分责；Domain Definition Release 由明确发布决定形成；正式词表与实验分析词表隔离；Term/首选名称/Alias、Knowledge Relation、Classification Run/Human Adjudication 和 Map View 分责；定义发布与重分类/统计就绪分开；改名、边界修订、拆分、合并和导航移动分别治理，旧 Claim、分类与统计不自动继承。Claim 是有范围、时间和依据的重要可争论判断，支持、反例与治理历史不压成一个可覆盖分数。
- DISC-001 Gate 3-B 已确认研究、采集权力与行动反馈边界：持续观察目标、主动 Research Question、Collection Plan 和实际 Run 分责；Information Need 不等于插件命令或任务状态；Acquisition Request、人的 Acquisition Authorization、执行前 Admission 和 producer Work Order 分责，采集授权、材料使用许可和插件设备凭证不得互相借用。Decision 固定确切提案/计划版本；Action Proposal/Plan、Action Attempt/Occurrence、Expected Outcome、Outcome Observation 与 Evaluation 分责；研究可以诚实结束或暂时中止，不创建万能 Research、Proposal、Action 或 Workflow。
- DISC-001 Gate 3-C 已确认应用资产与发布视图边界：Corpus 是同一来源材料上的用途化选择；Signal 与注意力处理、Claim 资格分开；Market Insight/Intelligence Brief 固定当时判断表达但不成为第二 Claim；Content Idea、采用决定、成稿、外部发布行动和 Outcome 分责；临时 Agent 回答不自动成为正式情报。
- DISC-001 Gate 3 已于 2026-08-20 获用户整体确认通过。逐项确认、全部确认版本的全链对抗审查和两处跨模块歧义修正均已完成；Gate 3 只冻结领域含义、责任、不变量和后续证明边界，不等于 producer、PostgreSQL、业务代码、AI Agent 或插件升级已通过。
- 2026-08-20 用户正式确认扩展后的 `USER-DEC-02`，并认可 `USER-DEC-01`、`03`、`04`、`05`、`06`。七道 Gate 因此全部关闭；`DISC-001` 已归档为完成计划，正式 `SCOPE-001` 已建立。
- 2026-08-20 用户进一步批准按独立审查修订后的 `SCOPE-001` 开始实现。授权只覆盖计划明列的合成/脱敏 fixture、两份 migration、Rust 事实内核、loopback API、worker、minimal CLI 和验证脚本，不向后续切片外溢。
- 2026-08-20 用户进一步确认代码前语义冻结结论：SCOPE-001 采用 Closed World，独立状态不得压成总成功，unknown 不得被默认值吞掉，低层不得越权产生高层 Claim，任何验证声明必须携带未证明范围。四轮攻击与已知 P1 收口后，用户又明确终止重复文档复核并要求进入代码阶段；因此当前为受控 TDD 开工，不改变上述语义或真实范围硬停止线。
- 2026-08-21 用户逐题确认第一阶段首页情报面形态 `HOME-01`–`HOME-17`，记录在 [`pages/home-intelligence-surface.md`](pages/home-intelligence-surface.md)，并据此收口 `DEC-G7-01`（首页部分）与 `DEC-G7-02`。该确认只固定产品形态与信息语义，**不是实现授权**：首页、常驻 Agent 编队、总编、通知推送、选题卡存储与校准回路全部未实现，也没有真实数据可驱动；当前在办事项仍只有 `SCOPE-001`，要实现其中任何一节必须另立 SCOPE 并经用户确认。

## 历史项目队列（未在本次 Browser Producer 现状同步中重审）

| 顺序 | 编号 | 事项 | 状态 | 退出条件 |
|---|---|---|---|---|
| 1 | GOV-001 | 文件治理、索引、冲突与变更留痕 | 已完成 | 规则、索引、记录与自动检查随治理提交进入 `origin/main` |
| 2 | GOV-002 | Agent 工程技能、GitHub Issues 与领域文档适配 | 已完成 | 配置、标签、索引、完成计划和治理检查一致 |
| 3 | ENV-001 | Rust 与 PostgreSQL 16 开发环境 | 已完成 | 固定配置、真实数据库副作用、Rust 检查和用户确认均完成 |
| 4 | DISC-001 | 产品定义、领域语言、不变量与责任边界基线 | 已完成 | 七道设计关口与 `USER-DEC-01`–`06` 已确认；不代表完整产品形态或物理技术架构完成 |
| 5 | SCOPE-001 | synthetic fact-kernel technical tracer | 执行中（F01 主链） | 完成 Package → PostgreSQL atomic ingress → two Records → Observation/Current → loopback API → minimal CLI 后 hard stop；不是用户可见产品切片 |
| 6 | ARC-001 | 产品与系统架构收口决策图 | 进行中（决策） | `product-shell`、`primary-daily-job`、`first-producer-canary` 已解决；P0 页面形态与各项实施合同仍需收口，最后由 Mog 批准首个用户可见 SCOPE |
| 7 | GOV-003 | 外部 Agent Issue → worktree → PR 协作闭环 | 修订中（review FAIL） | 同一 PR 新 head 通过独立复核；integrator 合并后重新核验 main/正式文档、记录完成层并手工关闭 Issue；不是第二业务 SCOPE |
| 8 | DESIGN-002 | Topic Intelligence 合成 Reference Page + LIDS v2.0 | 执行中（设计参考/治理） | Issue #7 将 LIDS 作为全项目设计表达标准，并交付其 L2 合成参考页、规格、组件晋升门和检查；不接入真实数据、Web 产品、运行时主题/组件或真实动作 |
| 9 | LOCAL-001 | 独立本地产品、Evidence Library V7 与 Local Canary 路线 | 已完成；路线归档（2026-09-02） | 历史父计划已建立并约束后续独立申请；不再作为当前路线图总编号，也不把 001C-2–3 的历史设想或真实媒体 / OCR / ASR 写成已完成 |

同一时间默认只允许一个事项处于“执行中”。状态流转为：`待讨论 → 需要决定 → 已确认 → 执行中 → 验证中 → 已完成`。来源不足使用 `SOURCE_INCOMPLETE`；必须由用户决定的边界使用 `DECISION_REQUIRED`；外部条件无法继续时使用 `BLOCKED`。

## 历史下一步记录（不覆盖本页顶部 `0.8.28` 当前快照）

当前主线实施事项仍是 [`plans/active/scope-001-content-evidence-vertical-slice.md`](plans/active/scope-001-content-evidence-vertical-slice.md)。它把已确认设计压缩为一条合成/脱敏技术 tracer：终态 Package 接入、逐 Record 处理、最小 Content 身份与 Observation、字段级 Current 来源，以及 API + minimal CLI 的解释结果；它不是用户可见产品切片。`AUD-XHS-001 / Issue #74` 已作为历史受限探针归档，不再是进行中现实页面工作；若未来真实链被某一具体字段阻断，必须另行授权最小探针，且不得由本历史卡自动恢复执行。

语义冻结已经经过四次独立只读攻击。第四轮发现的 payload/ingress 分层、pre-routing audit union、动态 ref/time snapshot 和跨 Attempt Satisfaction 已分别用 processor owner、封闭数据库 union、固定 proof clock/ref 与 Work 1:1 Attempt 收口。按用户最新裁定不再进行第五轮文档复核。F01 已完成 contracts tracer、手工静态 manifest Oracle，以及 **TDD 步骤 3 的 Package ingress**：有效 Package 在随机隔离的 PostgreSQL 16 schema 中原子接入，行数逐表对齐 manifest 的 `fresh_seed` 与 `final` 阶段，六个事务故障注入点任一失败都零半写，同 hash replay 与不同 hash conflict 只新增一行 delivery 且不覆盖既有 Package，接入后 Observation/Current/Source 侧表仍不存在。三项主链保护中前两项（接入阶段不提前形成 Observation/Current、接入故障零半写）已有真实数据库证据；第三项（坏 Record 不撤销合格 Record）属于 Record processing，尚未实现。下一步是 TDD 步骤 4 的两条 Record 独立处理与 Observation/Current，需要先创建 `0002` migration；随后才是 loopback API 与只经 API 的 CLI。完成该链后停止实施扩张；其余 canonicalization、完整 envelope、资源上限和 F02–F10 等待 ARC-001 收口及后续明确排期，不从 F01 自动继续。

并行的产品决策下一步是推进 [`plans/active/arc-001-architecture-closure-decision-map.md`](plans/active/arc-001-architecture-closure-decision-map.md) 的 `p0-surface-prototype`。001A 已完成其受限本地入口与诚实空态；#34 已合并，提供 discovery-only 接纳与本地读取投影。当前 Issue #38 只为这条已经合并的本地链补充持久数据库 migration、健康 readiness 与重启保留证明，仍不证明真实采集能力。`DESIGN-002` 的 Topic Reference Page 是 `p0-surface-prototype` 的一个可走查输入，而不是这张票的完整答案。首页职责到运行时的语义已澄清，但这不授权 P0、Agent、真实 producer、媒体或旧系统迁移实现；真实 producer 只能在相应 Local 子卡的 preflight 后申请。`ADV-AUDIT-001` 的独立复核最终处置见 [`audits/pre-implementation-architecture-audit-2026-08-21.md`](audits/pre-implementation-architecture-audit-2026-08-21.md)；它没有改动 F01 migration/合同，也不授权真实访问。

`SCOPE-001` 不包含真实 XHS 访问、真实原文、插件升级、Topic/Corpus/Signal/AI Agent、正式趋势、Web UI、生产部署或旧数据迁移。这个排除继续约束 SCOPE-001 本身；与之并列的 LOCAL-001 只为独立本地产品路线建立后续申请顺序，绝不把真实 producer 实验自动视为已获授权。任何真实搜索、详情、媒体 acquisition、OCR/ASR 或原文处理仍须取得对应的账号、对象、访问、存储、隐私、处理器和原件处置合同并通过 preflight。

已确认边界：

1. Rust 固定 `1.95.0`，使用本机工具链。
2. PostgreSQL 固定 Docker 官方 `16.14-bookworm`，不安装本机 PostgreSQL。
3. 开发库和 proof 数据库与旧项目隔离，禁止恢复旧 dump。
4. Node 固定 `24.13.0`，只用于历史 fixture 验证。
5. `.env` 自动生成本地随机密码，禁止进入 Git。

当前仓库的 Git 提交身份已确认为 `Himog0921 <188755262+Himog0921@users.noreply.github.com>`；它是仓库本地配置，不改变其他项目。

实施前准备矩阵见 [`audits/pre-scope-001-readiness-2026-08-20.md`](audits/pre-scope-001-readiness-2026-08-20.md)，正式范围以当前 `SCOPE-001` 为准。GitHub noreply 身份和设计基线推送已经真实验证；后续提交仍需逐次核对实际 Git 状态。

### TOPIC-MAP-V41-001 / 2026-10-09

Mog 授权按手册 v1.1 与 V4.1 Demo 全面开发主题图谱，并允许 subagent 并行。[实施计划](plans/active/topic-map-v41-001.md)记录完整表面、状态、依赖、验收与源授权；[实施验收](design/acceptance/topic-map-v41-001-acceptance.md)区分源码、浏览器与业务边界。V4.1 源码已包含在本轮 `main@a88ff387` 基线中；本节下方“未进入 main/3000”等文字为相应日期的阶段回执，不代表这一基线仍未集成。生产迁移/3000 的实际状态另依部署回执，本轮核心建设未重新访问运行库核验。旧 Topic Workspace 深链保留，`/topics` 默认完整概览。


2026-10-09 前端追加纠偏：Mog 已指出初版与Demo不一致。当前包按Demo最终布局修复整体页面，并重新核验桌面/窄屏与来源失败状态；初版浏览器只证明功能路径，不能替代本轮视觉核验。修复与证明仍仅在专属开发分支及合成3109环境，尚未进入main/3000，详见同一实施验收与交付计划。

2026-10-09 公共页头追加纠偏：Mog 明确要求沿用其他页面的页头和面包屑，覆盖此前64px独立页头方向。开发分支已恢复shared header/CSS/domain picker；与采集页同构建浏览器计算值一致，1440/390与两域切换实际核验，5组UI回归及独立复审通过。仍未进入main/3000，最新截图/边界见同一实施验收。


### TOPIC-MAP-CORE-001 / 2026-10-12 / 真实语义校准

已按 Mog exact-head 授权将 PR #394/f3ccf34a 合并为 main `050ff3dd`，CI 38218668561 成功；本机 runtime-main 的 API、巡检与媒体进程 cwd/txt、3000 页面及 schema0120 已核验。没有迁移。三篇真实材料五窗口试跑产生5次模型调用、23754token：0接纳、3失败、2未尝试。旧失败和费用保留，研究再次暂停、自动扩批及补采关闭；不把提案成功、退役或失败计为有效进展。真实引用遗漏时长仍被复核放过，以及严格复核额外字段、讨论粒度与回应笼统拒绝，已进入同一Issue #382的专属校准候选。候选只向复核提供精确引用，按具体问题而非语态区分讨论，细化回应规则，并对唯一明确格式错误作有界重新生成；未部署，也未取得同义归并、不同障碍或父包含的真实业务验收。详见[活跃计划](plans/active/topic-map-core-001.md)及当月进度。以下段落是各自日期的历史回执。

### TOPIC-MAP-CORE-001 / 2026-10-11 / 主题质量与规模修复

Mog 认可运行审计后的修复方案。部署基线为 `6feaaf60`（#385），研究已通过产品命令暂停；新候选位于专属 `codex/topic-map-quality-382`。来源短文本分组、v3 稳定概念归纳与候选父树、有界合并重评/首次材料公平调度及前端判断依据/进度分责已实现。最终主题 Rust 107/107、实际 JS 18 组、Pi 适配器 41/41、隔离 PostgreSQL 52/52 和 API/core 编译均通过；隔离容器和卷已清理，独立提交复审 PASS。复用 0120，无新增迁移、真实模型调用或预算提升。旧候选保留历史标识与可读入口，不自动改写正式定义或旧绑定。完整证明见[活跃计划](plans/active/topic-map-core-001.md)及当月进度；尚未推送、合并或部署，真实中文语义质量与 Mog 验收尚未验证。下方均为对应日期的历史阶段回执。

### TOPIC-MAP-CORE-001 / 2026-10-10 / Issue #382

后续状态：PR #383 已按 Mog 授权合并，main / 本机运行基线为 `d214cdc5`，0120 已迁移。真实研究核查发现自动入队无背压和归属调度饥饿、SSE 过小限制及进度口径缺口，Mog 已要求修复。研究现已通过产品暂停以保留账本；修复候选在 `codex/topic-map-research-recovery-382`，范围与验证见同一[活跃计划](plans/active/topic-map-core-001.md)的研究阻塞修复章节。以下段落记录前次开发候选交付，不是当前部署状态；新修复尚未合并或部署，真实语料效果仍待运行核验。

Mog 要求把已搭建的主题模块发展为真实业务内核，算法、数据库和前端一次打通。专属分支 `codex/topic-map-core-001` 基于 `a88ff387`，交付见 [Draft PR #383](https://github.com/Himog0921/linggan-intelligence/pull/383) 及[主题图谱业务内核](plans/active/topic-map-core-001.md)。候选采用来源窗口提炼、BM25/CJK 与既有本地 WeMM 混合召回、按精确定义逐讨论归属和可追溯对照；新增 0120 的不可变讨论/归属/概念规则、512 维向量缓存和有界定义重评队列。完整概览、五阶段旅程、来源阅读与备选保存继续复用既有界面。

独立复审发现的并发派发、旧定义归属、跨域身份、概念关系、作者/评论对照、保存重开、来源撤回、同文重采、筛选计数、部分结果和评论独立可读问题已逐项修复。同一来源窗口只投影最新已接纳版本；缺少可复核创建来源的旧机器定义停止进入展示、后续召回和保存，人工裁定版本仍保留自身资格。比较保留单份结果，在全部实际参与作品投影，按原结果、条目索引及方法版本保存；限制相关来源后两侧派生结果同时撤回。研究进度显示提炼/归属/比较和有界真实失败明细，终态不提供后端拒绝的恢复操作。

代码提交 `32887e24` 的 [CI 38041566274](https://github.com/Himog0921/linggan-intelligence/actions/runs/38041566274) 已完成成功：主题 Rust 82/82、UI 14 组、结构化适配器 18/18、原生 PostgreSQL + pgvector 34/34、API/core 编译和治理/差异检查全部通过。根与独立协作者读取完整日志逐项核验；隔离容器及卷清理成功。原生证明包含长文及 OCR/ASR 尾段、31+ 评论、比较双方保存、来源/定义撤回、unknown 防重发、有限授权、维护并发与真实失败进度；早期失败及修复回执保留在本月进度。前序 Chrome 的 15 个合成状态在桌面及窄屏可读，无页面错误或整页横向溢出；后续进度增量由实际 JS 验证，未重复浏览器验收。

当前为已实现且完成上述自动化验证的开发候选，独立复审通过。全库 Clippy/旧函数边界仍有基线失败，本包未新增豁免。真实中文材料的主题准确率、召回率、本地 WeMM 质量及大规模时延尚未测量；合成证明不代表论文指标或业务验收。未合并、执行生产迁移或部署，无真实模型及新增平台采集。


### 2026-10-13 · 主题图谱进入性能后续候选

Mog再次要求测试优化，独立分支基于main9e45e01c。候选保留全部3224作品/48主题/141研究作品及来源资格，完整投影/指纹与稳定正式读数一致，统计重算差异0。完整API8489/8638→4591ms；同法浏览器进入11015→5950ms、刷新11482→中位5575ms、旅程11551→6076ms。全套96隔离PG与最终定向/分页反例、160单元、29JS通过。仍约5–6秒，不宣称秒开。只读候选验证已完成，未提交/推送/部署；本任务未修改正式3000、迁移、研究/采集/模型权限。见[性能计划](plans/active/topic-map-read-performance-382.md)13日回执。

### 2026-10-13 · 主题图谱首屏分阶段候选（未发布）

同一专属性能分支完成只读RAM投影预构建、全统计+首批12材料、完整紧凑索引及单作品资格传输。3224作品/48主题与稳定正式canonical投影一致，统计重算差异0；资格/定义/配置变化失效，运行心跳不造成许可失效。浏览器进入首屏11140→542ms、完整材料1268ms；刷新3次中位首屏563ms/全材料1148ms，但一次重建仍7598/8103ms；同一证据详情中位10621→440ms。冷默认启动/新范围仍5398/5283ms，未实现持久化或增量维护，不宣称每次亚秒/P95。98完整PG及最终8定向、160单元、3API、31JS通过。候选已关闭，正式runtime-main@9e45e01c/PID16192未更新；未提交/推送/合并/部署，研究active为原有运行状态，本包未触发模型/采集或改变策略。见[性能计划](plans/active/topic-map-read-performance-382.md)最终回执。


### 2026-10-14 · 采集读取性能候选

Mog当前授权的[采集读取性能](plans/active/collection-read-performance-001.md)在独立分支实施和实测通过，保留完整统计、来源资格和Lease；目标页/抽屉重复读取已收窄。此前#414主线任务修复已核验，本候选尚未提交、合并或部署，不能与正式3000完成层混同。


RESEARCH-DOSSIER-001 / #420 历史记录（现行实现见 RESEARCH-DOSSIER-V2）：第一阶段候选已实现 rules-v2、本篇研究底稿、原图对照、固定范围重新分层，并接入创作者/评论 P1-P2/主题材料。隔离 PostgreSQL、相关回归和合成页面验证见[模块计划](plans/active/research-dossier-001.md)。PR #423 已合并并更新 3000；Mog 随后授权三篇真实作品的本机重新分层，旧版面与原始文字保留，研究暂停及 worker 停用不变。底稿操作不调用模型；后续恢复自动研究须考虑新指纹触发原调度器重新准备的影响。


## 2026-10-20 · LIDS-UI-001 本地候选

用户直接批准真实 UI 实施；[计划](plans/active/lids-ui-implementation-001.md)与[验收](design/acceptance/lids-ui-implementation-001-acceptance.md)记录共享基础/A壳/采集账本。基于612dcbea独立工作树，不代表当前main/3000更新。前期规范包runtimeMigrated=false为历史范围，不是当前实现进度。


2026-10-20创作者续修：Mog已派按P05:list/B统一FIND、选择/筛选、八列表格与对象抽屉，继续当前独立LIDS候选。页面规格已补当前组合及S1边界；实施与验证只读[同一验收](design/acceptance/lids-ui-implementation-001-acceptance.md)，先不合并部署。

2026-10-20证据库续修：同一LIDS-UI-001真实候选采用B查询/视图合带、统一筛选/作品卡与对象详情，保留FIND/Z02领域/E1白场及三种排版；参数从LIDS单源读取。实施/回执沿本包计划与验收，Mog签收与合并部署另计。
