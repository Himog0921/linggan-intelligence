# 08 — XHS CollectionContract 实施合同

> 状态：已确认。由 DEC-B1-021 固定；B1-B-03-R1 的插件与工作台实现必须逐项遵循，不得按 `jobType`、`profile`、`requiredFields` 或测试数据推测替代。

## 1. 来源与边界

- 当前工作台的现役采集能力是 `list_scan`、`note_detail`、`note_full`、`comment_probe`、`author_profile`、`author_links`：`src/lib/services/task-demand-service.ts:312-325` 与 `src/lib/execution-task-routing.ts:107-117`。
- 当前插件在已验证的 reservation 中把这些 XHS 工作流分派为 `xhs.list_scan`、`xhs.note_full`、`xhs.comment_scan`、`xhs.author_profile`、`xhs.author_links`：`linggan-boom/src/workbench/runtime/taskLeaseClient.js:345-352`。
- 现役插件仍只发送 V1 `commit_raw_snapshot`，终态可为空：`linggan-boom/docs/technical/MESSAGE_PROTOCOL.md:252-285`。本文件定义的是其后续暗态 V2 包，不授权改写 V1 或接运行流量。
- 插件目前产出 `note`、`comment`、`author` 与 `media` V1 记录：`linggan-boom/src/workbench/runtime/taskPoller.js:799-837`。V2 没有 `media` RawRecord；该类数据只能序列化为 `media_inventory` Artifact。V2 首期不产生 `metric` RawRecord。

## 2. 六份固定合同

所有合同：`platforms=["xhs"]`、`version=2`、`terminalPolicy.allowedStates=["completed","blocked","cancelled","error"]`、`terminalPolicy.allowEmptyRecords=true`。v2 将 `xhs.record-payload/v2` 与 `xhs.media-inventory/v2` 的 schemaVersion/hash 一并纳入 CollectionContract canonical definition；来源合同变化会改变合同 hash，旧 v1 header 必须按未注册拒绝。空包只可表达终态无可写结构化记录；`completed` 且 `counters.emitted>0` 时仍受 07 §3.3 的非空 records 规则约束。

| Workflow | Contract id | RawRecord kinds | Slots | Media policy |
|---|---|---|---|---|
| `list_scan` | `xhs.list-scan` | `note` | `note_list` required | `metadata_only` |
| `note_detail` | `xhs.note-detail` | `note`, `comment` | `note` required; `comments` conditional | `metadata_only` |
| `note_full` | `xhs.note-full` | `note`, `comment` | `note` required; `comments` required | `metadata_only` |
| `comment_probe` | `xhs.comment-probe` | `comment` | `comments` required | `not_required` |
| `author_profile` | `xhs.author-profile` | `author`, `note` | `author` required; `note_list` optional | `metadata_only` |
| `author_links` | `xhs.author-links` | `note` | `note_links` required | `not_required` |

`metadata_only` 只允许 `media_inventory` 描述已观察到的媒体候选，不要求下载或上传媒体字节；任何真实附件仍须符合 07 §3.2 的 base64、长度和 hash 规则。`not_required` 不要求媒体附件，也不允许该合同把缺少媒体解释为采集失败。

## 3. 镜像与 fixture 规则

1. 插件仓库先维护六份 canonical definition、六个脱敏 CapturePackage fixture 和 fixture 预期 package hash；这些文件是跨仓来源真值。
2. 工作台注册同一六份 definition，必须由现有 `canonicalJson`/`computeContractHash` 计算各 contract hash；不得手写或信任客户端 hash。
3. 跨仓检查必须逐合同比较 canonical definition JSON、contract hash、fixture header 的 `(contractId, contractVersion, contractHash)` 与 fixture package hash；任何一项不同即失败。
4. B3-CONTRACT-SRC-001/B3-MEDIA-SRC-001 在六个 CollectionContract 之外增加两个同样跨仓 canonical 比较的来源合同：`xhs.record-payload/v2` 与 `xhs.media-inventory/v2`。前者固定 note/author 双身份相等与 note `type=normal|video`；后者首期只接受同包 note subject，稳定 slot 精确为 `subjectKey:purpose:kind:ordinal`，同时固定 observedAddress 与独立 cover provenance。comment media、author/avatar 在来源闭环前拒绝。fixture、真实 terminal mapper、工作台 validator/B2 任一侧漂移均须使跨仓检查失败。
5. fixture 使用结构完整的 `execution` CaptureSubmissionV2，但只使用脱敏占位的 job/attempt/station/lease 值；不得实例化 EvidenceIngress、不得请求 API 或数据库。
6. V1 `failed/stopped/cancelled` 到 V2 terminal 的逐原因映射、authority 绑定、captureId/targetKey 生成及实际发包只属于后续 adapter 工单；B1-B-03-R1 不实现它们。

## 4. 禁止项

- 不接 API、CLI、scheduler、插件 outbox 或现役 V1 `commit_raw_snapshot`。
- 不修改 Prisma、migration、Outbox、Projection、Canonical、Media 或九工位版本。
- 不创建第二 registry、数据库合同表、fallback、双读、双写或兼容路径。
