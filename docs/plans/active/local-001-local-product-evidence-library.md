# LOCAL-001 本地 Linggan 产品、Evidence Library V7 与插件 Local 接缝

> 状态: 活跃计划
> 最后核对: 2026-08-24
> 适用范围: Linggan 独立本地 Web 产品的第一个真实页面及其最小本地采集接缝
> 事实来源: Mog 于 2026-08-24 的产品确认、`AGENTS.md`、领域不变量、UI 执行合同、LIDS 与 Issue #23
> 冲突时以谁为准: 用户最新确认、`AGENTS.md`、真实代码/运行/合同、ACCEPTED 决定；本计划不扩张现有 SCOPE-001

## 目标与用户可见结果

Linggan 不再经由旧内容工作台运行。它是独立的本地 Web 产品；浏览器插件以 **Local Linggan 模式**连接 `http://localhost:3000`。用户第一个真实页面是 **Evidence Library**：在同一个地方找到、看懂、验证、追溯并继续研究本地 Linggan 已接纳的材料。

用户给出的 V7 handoff 是此页的产品与视觉 **Gold Master**。这表示后续页面实施必须以 V7 的信息架构、布局、交互骨架和视觉验收为准绳；它不表示 V7 中的模拟数据、历史指令、技术栈、数据权限或运行状态可以替代 Linggan 的现行事实规则。

## 已确认边界

```text
真实平台页面
        ↓
原插件的 Local Linggan 模式
        ↓  仅 http://localhost:3000
独立 Linggan 本地 host / API / 数据库
        ↓
Evidence Library V7（真实状态与本地读投影）
```

旧内容工作台只可提供能力继承审计、代码阅读和历史踩坑参考。它不是此链路的运行时服务、数据库、验证中转、默认回退或数据来源。

本计划与 SCOPE-001 并列，而非覆盖它：SCOPE-001 仍只证明合成事实内核；任何 Local-001 实现均须明确它究竟复用了哪些已证明接口，哪些仍未证明。不得把本地页面出现、HTTP 成功或插件任务结束写成 Evidence、Observation、真实平台或市场洞察已经成立。

## 页面级设计裁定：V7 与 LIDS

`REF-V7-001` 已在设计参考登记表中登记。它是 **仅限 `/corpus/evidence` 的页面 Gold Master**，不是第二套全局设计系统。

| V7 可冻结的本页内容 | 仍由 Linggan 权威规则决定 |
|---|---|
| Evidence Library 的核心任务：找到 → 看懂 → 验证 → 追溯 → 继续研究 | Evidence、Observation、Capture、Coverage、Unknown、权限和隐私的语义 |
| L1 Corpus Explorer + 右侧嵌入 L2 Inspector 的结构、导航层级、三种浏览视图与状态版式 | 实际路由、后端合同、当前数据是否存在、何时可展示原文或媒体 |
| V7 的视觉布局、硬边界、连续列表、Inspector、状态不可压成总质量分的表达意图 | LIDS 的 Token → Primitive → Component → Pattern → Page、可访问性与全站一致性 |

**页面级例外 `LOCAL-001-UI-EX-01`：** V7 的冻结视觉是本页回归基准；但其 `v7-frozen-visual-tokens.css` 不得直接成为第二套全局 token 真源。实施时只能把每个必要视觉值映射至既有 `--lgi-*` Token，或将无对应项明确登记为本页局部、可撤销例外；不得复制一套并行全局 token。

## 三张有序实施卡

### 001A — 本地实际 host + V7 页面真实状态

**用户结果**：访问 `http://localhost:3000/corpus/evidence` 时，看到的是真实 Linggan 本地页面及真实运行状态，而不是旧内容工作台、静态 handoff 或伪造的“已采集”数字。

**范围**：

- 明确本地 host、页面路由、API 责任、数据库读取边界和启动方式；仅绑定 loopback；
- 按 V7 Gold Master 实现页面骨架与其 `normal / empty / loading / partial / unknown / failed / stale` 等状态的诚实表达；
- 无可读本地数据时显示 `empty`、`unknown`、`partial` 或 `failed` 的真实原因，绝不复用 V7 示例内容填充页面；
- 形成该页实施所需的 Page Spec、UI Change Manifest 和 Visual Acceptance 记录，遵循 LIDS。

**明确禁止**：

- 不通过旧内容工作台、其数据库、线上 API 或历史数据获得页面内容；
- 不把前端本地假数据、toast、加载完成或 HTTP 200 说成已接纳 Evidence；
- 不新增 Topic、Signal、Insight、研究发起、保存视图等会产生业务副作用的能力；
- 不在本卡接入真实平台、插件、媒体、OCR、ASR、评论深采或 AI Agent。

**成功条件**：

1. 端口 `3000` 的实际所有者、启动/停止方法、loopback 绑定和页面/API 分责可复现且写入 runbook；
2. `/corpus/evidence` 按 V7 Gold Master 通过视觉与键盘/状态走查，且所有显示值来自明确的本地 read contract 或明确的无数据状态；
3. 页面不把 Unknown、未观察、部分完成、失败、陈旧、无记录压成 `0`、`正常` 或统一成功；
4. 无旧工作台运行依赖、无真实平台请求、无真实数据副作用。

**验证阶梯**：单元/合同 → 本地 host 路由 → 浏览器实际页面与每个状态 → LIDS/V7 视觉回归 → 治理检查。页面截图只能证明表达，不证明采集、数据库或市场事实。

**停止条件**：若现有 Rust/API 无法在不扩张 SCOPE-001 的前提下承接真实状态，先停在产品 host/页面合同并提出下一张最小架构卡；不得临时引入旧工作台、任意 mock 数据或未审计前端技术栈。

### 001B — Evidence Library 本地只读投影

**用户结果**：当 Linggan 本地确实有已接纳材料时，Evidence Library 让用户以 `ContentItem` 为一级浏览单位、以 `EvidenceFragment` 为搜索命中单位，查看其来源、观察、Capture 与 Coverage 边界，并能回到所选材料的上下文。

**范围**：

- 建立仅供 `/corpus/evidence` 读取的受控 read projection / API 合同；
- 依照 V7 的 Research、Table、Cover 三种视图，展示同一来源集合的不同呈现，不复制第二事实源；
- 显示 `ContentItem / EvidenceFragment / Observation / CaptureRun / Comment` 的责任边界，以及 source、observed time、接纳/处理水位、Coverage 与限制；
- 仅显示当前用途被允许的最小必要片段；无真实本地材料时仍回到 001A 的诚实状态。

**明确禁止**：

- 不导入旧内容工作台库、不迁移历史评论、不以旧数据充当 Linggan Evidence；
- 不直接读数据库、不返回未受控原文、不将全文、私信、咨询或敏感儿童/医疗材料默认暴露到 UI；
- 不从局部列表、搜索命中或单条评论推导需求、趋势、代表性、Topic 或市场机会；
- 不接入媒体下载、OCR、ASR、视频帧、embedding、聚类或 AI 总结。

**成功条件**：

1. Read contract 能明确回答每个字段来自哪个 Observation / Record / Package，及何种状态尚未知；
2. 搜索命中片段不脱离所属 ContentItem 的上下文；对象去重不抹掉 Discovery/Capture 来源；
3. 任何部分结果均同时显示实际取得、目标语义、Coverage gap/reason 和用途限制；不凭 `target - emitted` 伪造平台剩余或完成率；
4. UI、API 与数据库投影不形成第二事实源，且没有敏感原文泄漏。

**验证阶梯**：read-contract 负例 → 本地数据库/API integration proof → 页面三视图与 Inspector → partial/unknown/privacy boundary walk-through → no-old-workbench dependency audit。

**停止条件**：若真实 Capture/Evidence 的保存、脱敏、访问范围或来源血缘尚不能由现有合同支持，001B 只能交付无数据/受限状态和合同测试；不得为“看见内容”而旁路接入旧系统或写入真实原文。

### 001C — 首批媒体 Canary 的三段连续路线

首批 Canary 已按用户确认纳入媒体；它不是“先做文本，以后再考虑媒体”。但媒体的发现、字节取得与语义处理是三种不同责任，不能让搜索页 discovery 假装完成它们。001C 因此只是一张父卡，必须按下列三个子卡顺序推进并分别证明。

#### 001C-1 — 插件 Local 模式 discovery ingress + ADHD 前 20 条发现面

**用户结果**：用户在本机使用原插件执行一次受控的 `ADHD` 搜索结果页 discovery；插件只向 Linggan local host 交付页面实际可见的前 20 个候选卡片及完整 Coverage 回执。随后 Evidence Library 只在数据已按 Local-001 合同接纳后显示它们。

**范围**：

- 核对原插件的实际 source、浏览器加载的 `dist`、版本与当前 host 合同，新增一个显式、可见、可撤回的 **Local Linggan mode**；
- 固定本次 query=`ADHD`、入口为搜索结果页、上限/目标语义为“本次观察到的前 20 个候选位置”；仅提交页面已实际看到的候选卡片字段与 Capture/Discovery/Coverage；
- 本地服务只接收符合已冻结 discovery ingress 合同的内容；失败、部分、未知、限流或未尝试必须原样回执；
- 建立从页面发现 → Local ingress → 本地 Evidence/Discovery → Evidence Library 的真实链路验证。

**明确禁止**：

- 不打开每条详情，不抓正文、评论、回复、作者历史、图片/视频实际字节；
- 不下载媒体、不做 OCR、ASR、转录、模型调用、embedding 或 Topic/Insight 判断；
- 不访问旧内容工作台 API/队列/数据库，不回退到线上 host；
- 不把“页面显示 20 条”或“任务 completed”说成平台总量、搜索完整性、代表性或市场趋势；
- 不提交 Cookie、账号凭据、真实原始正文到 Git、普通日志或未受限服务。

**成功条件**：

1. 浏览器实际加载的插件版本与 source/dist/manifest 可核对；Local mode 默认不向旧 host 或线上 host 发出数据；
2. Local host 收到并校验一个 discovery-only Package，保存每条安全取得的候选材料、Discovery、Capture 与 Coverage；
3. 若少于 20 条，已取得材料仍可入库，且页面清楚显示实际观察范围、缺口和原因；不得把未观察位置伪造成不存在；
4. Evidence Library 能只读取已被本地接受的结果，并显示其 Capture/Discovery/Coverage 边界；
5. 真实平台副作用限定为这一次用户授权的搜索页 Canary，证据不越权扩展为详情/评论/媒体。

**验证阶梯**：source/dist/host preflight → 合同与拒绝路径 → local host + plugin loopback dry-run → 用户在本机可见的单次真实搜索页 Canary → 数据库/页面 provenance 与 partial proof → no-old-workbench route audit。

**停止条件**：在以下任一情况发生前不得触发真实搜索：Local host 未可运行、插件实际加载版本不明、localhost 合同未通过、隐私/日志边界未验证、或页面/API 无法显示 Coverage。此时停在 source audit 或本地 dry-run，不以直接连旧工作台代替。

#### 001C-2 — 内容详情与媒体 acquisition Canary

**用户结果**：对于 001C-1 已安全发现、并被明确准入的有限内容，Linggan 能把“作品中的第几个媒体位置”“本次观察到的 URL”“实际取得的确定字节”分开记录，并诚实展示哪些图片/视频成功取得、失败、未尝试或仍未知。

**范围**：

- 仅在独立的详情/Capture、媒体生命周期、隐私与存储合同通过后，对经过准入的少量发现对象进行内容详情和图片/视频 acquisition；
- 分开记录 Content、Media Slot、Media Observation、下载 Attempt、Media Blob、MIME/hash/size 与 per-slot Coverage；
- URL 不是媒体身份，文件路径不是媒体身份；同一媒体重复出现、URL 变化和部分成功必须可追溯；
- Media Canary 完成后，Evidence Library 只显示符合当前用途资格的媒体存在、处理状态和受控预览/引用。

**明确禁止**：

- 不把搜索页的封面或 candidate 卡片当作详情媒体已取得；
- 不以“内容任务完成”覆盖媒体 slot 的失败/unknown，不把 URL、缩略图或下载路径当作 blob 事实；
- 不默认批量下载、不开展历史回填、不从旧内容工作台复制媒体、不将原始媒体写入 Git、普通日志或无权限存储；
- 不运行 OCR、ASR、摘要、embedding 或任何模型处理。

**成功条件**：

1. 每个媒体 slot 的 observed / attempted / acquired / verified Coverage 能单独解释，部分取得的 blob 仍可按用途受控保存；
2. 每个可读 blob 都能回链到来源内容、slot、观察、下载 attempt 与 hash，而不是只回链 URL；
3. 失败、未尝试、MIME 不符、URL 失效与下载中断不损坏已成功媒体，也不被写成不存在；
4. 真实平台访问仍受已批准的有界对象/数量/风险/隐私合同约束，不能因“已发现 20 条”自动深化所有详情。

**验证阶梯**：媒体合同负例 → 本地受控存储/hash/MIME proof → slot 级 partial proof → 小样本真实详情/media Canary → Evidence Library provenance/受限预览走查。

**停止条件**：没有独立通过的 Capture、媒体生命周期、私密保存/读取和删除传播合同，或没有明确的最小对象/字节预算时，001C-2 不得开始；001C-1 的发现材料保留其自身价值，但不得被升级为已取得媒体。

#### 001C-3 — 异步 OCR / 视频转录（ASR）派生材料

**用户结果**：图片中的文案和视频口播不再被当成不可见内容；在原始媒体已经被合法取得且有明确用途后，OCR/ASR 在独立异步处理队列生成可追溯的派生材料。用户能看见处理尚未开始、处理中、部分完成、失败或可用，但不会把派生文本误当平台原文。

**范围**：

- image OCR 与 video audio/ASR 是异步、可重试、可版本化的派生处理，不阻塞原始 Capture/Evidence 接纳；
- 派生材料必须带 source media/blob、处理器/模型版本、输入范围、时间、状态与失效/撤回血缘；
- 多图内容按 slot 处理；视频处理至少区分音频提取、口播转录与可选帧 OCR，不能用一个总“转录成功”状态压平；
- Evidence Library 仅在访问资格允许时，把派生文本作为明确标记的媒体派生信息与原始媒体来源并列展示。

**明确禁止**：

- 不同步阻塞采集、不将 OCR/ASR 结果写回或覆盖 Content 正文、媒体原件或历史 Observation；
- 未确认第三方处理合同前，不向外部模型发送真实原文、图片、视频、音频或可识别个人材料；
- 不以 OCR/ASR 完成证明内容真实性、完整性、用户身份、需求、趋势或媒体处理链整体成功；
- 不训练模型、不建立长期 prompt 缓存、不把派生文本作为未经审计的外部 Agent 语料。

**成功条件**：

1. 原始媒体取得与派生处理的状态、失败和重试资格分离；原始媒体可用时，OCR/ASR 失败不会撤销其 Evidence；
2. 每段派生文本可回链 blob、slot、处理任务和处理版本，且被清楚标为 OCR/ASR，不伪装平台字段；
3. 任何撤回、脱敏或访问限制能沿血缘阻断派生文本、索引、预览与后续输出；
4. 实际模型/服务、隐私、成本、队列和失败恢复通过独立合同与最小样本验证后，才允许真实媒体处理。

**验证阶梯**：派生合同与隐私负例 → 异步队列/版本/lineage test → 受控本地样本 → 批次 partial/failure/withdrawal proof → 页面状态走查。真实第三方模型处理属于额外外部副作用，必须单独确认。

**停止条件**：如果尚无明确的处理器、第三方数据处理、敏感材料访问、异步队列、成本和撤回传播合同，001C-3 只能停在接口与状态设计；不得把 OCR/ASR 作为浏览器或采集插件中的隐式同步步骤。

## 顺序、依赖与不做清单

```text
001A 先建立本地实际入口与诚实页面状态
  ↓
001B 建立受控只读投影；真实数据绑定验证等待 001C
  ↓
001C-1 才允许一次受控的前 20 条 discovery Canary
  ↓
001C-2 对经准入的小样本完成详情与媒体 acquisition Canary
  ↓
001C-3 对合法取得媒体完成异步 OCR / ASR 派生处理验证
  ↓
001B 对真实接纳结果完成页面证明
```

Local-001 不包含：旧系统迁移、线上部署、完整采集调度、历史批量回填、评论深采、作者档案、Topic/Corpus 精选、Signal/Insight/Intelligence、AI Agent、真实趋势、Outcome 或外部 Agent CLI。首批媒体、OCR/ASR 只以 001C-2/001C-3 的独立、受限、合同先行路线纳入；并不授权在 001C-1 或本治理事项中实施它们。

## 当前需要由技术审计证明、不得臆测的事实

| 事项 | 当前状态 | 影响 |
|---|---|---|
| Linggan 本地实际 Web/API host 与 port `3000` 的所有权 | `SOURCE_INCOMPLETE` | 001A 必须先核对，不能假称现有 Rust service 已满足 |
| 原插件 source、浏览器加载 `dist`、当前 host 与交付合同 | `SOURCE_INCOMPLETE` | 001C 只能先做只读 preflight |
| discovery-only 实际可见字段、分页/排序/终止与风控行为 | `SOURCE_INCOMPLETE` | 001C 在真实 Canary 前必须冻结合同 |
| 本地原始材料的显示/脱敏/保留与访问处理 | 部分已有高层不变量；具体合同未建立 | 001B 不得默认展示全文或敏感内容 |

以上不是阻断 Local-001 的计划建立，而是每张实现卡的硬停止线。

## 实施治理

- 每张卡必须有独立 GitHub Issue、Claim、`codex/` branch/worktree、draft PR、非实现者审查和 integration owner 的合并后核验；
- 001A 的 UI 变更必须额外产出 Page Spec、UI Change Manifest 与 Visual Acceptance；
- 001B 和 001C 的数据/插件合同不得通过页面代码暗中定义；先在各自 Scope 中冻结，再写实现；
- 完成声明必须拆分为：代码/合同、自动验证、本地真实链路、真实平台副作用、用户验收。任一未发生层标记 `NOT VERIFIED`。

## 本计划自身的完成条件

本计划在 Issue #23 的治理 PR 合并后成立；它只授权后续子卡按上述顺序申请独立实施，不证明 Local host、Web UI、数据库、插件、真实采集或 V7 页面已经完成。
