# ACC-WEB-REACT-001-F · 主题图谱与 Topic Workspace 迁移证明

> 状态: 一次性报告
> 最后核对: 2026-10-18
> 适用范围: WEB-REACT-001 阶段 F / Issue #440 的候选实现和验收证据
> 事实来源: main/runtime 82d48a72 实际 UI、独立工作树、正式 Axum/一次性 PostgreSQL/React 浏览器证明
> 冲突时以谁为准: Mog 最新确认、真实回执、当前代码和来源合同；候选不等于部署与业务签收

## 2026-10-18 当前状态补记

F 已随 [PR #441](https://github.com/Himog0921/linggan-intelligence/pull/441) 合入并部署为 `acdfbb87`；概览采用 Mog 已定 B「材料优先」。原两项 Topic 内核回归已修复：最终本机 18 个目标、99 项通过、0 失败，前端 79 文件／838 项通过；最终 head 的四项执行 CI SUCCESS，四项 SKIPPED 不计通过。正式 3000 与候选 31 个前端产物字节一致；实际 212 张页面截图已核验（概览 72 张含完整滚动，其他状态 140 张主要为首帧）。差异中包含滚动时点、时间文本和少量栅格边缘，原始差异保留，不声称每张像素全同。私有证明在 `artifacts/private/web-react-001-f/kernel-release/release-verification.json`（仓库主 checkout，未入 Git）。旧性能证据仍属于改版前候选；B 最终性能和 Mog 业务验收不据此关闭。

当前 `origin/main` 与正式本机 3000 均为 `acdfbb876bf3fcb5cb4aaf20ceea565562f0b428`（PR #441）。本次同步依据 GitHub 合并记录、运行进程及 `/health.webApp`，不把历史候选证明改写成新版本重跑结果。

## 2026-10-17 适用范围更新

Mog 已在当前会话选定 B「材料优先」并授权正式改版。下列旧迁移的像素一致、21轮性能和5a545dfb审查结果均保留为 **a62d280e 的历史证明**，不覆盖新的 B 源码。B 当前验收以本报告末尾的修订记录及 [F 计划 §7](../../plans/active/web-react-001-f-topics.md#7-2026-10-17--mog-选定-b材料优先正式改版) 为准。旧概览保真要求在本轮被明确选定 B 替代；原功能与数据合同继续有效。

## 对象与运行边界

执行者 Codex root，task-id `WEB-REACT-001-F-TOPICS-20261016`，branch `codex/web-react-001-f-topics`，工作树 `.worktrees/web-react-001-f-topics`。范围、全部表面/状态和依赖见 [F 分册](../../plans/active/web-react-001-f-topics.md) 与 [Issue #440](https://github.com/Himog0921/linggan-intelligence/issues/440)。两个获准实施代理曾分别辅助主图谱与工作区，额度耗尽后由 root 完成集成与证明；没有新增实施代理。

基线是 3000 正式运行的 `82d48a72b95d905bffd0def9314fdee004893ddb`。当前阶段没有修改运行目录、重启 3000、执行共享库 migration/写入、启用 worker/研究，或调用业务模型/外部平台。实际数据只在本机私有证据中使用；Git 与 CI 只含手写合成 fixture。候选只读桥接 3112 拒绝所有非 GET/HEAD；不是正式 Axum 链路，也不支持保存。

`/api/local/topic-map/context` 是把旧 HTML 已嵌入的领域列表与默认领域改为 JSON；正式 API 仍按旧 SQL `ORDER BY name LIMIT 1` 取默认领域，不能用列表创建顺序代替。3000 尚无此新接口，3112 从当前 corpus/shell GET 和原生 HTML 的默认领域属性适配相同事实。这个适配例外明确标记为预览，不冒充正式 API 部署；正式 context 另由真实 PG/Axum 验证名称排序不同于创建顺序的场景。

## 能力与状态核对

| 范围 | 核对与证据边界 |
|---|---|
| 主图谱 | 六页签、共用主题树、全范围统计、分批材料读取、关键词/URL/返回、图表/散点、同名不同身份、地图/列表/主阶段/涉及率和待分析均保留 |
| 阅读与判断 | 原文/评论/研究独立资格、Unicode 精确片段、作者/评论讨论身份、嵌套阅读和精选替换、关闭/切域/卸载后晚到读取不回写；字段缺失失败，未知不变 0 |
| 保存与命令 | 我方账号、表现规则、人工备选、研究配置/暂停恢复、原有搜索/详情轮次停止、结构 preview/apply；全请求 body/身份冻结在内存，网络/5xx/异常成功回包均需显式重试，4xx 明确拒绝才释放 |
| 来源与导出 | 人工备选真实冻结来源、重新读取资格、Markdown 精确引用；来源限制不以当前原文替代历史依据。没有为界面迁移改变后端来源规则 |
| Workspace | 定义/版本、冻结包、支持/挑战/边界筛选、Inspector、精确 WorkRef、404/503/非法 key；保留真实未知和现有链接行为 |
| 生命周期与错误 | 研究 GET 失败仍可读材料；旧领域请求即便忽略 abort 也不会回写；对话框关闭不被晚到响应重开；不同领域/弹窗的命令结果不灌入新表单 |

前端按路由懒加载；页面展示为 React JSX，类型和运行时 decoder 明确，算法保持原语义。主内容与研究就绪状态分别通知，避免每次进度读取重绘长列表。带引号名称通过类型化 React props 传递，不参与属性片段解析；等待回执时通过局部 Context 禁用显式写入按钮，保留其原 disabled 条件。原生重建内容与弹窗指定滚动位置的行为映射到 React key/scroll 恢复；没有保留 innerHTML 页面 renderer 或双轨开关。

## 实际 UI 截图对照

- 捕获脚本 `apps/web/test/e2e/topics-visual.mjs`、精确比较器 `topics-pixels.py`（Pillow；差异/缺图即非零退出，原图不修改）；四视口 1440/1280/1024×900、390×844，deviceScaleFactor=1、reduced motion。
- 同一组当前 3000 GET 响应逐字节录制后固定输入；旧页面 HTML 入口固定同一次实际输出，JS/CSS 仍是 3000 原生实现；候选读取相同 body。媒体走现有同源 hash 资源。请求仅 GET/HEAD，写入与外部源均阻断并使证明失败。
- 最终源码5a545dfb构建完整复跑 `baseline-full-v2/` 与 `candidate-reviewed-final/` **2,905 / 2,905 逐 RGB 像素一致**；尺寸一致，无缺失/额外图，无裁切、遮罩、模糊或容差。两侧浏览器 errors、blocked、failures 均为 0。比较记录 `comparison-reviewed-final/report.json`。
- 同脚本嵌套补充模式最终复跑 `baseline-nested-v2/` 与 `candidate-nested-final/` **2,279 / 2,279 逐 RGB 一致**；对每个选定外层纵段，捕获当前可见的单个 auto/scroll 内层容器横纵段及两端。errors/blocked/failures、缺失/额外图均0，比较记录 `comparison-nested-final/report.json`。两组共 **5,184 / 5,184**；不宣称更深多级祖先的完整笛卡尔组合或未出现状态已穷尽。
- 包括整页与脚本选定外层滚动容器的纵段/右端、双视图、六页签、地图/列表、精选/判断/替换/产品/来源/补采、研究配置/历史、手动保存和独立 Workspace。
- 首轮完整对照 2,901 组中有 25 组差异、4 组漏拍；已纠正主区重建、弹窗滚动语义和 Workspace 滚动选择器后复跑。早期读取中截图及失败迭代保留，未计作最终完成态证明；审查前 candidate-delivery-locked、candidate-reviewed、candidate-nested-v2 的通过记录也只作迭代，不冒充最终构建。
- 原图/实际响应和最小 DOM 几何均仅在本工作树 `artifacts/private/web-react-001-f/`；不提交、不上传 GitHub/CI。截图证明实际可读状态的保真，不证明所有尚未出现的状态组合或 Mog 体验签收。

## 正式真实链路与回归

正式 `apps/api` Router + 本候选生产构建 + 一次性 PostgreSQL，所有输入均合成。`scripts/test-web-app-postgres.sh` 默认全阶段执行，B 设置、D1 创作者、E 运营、F 主题均已通过；F 9 个浏览器步骤全部完成，errors/blocked 为 0。`isolated-final/` 保存全阶段证明；最终源码构建的F再验在 `isolated-reviewed-final/`，含合成读取/回执/下载与截图。

F 证明包含：名称排序默认领域；27 篇分域材料、完整统计与未裁剪范围；文本中的 HTML 安全显示；我方账号真实提交后丢失答复、修改表单后原 payload/身份重试、回执复用及变更 payload 的真实 409；平台规则版本；人工备选保存/重开/导出；false/false 研究策略与预算配置、暂停/恢复；已有已停止搜索与详情轮次独立读取/停止；精确来源结构合并 preview/hash/apply/禁用重复应用；Workspace 角色、引用、真实 404/非法 key 400；窄屏导航与 foreign guard 403。Rust 断言我方账号、备选各只落一条，模型调用及采集工单数不变。没有合成研究成功或平台采集结果。

前端 TypeScript/lint/生产构建及 **46 files / 490 tests** 通过，含带引号主题名属性隔离、研究503材料可读、切域逆序、卸载取消和提交等待禁用反例。API 非 ignored 单元 **169 passed / 69 ignored**，关键浏览器 proof 另行实跑，不把 ignored 当通过。Workspace 内核/HTTP 隔离 proof **7 passed**。

原有 `scripts/test-topic-map-postgres.sh` 实跑 **97 passed / 2 failed**（18 个 test targets，0 ignored，隔离容器/卷清理已核验）。失败是：

1. `topic_map_core_grouped_postgres::saved_author_context_restores_exact_ranges_and_withdrawal_keeps_independent_comment_raw_voice`：保存的作者 ASR 语境重开返回 `source_unavailable`。
2. `topic_map_saved_sources_postgres::later_media_pages_and_full_asr_ocr_reopen_without_authorizing_restricted_siblings`：材料 fragments 未包含预期完整 OCR。

`crates/**`、`database/**`、Cargo manifest/lock 与 `apps/pi-adapter/**` 对 base/main 82d48a72 的 diff 为 0；上述失败发生在不引用页面/API 的原有内核 integration target，故不能归因于此 React 迁移，也不能将本包相关内核检查写为全绿。源码独立核查定位到两个既有冲突：作者语境 producer 的 `media_fragment.source_ref` 使用 derivativeRef，而冻结媒体恢复按 jobRef 查找；OCR fixture 仍使用 `synthetic-v1`，材料读取只接纳 `rules-v1/v2/v3`，v2/v3 还要求保留实质文本引用。以上是源码层定位，不代表已修复或现场重算；本包不修改领域资格或放宽测试掩盖它们；合并前须处理/明确裁定这一失败边界。完整原始失败与通过日志都在 private 的 `topic-kernel-final.log`。

## 原有 Workspace JSON 链接缺口

现有 Workspace API 给出的 detailUrl 未携带 ObservationDomain UUID，而 Work Resource API 要求 `?domain=<uuid>`；原页面点击该链接会返回真实 400 `domain_required`。Workspace 的 domainKey 不是这个 UUID，本迁移没有推断映射或用 `domain=all` 绕过。证明分别核对原 href 的 400，以及明确传 fixture domain 后同一精确 WorkRef 的 200/identity。不能将后者写成现有链接闭环已修好。

现有结构 producer 还保存 `candidate/machine_proposed/completed` 工作区；迁移解码器已兼容该合法形状。原页固定“人工裁定”标签不随 runKind 改变，候选保留该 UI 但不将其视为人工身份事实；此展示语义缺口同样登记，不能因迁移扩大为新分类功能。

原 404 空态的 Inspector link 带 hidden 且无 href，旧 CSS display 规则仍会画出空按钮；候选保留现场，不顺带改变设计。本报告显式登记这些旧行为，后续是否修复由独立产品/合同事项决定。

## 删除、替代与完成层级

同包删除 `topic_map.js/.css`、`topic_workspace.js/.css`，Rust 原页面 HTML/资产 handler，以及只服务旧 DOM 的 `scripts/test-topic-map-ui.cjs`、`scripts/test-topic-map-compact-ui.py`。业务 API/领域与 schema 保留，新增的仅是原 HTML 引导事实的 JSON 读取。CI 改为 React 测试和正式浏览器链路；旧 asset 404 与非法/未知路径、API JSON 边界另有路由回归。

Workspace 隔离证明脚本原来使用无 pgvector 的 PostgreSQL 镜像，当前全 schema 会失败。已对齐项目固定 pgvector 镜像与 TCP readiness；容器/卷均独立并清理，不修改共享服务。

Design（当前 UI 保真）、Code（候选）、Automated、隔离 Real-chain 的证据分列如上。性能见下文；GPT 独立源码审查已 PASS（范围与指纹见下文）；Deploy 和 Mog acceptance 均 **NOT VERIFIED**。未确认写请求只保留在当前图谱页面实例内；关闭弹窗可恢复原请求，完整页面导航、离开路由、刷新或关闭浏览器后不保留，不宣称跨导航/刷新恢复。此包交付 Draft PR，不含合并部署授权。

## 提交前独立审查与交付状态

已实际调用 AGENTS.md 指定的 `commit-reviewer`（Sonnet/high），仅对子进程移除冲突认证/model 环境覆盖，使用 CLI 正式账号；返回 `You've hit your session limit · resets 10:30pm (Asia/Shanghai)`。**没有审查结论，不算 PASS。** 日志 `commit-reviewer-sonnet.log`；无实际材料/截图送交该模型。Mog 随后在本会话明确允许本 F 包改用 1 个 GPT 只读复核后提交 Draft PR；已派唯一审查代理 `/root/topics_commit_review`，按同一规范复核，未扩展实施代理。成立/不成立发现与处理逐项记录如下。最终源码审查 PASS，代理未改代码、未读取实际材料/截图；Draft 的最终 head 与 GitHub CI 状态以 PR 回执为准，不默许合并/部署。

2026-10-16 收尾只读核对：GitHub main、干净的 runtime-main 均为 82d48a72，3000 listener PID 54949。候选仍在专属工作树与限 GET/HEAD 的 3112；不能将此预览当作 runtime 部署。源码审查、GitHub CI、旧内核2项失败的裁定、部署和Mog签收均保持独立完成层。

## 同场景性能复核

固定实际 GET、1440×900、fresh context、交替顺序。审查前7轮及追加21轮完整保留；全部修复后的5a545dfb生产构建在截图任务结束后，另独立复跑同条件21轮，没有并发本包截图/测试。没有改变比较场景、输入或倒填阈值。最终每侧数据如下（ms；小样本p95是观察值，非上线SLO）。

| 场景 | 原生 median / p95 | React median / p95 |
|---|---:|---:|
| 全材料就绪 | 212.1 / 284.1 | 249.5 / 378.6 |
| 结构页签 | 21.0 / 32.5 | 21.3 / 33.7 |
| 散点切换 | 22.4 / 38.6 | 22.4 / 33.3 |
| 来源页签 | 128.7 / 152.2 | 141.1 / 147.8 |
| 材料页签 | 18.1 / 27.5 | 23.1 / 29.4 |
| 阅读器（含录制读取） | 132.9 / 264.2 | 125.2 / 149.5 |

全材料就绪中位增加37.4ms、p95增加94.6ms；来源切换中位增加12.4ms，结构/散点接近，材料增加5ms，阅读器中位降低7.8ms。不能说所有指标变快，但没有观察到明显的中位交互阻滞。最初候选首次进入约869ms，已定位并移除React.lazy/Suspense页面内部提交等待，保留路由层按需加载；没有以删材料、身份守门或强解码换性能。

最终阅读器单次最大原生350.5ms、React573.2ms，完整保留而不移除长尾；21轮中的p95如表，不承诺没有尾部波动。最终errors/blocked均0，记录 `performance-reviewed-final/report.json`。审查前7轮 `performance-closed/` 和21轮 `performance-delivery-locked/` 仅是迭代测量（其中ready median203.1/237.5ms、reader122.8/119.0ms），不冒充最终构建。早期networkidle口径也不作最终依据；当前为明确overview完成、读取settlement和两次动画帧。

比较排除了现场SQL、平台和模型延迟。阶段F未在开工时冻结数值SLO，故报告条件、绝对差值与局限，不能倒填C阶段的S0预算。

最终源码快照（54个变化的源码/脚本/CI文件，删除以null登记）SHA-256：`5a545dfb4e79dfffc8698398bd799e7e7573240eaa3bbd710d6616967c3d5a0c`。完整路径/文件 hash 在 private 的 `source-manifest-reviewed-final.json`，算法为按路径排序 files 清单、对象键排序的 compact UTF-8 JSON SHA256；这是源码清单指纹，不是 Git 提交。独立审查代理逐文件重算一致。早期清单与构建保留为迭代证据，不混作最终版本。

## 当前数据人工浏览器走查

审查前候选的3112（当时3000 GET，非录制回放）检索“对立违抗”显示20篇/服务端3274篇；切换用户生命旅程仍保留query，打开首篇原作后来源资格完成、已有评论2/2可读，关闭后回到原范围。研究状态保持paused。当次候选标签已恢复默认概览供审阅，用户原3000标签未改。这条完整搜索/阅读走查不冒充5a545dfb最终构建；最终预览另通过只读重载核对材料3274、研究paused、无读取错误。最小回执`actual-ui-smoke.json`不含正文/评论；不把这条走查解释成真实写入、研究产物质量或Mog验收。

审查前构建收尾（迭代证据）：`verify-delivery-locked.log` 的479测/TypeScript/lint/build通过；`candidate-delivery-locked`与原生基线2905张全部RGB一致，errors/blocked/failures均0；`isolated-closed.log`的F9步通过（最后一次构建包含提交等待禁用修正），容器/卷/支持目录清理核验。治理、UI手册、YAML和shell语法通过。尚无PR，GitHub CI未运行，不能称CI通过。

### GPT 只读审查发现与 root 裁定

| 发现 | root 核实与处理 | 反例与证明 |
|---|---|---|
| P2 表现图平台选择未写入状态 | 成立；plotPlatform 不在 URL 材料筛选键中，独立处理该展示状态，不触发材料 refetch | 修前强制重建图表后选择丢失；修后保留平台且 GET 数/材料 scope 不变 |
| P1 失效备选合法 null 被当成坏 JSON | 成立；title/angle/rationale 接受显式 null，仍拒绝缺字段；只有可读资格和三文本均成立才导出 | 修前 summary/单读/组件失败；修后主图谱仍可读、失效原文不展示、无导出入口 |
| P2 既有机器候选 Workspace 被拒绝 | 成立；仅加已有 machine_proposed completed 类型，保持 exact pack/identity 守门 | decoder 与页面两项修前均失败、修后通过；未知 kind 仍失败 |
| P2 首次图谱等待/503 阻断独立研究 | 成立；研究读取/刷新/启停及弹窗关闭/重试不依赖图谱 snapshot；表单只在真正用图谱处断言 | pending/503 两项仓库反例修前红、修后绿，覆盖打开/刷新/明确关闭自动与采集的设置回执/关闭，晚到 summary 不重开 |
| 原生成物登记仍称已删 DOM runner 权威 | 成立；降为历史并指向 React/真实链路替代 | 治理检查及 diff 核对 |
| 2,905 组的滚动覆盖表述过宽 | 成立；原脚本只穷尽选定外层纵段及右端，原证据不作内层二维穷尽声明 | 新增嵌套容器补充模式，逐外层纵段 × 单个可见嵌套容器横纵段整页捕获（含末端，不宣称多层祖先全部组合），原图/早期记录保留；结果另列 |

最终全前端 verify 46 文件 / 490 测、TS/lint/build 通过（`verify-reviewed-final.log`）。最终正式 F 九步骤在生产构建上再验通过（`isolated-reviewed-final.log`），受控容器/卷清理已核验。独立研究的 pending/503 两项修前红、修后绿分别见 `independent-research-before.log` / `independent-research-after.log`。失效备选组件 fixture 曾缺 domainRef 被正确域守门拒绝，补足真实 API 身份后通过，没有放宽守门。代理独立执行 6 文件 / 76 测、临时 5 项反例、API 169 passed/69 ignored、路由 13 passed/1 ignored；root 的全量 490 测不冒充代理亲自执行。最终源码审查 PASS，仅限上述指纹；CI/旧内核/部署/Mog 验收边界不随之升级。

补充捕获的审查反例：nested-only 跳过普通 shot 后，入口没有等待读取稳定即枚举容器，会漏掉晚出现滚动区域。该发现成立；入口加入同一 settle、字体就绪和两帧屏障，独立合成 150ms 晚出现的双向容器由修前 0 张变为修后 25 张，覆盖 x450 右端/y230 底端。未完成的 baseline-nested/candidate-nested 已停止并保留，不计通过；v2 从新目录重跑。

初始“地图未读时直接根崩溃”假设不成立：dispatcher 的 snapshot 闸门先返回。root 与 reviewer 明确驳回该假设；复现的真实问题是研究入口静默无动作，按上表修正，未将两者混写。


## 2026-10-17 · B 材料优先修订验收

本轮在同一 F 分支中同步 origin/main@24e3ec23 的 D2 迁移。全局 shell 继续使用现有实现；本页双视图移到左侧，全部主题及旧方法候选分区保留。概览换为紧凑服务端读数、六页签、左材料右分布与来源边界；片段/作者/时间/赞藏评分享/阅读/人工备选/全部匹配/讨论场景、动态规则与其他五视角完整保留。右侧六分桶只描述当前返回作品，搜索不改变它或上方完整统计；点赞未知分开，跨阈值桶不标为高表现。

当前已运行：TypeScript/lint/build、前端78文件834测、API155 passed/72 ignored、全阶段正式隔离链路 B/D1/D2/E/F 通过；F10步（原9步+新增B统计/搜索/双导航/图表精确材料回链）通过，模型与采集无增量，受控容器/卷/支持目录清理核验。第一轮全量前端有1项未修改采集页焦点断言失败，原日志保留，限制2 workers的两次全量复核均834通过；没有删除/跳过/放宽断言。日志位于登记的 private/overview-b/。

指定 Sonnet/high 本次 CLI 未登录，返回 is_error=true，没有审查结果，不能算通过；沿用 Mog 对同一 F 包的 GPT 替代授权，由原唯一 reviewer 复审新源码与主线集成，未新增实施代理。原唯一 GPT reviewer 已逐项复核当前源码与共享集成接缝；新增发现是视觉捕获器跨视口无条件 toggle 可把旧方法候选/窄导航关上，已改为按 open/aria-expanded 幂等展开并在截图前断言。独立合成负例修前后三宽遗漏、修后四宽均真实展开；选中页签四宽均为3px signal橙线，切换后旧页签透明；普通Tab与旧React对照没有复现外层滚动回归。真实CUA定位焦点曾移动外层页面，未将此工具观察擅自当成普通键盘产品缺陷，也未修改未获准的全局shell。

最终源码为59项内容/删除状态清单，SHA256 `0d1cf1a94bc939ba22744b2a665c88b8a47addb2b7293a95884cd2c64efef660`。reviewer 独立重算一致；额外实跑 Topic79测、14路由/1ignored、一项多平台/搜索/精确材料回链反例和四宽捕获/样式检查。没有未闭合新增源码缺陷；提交前还按该指纹及暂存树核对文档/最终汇总，真实原文与截图未交审查模型。

### B 最终实际页面截图

- 最终构建 `overview-final/`：1280/1440/1920×900、375×844，共72张，包含概览主区完整纵向滚动、左右材料/分布与表格、完整导航（含展开旧方法候选）的全部滚动段。报告明确此专项不覆盖弹窗。
- 相同最终构建 `states-final/`：四宽、六视角/旅程/主要既有弹窗/Workspace共140张状态首屏，**不宣称这140张含所有弹窗内部滚动**。两组合计212张，errors/blocked/failures均0，document横向溢出0；无裁切/遮罩/容差。fullPage保留实际文档溢出高度，主区内部滚动另逐段捕获，不拼接成伪长图。
- 补选中线前 `actual-candidate/` 2,701张全场景完整滚动，0errors/blocked/failures，作为历史迭代保留，不能冒充最后9行选中线CSS后的完整场景结果。原a62d的5,184 RGB与21轮性能也只作历史证据，本轮不声称旧页像素一致或性能已重测。
- 与用户已选择B逐项实看：紧凑统计、左侧双导航、白色材料列表/右侧分布及来源边界、黑色读数和橙色局部强调一致。正式版保留现有全局Header/Context、完整候选分区、服务端时间/规则/未知读数、首12条而非原型6条，以及正文片段/分享数/人工备选等全部功能，因此主动保留这些相对简化原型的差异。
- 实际3000与3112同源GET核对：statistics、scope除asOf、works、topics逐值相同；3,274篇、1,516作者、3,269可读；点赞可判定3,237，高表现1,122，未知37单独。只读桥接不更改API事实，不以图表分桶产生第二套官方统计。
- 用户查看副本在当前Codex visualization任务的 `topic-overview-redesign-20261017/formal-b/`；全部实际快照/原图/DOM只留本机private，不进Git/CI。对照属于已选B视觉与既有功能核对，不是与旧版RGB=0。

最终 source whitespace检查、TypeScript/lint/build、78文件834测、最终正式F10步和受控资源清理再次通过；API155/72ignored及全阶段B/D1/D2/E/F已在本轮实跑。第一次全量采集焦点断言失败保留原日志，不通过隐藏/删除测试消除；最后限制2worker完整复核通过。独立复审的修正与最终源码指纹对应。

运行层：2026-10-17收尾main为24e3ec23，runtime-main干净且/health仍82d48a72；3000没有本轮部署。PR仍Draft，原Topic内核PG两项失败/旧head红CI继续如实保留，新head以Checks为准。未启动真实模型/采集，未启用worker或恢复研究，未合并部署；Mog体验签收独立。


## 2026-10-17 · PR #441 内核修复与条件发布验收

Mog 本轮授权修复原两项回归，通过后合并部署并核对实际3000的B方案；本节取代上文“本包不修改内核/不部署”的当前适用限制，历史结果保留。同步main07d6c6ca，仅progress冲突保留双方记录。

修前相同两个隔离target共9项中2项失败；修后9项全通过并清理容器/卷。冻结恢复现同时识别dossier derivativeRef和旧保存jobRef，逐衍生物通过当前处置/退役/分层资格，使用权威完整分页读取的当前全文，不再补查所有历史ACCEPTED层。封面标题沿用dossier的ACCEPTED/PARTIAL资格。OCR合成fixture改用rules-v3和实质行引用，研究task精确选当前producer的derivativeRef，原长文/Unicode尾部/保存重开/同job撤回断言保留；增加新旧身份、未知版本、无实质行和旧层不能借资格的负例。没有放宽生产资格、删除断言或跳过失败。

最新主线集成后前端79文件/838测试、TS/lint/build通过。完整原Topic内核proof、正式topics Web隔离链路和独立只读复核正在运行；当前head CI、合并、部署和实际3000截图仍待后续回执，不能以focused或历史B证明替代。证据位于private/web-react-001-f/kernel-release/。
