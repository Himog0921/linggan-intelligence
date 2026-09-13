# XHS Comment 来源字段审计（新 Rust 项目 P1）

状态：`PARTIAL — Comment Fact Storage V0 已有隔离 PostgreSQL 证明；完整生产来源模型仍冻结`
审计日期：2026-09-13
范围：只读审计 `references/current-v2/` 的插件与工作台固定快照；不代表新仓库已经接入这些 producer。

## 结论

当前参考快照足以确认：XHS 评论必须以 `noteId + commentId` 组成稳定身份，最小可确认字段是父作品身份和评论文本。它不足以证明父/根评论、评论者、点赞、发表时间、作品正文/OCR/ASR 在每个采集 profile 中都稳定可用。

一组来自现有本地运行时、已去标识化的 `content_detail + comments` CapturePackage 已保存为 [`fixtures/xhs/comment-evidence-set-v1.json`](../../fixtures/xhs/comment-evidence-set-v1.json)，其 SHA-256 为 `0eb0dd567bafabb4b2511c5c83531959c544cb8a90261050c17a6e4ca38e6234`。它已经支持一个严格限于已证明字段的 **Comment Fact Storage V0**：来源形状 validator、不可变 Evidence/CommentObservation、稳定评论身份与 current projection 均有隔离 PostgreSQL proof。由于只有一个配对样本，它仍不能把所有可见字段写成必填或默认事实，更不能扩展为完整生产 DDL。特别是 `comment_probe` 不能伪造已获得作品正文或评论发表时间。


## Context V0 来源追加审计（2026-09-13）

第二份真实 producer 去标识化证据集 [`fixtures/xhs/comment-context-evidence-set-v0.json`](../../fixtures/xhs/comment-context-evidence-set-v0.json) 已保存，SHA-256 为 `310c9fdad4f5c1634fa686036d36462f5e23471667c54bab6c258c6e3fb68c67`。它包含 `content_detail`、`comments`、`replies` 三份独立包，且保留一条同时具有 `parentCommentId` 与 `replyToCommentId` 的真实 reply 关系反例。

已确认：

- `content_detail` 的 `title`、`bodyText` 和 `authorId` 只能作为按字段可用的上下文；详情包存在不等于正文存在。
- `replies` 的 `rootCommentId`、`parentCommentId` 与 `replyToCommentId` 必须分别保存；后两者不可按 XOR 或 fallback 规则折叠。
- 所有上下文都必须带具体来源 Evidence/record 引用，并限定在同一平台、同一 `noteId`；它们帮助理解当前评论，不能被写成当前评论者的直接证据。
- OCR、ASR、平台完整树、采集 profile/终态、评论发表时间与互动指标仍是 `UNKNOWN`。

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

## 已取得 fixture 与仍存缺口

已取得的 `comment-evidence-set-v1.json` 是真实 producer 的去标识化来源形状证据，不是手工按旧 schema 编造的 JSON。它证明本地运行时可分别交付 `content_detail` 与 `comments` 包，并让新项目在不接触原始个人数据的前提下验证外层包、record 和跨包 `noteId` 关系。

它**不**证明完整 collection profile、评论树、缺字段包或不同 collection profile 的稳定行为。V0 的 replay 与原文变化规则由对这个真实形状的脱敏、攻击性输入和隔离 PostgreSQL proof 验证；它不应被误读为 producer 已提供了所有这些场景。

## Fixture 仍需补齐的场景

后续 fixture 必须继续来自真实 producer 的脱敏 `CapturePackage`，而不是人工按照旧 Prisma schema 拼出的 JSON。至少覆盖：

1. `note_full`：一条作品与一级/回复评论，包含包终态和 coverage；
2. `comment_probe`：只有评论时的输入，明确没有作品内容而非填充空默认；
3. 同一 `commentId` 属于不同 `noteId` 的身份反例；
4. 同一评论的 replay 和文本变化输入；
5. 一个无正文、缺身份或非完整包的拒绝样本。

fixture 必须删除或替换账户名、原文 URL、Cookie、授权、个人联系方式和可反向识别信息，同时保留字段存在性、类型和 nested shape。其 canonical JSON、来源版本和 SHA-256 应写入新项目的 fixture manifest。

## P1 V0 开工门槛与剩余冻结项

以下条件已满足，因此可以新增**仅限 V0** 的数据库 baseline 与 Rust ingress：

- 当前 fixture 已进入本仓库；对应的 Rust runtime validator 与攻击性测试通过；
- Comment / CommentObservation / Current 的 V0 replay 规则有攻击性用例；
- 有隔离 PostgreSQL proof 证明 append-only、replay、跨作品同 comment id 和 current pointer 约束。

以下更宽的 P1 条件仍未满足，必须继续冻结：

- collection profile、terminal 和 coverage 的版本化语义；
- `comment_probe` 的父作品上下文不可用行为；
- WorkObservation、评论树、评论发表时间和完整来源字段的运行时合同。

V0 只证明现有配对形状下的事实接入链。缺少这些 fixture 和合同，相关领域与页面能力仍不得声称已实现。
