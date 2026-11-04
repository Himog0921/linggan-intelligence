# 切片 A 冻结版本保存与完整链路核对

> 状态: 一次性报告
> 最后核对: 2026-11-04
> 适用范围: 本地切片 A 代码、冻结内容、已有工程/模型证据及下一步安排
> 事实来源: 基底 f34dc08a 上的 codex/ai-midterm-quality-002 工作树、源码、隔离测试和私有试跑回执
> 冲突时以谁为准: 当前源码与可复现证据；本报告不授权模型外发、正式写入或部署

## 当前结论

单篇发起、排队、调用适配、校验保存和前端读取的代码已经接通，已有隔离PostgreSQL和模拟供应商的完整工程证明，也有桌面实际操作记录。真实DeepSeek试跑走的是私有直接API工具，没有经过产品队列和业务结果表。

因此当前缺的是“最终冻结版本、真实来源输入、真实供应商、同一次持久化结果、浏览器读取”的联合验收，不应将两组独立成功证据拼成已完成的真实产品闭环。分析方法已冻结，本轮不再优化提示词。

冻结内容：[note-analysis-baseline-20261104](../data-contracts/ai-midterm-quality-002/frozen-baseline.md)。本报告随切片A本地候选提交保存，不表示合并、发布或上线。

## 链路与证据

| 环节 | 实现 | 当前证据 | 尚缺 |
|---|---|---|---|
| 单篇发起 | ReadingPanel → NoteAnalysisPanel → preview/start接口，冻结范围与命令身份 | 桌面按钮、四接口与重放/409回归 | 在最终冻结配置下重验 |
| 输入组装 | note_analysis/source.rs从research_dossier与评论读取，保存input_manifest | 原有真实PG夹具；直连试跑另有完整来源映射 | 产品packet补齐逐citation来源语义，避免与直连输入不一致 |
| 模型执行 | queue允许NoteInterpretation，executor派发，research_call准备Pi请求，dispatch调用 | 实际Pi adapter＋自有loopback；真实DeepSeek仅直连 | 真实模型通过同一产品任务执行一次 |
| 校验保存 | accept后写request/result，invocation与item结算；失败记录和旧成功结果保留 | 隔离PG正反路径；本地9项合同测试 | 真实调用账与业务结果关联核对 |
| 读取展示 | read/latest返回最新运行、最近可读成功、新鲜度；前端轮询、刷新、展开证据和标签 | 真实桌面交互，但结果为模拟或人工回放 | 浏览器读取同一次真实结果、刷新后仍为同一requestRef |
| 冻结配置 | 仓库已冻结Skill/Prompt/Schema/validator | 文件快照与哈希 | 导入并绑定实际数据库版本，核对实际发出的内容哈希 |

## 两项已定位的实现/配置差距

### 1. 产品输入丢失逐条来源语义

`crates/intelligence/src/ai_foundation/note_analysis/source.rs::load` 使用fragment.field筛选标题、正文、OCR、ASR，但Source只保留material/comment及引用身份，coverage没有逐citation的sourceOrigins和时间映射。私有真实试跑输入包含来源类别、识读状态和转写时间。

来源身份仍可追溯，但模型不能仅凭不透明ID可靠区分正文和OCR、识读状态或时间。因此此前对来源理解的直连表现不能直接外推到产品输入。

最小实现安排：沿用Packet.coverage，按canonical dossier现有字段建立逐citation来源映射，保留可取得的识读/时间信息，未知照实保留；不新增分析层、不重新OCR、不重采集，不改冻结方法。以真实dossier结构复现N03的OCR/ASR和N05正文替字，不能再全部拼入body载体。

### 2. 仓库文件不等于任务已安装版本

0191迁移种子安装提示词，但继承旧配置的skill_version_ref；`ai_foundation/prompt.rs::frozen_in` 读取数据库Skill/Prompt版本，不直接读取仓库Markdown。旧PG/浏览器夹具有import_skill及switch_skill步骤，但后续冻结发生在这些证据之后。

本轮未查询或修改实际数据库绑定，不能断言某个环境当前一定使用旧版或最终版。下一步在隔离环境使用既有导入、绑定机制安装已冻结内容，读取任务实际配置并比较内容哈希；不新建一套配置系统、不改正式设置。

## 下一步实施和验收顺序

1. **先补输入来源映射。** 只改source组装与针对性测试；核对实际模型packet内每条可见材料的来源及原句，保留评论角色和父级关系、未知值、排除缺口。验收单位是完整的单篇输入包，不只是映射条数。
2. **在隔离环境安装冻结版本。** 按现有机制导入Skill、绑定Prompt和模型配置；核对任务版本引用与内容哈希，覆盖材料、评论权限、输出额度与预算。验收以实际准备调用的请求为准，不能只看设置页。
3. **同一候选先跑本地模拟闭环。** 从桌面单篇入口发起，实际API/队列/PG保存/刷新；确认requestRef、itemRef、invocationRef与result一一对应，来源定位正确。复用既有失败保留、未知提交重放、409和跨作品隔离用例；不创建新的产品状态。
4. **另行授权后只做一次真实产品闭环。** 建议N03一篇、一次、不自动重试，先提供当时的实际输入、模型参数和预算；从浏览器发起，经真实adapter完成调用与保存，刷新读取同一结果。不能将私有直连结果手工塞入表后算通过。
5. **以使用任务收口。** 现场查看完整内容理解、解释、标签、方法及引句。技术失败按事实报告并停止，不因此重开冻结提示词优化；已知语义局限保留。N01/N07未实测，整体质量分布未知，不以N03一篇代表全体。

第4步真实调用、正式配置、迁移、部署、归并与全量均没有在本轮执行或由本报告自动授权。当前用户要求的是保存与核对安排。

## 完整闭环验收记录

应记录同一次动作的：输入范围和hash、实际Skill/Prompt内容hash、commandRef/requestRef/batchRef/itemRef/invocationRef、供应商响应状态与usage、业务表成功或失败记录、GET读取、刷新后同一结果、桌面展开原文依据与标签的操作。技术接收、内容可用与用户接受分别记录，冻结版本不等于承诺模型每次零错误。

本轮未运行新的PG/浏览器/供应商任务；旧工程证明来自隔离模拟，真实模型证明来自直连，二者边界保持。私有历史入口为 `slice-a-design/execution-proof/`、`browser-proof/`、`real-trial-*`；本轮提交与检查回执放在 `slice-a-design/save-and-chain-audit-20261104/`，不进入Git。
