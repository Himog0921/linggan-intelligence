# PLUGIN-MIGRATION-001 Popup UI Change Manifest

> 状态: 权威当前
> 最后核对: 2026-08-25
> 适用范围: Issue #37 的 Linggan Browser Producer popup；显示本机接纳准备度、一次用户手势 Discovery 与诚实 receipt
> 事实来源: Issue #37、`PLUGIN-MIGRATION-001`、`PAGE-PLUGIN-001`、`LOCAL-001C0-DISCOVERY-BOUNDARY-V1`、LIDS 与实际 MV3 source
> 冲突时以谁为准: 用户最新确认、`AGENTS.md`、实际合同/插件代码；本清单不扩大动作权限

## 事项与读取回执

- 目标：在不引入内容工作台运行依赖的前提下，让 popup 承接唯一获准的 XHS `ADHD` / “综合” / 当前可见前 20 卡 Discovery。
- 非目标：详情、评论、作者、媒体、OCR/ASR、自动化、任务轮询、账号/Cookie、旧工作台、Evidence Library 展示和任何新页面。
- 已读取：`AGENTS.md`、`docs/current-state.md`、UI execution contract、design README/governance、LIDS README、`PAGE-PLUGIN-001`、Discovery contract、Issue #37 及迁移计划。
- 变更分类：混合（展示、状态语义、权限/行动）；最高风险为权限/行动。

## 行动与状态合同

| 用户可见项 | 被允许的含义 | 禁止暗示 |
|---|---|---|
| Local ingress | `/health` 的 `LOCAL_INGRESS_READY`、host only 或不可达 | 数据库持久化、Evidence/页面已接通 |
| Discovery button | 用户当前激活页面的一次受限读取 | 持续监控、后台或任何深采 |
| Not ready | 页面、关键词、排序、可见 DOM 或本机条件不满足，未提交 | 平台无内容、系统已经失败或数据为零 |
| Receipt | 单次 ingress 的接纳/重放/未接纳回执 | Evidence 已入库、媒体已保存或研究结论成立 |

## 设计组合与文件边界

- L1 / Settings / Governance；消费构建时复制的唯一 `--lgi-*` token，使用 readout、文本状态和单一操作按钮；不创建 CMP、Scene 或 Motion。
- 受影响：popup HTML/CSS/JS、service worker 与 adapter 的状态文案；不会修改 Web、Rust、API、数据库、LIDS token source 或内容工作台。
- 停止条件：需要 persistent XHS permission、页面滚动、隐藏状态/网络读取、扩大 Discovery 合同、读取账号/Cookie 或显示真实材料时停止并升级。

## 验收与未证明边界

| 层级 | 本卡检查 | 不证明 |
|---|---|---|
| 任务/状态 | unit + mock ingress：符合条件才 submit，receipt 与 `NOT_READY` 分开 | 浏览器/XHS 当前页面仍匹配 selector |
| 视觉 | LIDS token-only CSS/static source audit | 浏览器视口、辅助功能实测 |
| 真实后果 | 固定本机 URL、最小 manifest/static legacy audit | 真实 host/database、真实 XHS、Evidence Library 读模型 |

旧 Issue #33 manifest 已被替代，但保留历史记录；本清单不自行宣布插件已被浏览器加载或真实采集成功。
