# ACC-COMMENT-STUDY-TABS-001 · 评论研究复核标签页验收记录

> 状态: 一次性报告
> 最后核对: 2026-09-17
> 适用范围: Issue #295 的 `/corpus/comments` 复核区（概览/评论目标/待归并/用户问题/运行记录 5 个标签页）；不覆盖初始研究输入面（见 ACC-COMMENT-STUDY-LAYOUT-001）、模型 adapter 或部署验收
> 事实来源: `codex/comment-study-tabs-001`、隔离 PostgreSQL 证明、页面静态测试
> 冲突时以谁为准: 当前代码、真实运行回执、用户最新确认与 AGENTS.md

## 起因

Mog 在合并部署 COMMENT-STUDY-LAYOUT-001 后发现新版评论研究页看不到历史用户原声、当前运行情况与运行结果。核查发现 `PAGE-COMMENT-STUDY-REBUILD-001` 已批准的信息架构（概览、评论目标、待归并、用户问题、运行记录 5 个文字 Tab）此前只完成了只读 API（`comment_study_read.rs`），页面本身从未把这 5 个视图实现出来——只有顶部"创建一次研究"工具栏和三个迷你统计框上线。本包按已批准设计把复核区补齐，不重新设计信息架构。

## 本次已验收的事实

- 页面复核区改为 5 个文字 Tab（`ADR-11`：字重 + 3px 信号下划线，无分段控件），与既有共享工作区骨架和 LIDS token 一致；默认选中"概览"。
- **评论目标**标签页新增对原始评论正文的读取：后端 `read_targets` 新增 `commentText`/`sourceState` 字段，直接关联 `linggan_material_comment.body_text`，并在返回前实时核对 `linggan_material_comment_restriction`——一旦来源在冻结之后被限制，文字改为如实显示的限制说明，不返回已失效原文（隔离证明 `read_targets_hides_comment_text_once_the_source_becomes_restricted_after_freeze` 覆盖这一行为）。原声使用 Serif（`--lgi-font-evidence`）呈现，与旧版评论研究页的排版约定一致。
- **待归并**标签页从既有 `signals` 只读接口按 `resolutionState`（`pending`/`deferred_context`/`deferred_ambiguous`/`deferred_novel` 或尚无 resolution）筛出仍在等待归并判断的 Signal，说明每种等待状态的具体含义，不与已终态的 `not_user_problem`/`protocol_rejected`/`failed`/已归并的 `assigned` 混淆。`read_signals` 同样新增 `sourceState` 并核对 `linggan_material_comment_restriction`：Signal 的 `evidence` 是被冻结评论原文的逐字子串、`proposition` 是其摘要，来源被限制后两者一并置空，卡片改为如实显示限制说明（提交前审核发现的问题，见下）。
- **用户问题**标签页展示已建立的稳定 Problem 定义、纳入/排除条件与关联 Signal 数，区分 `active`/`retired`。
- **运行记录**标签页把原本挤在迷你框里的运行统计展开为完整表格：每次运行的作品数、目标数与逐状态计数（已产出/无信号/等待语境/失败/来源受限）。
- **概览**标签页保留原有的最新运行摘要，但把状态计数从纯数字堆砌改为带中文标签的独立统计块。
- 所有技术状态词（`target.state`/`signal.eligibility_state`/`resolution.state`/`problem.state`）在前端都有对应的中文说明；未识别的取值原样显示而不是被吞掉或替换为空——不制造虚假的"正常"空态。
- 本次不改变 StudyRun/Policy/Batch 的写入路径、不新增写接口；`{overview,runs,problems}` 的既有字段语义不变，`targets` 追加 `commentText`/`sourceState`，`signals` 追加 `sourceState` 并把 `evidence`/`proposition` 改为受限时可空。

## 提交前独立审核（commit-reviewer，逐条裁定）

审核发现 3 条成立的真实缺陷（1 条高、1 条中、1 条低）与 1 条既有模式说明，全部已核实并处理：

1. **成立（高）· 已修** — `read_signals`（"待归并"标签页的数据源）完全没有做 `read_targets` 刚建立的"来源限制后不再返回原文"核对。`Signal.evidence` 被 `comment_study_semantic.rs` 保证是被冻结评论原文的逐字连续子串，等价于一处引用；本次已修：`read_signals` 追加与 `read_targets` 相同的 `linggan_material_comment_restriction` 核对，限制后 `evidence` 与 `proposition`（摘要，同样属于血缘传播范围）一并置空，新增 `sourceState` 字段，前端"待归并"卡片相应显示限制说明而非继续引用原文。新增隔离测试 `read_signals_hides_evidence_and_proposition_once_the_source_becomes_restricted_after_freeze` 覆盖。
2. **成立（中）· 已修** — Tab／运行切换存在竞态：旧代码里较慢的请求可能在用户已经切到另一个 Tab 之后才返回，并把已经过期的内容写回 DOM，导致高亮的 Tab 与实际显示内容不一致。已修：5 个渲染函数改为返回 HTML 字符串而不是直接操作 DOM，`renderActiveTab` 用自增的 `renderToken` 判断"这次渲染是否已经过期"，过期结果直接丢弃。新增静态断言 `comment_study_script_discards_a_stale_tab_render_instead_of_overwriting_a_newer_one` 覆盖。
3. **成立（低）· 已修** — `sourceState:"unknown"`（评论正文非 `KNOWN`）分支此前没有任何测试真正执行到。补充测试时发现 `linggan_material_comment` 受数据库触发器保护、直接 `UPDATE` 会被拒绝（"append-only"），进一步证实这条分支在当前设计下确实不可达，只能通过测试夹具在**建档时**就构造一条正文缺失的评论、绕过来源资格闸门直接接到 Run 上来触发。新增隔离测试 `read_targets_reports_unknown_source_state_without_panicking_when_body_text_is_absent`，同时在代码注释里写明它验证的是一个当前不可达、仅作防御性验证的组合。
4. **信息性，无需处理** — 页面静态测试是对源文件做字符串包含匹配，不执行 JS/DOM，测不出竞态一类的运行时问题。这是本文件既有的测试风格（早于本次改动），本次沿用而非新引入；已通过新增隔离测试和第 2 条的重构来弥补这类测试的盲区，而不是假装静态测试能覆盖运行时行为。

修复后重新在隔离 PostgreSQL 上跑了一次全套（含新发现问题引出的两处测试内容修正：一处是我最初的断言用错了半角逗号，跟原始评论文字的全角逗号不一致；`accept_semantic_output` 会把 model 给出的 evidence 重新定位到原文中的逐字子串再入库，这是设计如此，不是缺陷，已改断言去匹配实际存储值）。

## 自动与隔离证据

| 层次 | 结果 | 证据 |
|---|---|---|
| Rust 页面静态测试 | PASS | `cargo test -p linggan-api local_web::comment_study::tests --locked`：**11 passed**、0 failed（4 项既有 + 7 项本次新增，含审核后新增的竞态防护与信号限制两项断言） |
| 隔离 PostgreSQL 证明 | PASS | `bash scripts/test-comment-study-rebuild-postgres.sh`：**17 passed**、0 failed（13 项既有 + 4 项本次新增：已知来源返回原文、来源冻结后被限制时原文消失但目标生命周期状态不受影响、Signal 侧同等限制核对、`unknown` 来源状态防御分支） |
| 编译 | PASS | `cargo check -p linggan-api -p linggan-intelligence --locked` |
| JavaScript 语法 | PASS | `node --check apps/api/src/local_web/comment_study.js` |
| 变更完整性 | PASS | `git diff --check` |
| LIDS 设计手册核对 | PASS | `bash scripts/verify-ui-design-handbook.sh` |
| 项目治理检查 | PASS | `bash scripts/check-project-governance.sh` |

## 顺带修复的既有编译阻断（与本次功能无关，但阻断了本次测试文件编译）

`crates/intelligence/tests/comment_study_rebuild_postgres.rs` 中既有用例 `source_gate_excludes_withdrawn_ocr_but_keeps_the_comment_target` 仍用 `.unwrap().unwrap()` 处理 `claim_media_processing_work`，而 PR #299（`MEDIA-CLAIM-MISSING-MATERIAL-001`）已把返回类型从 `Option<Claim>` 换成三态枚举 `MediaProcessingClaimOutcome`。这处遗留没有被 #299 自己的验证覆盖到——它需要跑本文件所在的隔离 PostgreSQL 套件才会暴露，而 #299 的验证跑的是另一个套件。已按仓库内既有写法（`crates/evidence/tests/media_processor_requeue_postgres.rs`）改为 `match ... { MediaProcessingClaimOutcome::Claimed(claim) => claim, other => panic!(...) }`，不改变该用例本身要验证的行为。

## 事故记录：临时预览清理时误伤线上 API 服务

验收过程中启动了临时本机预览（`127.0.0.1:3001`，读同一开发库、不新建 Run）以准备真实浏览器走查。`chrome-devtools` MCP 工具的 `new_page` 调用挂起 6 分钟无响应，判断为工具连接故障后中止走查。清理临时预览时用 `pkill -f "target/debug/linggan-api"` 停止 3001 进程，因常驻 `runtime-main` 的二进制路径同样以 `target/debug/linggan-api` 结尾而被一并误杀——这正是部署手册明确写着"不要用 pkill 停服务"的那个坑。核实：launchd `KeepAlive` 同秒自动重启 API（PID 47271 → 89327），巡检 worker（47278）与媒体 worker（47290）PID 不变、未受影响；重启后 `/health` 返回 `READY`、scheduler `running`、`lastTickCompletedAt` 为最新一次 tick。影响范围是 API 服务几秒内不可用，未发现数据、任务或调用账本丢失。

## 未由本次证明的层次

| 层次 | 状态 | 原因 |
|---|---|---|
| 共享 `main` / PR / 推送 | NOT VERIFIED | 本次只在隔离 worktree 验收，未获本轮提交、推送或合并授权 |
| `:3000` runtime | NOT VERIFIED | 本次未按计划刷新常驻服务；线上仍是本包实现之前的版本（上条事故记录的意外重启用的是重启前的旧二进制，未包含本包改动） |
| 真实浏览器走查 | NOT VERIFIED | `chrome-devtools` 工具连接故障导致未能实际点击五个标签页；只有静态断言与隔离数据库证明 |
| 待归并/运行记录的分页 | NOT VERIFIED | 当前固定 `limit=100`（评论目标/待归并）与 `limit=50`（运行记录），超过该量级的真实数据尚未验证 |
| 模型 adapter、语义接纳质量 | NOT VERIFIED | 本包不改其合同，也不以页面能否渲染推导语义正确性 |
| Mog 业务验收 | 未完成 | 需在获授权合并和 runtime 刷新后由 Mog 在 3000 页面确认能看到原始评论、运行情况与运行结果 |

本记录不把静态测试、隔离数据库证明或编译通过写成共享运行、真实浏览器可用性或用户验收。
