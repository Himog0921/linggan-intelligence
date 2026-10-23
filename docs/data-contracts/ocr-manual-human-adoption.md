# 手动云端 OCR 的人工采用合同

> 状态: 历史归档
> 最后核对: 2026-10-23
> 适用范围: #454 已保存 `cloud_manual` OCR 结果的人工核对、追加版本与受控读取
> 事实来源: `OCR-CONTENT-LAYERING-001`、`note-material-input.v1`、现有 `linggan_media_ocr_review` 投影和 Mog 继续收口授权
> 冲突时以谁为准: Mog 最新明确确认、原图/原始结果/当前权限、现行迁移与运行时校验


本文保留 #479 的实现与发布设计历史。当前实现由 [research-dossier.v2](research-dossier.md) 取代：人工采用统一使用 material_reading 命令，旧 overlay、独立 API 和表单已退役。下文旧迁移/端点步骤不能用于当前候选发布；0145 历史迁移保持原样，并在 canonical 0146 之前执行。

## 目的与非目标

本合同只为已经保存、仍可读、且来自 `cloud_manual` 的单图 OCR 结果提供人工采用入口。它使审核人在对照原图后把一段完整正文追加为新版本；原始 provider JSON、机器预览、平台标题、既有本地 OCR 和旧人工版本均不改写。

它不是质量抽检样本，不创建 `linggan_ocr_audit_sample` 或 `linggan_ocr_audit_issue`，不产生模型调用、图片外发、采集、重分层或媒体工作。不能由保存成功、页面显示、HTTP 200 或测试替代 Mog 的 G4 原图业务验收。

## 写入资格与命令

写命令只接受工作范围内的 `linggan_ocr_recognition_item.adoption_ref`，并在同一事务中核对：

- adoption 的 layout 引擎为 `cloud_manual`，规则版本为 `cloud-ocr-adoption.v1`；
- 该 item 属于请求中的 work，且图片/materialization/blob 仍未被 disposition、撤回或退休屏蔽；
- 目标字段只允许 `adopted_body`；
- 调用者提交当前 adoption 的 `expectedRevision`，并且 command identity、actor 与 reason 有效。

命令身份是 `linggan_ai_command` 的 `manual_ocr_human_adoption` 操作。相同 command ref 与相同 payload 只返回原回执；相同 command ref 指向不同 payload、版本已前移、越作品或来源失效均拒绝，绝不静默重放或改写。

新增追加记录允许 `sample_ref = NULL`，并以 `issue_ref = NULL` 明示其不属于抽检。已有抽检记录保持 sample/issue 外键和不可变性；报表继续按 sample 读取，不能把人工采用计为抽检质量分子或分母。

## 文字、来源与投影

`adopt` 必须携带所选字段的完整新文本；`reject` 和 `pending` 不替换有效采用文本。若人工输入恰好唯一对应保存 cloud region 的完整 Unicode 范围，则记录 `ocr_block`、region ref、半开字符范围和原 bbox；否则记录 `manual_transcription`，bbox 为 null，不能捏造本地 OCR 行或字符坐标。

读取层只选择 cutoff 之前每字段最高 revision 的 `adopt`。`adopted_body` 将该图 `decisionSource` 标成 `human_ocr_review`，保留原机器预览为 `machineResearchText`，并把人工来源片段带入 note-material 投影；它绝不覆盖平台标题。没有人工采用的 cloud preview 继续是 `cloud_vision_unverified`、`cleanCorpusEligible=false`。

撤回、清理、退休和 cutoff 先于读取；来源失效时既不显示旧人工文字，也不接受新命令。历史派生缓存不批量重算。

## 页面与证明边界

入口只位于当前作品的已保存 OCR 结果内，先让用户打开受控原图，再填写完整新版本、审核人和依据。提交中复用同一 command ref；网络结果未知时只允许“核对同一采用请求”，并在服务端回执后刷新读取，不能乐观把机器预览渲染成已采用文本。

本候选只以合成图、loopback API 与自有临时 PostgreSQL 证明资格、追加性、幂等/冲突、cutoff/撤回和页面状态。未证明真实图片对照、共享库迁移、运行时部署、真实材料是否应采用，或 Mog/G4 业务验收。
