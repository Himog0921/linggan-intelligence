# OCR-MANUAL-HUMAN-ADOPTION-454 · Evidence Library UI 变更清单

> 状态: 草案
> 最后核对: 2026-10-23
> 适用范围: Evidence Library 当前作品的已保存手动 OCR 结果
> 事实来源: PAGE-EVIDENCE-001、OCR-CONTENT-LAYERING-001、手动云端 OCR 人工采用合同、LIDS v7.0
> 冲突时以谁为准: Mog 最新确认、AGENTS.md、受控 API 回执与数据合同

## 1. 事项

- Issue / SCOPE：#454，叠加 PR #477 的 Draft 候选。
- Agent 与 worktree：root，`codex/ocr-manual-human-adoption-454`。
- 目标：让人对照已保存原图后，显式采用或保留云端 OCR 的完整单图文字，并得到不可变回执。
- 用户可见结果：机器预览保持“未人工核验”；只有回执确认后才显示人工采用版本及其审核信息。
- 明确非目标：不提供抽检、再识别、模型调用、批量采用、编辑平台标题或 raw JSON 的入口。

## 2. 读取回执

| 来源 | 状态 | 本次解决的问题 | 已核对 |
|---|---|---|---|
| AGENTS.md / current-state | 已读 | 受保护交付、实际状态与不越权 | 是 |
| UI execution contract | 已读 | 表面/状态/依赖/验收先行 | 是 |
| PAGE-EVIDENCE-001 / ReadingPanel | 已读 | 入口只属于当前作品图组 | 是 |
| LIDS data boundaries / primitives / interaction | 已读 | 来源、真实回执、单一主动作与未知状态 | 是 |
| OCR 数据合同 | 已读 | 原始观察、预览、人工采用与材料资格分离 | 是 |
| 当前 OCR recognition / audit UI | 已读 | 不把非抽检采用接到 sample/issue UI | 是 |

## 3. 变更分类

- 分类：混合（交互、状态语义、权限行动）。
- 最高风险类别：权限行动；写入只由后端资格、版本和 work scope 决定。
- 对应来源：`OCR-CONTENT-LAYERING-001`、`note-material-input.v1`、`LIDS-INTERACTION-001`。
- 为什么足以覆盖本次风险：界面只收集人工确认，后端决定可否形成新版本；回执后重新读取才改变呈现。
- DECISION_REQUIRED：否；无法永久区分 sample 与手动采用时立即停止。
- L1 / Pattern：现有 Evidence Library L1 当前作品 Inspector；复用图组 Section、ActionButton、表单和本地原图查看器。
- Token / Primitive / Data Truth：不新增 Token、Primitive、组件或导航；使用既有 M2 动作、字段错误和真实回执表达。

## 4. 影响边界

- 受影响的页面：`/corpus/evidence` 当前作品 ReadingPanel 的 OCR 结果。
- 受影响的组件：OcrRecognitionResults 及专属人工采用表单/API decoder。
- 受影响的状态：未核验、可写、提交中、回执未知、冲突/受限、回执确认。
- 数据/权限/敏感展示：只发送内部 UUID、版本、文字、actor、reason；不发送图片/URL/原始 provider JSON。
- 禁止修改：OCR audit 的 sample/issue 表单语义、provider/egress、raw OCR、平台标题、通用导航和运行时 Token。
- 停止条件：来源失效不能可靠重核、或页面无法在回执后重新读取而只能乐观渲染。

## 5. 验收与证明边界

| 层级 | 验收方法 | 实际结果 | 未证明边界 |
|---|---|---|---|
| 任务可用 | 合成 React/API/PG 单图采用与刷新读回 | VERIFIED（owned PostgreSQL + browser） | 真实人工判断 |
| 状态诚实 | 测试未采用、未知、冲突、受限和确认回执 | VERIFIED（组件 + owned data chain） | 生产数据 |
| 视觉一致 | 既有 Section/Form/ActionButton 的组件测试及浏览器走查 | VERIFIED（合成浏览器）；Mog 验收仍 NOT VERIFIED | Mog 视觉验收 |
| 真实后果 | owned PostgreSQL 检查 immutable command/review 和投影 | VERIFIED（owned PostgreSQL）；共享库、部署、G4 仍 NOT VERIFIED | 共享库、部署、G4 |

## 6. 交接

- 修改文件：见本包最终 diff 与验收记录。
- 验证：定向 Rust、owned PostgreSQL、Web 单测、治理/LIDS/格式检查。
- 规则或索引同步：docs index、进度和本清单/验收记录。
- 例外与替代：无；旧 OCR audit UI 仅用于已有 sample。
- PR / reviewer / integration owner：Draft、独立收尾审查；合并/部署必须另获 Mog 明确授权。
