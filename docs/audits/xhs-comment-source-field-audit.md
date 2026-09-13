# XHS Comment 来源字段审计（新 Rust 项目 P1）

状态：`PARTIAL — 不足以创建生产 DDL`
审计日期：2026-09-13
范围：只读审计 `references/current-v2/` 的插件与工作台固定快照；不代表新仓库已经接入这些 producer。

## 结论

当前参考快照足以确认：XHS 评论必须以 `noteId + commentId` 组成稳定身份，最小可确认字段是父作品身份和评论文本。它不足以证明父/根评论、评论者、点赞、发表时间、作品正文/OCR/ASR 在每个采集 profile 中都稳定可用。

因此新项目在取得一组脱敏的真实 `CapturePackage` fixture 前，不能创建把这些字段写成必填或默认事实的数据库基线。特别是 `comment_probe` 不能伪造已获得作品正文或评论发表时间。

## 已确认的来源线索

| 事实 | 参考证据 | 新项目可采用的含义 | 确定性 |
| --- | --- | --- | --- |
| 评论身份依赖父作品 | 工作台 `xhs-derived-contract.ts`：comment adapter 以 `noteId` + `commentId` 形成 `subjectKey` | Comment identity 的最小形式为 `(workspace, platform, note_source_id, comment_source_id)` | 已确认 |
| 文本存在时是 comment 内容 | 同一 adapter 的 `fieldPresence.text` | `text` 为 CommentObservation 的候选事实；缺失必须被拒绝或保留为未接受输入 | 已确认 |
| note_full 与 note_detail 可携带 comment record | 工作台 `08-xhs-collection-contracts.md` | comment 可与作品事实同包进入，但不代表所有 profile 都有完整作品上下文 | 已确认 |
| comment_probe 可只携带 comment record | 同一 collection contract | 研究 UI 必须允许 `work_context_availability=unavailable` | 已确认 |
| 插件会在部分路径产出 parent/root/reply/likes/time 等字段 | 插件 `commentApi.js`、`commentCollector.js` | 这些只能作为待 fixture 证明的可选字段，不能成新库默认或模型输入承诺 | 来源线索，未完成 fixture 证明 |
| CommentObservation 是不可变历史，Current 指向 accepted observation | 工作台 `01-decisions.md` 的 DEC-B3-004 | 新项目应重建 replay/变化/accepted-current 语义，不复制旧 Prisma 表 | 已确认的参考语义 |

## 已确认的最小来源合同

```text
platform = "xhs"
noteId: non-empty string
commentId: non-empty string
text: non-empty string
```

它只够识别一个评论候选，不够生成完整的评论研究输入。

## 禁止的推断

以下字段在新项目当前状态都是 `UNKNOWN`，不能写为 0、空字符串默认事实、必填列，或被假装输入模型：

- 评论者身份、昵称和是否为作者；
- 父评论、根评论、回复指向；
- 点赞/回复数；
- 评论发表时间及时间精度；
- 作品标题、正文、OCR、ASR；
- 采集时的排序、页面可见总数、采集数、停止原因和 coverage；
- 评论媒体、位置、联系方式和其它附加字段。

## Fixture 缺口与取得条件

第一份新项目 fixture 必须是来自真实 producer 的脱敏 `CapturePackage`，而不是人工按照旧 Prisma schema 拼出的 JSON。至少覆盖：

1. `note_full`：一条作品与一级/回复评论，包含包终态和 coverage；
2. `comment_probe`：只有评论时的输入，明确没有作品内容而非填充空默认；
3. 同一 `commentId` 属于不同 `noteId` 的身份反例；
4. 同一评论的 replay 和文本变化输入；
5. 一个无正文、缺身份或非完整包的拒绝样本。

fixture 必须删除或替换账户名、原文 URL、Cookie、授权、个人联系方式和可反向识别信息，同时保留字段存在性、类型和 nested shape。其 canonical JSON、来源版本和 SHA-256 应写入新项目的 fixture manifest。

## P1 开工门槛

以下条件满足后，才可以新增数据库 baseline 与 Rust ingress：

- fixture 已进入本仓库并通过 runtime validator；
- 选定的 collection profile、terminal 和 coverage 字段已写成版本化合同；
- Comment、CommentObservation 与 WorkObservation 的 accepted/current 规则有攻击性用例；
- 明确 `comment_probe` 的父作品上下文不可用行为；
- 有隔离 PostgreSQL 环境用以证明 append-only、replay、跨作品同 comment id 和 current pointer 约束。

在此之前，本仓库可独立推进确定性清洗、页面信息合同与 fixture tooling，但不能宣称 Evidence 或 Comment 领域链已实现。
