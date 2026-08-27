# LOCAL-001D · 未知发布时间默认读取视角 UI 变更清单

> 状态: 权威当前
> 最后核对: 2026-08-26
> 适用范围: Issue #62 对 `/corpus/evidence` 的时间视角和未知发布时间状态表达
> 事实来源: Issue #62、LOCAL-001D、PAGE-EVIDENCE-001、LOCAL-001C0、LIDS 与当前实现
> 冲突时以谁为准: 用户最新确认、AGENTS.md、数据合同与真实代码；本清单不授予新数据或行动

## 1. 事项

- Issue / Scope: Issue #62；`LOCAL-001D`
- Agent 与 worktree: `codex:/root/evidence_library_unknown_publish_fix`；隔离 Issue worktree
- 目标: 让默认 Evidence Library 读取可显示已接纳但来源发布时间未知的 discovery 卡片。
- 用户可见结果: 默认页面显示“视角 最新已接纳”和 `PUBLISHED_AT UNKNOWN`；显式 7/30 天发布时间窗口仍严格排除未知对象并报告数量。
- 明确非目标: 插件、采集、媒体、OCR/ASR、写操作、新过滤控件、真实数据修改、数据库迁移和视觉重做。

## 2. 读取回执

| 来源 | 状态 | 本次解决的问题 | 已核对 |
|---|---|---|---|
| `AGENTS.md` / `current-state.md` | 已读 | 闭集实施、Unknown 不得压成零、真实/合成边界 | 2026-08-26 |
| UI execution contract | 已读 | LIDS、页面规格、状态诚实和验收记录 | 2026-08-26 |
| PAGE-EVIDENCE-001 | 已更新 | 默认读取与显式发布时间窗口的页面含义 | 2026-08-26 |
| LIDS Token / Primitive / Pattern / Agent guide | 已读 | 使用现有状态色与 L1 + embedded L2 页面组合 | 2026-08-26 |
| LOCAL-001C0 data contract | 已更新 | 不以其他时间替代来源发布时间 | 2026-08-26 |
| 当前 Rust 代码/测试 | 已读 | 两个既有 read projection 与 route/render 边界 | 2026-08-26 |

## 3. 变更分类

- 分类: 状态语义 + 展示
- 最高风险类别: Data Truth
- 对应来源 ID: `LOCAL-001D`、`PAGE-EVIDENCE-001`、`LOCAL-001C0-DISCOVERY-BOUNDARY-V1`、`LIDS-PRI-001`、`LIDS-PAT-001`
- 为什么该类别足以覆盖本次风险: 该改变只影响“能否看见已接纳材料”和“未知发布时间如何诚实表达”；不改变采集或事实接纳。
- 是否存在 `DECISION_REQUIRED`: 否；默认/显式窗口语义已由 Mog 确认。
- L1 / L2 / L3 与主 Pattern: L1 `Corpus Explorer`，嵌入右侧 L2 `Split Evidence Inspector`；没有 L3。
- 是否触及 Token、Primitive、CMP、Scene、Motion 或 Data Truth: 只在页面局部使用既有 Unknown token；没有新 Token/CMP/Scene/Motion，Data Truth 语义由合同明确更新。

## 4. 影响边界

- 受影响的页面: 仅 `/corpus/evidence`
- 受影响的组件: 现有 Evidence Library readout 与 card 时间状态
- 受影响的状态: `latest_accepted_discovery`、`PUBLISHED_AT UNKNOWN`、显式窗口的 `UNKNOWN_PUBLISHED_TIME_EXCLUDED`
- 是否影响数据口径、权限、敏感展示或真实行动: 只改变已有只读投影的时间视角；不扩大权限、不展示新字段、不执行行动。
- 禁止修改的文件/能力: 插件、ingress、真实 Canary、migration、媒体、Topic/analysis/OCR/ASR。
- 停止条件: 任何需要产生、修复或推断发布时间的方案均停止。

## 5. 验收与证明边界

| 层级 | 验收方法 | 实际结果 | 未证明边界 |
|---|---|---|---|
| 任务可用 | default/explicit route and projection synthetic tests | 见 ACC-LOCAL-001D-001 | 不证明真实用户使用 |
| 状态诚实 | unknown label/no surrogate tests | 见 ACC-LOCAL-001D-001 | 不证明来源时间会被平台补齐 |
| 视觉一致 | existing V7 shell + LIDS-token page-local render tests | 见 ACC-LOCAL-001D-001 | 未重新进行跨视口人工视觉验收 |
| 真实后果 | 不适用；本事项只读 | N/A | 不证明采集、接纳、平台或业务结果 |

## 6. 交接

- 修改文件: contracts/read projections/local page CSS and Rust tests; LOCAL-001D plan, contract/page/runbook/LIDS/index/progress records.
- 验证命令/走查: `cargo fmt --all -- --check`、focused tests、isolated PostgreSQL proof、clippy、governance checks.
- 规则或索引同步: docs README、PAGE-EVIDENCE-001、LOCAL-001C0、runbook、LIDS migration log、progress.
- 例外与替代: 不创建新页面或视觉例外；仅替代此前“缺省 URL 等于隐式 30D 发布时间窗口”的页面/read-model语义。
- LIDS migration log / 预览同步: 已登记；没有新增截图或生成物。
- PR / reviewer / integration owner: Draft PR；独立 Spec 和 Standards reviewer；`/root` 为后续 integration owner。
