# TOPIC-WORKSPACE-REAL-001 · UI Change Manifest

> 状态: 权威当前
> 最后核对: 2026-08-31
> 适用范围: Issue #112 `/topics/{canonical_key}` / PAGE-TOPIC-WORKSPACE-001 的 UI 变更
> 事实来源: 用户授权、Topic 合同、Work Resource Read、LIDS、当前 Rust/CSS/JS/tests
> 冲突时以谁为准: 用户最新确认、AGENTS.md、PAGE、Topic 合同、LIDS 与真实运行证据
> 页面: `/topics/{canonical_key}` / PAGE-TOPIC-WORKSPACE-001

## 来源回执

| 来源 | 状态 | 本次用途 |
|---|---|---|
| AGENTS/current-state/Issue #112 Claim | 权威当前 | 范围、工作区、停工与交付门 |
| DISC-001 Topic 语言与不变量 | 权威当前 | Identity/Version/Run/Adjudication/Release 分责 |
| PAGE-TOPIC-001 | 静态 reference | 只吸收 L2 阅读节奏；不复用合成事实或假动作 |
| LIDS system/tokens/primitives/patterns/language | 权威当前 | Token → Primitive → Component → Pattern → Page |
| Work Resource Read | 当前代码合同 | 作品显示字段的唯一共享入口 |

## 变更分类与表面

- 混合变更：新页面 + 新交互 + 新状态语义；最高风险为把暂定研究冒充正式 Topic/Trend。
- 新一级入口：共享 Header 的“主题图谱”从 disabled 变为 `/topics/task-initiation-difficulty`，状态为“暂定研究”。
- 新 L2 页面：定义/版本、人工裁定、冻结材料、当前 Work Resource、来源边界。
- 新 loopback 读取和导入：页面只有 GET；POST 不在 UI 暴露。

## LIDS 影响

| 层 | 影响 |
|---|---|
| Token | 无新增；CSS 只消费现有 `--lgi-*` 与共享 `--v7-*` alias |
| Primitive | 复用共享 Focus、按钮、技术键、硬阴影、状态 soft fill |
| Component | page-local Topic role lens、material row、boundary strip；不晋升 CMP |
| Pattern | 新的 PAGE-TOPIC-WORKSPACE-001 L2 组合；不改 LIDS 全局 pattern |
| Page | 新 runtime route；旧 static PAGE-TOPIC-001 保持独立 |
| Data Truth | 增加 `PROVISIONAL + HUMAN_ADJUDICATED + FROZEN_PACK` 的真实表达 |

## 诚实状态和敏感边界

- 未读取、not found、schema unavailable、Work Resource unavailable 分开处理。
- 页面不显示 raw body/comment；前端只用 `textContent` 渲染来源字段。
- 角色和 rationale 是人工裁定，不是模型置信度；材料数只指 pack 内引用数。
- 没有正式发布、趋势、市场事实、Agent 命令、采集或成功回执。

## 响应式与动效

- ≥1080px 三列研究面；≤1080px Inspector 下移；≤720px 单栏。
- 选择、筛选与链接反馈使用 100–160ms LIDS motion；Reduced Motion 关闭全部过渡。
- 无 WebGL、图表、外部字体/脚本或远程资产。

## 停止与交接

- 需要正式 Topic Release、自动分类、Claim、趋势、原文或采集时停止。
- 共享 Shell 的一级导航变更需要 integrator 注意与其它页面分支的行级冲突。
- 验收见 `ACC-TOPIC-WORKSPACE-REAL-001`；合并、迁移、部署和业务验收不由本 manifest 宣布。
