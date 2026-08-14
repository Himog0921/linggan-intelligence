# 06 — V2 运行契约

> 状态：Phase 0 已固化。本文是运行边界，不是 DDL、Prisma schema、迁移脚本或实施授权。

## 目标

V2 建立一条单轨事实链：

```text
外部世界 → Evidence → Derived → Canonical → Projection / UI
```

任何页面、AI 能力或业务模块都不得重新解释原始采集数据、旧字段或媒体 URL。

## 允许的写入与读取边界

| 层 | 唯一允许的入口 / 写入者 | 允许读取者 | 禁止事项 |
|---|---|---|---|
| Evidence | `EvidenceIngress` 接收真实执行、手动导入、恢复、历史迁移四类提交 | 受控 Derived 服务 | 覆盖/移动旧 Evidence；页面、AI、业务模块直读；非执行来源伪造执行关系 |
| Derived | 标准化、合同裁决、匹配和规则处理服务 | 受控 Canonical Projector、AI 服务 | 修改 Evidence；绕过来源创建结果 |
| Canonical | 仅由接受的 Derived 结果经领域 Projector 写入 | Projection Builder、受控业务服务、AI 服务 | 直接消费 RawSnapshot / RawRecord；从旧字段或 URL 猜造事实 |
| Media Domain | 单一 Media Service Boundary | Canonical Projector、Projection Builder | 内容、作者、选题等业务模块直接创建媒体、直接查询媒体表或拼接 URL |
| Projection | Projection Builder | 页面和页面 API | 页面自行查表、解析媒体、拼 URL、实现 fallback |

## Evidence 运行规则

1. `RawSnapshot` 是一次外部观察的唯一 Evidence Registry；`RawRecord` 只能是该快照的原始片段。
2. 相同 capture identity 且完整 Capture Package hash 相同，只关联既有观察；不同观察必须追加新快照和片段。
3. 相同 capture identity 但 hash 不同，保留冲突 Evidence、阻止其进入投影、留下审计记录；重复同一冲突包不得无限新增。
4. `ARCHIVED` 可以用于分析和重算；`REDACTED`、`PURGED` 不可读取、分析或展示。具体自动归档周期不在 B1 自行设定。
5. 手动导入、恢复、迁移和人工修复必须真实标记来源，不得伪造 `ExecutionJob` 或 `CaptureAttempt`。
6. Evidence 的质量描述只表达本次观察的质量、置信度和验证结果，不表达永恒的“真相评分”。

## Media 与“打开原文”

- 本机现有媒体文件存储继续运行；本阶段不搬迁到新的数据库或对象存储。物理存储升级必须另行决策和验证。
- Media Domain 使用现有 `MediaItem`、`MediaOrigin`、`MediaMaterialization`、`MediaBlob`、`MediaReplica` 与业务关系模型演进，不新增平行媒体平台或多层 Resolver。
- `打开原文` 是明确的产品能力：Projection 可以提供一个由 Canonical 来源引用生成的“打开原文”动作。
- 该动作不是媒体交付 URL；页面不得从 Raw payload、旧 `sourceUrl` 字段或 Media Origin 自行取得、拼接或替换链接。
- 封面由一个中央 Media/Projection 规则统一决定：平台明确声明的封面为第一顺位；仅当它不存在时，才使用**同一内容、同一已认可观察中按顺序第一张已证明图片**作为第二顺位；再无候选时投影为明确的不可用状态。
- 这条规则是受控的封面选择，不是页面 fallback：页面不得自行取首图，不能使用旧 URL、跨内容图片或未经认可的原始 payload。人工封面选择规则仍以后续受控业务命令决定。

## 旧路径禁止项

下列名称只能作为平台 Adapter 解析历史输入的别名，不能成为 V2 运行时 Canonical、Projection 或页面取数来源：

```text
Author.avatar / avatarUrl
Content.coverImage
Topic.videoUrl
媒体 JSON
媒体 source URL / delivery URL 展示字段
旧媒体 helper、旧 URL 拼接、fallback
RawEvidence 新写入链
```

此处不禁止 Canonical 来源引用经 Projection 形成的“打开原文”动作；它与媒体展示 URL 是两种不同语义。

## 关系撤销与状态边界

- 内容、作者、选题拒绝或撤销时，撤销其媒体关系和该对象的可见 Projection；Media Asset 本身保留，供其它有效关系继续使用。
- 不为此新增无限组合状态。Capture、Evidence、Canonical、Media 的状态各自只表达所属责任；一个层的成功不得替代另一个层的完成。
- 关系撤销必须可审计；其最终字段或事件形式待对应模型 blocker 证明，实施者不得自行扩写状态体系。

## 阶段顺序与门禁

| 阶段 | 可开始条件 | 交付边界 | 完成门禁 |
|---|---|---|---|
| Phase 0 | 本契约已确认 | 文档、Blocker 映射、禁止项 | 无代码、schema、数据库改动 |
| Phase 1 / B1-E | BLK-001、002、003、010、015、016 已完成模型契约与隔离证明 | 所有入口统一进入 EvidenceIngress；停止新 RawEvidence、伪执行关系、Evidence 覆盖 | 新入口追加、重复关联、冲突隔离、权限反例均通过 |
| Phase 2 | BLK-004～009、012～014 已证明 | Canonical 与 Media 单一事实；移除 Evidence 直写媒体 | 新数据无旧媒体字段、无直写 Media、撤销不误删共享资产 |
| Phase 3 | Projection 读取模型已证明；强一致页面另满足 BLK-011 | 页面只读 Projection；封面按统一两级规则生成；移除旧 URL 和页面级 fallback | 页面绕过 Projection、旧字段读写、非统一封面逻辑均为零 |
| Phase 4 | Phase 1～3 稳定 | 仅迁移可证明历史关系 | 无法证明项不展示，进入人工审核清单 |
| Phase 5 | 全部 Release-B blocker CLOSED 且线上连续 48 小时门禁通过 | 删除旧字段、旧服务、旧 helper、兼容路径 | 旧读/旧写/fallback/页面绕过均为零 |

## 本阶段明确不做

- 不新增 `RawEvidence` 或平行 Evidence Registry。
- 不新增 MediaRegistration 平台或多层 Resolver。
- 不写 DDL、migration，不创建数据库，不迁移媒体文件。
- 不为历史数据猜造关系、补造 Evidence，或保留旧路径兜底。
