# linggan-boom 文档地图

> 本文件是 `docs/` 的总索引(渐进式披露第 2 层)。
> **第 1 层入口**:根目录 `AGENTS.md`(协作铁律 + 权威事实源表 + 源码地图)。
> **第 3 层**:以下各文档本身。
> **判断原则**:产品问题看 `product/`、技术事实看 `technical/` + 真实代码、当前进度优先看项目根 `progress.txt`、关键转向看 `decisions/`。

## 状态图例

- ✅ **权威当前**:对齐 v2.0.95 候选代码,可作事实源
- ⚠️ **可能滞后**:仍可参考,但部分内容可能未跟上最新代码(用前核对源码)
- 🗄️ **历史归档**:仅供背景,不再是事实源
- 📝 **草案**:未落地或部分落地

---

## 产品层(`docs/product/`)

| 文档 | 用途 | 状态 |
|---|---|---|
| [PRD.md](product/PRD.md) | 统一产品 PRD(插件执行端视角);**第 2 节功能规格库是 agent 实现 spec 权威** | ✅ |
| [APP_FLOW.md](product/APP_FLOW.md) | 用户操作流程总览 | ✅ |
| [TEST_CHECKLIST.md](product/TEST_CHECKLIST.md) | 验收检查清单(分层) | ✅ |
| [BEGINNER_GUIDE.md](product/BEGINNER_GUIDE.md) | 小白版使用教程 | ⚠️ |

## 文件治理层(`docs/governance/`)

| 文档 | 用途 | 状态 |
|---|---|---|
| [governance/file-placement-standard.md](governance/file-placement-standard.md) | 新增文件放哪里、根目录允许什么、命名红线 | ✅ |
| [governance/document-maintenance-protocol.md](governance/document-maintenance-protocol.md) | 代码/协议/版本/页面事实变化后应该同步哪些文档 | ✅ |

## 架构层

| 文档 | 用途 | 状态 |
|---|---|---|
| [ARCHITECTURE.md](ARCHITECTURE.md) | 系统架构机器可读描述(上下文/通信/数据层) | ✅ |
| [REPLICATION_BLUEPRINT.md](REPLICATION_BLUEPRINT.md) | 1:1 复刻蓝图(自包含 agent 执行手册,事实源=代码) | ✅ |

## 数据与技术层

| 文档 | 用途 | 状态 |
|---|---|---|
| [technical/TECH_STACK.md](technical/TECH_STACK.md) | 技术栈锁定(MV3/React 19/Dexie v13/Webpack 5) | ✅ |
| [technical/DATA_MODEL.md](technical/DATA_MODEL.md) | 数据模型(LingganBoomDB Dexie v13 表结构) | ✅ |
| [technical/MESSAGE_PROTOCOL.md](technical/MESSAGE_PROTOCOL.md) | 消息协议(事实源 `src/shared/constants.js`) | ✅ |
| [technical/AI_READY_DATA_CONTRACT_V1.md](technical/AI_READY_DATA_CONTRACT_V1.md) | AI Ready 数据契约 v1 | ✅ |
| [technical/PLUGIN_AUTHORIZATION_PROTOCOL.md](technical/PLUGIN_AUTHORIZATION_PROTOCOL.md) | 插件授权协议(授权码 + 工位配对两层身份) | ✅ |
| [technical/XHS_FIELD_SURVEY.md](technical/XHS_FIELD_SURVEY.md) | 小红书字段与页面结构调研 | ✅ |
| [technical/DOUYIN_FIELD_SURVEY.md](technical/DOUYIN_FIELD_SURVEY.md) | 抖音字段与页面结构调研 | ⚠️ 2026-03-27 较旧 |
| [technical/ANTI_DETECT.md](technical/ANTI_DETECT.md) | 反检测策略清单 | ⚠️ |
| [SELECTORS.md](SELECTORS.md) | DOM 选择器清单(带验证日期,30 天过期规则) | ✅ |

## 设计层

| 文档 | 用途 | 状态 |
|---|---|---|
| [DESIGN.md](DESIGN.md) | Neobrutalism UI 设计规范 | ⚠️ 可能未跟上 AEDS 换肤 |

## 决策层

| 文档 | 用途 | 状态 |
|---|---|---|
| [decisions/index.md](decisions/index.md) | 技术决策日志(D1~D24,关键架构转向) | ✅ |

## 计划层(`docs/plans/`)

| 文档 | 用途 | 状态 |
|---|---|---|
| [plans/active/](plans/active/) | 活跃计划(⚠️ 多数为 2026-04 接入前计划,接入已完成,见 active/README.md 真实状态标注) | 📝 |
| [plans/completed/](plans/completed/) | 已完成计划(历史参考) | 🗄️ |
| [plans/tech-debt.md](plans/tech-debt.md) | 技术债务清单(持续维护) | ✅ |

## 进度层

| 文档 | 用途 | 状态 |
|---|---|---|
| `progress.txt`(项目根) | 真实时间线,**最新事实源** | ✅ |
| [TODO.md](TODO.md) | 当前待办总表 | ⚠️ |

## 调研层(顶层)

| 文档 | 用途 | 状态 |
|---|---|---|
| [STABILITY_RESEARCH.md](STABILITY_RESEARCH.md) | 平台采集稳定性调研(2026-04 基线,分级 + 风险热力图) | ⚠️ |

## 评审与验收层(`docs/reviews/`)

历史评审报告(PLUGIN_REVIEW_* / PHASE1~4 / MIDTERM / FULL_AUDIT)+ 验收报告(ACCEPTANCE_REPORT_*)。**仅供背景**,不是当前事实源。详见 [reviews/README.md](reviews/README.md)。状态 🗄️。

## 归档层(`docs/archive/`)

已被取代的文档(DATA_EXPLORATION_REPORT 被 XHS_FIELD_SURVEY 取代、FIX-SUMMARY 历史 fix)。详见 [archive/README.md](archive/README.md)。状态 🗄️。

## 上架资料(`docs/chrome-web-store/`)

Chrome 商店上架相关(store-listing / privacy-policy / reviewer-notes / dashboard 填表速查)。当前候选代码对齐版本 **v2.0.95**；实际商店上传状态以 `chrome-web-store/README.md` 的分项说明为准。

---

## 文档同步纪律

任一字段 / 选择器 / 协议 / 任务语义 / 用户可见行为变更,必须同步更新对应层文档(详见根目录 `AGENTS.md` 的"文档同步硬门禁")。最低不少于 `progress.txt`。
