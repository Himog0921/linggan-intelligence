# CORPUS-CROSS-DOMAIN-RENDER-001 · 外部领域样本显示与领域切换器

> 状态: 权威当前
> 最后核对: 2026-09-08
> 适用范围: `GET /corpus/evidence?domain=<external-domain-ref>` 的既有外部样本读取、以及该页 Context Bar 的当前观察领域切换器
> 事实来源: Mog 2026-09-08 的运行页反馈、Issue #130、`PAGE-EVIDENCE-001`、LIDS v7、当前 Rust/JS/CSS 与只读运行核验
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实运行/代码/合同、ACCEPTED 决定；本清单不扩大领域配置、采集、数据写入或部署授权

## 1. 用户结果与分类

- 用户结果：选择已有外部领域“考研自习”时，已由跨行业读取接口返回的 21 条样本必须显示为**跨行业参照样本**；页面不得把它们称为本领域 Evidence，也不得因没有 Work Resource `detailUrl` 而丢弃整张列表。
- 用户结果：领域切换器的展开态必须由 Linggan 渲染，采用 LIDS 方形触发器、白色菜单面、4px 实色硬投影、中文主标签与可聚焦链接；不再把系统原生圆角/蓝色菜单暴露给用户。
- 变更分类：状态/语义 + 展示 + 交互。外部样本的“列表级已读取、详情未读取”是数据边界，不是视觉装饰。
- 明确非目标：领域自助配置、领域创建/改名/暂停、本领域切换、采集/派发/插件、数据库与 migration、跨行业/证据接口签名、详情/正文/评论/媒体读取、随手记、散点图、部署和真实平台动作。

## 2. 读取回执与事实

| 来源 | 本次确认的事实 | 不可外推为 |
|---|---|---|
| `GET /api/local/cross-industry/samples?domain=...0002` | 返回 HTTP 200、21 个 `cross_industry_sample` 列表项 | 已取得详情、原文、评论、媒体或本领域 Evidence |
| `GET /corpus/evidence?domain=...0002` | 服务端正确下发 `data-corpus-domain-own="false"` 与“考研自习 · 21” | 前端已能够渲染外部样本 |
| `evidence_library.js` | 旧版读取了外部接口，但只以 `identity.publicRef` 渲染 Work Resource 行；外部样本只有 `sampleRef` | 计数为空或接口不可用 |
| `shell.css` + `corpus_domain_picker` | 旧版原生 `<select>` 的展开态由浏览器绘制；候选已替换为 `details + summary + link list` | 已部署或已具备领域配置能力 |

## 3. 表面、状态、依赖

| 表面 | 本次行为 | 常用 | 空/失败/受限 | 不做 |
|---|---|---|---|---|
| `EV-S01` Context Bar | 当前领域菜单 | 当前项与其它已启用领域可导航 | 领域表读不到时不渲染菜单；一项时不渲染 | 不提供配置动作 |
| `EV-S04` 结果上下文 | 以外部样本数量作为本次读取数 | 21 条可扫描 | 0 条仍是“尚未采过”；读取失败不说库为空 | 不把计数写为 Coverage |
| `EV-S05` 列表 | 复用行密度，但渲染外部样本的列表级字段 | 标题、作者、互动、来源目标/关键词/排序、最近观察 | 缺封面/作者/互动明确为未知；不补远程媒体 | 不显示材料 lane 或本领域 Evidence 摘要 |
| `EV-S06` Inspector | 显示当前外部样本的列表级来源边界 | 说明是参照样本，哪些字段已读到 | 没有 detail 不是失败；详情/正文/评论/媒体为未读取 | 不构造 Work Resource URL、复观测、补采或来源链接 |

状态词典：`已读取的跨行业样本` 只说明对应样本接口在当前页面读取成功；`详情未读取` 不等于失败或不存在；`样本为参照物` 不等于可支持 ADHD 领域判断；`0` 只在接口确认没有样本时显示；接口不可读时维持 `SOURCE_INCOMPLETE`/读取失败而不显示 0。

依赖：外部样本接口归 `cross_industry_read`，本领域 Material Projection 归 `work-resources`。两条路径保持分离；页面只适配各自返回的读取模型。全局壳层样式归 `shell.css`，页面样本表达归 `evidence_library.js/css`。

## 4. LIDS 选择

- `Token → Primitive → Component → Pattern → Page`：只消费既有 `--lgi-*`/当前 V7 页面映射；`details + summary + link list` 是当前观察领域的 Quiet navigation control，不升格为通用组件。
- 触发器保持方形、中文优先、11px 以上；菜单为白色 Surface、1px 墨线、`--lgi-shadow-brutal` 单层实色投影。键盘使用原生 `details` 与链接顺序；焦点使用全局 token。
- 外部样本行使用连续档案列表的密度，不另造圆角卡、渐变、远程封面回退或伪 KPI。

## 5. 验收矩阵与停止条件

| 层级 | 验收 | 证明/边界 |
|---|---|---|
| 数据路径 | 真实 loopback 外部接口与已有 21 条样本 | 已只读核验；不代表采集或详情完整 |
| 自动 | focused Rust/DOM contract 测试、`cargo fmt --check`、治理/设计检查 | 证明已声明的代码路径，不代替浏览器或部署 |
| 浏览器 | 候选服务下考研自习列表出现 21 行、第一项显示列表级 Inspector、领域菜单无原生 select | 已在 :3107 候选服务用当前本机只读数据核验；不等同部署 |
| 用户验收 | Mog 确认样本阅读与切换器符合预期 | 未验收前不得声称业务完成 |

停止并报告：需要跨行业/证据接口混读；需要伪造 Work Resource detail；需要为缺少的配置权限、领域字段或外部动作作决定；或发现 `shell.css` 有已声明的并行 owner。
