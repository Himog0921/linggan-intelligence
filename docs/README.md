# 文档总索引

- [OCR 封面标题选择与异常内容拦截](plans/active/ocr-title-quality-454.md)：#454/#464 的区块采用、局部复核与自有库链路验证；生产默认、共享运行与业务签收仍另列。
- [OCR 手动图片文字识别：实施与验收](plans/active/ocr-manual-recognition-454.md)：#454 当前候选；图组手动 OCR、真实持久化百分比、检查器结果与 OCR 模型设置，未合并、未部署。
采集完整交付入口：[COLLECTION-JTBD-001 单次采集](plans/active/collection-single-run-398.md)（Issue #398 / Draft PR #406；替代 #400 的限制，含 UI 与验收矩阵）。[运行与数据合同](data-contracts/collection-single-run-001.md)、[正式 UI 核对](design/changes/collection-single-run-398.md)、[单独 UI 审美审查](design/changes/collection-single-run-398-visual-audit.md)。旧[两页导航](plans/active/collection-jtbd-001.md)保留历史范围。

> 状态: 权威当前
> 最后核对: 2026-09-30
> 适用范围: 全仓库文档导航、权威状态与渐进式披露
> 事实来源: 当前 Git 文件树、`AGENTS.md` 与文档状态头
> 冲突时以谁为准: `AGENTS.md` 和真实代码、合同、测试、运行结果

## 2026-10-22 · COMMENT-WORD-CLOUD-V23（候选）

- [紧凑概览 + 领域词云执行计划](plans/active/comment-wordcloud-v23-20261022.md)：用户批准 V2.3，保留研究与来源边界。
- [V2.3 UI 与词云 API 变更](design/changes/comment-wordcloud-v23-20261022-ui-change-manifest.md)：单轴上方的黑柱/橙红渐变曲线，真实评论按身份去重词频，来源回查。
- [V2.3 验收矩阵](design/acceptance/comment-wordcloud-v23-20261022-acceptance.md)：静态/组件、隔离 PG、真实浏览器与未证边界。



本页是项目的“总书目”。Agent 不应先通读所有材料，而应从这里逐层进入与当前事项直接相关的文件。

协作与风险分级统一读[工作包协作协议](governance/agent-collaboration.md)。业务行为、数据、权限和外部调用先读[安全与领域约束](agents/safety-invariants.md)；该文件保留原根 AGENTS 的不变量。运行状态仅在运行/发布/跨包依赖任务中读取，不作为所有实现的预读门。

- [手动云端 OCR 的人工采用合同](data-contracts/ocr-manual-human-adoption.md)：#454 叠加候选；云端机器预览与人工采用的追加版本边界，不混入 OCR 抽检样本。
- [手动云端 OCR 人工采用实施计划](plans/active/ocr-manual-human-adoption-454.md)：叠加 Draft PR #477 的受控写入口和合成验证范围；不写共享库、不部署。
- [手动云端 OCR 人工采用发布运行手册](runbooks/ocr-manual-human-adoption-release-454.md)：#477→#479 合并后才可按精确 main 授权使用的备份、0145、3000 回读与回退条件；本身不授权发布。
- [UI 变更清单](design/changes/ocr-manual-human-adoption-454-ui-change-manifest.md) / [验收记录](design/acceptance/ocr-manual-human-adoption-454-acceptance.md)：当前作品图组的人工核对表单、真实回执和未证明边界。


- [主题图谱紧凑工作区变更](design/changes/topic-map-compact-382.md)：#382 当前 UI 后续，取代旧工作区布局，不改研究与来源合同。


## 2026-10-21 · AI 设置左栏导航候选

- [AI-SETTINGS-LEFT-NAV-001 变更清单](design/changes/ai-settings-left-nav-001-ui-change-manifest.md)：共享左栏、四视图深链接与无 Hero/刷新控件的候选范围；未合并、未部署。
- [ACC-AI-SETTINGS-LEFT-NAV-001](design/acceptance/ai-settings-left-nav-001-acceptance.md)：聚焦自动检查、八个隔离浏览器视图场景、已知基线导入失败与未证明边界。

## 2026-10-19 · 评论研究紧凑工作台与三级导航候选

[评论研究 UI 最新规格](design/pages/comment-study-productization-001.md) 与 [LIDS Context Bar 已批准例外](design/lids/shell-zones.md)：四视图导航移入面包屑同行、评论读数折叠、批量研究按需出现、筛选单行、评论表格 48px 单行，React 分支待 PR 复审。仅本次 UI 显示改造，不改变研究业务/API/库，未合并、未部署。

## 2026-10-22 · 评论研究与创作者目录 UX 收口

- [变更清单](design/changes/corpus-study-creator-ux-20261022-ui-change-manifest.md)：PR #470 的页面、状态、依赖和非目标边界。
- [发布前验收](design/acceptance/corpus-study-creator-ux-20261022-acceptance.md)：自动检查结果与真实链路、部署、业务验收的待验证边界。
- [评论目录密度与检查器续修清单](design/changes/comment-study-density-inspector-471-ui-change-manifest.md)：表格、统一控件、检查器和概览对齐的增量边界。
- [评论目录密度与检查器验收](design/acceptance/comment-study-density-inspector-471-acceptance.md)：本次浏览器与工程门禁的独立回执。

## 2026-10-19 · WEB-REACT-001 完整收尾包（当前）

#446已合并main@d635718e（c0ab精确head独立复核及全部执行CI通过）；本机3000仍362dad3f。root合并门控未在新发现断言失败后停止，新增的副本标签/双页缩放P2同包续修，发布暂缓；983项前端与67项定向通过，正式浏览器23步通过并清理，最终复核/新CI待完成。17个正式路由及全部子视图已在main归统一React，旧运行界面删除；上线、离线/回退和真实首篇仍须独立证明。入口为[完整收尾矩阵](design/acceptance/web-react-001-closeout.md)，同一#430完整授权持续执行，保留其他工作树未提交资料。

## 网页前端统一为 React · 当前交付入口

- [全站收尾验收矩阵](design/acceptance/web-react-001-closeout.md)：当前 #430 完整收尾包，C/H、全路由/子视图、运行演练与发布。

2026-10-18 核对：正式 3000 与 main 同为 `acdfbb87`；A+B、C 基线、D1/D2、E/F/G 已部署，C 后续 #434 仍为 Draft。当前实施 D3 评论研究四视图，H 待办；分层验收缺口见主计划最新补记。

Mog 2026-10-15 决定所有正式网页统一为一个 React + TypeScript 应用，不留新旧双轨（[DEC-0010](decisions/0010-web-frontend-react.md)）。唯一推进入口是 [WEB-REACT-001 计划](plans/active/web-react-001.md)（Issue #430），它合并了 UI 改版专项 UIUX-CONTENT-WORKBENCH-001（Issue #425，原生前端停止、后端继续，React 归阶段 C）。迁移期间旧页面冻结，只修阻断使用的缺陷；新页面一律用 React。阶段 A+B 已随 PR #431 合并至 main@0a90fd4a，3000 同版本部署和线上只读链路经 #430 核验；阶段 C 已由 Mog 派定并在专属工作树实施，验收记录见 [ACC-WEB-REACT-001-B](design/acceptance/web-react-001-b-acceptance.md)。阶段 D（语料三页）按[分册](plans/active/web-react-001-d-corpus.md)拆为 D1 创作者页与共享外壳、D2 证据库、D3 评论研究（Issue #433）；D1 已随 PR #436 合并至 main@baaf5a51 并上线（验收见 [ACC-WEB-REACT-001-D1](design/acceptance/web-react-001-d1-acceptance.md)，页面规格见 [PAGE-CORPUS-CREATORS-V1-001](design/pages/creators-page.md)）；D2 证据库已随 PR #443 合并并部署，验收见 [ACC-WEB-REACT-001-D2](design/acceptance/web-react-001-d2-acceptance.md)，D3 已按 Mog 2026-10-18 派定开始（保持四视图全部功能、口径和 UI，实际截图核对）。

Mog 2026-10-15 决定所有正式网页统一为一个 React + TypeScript 应用，不留新旧双轨（[DEC-0010](decisions/0010-web-frontend-react.md)）。唯一推进入口是 [WEB-REACT-001 计划](plans/active/web-react-001.md)（Issue #430），它合并了 UI 改版专项 UIUX-CONTENT-WORKBENCH-001（Issue #425，原生前端停止、后端继续，React 归阶段 C）。迁移期间旧页面冻结，只修阻断使用的缺陷；新页面一律用 React。阶段 A+B 已随 PR #431 合并至 main@0a90fd4a，3000 同版本部署和线上只读链路经 #430 核验；阶段 C 首版已随 PR #432 合并；Mog 对其 V3 还原不予验收，当前在独立候选重新落地并补齐本篇材料准备，验收前不合并部署。基础验收记录见 [ACC-WEB-REACT-001-B](design/acceptance/web-react-001-b-acceptance.md)。

## 研究底稿与图片分层第一阶段

- [实施计划](plans/active/research-dossier-001.md)：Issue #420，完整模块实施中。
- [底稿与任务材料合同](data-contracts/research-dossier.md)：本期数据、分层和输入范围。
- [UI 变更清单](design/changes/research-dossier-001-ui-change-manifest.md)：详情、对照和预览。

## 评论研究产品化 · 当前交付入口

Mog 已批准 COMMENT-STUDY-PRODUCTIZATION-001 增量开发，并于 2026-10-02 将当前目标明确为手册 P0–P5 的完整可用页面和真实接口端到端验收；旧 P6–P8 快照及逐 T 形式测试不再阻塞功能开发，自动每日计划仍是下阶段。先读[完整手册入口](runbooks/comment-study-productization-package.md)，再按职责读取其中的八个分册、机器合同与验收台账。当前事实与未验证项以[当前项目账本](audits/comment-study-productization-001/README.md)及实际运行回执为准，不将文档入库视为代码、数据库或 UI 已交付；页面合同见[评论研究产品化 UI 规格](design/pages/comment-study-productization-001.md)。

本包保留当前 clean-study 合法历史，不延用 DEC-0006 的旧 reset 授权；取代旧版五视图与旧启动交互目标规格，不恢复 V1 路径。自动化分册是后续 P6 设计，不是定时外发授权。2026-09-28 的 P0–P8 推进快照见 COMMENT-STUDY-PRODUCTIZATION-001 验收账本；P2 收口的实施与 UI 边界见 [P2 closeout 清单](design/changes/comment-study-p2-closeout-001-ui-change-manifest.md)。

本包分册索引：[执行入口](agents/comment-study-productization-001-handoff.md)、[架构](architecture/comment-study-productization-001.md)、[P0 回执](audits/comment-study-productization-001/p0-execution.md)、[P1 闭环](audits/comment-study-productization-001/p1-closure.md)、[P1 计数与目录](audits/comment-study-productization-001/p1-read-counts.md)、[P1 作品目录](audits/comment-study-productization-001/p1-work-catalog.md)、[数据库合同](data-contracts/comment-study-productization-001.md)、[HTTP 合同](data-contracts/comment-study-http-001.md)、[机器合同](data-contracts/comment-study-productization-001/README.md)、[P3 Run 控制验收](design/acceptance/comment-study-productization-001-p3-control-acceptance.md)、[总体计划](plans/active/comment-study-productization-001.md)、[自动化后续计划](plans/active/comment-study-automation-002.md)、[开发与升级手册](runbooks/comment-study-productization-001.md)、[V3 结果页实施回执（#426）](audits/comment-study-productization-001/results-v3-426.md)。

## 固定阅读顺序

1. 根目录 [`AGENTS.md`](../AGENTS.md)：最高约束、事实优先级和不可违反规则。
2. [`current-state.md`](current-state.md)：当前阶段、唯一在办事项、阻塞和下一步。
3. 本页对应分类中的权威文档。
4. 只有当前任务明确需要来源核对时，才进入 [`references/README.md`](../references/README.md)。

- [采集读取性能](plans/active/collection-read-performance-001.md)：当前授权的性能候选；保留统计、来源资格和 Lease，不代表正式部署。

## UI/UX 内容工作台专项

- [V3 空间材料浏览与作品关联原型](design/pages/content-workbench-prototype/v3/index.html)：纸条筛选、翻页定位、分布索引，片段一次用于正文和指定内页；独立演示。
- [V2 材料片段与改稿画布原型](design/pages/content-workbench-prototype/v2/index.html)：本篇密集研读、多来源片段、创作编排与改稿对照；独立演示。

- [阶段二三页原型](design/pages/content-workbench-prototype/index.html) · [设计提案](design/changes/uiux-content-workbench-001-prototype.md) · [原型验收](design/acceptance/uiux-content-workbench-001-prototype-acceptance.md)：独立演示；V3 方向已获 Mog 认可，说明精简已回填，业务未接入。

- [UI/UX 原专项与衔接记录](archive/plans/uiux-content-workbench-001.md)：用户结果、V3、S0–S6 与 A01–A13 的历史来源；推进已并入 [WEB-REACT-001](plans/active/web-react-001.md)。#425 后端继续，原生前端暂停且不合入，React 归阶段 C。
- [第一阶段真实页面与代码审计](audits/uiux-content-workbench-baseline-2026-10-14.md)：11 项能力现状、21 个问题及优先级，下一阶段的模块顺序。
- [页面覆盖与证据附录](audits/uiux-content-workbench-evidence-2026-10-14.md)：51 组有效状态证据、源码定位、可复现路径及未验证项。

## 当前权威与状态

| 文档 | 状态 | 用途 |
|---|---|---|
| [design/acceptance/web-react-001-b-acceptance.md](design/acceptance/web-react-001-b-acceptance.md) | 权威当前；已合并部署，Mog 待验收 | WEB-REACT-001 阶段 B 的场景矩阵、隔离真实链路浏览器证明结果与分层结论；迁移中修正的旧设置页缺陷 |
| [design/acceptance/web-react-001-g-acceptance.md](design/acceptance/web-react-001-g-acceptance.md) | 一次性报告；已合并部署，Mog 待验收 | 采集四页、全部抽屉/弹窗迁移，JSON 回执边界、实际截图与隔离证明；已随 #437 合并部署 |
| [design/acceptance/web-react-001-c-acceptance.md](design/acceptance/web-react-001-c-acceptance.md) | 一次性报告；候选验证中 | 阶段 C React V3 工作台、材料到成品、独立审核与未证明边界 |
| [design/acceptance/web-react-001-d1-acceptance.md](design/acceptance/web-react-001-d1-acceptance.md) | 一次性报告；已合并上线（PR #436，`baaf5a51`），Mog 待验收 | 阶段 D1 创作者页与语料共享外壳：场景矩阵、隔离真实链路 17 步浏览器证明、分层结论与未证明边界 |
| [design/acceptance/web-react-001-d2-acceptance.md](design/acceptance/web-react-001-d2-acceptance.md) | 一次性报告；已随 #443 合并部署，Mog 待验收 | 阶段 D2 证据库：场景矩阵、隔离真实链路 19 步浏览器证明与插件入口证明、失效控件实测、分层结论与未证明边界 |
| [design/acceptance/web-react-001-d3-acceptance.md](design/acceptance/web-react-001-d3-acceptance.md) | 一次性报告；D3已随#445合并部署，业务验收待Mog | 评论研究四视图、实际页面对照、隔离浏览器与队列/回执证明、未合并部署边界 |
| [design/pages/creators-page.md](design/pages/creators-page.md) | 权威当前；React 实现已上线（`baaf5a51`） | `/corpus/creators` 的页面责任、状态词典、动作与真实后果、LIDS 采用清单 |
| [decisions/0010-web-frontend-react.md](decisions/0010-web-frontend-react.md) | 权威当前 | 网页前端统一为 React + TypeScript、不留双轨、开发期数据不构成迁移约束；所取代的「原生 JS / 不引入框架」条款 |
| [plans/active/web-react-001.md](plans/active/web-react-001.md) | 活跃计划；Issue #430；阶段 A+B 随 PR #431 合并并按 #430 回执部署；阶段 C 已派定 | 全站 React 迁移与 UI 改版的唯一推进入口：目标架构、R1–R7 不留双轨规则、阶段 A–H、内容工作台首包规格 |
| [plans/active/web-react-001-d-corpus.md](plans/active/web-react-001-d-corpus.md) | 活跃计划；Issue #433（#430 阶段 D 分册）；D1 已上线；D2/D3 已部署，业务验收待Mog | 语料三页迁 React：拆分与顺序、已定设计改动、D1 与 D2 能力清单（旧页能力逐项落点）、D2 裁定、回执与语料外壳接口合同 |
| [plans/active/web-react-001-g-collection.md](plans/active/web-react-001-g-collection.md) | 活跃计划；#430 阶段 G；Mog 2026-10-16 派定 | 整个采集模块 React 迁移；能力盘点、实际页面截图基线、逐表面对照与隔离回执验证 |
| [plans/active/topic-map-workflow-382.md](plans/active/topic-map-workflow-382.md) | 活跃计划 | 左侧双入口、内容优先、统一检索、状态与人工备选闭环；迁移/部署另确认 |
| [plans/active/topic-map-read-performance-382.md](plans/active/topic-map-read-performance-382.md) | PR #389历史已发布；10月13日首屏候选实现/实测通过，冷重建仍慢，未提交/发布；Issue #382 | 进入首屏542ms/完整1268ms，冷重建约5–8秒；完整统计和来源资格不放宽 |
| [plans/active/comment-study-read-performance-358.md](plans/active/comment-study-read-performance-358.md) | 活跃计划；Issue #358；独立工作树 | 评论读取性能及概览进入的加载依赖；PR #380 已发布，整页进入修复为后续候选 |
| [plans/active/comment-like-library-001.md](plans/active/comment-like-library-001.md) | 活跃计划；Issue #397；候选完成，待 Mog 授权合并/迁移/部署 | 高赞原话库：评论点赞观测、按赞排序/筛选、改帖标记命令与界面 |
| [plans/active/corpus-performance-089.md](plans/active/corpus-performance-089.md) | 活跃计划；Issue #89；受保护工作树 | 共享作品读取提前收窄、批次 enrichment、语义等价和数据库查询次数性能防护 |
| [design/changes/collection-runtime-state-20260929-ui-change-manifest.md](design/changes/collection-runtime-state-20260929-ui-change-manifest.md) | 候选实现；待集成与页面验收 | recovery 全局暂停、当前排队读数和派发回答的状态纠偏 |
| [design/changes/collection-keyword-sampling-recovery-001-ui-change-manifest.md](design/changes/collection-keyword-sampling-recovery-001-ui-change-manifest.md) | Issue #362 交付分支实施中 | 关键词 3 次下拉／200 候选／20 样本、失败有界恢复、部分巡查和容量等待的状态词义 |
| [design/changes/creator-gap-request-20260929-ui-change-manifest.md](design/changes/creator-gap-request-20260929-ui-change-manifest.md) | 活跃计划；候选实现未集成 | 创作者缺详情/评论补采、巡查新增自动跟进条件、真实失败回执与未知来源边界 |
| [plans/active/domain-unification-001.md](plans/active/domain-unification-001.md) | 活跃计划；#343/#344 已合并，全部领域作品读取与原包补投影为隔离候选、尚未写共享库或刷新运行；#338 后续适配，Mog 业务验收待办 | 多个正式 Domain 平权、Domain–Target role、统一 Material/Comment/Media、Comment Study 领域化、领域管理与开发期旧 cross 模型清理的唯一推进文件 |
| [plans/active/account-observation-bootstrap-002.md](plans/active/account-observation-bootstrap-002.md) | 活跃计划 | Issue #218：新安装插件的首单软准入、账号观察启动、人工绑定审计与无秘密配置诊断 |
| [plans/active/detail-page-session-replay-safety-001.md](plans/active/detail-page-session-replay-safety-001.md) | 活跃计划；受保护工作树 | 同一 Work Order/内容详情页的单次导航授权、浏览器持久消费闸门、缓存缺失 fail-closed 与既得数据重传边界 |
| [plans/active/collection-upgrade-001.md](plans/active/collection-upgrade-001.md) | 活跃计划；受保护工作树 | Mog 交付包：缺输入止转、跨工单重试预算、交付恢复、统一材料完成判据与运行诊断的 S0–S6 对照表 |
| [plans/active/corpus-evidence-read-recovery-001.md](plans/active/corpus-evidence-read-recovery-001.md) | 活跃计划；Issue #308 已关闭；main/本机 loopback 已部署 | Comment Study 重建后恢复 Evidence 列表与受控评论读取，不复活 V1 研究合同 |
| [plans/active/ocr-content-layering-001.md](plans/active/ocr-content-layering-001.md) | 活跃计划；Issue #296；受保护工作树 | PaddleOCR 替换、逐行版面/内容分层、封面标题补位与 Tesseract OCR 可审计退役 |
| [decisions/0007-ocr-content-layering.md](decisions/0007-ocr-content-layering.md) | 权威当前 | OCR 原始事实、内容分层、视觉选择边界和展示标题来源的长期决定 |
| [decisions/0008-peer-domains-and-unified-material-pipeline.md](decisions/0008-peer-domains-and-unified-material-pipeline.md) | 权威当前 | 平级 Domain、共享真实 Target、primary/reference 用途、统一材料链与 Domain pause 的长期决定 |
| [decisions/0009-stage-one-job-topic-pool.md](decisions/0009-stage-one-job-topic-pool.md) | 权威当前 | 第一阶段主 Job：小红书选题池经营（复用与试新、爆款标准、成功指标、升级条件）、运营模块整体迁入与先后顺序；所取代的旧条款与待裁定事项 |
| [plans/active/ops-launch-001.md](plans/active/ops-launch-001.md) | 活跃计划；Issue #483；受保护工作树实施中 | 内部团队邀请制登录、原生运营写入、已登记发布的精确同步、交接停用及 React 页面收口；尚未推送、合并或部署 |
| [data-contracts/ops-auth-001.md](data-contracts/ops-auth-001.md) | 权威当前；认证切片实现中 | 内部邀请成员、Argon2id 密码哈希、会话 cookie、同源写入边界与机器身份分离 |
| [data-contracts/ops-team-responsibility-001.md](data-contracts/ops-team-responsibility-001.md) | 权威当前；原生写权限结构基础 | 团队组、追加式成员归属、账号当前负责人和责任变更回执；不替代后续服务端授权 |

> **运营文档状态提示（2026-10-23）：** 下列 OPS-MIGRATION、PAGE-OPS、OPS-LEDGER 与 OPS-HTTP 条目保留 D1–D3 的历史迁入、读取和上线证据；其中声称“权威当前”或规定 D4–D6 后续路线的部分，均受 OPS-LAUNCH-001 的优先规则限制。新认证、原生写入、同步及交接合同会以独立的当前文档加入本索引，不能由旧条目的本机守卫或只读约束推导。
| [plans/active/ops-module-migration-001.md](plans/active/ops-module-migration-001.md) | 历史迁入计划；D1–D3 的事实仍保留，D4–D6 已由 OPS-LAUNCH-001 取代 | 内容工作台运营管理迁入的历史边界、导入与只读证据；不再规定原生可写运营的未来路线 |
| [design/pages/ops-page.md](design/pages/ops-page.md) | 权威当前；Issue #405，Mog 2026-10-13 确认，随 PR #407 上线（`main@9e45e01c`） | PAGE-OPS-001：一级页面「运营」的壳层与左侧二级页面，发布记录、账号管理、每日数据、导入导出的只读呈现、状态词典、与工作台的预期差异和验收矩阵 |
| [design/pages/ops-overview-page.md](design/pages/ops-overview-page.md) | 权威当前；Issue #409，Mog 2026-10-13 确认，随 PR #415 已上线，业务验收待 Mog；Issue #417 三处小修随 #418 已上线，读屏跟进为候选 | PAGE-OPS-002：运营「总览」五块驾驶舱、周与月加微目标的设定与修改记录、每日数据执行状态的新口径、与工作台的差异和验收矩阵 |
| [data-contracts/ops-ledger-001.md](data-contracts/ops-ledger-001.md) | 权威当前；随 PR #403 上线（`main@3029efff`） | 运营台账五张表的含义、内容工作台导出的导入规则 `ops-import.v1`（校验、重复登记合并、阅读量填报、消失记录标记、预览指纹）、新旧对账口径与 D2/D3 读取约定 |
| [data-contracts/ops-http-001.md](data-contracts/ops-http-001.md) | 权威当前；D2/D3 随 PR #407/#415 上线；Issue #417 三处修复随 #418 已上线，读屏跟进为候选 | 运营读取与本机目标设定接口：业务选择、记录/账号/每日数据/导出/封面、总览覆盖与趋势未知、目标周期及回执 |
| [design/acceptance/ops-page-001-visual-acceptance.md](design/acceptance/ops-page-001-visual-acceptance.md) | 一次性报告；已上线，Mog 业务验收待做 | ACC-OPS-PAGE-001：运营页面在共享库副本上的走查、四类数据边界、失败与快速切换、共享页头放下第 6 项及 13 处修正 |
| [design/acceptance/ops-page-002-visual-acceptance.md](design/acceptance/ops-page-002-visual-acceptance.md) | 一次性报告；D3 已上线，业务验收待 Mog | ACC-OPS-PAGE-002：运营总览隔离走查、D3 上线回执与分层证明、Issue #417 合并后修复验证 |
| [data-contracts/ocr-content-layering.md](data-contracts/ocr-content-layering.md) | 权威当前 | OCR raw/layout/行引用、分层状态、标题回退与视觉模型输出闭集 |
| [runbooks/paddle-ocr-runtime.md](runbooks/paddle-ocr-runtime.md) | 权威当前 | 本机 Paddle runtime 准备、无 Tesseract 回退、迁移后受控重排步骤 |
| [decisions/0006-comment-research-clean-rebuild.md](decisions/0006-comment-research-clean-rebuild.md) | 权威当前 | Issue #295：开发期销毁旧评论研究派生层，保留原始 Evidence 与资格事实；唯一的新评论研究合同与本机 reset 边界 |
| [plans/active/comment-research-rebuild-001.md](plans/active/comment-research-rebuild-001.md) | 活跃计划；Issue #295；实现中 | 按领域选择笔记、冻结同篇语境与评论目标、严格语义接纳、长期 Problem 归并及旧路径删除的交付顺序 |
| [plans/active/comment-study-p3-quality-gate-010.md](plans/active/comment-study-p3-quality-gate-010.md) | 活跃计划；Issue #316；受保护工作树 | P3 候选的主线整合准备、Gold Set/Recall@K 协议與无真实评论/模型调用的验证边界 |
| [plans/active/model-secret-dev-001.md](plans/active/model-secret-dev-001.md) | 活跃计划；Issue #354；已合并部署，真实调用待验 | 开发期模型凭据单一文件存储、旧 Keychain 凭据清理与运行核验 |
| [plans/active/comment-research-reset-001.md](plans/active/comment-research-reset-001.md) | 历史基础 | 旧评论研究 reset 的历史记录；不再规定当前 schema、Run、输入或 UI 合同 |
| [plans/active/comment-research-v1-real-closure-001.md](plans/active/comment-research-v1-real-closure-001.md) | 历史基线 | V1 Derivation/Run 运行记录；不得作为新实现或兼容路径 |
| [plans/active/comment-research-semantic-output-001.md](plans/active/comment-research-semantic-output-001.md) | 历史基线 | V1 结构化输出修复记录；新输出合同由 COMMENT-STUDY-REBUILD-001 单独定义 |
| [plans/active/comment-research-output-diagnostics-001.md](plans/active/comment-research-output-diagnostics-001.md) | 历史基线 | V1 输出诊断记录；不得重新接入新路径 |
| [plans/active/comment-research-partial-publication-001.md](plans/active/comment-research-partial-publication-001.md) | 历史基线 | V1 发布合同记录；新读取和发布在替换阶段另行实现 |
| [plans/active/comment-research-cumulative-state-001.md](plans/active/comment-research-cumulative-state-001.md) | 历史基线 | V1 累计状态记录；不迁移其历史研究结果 |
| [plans/active/comment-research-problem-resolution-v2.md](plans/active/comment-research-problem-resolution-v2.md) | 历史基线 | 长期 Problem 的部分业务原则可审计参考；不复用其表、worker 或合同 |
| [plans/active/comment-research-window-001.md](plans/active/comment-research-window-001.md) | 活跃计划；Issue #267；#281 已收束概览/问题主体 | 当前统计窗口无样本时说明基线与不可比；累计问题读取不再使用窗口统计 |
| [plans/active/comment-research-pagination-001.md](plans/active/comment-research-pagination-001.md) | 活跃计划；Issue #269 | 评论研究三张增长型列表的有界分页浏览与 URL 页码保持 |
| [plans/active/aud-code-quality-20260914.md](plans/active/aud-code-quality-20260914.md) | 活跃计划；Issue #279 | 恢复任务所有权、历史详情可达、退役材料读取、目标问题入口与可信检查器；不触碰生产运行 |
| [decisions/0003-comment-research-single-semantic-kernel.md](decisions/0003-comment-research-single-semantic-kernel.md) | 历史基础 | 旧评论语义路径的退役记录；其表和运行合同由 DEC-0006 取代 |
| [decisions/0004-comment-research-problem-resolution-v2.md](decisions/0004-comment-research-problem-resolution-v2.md) | 历史基础 | 稳定 Problem 的设计沿革；新实现只继承已在 DEC-0006/当前计划明确的业务不变量 |
| [decisions/0005-lids-material-06-paper-residue.md](decisions/0005-lids-material-06-paper-residue.md) | 权威当前 | LIDS 第七种材料 `M-06` 纸面残留的格点归属、密度上限与允许区位 |
| [design/pages/comment-research-v1-page.md](design/pages/comment-research-v1-page.md) | 权威当前；交付分支实现 | 评论研究 V1 的五视图、研究设置与用户状态合同 |
| [design/pages/comment-study-rebuild-page.md](design/pages/comment-study-rebuild-page.md) | 权威当前；受保护工作树 | 评论研究重建后的 L1 读取职责、状态与验收边界；不复用 V1 结果版本或向量语义 |
| [design/changes/comment-research-cumulative-state-001-ui-change-manifest.md](design/changes/comment-research-cumulative-state-001-ui-change-manifest.md) | 权威当前；实施中 | Issue #281 的累计已确认知识、统计版本与 Run Health 状态表达 |
| [design/changes/ocr-content-layering-001-ui-change-manifest.md](design/changes/ocr-content-layering-001-ui-change-manifest.md) | 活跃计划；Issue #296 | Evidence Library 的封面 OCR 标记、标题来源与图片文字可追溯读取边界 |
| [design/changes/comment-research-reset-001-ui-change-manifest.md](design/changes/comment-research-reset-001-ui-change-manifest.md) | 权威当前；交付分支实现 | 旧评论页面/API/自动计划替代与用户体验验证范围 |
| [design/changes/comment-study-rebuild-001-ui-change-manifest.md](design/changes/comment-study-rebuild-001-ui-change-manifest.md) | 权威当前；受保护工作树 | 评论研究 clean rebuild 的 API/UI 替换范围、状态词典、依赖与验收矩阵 |
| [design/changes/comment-study-release-polish-358-ui-change-manifest.md](design/changes/comment-study-release-polish-358-ui-change-manifest.md) | 活跃计划；Issue #358 | 评论研究页面状态、运行数字解释、九类标签与发起／结果布局的范围 |
| [design/acceptance/comment-study-release-polish-358-acceptance.md](design/acceptance/comment-study-release-polish-358-acceptance.md) | 一次性报告；分阶段验收 | 已发布阶段与最新候选浏览器验收分开记录；当前九类标签候选尚未合并部署 |
| [design/acceptance/comment-study-layout-001-acceptance.md](design/acceptance/comment-study-layout-001-acceptance.md) | 一次性报告；候选布局 | Issue #295 初始研究输入面的表格布局、实际 100 篇候选读取与浏览器验证；不代表合并或 3000 刷新 |
| [design/acceptance/comment-study-tabs-001-acceptance.md](design/acceptance/comment-study-tabs-001-acceptance.md) | 一次性报告；候选实现 | Issue #295 补齐概览/评论目标/待归并/用户问题/运行记录 5 个复核 Tab，含原声受限时如实降级；不代表合并或 3000 刷新 |
| [design/changes/comment-research-problem-resolution-v2-ui-change-manifest.md](design/changes/comment-research-problem-resolution-v2-ui-change-manifest.md) | 权威当前 | Issue #285 的 confirmed/deferred 问题读取、受限原声边界与只读详情 |
| [design/acceptance/comment-research-reset-001-acceptance.md](design/acceptance/comment-research-reset-001-acceptance.md) | 一次性验收记录；进行中 | V1 数据库、worker、UI、runtime 与用户验收边界 |
| [archive/ci-auto-004-comment-auto-research.md](archive/ci-auto-004-comment-auto-research.md) | 历史归档 | 既有 P0–P5 与本机发布证据；其未来语义、聚类、读取和页面路径已被 COMMENT-RESEARCH-RESET-001 替代 |
| [archive/ci-auto-004-execution-contracts.md](archive/ci-auto-004-execution-contracts.md) | 历史归档 | 既有数据/API/验收证据；不再规定新评论研究内核 |
| [design/changes/ci-auto-004-ui-change-manifest.md](design/changes/ci-auto-004-ui-change-manifest.md) | 历史实施记录 | 既有 UI 状态证据；新用户路径由 COMMENT-RESEARCH-RESET-001 重新定义 |
| [data-contracts/comment-research-automation.md](data-contracts/comment-research-automation.md) | 历史基线 | 保留自动清洗、权限、版本与调用账本基础；旧问题/向量/变化合同不再规定未来实现 |
| [plans/active/target-inspector-performance-001.md](plans/active/target-inspector-performance-001.md) | 活跃计划 | Issue #158 目标列表、统一状态投影、检查器与作品表现实施合同 |
| [plans/active/keyword-author-evidence-001.md](plans/active/keyword-author-evidence-001.md) | 活跃计划；Issue #369 隔离候选 | 关键词命中作品作者归属、主页链接观察与头像媒体证据边界，含 UI 变更清单和验收矩阵 |
| [plans/active/target-deletion-author-attribution-001.md](plans/active/target-deletion-author-attribution-001.md) | 活跃计划 | Issue #214 安全撤回观察目标、独立作者归属 projection 与本机发布边界 |
| [design/changes/target-deletion-author-attribution-001-ui-change-manifest.md](design/changes/target-deletion-author-attribution-001-ui-change-manifest.md) | 活跃计划 | Issue #214 的 Targets 行级停止/恢复、删除确认、保留事实与回执清单 |
| [design/changes/target-inspector-performance-001-ui-change-manifest.md](design/changes/target-inspector-performance-001-ui-change-manifest.md) | 权威当前 | 目标检查器的信息架构、LIDS 与状态诚实性变更清单 |
| [design/acceptance/target-inspector-performance-001-acceptance.md](design/acceptance/target-inspector-performance-001-acceptance.md) | 一次性报告 | 自动、隔离 PostgreSQL、浏览器与未部署边界的分层验收 |
| [design/changes/collection-action-001-ui-change-manifest.md](design/changes/collection-action-001-ui-change-manifest.md) | 交付分支实施中；Issue #283 | keyword Drawer 与列表行主操作一致性、检查器读取失败边界与验收矩阵 |
| [design/acceptance/collection-action-001-acceptance.md](design/acceptance/collection-action-001-acceptance.md) | 交付分支验收记录；Issue #283 | keyword Drawer 主操作一致性的自动、视觉与真实后果分层边界 |
| [design/changes/keyword-archive-002-ui-change-manifest.md](design/changes/keyword-archive-002-ui-change-manifest.md) | 交付分支实施中；Issue #286 | 未建档关键词的建档入口可达性、两段路由与回执层级，含停止条件与未证明边界 |
| [design/acceptance/keyword-archive-002-acceptance.md](design/acceptance/keyword-archive-002-acceptance.md) | 交付分支验收记录；Issue #286 | 建档入口、回执真实性、层叠复核与隔离 PostgreSQL 真实后果的分层边界 |
| [design/changes/runtime-station-table-001-ui-change-manifest.md](design/changes/runtime-station-table-001-ui-change-manifest.md) | 权威当前 | 执行工位改为以工位为单位的管理台：工位总表、语言收敛与页头计数口径 |
| [design/changes/runtime-station-v7-2-001-ui-change-manifest.md](design/changes/runtime-station-v7-2-001-ui-change-manifest.md) | 交付分支实施完成、验证通过；未推送未部署；Issue #309 | 执行工位内容区按 v7.2 稿复刻、运行概览抽屉，及稿中无源数值的逐条替代与对稿偏离 |
| [archive/model-pi-001.md](archive/model-pi-001.md) | 历史归档 | Issue #169 的模型设置/Pi 基础能力历史记录；当前调用路径由 COMMENT-RESEARCH-RESET-001 定义 |
| [pages/model-ai-settings.md](pages/model-ai-settings.md) | 权威当前；交付分支 | 单页供应商弹窗、调用限制与用量语义 |
| [design/pages/model-ai-settings-page.md](design/pages/model-ai-settings-page.md) | 权威当前；交付分支 | L1 模型与 AI 页面/状态/交互规格 |
| [design/changes/model-pi-001-ui-change-manifest.md](design/changes/model-pi-001-ui-change-manifest.md) | 历史归档；#169 交付分支 | 当时模型设置及个人菜单的授权、共享接缝和验证矩阵；当前密钥合同见模型运行说明 |
| [design/acceptance/model-pi-001-acceptance.md](design/acceptance/model-pi-001-acceptance.md) | 一次性报告 | 真实 SDK/隔离 PG/HTTP/Keychain 与未证实层 |
| [runbooks/model-pi-runtime.md](runbooks/model-pi-runtime.md) | 权威当前；交付分支 | 固定 Node/lock、worker、隔离预览与真实试验前提 |
| [archive/comment-research-001.md](archive/comment-research-001.md) | 历史归档 | Issue #167 的首包与模型接入证据；后续评论研究架构由 COMMENT-RESEARCH-RESET-001 规定 |
| [pages/comment-research.md](pages/comment-research.md) | 历史实施记录 | 旧评论研究的页面语义与入口；后续用户结果由 COMMENT-RESEARCH-RESET-001 定义 |
| [design/pages/comment-research-page.md](design/pages/comment-research-page.md) | 历史归档 | Issue #167 的 L1 Corpus Explorer 规格；不再规定新评论研究页面 |
| [design/changes/comment-research-001-ui-change-manifest.md](design/changes/comment-research-001-ui-change-manifest.md) | 历史归档 | Issue #167 的 UI 变更与验收边界；后续 UI 由 COMMENT-RESEARCH-RESET-001 定义 |
| [design/acceptance/comment-research-001-acceptance.md](design/acceptance/comment-research-001-acceptance.md) | 一次性报告 | 本包自动、隔离 PostgreSQL/API/UI 与真实模型的分层验收记录 |
| [`current-state.md`](current-state.md) | 权威当前 | 当前阶段、事项队列和决策缺口 |
| [`development-stage-tracker.md`](development-stage-tracker.md) | 权威当前 | 从开发基线到真实运行与业务验收的全项目阶段总表、阶段证据和 Mog 跟进入口 |
| [`governance/file-placement-standard.md`](governance/file-placement-standard.md) | 权威当前 | 文件分类、命名、放置、归档与生成物规则 |
| [`governance/agent-collaboration.md`](governance/agent-collaboration.md) | 权威当前 | Agent 读取、任务、冲突、更新和验收流程 |
| [`governance/generated-artifacts-registry.md`](governance/generated-artifacts-registry.md) | 权威当前 | 所有生成型文件的固定位置与入库许可 |
| [`agents/issue-tracker.md`](agents/issue-tracker.md) | 权威当前 | GitHub Issues 的 Agent 任务追踪与授权边界 |
| [`agents/triage-labels.md`](agents/triage-labels.md) | 权威当前 | GitHub Issue 的五类任务分流角色 |
| [`agents/domain.md`](agents/domain.md) | 权威当前 | 工程技能读取 Linggan 领域语言、决策与不变量的适配规则 |
| [`agents/scope-001-execution-contract.md`](agents/scope-001-execution-contract.md) | 权威当前 | SCOPE-001 的 Closed World、状态分责、unknown、语义 Oracle 与证明边界执行合同 |
| [agents/ui-execution-contract.md](agents/ui-execution-contract.md) | 权威当前 | 所有 UI 协作 Agent 的读取、闭集执行、停工、验收与交接合同；不授予 UI 实现范围 |
| [`context/START-HERE.md`](context/START-HERE.md) | 权威当前 | 新机器和新 Agent 的项目背景入口 |
| [`context/current-system-inventory.md`](context/current-system-inventory.md) | 代码事实优先 | 固定来源资产及已证实能力盘点 |
| [`context/discussion-decisions.md`](context/discussion-decisions.md) | 权威当前 | 已确认结论和待决定事项摘要 |
| [`context/domain-language.md`](context/domain-language.md) | 权威当前 | DISC-001 已确认的产品、领域与采集责任共同语言；不预设数据库对象 |
| [`decisions/0001-greenfield-rust-clean-db.md`](decisions/0001-greenfield-rust-clean-db.md) | 权威当前 | 已接受的 Rust 与全新 PostgreSQL 决策 |
| [`product/PRD.md`](product/PRD.md) | 草案 | DISC-001 的产品输入，不是已接受实现合同 |
| [`product/domain-invariants.md`](product/domain-invariants.md) | 权威当前 | 跨 Gate 的领域不变量、判断资格与持续扩展的对抗性验收案例 |
| [`product/collection-monitoring-rules.md`](product/collection-monitoring-rules.md) | 权威当前 | 博主/关键词监控的产品规则：目标生命周期、深度建档、频率、爆款追踪；含 4 项待 Mog 决定 |
| [`plans/completed/observation-runtime-001-implementation-manual.md`](plans/completed/observation-runtime-001-implementation-manual.md) | 已完成；Issue #94 / PR #95 / PR #96 | 观察规则自动调度、插件无人领取、Package/Receipt、封面材料投影、发布与一次性验收的实施规格和已完成边界 |
| [`data-contracts/local-001-discovery-evidence-boundary.md`](data-contracts/local-001-discovery-evidence-boundary.md) | 权威当前 | `LOCAL-001 / 001C-0` 与 #34 localhost binding：平台 discovery 与本地 Evidence Library 检索、partial Coverage、DiscoveryOccurrence、接纳回执与本地封面引用的跨边界合同；不证明真实插件或媒体取得 |
| [`architecture/material-projection-data-map.md`](architecture/material-projection-data-map.md) | 权威当前；MATERIAL-PROJECTION-001 | 最新 Browser Producer 产出到服务接纳、作品级材料投影与 Evidence 列表/详情受控媒体读取的逐 lane 映射及代表性 fixture |
| [`architecture/work-resource-read-contract.md`](architecture/work-resource-read-contract.md) | 权威当前；Issue #110 / #128 | Intelligence 跨页面唯一作品资源读取入口；`0.8.28` 真实链已验收封面/头像/作者-目标分责，评论图片非空真实样本仍未观察 |
| [`architecture/topic-workspace-contract.md`](architecture/topic-workspace-contract.md) | 验证中；Issue #112 | 暂定 Topic Definition、人工 Classification Run、冻结 Material Pack、版本/幂等与 Agent 准入边界；不复制 Evidence 原文，不形成正式 Topic、Claim 或趋势 |
| [design/README.md](design/README.md) | 权威当前 | 前端设计手册入口、权威地图、读取路径和当前已建立/未建立边界 |
| [design/design-governance.md](design/design-governance.md) | 权威当前 | UI 设计来源优先级、闭集执行、变更分类、例外和验收治理 |
| [design/reference-register.md](design/reference-register.md) | 权威当前 | 设计参考的登记格式与“参考不等于指令”边界；LIDS v2.0 的可继承/不继承边界已登记 |
| [design/lids/README.md](design/lids/README.md) | 权威当前；Proposed | LIDS v7.0 全项目设计表达标准的入口、来源回执、权威边界和成熟度 |
| [design/lids/system.md](design/lids/system.md) | 权威当前；Proposed | LIDS-SYS-001：Token→Primitive→Component→Pattern→Page、L1/L2/L3、状态、动效、场景、a11y 与治理 |
| [design/lids/tokens.md](design/lids/tokens.md) | 权威当前 | LIDS-TOK-001：v7 三层 Token 架构（目标态）、完整 `--lgi-*` 运行时基线、差异表与分步迁移顺序 |
| [design/lids/materials.md](design/lids/materials.md) | 权威当前 | LIDS-MAT-001：8px 采样点阵六态、材料预算 70/20/10 与区位限制 |
| [design/lids/shell-zones.md](design/lids/shell-zones.md) | 权威当前 | LIDS-SHELL-001：页头 3 区、Context Bar 4 区的容量上限、禁止清单与静默区保护 |
| [design/lids/data-boundaries.md](design/lids/data-boundaries.md) | 权威当前 | LIDS-BOUND-001：超长/缺失/零值与极大值/未知四态的渲染契约，组件准入条件 |
| [design/lids/decisions.md](design/lids/decisions.md) | 权威当前 | LIDS-ADR-001：v7 全部结构性决定台账，规则状态与运行时状态分列 |
| [design/lids/primitives.md](design/lids/primitives.md) | 权威当前 | LIDS-PRI-001：文字、按钮、状态、Surface、Readout、反馈和 Focus 的基础契约 |
| [design/lids/patterns.md](design/lids/patterns.md) | 权威当前 | LIDS-PAT-001：L1/L2/L3 页面 Pattern、结构与组合限制 |
| [design/lids/agent-execution-guide.md](design/lids/agent-execution-guide.md) | 权威当前 | LIDS-AGENT-001：UI Agent 强制决策树、禁止项和交付前检查 |
| [design/lids/language-policy.md](design/lids/language-policy.md) | 权威当前 | LIDS-LANG-001：中文主表达、英文 Mono 三类预算（LANG-05）、状态诚实性和有界页面迁移规则 |
| [design/lids/prototype-audit.md](design/lids/prototype-audit.md) | 权威当前 | LIDS-AUD-001：Observatory V3 原型的品牌母题与非授权边界 |
| [design/lids/migration-log.md](design/lids/migration-log.md) | 权威当前 | LIDS-LOG-001：设计系统迁移、替代、例外与验证记录 |
| [design/templates/page-spec-form.md](design/templates/page-spec-form.md) | 权威当前 | 获准页面的设计执行规格表单 |
| [design/templates/component-spec-form.md](design/templates/component-spec-form.md) | 权威当前 | 可复用组件规格表单 |
| [design/templates/ui-change-manifest-form.md](design/templates/ui-change-manifest-form.md) | 权威当前 | 每次 UI 变更的来源、范围、影响与证明清单表单 |
| [design/templates/visual-acceptance-form.md](design/templates/visual-acceptance-form.md) | 权威当前 | 视觉、任务、状态和真实后果四层验收记录表单 |
| [design/pages/evidence-library-page.md](design/pages/evidence-library-page.md) | 权威当前 | `PAGE-EVIDENCE-001`：多材料 Evidence Library 产品手册与技术呈现要求；运行页已按现行 Material Projection 落地作品 lane 与 Inspector，真实垂直证明仍未完成；WEB-REACT-001 D2 起为 React 实现，修订见其 §14 |
| [design/changes/evidence-cover-flip-card-001-ui-change-manifest.md](design/changes/evidence-cover-flip-card-001-ui-change-manifest.md) | 交付分支实现；未合并、未部署 | `EVIDENCE-COVER-FLIP-CARD-001`：封面排版作品卡的局部翻面主视觉与固定元数据板；不改变读取、详情或状态语义 |
| [design/acceptance/evidence-cover-flip-card-001-acceptance.md](design/acceptance/evidence-cover-flip-card-001-acceptance.md) | 一次性报告；未部署 | `ACC-EVIDENCE-COVER-FLIP-CARD-001`：自动合同、桌面真实数据预览与未切换 runtime 边界 |
| [design/changes/evidence-cover-card-proportion-002-ui-change-manifest.md](design/changes/evidence-cover-card-proportion-002-ui-change-manifest.md) | 权威当前；main 已合并、本机 runtime 已刷新 | `EVIDENCE-COVER-CARD-PROPORTION-002`：保留六列封面扫描密度，将卡片校准为高档案主视觉与低横向数据铭牌的成组比例，不改变读取、详情或状态语义 |
| [design/acceptance/evidence-cover-card-proportion-002-acceptance.md](design/acceptance/evidence-cover-card-proportion-002-acceptance.md) | 一次性报告；main 已合并、本机 runtime 已刷新 | `ACC-EVIDENCE-COVER-CARD-PROPORTION-002`：自动合同、真实本机只读预览与 3000 运行回执；Mog 前端验收仍独立 |
| [design/changes/evidence-cover-card-material-004-ui-change-manifest.md](design/changes/evidence-cover-card-material-004-ui-change-manifest.md) | 已合并 main；`:3000` runtime 已切换 | `EVIDENCE-COVER-CARD-MATERIAL-004`：封面作品卡的四级表面分层、卡面材质、舞台测量场、标题字阶与结果区横条删除；不改变读取、详情或状态语义 |
| [design/acceptance/evidence-cover-card-material-004-acceptance.md](design/acceptance/evidence-cover-card-material-004-acceptance.md) | 一次性报告；已部署到本机 `:3000` | `ACC-EVIDENCE-COVER-CARD-MATERIAL-004`：只读预览代理上的六列实测、翻面位移与封面填充核对；远端生产与真实触摸设备未验证 |
| [design/changes/domain-unification-001-ui-change-manifest.md](design/changes/domain-unification-001-ui-change-manifest.md) | 权威当前；隔离交互与本机部署后浏览器 smoke 已验证，Mog 验收待办 | 平级 Domain、领域管理、primary/reference、统一 Evidence Library 与 Domain-scoped Comment Study |
| [design/acceptance/domain-unification-001-acceptance.md](design/acceptance/domain-unification-001-acceptance.md) | 一次性隔离验收报告；附本机部署后 smoke 补充，非 Mog 业务验收 | ACC-DOMAIN-UNIFICATION-001：领域管理交互、跨 Domain role、1440/390 布局与边界 |
| [design/changes/corpus-cross-domain-render-001-ui-change-manifest.md](design/changes/corpus-cross-domain-render-001-ui-change-manifest.md) | 历史基线；已由 DOMAIN-UNIFICATION-001 取代 | 旧双轨样本列表与只读领域切换器；不得作为当前 Evidence、Domain 或材料能力合同 |
| [design/acceptance/corpus-cross-domain-render-001-acceptance.md](design/acceptance/corpus-cross-domain-render-001-acceptance.md) | 一次性报告 | `ACC-CORPUS-CROSS-DOMAIN-RENDER-001`：外部样本 21 行、列表级 Inspector、LIDS 菜单与未部署边界 |
| [design/changes/work-resource-read-001-ui-change-manifest.md](design/changes/work-resource-read-001-ui-change-manifest.md) | 交付分支实现；Issue #110 | Evidence Library 的共享 Work Resource 接口、作者/目标诚实表达、时间来源状态和研读/表格/封面三种排版变更清单 |
| [design/changes/evidence-v9-001-ui-change-manifest.md](design/changes/evidence-v9-001-ui-change-manifest.md) | 交付分支实现；EVIDENCE-V9-001 | `/corpus/evidence` 的 V9 研读密度、行级原声引用读能力、Inspector 三档宽度与 LOCAL MEDIA 横向材料浏览变更清单，含七处与 V9 的有据偏离 |
| [design/pages/evidence-library-multi-material-reference.html](design/pages/evidence-library-multi-material-reference.html) | 权威当前；静态参考 | `EVIDENCE-PAGE-002`：多材料 Evidence Library 合成高保真参考；不连接 API、真实平台、媒体或处理器 |
| [design/pages/collection-workspace-page.md](design/pages/collection-workspace-page.md) | 权威当前；PAGE-COLLECTION-001 | Collection 六个子面的职责、状态诚实性、页面级例外与未证明边界 |
| [design/pages/domain-management-page.md](design/pages/domain-management-page.md) | 权威当前；PAGE-DOMAIN-MANAGEMENT-001 | Domain 与 Target 用途管理、pause 后果、材料 lane 与状态合同；候选实现待验收 |
| [design/pages/collection-operations-html-prototype-brief.md](design/pages/collection-operations-html-prototype-brief.md) | 权威当前；静态原型委托书 | 面向无法读取本地仓库的外部模型的自包含 Collection 静态原型说明；不是运行实现或事实来源 |
| [design/pages/topic-workspace-page.md](design/pages/topic-workspace-page.md) | 交付分支权威；PAGE-TOPIC-WORKSPACE-001 | 真实但明确暂定的 L2 Topic 工作区：定义、人工裁定、冻结材料与来源边界 |
| [design/changes/topic-workspace-real-001-ui-change-manifest.md](design/changes/topic-workspace-real-001-ui-change-manifest.md) | 交付分支实现；Issue #112 | Topic runtime page 的来源回执、LIDS 影响、状态与响应式变更清单 |
| [design/changes/collection-lifecycle-001-ui-change-manifest.md](design/changes/collection-lifecycle-001-ui-change-manifest.md) | 交付分支实现；Issue #148 | creator 生命周期读模型、四职责目标抽屉、Evidence tab 退役与 Corpus 精确深链的变更清单 |
| [design/changes/observation-target-dossier-ui-001-ui-change-manifest.md](design/changes/observation-target-dossier-ui-001-ui-change-manifest.md) | 交付分支实现；Issue #158 | 观察目标 creator/keyword 宽表、200 篇上限渐进建档、三职责档案工作区与两级生命周期散点合同 |
| [design/changes/collection-control-closure-001-ui-change-manifest.md](design/changes/collection-control-closure-001-ui-change-manifest.md) | 交付分支实现；Issue #149 / Package 2 + V4 UI 扩展 | Collection Control 规则 modal、账号准入、五面控制事实与 V4 桌面工作区复刻的变更清单 |
| [design/changes/collection-scheduler-scale-001-ui-change-manifest.md](design/changes/collection-scheduler-scale-001-ui-change-manifest.md) | 源码实施中；COLLECTION-SCHEDULER-SCALE-001 | `/collection/runtime` 的平台并发、技术 lane、工位 Lease 与 Rule 排程只读投影；不新增采集操作 |
| [design/pages/plugin-producer-popup.md](design/pages/plugin-producer-popup.md) | 权威当前 | `PAGE-PLUGIN-001`：Linggan 自有 Browser Producer popup 的身份、本机接纳准备度与受限手动 Discovery receipt |
| [design/changes/local-001a-evidence-library-ui-change-manifest.md](design/changes/local-001a-evidence-library-ui-change-manifest.md) | 权威当前；LOCAL-001A | Issue #25 的 UI 来源、范围、例外和证明边界 |
| [design/changes/local-001d-unknown-published-discovery-ui-change-manifest.md](design/changes/local-001d-unknown-published-discovery-ui-change-manifest.md) | 权威当前；LOCAL-001D / Issue #62 | 默认最新已接纳读取、`PUBLISHED_AT UNKNOWN` 与显式发布时间窗口的来源和范围 |
| [design/changes/design-003-lids-visual-baseline-ui-change-manifest.md](design/changes/design-003-lids-visual-baseline-ui-change-manifest.md) | 权威当前；DESIGN-003 | Issue #44 的视觉基线换向：token、规则、页面与数据诚实性逐条核对 |
| [design/changes/design-004-corpus-rail-ui-change-manifest.md](design/changes/design-004-corpus-rail-ui-change-manifest.md) | 权威当前；DESIGN-004 | Corpus rail 面层差异化与选中态重音：规则依据、参考图取舍与重音计数 |
| [design/changes/design-005-collection-workspace-ui-change-manifest.md](design/changes/design-005-collection-workspace-ui-change-manifest.md) | 权威当前；DESIGN-005 | Collection 五个子面落地：Mog 三项裁定、清掉的冗余、消化的断层与两项待决 |
| [design/changes/design-006-collection-ux-revision-ui-change-manifest.md](design/changes/design-006-collection-ux-revision-ui-change-manifest.md) | 权威当前；DESIGN-006 | Collection 五面 UX 修订：顺序与默认入口、命名去重、空态分级、读数异常优先、无数据时结构留数据格延后 |
| [design/changes/design-007-chinese-first-evidence-library-ui-change-manifest.md](design/changes/design-007-chinese-first-evidence-library-ui-change-manifest.md) | 权威当前；DESIGN-007 | Evidence Library 中文优先表达：中文独立承担用户含义，英文仅作技术旁注 |
| [design/changes/evidence-page-002-multi-material-ui-change-manifest.md](design/changes/evidence-page-002-multi-material-ui-change-manifest.md) | 权威当前；EVIDENCE-PAGE-002 | 多材料 Evidence Library 产品/静态参考的来源、表面、状态和明确非目标 |
| [design/changes/evidence-runtime-001-ui-change-manifest.md](design/changes/evidence-runtime-001-ui-change-manifest.md) | 权威当前；EVIDENCE-RUNTIME-001 / Issue #90 | Material Projection 运行页的唯一数据入口、作品 lane、Inspector、敏感材料与一次性验收边界 |
| [design/changes/design-008-shared-shell-chinese-first-ui-change-manifest.md](design/changes/design-008-shared-shell-chinese-first-ui-change-manifest.md) | 权威当前；DESIGN-008 / Issue #68 | 共享 local-web shell 的中文主语义、英文技术旁注与 Corpus/Collection 双页面验收边界 |
| [design/changes/design-009-runtime-capacity-surface-ui-change-manifest.md](design/changes/design-009-runtime-capacity-surface-ui-change-manifest.md) | 权威当前；DESIGN-009 | 执行工位改为产能判定面，运行状态由真实容量读取而非写死文案 |
| [design/changes/design-010-lids-v7-adoption-ui-change-manifest.md](design/changes/design-010-lids-v7-adoption-ui-change-manifest.md) | 权威当前；DESIGN-010 | LIDS 升级 v7.0 的范围、逐条差异、运行时欠账与明确非目标；本次零运行时改动 |
| [design/changes/design-011-collection-surfaces-v7-ui-change-manifest.md](design/changes/design-011-collection-surfaces-v7-ui-change-manifest.md) | 权威当前；DESIGN-011 | 生产流与执行工位落地 v7、工位能力矩阵读模型、四态判定与已知限制 |
| [design/changes/design-012-evidence-information-hierarchy-ui-change-manifest.md](design/changes/design-012-evidence-information-hierarchy-ui-change-manifest.md) | 权威当前；DESIGN-012 | `/corpus/evidence` 信息层级四层、LANG-05 运行时迁移、边界带收口，以及 provenance 覆盖等三处错误信念的修复 |
| [design/changes/plugin-001-producer-popup-ui-change-manifest.md](design/changes/plugin-001-producer-popup-ui-change-manifest.md) | 已被替代；PLUGIN-001 | Issue #33 popup 的历史 UI 边界；不再约束当前实现 |
| [design/changes/plugin-migration-001-producer-popup-ui-change-manifest.md](design/changes/plugin-migration-001-producer-popup-ui-change-manifest.md) | 权威当前；PLUGIN-MIGRATION-001 | Issue #37 popup 的 Discovery 行动与状态边界；启动恢复由 Issue #53 文档补充 |
| [design/changes/targets-row-actions-layout-001-ui-change-manifest.md](design/changes/targets-row-actions-layout-001-ui-change-manifest.md) | 活跃计划；TARGETS-ROW-ACTIONS-LAYOUT-001 | 观察目标行内操作竖排溢出的排版修复；13 列密度问题与 V4 结构差距留给 Mog 决策 |
| [design/changes/plugin-popup-recovery-001-ui-change-manifest.md](design/changes/plugin-popup-recovery-001-ui-change-manifest.md) | 权威当前；PLUGIN-POPUP-RECOVERY-001 | Issue #53 popup 启动恢复、无副作用 fallback 与 v0.4.2 release 边界；Issue #55 的运行时修订见当月进度记录 |
| [design/acceptance/local-001a-evidence-library-visual-acceptance.md](design/acceptance/local-001a-evidence-library-visual-acceptance.md) | 一次性报告 | `ACC-EVIDENCE-001`：本地页面的视觉、状态与真实后果分层验收记录 |
| [design/acceptance/local-001d-unknown-published-discovery-acceptance.md](design/acceptance/local-001d-unknown-published-discovery-acceptance.md) | 一次性报告 | `ACC-LOCAL-001D-001`：未知发布时间默认读取和显式窗口的分层验收记录 |
| [design/acceptance/design-003-lids-visual-baseline-visual-acceptance.md](design/acceptance/design-003-lids-visual-baseline-visual-acceptance.md) | 一次性报告 | Issue #44 的双实例同屏实测：计量、诚实性逐条核对与未证明范围 |
| [design/acceptance/design-004-corpus-rail-visual-acceptance.md](design/acceptance/design-004-corpus-rail-visual-acceptance.md) | 一次性报告 | `ACC-RAIL-004`：rail 三视口实拍验收；hover/focus 因全部路由禁用而未验证 |
| [design/acceptance/design-005-collection-workspace-visual-acceptance.md](design/acceptance/design-005-collection-workspace-visual-acceptance.md) | 一次性报告 | `ACC-COLLECTION-001`：五个子面与抽屉四态实拍；无数据的行与图表未验证 |
| [design/acceptance/design-008-shared-shell-chinese-first-acceptance.md](design/acceptance/design-008-shared-shell-chinese-first-acceptance.md) | 一次性报告 | `ACC-DESIGN-008`：共享壳层的中文主语义、技术注释层级与未证明边界 |
| [design/acceptance/plugin-001-producer-popup-visual-acceptance.md](design/acceptance/plugin-001-producer-popup-visual-acceptance.md) | 一次性报告 | `ACC-PLUGIN-001`：Issue #33 popup 的静态 source/release 验收；不证明浏览器加载或真实采集 |
| [design/acceptance/plugin-migration-001-producer-popup-visual-acceptance.md](design/acceptance/plugin-migration-001-producer-popup-visual-acceptance.md) | 一次性报告 | `ACC-PLUGIN-002`：Issue #37 adapter/mock ingress/release 验收；不证明真实浏览器或采集 |
| [design/acceptance/plugin-popup-recovery-001-visual-acceptance.md](design/acceptance/plugin-popup-recovery-001-visual-acceptance.md) | 一次性报告 | `ACC-PLUGIN-POPUP-RECOVERY-001`：Issue #53 source/release 启动保护验收；不证明 Chrome 已加载或真实采集 |
| [design/acceptance/design-007-chinese-first-evidence-library-acceptance.md](design/acceptance/design-007-chinese-first-evidence-library-acceptance.md) | 一次性报告 | `ACC-DESIGN-007`：Evidence Library 中文优先表达的分层验收记录 |
| [design/acceptance/evidence-page-002-multi-material-reference-acceptance.md](design/acceptance/evidence-page-002-multi-material-reference-acceptance.md) | 一次性报告 | `ACC-EVIDENCE-PAGE-002`：多材料 Evidence Library 静态参考的双视口、状态、互动和边界验收 |
| [design/acceptance/evidence-runtime-001-visual-acceptance.md](design/acceptance/evidence-runtime-001-visual-acceptance.md) | 一次性报告 | `ACC-EVIDENCE-RUNTIME-001`：Issue #90 运行页的自动检查、桌面/375px 浏览器证据与未证明边界 |
| [design/acceptance/work-resource-read-001-visual-acceptance.md](design/acceptance/work-resource-read-001-visual-acceptance.md) | 一次性报告 | `ACC-WORK-RESOURCE-READ-001`：共享作品资源、目标/作者关系、时间资格和三种排版的分层验收；真实视觉与部署仍未核验 |
| [design/acceptance/topic-workspace-real-001-visual-acceptance.md](design/acceptance/topic-workspace-real-001-visual-acceptance.md) | 一次性报告；Issue #112 | `ACC-TOPIC-WORKSPACE-REAL-001`：暂定 Topic 事务、API、页面与未证明边界的分层验收 |
| [design/acceptance/collection-lifecycle-001-visual-acceptance.md](design/acceptance/collection-lifecycle-001-visual-acceptance.md) | 一次性报告；Issue #148 | `ACC-COLLECTION-LIFECYCLE-001`：生命周期读模型、隔离浏览器视口、精确深链与未部署边界的分层验收 |
| [design/acceptance/observation-target-dossier-ui-001-visual-acceptance.md](design/acceptance/observation-target-dossier-ui-001-visual-acceptance.md) | 一次性报告；Issue #158 | `ACC-OBSERVATION-TARGET-DOSSIER-UI-001`：creator/keyword 目录、档案工作区、渐进建档后果与 1440 桌面分层验收 |
| [design/acceptance/collection-five-page-v4-ui-001-visual-acceptance.md](design/acceptance/collection-five-page-v4-ui-001-visual-acceptance.md) | 一次性报告；Issue #149 V4 UI 扩展 | `ACC-COLLECTION-FIVE-PAGE-V4-UI-001`：五页结构、隔离 PostgreSQL、1440×900 浏览器互动与未部署边界 |
| [design/acceptance/dev-05-xhs-content-observation-001-visual-acceptance.md](design/acceptance/dev-05-xhs-content-observation-001-visual-acceptance.md) | 一次性报告 | `ACC-DEV-05-XHS-CONTENT-OBSERVATION-001`：Issue #133 的当前/历史字段、Coverage 历史与有界复观测分层验收；未重载运行时、未进行真实 XHS 或 Mog 验收 |
| [design/acceptance/evidence-v9-001-visual-acceptance.md](design/acceptance/evidence-v9-001-visual-acceptance.md) | 一次性报告 | `ACC-EVIDENCE-V9-001`：V9 研读密度、材料完整度口径、Inspector 宽度与 LOCAL MEDIA/Lightbox 的真实数据走查；多图链路只有一个样本，未部署 |
| [design/acceptance/design-009-runtime-capacity-surface-acceptance.md](design/acceptance/design-009-runtime-capacity-surface-acceptance.md) | 一次性报告 | `ACC-RUNTIME-001`：产能判定、状态诚实性与视觉的验收记录 |
| [design/foundation/topic-intelligence-visual-language.md](design/foundation/topic-intelligence-visual-language.md) | 权威当前 | DESIGN-002 的 DS-001–DS-007：合成 Topic Reference Page 的受限视觉语言 |
| [design/patterns/evidence-candidate-and-boundary-patterns.md](design/patterns/evidence-candidate-and-boundary-patterns.md) | 权威当前 | DESIGN-002 的 PAT-001–PAT-004：观察、候选、边界与无副作用意图表达 |
| [design/components/component-promotion.md](design/components/component-promotion.md) | 权威当前 | 从 Reference Page 局部块到真实 CMP 的晋升条件；当前没有已晋升组件 |
| [design/pages/topic-intelligence-reference-page.md](design/pages/topic-intelligence-reference-page.md) | 权威当前 | PAGE-TOPIC-001 的组合规格、状态、互动和验收边界 |
| [design/pages/topic-intelligence-reference.html](design/pages/topic-intelligence-reference.html) | 权威当前 | 可本地打开的合成静态 Topic Reference Page；不是运行 Web 产品 |
| [design/pages/topic-intelligence-reference-acceptance.md](design/pages/topic-intelligence-reference-acceptance.md) | 权威当前 | ACC-TOPIC-001：静态参考实现的视觉、互动、自动检查和未证明边界 |
| [`architecture/technical-architecture-baseline.md`](architecture/technical-architecture-baseline.md) | 权威当前 | 全产品技术架构统一入口：权威层级、确定性、当前/目标边界、模块/入口/数据/运行主干与 SCOPE 符合性 |
| [`architecture/target-architecture.md`](architecture/target-architecture.md) | 草案 | DISC-001 已确认硬边界之上的总体架构建议；具体实现按 SCOPE 渐进冻结 |
| [`architecture/system-overview-diagram.html`](architecture/system-overview-diagram.html) | 草案 | AEDS 风格的整体架构图；在同一视图中区分当前代码骨架、已批准 Evidence 切片与后续目标能力 |
| [`architecture/business-process-diagram.html`](architecture/business-process-diagram.html) | 草案 | AEDS 风格的内容情报业务流程图；展示人、控制层、Capture、Evidence 与 Intelligence 的责任交接 |
| [`architecture/project-architecture-atlas.html`](architecture/project-architecture-atlas.html) | 代码事实优先；本机快照 2026-08-28 | 面向非技术项目负责人的 AEDS 中文架构全景页；以当前本机代码、PostgreSQL、loopback API 与页面读取核对为主，分层展示独立本地产品、受控浏览器采集、模块化单体、数据/事实边界、界面、运行环境、AI Agent 与路线图；不替代运行、长期采集或生产证明 |
| [`architecture/module-architecture.md`](architecture/module-architecture.md) | 草案 | Gate 6 Rust 模块、接口、依赖、adapter、测试表面与无巨型文件门禁；不授权创建 crate |
| [`architecture/agent-architecture.md`](architecture/agent-architecture.md) | 草案 | Gate 6 Agent Invocation、工具权限、预算/停止、恢复、结构化输出、隐私与评测；不选择模型或授权真实原文 |
| [`architecture/capture-plugin-architecture.md`](architecture/capture-plugin-architecture.md) | 代码事实优先 | Linggan 采集控制层与受控 Browser Producer 的当前责任边界；`0.8.28` 单工位标准详情真实链已通过，未实现的长期候选仍以草案待决策 |
| [`architecture/capture-control-contract.md`](architecture/capture-control-contract.md) | 草案 | ARC-001 首批真实 Canary 前的采集控制合同：Need/授权/准入、有限资源、lane、Coverage、部分结果、回执与恢复边界；不授权真实执行 |
| [`architecture/media-lifecycle-contract.md`](architecture/media-lifecycle-contract.md) | 权威当前 | 统一 Slot/Origin/Download/Blob/Materialization/Derivative/Relation 合同；`0.8.28` 真实封面、正文图、Live Photo 与作者头像链已验收，ASR 失败与 `comment.image NOT_OBSERVED` 继续如实保留 |
| [`architecture/runtime-operations-architecture.md`](architecture/runtime-operations-architecture.md) | 代码事实优先 | 当前 loopback API / 巡检 worker / 媒体 worker / PostgreSQL / scheduler 运行拓扑，以及未实现的长期运维候选边界 |
| [`architecture/data-architecture.md`](architecture/data-architecture.md) | 草案 | Gate 5 数据分类、身份、版本、Current、隐私、统计资格与 PostgreSQL 概念模型；不是 DDL |
| [`architecture/data-relations.md`](architecture/data-relations.md) | 草案 | `data-architecture.md` 的渐进披露子文档；收敛候选基数、外键责任、类型化关系与并发约束，不是最终表清单 |
| [`architecture/data-consistency.md`](architecture/data-consistency.md) | 草案 | 数据架构第三层；定义事务、重放、并发、隐私传播和 PostgreSQL 16 可证伪验收，不是 SQL 或 migration |
| [`architecture/current-v2-architecture.md`](architecture/current-v2-architecture.md) | 代码事实优先 | 现役 V2 固定点的参考架构 |
| [`architecture/rust-porting-map.md`](architecture/rust-porting-map.md) | 草案 | Rust 移植候选参考；只有正式 SCOPE 内明确列出的部分可以实施 |
| [`pages/intelligence-product-blueprint.md`](pages/intelligence-product-blueprint.md) | 草案；DES-INTELLIGENCE-20260906 | 独立首页与四工作区、16 个二级页、功能/状态/跳转、事实归属与落地依赖；"移出选题生产"已由 DEC-0009 修订 |
| [`plans/active/intelligence-product-001-implementation-plan.md`](plans/active/intelligence-product-001-implementation-plan.md) | 活跃计划；仅计划编制，实施待派定 | 已确认决定、main/PR 事实核对、P00–P08 交付顺序、全页面覆盖、单包任务模板、模型引用与反例验收；语料资产归评论研究页内 |
| [`pages/intelligence-home-threejs.md`](pages/intelligence-home-threejs.md) | 草案；DES-INTELLIGENCE-20260906 | 三栏首页、稳定地形、语义映射、安静/中断/空态、Three.js 与 HTML 等价入口及验证边界 |
| [`design/pages/intelligence-product-prototype.html`](design/pages/intelligence-product-prototype.html) | 合成设计演示 | 自包含可点击原型，实际 Three.js；预置分析与本地状态，不接真实系统 |
| [`pages/page-map.md`](pages/page-map.md) | 草案 | 页面与情报工作流的初步映射 |
| [`pages/product-interface-architecture.md`](pages/product-interface-architecture.md) | 草案 | Gate 7 工作台、Topic/Corpus/研究/行动/运行中心、API 与外部 Agent CLI 入口架构 |
| [`pages/home-intelligence-surface.md`](pages/home-intelligence-surface.md) | 草案 | `HOME-01`–`HOME-17` 历史首页推演：三栏、稳定地形；选题卡/战绩回流已被 2026-09-06 最新范围移出，详见新首页专项 |
| [`pages/topic-intelligence-surface.md`](pages/topic-intelligence-surface.md) | 权威当前 | DESIGN-002 的 Topic 深入任务、信息/状态/行动边界；当前只授权合成静态参考页 |
| [`migration/action-plan.md`](migration/action-plan.md) | 活跃计划 | 基础设计阶段形成的重建顺序与架构工作包参考；实时阶段状态、完成证据和决策门以 `development-stage-tracker.md` 为准 |
| [`migration/transfer-checklist.md`](migration/transfer-checklist.md) | 活跃计划 | 跨机器搬迁和隔离检查 |
| [`reviews/bootstrap-review.md`](reviews/bootstrap-review.md) | 一次性报告 | Bootstrap 固定点审查，不代表当前实现状态 |
| [`audits/gate-3-domain-model-audit-2026-08-20.md`](audits/gate-3-domain-model-audit-2026-08-20.md) | 一次性报告 | 已获用户整体确认的 Gate 3 设计快照与全链压力测试；不替代权威共同语言，也不证明 Gate 4–7 已通过 |
| [`audits/gate-5-data-architecture-audit-2026-08-20.md`](audits/gate-5-data-architecture-audit-2026-08-20.md) | 一次性报告 | Gate 5 平台无关概念数据模型的第一轮对抗审查、已修漂移、条件通过项和待确认决定；不等于数据库实现获批 |
| [`audits/gate-4-to-7-cross-consistency-audit-2026-08-20.md`](audits/gate-4-to-7-cross-consistency-audit-2026-08-20.md) | 一次性报告 | Gate 4–7 在统一决定与 SCOPE-001 前的采集、数据、架构、页面/CLI 跨文档审计 |
| [`audits/gate-7-synthetic-task-walkthrough-2026-08-20.md`](audits/gate-7-synthetic-task-walkthrough-2026-08-20.md) | 一次性报告 | 用完全合成的“任务启动困难”负面场景走查今日关注、Topic、50/100 运行状态与外部 Agent CLI |
| [`audits/pre-scope-001-readiness-2026-08-20.md`](audits/pre-scope-001-readiness-2026-08-20.md) | 一次性报告 | 不启动 SCOPE/代码的前提下预演首个 Content Evidence 切片的 fixture、数据库副作用、模块/文件预算和完成标准 |
| [`audits/scope-001-final-adversarial-audit-2026-08-20.md`](audits/scope-001-final-adversarial-audit-2026-08-20.md) | 一次性报告 | 正式 SCOPE-001 的独立代码前审查；P0/P1/P2、修订结果和最终实施确认边界 |
| [`audits/implementation-readiness-review-2026-08-20.md`](audits/implementation-readiness-review-2026-08-20.md) | 一次性报告 | 全项目正式编码前实施就绪终审；NO-GO 证据、P0/P1、15 个场景、真实最小闭环及主线 Agent 复核协议 |
| [`audits/pi-agent-kernel-upstream-assessment-2026-08-20.md`](audits/pi-agent-kernel-upstream-assessment-2026-08-20.md) | 一次性报告 | Pi 上游发布版/源码/安全边界审查；裁定为统一 Agent 执行适配器候选，不是权限、持久任务或业务事实内核，也不授权当前编码 |
| [`audits/pre-implementation-architecture-audit-2026-08-21.md`](audits/pre-implementation-architecture-audit-2026-08-21.md) | 一次性报告 | `ADV-AUDIT-001` 的独立复核最终处置：保留权威冲突、真实 producer 接缝、产品职责和 dirty worktree 风险，否定当前解除 Attempt 1:1、强加 `attemptRef`、改 crate 或整批提交的危险建议；不改变 F01 代码门 |
| [`audits/intelligence-system-review-2026-09-05.md`](audits/intelligence-system-review-2026-09-05.md) | 一次性报告 | `AUD-INTELLIGENCE-20260905`：固定主线 `4864edd` 的产品、领域、Rust/API/worker、PostgreSQL、Browser Producer、媒体、UI/UX、Topic/Agent 与运行协作全项目梳理；含已具备能力与尚未闭环范围的分别结论，不授予修复、采集、部署或合并权限 |
| [`audits/intelligence-system-review-2026-09-05-evidence.md`](audits/intelligence-system-review-2026-09-05-evidence.md) | 一次性报告 | 上述主报告的 E01–E16 证据、检查范围与复核方法；197 项常规 Rust / 118 项隔离 PostgreSQL / 252 项插件测试、只读数据库与浏览器观察、明确未证明边界；不含账号原始身份、评论原文或凭据 |
| [`audits/code-quality-review-2026-09-14.md`](audits/code-quality-review-2026-09-14.md) | 一次性报告 | `AUD-CODE-QUALITY-20260914`：固定主线 `431e79b` 的第七层代码质量核实包——逐条给出定位、判据原文、复现命令、预期输出与反证条件，附已排除项与未覆盖范围；供独立复核，不授予修复、采集、部署或合并权限 |
| [`audits/ops-module-inheritance-audit-2026-10-13.md`](audits/ops-module-inheritance-audit-2026-10-13.md) | 一次性报告 | `OPS-MIGRATION-001`：内容工作台运营管理按 ARC-001 四类口径的能力继承审计——页面与依赖、真实用量与填写率（只含聚合数）、Mog 六项去留裁定、不变量与未证明边界；不授予实现、迁移或部署权限 |
| [`runbooks/development-environment.md`](runbooks/development-environment.md) | 权威当前 | Rust 与 Docker PostgreSQL 16 的统一运行入口 |
| [`runbooks/local-evidence-library.md`](runbooks/local-evidence-library.md) | 权威当前 | Rust loopback host、health、受控 discovery ingress 与 Evidence Library 本地启动/验证；明确不含真实插件、平台或媒体链路 |
| [`runbooks/local-runtime-deployment.md`](runbooks/local-runtime-deployment.md) | 权威当前 | 三个 launchd 常驻服务的运行来源、跟随 origin/main 的更新方式、迁移闸门与故障处置；§7 为运行诊断（一个 tick 号串一条链的查询、事件与选择器快照的字段白名单，样例为手工构造的脱敏样例）；取代已废弃的冻结快照模型 |
| [`runbooks/linggan-browser-producer-local.md`](runbooks/linggan-browser-producer-local.md) | 权威当前 | Browser Producer 当前 `0.8.47` 构建/发行/加载/工位/真实回执的分层核对步骤；操作授权仍以当次任务为准 |
| [`platforms/xiaohongshu/capture-capability-registry.md`](platforms/xiaohongshu/capture-capability-registry.md) | 权威当前；AUD-XHS-001 范围内 | 小红书页面字段、探针、插件交付能力、Coverage 与平台漂移的唯一登记入口；不保存真实原文或接入真实材料 |
| [`decisions/0002-comment-collection-completion-and-retry.md`](decisions/0002-comment-collection-completion-and-retry.md) | ACCEPTED | 小红书评论的标准 30 条窗口、全量深采完成判据、部分材料可用性、重新从详情页采集及当前去重投影规则 |
| [`proposals/real-canary-002-controlled-discovery-through-authorization-chain.md`](proposals/real-canary-002-controlled-discovery-through-authorization-chain.md) | 草案 | 让一次已获准过的 ADHD 发现面原样再走一遍，但全程经过「授权 → 准入 → 工单 → 租约 → 闸门 → 派发」，用于解锁采集控制合同 §12 第 5 条；不扩大平台足迹，不授权 001C-2/001C-3 |
| [`plans/active/arc-001-architecture-closure-decision-map.md`](plans/active/arc-001-architecture-closure-decision-map.md) | 活跃计划 | F01 后、首个真实 producer 和用户可见切片前的产品形态、P0 页面、Canary、Capture/Media 与第一阶段运行架构决策图；当前首票为 `product-shell` |
| [`plans/active/collection-three-package-closure-001.md`](plans/active/collection-three-package-closure-001.md) | 活跃计划；Package 1 / Issue #148 已合并 | Collection 三包串行收口：目标级生命周期只读投影、LIDS v7 抽屉与精确 Corpus 深链已进入主线；Package 3 仍未启动 |
| [`plans/active/collection-control-closure-001.md`](plans/active/collection-control-closure-001.md) | 活跃计划；Package 2 已合并，V4 UI 扩展在独立分支实现 | 平台账号准入、安装凭证、版本化巡检规则、调度回执与 Collection 五面控制事实；V4 桌面复刻已完成隔离 1440 验证，待提交与 Mog 验收 |
| [`plans/active/collection-scheduler-scale-001.md`](plans/active/collection-scheduler-scale-001.md) | 源码实施中 | 大量观察目标、多工位、多来源下的稳定相位、统一 queue 公平性、批量蓄水、平台并发、失败冷却和 Runtime 只读事实 |
| [`plans/active/design-002-topic-intelligence-reference-page.md`](plans/active/design-002-topic-intelligence-reference-page.md) | 活跃计划 | 合成 Topic Intelligence Reference Page 的范围、规则、原型与验证计划；不替代完整 P0 或业务 SCOPE |
| [`plans/active/topic-workspace-real-001.md`](plans/active/topic-workspace-real-001.md) | 活跃计划；已部署、待 Mog 业务验收；Issue #112 | 首个真实 Topic 工作区垂直切片；只到暂定定义、人工裁定、冻结 Material Pack 与共享 Work Resource Read |
| [`plans/active/scope-001-content-evidence-vertical-slice.md`](plans/active/scope-001-content-evidence-vertical-slice.md) | 活跃计划 | 当前唯一获准实施事项：F01 synthetic fact-kernel technical tracer 受控进入 TDD；完成 loopback API + minimal CLI 后 hard stop，不扩入 F02–F10、真实平台/AI/插件/Web |
| [`plans/active/plugin-runtime-001-full-capability-retrofit.md`](plans/active/plugin-runtime-001-full-capability-retrofit.md) | 活跃计划 | Issue #50：将灵感爆爆爆的成熟浏览器采集能力一次性适配为 Linggan 自有 Browser Producer Runtime；媒体本地资产与异步处理血缘为本事项第一类约束 |
| [`plans/active/author-target-sync-001.md`](plans/active/author-target-sync-001.md) | 活跃计划 | 已缓存作者资料经既有 Browser Producer outbox 接纳后，幂等同步为观察目标；作者头像经既有媒体管道本地物化并在观察目标页以本机资产展示 |
| [`plans/completed/aud-xhs-001-xiaohongshu-capture-probe-refresh.md`](plans/completed/aud-xhs-001-xiaohongshu-capture-probe-refresh.md) | 已完成；治理终止；Issue #74 | 历史小红书页面字段与能力探针；未确认字段保持 unknown，只有真实链被具体字段阻断时才可另行最小探针 |
| [`plans/completed/plugin-xhs-capture-upgrade-001.md`](plans/completed/plugin-xhs-capture-upgrade-001.md) | 已完成；源码升级；Issue #76 / PR #77 | 已合入搜索页面事实、详情 DOM 回退与统一详情包回执源码；不证明真实兼容性、运行时或用户验收 |
| [`plans/active/plugin-xhs-active-collection-001.md`](plans/active/plugin-xhs-active-collection-001.md) | 活跃计划；Issue #78 | 将小红书搜索、标准详情、单篇评论深采与批量评论升级为目标驱动、可恢复的页面执行；不进行真实材料交付、真实页面回归或发布部署 |
| [`plans/active/plugin-xhs-finalization-001.md`](plans/active/plugin-xhs-finalization-001.md) | 活跃计划；Issue #128 | 一次性收口 Browser Producer 派发/回执/评论 Attempt 可靠性与统一 MediaResource；保留人工媒体窗口，真实链只对当前授权样本运行一次 |
| [`plans/completed/plugin-xhs-adaptive-scroll-and-detail-receipt-001.md`](plans/completed/plugin-xhs-adaptive-scroll-and-detail-receipt-001.md) | 已完成；Issue #80 / PR #81 | 将 XHS 页面加载滚动收敛为可解释的有限过程；分离 Intelligence 详情交付与人工媒体下载，并逐 lane 显示接纳回执 |
| [`plans/completed/local-001-local-product-evidence-library.md`](plans/completed/local-001-local-product-evidence-library.md) | 已完成；路线归档；Issue #23 | 历史 Local-001 父计划：建立本地产品/受控读取/Canary 的申请边界；不再作为当前路线图总编号，也不把所有后续设想写成已完成 |
| [`plans/active/local-001d-unknown-published-discovery-view.md`](plans/active/local-001d-unknown-published-discovery-view.md) | 活跃计划；Issue #62 | 修复默认 Evidence Library 把已接纳、来源发布时间未知的 discovery 卡片隐藏的问题；显式 7/30 天发布时间窗口保持严格 |
| [`plans/active/local-runtime-001-persistent-loopback.md`](plans/active/local-runtime-001-persistent-loopback.md) | 活跃计划 | Issue #38 的持久本地 PostgreSQL migration、loopback readiness 与重启保留证明；不含插件或真实平台 |
| [`plans/active/plugin-migration-001-linggan-owned-xhs-discovery.md`](plans/active/plugin-migration-001-linggan-owned-xhs-discovery.md) | 活跃计划 | Issue #37：将成熟旧插件的可见 XHS 搜索卡发现规则适配为 Linggan 自有包，且只启用一次手动 `ADHD` / “综合” / 前 20 卡 Discovery → 固定 localhost ingress |
| [`plans/completed/plugin-rehome-001-full-legacy-browser.md`](plans/completed/plugin-rehome-001-full-legacy-browser.md) | 已完成；Issue #41 / PR #42 | 完整迁入 `linggan-boom v2.0.91` 的浏览器 source/UX 作为 Linggan 自有包，并切断旧内容工作台运行时；不证明真实采集 |
| [`plans/completed/gov-004-delivery-package-harness-autonomy.md`](plans/completed/gov-004-delivery-package-harness-autonomy.md) | 已完成；已被 GOV-005 替代 | Issue #70：确立交付包、表面/状态/依赖/验收矩阵；其中 Harness 自主编排已由 GOV-005 取代 |
| [`plans/completed/gov-005-human-directed-collaboration.md`](plans/completed/gov-005-human-directed-collaboration.md) | 已完成；Issue #72 / PR #73 | 将派单、并发、审查和合并的默认控制权交由 Mog；Agent 仅执行明确分配的 Work Package |
| [`plans/completed/plugin-retrofit-local-trusted-001.md`](plans/completed/plugin-retrofit-local-trusted-001.md) | 已完成计划 | Issue #43 / PR #46：已合并的 LOCAL_TRUSTED synthetic manual `TaskSpec → Attempt → durable outbox → loopback receipt`；不证明真实浏览器、平台、媒体、scheduler 或 Evidence Library 真实数据 |
| [`plans/completed/plugin-001-linggan-owned-producer.md`](plans/completed/plugin-001-linggan-owned-producer.md) | 已完成计划 | `PLUGIN-001`：Linggan 自有 MV3 基础包、loopback 探测与静态可安装 release；后续 Discovery 由 PLUGIN-MIGRATION-001 承接 |
| [`plans/completed/disc-001-project-foundation-design.md`](plans/completed/disc-001-project-foundation-design.md) | 已完成计划 | 七道项目基础设计关口与 `USER-DEC-01`–`06` 的完成记录 |
| [`plans/completed/env-001-development-environment.md`](plans/completed/env-001-development-environment.md) | 已完成计划 | ENV-001 的范围、执行和验收记录 |
| [`plans/completed/gov-001-project-file-governance.md`](plans/completed/gov-001-project-file-governance.md) | 已完成计划 | GOV-001 的范围、交付和验收记录 |
| [`plans/completed/gov-002-agent-skills-configuration.md`](plans/completed/gov-002-agent-skills-configuration.md) | 已完成计划 | Matt Pocock 工程技能、GitHub Issues、triage 与领域文档适配的配置记录 |
| [`progress/README.md`](progress/README.md) | 权威当前 | 变更记录规则和月份索引 |
| [`progress/2026-10.md`](progress/2026-10.md) | 权威当前 | 2026-10 的重要变更记录 |
| [`progress/2026-09.md`](progress/2026-09.md) | 权威当前 | 2026-09 的重要变更记录 |
| [`progress/2026-08.md`](progress/2026-08.md) | 权威当前 | 2026-08 的重要变更记录 |

## 仓库外层资料入口

| 文档 | 状态 | 用途 |
|---|---|---|
| [`database/README.md`](../database/README.md) | 权威当前 | 新数据库交付与安全边界 |
| [`database/legacy-archive-manifest.md`](../database/legacy-archive-manifest.md) | 历史归档 | 旧数据库和媒体的只读归档清单 |
| [`references/README.md`](../references/README.md) | 历史归档 | 历史证据区阅读边界 |
| [`references/current-v2/SOURCE-PROVENANCE.md`](../references/current-v2/SOURCE-PROVENANCE.md) | 历史归档 | 固定源码快照来源证明 |

## 按任务渐进读取

- 项目管理或新增文件：先读 `governance/`，再读当前事项文档。
- 使用 Matt Pocock 工程技能：先读 `agents/` 中与任务追踪、triage 或领域文档相关的配置，再进入当前 Issue 和直接关联文档。
- UI 设计、前端实现或 UI 审查：先读 [agents/ui-execution-contract.md](agents/ui-execution-contract.md) 和 [design/README.md](design/README.md)，再进入对应产品页面、已批准设计规则、数据合同与当前 SCOPE；产品草案、截图、旧系统和参考材料不能单独授权实现。
- DESIGN-002 Topic Reference Page：依次读 [`pages/topic-intelligence-surface.md`](pages/topic-intelligence-surface.md)、[`design/foundation/topic-intelligence-visual-language.md`](design/foundation/topic-intelligence-visual-language.md)、[`design/patterns/evidence-candidate-and-boundary-patterns.md`](design/patterns/evidence-candidate-and-boundary-patterns.md)、[`design/components/component-promotion.md`](design/components/component-promotion.md) 与 [`design/pages/topic-intelligence-reference-page.md`](design/pages/topic-intelligence-reference-page.md)；静态 HTML 只用于视觉走查，不能当成真实 API/UI 已实现的证据。
- 实施或审查 SCOPE-001：读取当前 SCOPE 后必须读取 `agents/scope-001-execution-contract.md`；当前仅按 CONTROLLED OPEN FOR TDD 推进，新增产品含义、权限或真实范围仍须停止并确认。
- 环境配置：读 `current-state.md`、[`runbooks/development-environment.md`](runbooks/development-environment.md) 和 `database/README.md`。
- 产品讨论：读 `product/PRD.md`、`context/discussion-decisions.md` 和相关 ACCEPTED ADR。
- 首页与日常入口形态：先读 [`pages/intelligence-product-blueprint.md`](pages/intelligence-product-blueprint.md) 与 [`pages/intelligence-home-threejs.md`](pages/intelligence-home-threejs.md) 了解本轮范围；历史推演再读 [`pages/page-map.md`](pages/page-map.md)，再读 [`pages/product-interface-architecture.md`](pages/product-interface-architecture.md)，最后读 [`pages/home-intelligence-surface.md`](pages/home-intelligence-surface.md)；三者均为草案，不得据此直接实现前端。
- 数据与 PostgreSQL 设计：先读 `architecture/data-architecture.md`；只有需要关系/基数时再读 `architecture/data-relations.md`，需要事务/并发/验收时再读 `architecture/data-consistency.md`，并同时遵守 `database/README.md` 和领域不变量；未确认草案不得直接生成 DDL。
- 架构或实现：先读 [`architecture/technical-architecture-baseline.md`](architecture/technical-architecture-baseline.md)，再按任务读相应专题架构、当前 SCOPE、真实代码与测试；文档不得替代代码事实。
- 来源审计：先读 `references/README.md` 和 provenance，再只打开被当前审计明确引用的 fixture、源码或 handoff。

## 入库完成标准

新文件只有同时满足“位置正确、名称稳定、状态明确、进入本索引、冲突已处理、变更已登记、检查通过”才算正式入库。

- [评论语料与每日研究 V1](plans/active/comment-daily-001.md)：历史实施记录；其 daily/旧页面路径已被 COMMENT-RESEARCH-RESET-001 terminal cutover 取代。

- [评论情报实施合同](data-contracts/comment-intelligence-v1.md)：历史归档，CI-20260907-V1 的实施合同与证据；未来架构由 COMMENT-RESEARCH-RESET-001 替代。
- [评论情报执行与交接](archive/comment-intelligence-v1.md)：历史归档，T0–T8、真实映射、文件所有权和验收记录；不再作为未来交付入口。

- [评论情报验收矩阵](design/acceptance/comment-intelligence-v1-acceptance.md)：一次性报告，84项逐条映射、实际自动证明和未验证层。
- [评论情报 UI 变更清单](design/changes/comment-intelligence-v1-ui-change-manifest.md)：历史归档，四视角、范围、状态、交互与视觉验收边界。
- [评论情报离线评测合同](audits/comment-intelligence-evaluation-contract.md)：历史归档；对应旧评测器已删除，不是 V1 质量门。

- [CREATOR-DISCOVERY-001 执行规格](plans/active/creator-discovery-001.md)：活跃计划，当前对话派定的语料与创作者集成交付。
- [CREATOR-DISCOVERY-001 实施验收](audits/creator-discovery-001-acceptance.md)：隔离验证、独立审查、浏览器限制及发布边界；2026-10-11追加PR #386八列/分区抽屉发布收口。

## TOPIC-MAP-V41-001

| 文档 | 状态 | 用途 |
|---|---|---|
| [product/topic-map-handbook.md](product/topic-map-handbook.md) | 权威当前 | 手册 D01–D39 来源与本包运行边界 |
| [plans/active/topic-map-v41-001.md](plans/active/topic-map-v41-001.md) | 活跃计划 | 表面、状态、依赖、验收与 Claim |
| [progress/2026-10.md](progress/2026-10.md) | 代码事实优先 | 本月实现与验证记录 |
| [data-contracts/topic-map-http.md](data-contracts/topic-map-http.md) | 代码事实优先 | 主题图谱读写、安全与研究接点 |
| [design/acceptance/topic-map-v41-001-acceptance.md](design/acceptance/topic-map-v41-001-acceptance.md) | 一次性报告 | 自动证明、隔离浏览器与业务验收边界 |

## TOPIC-MAP-CORE-001

| 文档 | 状态 | 用途 |
|---|---|---|
| [plans/active/topic-map-core-001.md](plans/active/topic-map-core-001.md) | 活跃计划 | Issue #382 研究内核、来源与定义身份、完整闭环；上线后研究阻塞、主题质量与规模修复验证 |

- [登录页与 Anatomy 动画](design/changes/login-anatomy-001.md)：LOGIN-ANATOMY-001，已确认 Login V2 的开发入口、源码再生与发布边界。

- [正式创作页面](design/pages/content-workbench-page.md)：WEB-REACT-001 阶段 C 的 V3 规则，#425 供给后端。
- [作品与材料使用合同](data-contracts/content-workbench-001.md)：首包typed文档、保存/引用/AI/导出边界。

阶段 E 实施入口：[运营台账五页 React 迁移](plans/active/web-react-001-e-ops.md)（Issue #438，已派定；保留当前实际 UI，候选验证不等于部署）。

阶段 E 证明：[运营台账五页验收记录](design/acceptance/web-react-001-e-acceptance.md)（候选、隔离链路与实际 UI 对照；部署及 Mog 验收独立）。

阶段 F 实施入口：[主题图谱与 Topic 工作区 React 迁移](plans/active/web-react-001-f-topics.md)（Issue #440；React迁移及已选B材料优先概览修订，截图与隔离链路证明，未部署）。

阶段 F 证明：[主题图谱与 Topic Workspace 迁移验收](design/acceptance/web-react-001-f-acceptance.md)（旧迁移保真及当前B改版实际截图、正式隔离链路，部署与 Mog 签收独立）。

主题概览 B「材料优先」正式改版：[阶段 F 计划 §7](plans/active/web-react-001-f-topics.md#7-2026-10-17--mog-选定-b材料优先正式改版)，Mog 已选定并授权；保留全部功能/口径，候选实施与验收中。

## AI-GEN1-001

| 文档 | 状态 | 用途 |
|---|---|---|
| [统一实施计划](plans/active/ai-gen1-001.md) | 活跃计划 | #454；PR-1A → PR-1B → PR-1C；各阶段权限与验收 |
| [第一代 AI 决定](decisions/0011-ai-generation-one.md) | 已确认决定 | 设置集中、skill 范围、顺序与旧决定替代关系 |
| [每篇解读材料输入](data-contracts/note-material-input.md) | 代码事实优先 | 新适配、标签、指纹/来源、就绪、快照及窄批处理 |
| [PR-1A 验收回执](audits/ai-gen1-pr1a-acceptance.md) | 一次性报告 | 合成隔离证明、收尾审查、Draft 与未证层 |

## LIDS-UI-001 · 正式前端采用

- [实施计划](plans/active/lids-ui-implementation-001.md)、[UI 变更清单](design/changes/lids-ui-implementation-001-ui-change-manifest.md)、[验收回执](design/acceptance/lids-ui-implementation-001-acceptance.md)。用户当前授权本地代码/UI实施，main/3000 未变。
- [LIDS 渐进执行标准](governance/ui-design-execution.md)与[逐项对齐](design/lids/review-alignment.md)；[前期收口审计](audits/lids-closeout-audit-2026-10-20.md)是历史边界证据，不作当前实现完成声明。

LIDS 模块索引：[字体](design/lids/typography.md)、[区域](design/lids/regions.md)、[交互](design/lids/interaction-contracts.md)、[待审与边界](design/lids/review-items.md)。


创作者页LIDS续修：同一[LIDS-UI-001计划](plans/active/lids-ui-implementation-001.md)内按P05:list/B制作真实代码候选；范围/四图与[验收](design/acceptance/lids-ui-implementation-001-acceptance.md)独立记录。Mog明确先不合并部署。

2026-10-20证据库续修：同一LIDS-UI-001真实候选采用B查询/视图合带、统一筛选/作品卡与对象详情，保留FIND/Z02领域/E1白场及三种排版；参数从LIDS单源读取。实施/回执沿本包计划与验收，Mog签收与合并部署另计。

- [COLLECTION-LIDS-UI-001：采集三页 UI 变更清单](design/changes/collection-lids-ui-001-ui-change-manifest.md) · 活跃计划；当前对话派定，依赖 #457 已提交设计基础。

- [COLLECTION-LIDS-UI-001：采集三页前端验收](design/acceptance/collection-lids-ui-001-acceptance.md) · 一次性报告；候选源码/自动检查/真实只读浏览器证据，未部署。


AI-GEN1 PR-1B 当前执行：[底座实施包](plans/active/ai-gen1-pr1b-foundation.md)；[AI底座合同](data-contracts/ai-foundation.md)。同一主任务继续，依赖未合并的#455；运行/数据/真实调用授权不继承编码目标。

- [AI-GEN1 C0离线计划](plans/active/ai-gen1-pr1c0-offline.md)：离线合同/合成流程，C1真实启用另计；[离线合同](data-contracts/note-interpretation-offline.md)。

- [每篇解读材料只读预览](plans/active/ai-gen1-note-material-preview.md)：#454/#420，Mog已确认G1；最小UI增量，运行/G4另列。

## 运营 LIDS 发布与标准件同步（2026-10-20）

- [组件同步合同](design/lids/component-sync.md)、[生成组件目录](design/lids/component-library.generated.md)：C09/C16规则、实现、展示同源。
- [运营页规格](design/pages/ops-page.md)、[创作页规格](design/pages/content-workbench-page.md)：数据/创作二级及Z02三级导航，发布证据沿LIDS-UI-001计划与验收。

- [OCR质量抽检与人工修正](plans/active/ocr-quality-audit-454.md)：#454 后续隔离实施，默认关闭；状态/证据在计划与PR。

- [OCR可选质量抽检数据合同](data-contracts/ocr-quality-audit.md)：冻结样本、盲读、人工版本、来源撤回、质量分母与未验证边界。
