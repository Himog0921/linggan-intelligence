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
| PostgreSQL | 本包 9 项 + 原评论 10 项 + Evidence 评论 7 项 + API/PostgreSQL 3 项，共 29 项通过 | 实际随机独立 PostgreSQL，完整迁移和合成材料；容器/卷已清理 |
| Keychain | `model_keychain` ignored 独立测试实跑 1 项通过 | 随机 service/account 合成凭据写、读、更换、删除；不枚举或读取原有项、不证明用户当前真实凭据有效 |
| HTTP | 扩展后的 `verify-model-pi-api.mjs` 通过真实 API / 独立常驻评论 worker / PostgreSQL / 官方 SDK | 连接/发现/独立 probe/用途/两条来源/标注和问题组/用量/暂停/显式恢复/过期恢复冲突/所属计划定位/旧请求/Origin；测试要求明确合成 store 与来源 |
| 依赖准备 | 固定 Node 24.13.0、lock 安装与 `--check` 通过 | 只在实现 checkout 安装 npm 依赖，没有调用实际 runtime install/sync 或重启服务 |
| UI 浏览器 | PASS；Mog 解锁后根代理在同一固定 head 完成真实操作与 1440×900 / 390×844 走查 | 仅证明合成隔离预览中的页面操作、状态表达和布局 |
| 真实外部模型 | 未执行 | 没有真实 provider/key/材料预算授权；合成 SDK proof 不证明模型理解或候选质量 |
| Merge / deploy / shared DB / Mog | #168、#170 已合并到 main；共享迁移、部署和真实业务验收未执行 | 合并不代表共享运行或真实外部质量完成 |

原有 6 项模型 PostgreSQL 证明覆盖：设置不分析、明确单源计划→真实 SDK→严格引用/候选→用量；旧请求幂等；启用后新源与历史分界；更换默认只影响新任务；停用不派发；未知用量保留预留、并发预留不能超用/重复；错误格式仍计消耗、成功无信号、两次配置上限重试；旧配置/连接请求不恢复当前默认或停用权限；真实 SDK 无限流在配置的 1 秒等待上限停止并留下未知用量，撤回来源不发起调用。原评论 20 项覆盖父评论/源限制传播、中文与 emoji 精确片段、人工修订优先、失效租约及查询/资产不复活等。

## 固定 ff95268 后的集中整改

本次只处理根代理唯一清单 R1/R2/R3，新增 3 项模型 PostgreSQL 回归，连同原有 26 项全部重跑通过：

- R1：原计划两条来源已完成一条后暂停；新补跑返回 0 并定位原计划，显式恢复后只执行剩余一条。旧创建请求和旧恢复修订不能解除再次暂停；切换默认后仍使用旧配置，来源数、尝试次数及已用额度保留。HTTP 另外验证恢复接口、409 冲突及所属计划链接数据。
- R2：24 条可读合成来源，首条属于另一份暂停试运行，自动计划额度 1。按全局来源/规则/配置幂等条件选中下一条，连续同步只纳入 1 条，真实 SDK 仅调用一次。只回拨隔离计划的启用时间模拟先启用后入库，没有改动 Evidence。
- R3：无其他工作、余额不足、最多 1/2 次已耗尽、评论已成功但 invocation 未收口，以及旧恢复遗留的超限 pending 分别验证。维护事务均持久化，未知用量和 18,000 token 预留保留；只有预置的 5 条调用回执，没有新增模型调用。

本次修改不涉及官方 SDK 或 SecretStore；8 项 SDK 和随机 Keychain 的通过结果沿用首轮实施证据，不冒称此次重新执行。最终复验结论仅见 PR #170 对新固定 HEAD 的根代理评论。

Pi usage presence 从真实 SSE 事件记录，不能用 SDK 默认零断言零消耗。用量在业务校验前先入账，校验失败仍保留。调用中断而用量不全时持有预留或已知的更大消耗；成功写入评论而回执收口中断，可在 120 秒恢复阶段按原 comment work 状态收口。停用只阻止新派发，本地 timeout/子进程终止不承诺远端已取消或未计费。

Rust boundaries 的冻结基线已有 35 errors / 15 warnings，本包最终为 35 / 16；新增一项为 `model_pi_postgres.rs` 689 行超过测试文件 600 行软阈值，未超过 900 行硬阈值，无新增应用 hard error/warning。扫描仅对 catch-all 目录检查排除安装的 node_modules，避免把上游 helpers/utils 当作自有代码；Rust 源文件扫描规则不变。严格 Clippy 的未改基线 `crates/contracts/src/producer_runtime.rs:408` 103/100 行继续阻断；`--cap-lints warn` 只用于本包新增诊断检查，不能称严格门通过。新增业务模块诊断已处理；测试复用 fixture 的四个 dead_code 警告如实保留。

最终格式、JS 语法、CSS Token 引用、bash/zsh 语法、diff 空白、项目治理及 UI 手册检查通过；审核 verdict 仅由根代理写在 PR 对应 HEAD 评论，本文件不写尚未发生的 PASS，也不为审核时间戳制造新提交。

可复现隔离运行见 [model-pi-runtime](../../runbooks/model-pi-runtime.md)。本次浏览器验收从空设置重建 `preview-model-pi.sh`，未复用 HTTP 验证已填入配置的预览。测试中没有真实材料外发、共享迁移、现有 worker 启停、插件或采集副作用。完整智能化仍不包含语义聚类、代表/边缘样本评估与真实业务质量。

## 2026-09-06 基础配置返修（#169，codex/model-settings-repair）

本节对应 main `4e8744e` 之后的返修，替代旧四区常驻交互；由根代理集中实现与一次类别自审，未增加 reviewer 或审核循环。

- 地址：基础地址与完整调用地址均规范化；预览最终 URL，保留自定义网关前缀，禁止 URL 内凭据、query/fragment 和未明确允许的 HTTP。三种协议的地址单测 1 项通过。
- 配置：一个供应商列表与一个可复用弹窗，填写名称、协议、API 地址、API Key、模型 ID；目录是可选辅助。“保存”没有供应商调用；“保存并测试”直接调用所选模型。未通过目录接口不再阻止调用测试。
- 状态：测试成功显示耗时，并分别显示模型可调用和评论输出校验；调用失败保留弹窗输入。编辑同目标可沿用已存凭据，改变地址/协议须重新提供。未初始化明确提示并禁用配置操作；恢复后刷新解除禁用并清除旧错误。
- 自动验证：`cargo check -p linggan-api --locked`、最终 `cargo build -p linggan-api --locked`、地址单测、Node 语法、格式和 diff 检查通过。真实 Pi 0.85.1 SDK 本机 9 项通过；隔离 PostgreSQL 本包 11 + 评论 10 + Evidence 7 + API 3，共 31 项通过，harness 容器和卷清理已验证。
- 浏览器：CUA 在独立合成预览验证完整 `/v1/chat/completions` 地址保存为 `/v1`，直接调用成功；错误模型失败并保留输入；留空凭据继续测试；仅保存不调用；临时移走并恢复隔离 schema 后检查禁用和恢复文案。实际截图检查了单页宽度及弹窗，修正移除侧栏后遗留的 grid 占位，弹窗固定显示标题及保存/测试按钮，字段区独立滚动。
- 基线：同日 main 边界扫描 35 errors / 16 warnings，返修 35 / 17。新增 `model_settings.rs` 364 行超过 350 软阈值，未超过 500 硬阈值；测试文件从 689 增至 837 行，仍在 900 硬阈值内。不把既有 35 项错误报告成通过，也未借本修复扩展全仓重构。
- 运行事实：共享库 migration ledger 截至 0038，缺少现有 0039/0040，3000 模型设置 API 返回 503。此次未迁移共享数据库、未更新 3000、未调用真实外部供应商或真实评论。源码、合成证明和 Mog 实际验收分别记录。

## 2026-09-07 DeepSeek 输出上限返修（同 #169）

共享只读回执确认：deepseek-v4-flash / Responses 的两次 probe 分别耗时 8881/7620 ms，均为 459 输入、1024 输出、output_limit；未保存思考 token 分项，不能声称已证明 1024 token 全用于思考。官方默认思考与本机 reasoning=false 未发送关闭参数的缺口有代码及官方文档依据。

- SDK 回归：新增本机 HTTP fixture 拦截官方域名请求，不发往外网。修复前真实 Pi 得到同类 output_limit/459/1024；修复后 Responses probe/analyze 发送 reasoning.effort=none 并完成。Chat Completions probe/analyze 发送 thinking.type=disabled。未知模型、相似域名不套用，1024 上限及单次调用保持。全部 11 项通过。
- PostgreSQL：原 31 项 + 新截断语义 1 项，共 32 项通过，独立容器/卷清理已验证。即使返回可解析 JSON，只要结束为 length 仍拒绝接纳；modelCallable=true、commentQualified=false、1824 token 用量留存、重放不新增调用、不能保存为默认。
- CUA：独立合成预览的 synthetic-limit 实际通过 API/SDK 返回截断；弹窗与供应商行显示“模型已响应，但输出达到本次上限，评论分析结果不完整”，默认选择中没有该模型，输入保留。没有打开或代填真实凭据、没有外部复测。
- Cargo 编译由完整 proof/preview 构建实际执行；JS 语法、fmt、diff、项目治理检查通过。证明的是已支持官方 V4 文本调用参数及截断保护，不是通用推理配置或真实模型分析质量。
