# 评论自动研究范围预览 V0 · 隔离 PostgreSQL 证明

状态：PROVEN IN ISOLATION  
日期：2026-09-13  
范围：向「用户原声」页提供一份当前、可解释、只读的自动研究范围预览。它只读取当前 Comment 投影和 comment-cleaning.v1 派生；不创建研究计划、任务、队列、指纹、输入包或模型调用。

## 用户可见的结果

在已加载工作空间的「用户原声」页，用户可打开右侧的「查看自动研究范围」抽屉，并调整：

- 范围：全部可用、可直接研究、需要上下文；
- 显示上限：20、50 或 100 条。

抽屉明确显示：

- 当前原声、可用语料、可直接研究、需要上下文、等待清洗和确定性排除的数量；
- 本次预览实际选到的来源作品分布；
- 清洗后研究表达及它在所属来源中的轮换轮次。

候选不是用户手工勾选的集合。系统先取每个来源的第 1 条，再取各来源的第 2 条；排序只使用本系统的本地接入顺序和稳定来源身份。它不代表平台发布时间、内容价值、模型优先级或真实研究计划。

任何一次预览都是 live read：重新打开或更新后会按当前 Comment Current 重新计算。预览不会冻结样本、锁定评论、创建任务、组装上下文、估算 Token/费用、调用模型或自动执行。

## 数据边界

存储适配器使用单条 SQL statement：

    comment_current_v0
      → current CommentObservation
      → LEFT JOIN comment_derivation_v1
           WHERE cleaning_contract = comment-cleaning.v1

该 statement 同时得到：

- current_total：所有当前 Comment Current；
- available_total：analyzable + needs_context；
- ready_total：analyzable；
- needs_context_total；
- awaiting_cleaning_total：当前 Observation 尚无 V1 派生；
- excluded_total：dropped + anomaly；
- 按来源轮换后的有界候选与本次选中来源分布。

dropped、anomaly 和尚未物化 V1 的 current Comment 都不会成为候选。浏览器 DTO 只返回清洗后研究表达、V1 readiness、系统本地接入时间、来源作品 ID、来源轮换轮次及已有 Evidence locator；不返回原始正文、Comment ID、作者、URL、点赞、平台发表时间、清洗原因、派生 ID、fingerprint、模型、OCR/ASR 或媒体字段。

当前 Comment 被新正文推进后，旧 Evidence locator 不再属于 current projection，因此旧来源不会继续出现在预览候选中。

## 空状态口径

页面按准备总数区分：

| 情况 | 用户看到的解释 |
| --- | --- |
| current_total = 0 | 尚未接入用户原声 |
| 可用为 0 且等待清洗大于 0 | 原声仍等待本地清洗物化；预览不会写入 |
| 可用为 0 且确定性排除大于 0 | 当前原声均被确定性排除；不等于没有讨论 |
| 所选范围无候选 | 切换语料状态可查看，不声称已研究或拒绝评论 |

## 运行证明

    scripts/prove-comment-research-plan-preview-v0.sh

该脚本启动随机命名的 postgres:16-alpine 容器、随机数据库和随机 loopback 端口，仅向 ignored integration test 提供临时 DSN；结束后删除容器。它不读取共享数据库、已配置运行时或外部服务。

已验证：

| 场景 | 结果 |
| --- | --- |
| 来源轮换 | A 来源有两条、B/C 各一条，四条预览顺序为 A1、B1、C1、A2 |
| 来源分布 | 返回本次已选来源的 scope 可用数与本次显示数 |
| 范围与总数 | available、ready、needs_context 与六项准备总数一致 |
| 清洗缺口 | 当前 Observation 无 V1 派生只计入等待清洗，不进入候选 |
| 确定性排除 | dropped、anomaly 保留事实但不成为候选 |
| 空状态数据 | 无 current、范围无候选、仅排除、仅等待清洗均返回可区分的真实总数 |
| 参数攻击 | 缺少工作空间、空工作空间、错误 scope、0/101/非整数 limit 返回 400 |
| 确定性 | Current 未变化时，重复 GET 返回相同预览 |
| Current 推进 | 正文变化后旧 Evidence locator 不再出现在候选，新 locator 出现 |
| 零副作用与隐私 | 全部 GET 前后表行数不变；DTO 不泄露禁止字段 |

## 未证明、不得声称

- 研究任务、Run、队列、失败重试或已研究/无信号结果；
- Research Fingerprint、Context Pack、Prompt/Skill、Token、成本或模型调用；
- 向量、聚类、Problem、趋势、日批自动化；
- 共享数据库 migration、main 合并、3000 端口或浏览器人工验收。
