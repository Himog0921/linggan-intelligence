# TOPIC-MAP-V41-001：完整主题图谱开发

> 状态: 活跃计划
> 最后核对: 2026-10-09
> 适用范围: TOPIC-MAP-V41-001 单一交付包
> 事实来源: Mog 当前 /goal、主题图谱手册 v1.1、V4.1 Demo、origin/main@5977c7b5 与现有代码
> 冲突时以谁为准: 用户最新确认、真实合同/代码、AGENTS.md；示例不构成数据与外发授权

## 用户结果与范围

按手册 D01–D39 与 Demo V4.1 实现 /topics 全域基础概览、五阶段旅程、我方对照与再做、判断弹窗、后端备选、产品机会说明、研究设置/进度、增量与有界补采。基础能力不被任务逻辑替换。现有 /topics/{key} 暂定工作区深链保留。

用户已明确允许 subagent 并行；本包即当前派定交付包与本地 Claim，不为 Issue 形式另加等待。独立 worktree codex/topic-map-v41 从 origin/main@5977c7b5 建立；共享 checkout 不改。本包允许必要的 Collection/模型/插件源码接点；未授权运行库迁移、真实模型外发、平台采集、插件发布或加载、merge/deploy。

## 读取回执与设计

已读 AGENTS.md、docs/README.md、current-state、全部治理规则、ui-execution-contract、design-governance、LIDS README/tokens/primitives/patterns/materials/shell-zones/data-boundaries/language-policy、scope-001 合同；手册与 Demo 为本次功能/布局来源。LIDS token → button/tab/readout/evidence fragment → topic tree/card/table/dialog → overview/journey page，L2 工作面。

视觉方向：Demo 的暖白阅读面、左侧稳定树、连续表格与作品卡，项目 LIDS 决定颜色/字号/间距。
内容顺序：范围 → 主题身份 → 指标 → 五个内部工作视图 → 材料下钻；旅程为平级入口。
交互方向：显式判断、筛选恢复、键盘弹窗、关闭不取消、仅真实回执显示保存成功；减弱动效。
CSS 策略：独立 topic_map.css，仅消费 lids_tokens.css，不改站级壳层。

## 表面地图

/topics 默认概览；/topics/{key} 旧深链；选域/平台/概览统计窗口与外部参考窗口；主题树及候选；结构与表现/内容格局/来源分布/新方向/我方对照与再做；3:4作品与全材料；比较与全主题×旅程矩阵；五主阶段旅程与跨阶段状态/事件；判断弹窗原文/原声/我方/角度/产品机会；备选列表搜索取回；研究设置/进度/补采/停止。

## 状态词典

unknown ≠ 0，read_failed ≠ empty，partial ≠ invalid；候选定义 ≠ 正式发布；已入库样本 ≠ 市场全部；未配置我方 ≠ 全部未做；粉丝未知 ≠ 低粉；爆款规则未设 ≠ 低粉高赞；我方爆款为手动标记；研究排队/模型已发送/结果接纳分责；保存回执不启动监控；预算未知禁止外发；历史结果继续可读。主阶段包含五阶段与其他，涉及率允许超100%，空格不表示机会。

## 依赖与文件所有权

- map_backend：topic_map 核心 Rust/DTO、迁移0116、PostgreSQL tests、lib exports；canonical Work/Domain/creator/topic 唯一来源。
- research_engine：topic_map_research Rust/DTO、迁移0117、typed 研究输入/输出/预算/模型 worker 与 Pi adapter 有界接点；不重建评论内核/任务平台。
- frontend：topic_map.js/css，消费统一 API，不复制业务统计/事实。
- root：local_web/topic_map.rs 与注册、结构变更 topic_map_structure/0118、API composition、安全 guard、WorkOrder 临时搜索预算映射、fixtures/migration 登记、文档、集成/浏览器验证。
- map_backend 追加：topic_map_collection_search/0119、临时目标隔离、关键词搜索准入、详情冻结、备选来源版本与单事务 canonical reader。
- research_engine 追加：合同/插件新增评论与回复合计30身份预算、搜索精确3轮/180秒预算；已有路径不启用新预算时保持原行为。
- commit_review：独立只读审查及反例验证；审核发现由原文件 owner 修复，root 复核并收敛。
共享变更由 root 集成，agent 不提交。基础模型 lib 由 map_backend 统一 export 研究模块；migration 编号先协调。所有新增源码归已有业务目录，计划/手册/合同/进度在文档书架，索引登记；生成证据只放 /tmp，不入 Git。

## 接入差异与实施

现有 /topics 固定 redirect、暂定 TopicWorkspace 单对象版本/人工裁定及冻结成员；复用长期身份与引用。Domain 采用 peer domains/canonical material usage。既有评论研究有效结果复用；模型配置/Invocation 与 Collection WorkOrder/Lease/Attempt 不复制。新增最小派生引用/我方用途/标记/备选/视图与研究状态，统计后端计算且平台分列。实际 DTO/接点以代码核对结果补入 HTTP 合同。

## 验收矩阵

全域/父/叶主题：统计集合、分页、未知、子分布、来源、作品、比较；旅程主阶段去重分母与涉及率/其他/格子精确集合；我方明确身份与手动标签真实回执；判断≤10独立作品，原文/解释分责与来源访问；备选幂等且跨重开取回且不启动副作用；研究输入幂等/实质变化/预算原子竞争/跨日/有限重试/无工作零调用；补采每轮10新详情、手动新增30评论、停止/关闭恢复；Host/Origin 与引用越界/受限材料负例。focused unit/API tests、隔离 PostgreSQL proof、fmt/workspace/governance、1440桌面与390窄屏浏览器。

## 决策与结束边界

只有重大产品语义冲突、成本/权限扩大或不可逆动作请求用户。技术字段/文件/线程选择不转嫁用户。源码/测试/隔离 proof/浏览器/PR/main/runtime/业务验收分层报告，禁止静态可点击宣称全面完成。源于评论研究/Collection 缺口的必要接点在本包最小补齐，不以 unavailable 占位完成。旧静态 PAGE 与 TOPIC-WORKSPACE-REAL-001 仅继续约束旧深链，不限制此包授权新功能。

## 实际接点与收敛

迁移0116–0119仅作为源文件与隔离fixture登记。新增派生表保存主题绑定、标注、我方用途、手动爆款、查看记录、备选引用、研究预算/Run/Task/Invocation引用、补采轮次/冻结详情集合及结构变更回执，原文继续读取共享Current与不可变来源沿革。Collection临时目标标记research_round，禁止创建长期规则与进入长期目标列表；该标记不承担授权，准入仍核验现有明确授权与执行设备。

真实实现验证按 HTTP → Intelligence → canonical Work/Comment/Domain →既有模型调度或Collection主链推进；不存在静态模拟“新一轮资料”按钮。手册T01–T50按相邻结果合并验证，真实研究语义质量、平台执行与Mog视觉/业务验收保留NOT VERIFIED，不借合成证明替代。

## 开发收口状态

源码与隔离证明完成，用户可审阅整个模块的开发分支；[实施验收](../../design/acceptance/topic-map-v41-001-acceptance.md)记录工作区613、插件308、本包PostgreSQL11项及实际桌面/窄屏和研究说明下载路径。独立提交复审通过。此计划仍保留为活跃交付记录，等待PR集成和Mog前端/业务验收；不把测试、草稿PR或3109合成服务称为main/3000发布。
