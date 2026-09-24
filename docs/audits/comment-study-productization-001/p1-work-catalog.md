# P1 作品目录与共享显示标题

> 状态: 最新主线整合后，隔离 PostgreSQL 与合成数据浏览器验证通过；P1整体未结项
> 最后核对: 2026-09-24
> 适用范围: COMMENT-STUDY-PRODUCTIZATION-001 / PR #338 的 P1 作品读取增量
> 事实来源: 分支起点 1849bdbf、原核对 main b9ff8237、当前集成 main a42315eb、实际工具回执
> 冲突时以谁为准: 已批准数据库/HTTP合同、实际测试与对应 Git diff

## 1. 本轮边界与主线差异

Mog 已连续授权按手册开发、追加 PR #338，并要求明确剩余工作。本轮实现 HTTP §2 的 `/works` 后端，复用既有目录资格与 Evidence 显示标题；不更改原型或四 Tab 产品结构。

初次制作本回执时，main 为 `b9ff8237`，PR 分支尚未同步后续主线；当时的候选以 `1849bdbf` 为父提交，产品化迁移文件名为 `0102_comment_study_productization_schema.sql`。该段记录的是初次实施时的主线差异，最新对齐结果见下方追加回执。

## 2. 实际增量与复用

- `comment_study_catalog/works.rs`、`works.sql`：默认50、上限100；作品 UUID 正序 keyset；中文及通配符样式输入按字面标题匹配；搜索先于分页，不在前100篇中筛选。当前领域已接纳的零评论作品也保留。
- `GET /api/local/comment-study/works`：继承当前 Host/Origin、no-store 与安全错误映射；GET 不清洗、不创建 Run、不调用模型。
- 作品行提供手册规定的 workRef、displayTitle/source、合格/已索引/待索引/已处理/处理中/待语境评论数和上次研究时间；独立 totalWorkCount 不以页长代替总量。各研究计数不是互斥进度，也不宣称市场规模。
- 评论目录原 SQL 的来源/历史/资格关系抽到 `facts.sql`，作品和评论同时使用；只调整静态拼装和参数顺序，不再复制一套作者或缓存资格 CASE。旧冻结选择器尚未接通该关系，不能把此局部共享宣称全链完成。
- 显示标题通过 Evidence 现有 `work_resource_currents_sql()` 的固定字段 Current 读取；不在 Intelligence 复制另一份 native 标题选择。固定 SQL 组合仅适配编译期占位符，调用参数均绑定，不提供用户可写 SQL 或通用查询 DSL。
- 原 `apply_cover_ocr_title_fallback` 的选择 SQL 提取成 `work_resource_read/cover_headline.sql`，既有 Evidence renderer 和新目录共用。原有平台标题优先、仅封面/前3张图的 cover_headline 可回退规则保留；不用 OCR 长文、评论或模型摘要充标题。
- 共享 OCR 选择增加已接纳 origin/as-of、处理成功、retirement、当前媒体 restriction 的检查与稳定排序。它同时影响原 Evidence 标题回退，属于必须进行跨页面回归的安全收束，不是零风险样式修改。
- 标题查询只返回目录所需字段；没有逐作品调用完整详情 API 的应用层 N+1。数据库内部成本尚未实测，不能据此声称已经达成百万数据量或 p95 目标。

## 3. 输出语义与未完成项

`studiedCommentCount` 继续表示当前可展示评论中曾有成功 Target head 的数量；它不是完整输入条件均一致的当前有效知识数。P2 的作品/父语境 fingerprint、P4 的有效关系和支持去重仍需完成。`input_changed` 继续明确拒绝，不能偷偷用正文差异代替完整输入变化。

原始目录回执完成时，作品选择器尚未切换新接口。PR #338 后续提交已把弹窗接到 `/works`，并实现用户评论、筛选、详情和历史读取；本回合在合并后的浏览器对这些路径作了合成数据验证。P1的全部语境资格、冻结选择与边界状态仍须按手册完成整体验收。

不新增表、依赖、Worker、执行通道或 migration；不改手册字段。`0103_comment_study_productization_schema.sql` 仍是候选，未注册或执行到共享库。无共享库、真实模型或计划副作用。

## 4. 测试与真实验证边界

新增7个单元测试候选：共享标题/缺OCR schema/封面资格3项，作品参数/游标/搜索分页4项。新增4个隔离PostgreSQL测试候选：125作品完整翻页及第100篇后搜索、资格计数与零评论作品、共享Evidence平台标题与字面搜索、跨筛选游标拒绝。旧四组数据库proof target全部保留，并追加 work catalog target。

实际运行：

- 16项本地源码形状检查通过：4个已保存文件与 Git blob SHA 一致；九参数绑定、有界读取、独立总数、搜索先于分页、125作品fixture等。此计数不是业务测试，不等于 Rust/SQL 执行。
- `cargo +stable check -p linggan-intelligence -p linggan-api --locked`：PASS。
- `cargo +stable test -p linggan-intelligence -p linggan-api --locked`：PASS。
- `scripts/test-comment-study-productization-postgres.sh`：PASS，6 个隔离 target 共 22/22；脚本清理了其专用临时容器与 volume。
- 本机 API 使用独立 PostgreSQL 容器、主线完整 schema、clean-study bootstrap、未注册的 0103 候选及合成记录，在 `127.0.0.1:3011/corpus/comments?view=overview` 进行浏览器验证。页面/health/setup/works 均 HTTP 200；概览读取 4 条原始评论、2 条可研究评论；列表展示 3 条用户声音，其中未知身份仍可读但不可研究，作品作者声音单独筛出且不可研究。字面搜索、评论详情、清洗文本、空历史与 Escape 关闭、作品搜索及选择均可用。
- 手动验证没有提交研究运行或保存策略。没有模型配置时策略按钮禁用；选择一篇零条可研究评论的作品后，“创建研究运行”仍可点，本次停止在提交前，列为 P2 启动合同的复验/修正规则。
- T43：PASS（第 101 条之后的服务端搜索由 `all_125_works_are_pageable_and_titles_after_the_first_100_are_searchable` 覆盖，弹窗另经真实本地 HTTP 搜索验证）。其余 T01–T54 未达到完整案例覆盖，保持 NOT_RUN。
- Rust fmt/clippy、OCR 回退专项、EXPLAIN/性能、未覆盖 T 项、真实模型、用户数据库保护盘点、部署与 Mog 业务验收：NOT_RUN。

本地分支已合入 `origin/main@a42315eb`；对应代码修正提交为 `d5b2d0cd`。主线同步已完成，但 PR 仍为 Draft；月报正文安全追加、原型入库、P0现场保护盘点及完整发布验收仍待后续执行。

## 5. 下一执行点

先完成 P1 全量验收中的完整语境、冻结来源资格一致性和剩余边界状态。下一开发阶段是 P2 启动与不可变方法合同；本次观察到的空选择启动按钮应并入 P2 验收。P3–P5、真实模型质量、部署与业务验收未完成。剩余阶段统一登记在 [来源与验收账本](README.md)，原技术手册不改写。

## 最新主线整合与浏览器验证 · 2026-09-24

分支已合并 `origin/main@a42315eb`。主线占用 `0102_cross_industry_creator_directory`，因此本包迁移候选及五个隔离 PG proof 均改用 `0103_comment_study_productization_schema.sql`；该迁移仍未注册进 `local-runtime.sh`，没有修改共享数据库。LIDS 的负 8 像素间距改为现有 spacing token。

上述编译、单元、22 个 PostgreSQL proof 与浏览器操作都针对代码提交 `d5b2d0cd`。浏览器用全新合成数据库，没有接触已存在容器、共享数据库、模型或用户页面 `:3000`。隔离 API/数据库留在本机供 Mog 继续查看；不是部署结果。
