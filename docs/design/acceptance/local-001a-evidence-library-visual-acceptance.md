# ACC-EVIDENCE-001 · LOCAL-001A Evidence Library 验收记录

> 状态: 一次性报告
> 最后核对: 2026-08-25
> 适用范围: Issue #25 的 local host，以及 Issue #29 修订后的 `/corpus/evidence` V7 精确视觉/骨架
> 事实来源: PAGE-EVIDENCE-001、LOCAL-001、LIDS、实际测试与浏览器检查
> 冲突时以谁为准: 真实运行/代码/合同、用户最新确认和当前 Issue；截图不能覆盖这些来源

## 1. 验收对象

- Issue / Scope: Issue #25（host）与 Issue #29（V7 fidelity）；`LOCAL-001 / 001A`
- 页面/组件/状态: local host、`/health`、`/corpus/evidence`、`SOURCE_INCOMPLETE` empty state
- 关联 PAGE / PAT / CMP / DS / ACC ID: `PAGE-EVIDENCE-001`、`LIDS-PAT-001`、`LIDS-PRI-001`、`LIDS-TOK-001`、`ACC-EVIDENCE-001`
- 验收日期与环境: 2026-08-25；macOS local Rust process; no database or external dependency
- 适用数据/权限前提: no Materials read contract, no accepted material display, no platform permission

## 2. 场景矩阵

| 场景 | 用户任务 | 预期状态含义 | 视觉检查重点 | 真实后果/回执 | 结果 |
|---|---|---|---|---|---|
| Local host response | Open the local Linggan entry | host is loopback, not a data proof | root enters the Evidence Library without a second page | `GET /` returned `307` with `location: /corpus/evidence`; `GET /corpus/evidence` returned `200` | VERIFIED |
| 信息缺失或未知 | Understand why no material is shown | source/read model incomplete; not zero | separated `SOURCE_INCOMPLETE` / `NOT CONNECTED` / `UNKNOWN` readouts | no action | VERIFIED |
| 处理中或部分结果 | N/A in 001A | no contract exists, so must not be invented | no fake loading/partial badge | N/A | VERIFIED (absence checked) |
| 权限受限或失败 | N/A in 001A | no source/permission contract is queried | no error disguised as source state | N/A | VERIFIED (not implemented by design) |

## 3. 视觉工作条件

- 声明的桌面工作区/视口: 1280×800、1440×900、1920×1080；窄屏 390×844
- 输入内容长度与数据密度: 无材料、无真实原文、无模拟计数
- 已批准的设计规则: `PAGE-EVIDENCE-001`、`LIDS-TOK-001`、`LIDS-PRI-001`、`LIDS-PAT-001`、`LOCAL-001-UI-EX-01 / Issue #29 修订`
- LIDS 强度 / Pattern / Token 依据: L1 Corpus Explorer + embedded L2 Inspector；`lids_tokens.css` 仍是唯一可编辑的 107-token 全局运行时来源，`tokens.md` 是版本化校验镜像。Issue #29 未改它；页面局部 `--v7-*` 值只服务本页的精确 V7 复刻。
- LIDS 状态五轴或合成边界依据: source/read model/coverage separated; no generic status tag
- Reduced Motion / 移动或静态 Poster 降级: no motion; mobile collapses three-column workspace into sequential regions
- 已检查的响应式/可访问性条件: 2026-08-25 使用独立 Chrome 对 local route 真实截图并测量：1280×800 得到 128px 页头、216px rail、420px inspector；1440×900 得到 128px 页头、216px rail、420px inspector；1920×1080 得到 128px 页头、216px rail、440px inspector；390×844 得到 128px 页头、顺序折叠的 375px 可用内容宽度、rail → workspace → inspector。四个视口均无横向内容溢出（文档宽度为可用内容宽度，剩余 15px 为浏览器滚动条）。每一处均检出 `SOURCE_INCOMPLETE` 和 `NO_ACCEPTED_MATERIAL_AVAILABLE`，未检出 V7 模拟运行态、计数、示例内容或动作结果；21 个未接通控件带 `aria-disabled`。页面保留 `nav`/`aria-current`、`main`/`aside`/heading 语义。
- 截图或录屏证据位置及生成物登记状态: 截图只存于本机临时目录并已人工检查；不提交 Git，因此无需登记为生成物。截图仅证明给定视口的呈现。

## 4. 分层结论

| 完成层 | VERIFIED / NOT VERIFIED / N/A | 证据 | 仍有限制 |
|---|---|---|---|
| 设计规格一致 | VERIFIED | PAGE-EVIDENCE-001、V7 精确页头/三栏/色值/断点检查、four-viewport visual walk-through | V7 仅为本页受限视觉/骨架 Gold Master，不是全局系统 |
| 前端/组件实现 | VERIFIED | Rust route + CSS served by local host | no client framework/read model |
| 自动检查 | VERIFIED | `cargo fmt --all -- --check`; `cargo clippy --workspace -- -D warnings`; `cargo test --workspace` including root redirect and exact 107-token name-to-value checks; Rust-boundary and governance checks | tests do not prove material/data chain |
| 真实链路/回执 | NOT VERIFIED | none | no DB/plugin/platform/material chain |
| 部署 | NOT VERIFIED | none | local loopback only |
| Mog / 业务验收 | NOT VERIFIED | none | pending user review |

## 5. 发现与后续

- 发现的规格冲突: none in 001A; real material states remain intentionally deferred to 001B.
- 是否需要 `DECISION_REQUIRED`: no.
- 是否需要设计例外或长期决定: 仅 page-local `LOCAL-001-UI-EX-01 / Issue #29 修订`，已在 PAGE/manifest/migration log 记录；不扩展到全局 LIDS 或其他页面。
- 不得因此推断的结论: no accepted Evidence, Observation, Capture, Coverage, plugin success, real platform access, database read, deployment, trend or business outcome.
