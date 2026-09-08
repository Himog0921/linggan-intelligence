# 评论研究本地质量评测合同

> 状态: 代码事实优先
> 最后核对: 2026-09-08
> 适用范围: CI-20260907-V1 / T7，离线 B0、B1、B2 对照与人工留出集质量核验
> 事实来源: `docs/data-contracts/comment-intelligence-v1.md` 第 10 节、`scripts/evaluate-comment-intelligence.py` 和对应合成单元测试
> 冲突时以谁为准: 用户实施包、可复现评测输入与代码；本说明不替代真实人工标注、数据使用授权或上线验收

评测器只读取两份本地 JSONL。它不调用模型、不联网、不生成 gold、不修改运行配置。报告不回显评论正文、引用文字或模型答案，只保存计数、版本、输入文件 SHA-256 和有限的错误对象引用。

## 输入

gold 每行必须包含以下字段：

| 字段 | 规则 |
|---|---|
| `sourceRef`、`workRef` | 稳定且非空的对象引用；一条原声只出现一次 |
| `text` | 原始文字，用于 UTF-8 SHA-256 与 Unicode 字符位置核验 |
| `labels` | `need`、`solution`、`story`、`quote` 的非重复数组，可多选或为空 |
| `problemKey` | 人工定义的问题身份；没有问题归属时显式 `null` |
| `split` | `calibration` 或 `holdout`；同一作品不能跨两侧 |
| `isSynthetic` | 必须显式声明 `true` 或 `false`；不得将合成测试改标为真实样本 |
| `annotationSource` | 真实质量资格要求为 `human`；辅助模型判断不能代替人工标准 |

predictions 每行必须包含 `sourceRef`、`labels`、`problemRef`、`sourceSha256`、`evidence`、`contextMode`、`modelVersion`、`ruleVersion`。`problemRef` 可为 `null`；`evidence` 是 `{sourceRef,startChar,endChar,quote}` 数组，位置为 Unicode 字符半开区间。没有语义判断时可以没有引用；有标签或问题判断时不能用空引用通过。

`contextMode` 只接纳 `B0`、`B1`、`B2`。同一模式下同一原声只能有一份预测，不能静默选取最好的一次。三种模式必须使用同一输入集合、同一个模型版本和同一个规则版本才能声明可比。模式区别单独由 `contextMode` 标记。可选 `latencyMs`、`inputTokens`、`outputTokens` 缺失时是未知，不填 0。

高共鸣、高冲突的审查单位是**观察**，不能拿评论级标签精度替代。仍使用上述两份文件：

- gold 行可附 `groupReviews`：`{observationRef,kind,accepted,sourceRefs,reviewer:"human"}`，`kind` 为 `resonance` 或 `conflict`。`accepted` 是独立人工判断。
- prediction 行可附 `observations`：`{observationRef,kind,sourceRefs}`。
- 同一观察可在多行重复引用，但内容必须一致；审查与预测的证据集合必须完全相同。跨校准／留出集或混合真实／合成证据的观察会被拒绝。
- 没有人工审查、没有足够正例或证据集合变化时，相关精度保留 `null`／证据不足状态。

## 输出与资格

报告按**真实／合成 × 校准／留出 × B0／B1／B2**分别计算，禁止把容易的合成样本混入真实精度分母。

- 每个评论类别分别报告 TP、FP、FN、precision、recall、F1；没有分母时为 `null`。
- 引用率核对原文哈希、对象存在、Unicode 边界与逐字引用。文本存在不等于引用支持结论，后者仍由人工 gold 的语义判断检验。
- 错误归并率为“预测同组的评论对中，不属于同一个人工问题的比例”；同时报告 pairwise recall 与未归并评论比例。完全拒绝归并的错误率为未知，不能得到 0% 错误的合格结果。
- 独立审查高共鸣／高冲突观察的 precision；未经审查或证据集合不一致的发现不得通过。
- 报告实际 B1−B0、B2−B1 的 F1 差异，允许零增益或负增益；差异不自动证明因果关系。

`qualifiedAutomaticDiscovery` 默认不满足资格；仅在声明为人工标注的 200–300 条真实评论、至少 20 篇作品、作品隔离的校准与留出集、合成补充不超过 60 条、三模式可比、B2 留出集引用与覆盖检查通过，并达到核心类 precision ≥0.85、两类观察 precision ≥0.90、错误归并率 ≤0.05 且存在实际归并时，才为 `true`。不满足条件时输出 `NOT_QUALIFIED` 和具体 `blockers`。

这一字段只表示**这份输入满足本地质量评测门槛**，不授权自动发布、外发评论或部署。输入的真实来源与人工身份仍是提交者的明确声明，程序不能认证；小分母也不能用阈值假装有统计置信度。真实语义质量尚未评测时必须保留未核实状态。

## 运行

```sh
python3 scripts/evaluate-comment-intelligence.py \
  --gold /private/path/gold.jsonl \
  --predictions /private/path/predictions.jsonl \
  --output /tmp/comment-intelligence-evaluation.json \
  --require-qualified

python3 -m unittest discover -s scripts/tests -p 'test_comment_intelligence_evaluation.py'
```

默认成功生成报告即返回 0；使用 `--require-qualified` 时，未达质量门槛返回 1。无效输入、作品泄漏、重复预测等返回 2。脚本禁止用报告覆盖任一输入文件。真实输入与生成报告保留在私有位置或系统临时目录，不进入 Git；提交的测试只有四条明确合成评论，用于验证计算与拒绝路径，不是模型质量成绩。
