# COMMENT-RESEARCH-WINDOW-001 · 无当前样本时的研究版本表达

> 状态: 活跃计划
> 最后核对: 2026-09-14
> 适用范围: Issue #267；评论研究概览与用户问题在 ResultRevision 当前比较窗口无样本时的展示口径
> 事实来源: 真实 ResultRevision 6dbf52b8-2c09-41f6-8b05-3e5654f37998、PAGE-COMMENT-RESEARCH-V1-001、当前 localhost 页面
> 冲突时以谁为准: 用户最新决定、真实 ResultRevision、代码和数据合同

## 用户结果

一份已经发布的研究版本不能因为其冻结输入全部位于基线窗口，就在概览和用户问题表中把所有研究结果显示为当前 0。

当当前窗口评论分母为 0、基线窗口评论分母大于 0 时，页面明确切换为“本版基线样本”：
- 概览显示本版已被研究的问题、基线评论占比、基线作品覆盖与基线评论数；
- 用户问题显示实际被研究的基线样本和证据 Atom；
- 结果元信息说明当前窗口没有可比样本；
- 变化观察仍保持不可比较，不将基线样本解释为实时趋势。

当前窗口存在样本时，现有当前窗口和前一窗口比较保持不变。

## 数据与行为边界

只读取现有 immutable ResultRevision：
- inputCounts 的 baseline/current comment/work denominator；
- 每个 Problem 的已有 baseline/current window stat；
- 已发布 ResultRevision 的比较窗口。

不新建数据对象、列、迁移、模型调用、Run、人工操作或页面入口。
不修改原始评论、研究派生结果、Run 状态、比较计算、变化 Observation 或调用账本。

概览与用户问题在基线模式下按 baseline comment/work count 排序；当前窗口存在时继续按 current count 排序。这个排序变化只影响读取展示，不改变冻结统计。

## 验收

1. 真实 ResultRevision 6dbf52b8-2c09-41f6-8b05-3e5654f37998 的 current denominator 为 0 时，概览与用户问题不再以“当前”列显示全 0。
2. 页面文字清楚说明样本位于基线窗口、当前窗口没有可比样本；变化页仍保持不可比较。
3. API 返回的 baseline/current 数字不被转换或重写；没有新增模型 invocation。
4. current denominator 大于 0 时，原有当前/前一窗口表格结构继续存在。
5. 复用 L1 Corpus Explorer、已有 table/intro/meta 和 LIDS token；不改全局 shell 或导航。
