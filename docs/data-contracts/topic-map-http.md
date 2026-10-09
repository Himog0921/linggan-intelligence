# Topic Map HTTP 合同

> 状态: 代码事实优先
> 最后核对: 2026-10-09
> 适用范围: TOPIC-MAP-V41-001 主题图谱 read/commands/research/structure/search 接口
> 事实来源: 手册 v1.1、当前 TopicMap DTO 与 Rust HTTP composition
> 冲突时以谁为准: 用户最新确认、当前代码和来源资格；本合同不授权运行库迁移/外发

## 页面与读取

`/topics` 为完整概览；`/topics/{canonical_key}` 旧暂定工作区保留。`/api/local/topic-map` 查询采用 camelCase：domainRef/topicRef/platform/windowDays/referenceWindowDays/overlay/path/includeReference。referenceWindowDays=0表示全部历史；概览windowDays缺省全时间。我方发布历史始终独立于这两个窗口。includeReference必须显式为true才把reference用途纳入概览统计。没有领域选择时先返回领域入口；领域材料集合来自 material_domain_usage，不由前端昵称推断我方或用户诊断。

读取包含 domains/scope/topics/works/statistics/journey/sources/candidates/changes/alternatives/ownCreators/methodVersion/sourceBoundary。Topic附bindingVersion用于导航归属版本竞争；definitionVersion用于内容定义引用。Topic identity/definition 与成员引用复用 TopicWorkspace；作品display/media由共享WorkResource Current读取。statistics由后端基于完整明确范围计算，各平台分列，样本rule未设为null；未知互动不进入中位/P90/规则分母。父主题作品是子集union去重。scope说明总量/读取覆盖/时间口径；作品分页不得替代完整统计。

旅程五主阶段固定 discover_understand/seek_assessment/choose_support/begin_practice/long_term_manage；其他保留跨阶段综合/背景/不明/待分析。主阶段分布每作品单值，涉及率允许跨阶段，matrix以本行主题去重作品为分母。格子引用精确workRefs；零不表示市场空缺。

## 明确写入

`POST /api/local/topic-map/commands` 使用带action的typed enum，unknown字段拒绝。action使用ownCreator/breakout/saveAlternative/bindTopic/performanceRule/viewed，字段idempotencyKey与domainRef；viewed含实际打开的workRefs，不以打开主题树代替阅读。全部成功必须真实提交后给receiptRef/subjectRef/revision/persistedAt；幂等键相同但payload不同冲突，版本冲突不覆盖。

我方身份为显式domain+platform+authorExternalId用途；爆款为canonical work手动标记。备选保留title/angle/说明、topic+definition、方法与作品引用，保存不启动模型/采集/跟踪、不保存第二份原文。读取/导出依据始终复核当前资格。

研究备选给researchResultRef，并且仅指定researchAngleIndex或researchOpportunityIndex之一。前者保存角度，后者保存可编辑的产品研究说明；kind由后端推导为angle/product_research。引用必须来自该接纳结果的相应条目，不能由界面拼造引用。

`GET /api/local/topic-map/alternative?domainRef={uuid}&alternativeRef={uuid}` 返回保存时的定义、说明与不可变来源引用对应的正文/OCR/ASR/评论片段。sourceState为available/version_changed_available/source_unavailable；当前版本变化时仍可读取合格的保存时原版本，并明确显示历史版本。当前Work/Domain/Comment访问限制继续复核，受限时说明/定义与片段隐藏，只保留最小引用及状态。导出每次重新读取此接口核验资格，再在浏览器本地生成有引用的Markdown，不写外部应用。

## 研究

`GET /api/local/topic-map/research?domainRef={uuid}` 返回设置、用量、run/task/result真实进度；`POST /api/local/topic-map/research/commands` 采用action configure/start/pause/resume/stop。configure明确modelConfigRef/dailyTokenLimit/runTokenLimit/automaticEnabled/collectionEnabled；没有具体用量配置不外发。start区分historical/incremental/on_demand。当前prototype按钮/页面读取/筛选/保存不能代替开启授权。

研究输入是canonical source refs+versions+hash与明确fragment；不冻结敏感原文到新账本。typed结果按严格输出版本校验，所有断言引用allowed fragments Unicode start/end，越界/外域/失效来源拒绝。采集/分析/结果接纳状态分责，已发unknown保留预算且不盲重发。日额度与run预算在实际dispatch之前原子核验；费用未知不计0。互动更新不重分析，输入内容/定义/方法实质变化产生新fingerprint，旧结果保留其版本。

## 有界采集与结构变更

`POST /api/local/topic-map/collection/commands` 的 action 为start_collection/deepen_comments/stop_collection，字段requestRef/domainRef，明确topicRef/targetRef/作品及purpose。详情按一轮最多10个新canonical身份冻结；已有合格详情不额外占新身份，失败尝试仍占已预留身份。评论追加使用knownCommentIds+newUniqueLimit=30，根评论、回复和需要补齐的父评论共同计数，重复ID不重复占额度；服务器复核包与本轮预算。关闭弹窗继续原轮；stop显式终止后续准入，已取得材料沿既有资格接纳。

`GET /api/local/topic-map/search?domainRef={uuid}` 返回现有可用keyword/deep_archive授权与轮次；`POST .../search/commands` 使用start_search/freeze_search_details/stop_search。start_search显式给authorizationRef、1–3个keywords与purpose，最多200候选配额按词分配；TaskSpec把工单冻结的临时预算传到实际浏览器执行，单词最多3轮/180秒。freeze_search_details给roundRef与1–10个本轮已接纳候选workRefs，停止搜索并冻结后续详情身份。搜索完成的partial阶段仍可选择详情；旧轮重开复用，newRound=true是明确新开意图，不由窗口重开自动触发。

`POST /api/local/topic-map/structure/preview` 接收requestRef/domainRef/kind(merge|split)/sources(topicRef+definitionRef)/destinations(新名称定义、parentTopicRef、显式members)/rationale。预览给previewHash与影响集合、未分配作品、旧备选保留策略。`POST .../apply` 给plan+previewHash，重新核验版本与当前来源，原子创建新定义、分类、绑定及回执。旧定义/引用保留，旧判断不继承。含子主题的旧节点先用bindTopic调整导航归属，避免把子主题留在隐藏旧节点下。

## 安全与错误

沿用现有local HTTP Host/Origin/Sec-Fetch-Site guard：只允许localhost/127.0.0.1指定端口、同Origin，无CORS开放；所有响应no-store/nosniff。非法参数/JSON 400、不存在404、幂等与版本409、来源/数据库暂不可用503，失败不返回成功回执或伪造空库。guard拒绝403。

已有 `/api/local/work-resources/{ref}?domain={uuid}` 与 `/comments` 是正文/评论安全读取面；主题页只组合引用，不维护第二原文存储。

## 验证边界

隔离PostgreSQL fixture证明写入/幂等/引用/统计/预算；fake provider证明执行状态，不证明真实模型语义质量。源码与HTTP/browser可用、main合并、运行库迁移/3000部署、付费外发、真实采集和Mog业务验收分别证明。
