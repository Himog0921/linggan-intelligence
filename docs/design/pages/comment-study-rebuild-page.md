# PAGE-COMMENT-STUDY-REBUILD-001 · 评论研究

> 状态: 权威当前
> 最后核对: 2026-09-22
> 适用范围: `/corpus/comments` 的 COMMENT-STUDY-REBUILD-001 页面与只读投影
> 事实来源: DEC-0006、COMMENT-RESEARCH-REBUILD-001、COMMENT-STUDY-INTELLIGENCE-OVERVIEW-001、LIDS
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实运行/数据合同和当前代码

## 1. 身份与授权

- LIDS 视觉强度 / 主 Pattern: L1 / Corpus Explorer。
- 用户任务: 看清已有评论里用户怎样表达、哪些用户问题值得继续阅读、有哪些办法与经历，以及当前还有什么没有看清。
- 三秒答案: 我们已经从评论中理解了什么，这些理解能否回到真实原声。
- 五秒主动作: 从总览下钻评论目标、待归并信号、长期用户问题或运行记录；需要时显式发起一次受控研究。
- 明确非目标: 不在本页编辑 raw comment、人工创建 Problem、发起采集、自动调用模型、恢复选题库、发布需求排行或伪造质量总分。
- 可用数据合同: `linggan_comment_study_*`、raw Evidence 的 current qualification、generic invocation ledger；不读取旧 V1/V2/V3 research relations。

## 2. 页面边界

- 入口: `/corpus/comments`；退出: Corpus 其它表面。
- 核心任务: 只读呈现评论研究的观察基础、Signal、Problem、原声与未闭合状态。
- 责任分界: 本页只显示后端已写入的 receipt 和研究事实；模型配置属于模型设置，真实执行属于 worker，raw comment 仍属 Evidence。
- 浏览、筛选、展开和切换 Tab 只能读取；只有「发起研究」弹窗中的显式提交可以写入 Policy 或创建 Run。

## 3. 五个工作面

| 工作面 | 默认回答 | 首层内容 | 禁止替代 |
|---|---|---|---|
| 概览 | 我们已经理解了什么 | 观察基础、表达类型、本期值得看的问题、归一表达、办法与经历、代表原声、尚未看清 | 最新 Run 工程报告、需求排行、虚构趋势 |
| 评论目标 | 这次研究实际处理了哪些评论 | 原声、处理状态、语境、Signal 数 | 把来源受限内容继续引用 |
| 待归并 | 哪些 Signal 仍没有安全终态 | Signal、归并资格、等待原因、Pair 结论 | 把所有非问题 Signal 当待建 Problem |
| 用户问题 | 已形成哪些稳定 Problem | 稳定定义、纳入/排除边界、membership 数 | 原始 JSON、市场规模宣称 |
| 运行记录 | 哪次研究做了什么 | 作品、目标、成功、无信号、等待、失败、来源受限 | 抢占概览首屏 |

## 4. 总览组合 `COMMENT-STUDY-INTELLIGENCE-OVERVIEW-001`

总览使用已有只读 endpoint 组合，不新增 schema，不在浏览器推断模型尚未写入的事实。

### 4.1 观察基础读数带

首层固定四项：

1. **观察到的评论**：当前领域 source preview 中的评论库存；未知显示 `—`。
2. **可研究评论**：通过既有来源资格 gate 的评论；不得写成已研究评论。
3. **本次接纳信号**：最近一次 Run 的 Signal 状态合计；不是需求强度或质量分。
4. **长期用户问题**：当前 active/retired Problem 读取总数；达到读取上限时显示 `100+`，不伪装完整总量。

读数共用上下结构线，不做四张独立统计卡。数字使用 Mono，标签和口径使用 Sans。

### 4.2 用户表达构成

- 使用已有 Signal `kind`：问题、需求、观念、情绪、经历、解决方案、引述、语境、疑问。
- 表达类型不是主题、用户规模或需求强度。
- 点击类型只筛选总览代表原声，不写数据库。

### 4.3 本期值得看的问题

- 优先显示最近一次 Run 中新增 membership 的 active Problem；没有新增时回退到累计 membership 较多的问题。
- 显示稳定定义、累计关联数、本次关联数与一条可读取原声。
- 明示“不是需求排行榜”；不得根据 membership 数推导市场规模。

### 4.4 归一表达、办法与经历

- “归一表达”来自 Signal proposition，不冒充社区原始词频。
- 只有完全相同的 proposition 才计为重复；点击后筛选相关原声。
- Solution 与 Experience 分开呈现；只陈述用户提到了什么，不推断方案有效性。

### 4.5 代表原声

- 使用 Evidence Serif；系统文字不得使用该字体伪装成原声。
- 来源受限时不显示原文。
- 原声按已关联 Signal 数优先展示，只是阅读入口，不是点赞或重要性排名。

### 4.6 尚未看清

- 只显示真实 target/resolution 状态：等待语境、独立新证据、多个候选、目录未完整、预算停止、协议拒绝、失败和来源受限。
- 这是信息边界，不是人工审核待办。
- Unknown、Known(0)、Partial 必须分别表达。

## 5. 状态与行动

| 状态 ID | 触发/数据来源 | 用户应理解什么 | 禁止暗示什么 | 可用下一步 |
|---|---|---|---|---|
| `NO_RUN` | 无 StudyRun | 尚未开始本轮研究 | 评论不存在或本轮失败 | 发起研究、查看评论库存 |
| `RUNNING` | queued/running target 或 prepared/leased batch | 工作已创建，尚无最终结果 | 已有 Signal/Problem | 查看运行记录 |
| `NO_SIGNAL` | target state `no_signal` | 模型明确未发现可接纳 Signal | 缺输出/失败、用户没有问题 | 查看原声 |
| `DEFERRED` | Signal resolution state | 信号存在，但当前不能安全归并 | 已建 Problem | 看等待条件 |
| `PRIMARY_PAIR_RECORDED` | selection / decision manifest | 已比较首个合格候选，结论与边界可读 | 已穷尽候选 | 查看中文结论 |
| `PROBLEM_LINKED` | membership | 已依据闭集比较关联稳定问题 | 规模、趋势或普遍性 | 查看 Problem |
| `PARTIAL` | accepted 与 retry/failed 同时存在 | 有效结果保留，其他项未完成 | 全量完成 | 查看运行记录 |
| `SOURCE_RESTRICTED` | target excluded / batch cancelled | 当前来源不可再处理 | 已外发或已安全完成 | 查看安全原因 |

## 6. 已批准组合

| 区域 | 来源 | 用途 | 禁止替代 |
|---|---|---|---|
| shell + corpus side nav | LIDS-SHELL-001 | 统一入口和范围 | 单页覆盖 shell 样式 |
| 文字 Tab | LIDS-PRI-001 ADR-11 | 五个工作面 | 分段控件/重复页面 header |
| Readout Strip | LIDS 数据展示 | 观察基础 | 大数字卡片墙 |
| Evidence Fragment | LIDS 数据展示 | 用户原声 | 把系统摘要伪装成原声 |
| 连续表格 | LIDS-PAT-001 Corpus Explorer | 20+ 同构目标、Signal/Problem 与执行记录 | 卡片瀑布流 |
| 状态/空态 | LIDS-BOUND-001 + LIDS-LANG-001 | 中文、Known(0)/Unknown/Partial 分开 | `暂无数据` 万能空态 |
| 主动作 | LIDS Controls | 发起研究 | 多个同权重黑色按钮 |

## 7. 研究启动弹窗

- 页面首屏只保留五个 Tab 与一个「发起研究」主动作，不重复页面标题、连接状态或说明 Hero。
- 模型配置、评论预算、语境预算和作品选择继续留在 Dialog。
- 候选作品使用连续表格；筛选只作用于已加载 `eligibleWorks`，全选只作用于当前可见结果。
- `selected-count`、checkbox、表头全选和创建 Run 启用状态消费同一选择集合。
- 创建 Run 成功只表示输入被冻结；不得写成模型完成。成功后选择新 Run 并进入运行记录。

## 8. UX 合同

- 总览筛选、表达类型、归一表达和代表原声使用同一阅读上下文；筛选可一键清除。
- 总览中的“查看”带到对应既有 Tab，不创建第二套详情对象。
- 慢请求不得覆盖用户后来切换的 Tab/Run；继续使用 render token 丢弃过期响应。
- 桌面以 1440 CSS px 为基线；窄屏回退为单列和自然页面滚动，不建立嵌套横向阅读区。
- 焦点环、键盘 Tab、Dialog 关闭和 Escape 行为使用共享 LIDS 规则。

## 9. 真实后果

- 总览只组合 `overview / setup / targets / signals / problems / runs` 的已有响应。
- 当前实现使用最近一次 Run 作为本期范围，不宣称跨时间趋势。
- 当前 API `limit=100` 的读取上限必须披露；不能用当前页长度冒充完整总量。
- 开始研究、保存策略仍是既有显式动作；其它页面动作均只读。

## 10. 验收边界

- 自动：原有路由、Dialog、五 Tab、选择集合、状态词典和来源限制测试继续通过；新增总览必须有静态结构与只读行为断言。
- 页面：1440px 可在首屏看到读数带和用户表达/问题；390px 无强制嵌套滚动；原声、系统文字和技术状态不混淆。
- 数据：有 Run、无 Run、零 Signal、无 Problem、部分完成、来源受限、读取失败各有独立中文状态。
- 不证明：真实模型质量、需求趋势、共享数据库完成率、部署、业务验收。
