# CLAUDE.md

本文件用于任何 AI 会话的起始约束。进入会话先读 `AGENTS.md`（项目地图），再按需读具体文档。

## 1. 项目定位

- 项目：灵感爆爆爆（Chrome MV3 小红书采集插件）
- 核心目标：稳定采集、可控流程、清晰数据管理、可持续迭代
- 知识库入口：`AGENTS.md`

## 2. 铁律（必须遵守）

1. 黑盒目标先调研：禁止猜选择器和字段名。
2. 不转嫁技术债：探查、调试是 AI 责任。
3. 方案阶段识别外部依赖风险：先写验证计划。
4. 实现前假设检查：每项都回答"已验证还是猜测"。
5. 对用户沟通用非技术语言。

## 2.0 本地工作台联调规则（2026-06-25 起）

Mog 当前一周内主要在本机打磨内容工作台和插件。默认联调目标是本地工作台，除非用户明确说“线上 / 生产 / 正式站”：

1. 插件要写入本地数据时，工作台地址必须设为 `http://localhost:3000`。
2. 插件如果指向 `https://lingganboom.fun`，采集结果、任务回传、媒体上传和工位同步都会进入线上正式站；本地打磨时不要这样做。
3. 本地和线上是两套授权关系；切到本地后，需要在本地工作台重新生成授权码、重新绑定执行设备。
4. 本地联调前先确认内容工作台本地服务可打开，并在插件里执行“测试连接”。
5. 插件已内置“本地 3000”快捷入口，优先使用它，不要手写临时地址。

## 2.1 协作协议（最高优先级）

> 本项目已从“脚本工具”进入“产品化插件”阶段。以下协作规则与铁律同级，必须执行。

1. 先定义验收结果，再开始实现。  
   每轮需求先写清楚：用户可见变化、复现路径、通过标准。
2. 小步快跑，但每步必须闭环。  
   固定流程：实现 → 构建/自测 → 用户实机验证 → 修正 → 更新 `progress.txt`。
3. 黑盒风险前置，不靠猜测推进。  
   选择器、反风控、高清链路等先调研验证，再进入实现分支。
4. 体验稳定性优先于功能堆叠。  
   优先处理“慢、卡、遮挡、状态不一致、失败不可恢复”，再扩功能。
5. 每轮结束必须产出可追溯资产。  
   至少包含：验收结论、关键改动、备份路径（如有）、下一步计划。

### 文档同步硬门禁（与铁律同级）

> 从本轮开始，任何“实现 + 测试 + 用户反馈”都必须触发文档闭环判断；未完成闭环，不算这一轮真正完成。

1. 每次代码改动后，至少必须主动更新 `progress.txt`。  
   不能等用户提醒，也不能把关键信息只留在聊天里。
2. 只要用户可见行为发生变化，必须同步更新：
   - `docs/product/PRD.md`
   - `docs/product/APP_FLOW.md`
   - `docs/product/TEST_CHECKLIST.md`
3. 只要字段、页面事实、选择器、协议、任务语义发生变化，必须同步更新对应技术文档：
   - `docs/technical/DATA_MODEL.md`
   - `docs/technical/MESSAGE_PROTOCOL.md`
   - `docs/technical/SELECTORS.md`
   - `docs/technical/DOUYIN_FIELD_SURVEY.md`
   - `docs/technical/AI_READY_DATA_CONTRACT_V1.md`
4. 只要出现新的稳定结论、架构转向、策略收口或“以后必须这样做”的规则，必须同步更新：
   - `docs/decisions/index.md`
   - 必要时更新 `docs/plans/active/*.md`
5. 每轮结束前必须明确给出“本轮已同步哪些文件”；如果有故意没同步的文件，也必须说明原因。
6. 若代码已改、测试已做、但文档未同步，则该轮状态只能算“部分完成”，不能对外表述为“已完成/已收口”。

### 推荐协作节拍

1. 用户给出本轮最高优先级问题（1~3 项，附截图/复现路径）。  
2. AI 输出“定位结论 + 改动范围 + 验收点”。  
3. AI 直接实现并提供最短测试路径。  
4. 用户只做实机验收，AI 负责技术排查与收口。  
5. 轮次结束后更新 `progress.txt`，必要时执行版本备份。

## 3. 文档导航

> 详见 `AGENTS.md` 的完整导航表。

快速索引：
- 产品需求 → `docs/product/PRD.md`
- 用户流程 → `docs/product/APP_FLOW.md`
- 验收清单 → `docs/product/TEST_CHECKLIST.md`
- 架构 → `docs/ARCHITECTURE.md`
- 选择器 → `docs/SELECTORS.md`
- 设计规范 → `docs/DESIGN.md`
- 技术栈 → `docs/technical/TECH_STACK.md`
- 数据模型 → `docs/technical/DATA_MODEL.md`
- 消息协议 → `docs/technical/MESSAGE_PROTOCOL.md`
- 反检测 → `docs/technical/ANTI_DETECT.md`
- 决策日志 → `docs/decisions/index.md`
- 当前计划 → `docs/plans/active/`
- 文件落位规则 → `docs/governance/file-placement-standard.md`
- 文档维护协议 → `docs/governance/document-maintenance-protocol.md`
- 技术债务 → `docs/plans/tech-debt.md`

## 4. 代码约定

- 批量任务必须可暂停/继续/停止，且在详情页可控。
- 用户看得到的错误必须可读、可执行。
- 所有外部依赖改动要更新 `docs/SELECTORS.md`。
- 每次功能完成必须更新 `docs/plans/active/` 中的对应计划。
- 技术决策要记录到 `docs/decisions/index.md`。
- 每次“实现 + 测试 + 用户反馈”闭环后，都必须按变更类型主动更新对应权威文档，最低不少于 `progress.txt`。
- 新增文件前必须按 `docs/governance/file-placement-standard.md` 判断位置；文档同步范围按 `docs/governance/document-maintenance-protocol.md` 执行。

## 5. 不允许行为

- 不验证就写死选择器。
- 功能未完成却只写"TODO"。
- 没有回归检查就宣称完成。
