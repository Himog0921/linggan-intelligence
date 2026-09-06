# MODEL-PI-001 实施验收记录

> 状态: 一次性报告
> 最后核对: 2026-09-06
> 适用范围: Issue #169 的实现分支，自 #168 冻结 911b1c1 起
> 事实来源: 实际 Cargo/Node/隔离 PostgreSQL/HTTP/Keychain 输出；运行方法见 model-pi-runtime
> 冲突时以谁为准: 当前固定 HEAD 与可复现回执；本记录不替代根代理审核或真实外部质量

| 层 | 实际验证 | 证明边界 |
|---|---|---|
| 编译 | `cargo check -p linggan-api -p linggan-worker --locked` 通过 | 源码可组合；不证明共享运行 |
| 单测 | Intelligence 3 通过；API 125 通过、19 个隔离场景 ignored | ignored 不算通过；其中本包需要的 3 个 PostgreSQL 场景另在隔离 harness 执行 |
| 官方 SDK | `npm test --prefix apps/pi-adapter` 8 项通过，真实 pi-ai/pi-agent-core 0.85.1 | OpenAI Completions、OpenAI Responses、Anthropic Messages 的本机协议；明确凭据、无工具、重定向/错误/凭据回显均拒绝泄露、用量缺失未知、无限流有限等待、无效请求拒绝 |
| PostgreSQL | 本包 6 项 + 原评论 10 项 + Evidence 评论 7 项 + API/PostgreSQL 3 项，共 26 项通过 | 实际随机独立 PostgreSQL，完整迁移和合成材料；容器/卷已清理 |
| Keychain | `model_keychain` ignored 独立测试实跑 1 项通过 | 随机 service/account 合成凭据写、读、更换、删除；不枚举或读取原有项、不证明用户当前真实凭据有效 |
| HTTP | `verify-model-pi-api.mjs` 通过真实 API / 独立常驻评论 worker / PostgreSQL / 官方 SDK | 连接/发现/独立 probe/用途/单条来源/标注和问题组/用量/暂停/旧请求/Origin；测试要求明确合成 store 与来源 |
| 依赖准备 | 固定 Node 24.13.0、lock 安装与 `--check` 通过 | 只在实现 checkout 安装 npm 依赖，没有调用实际 runtime install/sync 或重启服务 |
| UI 浏览器 | 根代理在本包固定 HEAD 后做集中审核 | 实施侧 JS 语法、HTTP 与页面接缝不冒充浏览器视觉/操作通过 |
| 真实外部模型 | 未执行 | 没有真实 provider/key/材料预算授权；合成 SDK proof 不证明模型理解或候选质量 |
| Merge / deploy / shared DB / Mog | 未执行或未验收 | Draft 交付不代表这些层完成 |

6 项模型 PostgreSQL 证明覆盖：设置不分析、明确单源计划→真实 SDK→严格引用/候选→用量；旧请求幂等；启用后新源与历史分界；更换默认只影响新任务；停用不派发；未知用量保留预留、并发预留不能超用/重复；错误格式仍计消耗、成功无信号、两次配置上限重试；旧配置/连接请求不恢复当前默认或停用权限；真实 SDK 无限流在配置的 1 秒等待上限停止并留下未知用量，撤回来源不发起调用。原评论 20 项覆盖父评论/源限制传播、中文与 emoji 精确片段、人工修订优先、失效租约及查询/资产不复活等。

Pi usage presence 从真实 SSE 事件记录，不能用 SDK 默认零断言零消耗。用量在业务校验前先入账，校验失败仍保留。调用中断而用量不全时持有预留或已知的更大消耗；成功写入评论而回执收口中断，可在 120 秒恢复阶段按原 comment work 状态收口。停用只阻止新派发，本地 timeout/子进程终止不承诺远端已取消或未计费。

Rust boundaries 的冻结基线已有 35 errors / 15 warnings，本包最终仍为 35 / 15，无新增应用 hard error/warning。扫描仅对 catch-all 目录检查排除安装的 node_modules，避免把上游 helpers/utils 当作自有代码；Rust 源文件扫描规则不变。严格 Clippy 的未改基线 `crates/contracts/src/producer_runtime.rs:408` 103/100 行继续阻断；`--cap-lints warn` 只用于本包新增诊断检查，不能称严格门通过。新增业务模块诊断已处理；测试复用 fixture 的四个 dead_code 警告如实保留。

最终格式、JS 语法、CSS Token 引用、bash/zsh 语法、diff 空白、项目治理及 UI 手册检查通过；审核 verdict 仅由根代理写在 PR 对应 HEAD 评论，本文件不写尚未发生的 PASS，也不为审核时间戳制造新提交。

可复现隔离运行见 [model-pi-runtime](../../runbooks/model-pi-runtime.md)。浏览器从空设置验收请重建 `preview-model-pi.sh`，不要复用 HTTP 验证已填入配置的预览。测试中没有真实材料外发、共享迁移、现有 worker 启停、插件或采集副作用。完整智能化仍不包含语义聚类、代表/边缘样本评估与真实业务质量。
