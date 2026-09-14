# COMMENT-RESEARCH-WINDOW-001 · 无当前样本时的研究版本表达

> 状态: 活跃计划
> 最后核对: 2026-09-14
> 适用范围: Issue #267；ResultRevision 统计元信息与变化观察在当前比较窗口无样本时的展示口径；概览/用户问题主体已由 #281 替代
> 事实来源: 真实 ResultRevision 6dbf52b8-2c09-41f6-8b05-3e5654f37998、PAGE-COMMENT-RESEARCH-V1-001、当前 localhost 页面
> 冲突时以谁为准: 用户最新决定、真实 ResultRevision、代码和数据合同

## 用户结果

一份已经发布的统计版本不能因为其冻结输入全部位于基线窗口，就被误读为“当前窗口的 0”。

当当前窗口评论分母为 0、基线窗口评论分母大于 0 时：

- ResultRevision 的统计元信息明确说明样本位于基线窗口、当前窗口没有可比样本；
- 变化观察仍保持不可比较，不将基线样本解释为实时趋势；
- 概览和用户问题的主体改由 COMMENT-RESEARCH-CUMULATIVE-STATE-001 读取累计 confirmed membership，不能借用 baseline/current 统计列、占比或排序。

当前窗口存在样本时，ResultRevision 的统计元信息照实说明当前窗口；变化比较规则不变。

## 数据与行为边界

只读取现有 immutable ResultRevision 的统计元信息与变化边界：
- inputCounts 的 baseline/current comment/work denominator；
- 每个 Problem 的已有 baseline/current window stat；
- 已发布 ResultRevision 的比较窗口。

不新建数据对象、列、迁移、模型调用、Run、人工操作或页面入口。
不修改原始评论、研究派生结果、Run 状态、比较计算、变化 Observation 或调用账本。

概览和用户问题的累计 membership 排序、计数与空态由 #281 规定；本计划不得要求它们按 baseline/current count 排序。

## 验收

1. 真实 ResultRevision 6dbf52b8-2c09-41f6-8b05-3e5654f37998 的 current denominator 为 0 时，统计元信息明确说明样本位于基线窗口，变化页仍保持不可比较。
2. API 返回的 baseline/current 冻结统计不被转换或重写；没有新增模型 invocation。
3. 概览和用户问题不出现 baseline/current 统计列、占比或统计排序；它们只按 #281 的累计 membership 合同读取。
4. current denominator 大于 0 时，统计元信息照实说明当前窗口；变化判断仍按既有可比条件。
5. 复用 L1 Corpus Explorer、已有 table/intro/meta 和 LIDS token；不改全局 shell 或导航。
