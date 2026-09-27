# COMMENT-STUDY-PRODUCTIZATION-001 · P3 Run 控制验收

> 状态: 一次性报告
> 最后核对: 2026-09-28
> 适用范围: `/corpus/comments` 研究批次 v2 Run 的暂停、恢复、停止和 CAS 回执
> 事实来源: 页面规格、Run 控制 HTTP 合同、当前源码与隔离 PostgreSQL / Axum 测试
> 冲突时以谁为准: 用户最新请求、真实代码/运行、技术合同和有效回执

## 1. 验收对象

- SCOPE：COMMENT-STUDY-PRODUCTIZATION-001，PR #338 候选分支。
- 页面/状态：研究批次列表上的 `enabled`、`paused/user_paused`、停止与终态 Run 控制动作。
- 责任文件：`comment_study.js/html/css`、local command API、Run 控制事务与隔离测试。
- 数据前提：只用脚本临时创建并清理的 disposable PostgreSQL、合成材料和 synthetic model 配置；没有调用 model/provider。
- 页面前提：本记录的 API/UI 源码尚未部署到 runtime；不从代码或 HTTP proof 推断浏览器视觉或 Mog 验收。

## 2. 场景矩阵

| 场景 | 用户任务 | 预期状态含义 | 视觉检查重点 | 真实后果/回执 | 结果 |
|---|---|---|---|---|---|
| enabled → pause | 暂停新派发 | `paused/user_paused`，既有 fence 外发仍按在途规则结算 | Run 行显示暂停/停止；成功后刷新并聚焦恢复动作 | Axum 同版本并发仅一条命令获胜，另一条收到含当前版本的 409；paused Run 不可 claim 新 batch | API/PG PASS；浏览器 NOT VERIFIED |
| paused → resume，原模型被禁用 | 恢复运行 | 恢复前置条件不足，Run 仍 paused，版本不变 | 反馈服务器错误并重读列表；不能假显成功 | 实际 Axum 收到 `policy_unavailable` 与安全当前回执，恢复后仍可在依赖恢复时显式重试 | API/PG PASS；浏览器 NOT VERIFIED |
| paused → resume，依赖可用 | 恢复派发 | 仅用户暂停可恢复；新状态 `enabled` | 成功回执后显示暂停/停止动作并将焦点移到暂停 | controlVersion 递增；事务核对领域 active、冻结方法 hash、模型可用与剩余额度 | API/PG PASS；浏览器 NOT VERIFIED |
| enabled/paused → stop | 停止运行 | `stopped/user_stopped`；不能恢复；保留合法在途结果机会 | 自定义危险动作确认展示 Run 与待处理数量 | CAS 递增；未越过 fence 的 reservation 以零费用结束并终态化目标 | API/PG PASS；浏览器 NOT VERIFIED |
| stop 与合法在途请求并发 | 停止后保留已越过 fence 的有效结果 | 已派发不是承诺可中断；合法期限内的结果可接纳，未开始/未处理兄弟项取消 | 停止回执不把未完成项显示为成功 | Axum/PG 证明未派发迟到输出拒绝、已派发有效结果接纳、其余目标取消 | API/PG PASS；provider 行为 NOT VERIFIED |
| stale controlVersion / 网络状态不明 | 再次控制 | 不猜当前状态；以服务端 Run 回执为准 | 显示状态变化并重新读取，键盘焦点回到状态/动作 | HTTP 冲突回传当前安全状态；前端没有本地乐观写 stopped/paused | 静态合同 PASS；浏览器 NOT VERIFIED |

补充控制边界：

- Run 已有 prepared batch 后再暂停：worker 不租该批次，批次保持 prepared、尝试账本不新增；同一个 Run 恢复后可立即租用该批次。
- worker 在暂停事务提交前已租出批次、但尚未预留 invocation：预留入口检测到暂停后清租约并还原为 prepared；隔离 Axum/PG 用例覆盖该串行化结果。
- reserve 已建立 invocation、但 pause 在 dispatch fence 前先提交：未派发 invocation 以零费用结束，旧批次留审计后取消，目标回到 queued 且不记语义尝试；恢复后重新冻结新批次。该交错由 Axum/PG 用例核验。
- pause/resume/stop 的 POST 已收到成功回执、随后 Runs 列表 GET 失败：页面保留“命令已由服务端确认、最新运行状态未重新读取”的状态消息并将键盘焦点落在反馈区；若命令本身没有确认，文案明确保持未确认。当前以静态 DOM 合同验证，真实浏览器故障注入仍 NOT VERIFIED。

## 3. 视觉工作条件

- 视口、真实数据密度、键盘和 200% zoom：未在候选页面上检查；页面未部署到当前 runtime。
- 已批准规则：沿用既有 LIDS token、研究批次表、停止确认对话框和真实运行状态词典；没有新增组件或视觉方向。
- 状态表达：Run state 与 dispatch state/reason 分开显示；`paused` 不等于终态，`stopped` 不等于已成功。
- 截图/录屏：无。

## 4. 分层结论

| 完成层 | 结论 | 证据 | 仍有限制 |
|---|---|---|---|
| 设计规格一致 | VERIFIED（源码合同层） | 页面规格 §5.1 与控制状态/回执交互一致 | 未作浏览器视觉检查 |
| 前端/组件实现 | VERIFIED（源码层） | 三动作 CAS 请求、停止确认、回执核对、冲突后重读、焦点恢复；静态测试 | 真实 DOM、浏览器行为与可访问性未实测 |
| 自动检查 | VERIFIED（本地） | 隔离 PG：Intelligence 8 suites 57/57、Axum/PG 9/9；Intelligence unit 123/123；API unit 292 passed/38 ignored；`cargo check`、定点 Rust 格式、Node syntax、项目治理/UI handbook、`git diff --check` | 精确 PR head CI 尚待本切片提交；CI 未运行浏览器脚本 |
| 真实链路/回执 | VERIFIED（隔离合成 Axum/PG） | 同版本并发、暂停 claim 排除、恢复前置条件、CAS stop 与派发 fence 测试 | 不含生产共享数据、真实 provider 或线上费用 |
| 部署 | NOT VERIFIED | 无本地 runtime 切换、无 shared migration | 当前 `runtime-main@2587fba4` 仍是主线版本 |
| Mog / 业务验收 | NOT VERIFIED | 尚未进行 | 需要用户在实际页面检查数字和动作 |

## 5. 发现与后续

- 控制源码是 PR #338 候选；HTTP/PG proof 只证明隔离 synthetic 路径。
- 本次没有注册或应用共享 migration，没有 provider 调用、部署、runtime 重启、合并或修改 T01–T54 状态。回归使用脚本创建并清理的 disposable PostgreSQL 环境；cargo 输出的 unused/dead-code warnings 属于现存警告。
- 下一步先完成提交前独立复核、push 与 exact-head CI；随后仍需 P2/P3 浏览器和 Mog 验收。公平调度、全阶段失败/部分接纳与 runtime drain 仍属于 P3 后续工作；Recall@K 等待独立人工 Gold Set。
