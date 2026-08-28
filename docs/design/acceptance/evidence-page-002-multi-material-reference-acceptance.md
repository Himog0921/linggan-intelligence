# ACC-EVIDENCE-PAGE-002 · 多材料证据库静态原型验收

> 状态: 一次性报告
> 最后核对: 2026-08-28
> 适用范围: Issue #85 的多材料 Evidence Library 产品规格与静态高保真原型
> 事实来源: PAGE-EVIDENCE-001、MEDIA-RECON-001、LIDS、静态 HTML、验证脚本与本次实际浏览器走查
> 冲突时以谁为准: 真实文件、实际验证输出、用户最新确认；静态截图不能证明后端或真实材料

## 1. 验收对象

- Issue / Work Package：Issue #85 / `EVIDENCE-PAGE-002`。
- 页面：`语料 → 证据库`；静态参考 `evidence-library-multi-material-reference.html`。
- 关联：`PAGE-EVIDENCE-001`、`LIDS-PAT-001 / Corpus Explorer + Split Evidence Inspector`、`LIDS-LANG-001`、MEDIA-RECON §6–7。
- 验收环境：本 worktree 静态 HTTP；全部材料人工合成；无数据库、平台、媒体或处理器访问。

## 2. 场景矩阵

| 场景 | 用户任务 | 预期含义 | 视觉/交互检查 | 真实后果 | 结果 |
|---|---|---|---|---|---|
| 部分但可用 | 判断详情、讨论、媒体和派生是否可用 | detail 可检索；comments partial；replies failed；slot observed；bytes not requested；OCR not enabled | lane 不压成总状态；选择后 Inspector 可核验 | 无 | VERIFIED：桌面/窄屏浏览器走查 |
| 风险停止 | 判断已有材料与本次停止的关系 | 保留此前合格详情；当前 Attempt `RISK_CONTROL` | danger 状态 + 不自动重试说明 | 无 | VERIFIED：第二作品行 |
| 字节已清理 | 判断字节历史与派生可用性 | bytes 曾取得后清理；ASR 可检索且保留血缘 | warning + searchable 并存 | 无 | VERIFIED：第三作品行 + media Inspector 交互 |
| Unknown / not requested | 避免把缺材料写成 0 或失败 | 作者/发布时间 unknown；媒体尚未请求；评论数量无结论 | 无 0、无“无媒体” | 无 | VERIFIED：第四作品行 + verifier 禁词 |
| Processing | 区分真正运行、排队和成功 | 图片字节/副本可读；OCR 处理器实际运行；派生尚不可检索 | `PROCESSING` + Job/版本/开始时间；不提前成功 | 无处理器调用 | VERIFIED：第五作品行、处理中筛选与 Inspector |
| 多候选来源 / Live Photo partial | 核验槽位来源组、地址断言、组件和组合关系 | 本次 Package 只有一个 slot origin group/generation；其 declared Bundle 下分 still/motion；每条断言具 `candidateRef/order/sourceField/observedAt/expiresAtState`，下载尝试绑定精确 `candidateRef` | still acquired / motion failed / overall partial；候选顺序不推断组件；历史代次不混入本次 Package | 无媒体请求 | VERIFIED：第六作品行与 Media Inspector |
| 无匹配 | 理解当前查询零结果 | 只对 query scope 为零 | inline scope/限制/下一步 | 无平台搜索 | VERIFIED：场景按钮与 DOM 状态 |
| 读取失败 | 理解页面没有读到材料 | 不是空库或平台无内容 | results/Inspector/selection 清空；inline impact/未发生/恢复 | 无 fallback | VERIFIED：场景状态机与 DOM 回归 |
| 访问受限 | 只看最小必要信息 | 脱敏片段可见，原文不展示 | restricted block 不泄漏内容 | 无权限扩张 | VERIFIED：列表与 Inspector |
| 未选择 | Inspector 不补造详情 | `SELECTION_REQUIRED` | 实现卡必须覆盖；本参考默认选择 01 | 无 | 规格验证 |

## 3. 视觉工作条件

- 设计方向：纯白台面上的精密情报基础设施；lane 带为首屏唯一视觉核心。
- Token：静态 HTML 通过相对路径消费唯一 `lids_tokens.css`；未声明 `--lgi-*`。
- CSS：原生 CSS only；无外部库、外部字体、图片、网络 API 或远程 CDN。
- 响应式：桌面三栏；≤900px 顺序折叠；≤640px lane 两列、选择后进入 Inspector，并可返回当前作品所在列表。
- Motion：只含 row/Inspector 的 transform/opacity；Reduced Motion 下关闭。
- 视觉资产：预览均为带标签的合成 placeholder，没有真实媒体或仿制插画。
- 最新截图：桌面/窄屏位于 `/tmp/linggan-evidence-reference.contract-fixed/desktop-1440x900.png` 与 `/tmp/linggan-evidence-reference.contract-fixed/mobile-390x844.png`。两张均已人工查看，不提交 Git；此前 error/processing 专项截图仍为同轮设计验收的辅助资产，但本次合同修订以最新两张为准。

### 3.1 实际视口几何

Chrome DevTools Protocol 在同一真实渲染页上逐个设置视口并读取 DOM 几何：

| 视口 | document | 横向越界 | 桌面 document 纵向越界 | 关键结论 |
|---|---:|---:|---:|---|
| 390×844 | 390×5258 | 否 | N/A（移动自然滚动） | rail→query→results→Inspector 顺序成立；列表 ↔ Inspector 可返回 |
| 430×932 | 430×5244 | 否 | N/A（移动自然滚动） | 技术键/筛选可换行，无横向滚动 |
| 1280×800 | 1280×800 | 否 | 否 | Results/Inspector 内部滚动 |
| 1440×900 | 1440×900 | 否 | 否 | 216px rail + 420px Inspector；三栏稳定 |
| 1536×960 | 1536×960 | 否 | 否 | 216px rail + 440px Inspector |
| 1728×1117 | 1728×1117 | 否 | 否 | 工作面填满剩余视口 |
| 1920×1080 | 1920×1080 | 否 | 否 | 首屏同时显示列表与 Inspector |
| 2560×1440 | 2560×1440 | 否 | 否 | 主工作区不拉伸成无界长行 |

首轮测量发现桌面 shell 使用 `min-height` 会被内容撑长；已改为固定剩余视口高度，随后全部桌面视口复测为 document 高度等于 viewport。移动端保持自然文档流。

## 4. 分层结论

| 完成层 | 状态 | 证据 | 限制 |
|---|---|---|---|
| 产品规格 | VERIFIED | `PAGE-EVIDENCE-001` 覆盖任务、表面、状态、IA、字段、回执、敏感边界与验收 | 等待后续实现消费 |
| 静态原型 | VERIFIED | HTML source + Chrome actual render + scenario/selection/tab CDP interaction | 不等于 runtime UI |
| 自动检查 | VERIFIED | reference verifier、UI handbook、project governance、bootstrap、diff/path audit 全部通过 | 不证明真实后端 |
| 视觉/响应式 | VERIFIED | desktop/mobile screenshot + 8 viewport geometry | 不等于 Mog 验收 |
| 真实链路/回执 | NOT VERIFIED | 本卡禁止真实动作 | 无 DB/platform/media/OCR/ASR |
| 部署 | NOT VERIFIED | 未部署 | static local only |
| Mog / 业务验收 | NOT VERIFIED | 等待 stacked PR 审查 | 用户尚未确认最终页面 |

## 5. 不得因此推断

- 静态 lane、数量和时间不是运行数据；
- 原型选择、Tab 和场景切换不是服务端回执；
- 多材料 API、类型化材料、真实评论树、媒体字节、OCR/ASR、清理和撤回传播仍未由本卡证明；
- 当前 `/corpus/evidence` runtime 仍以 Discovery-only 窄投影为主；本卡没有修改或发布它。

## 6. 实际交互与视觉结论

- 场景按钮依次验证 `results → empty → error → results`；empty/error 均隐藏结果与 Inspector、清空当前选择和 Inspector DOM，恢复 results 时只从 `scenarioData` 单一映射重新渲染，没有残留 `WORK-REF-01`。
- 全部、部分取得、风险停止、字节已清理、处理中五个筛选逐一点击；可见 option 分别为 `01–06 / 01+06 / 02 / 03 / 05`，结果数、当前选择、`aria-pressed` 与 read receipt 同步。
- 初始作品行与 Inspector 的标题、lane 状态、概览、讨论、媒体和血缘均由 `renderWorkRows → setScenario('results') → applyFilter → renderSelection → scenarioData['01']` 生成；静态列表和 Inspector 容器保持空，不存在两套初始事实。
- 选择第三作品后，`context-selection=03`、Inspector 标题切换为“把‘快点’换成可执行的一小步”，media panel 成为 active，且始终只有一个 `aria-selected=true` 作品行；Inspector 同步显示来源代次、Blob 曾取得、字节已清理、ASR 可检索、处理版本和禁用 CDN 回退。
- 六个作品 option 均独立显示作者 lane；评论和回复在讨论组内仍保留各自状态，未被压成单一讨论状态。结果使用 `role=listbox` + `role=option` 和 roving `tabindex`，任一时刻只有当前可见 option 为 `tabindex=0`；ArrowUp/ArrowDown、Enter/Space 均可切换当前作品。
- Inspector tabs 使用 roving `tabindex`，ArrowLeft/ArrowRight 循环切换，Home/End 跳到首尾并同步 panel；selected work、scenario、filter 和 tab 的 hover 均明确排除当前选中态。
- 第五作品验证 `PROCESSING` 只在合成处理器实际运行时出现，Inspector 同时显示已取得 Blob、本地副本、Job、处理版本和“尚未形成可检索派生”。
- 第六作品验证本次 `package/synthetic-006` 对 `slot/live-photo/body/01` 只有一个槽位级 `origin-group/slot-006/g3`；declared Bundle 再关联 `still_image` 与 `motion_stream`。still A/B 与 motion M1 均逐条显示 `candidateRef/order/sourceField/observedAt/expiresAtState`，两个下载尝试分别绑定 `candidate/still/A` 与 `candidate/motion/M1`。候选顺序不推断组件，跨代历史不混入本次 Package。
- 原型保持无依赖单文件，内部按 `scenarioData`（唯一场景事实）、`renderWorkRows/renderSelection`（渲染）、`applyFilter/setScenario`（状态）及 tab/list/mobile handlers（导航）分责。作品标题、lane 状态标签与 Inspector 均从同一 `scenarioData` 渲染，避免静态行和 Inspector 两套事实漂移。
- 390px 窄屏选择第六作品后进入 Inspector；点击“返回当前作品所在列表”后当前 option 回到视口内且获得焦点，避免单向滚动和焦点滞留。
- 桌面首屏的主视觉是作品行的 lane 带；Inspector 视觉重量次于中央列表，没有形成第二首页。
- 移动端首屏先给出合成边界、产品方位、二级导航和查询；结果与 Inspector 在后续自然流中，不覆盖或横向压缩。
- 未使用真实图像；预览采用带状态名称的 placeholder，避免把低质量仿制媒体误认成来源资产。
- AI Slop 检查：无营销 Hero、蓝紫渐变、玻璃拟态、三张同构 KPI 卡或大圆角卡片；使用现有冷白/冷黑/Signal 和页面级 lane 结构。
