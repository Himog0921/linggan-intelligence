# STANDARD-DOSSIER-001 实施、系统测试与问题报告

> 状态: 一次性报告
> 候选阶段: 自测通过，独立审查待开始
> 最后核对: 2026-10-25（Asia/Shanghai）
> 适用范围: 统一研究底稿实施包；候选不等于主线或3000部署
> 事实来源: Mog 当前指令、实施手册、实际源码与本包隔离运行回执
> 冲突时以谁为准: 用户最新确认、真实运行/测试；产品字段以唯一底稿合同为准

## 基线与授权

base / origin/main / 3000 起始均为 `df6dde9e87a6b72c4eeba5b84bd29043ae61d405`。API PID64924，cwd/executable 位于 canonical runtime-main，health ready；patrol/media/ai-executor disabled。共享checkout停留旧分支且210项既有修改，未改动。本包工作树 `.worktrees/standard-dossier-001`，分支 `codex/standard-dossier-001`。Claim及包内所有权见当月progress。

实施授权包含包内测试、一次独立审查及问题修复；新的模型外发单独确认。没有针对本包最终head的合并/部署授权，候选完成后一次提交确认。生产不回填、不迁移、不启用worker；所有PG写入在自有一次性容器/数据库。媒体验证用合成视频和保存结果回放，未将合成文字冒称真实识别输出。

## 实施结果

唯一 `research-dossier.v2` 增加 readingView 和 titleBlockRefs；displayTitle 优先平台，否则仅明确封面/已知第一页标题块。主文/补充、Markdown、显式selection及内部fragments共享排序；文字只在materials.blocks中公开一次。无文字仍有底稿和原件。GET只读，已有平台原文缺持久读本时提供带真实material来源的只读块；native准备仍可补持久读本，重复准备幂等。

离线输入移除整篇ready拒绝，保留每列真实状态和来源检查；部分列不会抹除已有合格块。评论上下文标题改用底稿；作者、主题、工作台复用fragments的共同来源，末端标题同步。日常local视觉任务可自动发布；显式云预览继续candidate，历史记录不批量自动采用。ASR无音轨/无语音/失败分开，不把诊断占位写成正文；失败保留已有同源读本。processing状态按asOf读事件。

唯一GET支持segmentSize/segmentOffset/asOf/expectedRevision。实际HTTP连续遍历463个块，并在首段后新增正文，后续仍读取冻结版本；未固定续读、错误revision、越界均拒绝。下游研究多轮推理未改造：现有消费者容量检查与明确容量错误保留，不能静默取前N字。

前端复用LIDS、Inspector与原修正命令。作品、研究底稿、采集档案共享后端顺序；页头统一标题，原件/修正/导出可达，诊断折叠。截图与日志固定在本工作树 `artifacts/private/standard-dossier-001/`（不进Git）。

## 系统验证

| 场景 | 实际范围与证据 | 状态 |
|---|---|---|
| T01–T03 | reading.rs参数化八组合、短句/表情/标签正文、封面分行、未知页序 | 纯组装通过 |
| T04 | standard_partial_results：五图三完成、一失败、一未取得；真实共享输入保留6块 | 隔离PG通过 |
| T05–T06 | selection重编排；来源更新与固定历史；封面/第一页字节去重、真实后页重复及人工差异 | 组装与既有隔离PG通过 |
| T07 | 视频口播/画面分责与时间；无音轨/无语音/失败及同源失败保留旧文字新用例 | 隔离PG通过 |
| T08 | 空文字原件、底稿存在；离线空文本明确错误 | 浏览器通过，离线已通过 |
| T09 | 日常机器发布、显式云预览、人工优先、CAS/请求幂等 | 既有PG通过，local daily worker新增已通过 |
| T10 | native原文与本地layout准备两次，第二次不复制、不新识别 | 隔离PG通过 |
| T11 | 研究期间更新后精确历史不变、两图/区块选择不夹未选材料 | 组装/PG通过 |
| T12 | 同一真实HTTP入口按37块遍历463块，新正文未混入旧asOf | 5个HTTP测试通过 |
| T13 | 真实共享输入和末端适配检查；作者/主题fragments、评论context_sources、工作台、离线输入 | 已通过 |
| T14 | React→Axum→一次性PG；刷新/切换/原件/采用/结构化编辑/保留当前/导出；1440、1280、390 | 实际浏览器及视觉增量通过 |

最终原生100%浏览器证据目录为 `browser-frontend-focused/`。22次宽度/指标几何检查、三次真实修正命令通过，pageErrors/consoleErrors/blockedRequests均为空；新增聚焦图显示视频补充画面、失败材料及实际加载原图，内容避开sticky页头。

原生100%浏览器截图五组：`standard-dossier-1-*`无正文多图，2有正文与图片补充，3视频，4部分失败，5无文字原件。每组1440与1280；fixture.json/report.json记录随机候选URL、作品/域和数据来源。合成回放不证明真实OCR/ASR质量；本机ffmpeg实际生成MP4并提取WAV和首帧，ffprobe证据为local-media-proof.json。当前PATH无whisper命令，未运行真实ASR；不使用mock声称通过。

## 问题与修复

| 场景 | 现象/影响 | 根因 | 修复 | 验证 | 仍存在 |
|---|---|---|---|---|---|
| 无平台标题的列表/检查器 | 列表请求panic，底稿无法从实际页面打开 | 原末端强制取layout_ref，canonical阅读来源不一定有layout | 取可空layout来源并保留unit/reading引用 | 首次browser-pg.log失败，browser-pg-fixed.log整链通过 | 已修复 |
| ASR无输出 | 诊断占位语进入“正文” | complete_without_output写gap假文字 | 空blocks+实际状态，无语音与无音轨分开 | 真实PG边界通过 | 已修复 |
| 部分可读 | 离线入口按整篇ready/列状态拒绝已有块 | 完整性与可读性混用 | 保留coverage，使用已有合格fragment | 纯函数与真实共享输入通过 | 已修复 |
| 1280列表/普通机器阅读 | 未知指标折行截断；未知角色标语反复打断正文 | flex-shrink及共用块默认角色标签 | 前端定向修正 | 第一轮截图已保留 | 已修复 |

初始测试中旧“缺持久读本则正文不可读”断言按已确认目标更新；离线列聚合限制不再连坐合格块。默认系统runner的旧评论筛选名已改为实际测试名，零测试guard保留；全新工作树AI证明缺证据父目录已补mkdir。初次OCR audit缺少固定SDK依赖导致probe未发送，按lock安装后实际loopback验证通过，不改真实供应商合同。

精确来源改变仍改变作者输入指纹，不把相同文字当成同一来源。编译/断言失败日志保留，不计作通过。

既有识别质量（竖排、多栏、手写等）不在本包更换引擎处理；本包保存来源/局部问题，不声称文字准确。生产历史整理范围尚未执行：只对已有平台原文及合格本地layout准备，显式云预览排除；最终发布时需按最终候选和确认范围办理。

## 独立审查与版本

自测通过：Evidence纯函数133通过/2忽略；Intelligence lib357通过/5忽略与离线输入21通过；完整Web116文件/1270测试（随后无音轨/无语音增量20测试）及类型/lint/生产构建通过；canonical-inputs、all-shared-consumers、daily-worker、ocr-audit-retest、saved-topic-inputs-retest全部通过，具体命令及逐项结果见私有日志。预检通过。完整Web之后的UI状态增量由定向测试和最终浏览器构建验证。

自测完成后交一个独立只读代理审查具体提交，修成立问题，仅复核受影响增量。最终提交、PR、实际CI结果将补在本节。主线与3000未由本包改动，Mog前端业务验收待定。
