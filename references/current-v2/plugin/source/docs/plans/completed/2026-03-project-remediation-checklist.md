# 2026-03 项目修订清单

> 对应文档：`2026-03-project-retrospective-audit.md`  
> 目标：把审查结论转成可执行、可验收、可排期的动作

---

## P0：必须先做，才能避免后续继续失真

### R1. 收口文档权威关系

问题：
- 外层 `02_*.md`、`03_*.md` 仍像现行设计文档
- 内层与外层的权威边界没有写清

动作：
- 在 `AGENTS.md` 明确声明：
  - 外层三份文档是历史资料
  - 内层 `docs/**` 是当前事实层
- 在 `AGENTS.md` 增加“Source of Truth”小节

完成标准：
- 新智能体只读 `AGENTS.md` 就能知道哪些文档可直接指导实现
- 外层文档不再被误当当前产品/架构依据

### R2. 更新产品现实文档

问题：
- `docs/product/PRD.md`
- `docs/product/APP_FLOW.md`
- `IMPLEMENTATION_PLAN.md`
都明显低估了抖音现状

动作：
- 把抖音能力至少更新到：
  - 单条视频采集/下载
  - 分享触发采集
  - 批量视频采集
  - 批量评论采集
  - 评论图片区能力状态
  - 数据面板二次下载状态

完成标准：
- 产品文档不再出现“抖音批量与评论仍在开发中”之类旧表述
- 计划文档与当前代码现实一致

### R3. 更新数据层权威说明

问题：
- `BACKEND_STRUCTURE.md` 仍停在 schema v4
- 现实已经是 Dexie v6

动作：
- 更新或降级 `BACKEND_STRUCTURE.md`
- 明确它是摘要文档还是事实源
- 若保留，至少同步：
  - schema v6
  - `collectionRuns`
  - `mediaAssets`
  - AI-ready 基线状态

完成标准：
- 不再存在同一项目里同时出现 v4 与 v6 的数据层描述冲突

### R4. 修正消息协议文档

问题：
- `docs/technical/MESSAGE_PROTOCOL.md` 已落后于真实 payload

动作：
- 补齐 `START_BATCH_COMMENTS.commentLimit`
- 标注平台差异化 payload
- 说明 Dashboard/Content/Background 当前真实消息边界

完成标准：
- 协议文档可直接指导后续功能接入，不需要“边看代码边猜”

---

## P1：应尽快做，否则技术债会继续放大

### R5. 拆分 `src/content/index.js`

问题：
- 当前 1545 行，承担过多职责

动作：
- 拆成至少四块：
  - bootstrap
  - message router
  - task orchestration
  - dashboard bridge

完成标准：
- 主入口回落到“初始化与装配”职责
- 平台分发、下载、面板桥接不再混在一个文件里

### R6. 模块化抖音视频上下文解析

问题：
- `src/platforms/douyin/videoCollector.js` 已成为第二个中心化风险点

动作：
- 抽出独立模块：
  - current video context resolver
  - API cache / alias registry
  - detail refresh / media resolution

完成标准：
- 抖音单条采集、下载、批量能力共享同一上下文解析层
- `videoCollector.js` 不再同时承载全部逻辑

### R7. 让 `collectionRuns` 真正接入业务链路

问题：
- 已建表未接线

动作：
- 批量视频、批量评论、评论图片区下载全部写入 `collectionRuns`
- 评论记录写入 `collectionRunId`

完成标准：
- 任一批量任务都能在本地数据中追溯到任务级上下文

### R8. 决策日志补齐抖音关键决策

问题：
- 近几轮抖音最重要的架构转向没有被正式记录

动作：
- 在 `docs/decisions/index.md` 新增决策条目，至少覆盖：
  - 从 DOM 猜测转向真实操作流建模
  - 分享按钮作为当前视频确认动作
  - 作品列表 API 驱动批量采集
  - 评论接口走页面桥接
  - 背景 webRequest 拦截链路的替代/移除结论

完成标准：
- 后续回看抖音设计时，不需要只靠 `progress.txt` 或聊天记录

---

## P2：体验与分析能力升级项

### R9. Popup 与页内注入交互去原生阻塞弹窗

问题：
- 目前仍大量依赖 `prompt / confirm / alert`

动作：
- 用统一设置弹层替代
- 统一成功/失败/进行中反馈组件

完成标准：
- Popup、Dashboard、页内注入都不再依赖浏览器原生阻塞弹窗

### R10. 补齐 UI 可访问性基线

问题：
- Notice、进度状态、弹层、按钮焦点都不够完整

动作：
- 为 popup notice / progress / toast 增加 `aria-live`
- 为弹层增加 `role="dialog"`、`aria-modal`
- 为按钮体系补 `:focus-visible`

完成标准：
- 通过一次基于 Web Interface Guidelines 的回归审查

### R11. 完善 AI-ready 原始证据层

问题：
- 当前缺少稳定的原始证据落库能力

动作：
- 为内容、评论采集补：
  - `collectorVersion`
  - `rawPayload`
  - `rawSource`
  - 必要时的 `rawDomText`

完成标准：
- 后续大模型分析和回算不再只依赖清洗后的结果

---

## P3：历史与维护清理项

### R12. 外层历史文档退位

动作：
- 在外层 `02_*.md`、`03_*.md` 文首增加“历史资料，不作为当前实现依据”的说明

完成标准：
- 新人或新智能体不会再把外层 PRD/架构设计误当现行文档

### R13. `progress.txt` 恢复为真实时间线

动作：
- 把 2026-03-25 到 2026-03-27 的关键抖音进展补回去
- 以后每轮结束必须同步记录：
  - 改了什么
  - 哪个文档同步了
  - 是否构建/实测通过

完成标准：
- `progress.txt` 至少不再与当前代码现实断层

---

## 建议执行顺序

1. R1 `AGENTS.md` 权威关系收口
2. R2/R3/R4 修正文档事实层
3. R8 补决策日志
4. R13 补进度时间线
5. R5/R6/R7 修结构与任务底座
6. R9/R10 修 UI/可用性
7. R11 补 AI-ready 证据层
8. R12 清理历史文档入口认知

---

## 本轮修订清单的使用方式

这份清单不是新的“愿望列表”，而是下一轮执行的唯一入口：

- 做治理修正时，以 P0/P1 为准
- 做结构重构时，以 R5/R6/R7 为准
- 做体验优化时，以 R9/R10 为准
- 做 AI-ready 能力时，以 R11 为准

后续每完成一项，应回写：

- `progress.txt`
- 对应权威文档
- `docs/decisions/index.md`（若涉及决策变更）
