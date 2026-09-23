# 评论研究产品化 · 来源和验收账本

> 状态: 实施记录；不作为已通过验证的声明
> 最后核对: 2026-09-23
> 适用范围: COMMENT-STUDY-PRODUCTIZATION-001
> 事实来源: 用户提供的手册v1.0与本分支的实际提交
> 冲突时以谁为准: 实际执行证据与对应版本合同

- `source-lock.json`：成文时固定源码和LIDS附件的来源回执，不是当前全量部署状态。
- `document-check.original.json`：2026-09-22文档交付时的静态检查原回执。保留其“未写GitHub”等历史描述，不以它报告本次状态。
- `acceptance-status.json`：T01–T54实际业务验收台账；每条由对应层级的执行证据才能改为PASS，文档或Schema检查不能替代Rust/数据库/浏览器证明。
- [P0来源与现场边界](p0-execution.md)：源码部分已核对，本机数据库与在途调用未核验。
- [P1逐步回看](p1-read-counts.md)：批次计数、schema-phase候选、确定性清洗缓存与测试边界；完整P1未完成。

机器合同位于 [data-contracts](../../data-contracts/comment-study-productization-001/README.md)，完整技术入口位于 [手册包](../../runbooks/comment-study-productization-package.md)。新执行记录按阶段追加，并引用手册章节及测试ID；不得用新设计默默覆盖过去已经批准的字段。
