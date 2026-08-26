# DESIGN-008 · 共享壳层中文优先迁移变更清单

> 状态: 权威当前
> 最后核对: 2026-08-26
> 适用范围: Issue #68 的 Linggan local web 共享页头、导航读数、共享导轨标注和页头运行状态表达
> 事实来源: Mog 对“中文为主、英文仅作装饰或注释”的明确确认；LIDS-LANG-001（Draft PR #67）；AGENTS.md；UI 执行合同；`PAGE-EVIDENCE-001`、`PAGE-COLLECTION-001` 与实际 shared shell
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实代码/合同、ACCEPTED 决策与当前 SCOPE；本清单不扩大任何数据、权限或行动授权

## 1. 事项

- Issue / SCOPE: Issue #68；`DESIGN-008`
- Agent 与 worktree: `codex:/root/shared_shell_chinese_migration`；`/Users/moglenny/proma/worktrees/linggan-design-008`；base `176ae82`
- 目标: 让 Corpus 与 Collection 共用的壳层以中文独立表达用户语义；英语仅保留为较小、紧邻中文的技术键或品牌注释。
- 用户可见结果: 一级导航、品牌说明、本机边界、运行状态、时区与共享导轨标注不再以孤立英文为唯一可读含义；`UNKNOWN` 仍表示未知，`UTC+08` 仍表示同一时区。
- 明确非目标: Evidence Library 页面局部模板、动态卡片、查询、API、路由、数据/数据库、采集/插件、媒体、按钮行为、真实材料和任何状态判定。`#67 / PR #67` 保留 Evidence Library 局部中文化责任。

## 2. 读取回执

| 来源 | 状态 | 本次解决的问题 | 已核对 |
|---|---|---|---|
| `AGENTS.md` / `docs/README.md` / `current-state.md` | 已读 | 闭集、Unknown 不默认化、Issue/PR 与文档治理 | 2026-08-26 |
| `docs/agents/ui-execution-contract.md` | 已读 | 共享 UI、状态语义与验收门 | 2026-08-26 |
| `docs/design/README.md` / `design-governance.md` | 已读 | 设计来源优先级、Manifest、收口边界 | 2026-08-26 |
| LIDS README / Primitive / Pattern / Agent guide | 已读 | Sans 中文主语、Mono 技术旁注、未知状态与 L1 页面组合 | 2026-08-26 |
| `PAGE-EVIDENCE-001` / `PAGE-COLLECTION-001` | 已读 | 两页的用户任务、页面边界与共享页头职责 | 2026-08-26 |
| `shell.rs` / `shell.css` / `collection.rs` / focused tests | 已读 | 实际共用文本位置与安全最小改动 | 2026-08-26 |
| Issue #65 / Draft PR #67 | 已读 | LIDS-LANG-001 的局部 Evidence 责任；避免越过 shell 文件边界 | 2026-08-26 |

## 3. 变更分类

- 分类: 展示 + 状态语义（按状态语义门执行）
- 最高风险类别: 状态语义
- 对应来源: LIDS-LANG-001、`LIDS-PRI-001` 排版/状态契约、`PAGE-EVIDENCE-001`、`PAGE-COLLECTION-001`
- 为什么: 本次不改变状态来源、判定、数据或行为；只确保 `UNKNOWN`、连接状态和时区不再被英文代码单独呈现。
- L1 / L2 / L3 与主 Pattern: 既有 L1 `Corpus Explorer` 与 `Collection Control`；本事项不新增 Pattern、Primitive、CMP、Token、Scene 或 Motion。
- DECISION_REQUIRED: 无。局部 Evidence 模板内残余英文不在本卡处理，交由已在审的 #67。

## 4. 影响边界

- 受影响页面: `/corpus/evidence` 与 `/collection/*` 的共用 global header / rail；Collection 运行模式的短标签。
- 受影响组件: `global_header`、primary nav readout、system boundary、context metadata、shared rail readout。
- 受影响状态: 已有 `UNKNOWN`、未接通、来源信息不完整、`UTC+08` 的显示文字；事实含义保持不变。
- 不影响: 数据口径、权限、敏感展示、真实行动、路由、查询、采集、数据库、媒体、插件。
- 禁止修改: `apps/api/src/local_web/evidence_page.rs`、`evidence_library.css`、`local_web.rs` 的 Evidence template 和所有跨边界合同。
- 停止条件: 需要改变 API/数据/查询/路由/按钮行为、状态真值、来源资格或 Evidence 页面局部模板时停止并移交相关 Issue。

## 5. 语言规则在本卡的可执行表达

| 区域 | 中文主语义 | 英文处理 | 不得发生 |
|---|---|---|---|
| 一级导航 | 雷达、主题图谱、语料、洞察、采集；并显示中文状态 | `RADAR`、`TOPIC MAP` 等作为较小技术旁注 | 英文 `UNKNOWN` 或破折号成为唯一状态 |
| 品牌副标题 | 领域情报工作台 | `EDITORIAL INTELLIGENCE` 作为较小旁注 | 品牌副标题仅英文 |
| 本机边界 | 本机服务 + 中文运行状态 | 原运行码作为较小技术键 | 把「未接通」改写成错误或成功 |
| 上下文状态 | 未知、读模型未接通、调度器未接通、来源信息不完整、中国标准时间 | `UNKNOWN`、`READ MODEL NOT CONNECTED`、`UTC+08` 紧邻保留 | 以 0、空值、完成或其他值替换 unknown |
| 导轨读数 | 语料库、采集 | 本卡不强制每一项展示英文技术键 | `CORPUS` / `COLLECTION` 单独作为可见主语义 |

## 6. 与 #67 的集成顺序

`#67 / PR #67` 仅处理 Evidence Library 的页面局部模板与局部 CSS；本卡仅处理共享 shell。两者的 runtime 文件不重叠，但都可能接触 focused test / 文档索引。因此建议 **先合并 PR #67，随后将本卡 rebase 到新的 main 并由 integration owner 处理文档/测试的行级冲突**。任何旧 head 的 reviewer 结论不适用于 rebase 后 head。

## 7. 验收与证明边界

| 层级 | 验收方法 | 实际结果 | 未证明边界 |
|---|---|---|---|
| 任务可用 | `cargo test -p linggan-api` 的 shared shell Corpus/Collection render coverage | IMPLEMENTER VERIFIED：32 passed、0 failed、6 ignored（隔离 PostgreSQL proof，和本事项无关） | 不证明真实采集或业务流 |
| 状态诚实 | 自动断言中文主语义与原技术码并存；代码审查不改状态数据 | IMPLEMENTER VERIFIED：Corpus 与 Collection 共享 shell 输入均有 focused coverage | 不证明英文仍存在的页面局部术语已经全部迁移 |
| 视觉一致 | 安全本地 DOM / 双页面浏览器走查；检查英文旁注相对中文更小 | IMPLEMENTER VERIFIED：`127.0.0.1:3010` 无材料本地运行时，中文主文案 11–13px Sans、英文技术旁注 9px Mono | 不证明真实素材或跨浏览器视觉验收 |
| 真实后果 | 无后端、数据库或外部请求 | N/A | 不改变任何真实运行结果 |

## 8. 交接

- 修改文件: `shell.rs`、`shell.css`、必要的 Collection 短模式标签、focused tests、本清单、验收、LIDS migration log、索引与 2026-08 进度记录。
- 验证命令: `cargo fmt --all -- --check`、`cargo test -p linggan-api`、`cargo clippy -p linggan-api --all-targets -- -D warnings`、`./scripts/verify-ui-design-handbook.sh`、`./scripts/check-project-governance.sh`，以及安全本地 DOM / 浏览器走查。
- PR / reviewer / integration owner: Draft PR；必须由非实现者复审；不自行合并、部署或关闭 Issue。
