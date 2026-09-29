# COMMENT-STUDY-P2-CLOSEOUT-001 · UI 变更清单

> 状态: 草案
> 最后核对: 2026-09-29
> 适用范围: `/corpus/comments` 的研究选择、目标详情、研究结果与运行汇总
> 事实来源: 用户确认的 P2 交付范围、COMMENT-STUDY-PRODUCTIZATION-001、当前 API/数据合同及 2026-09-29 Run `4685462d-96e3-40c5-8496-e4805421414e`
> 冲突时以谁为准: 用户最新确认、`AGENTS.md`、当前数据库事实、当前代码与合同

## 1. 事项

- Issue / SCOPE: COMMENT-STUDY-P2-CLOSEOUT-001；不替代已合并的 PR #338/#348。
- Agent 与 worktree: `/root` / `codex/comment-study-p2-closeout-001`。
- 基线: `origin/main@df021f72548cc37d93d9e3df1a6f4bd0a6d250dd`。
- 目标: 完成 P2 的输入上下文、继续可恢复目标、过大响应恢复、研究结果透明度与重复/零合格作品排除。
- 用户可见结果: 默认启动能继续输入已变化且现已可研究的 needs_context 项、重试可恢复未完成项；每条结果显示原声、该 Run 实际冻结的工作/父评论语境、模型原因或故障原因和下一步；合法 Signal 可直接查看；作品列表去重并隐藏零条可研究评论的作品。
- 明确非目标: 不推进 P3/P4 新状态机或 Problem 归并、不调用真实模型、不改共享数据库/迁移、不删除合法研究历史。用户此前已授权本事项完成后合并 main 并刷新 `:3000`；业务验收仍由 Mog 完成。

## 2. 读取回执

| 来源 | 状态 | 本次解决的问题 | 已核对 |
|---|---|---|---|
| `AGENTS.md` / current-state | 已读 | 受保护交付、worktree、真实副作用边界 | 2026-09-29 |
| UI execution contract / LIDS | 已读 | 页面状态、展示限制与验收证据范围 | 2026-09-29 |
| COMMENT-STUDY-PRODUCTIZATION-001 总体计划/页面规格 | 已读 | P2 顺序、输入合同与现有用户路径 | 2026-09-29 |
| HTTP / 数据合同 | 已读 | Run 模式、目标投影与状态名称 | 2026-09-29 |
| 当前实现与运行数据 | 已读 | latest Run、target 输入、模型 reason、实际父评论关系 | 2026-09-29 |

## 3. 变更分类

- 分类: 混合；最高风险为状态语义与冻结输入真实性。
- 对应来源: P2、LIDS Data Truth、`comment-study.read.v1/v2`、`comment-study.run-selection.v2`。
- Pattern: L1 Corpus Explorer；复用当前表格、详情 dialog、Tab 与状态组件；不新增 Token、CMP、Scene 或 Motion。
- `DECISION_REQUIRED`: 无。直接父评论只解释指代，不能替代目标评论证据；Problem 建立仍不在本范围。

## 4. 表面、状态、依赖与边界

| 表面 | 必须表达 | 禁止表达 |
|---|---|---|
| 研究作品目录 | 每个 workRef/role 一行；仅列至少一条当前可研究评论的作品 | 重复采集记录变成重复研究作品；零合格作品仍可勾选 |
| 启动默认值 | 新评论 + 失败/取消续做 + 输入变化后的 needs_context/恢复资格项；成功项和在途项不重跑 | 把 continue-ready 伪装成旧 `new_only`；对未变化 needs_context 无限重试 |
| 目标详情 | 原声 → Run 冻结工作语境 → Run 冻结父评论 → Signal 或模型 reason/失败原因 → 可恢复动作 | 把当前新读到的语境说成历史 Run 输入；把上下文写成评论证据 |
| 研究结果 | 选中 Run 的所有合格 Signals，包括已归并、待归并与其他合法状态 | 只显示“待归并”而隐藏已处理 Signal |
| 运行汇总 | 成功信号、无信号、等待语境、失败、排除分别计数 | 将所有终态压成单一“已完成”计数 |

页面按权威 PAGE 规格使用四个一级视图：概览、用户评论、研究批次、用户问题。研究批次内通过目标评论／研究结果子面板先展示冻结目标和输入诊断，再展示该 Run 全部合法 Signal；“待归并”只表示 Problem 归并状态，不作为全部研究结果的筛选条件。

父评论原文当前受限时必须隐藏；技术 batch/call 细节折叠，不展示 prompt、密钥或 provider 原始流。

## 5. 验收与证明边界

| 层级 | 验收方法 | 实际结果 | 未证明边界 |
|---|---|---|---|
| 目录 | 隔离 PostgreSQL 重复 association/零 eligible 反例 | 待实施 | 当前生产/本机数据之外的覆盖规模 |
| 上下文与续做 | 固定样本验证 root、reply、parent missing、context fingerprint 改变及不变 | 待实施 | 人工 Gold Set 与语义质量 |
| 响应恢复 | 注入 SSE 262144 字节超限，验证仅未接纳目标拆为单目标尝试 | 待实施 | 真实 provider 端容量与成本 |
| 页面与结果 | API/静态脚本及隔离浏览器检查；Run 理由、冻结上下文与所有合法 Signal 可见 | 待实施 | 共享运行时 `:3000` 和 Mog 人工验收 |
| 页面 URL 路由 | 页面入口接受页面规格登记的导航字段（domain、view、q、workRef、runRef、problemRef、commentExternalId、state、cursor、detail、panel），API 查询仍各自严格 | exact-head `f7b1201a61b1e74cf582e8d825f1451ba4ab84b1` 独立静态复审无阻断 | 候选修复尚未合并或部署到共享运行时 `:3000`；运行时回归和 Mog 人工验收未完成 |

## 6. 交接

- 预期文件: selection/context、batch failure handling、read projection/API、`comment_study.html/js/css`、HTTP 合同、总体计划、progress 与本清单。
- 验证命令与结果待实施后登记。
- 不申请共享迁移或真实 provider。完成 exact-head 验证后按用户既有授权合并并刷新 `:3000`；Mog 的实际页面验收仍待完成。
