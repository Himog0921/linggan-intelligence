# ACC-LOCAL-001D-001 · 未知发布时间默认读取视角验收记录

> 状态: 一次性报告
> 最后核对: 2026-08-26
> 适用范围: Issue #62 的 Evidence Library 默认读取与显式发布时间窗口状态
> 事实来源: LOCAL-001D、PAGE-EVIDENCE-001、LOCAL-001C0、当前测试与代码
> 冲突时以谁为准: 真实运行/代码/合同、用户最新确认和当前 Issue；截图不能覆盖这些来源

## 1. 验收对象

- Issue / Scope: Issue #62；`LOCAL-001D`
- 页面/组件/状态: `/corpus/evidence` 的默认 `latest_accepted_discovery` view、`PUBLISHED_AT UNKNOWN` 以及显式 7/30 天 published window。
- 关联 PAGE / PAT / CMP / DS / ACC ID: `PAGE-EVIDENCE-001`、`LIDS-PAT-001`、`LIDS-PRI-001`、`ACC-LOCAL-001D-001`
- 验收日期与环境: 2026-08-26；隔离 Rust test / 临时 PostgreSQL proof；无真实平台材料写入。
- 适用数据/权限前提: 仅 synthetic accepted discovery records；不读取或记录真实原文。

## 2. 场景矩阵

| 场景 | 用户任务 | 预期状态含义 | 视觉检查重点 | 真实后果/回执 | 结果 |
|---|---|---|---|---|---|
| 默认读取 | 看刚刚已接纳的 discovery 材料 | 未指定 URL `window` 时为最新已接纳视角，已知与未知发布时间可并存 | `视角 最新已接纳` 与明确来源限制 | 只读 synthetic projection | 自动验证 |
| 发布时间未知 | 分辨未知而不是“刚发布” | 卡片显示 `PUBLISHED_AT UNKNOWN`，不显示首次发现/观察/接收时间替代 | Unknown token、无伪造日期 | 无写操作 | 自动验证 |
| 显式 30D | 只看来源发布时间可验证的最近发布材料 | unknown 必须不在结果内，并报告排除数 | 窗口 readout 与排除说明 | 只读 synthetic projection | 自动验证 |
| 无数据库/采集副作用 | 不将展示修复误读成采集功能 | 不新增 action；既有空态保持 | 无新增按钮或成功状态 | N/A | 代码范围检查 |

## 3. 视觉工作条件

- 声明的桌面工作区/视口: 未新增跨视口视觉审查；沿用 PAGE-EVIDENCE-001 既有 V7 fixed-shell 验收。
- 输入内容长度与数据密度: 合成 title/creator/time-state，未使用真实平台文本。
- 已批准的设计规则: `PAGE-EVIDENCE-001`、`LOCAL-001D`、`LIDS-TOK-001`、`LIDS-PRI-001`、`LIDS-PAT-001`。
- LIDS 强度 / Pattern / Token 依据: L1 Corpus Explorer + embedded L2 Inspector；未知标签只使用已有页面 token，不新增全局 token。
- LIDS 状态五轴或合成边界依据: source time known/unknown 与 default/explicit query view 分开表达；synthetic test 不伪装真实链路。
- Reduced Motion / 移动或静态 Poster 降级: 未引入动效或媒体。
- 已检查的响应式/可访问性条件: Rust page-render tests 检查默认/显式文案与无替代时间；完整视觉/浏览器走查留给 PR 合并后的本机 release 验收。
- 截图或录屏证据位置及生成物登记状态: 本事项未提交截图；无生成物。

## 4. 分层结论

| 完成层 | VERIFIED / NOT VERIFIED / N/A | 证据 | 仍有限制 |
|---|---|---|---|
| 设计规格一致 | VERIFIED | PAGE / contract / manifest 同步 | 不含新页面视觉重做 |
| 前端/组件实现 | VERIFIED | route + read projection + page render unit tests | 未在用户本机浏览器验收 |
| 自动检查 | VERIFIED | `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo test --workspace --all-targets --locked`、`./scripts/test-local-001-discovery-postgres.sh`、UI handbook 与 project governance 检查均通过 | final CI/independent review pending |
| 真实链路/回执 | NOT VERIFIED | none in this Issue | 不证明真实 #52 数据、plugin、平台或接纳 |
| 部署 | NOT VERIFIED | none | loopback release/integration pending |
| Mog / 业务验收 | NOT VERIFIED | none | pending after integration |

## 5. 发现与后续

- 发现的规格冲突: 旧文档曾把 URL 缺省读法描述成隐式 `PUBLISHED:30D`；LOCAL-001D 以用户最新确认替代该语义，显式窗口规则未被放宽。
- 是否需要 `DECISION_REQUIRED`: 否。
- 是否需要设计例外或长期决定: 否；这不是全局时间控件或新交互。
- 不得因此推断的结论: 已接纳 discovery 卡片不等于最新发布、平台完整搜索、详情、媒体、评论、趋势、市场结论或真实业务完成。
