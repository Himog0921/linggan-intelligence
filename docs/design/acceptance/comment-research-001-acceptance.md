# COMMENT-RESEARCH-001 验收记录

> 状态: 一次性报告
> 最后核对: 2026-09-07
> 适用范围: Issue #167 交付分支的代码与隔离证明
> 事实来源: 本包实际测试命令、真实隔离 PostgreSQL 与 API 回执
> 冲突时以谁为准: 可复现当前源码/数据库证据；不得以本报告证明未发生的真实运行

**最新增量为文末 COMMENT-DAILY-001。** 前面的 #167 记录保留原时点，不代表本次代码、共享运行或当前全局测试状态。

| 层 | 当前证据 | 边界 |
|---|---|---|
| 设计/代码 | 三视图、来源与人工语料、查询/集合、严格分析接缝、queue-only worker 已实现 | 尚未在共享服务运行 |
| Rust 编译 | `cargo check -p linggan-api -p linggan-worker --locked` | 编译不证明产品运行 |
| Rust 单测 | Intelligence 3 项；API 125 项通过、19 项隔离场景未在此命令执行 | 不把 ignored 算通过 |
| PostgreSQL | 本包 10 项真实隔离集成 + 既有评论 7 项 + API/PostgreSQL 3 项通过，容器/卷清理验证 | 测试来源合成，非真实平台覆盖/市场事实 |
| API | 隔离预览及 `verify-comment-research-api.py` 已通过（真实 HTTP + 真实隔离 PostgreSQL，合成来源） | 不触及共享数据库或当前服务 |
| UI | 根代理固定送审 HEAD 后进行真实浏览器审核 | 静态 HTML 不替代运行交互证据 |
| 真实模型 | 未接入 | 无真实 provider、敏感材料许可、费用边界或语义质量证明；完整 P01-C 未完成 |
| Merge/Deploy/Mog | 未执行 | 代码完成度不替代这些层 |

PostgreSQL 场景覆盖跨作品全数及 keyset 分页、同身份多次观察去重、字面 `%_` 检索、Unicode emoji 范围、幂等保存与冲突、服务端查询动态、集合持久化、人工修订留存与撤销、限制后原有单作品入口/新页/资产停止暴露、同源新观察不绕过限制、分析精确引用拒绝、成功无信号、失效租约恢复和旧租约拒绝。指令注入测试只证明输入合同与无工具边界，不能证明实际模型表现。

`scripts/check-rust-boundaries.sh` 主线基线已有 35 errors / 14 warnings；本包最终为 35 errors / 15 warnings：新增仅 `apps/api/src/local_web/comment_research.rs` 391 行越过 350 行 warning 线（低于 500 行 hard limit）。不写成全局通过，也不借本包重构无关历史文件。

## 可复现隔离预览

在本分支运行 `./scripts/preview-comment-research.sh`。脚本创建随机独立 PostgreSQL/container/volume，应用真实 migration、接纳 24 条显式合成评论、建一条人工资产和问题标注，再输出独立本机 URL。保持该脚本运行供浏览器检查；停止该脚本拥有的 API 时自动清理容器/卷。`python3 scripts/verify-comment-research-api.py <脚本输出的本机 origin>` 会先验证所有输入带合成标识再执行 API 保存/幂等/Origin 等场景。

真实运行前仍需：Mog 确认具体合并 head、授权共享 migration/runtime，模型 provider/费用/敏感材料处理合同，以及明确样本的语义质量验收。当前不宣称部署就绪或完整智能化已完成。

## 实施自检收口

- JavaScript `node --check`、Rust `cargo fmt --check`、三个 shell 脚本 `bash -n`、`git diff --check`、UI 手册与项目治理检查通过。
- 严格 Clippy 被未经本包修改的基线 `crates/contracts/src/producer_runtime.rs:408` 的 103/100 行函数拦截。附加 `--cap-lints warn` 仅用于发现本包新增问题，不能称严格门通过；本包新增模块/测试的已发现诊断已整改，最终诊断不再命中新模块路径。Rust boundaries 最终 35 errors / 15 warnings，新增仅上述 391 行组合文件 warning，无新增 hard error。
- 资产/查询 API 证明运行于本包临时隔离 preview；其中第一次未入账 LOCAL-001 迁移的预览按真实 schema gate 拒绝读取，已补齐合成 fixture 的准确 migration ledger，再重建隔离资源通过。没有绕过 gate 或修补共享库。
- 送审后的根代理 verdict 只以 PR 当前固定 HEAD 评论为准。本文件不写入尚不存在的 PASS，后续不会为更新审核时间戳制造新代码 HEAD。

## 集中清单整改验证

- R1：新增资产和查询追加修订。隔离 PostgreSQL 验证既有资产加入/移出/移动集合、理由更改、同时提交的冲突、撤销/删除后原始创建及旧修订重放不复活、固定原始片段不变、受限历史理由隐藏。页面提供「整理与修订」「编辑或删除」，冲突保留输入并提示刷新。
- R2：模型调用最多等待 60 秒，小于 120 秒租约；超时持久记为 provider_timeout，超过本地期限或旧租约的输出不得成功。永不返回的合成 provider 用 250ms 等待边界证明本地 Future 被丢弃、三次失败及重试上限；不证明远端取消、真实费用或语义质量。
- 本次隔离 harness 为 10 + 7 + 3，共 20 项通过；HTTP 脚本增加资产/查询修订、集合归属、撤销/删除及旧请求重放证明。独立浏览器复验仍由根代理对新固定 HEAD 进行，审核结论继续只引用 PR 评论。

## COMMENT-DAILY-001 · 2026-09-07 本地交付

基线 73f5996，codex/comment-daily-v1，单一实施及一次集中自检；问题仅复验直接影响，不新增代理或衍生审核循环。源码包含原声紧凑表格、事实/上下文、清洗映射、同作品分包、每日冻结清单、额度、逐条结果、手动重试和设置入口迁移。说明以本节及 COMMENT-DAILY-001 合同为准。

| 层 | 实际结果 | 边界 |
|---|---|---|
| 构建与语法 | API/comment worker 构建、Rust fmt、JS node --check、shell bash -n、diff 空白检查通过 | 编译不是部署 |
| 单元测试 | Intelligence 7 项通过；真实 Pi SDK 对本机 fixture 11 项通过 | 合成协议证明，不是语义质量 |
| PostgreSQL | test-model-pi-postgres.sh 40 项通过：新增 daily 8 + 原 Pi 11 + probe 1 + 原评论 10 + Evidence 7 + API 3 | 随机独立 PG/container/volume，结束清理已验证 |
| HTTP | verify-comment-daily-api.mjs 通过：v2 probe，三条同作品一次调用、900 个合成 token、结果/精确问题组、清洗筛选、日设置开关、同请求重放 | 只用显式 SYNTHETIC_PREVIEW_ONLY 和本机 fake provider |
| 浏览器 | CUA 检查连续表格、低信息筛选、0/900万点赞、文本检索、选择一条→确认模型供应商额度→批次回执、批次→原声与引用、每日设置弹窗、29 条按 20+9 分页；长文行高 54px/预览 39px | 仅合成材料；1440×900 及默认桌面视口；Mog 审美/业务验收未完成 |
| API 全量单测 | 124 通过，1 失败，19 ignored | 失败为既有 collection_stylesheet_authors_no_colour_of_its_own，shell.css 含原始颜色；相关 CSS/测试与 73f5996 无 diff。ignored 不算通过 |
| 全局边界 | 35 errors / 19 warnings | 基线尺寸告警 18；本包新增 Evidence comment_research_read.rs 368 行的 1 项 warning；原 API 文件仍低于 500 行 hard limit，无新增 hard error。全局不算通过 |
| 严格 Clippy | 被基线 producer_runtime.rs:408 的 103/100 行函数拦截 | 未修改该文件，不写成 Clippy 通过 |
| 治理 | 项目治理通过；更新当前状态、计划、页面/LIDS、数据库和运行说明 | 检查在最终提交前再执行 |
| 外部/共享 | 未发送真实评论，未迁移共享 0041，未切换 3000，未 push/merge | 本地预览和业务接受分开记录 |

新增 PostgreSQL 场景包括：23点精确边界/漏跑补偿、首次稳定身份去重、延迟清洗不漏批、同作品分包、并发一次派发、逐条部分失败与无信号、未知消耗保留及显式幂等重试、原批次数量/Token 限制、清洗筛选跨页完整性、同接纳记录的 0/未知/发布时间、同包来源限制传播，以及每日与旧自动权限互斥、连接停用后仍可暂停。

集中自检整改包括：数据库 trigger 跨表字段访问、重复引用重叠消歧、停用供应商时的暂停、模型版本一致性、旧自动互斥检查进入同一锁、状态筛选跨页、零命中文案。浏览器发现 main 的 data-view 与导航事件选择器相撞，点击筛选区会重置表单；已限定到 .lgi-research-tabs 并复验查询→勾选→确认分析→持久批次回执。

隔离预览使用 preview-model-pi.sh，29 条合成来源（含短回复/emoji/长文/点赞边界）及合成供应商。此前验证预览均已清理；最后一个按用户前端验收保留为临时交付，关闭脚本所拥有的 API 时清理自己的 worker/fixture/container/volume。不得将其中数据、用量或判断视作真实情报。
