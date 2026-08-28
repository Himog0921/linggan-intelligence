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
| 无匹配 | 理解当前查询零结果 | 只对 query scope 为零 | inline scope/限制/下一步 | 无平台搜索 | VERIFIED：场景按钮与 DOM 状态 |
| 读取失败 | 理解页面没有读到材料 | 不是空库或平台无内容 | inline impact/未发生/恢复 | 无 fallback | VERIFIED：场景按钮与 DOM 状态 |
| 访问受限 | 只看最小必要信息 | 脱敏片段可见，原文不展示 | restricted block 不泄漏内容 | 无权限扩张 | VERIFIED：列表与 Inspector |
| 未选择 | Inspector 不补造详情 | `SELECTION_REQUIRED` | 实现卡必须覆盖；本参考默认选择 01 | 无 | 规格验证 |

## 3. 视觉工作条件

- 设计方向：纯白台面上的精密情报基础设施；lane 带为首屏唯一视觉核心。
- Token：静态 HTML 通过相对路径消费唯一 `lids_tokens.css`；未声明 `--lgi-*`。
- CSS：原生 CSS only；无外部库、外部字体、图片、网络 API 或远程 CDN。
- 响应式：桌面三栏；≤900px 顺序折叠；≤640px lane 两列、Inspector 同页后续。
- Motion：只含 row/Inspector 的 transform/opacity；Reduced Motion 下关闭。
- 视觉资产：预览均为带标签的合成 placeholder，没有真实媒体或仿制插画。
- 截图：实际截图位于 `/tmp/linggan-evidence-reference.t4dWzW/desktop-fixed-1440x900.png` 与 `/tmp/linggan-evidence-reference.t4dWzW/mobile-fixed-390x844.png`；已人工查看，不提交 Git。

### 3.1 实际视口几何

Chrome DevTools Protocol 在同一真实渲染页上逐个设置视口并读取 DOM 几何：

| 视口 | document | 横向越界 | 桌面 document 纵向越界 | 关键结论 |
|---|---:|---:|---:|---|
| 390×844 | 390×4042 | 否 | N/A（移动自然滚动） | rail→query→results→Inspector 顺序成立 |
| 430×932 | 430×4028 | 否 | N/A（移动自然滚动） | 技术键/筛选可换行，无横向滚动 |
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

- 场景按钮依次验证 `results → empty → error → results`；对应可见区互斥，read receipt 同步显示准确中文和技术键。
- 选择第三作品后，`context-selection=03`、Inspector 标题切换为“把‘快点’换成可执行的一小步”，media panel 成为 active，且始终只有一个 `aria-selected=true` 作品行；Inspector 同步显示来源代次、Blob 曾取得、字节已清理、ASR 可检索、处理版本和禁用 CDN 回退。
- 四个作品行均独立显示作者 lane；评论和回复在讨论组内仍保留各自状态，未被压成单一讨论状态。
- 桌面首屏的主视觉是作品行的 lane 带；Inspector 视觉重量次于中央列表，没有形成第二首页。
- 移动端首屏先给出合成边界、产品方位、二级导航和查询；结果与 Inspector 在后续自然流中，不覆盖或横向压缩。
- 未使用真实图像；预览采用带状态名称的 placeholder，避免把低质量仿制媒体误认成来源资产。
- AI Slop 检查：无营销 Hero、蓝紫渐变、玻璃拟态、三张同构 KPI 卡或大圆角卡片；使用现有冷白/冷黑/Signal 和页面级 lane 结构。
