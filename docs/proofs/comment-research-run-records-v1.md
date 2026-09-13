# 评论研究运行记录 V1 · 隔离 HTTP / PostgreSQL 与页面合同证明

状态：PROVEN IN ISOLATION
日期：2026-09-13

范围：用户原声页内的「运行记录」tab，以及两个严格只读 API：

```text
GET /api/v0/comment-research/runs?workspace_id=&limit=&offset=
GET /api/v0/comment-research/runs/{run_ref}?workspace_id=
```

它们只读取已经存在的本地 Run、RunItem、最新 event 和已存在的 Conclusion，返回 Run 回执、创建时间、冻结输入、待执行、上下文阻断、未继续执行、已记录最终状态和来源作品覆盖的聚合数量。详情把 `needs_context_insufficient` 译为「需要补足上下文（阻断，不是执行失败）」。没有评论正文、冻结输入文本、Context Pack、fingerprint、Evidence/Derivation/Observation ID、模型策略、凭据或结构化输出字段。

## 隔离运行证明

```text
scripts/prove-comment-research-run-preparation-v1.sh
```

该脚本创建随机命名的 `postgres:16-alpine` 容器、随机数据库和随机 loopback 端口，并通过真实 Axum Router 先读取空记录，再用本地确认 POST 创建 Run，最后读取列表和详情。它不读取共享 runtime DSN、不占用 3000 端口，也不启动浏览器、worker、queue、provider、模型、向量服务或外部来源。

已验证：

| 场景 | 结果 |
| --- | --- |
| 空态 | 创建前 `GET /runs` 返回 `total=0` 和空列表；页面合同含「尚无待执行研究」。 |
| 创建后读取 | 创建 3 条冻结输入后，列表返回 1 个 Run：2 条待执行、1 条上下文阻断、0 条未继续执行、0 条已记录最终状态，覆盖 3 个来源作品。 |
| 阻断语义 | 详情把唯一阻断理由呈现为「需要补足上下文（阻断，不是执行失败）」；它不称为模型失败或评论结论。 |
| 严格只读 | 读取列表和详情前后的 Run、RunItem、RunItem event 与 Analysis 行数一致。 |
| 泄漏边界 | 递归断言列表和详情没有文本快照、fingerprint、内部 locator/ID、模型/provider、token、费用、prompt、queue 或 worker 字段。 |
| 页面合同 | 真实页面响应含可用「运行记录」tab、轻量详情入口、未配置执行器说明和两个 GET URL；页面不包含运行控制按钮。 |

## 未证明、不得声称

- 有头浏览器视觉验收、3000 runtime、共享数据库 migration、main 合并或业务验收；
- Provider / 模型 / Prompt / Token / 费用、队列、worker、租约、重试、取消、自动调度或任何自动执行；
- 评论结论、用户问题、问题归并或变化观察已经生成。
