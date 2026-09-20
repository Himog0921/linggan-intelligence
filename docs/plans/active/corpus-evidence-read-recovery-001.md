# CORPUS-EVIDENCE-READ-RECOVERY-001 · 语料 Evidence 读取恢复

> 状态: 活跃计划
> 最后核对: 2026-09-21
> 适用范围: Issue #308；Evidence Library 列表与受控评论通道的本机读取恢复
> 事实来源: 2026-09-20 loopback API/数据库查询计划、DEC-0006、共享 Work Resource Read 合同与当前代码
> 冲突时以谁为准: 用户最新授权、AGENTS.md、DEC-0006、真实数据库副作用、当前代码与可复现测试

## 用户结果

语料库页面能在受控的 Work Resource 读取合同下返回已有材料；从检查器进入评论时，只返回当前可读、未受限评论。受限评论正文和外部身份继续不暴露。

## 已确认事实

- Comment Study reset 有意删除 `linggan_comment_research_readable` 等 V1 关系；DEC-0006 禁止复活 V1 兼容路径。
- Evidence 的受控评论路由仍 JOIN 该退休视图，导致 `material_comment_channel_unavailable`。
- `/api/local/work-resources` 的固定 50 条页逐条执行完整 Media V2 enrichment；live 查询计划显示 media origin、processing job、derivative 和 job event 的关键 lookup 缺少索引，列表请求超时而非返回空集。
- 原始 Content、Comment、CapturePackage 与当前 material restriction 事实不在本事项中修改。

## 实施边界

1. 以 append-only `0094` 建立/保留当前 `linggan_material_comment_restriction` 资格表，并在旧限制表仍存在时只复制限制事实，不复建 V1 read/run/result schema。
2. 评论通道直接查询 `linggan_material_comment_current`，以当前 restriction table 排除不可读评论。
3. 为现有固定页 Work Resource 媒体 enrichment 的 `content_ref/slot/job/event` lookup 增加四个读取索引；不复制 Current 裁定、不改变 Media V2 DTO 或详情字段语义。
4. 隔离 PostgreSQL/API 证明旧 V1 view 已不存在时的可读/受限行为，以及新索引确实是目标表上的兼容 btree 索引（键顺序与排序均匹配）。

## 非目标与停止条件

- 不修改 Raw Comment/Evidence、采集、插件、模型调用、Comment Study Run/Policy/Problem、UI 设计或页面字段。
- 本机 loopback 已在用户授权下完成共享 `0094` migration、PR #320 合并和 3000 runtime 切换；未执行浏览器插件重载、外部平台访问、采集或模型调用。
- 若当前 restriction 合同无法表达原有读边界、需要恢复 V1 relation、或会改变原始材料，停止并报告。

## 验收矩阵

| 表面 | 正向证据 | 负向证据 |
|---|---|---|
| Evidence 列表 | 完整 schema 中四个索引存在；固定页仍使用唯一 Work Resource Current owner | 不创建第二个列表投影，不改变详情 Media V2 contract |
| 评论检查器 | 退休 V1 view/table 被删除后，当前可读评论仍返回 | 写入 current restriction 后正文和条目均不返回 |
| 数据边界 | 迁移只建索引、建当前 restriction 表和复制旧限制事实 | 不写 Raw Comment、CapturePackage 或任何 Study 派生数据 |
| 运行边界 | isolated PostgreSQL/API、静态/编译/治理检查；0094 真实账本/索引、3000 API 与页面 | 浏览器插件、外部平台、采集、模型和 Mog 业务验收均 NOT VERIFIED |
