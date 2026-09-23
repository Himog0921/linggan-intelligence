# P1 作品目录与共享显示标题

> 状态: 已编写候选；Rust/PostgreSQL/浏览器未运行，不是已上线声明
> 最后核对: 2026-09-23
> 适用范围: COMMENT-STUDY-PRODUCTIZATION-001 / PR #338 的 P1 作品读取增量
> 事实来源: 分支起点 1849bdbf、main b9ff8237、当前变更与实际工具回执
> 冲突时以谁为准: 已批准数据库/HTTP合同、实际测试与对应 Git diff

## 1. 本轮边界与主线差异

Mog 已连续授权按手册开发、追加 PR #338，并要求明确剩余工作。本轮实现 HTTP §2 的 `/works` 后端，复用既有目录资格与 Evidence 显示标题；不更改原型或四 Tab 产品结构。

main 已从 c74d72e3 前进到 b9ff8237，两次提交涉及跨行业作者目录等功能。候选以 PR 自己的 1849bdbf 为父提交，不覆盖 main，不把其他交付回退。main 新增 `0102_cross_industry_creator_directory`，本包未注册的 `0102_comment_study_productization_schema` 为不同完整 migration ID；最终集成仍需核对序列与 ledger，不能只凭相同数字前缀重写已应用文件。本轮没有同步合并 main，也不声称 PR 无冲突。

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

本轮只完成 `/works` 后端候选。现有 HTML/JS 作品选择器还未切换新接口，因此用户本机弹窗的100篇限制仍不能宣称已经消失；T43必须在真实接口和前端连接后验收。完整作品上下文、冻结来源统一、清洗后台接入与用户评论 UI 仍在 P1 待办。

不新增表、依赖、Worker、执行通道或 migration；不改手册字段。0102候选继续不注册、不执行。无共享库、真实模型或计划副作用。

## 4. 测试与真实验证边界

新增7个单元测试候选：共享标题/缺OCR schema/封面资格3项，作品参数/游标/搜索分页4项。新增4个隔离PostgreSQL测试候选：125作品完整翻页及第100篇后搜索、资格计数与零评论作品、共享Evidence平台标题与字面搜索、跨筛选游标拒绝。旧四组数据库proof target全部保留，并追加 work catalog target。

实际运行：

- 16项本地源码形状检查通过：4个已保存文件与 Git blob SHA 一致；九参数绑定、有界读取、独立总数、搜索先于分页、125作品fixture等。此计数不是业务测试，不等于 Rust/SQL 执行。
- `bash -n scripts/test-comment-study-productization-postgres.sh`：PASS。
- 实际执行该脚本：退出1，`Cargo is unavailable`；未进入Docker或数据库步骤。
- Rust编译/fmt/clippy、PostgreSQL、OCR回退真实fixture、EXPLAIN、浏览器、全仓库治理：NOT_RUN。
- T01–T54保留NOT_RUN；本轮为T43/T45/T46等补代码和验证用例，不把它们自动勾为PASS。

提交前回读 Git diff，确认大文件只发生本轮接缝变化。主线合并、月报正文安全追加、原型入库仍需后续整合，不能用此说明替代相关检查。

## 5. 下一执行点

P1先完成完整作品语境与原冻结资格统一、后台有界清洗接入和用户评论页面，再对照P1可验收结果执行真实测试。P2–P5不跳过、不因已有SQL候选宣布完成。剩余阶段统一登记在 [来源与验收账本](README.md)，原技术手册不改写。
