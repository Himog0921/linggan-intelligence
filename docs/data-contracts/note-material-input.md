# 每篇解读材料输入合同

> 状态: 代码事实优先
> 最后核对: 2026-10-19
> 适用范围: AI-GEN1 / PR-1A，统一底稿之上的新材料适配；不切换旧研究消费者
> 事实来源: Mog 已确认设计、#454 Claim、既有 research-dossier.v1 与本 PR 实现/隔离证明
> 冲突时以谁为准: Mog 最新要求、真实来源与类型化执行合同；未知不得伪装成空或已完成

## 输入、版本与引用

`note-material-input.v1` 读取同一 domain/cutoff 内的统一底稿及已接纳标签投影，标题、封面主文案、正文、标签、逐页图片正文、转写各列状态与原因。旧 `research-dossier.v1` assemble 和三类消费者不增加标签或换指纹。评论仍走原受控通道，视频帧文字标未接入，不充当作者正文。

文字仍来自原不可变材料。短编号绑定原始 sourceRef/sourceVersion、栏目、逻辑槽位和 Unicode 字符范围。NFC/换行规范化只用于哈希，不修改来源字节或引用偏移；正文变化、顺序变化、实际输入栏目状态变化改变内容身份，点赞、时间、随机引用与重复观察不改变。

内容指纹与来源 manifest 分开。持久快照绑定 domain、固定 as_of、内容指纹、来源摘要及就绪/状态摘要；同内容换来源不覆写旧快照，同一固定输入重复保存幂等。快照/片段只保存引用、哈希、状态和范围，不复制正文。当前受限检查始终优先于历史 cutoff；笔记级全链失效与解读 serving 由 PR-1C 接续，不把本 PR 的媒体过滤称为已完成全部失效传播。

## 标签

从原包对应 detail record 的独立 `hashtags` 数组投影，绑定已有 material_ref；每个版本有完成回执。字段缺失、提供空数组、有效数组、无效值独立表达；空行不代表投影已完成，缺失不能断言平台没有标签。顺序保留，不混 keywords/topicIds。新详情与完成回执/标签同事务；旧详情窄补投影不重新插入 detail、不改包/接纳/其他 lane。

历史 plan 默认只读；apply 只限显式 domain/作品/冻结截止点及当前 revision，通过已有详情映射回原包。已投影 detail 也会补标签。无效值保留失败/原因，不静默当空；不在普通操作回执输出标签原文。

## 就绪

详情存在，或从该 domain 首次合格入域事实 `min(material_domain_usage.created_at)` 起达到 24h，且相关非上下文图片/视频无 pending/running/retry_wait/leased 等尚待处理。入域时间限定 cutoff，重复观察不重置。宽限只解除等详情，不解除媒体等待。

FAILED 事件要结合 processing_work 判断：retry_wait/leased 是等待，terminal 才终态失败。无 work、未启用和状态不足保留等待/未知。可读旧结果不隐藏新待处理；无文字/语音与终态失败是有依据的缺栏。未按当前 rules-v3 分层标需重新分层，不等于缺 OCR。上下文/头像/评论图片和未采用处理不混入材料，按声明范围排除。就绪可以材料不足，不等于完整、相关或可以发模型。

## 显式操作与范围

CLI 提供只读 plan，冻结 domain、as_of、最多 200 个无重复作品、版本及每篇预期 revision；显式 apply 读取该 plan 执行一个操作：标签补投影、已有 OCR 重新分层、材料快照保存。逐篇事务与结果，失败/过期/需 OCR/残留/未尝试可区分。重新运行幂等，重新读计划后才能处理变更输入。

重分层调用既有 `relayer_work`（每篇最多 100 layout），只复用 OCR 行/坐标/置信度追加规则结果，不识字、不转写、不建采集/媒体作业、不调用模型、不启动后台服务。旧层和历史研究输入不改写。作品与 layout 分母分开；部分可处理而仍缺 OCR 不能报整篇全部完成。

读取/预览不保存快照或发模型。local API 沿原本机 guard，domain 必填并验证作品用途；写操作不向浏览器增加隐式启动入口。默认日常读取不运行批量操作。

## 证明与未证

验证必须覆盖标签新旧路径、实际状态、24h 边界、身份稳定/变化、来源版本、历史 cutoff/当前限制、快照幂等/不可变、有界 plan/apply、旧消费者兼容和副作用表。证明只用一次性 PostgreSQL 与合成材料，不读取共享 DSN/真实材料或真实供应商。

本 PR 不证明真实材料覆盖、v3 全库质量、真实模型效果、部署或 #420/G4。具体检查结果写 [统一计划](../plans/active/ai-gen1-001.md) 和 PR；不能预填通过。

## 操作接口（候选）

只读 GET：`/api/local/work-resources/{work}/research-dossier/note-material?domain={ADHD-domain}`，可选 `as_of`；未来时间拒绝，越域/非 ADHD 为 404，缺 domain 为 400。读取无持久写入。`processingScope` 明示 image_ocr/asr、上下文/中间产物和未接入的 video_frame_ocr；正文、ASR 和封面不混写。

显式 CLI 用法（仅说明，不是本次对共享库运行的授权）：

```text
linggan-note-material plan --domain UUID --works UUID,UUID --operation hashtags
linggan-note-material plan --domain UUID --works UUID --operation relayer --as-of TIMESTAMP
linggan-note-material plan --domain UUID --works UUID --operation snapshot
linggan-note-material apply --plan /absolute/path/to/saved-plan.json
```

apply计划文件有界16MiB，覆盖最大200×500详情引用的合法pretty JSON；超限拒绝。snapshot apply除旧revision外，同事务比对saved plan的materialFingerprint，拒绝mutable work造成的冻结输入漂移。

连接仅从操作者显式设置的 `LINGGAN_LOCAL_DATABASE_URL` 取得，不自动读环境文件。plan 输出重定向由操作者完成，保存后 apply；plan 哈希用于校验合同/内容完整性，不是权限凭证。每次仅一种操作，最多 200 篇、每篇标签详情最多 500，超过就拒绝而不静默截断。标签、重分层完成后若要保存新结果，应生成新的 snapshot plan；旧 cutoff 不自动前移。

回执的作品主结果 updated/already_current/partial/failed 互斥；另列 layoutUpdated/layoutAlreadyCurrent/layoutSkipped/needsOcr。CLI 对 partial/failed 返回非零。中断只影响当前篇事务，已提交篇可幂等续跑；旧 revision 仅在不可变来源摘要相同、全部布局已当前时可走只读重分层重放，原文/来源/资格变化仍冲突。缺 OCR 单列，不自动创建作业。

内容哈希仅做 NFC 和 CRLF/CR→LF，保留其余空白。栏目顺序固定、标签按原始序号（含重复），媒体按稳定语义槽位；随机 source/job/layout/blob、处理时间和互动数不进内容指纹。来源 hash 与原字符范围另记；同一语义槽位、同规范化文字去重，不因数据库行返回顺序改变引用编号。原始字符范围按 Unicode scalar 计，不按 UTF-8 字节或 JS UTF-16 长度。
