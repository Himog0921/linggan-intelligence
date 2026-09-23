# P1 闭环实施与验收

> 状态: P1候选经隔离数据库与合成浏览器验证；非整包通过声明
> 最后核对: 2026-09-24
> 适用范围: COMMENT-STUDY-PRODUCTIZATION-001 / P1 / PR #338
> 事实来源: exact head 7f22b14e、已批准手册、CI run 35848941778
> 冲突时以谁为准: 当前用户授权、固定手册与实际测试
> 当前集成源码: `d5b2d0cd`（本地合入 `origin/main@a42315eb` 后）

## 本轮范围

修复 SHA-256 格式化的实测编译错误；补齐完整作品与父语境、统一目录与选样资格、有界清洗 tick；接用户评论/作品选择和历史下钻；运行编译、单元、隔离 PostgreSQL 和真实 HTTP 浏览器验证。保留概览上半区，不实现 P2 方法/默认排重、P3 调度/额度和 P4 有效知识。共享迁移、真实模型、合并部署未授权。

## 表面、状态、依赖与验收

表面为评论页、作品选择弹窗和评论检查器；共享 shell/Token 不改。状态分别处理已索引部分、无匹配、读取失败、受限、正文未知、无效文本、研究失败仍有旧结果。后端只复用现有材料、clean.v2、catalog 与 source；GET 不写缓存或建 Run。对应 T05/T06/T07/T43/T44/T45/T46/T47/T48 的 P1 子场景，其他阶段不能因 P1 通过而勾选。

## 第一项实测修复

CI 在 sha2 0.11 的摘要 LowerHex 格式化处发现两处 E0277。目录缓存及游标复用逐字节小写 hex，不降级依赖、不修改哈希语义。源码与上传 blob 已核对；后续以新 exact-head CI 回执为准。独立 commit-reviewer 工具不可用，未冒充独立审查，最终交 Codex。


## P1-B · 用户评论与作品目录已接通

exact head `7f22b14e` / CI run `35848941778`：Rust compile、单元测试、隔离 PostgreSQL proof 全部 PASS。页面已经从旧 run-scoped「评论目标」切到全库「用户评论」，接入评论搜索/筛选/keyset、评论详情、父语境、清洗文本与可继续翻页的研究历史；研究发起弹窗改用服务端 `/works` 搜索/翻页，跨页选择保持且最多 100 篇。此处不把 P2 的最终启动合同或 P4 的四 Tab 重构提前算入 P1。

## P1-C · 有界清洗维护

本提交将既有 `maintain_comment_catalog` 接入原模型 Worker 的一次 tick 入口：每 tick 最多处理 128 个缺当前 cleaner 版本的 raw head，随后继续原 pair/resolution/semantic 链，不因本地清洗成功而提前 return。缺 `clean_cache` 表时返回 no-op，不能借此 bootstrap/reset；数据库错误按既有 Worker 错误边界结算。已有隔离 PG 用例证明 130 条会按 128+2+0 三次推进且不创建 Run / model invocation；本提交仍需 exact-head CI 复验。

## 最新主线整合与本机只读浏览器回执 · 2026-09-24

- `origin/main@a42315eb` 已合入独立 PR worktree；合并提交为 `1c43495d`。迁移号冲突已修正为 `0103_comment_study_productization_schema.sql`，仍未注册或应用到共享运行路径。
- 合并后 `cargo +stable check -p linggan-intelligence -p linggan-api --locked` 与 `cargo +stable test -p linggan-intelligence -p linggan-api --locked` 均通过。`scripts/test-comment-study-productization-postgres.sh` 的 6 个隔离 target 共 22/22 通过；这些是选定 P1 proof，不是 T01–T54 全量。
- 本机 HTTP 页面连接本轮新建的合成 PostgreSQL；健康检查、HTML 页面、setup 和作品目录 API 均返回 200。浏览器验证了概览计数、用户评论列表与未知声音标注、按“老师”搜索、详情中的原声/清洗文本/空研究历史、Escape 关闭、作品作者声音筛选、作品目录搜索和复选。
- 作品选择弹窗没有启用模型配置，策略保存按钮禁用；选中零条可研究评论的作品后，启动按钮仍启用。没有点击启动，因此没有创建 Run；该按钮条件需要与 P2 `no_work`/`index_pending` 启动语义一起修订或证明。
- T43 在 `acceptance-status.json` 标为 PASS；其他 T 编号仍按完整案例覆盖要求保持 NOT_RUN。P0用户库历史/在途请求盘点、真实模型质量、P1全量边界、P2–P5、迁移/部署和 Mog 业务验收仍未验证。
- 浏览器地址：`http://127.0.0.1:3011/corpus/comments?view=overview`。页面当前停在默认用户评论视图。合成容器与 API 故意保留给 Mog 手动查看，不复用 `:3000` 或已有容器。
