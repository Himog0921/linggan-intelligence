# #454 手动云端 OCR 人工采用：实施与验收

> 状态: 历史归档
> 最后核对: 2026-10-23
> 适用范围: 叠加 Draft PR #477 的 #454 收口包；只处理已保存 `cloud_manual` OCR 的人工采用
> 事实来源: Mog 继续收口授权、Issue #454 Claim、当前 OCR recognition/review/projection 实现
> 冲突时以谁为准: Mog 最新确认、现行数据合同与真实原图/回执；本计划不授予外发、迁移、部署或 G4 通过


本文保留 #479 的实现与发布设计历史。当前实现由 [research-dossier.v2](../../data-contracts/research-dossier.md) 取代：人工采用统一使用 material_reading 命令，旧 overlay、独立 API 和表单已退役。下文旧迁移/端点步骤不能用于当前候选发布；0145 历史迁移保持原样，并在 canonical 0146 之前执行。

## 目标

补齐已保存云端手动 OCR 的可审计人工采用写入口，使“机器预览不入材料”具备可由人逐图完成的闭环。候选以 PR #477 的 `5d5d0dfd` 为基线；在基础 PR 未合并前保持 Draft，不合并、不部署。

## 闭集与非目标

- 新增窄服务、受控本机 API、最小页面表单、追加式迁移和合成 PostgreSQL/浏览器证明。
- 复用现有人工覆盖读取投影，但不重用抽检 sample/issue 写入口。
- 不发送图片或文本给模型，不启用 worker/任务，不重分层、不改 raw JSON、不创建本地 OCR 行、不修改平台标题。
- 不写共享数据库、不运行真实采用、不申请图片外发、不合并/部署，不把本候选称作 #420/G4 或业务验收。

## 设计地图

| 层 | 最小改动 | 不可越界 |
|---|---|---|
| 数据 | `linggan_media_ocr_review.sample_ref` 对手动采用可为空，并以约束保留 `issue_ref` 的 sample 依赖 | 不回填、更新或删除历史审阅；不伪造 sample |
| 服务 | 查询当前 work 的 cloud-manual item/adoption、版本锁、写 immutable command/review receipt | 不接受 URL、图片、provider 结果或任意 adoption ref |
| 读取 | cloud 读取复用现有 review overlay，把已确认人工正文投影为 `human_ocr_review` | 未采用 cloud preview 仍不可进干净材料 |
| API | `POST /api/local/work-resources/{work}/ocr-recognition/adoptions` | guard、work 范围、版本和来源均在服务端重核 |
| UI | 已保存 OCR 结果中的“人工核对并采用”表单，提交后刷新同一 run | 不替代抽检 UI；不乐观成功；不新增导航 |

## 状态字典

| 状态 | 页面表达 | 禁止暗示 |
|---|---|---|
| `cloud_vision_unverified` | 模型识别，未人工核验 | 可作为材料或已被采用 |
| 可写 | 已保存结果，可对照原图后填写采用 | 机器文字是真值 |
| 提交中 | 正在保存本次人工采用 | 已保存或已进入材料 |
| 回执未知 | 正在核对同一请求 | 可新建第二版 |
| 版本冲突/来源受限 | 当前版本或来源已变化，重新读取 | 自动覆盖旧版本 |
| `human_ocr_review` | 人工采用已由后端回执确认 | 平台标题或原始 OCR 已改写 |

## 验收矩阵

| 层 | 必测 | 不由本包证明 |
|---|---|---|
| Rust/PG | cloud-only 资格、跨 work 拒绝、revision 冲突、append-only、idempotency、block/manual provenance、withdraw/cutoff | 真实原图质量 |
| API | 无数据库、非法 body、受限/错误来源、成功 receipt 和重放 | 共享运行状态 |
| React | 表单只在保存结果出现；采用/未知/失败文案；同一 command 重试；成功后重读 | 真实人员判断 |
| 受控数据链路 | 自有临时 PG 的已保存 cloud 结果、人工采用与 dossier 读回 | 浏览器交互、生产、迁移、Mog/G4 |

## 停止条件

若现有不可变 review 表无法把“非抽检人工采用”与 audit 统计永久区分，或读取投影会让未采用预览进材料，则停止实现并记录 `DECISION_REQUIRED`。基础 PR #477 失败或出现非叠加冲突时，也停止此包的远程交付。
