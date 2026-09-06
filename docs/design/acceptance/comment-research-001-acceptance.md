# COMMENT-RESEARCH-001 验收记录

> 状态: 一次性报告
> 最后核对: 2026-09-06
> 适用范围: Issue #167 交付分支的代码与隔离证明
> 事实来源: 本包实际测试命令、真实隔离 PostgreSQL 与 API 回执
> 冲突时以谁为准: 可复现当前源码/数据库证据；不得以本报告证明未发生的真实运行

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
