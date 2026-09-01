# EVIDENCE-V9-001 · UI Change Manifest

> 状态: 权威当前
> 最后核对: 2026-09-01
> 适用范围: `/corpus/evidence` 按 V9 Gold Master 的研读密度、Inspector 宽度状态机、LOCAL MEDIA 横向材料浏览与行级原声引用
> 事实来源: Mog 当前交付包、V9 Agent Handoff（`~/Downloads/linggan-evidence-library-v9-agent-handoff`）、DESIGN-003、PAGE-EVIDENCE-001、LIDS tokens、`media-lifecycle-contract.md`、当前分支代码与本机真实数据
> 冲突时以谁为准: 用户最新确认、AGENTS.md、当前代码/API 合同与真实运行证据；V9 HTML 只对视觉与交互有权威，不对数据语义有权威
> Issue: 未开卡；由 Mog 在本次对话中直接派定交付包并授权「从 main 开新工作树，一次做完给一个 PR」

## 事项与读取回执

- Agent / worktree：Claude；`claude/evidence-library-v9-ui`；`/Users/moglenny/proma/worktrees/linggan-evidence-v9-ui`（自 `origin/main` a85bedc）
- 目标：把 V9 已确认的页面骨架、三视图密度、Inspector 三档宽度与 LOCAL MEDIA 材料浏览落到真实读模型上。
- 明确非目标：不采集新材料、不回填历史 Package、不改 Media V2 身份、不做 mutation（打开原文 / 补采保持 disabled 并说明原因）、不迁移共享库、不发布运行时、不合并。

| 来源 | 状态 | 本次用途 | 已核对 |
|---|---|---|---|
| V9 Gold Master HTML + 三张参考图 | 视觉/交互权威 | 页面骨架、行密度、宽度档位、媒体轨与 Lightbox | 是 |
| `AGENTS.md` / `docs/governance/` | 权威当前 | 受保护交付流程、文件治理、生成物登记 | 是 |
| DESIGN-003（页头回收） | 权威当前 | 页名不得第三次出现；本次据此裁定，见「与 V9 的偏离」 | 是 |
| LIDS tokens / 语义护栏 | 权威当前 | 颜色唯一来源；新增 Evidence/Display 字族与马赛克纹理 | 是 |
| `media-lifecycle-contract.md` | 权威当前 | 「普通列表使用脱敏片段，原文留在受控 Inspector」 | 是 |
| 当前 Rust 读模型与本机真实 14 条作品 | 当前事实 | 片段可得性、媒体顺序资格、排序支持范围 | 是 |

## 分类、表面与停止条件

- 分类：展示 + 交互 + **新读能力**的混合变更；最高风险为新读能力（行级原声引用）。
- 影响表面：`/corpus/evidence` 页面、`/api/local/work-resources` 列表响应、LIDS token 源。
- 停止条件：任何需要触发平台访问、放宽受限材料访问范围或改写媒体身份的动作，一律停在 disabled 并写明原因。

## 变更清单

### 读模型（新能力）

- 新增 `crates/evidence/src/material_evidence_fragment.rs`：为每个作品读取**一条有界原声摘录**，随摘录返回来源类型（正文 / 评论 / 图片文字 / 视频画面文字 / 转录）、选取依据、是否截断、来源引用与命中槽位。
- 新增响应字段 `items[].evidenceFragment`。`null` 是真实答案，表示这个作品还没有可引用的文本，页面按诚实空态渲染，不用标题顶替。
- 摘录上限 88 字符，命中时保留 20 字符前文；每个偏移量都是**字符**偏移，不是字节偏移。
- 取材顺序：有检索词时 正文 → 图片文字/转录 → 评论（命中优先，且携带 `slotKey` 供 GO TO MATCH 定位）；无检索词时 正文 → 评论 → 正文图片文字。
- **封面 OCR 不作为无检索词时的引用来源**。本库真实封面 OCR 产出形如 `oy 六 字 =] Li`、`6双干失信各关 | oy 一 Re ea aos`；把它当作「最强证据」会在读者预期看到一句话的位置放一串乱码，并且是系统自己无法背书的说法。有检索词命中封面 OCR 时仍然展示，因为那正是读者要找的字符串。

### 页面

- 页头改为「任务陈述 + 本次读取读数」，不再出现页名（见「与 V9 的偏离」）。读数为 作品 / 已留存评论 / 本地材料 / 部分·缺口 四项，全部由本次读取的 items 计算，标签写明本次读取，不冒充全库总数——读模型按扫描预算工作，本来就没有全库总数。
- 查询区收敛为 一条 FIND 主检索 + FILTER 气泡 + 排序 + 主动作；四个筛选下拉收进气泡并显示激活计数。快捷视图与排版切换压成一条标签带。
- 研读行按 V9 重排：微缩封面 / 身份 / 一句衬线原声 / 互动四数 / 材料摘要轨 + 状态 + 最近观察。**九宫格通道矩阵从列表行移入 Inspector 概览**，符合 V9「禁止把完整材料矩阵塞回列表行」。
- 表格视图：无封面、六列固定高密度。封面视图：图片主导、不做卡中卡、缺封面走诚实空态。
- Inspector：四 Tab 更名为 概览 / 证据 / 材料 / 来源轨迹；新增三档宽度 408 / 640 / min(920px, 72vw) 与关闭，宽度偏好写 `localStorage` 并进 URL；1180px 以下改右侧抽屉，`Esc` 关闭。
- 新增 LOCAL MEDIA 横向材料浏览：按采集顺序展示全部媒体对象（作者头像除外），支持拖拽 / 滚轮横移 / 左右按钮 / 方向键、位置读数、进度轨、GO TO MATCH，以及原尺寸 Lightbox（左右切换、方向键、`Esc`、焦点归还原卡片、锁定背景滚动）。图片一律 `object-fit: contain`——证据不能被裁切。
- 每个媒体对象独立显示本地对象 ID、字节状态与文字处理状态；单个对象加载失败只在该卡片内降级，不会让整篇作品的媒体变空。

### 材料完整度口径

固定五维：正文 / 评论 / 媒体 / 图片文字 / 转录。`applicable = 状态 != NOT_APPLICABLE`；`ready = COMPLETE | ACQUIRED`（含 `SEARCHABLE` / `KNOWN_EMPTY`）；`summary = ready / applicable`。

- 「转录」只有在**媒体已观察且确认没有视频**时才判为不适用；媒体尚未观察时它留在分母里作未知——「没看过」和「不适用」不是同一个断言。
- 分子在全部维度都未知时显示「未知」，不显示 0。

### Token

`lids_tokens.css` 与 `docs/design/lids/tokens.md` 同步新增四个 token：`--lgi-font-evidence`（衬线，只承担逐字引用）、`--lgi-font-display`（窄体，页面级强层级）、`--lgi-mosaic-on-dark` / `--lgi-mosaic-on-light`。语义护栏同步补充 Type 与 Mosaic 两行；基线数由 127 增至 131。

## 与 V9 的偏离（均为有依据的裁定，不是遗漏）

| V9 要求 | 本次做法 | 依据 |
|---|---|---|
| 页头显示「Evidence Library」大字页名 | 只保留任务陈述与读数，页名回到 `sr-only` h1 | DESIGN-003 已记录：面包屑与左栏已经两次命名本页，第三次是重复；该测试仍在 `every_surface_reclaims_its_header_instead_of_restating_its_own_name` 中守门 |
| `SOURCE ORDER PRESERVED` | 写「采集顺序 · 平台排列顺序未经验证」 | 真实数据里每个槽位 `displayOrderState = UNKNOWN`、`displayOrderBasis = producer_global_sequence_unverified`；Mog 已拍板按采集序展示并标注未验证 |
| 排序含发布时间 / 点赞 / 评论 | 只保留 最近观察 与 相关度 | 读模型只支持 `latest_discovery` 与 `relevance`，其余排序无实现；给出无效选项等于假 UI |
| 页头读数为全库量级 | 读数描述本次读取 | 读模型按扫描预算工作，没有全库总数；编一个会在数据长大后变成谎话 |
| `SAVED VIEW / 06` | 「暂无已保存视图 SAVED VIEWS NOT CONNECTED」 | 已存查询未接通；显示计数等于宣称功能存在 |
| 打开原文 / 补采 / 加入研究 | 保持 disabled 并在 `title` 写明原因 | 本页只读，不触发平台访问；补采需要采集授权 |
| 信号色 `#f24a23`、墨色 `#0b0f12`、侧栏 214px | 沿用 LIDS 的 `#ef4f25` / `#111315` / 216px | LIDS 是颜色唯一来源；差异在感知阈值以下，token 纪律不在 |

## Mog 走查后的信息密度修订

走查提出「信息重复、字太小没层次、控件不统一」。逐条处理：

| 位置 | 重复/问题 | 处理 |
|---|---|---|
| 读取回执行 | 「读取成功，当前显示 14 个作品集合 · 扫描 14」+ `AS OF …` 与页头读数、列表行三重重复 | 整行改为只在扫描预算触发或截断无游标时出现；读取时间与扫描数移到结果栏 `title` |
| 结果栏作品计数 | 与页头读数「14 作品」重复，且页头不随列表滚动，始终可见 | 删除 |
| 左栏「语料与证据 / 语料库」 | 面包屑与栏目项已两次命名本页 | 删除 |
| `FIND / 检索作品与证据` 标签 | 输入框内已有 `FIND` 标记 | 删除标签，保留标记与 `aria-label` |
| 表格每格「材料 3 / 5」 | 列头已写「材料」 | 表格布局隐藏内联标签 |
| 检查器头部 | `XHS / WORK …` + 标题 + `作者 · 最近观察 · 主要限制 <枚举>` 三行，作者与观察时间与选中行重复 | 压缩为 标题 + 一行主要限制（中文在前，原始枚举作弱化旁注）；ref 缩为 8 位移到动作行右端 |
| `SYSTEM VIEWS / 系统视图` 等 | **违反 LANG-02/LANG-04**：英文在前且与中文等权重 | 改为「系统视图 <tech>SYSTEM VIEWS</tech>」，英文作更小的技术旁注 |

控件与排版：

- **筛选、排序改为锚定浮层**。原先筛选在查询区下方展开一个容器，每次开合都把结果列表推走；排序用原生 `<select>`，其弹层是系统渲染的圆角蓝高亮，无法纳入本系统的硬边界语言。两者现在都是挂在自己按钮下的浮层，互斥、点外部关闭、`Esc` 关闭；排序改为 `role="listbox"` 的自绘选项。
- **hover 配色 bug**。原先一条全局 `.ev-button:hover{color:Ink}`，让本就是墨底的按钮（展开态的筛选、主动作）在悬停时变成黑底黑字。现按面分别定义 hover。
- **搜索框不再画红框**。点击输入不是错误状态；Signal 留给需要注意的事。键盘焦点仍有可见环，改用墨色。
- **字号整体上调并拉开层级**：行标题 15→18px、原声引文 12.5→15px、行内元信息 8.5px mono→12px sans、事实网格 10→12.5px、Tab 11→14px、读数 24→30px。行内时间戳由 ISO 全量改为分钟精度，完整值保留在 `title` 与检查器事实网格。

## 验证

- `cargo test --workspace`：26 个测试目标全部通过，无失败。
- `cargo fmt --check` 通过。
- 8 个既有断言按新结构重写或按新事实更新，均保留原不变量意图，详见验收记录。
- 真实数据走查见 `docs/design/acceptance/evidence-v9-001-visual-acceptance.md`。

## 未做与已知边界

- `cargo clippy --workspace` 仍被 `crates/contracts/src/producer_runtime.rs:408` 的既有 `too_many_lines` 阻断。该函数在 `origin/main` 上逐字相同，本次未触碰 `crates/contracts`，也未借机修改。
- `check-rust-boundaries.sh` 的 13 项文件长度错误与 1 项 SQL 归属错误全部在 `origin/main` 上已存在；本次没有新增任何一项跨过硬阈值的文件。新增的 `material_evidence_fragment.rs` 为 431 行，属警告区（警告 350 / 硬限 500）。
- 未部署：`:3000` 常驻服务仍指向冻结快照 `linggan-intelligence-xhs-media-author-e3f8990`，本次改动只在 `:3100` 开发实例验证。
