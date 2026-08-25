# PLUGIN-001 Popup UI Change Manifest

> 状态: 历史归档
> 最后核对: 2026-08-25
> 适用范围: 历史 Issue #33 的 Linggan Browser Producer popup 视觉、状态与禁用 Discovery 边界；不再约束当前实现
> 事实来源: Issue #33、`PAGE-PLUGIN-001`、Discovery boundary contract、LIDS 与实际 MV3 source
> 冲突时以谁为准: [`plugin-migration-001-producer-popup-ui-change-manifest.md`](plugin-migration-001-producer-popup-ui-change-manifest.md) 与用户最新确认；本清单只保留历史回执

> 替代说明: Issue #37 / `PLUGIN-MIGRATION-001` 获准将受限的用户手势 Discovery 接入本机 ingress。本文件中“按钮持续 disabled”“无真实 ingress”仅是 Issue #33 的历史边界，不能与当前 popup 规格并存。

## 1. 事项

- Issue / SCOPE：Issue #33 / `PLUGIN-001`。
- Agent 与 worktree：`plugin-001-linggan-owned-producer-v1` / `/Users/moglenny/.proma/agent-workspaces/linggan-intelligence/issue-33-linggan-owned-producer`。
- 目标：让独立 Linggan Producer popup 诚实显示包身份、固定 loopback 与不可执行的 Discovery 边界。
- 用户可见结果：用户不会把安装包、loopback health、授权或 Discovery ingress 混为一次“采集已接通”。
- 明确非目标：不加载浏览器、不操作小红书、不显示任何材料、不创建可点击采集动作。

## 2. 读取回执

| 来源 | 状态 | 本次解决的问题 | 已核对 |
|---|---|---|---|
| AGENTS.md / current-state | 权威当前 | 本卡唯一运行时 producer、真实边界与停止条件 | 是 |
| UI execution contract | 权威当前 | popup 是用户可见状态/权限界面，必须闭集实现 | 是 |
| 产品页面文档 | `PAGE-PLUGIN-001` | popup 的单一用户任务与状态含义 | 是 |
| 设计规则/页面/组件规格 | `LIDS-PAT-001`、`LIDS-PRI-001` | L1 Settings/Governance、readout、disabled button | 是 |
| LIDS | Token / Primitive / Pattern / Agent guide | 唯一 Token 值源、无第二视觉系统、disabled 状态 | 是 |
| 数据/权限/行动合同 | `LOCAL-001C0-DISCOVERY-BOUNDARY-V1` | Discovery 不等于 Evidence/媒体，按钮不可越权 | 是 |
| 当前代码/测试 | MV3 source、build/static checks | popup 当前可表达与未表达的边界 | 是 |

## 3. 变更分类

- 分类：混合（展示、状态语义、权限/行动）。
- 最高风险类别：权限/行动。
- 对应来源 ID：Issue #33、`PAGE-PLUGIN-001`、`LOCAL-001C0-DISCOVERY-BOUNDARY-V1`、`LIDS-PRI-001`。
- 为什么足够：唯一按钮持续 disabled，health 只读取 Linggan loopback，未创建任何平台权限或提交路径。
- DECISION_REQUIRED：无；真实 ingress、Discovery execute、浏览器加载和平台 host permission 明确在本卡外。
- L1 / Pattern：`L1 / Settings / Governance`。
- Token/Primitive/CMP/Scene/Motion/Data Truth：使用 build-time copied `--lgi-*` tokens、L1 readout/disabled button/Data Truth；无 CMP、Scene 或 Motion。

## 4. 影响边界

- 受影响页面：Chrome extension popup only。
- 受影响组件：package readout、loopback health readout、disabled Discovery boundary。
- 受影响状态：`CHECKING`、`LOCAL_HOST_REACHABLE`、`LOOPBACK_UNREACHABLE`、`NOT_AUTHORIZED`、`NOT_CONNECTED`。
- 数据/权限/敏感展示/真实行动：只允许 `GET http://localhost:3000/health`；不展示或处理敏感数据，不执行真实行动。
- 禁止修改：Rust/API/DB/Evidence Library runtime、平台/媒体/OCR/ASR、内容工作台依赖、LIDS token source。
- 停止条件：需要平台 host permission、真实 ingress、操作回执、读取真实材料或新增视觉系统时停止。

## 5. 验收与证明边界

| 层级 | 验收方法 | 实际结果 | 未证明边界 |
|---|---|---|---|
| 任务可用 | 静态 DOM/source 检查 | PASS：popup 保留 package、loopback、authorization 与 disabled Discovery 区域 | 浏览器实际打开 |
| 状态诚实 | source/contract assertions | PASS：health、authorization、ingress 三种责任分离；无 Evidence/采集成功文案 | 实际 host 与 ingress receipt |
| 视觉一致 | LIDS token-only CSS audit | PASS：popup CSS 只消费打包的 `--lgi-*` token；无第二主题或远程视觉资源 | 浏览器视口走查 |
| 真实后果 | 禁用按钮与最小 host permission audit | PASS：`permissions=[]`，仅 `localhost:3000`，Discovery 无提交/平台路径 | 真实采集或 Evidence 接纳 |

## 6. 交接

- 修改文件：见 Issue #33 Claim。
- 验证命令/走查：`npm run check`、release manifest/ZIP inspection、governance check；不运行浏览器或平台。
- 规则或索引同步：docs README、LIDS migration log、progress、generated artifacts registry。
- 例外与替代：无。
- PR / reviewer / integration owner：Draft PR #35；独立 reviewer 与 integration owner 尚待分配。
