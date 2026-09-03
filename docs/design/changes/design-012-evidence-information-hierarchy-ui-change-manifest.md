# DESIGN-012 · UI Change Manifest

> 状态: 权威当前
> 最后核对: 2026-09-03
> 适用范围: `/corpus/evidence` 的信息层级、LANG-05 Mono 预算迁移、边界带收口，以及三处会让用户形成错误信念的缺陷
> 事实来源: Mog 于 2026-09-03 提交的 9 张运行时截图与「信息太过繁杂，用户不知道看什么、怎么看」的走查结论；LIDS-LANG-001 §LANG-05 与 §6 欠账表；`evidence-candidate-and-boundary-patterns.md` PAT-003；EVIDENCE-V9-001 已确认的完整度口径；当前分支代码与本机真实数据
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实运行/代码/合同、ACCEPTED 决策与当前 SCOPE
> Issue: 未开卡；由 Mog 在 2026-09-03 对话中走查后直接授权「按照规则推进」

## 事项与读取回执

- Agent / worktree：Claude；`claude/evidence-info-hierarchy-001`；`<仓库>/.worktrees/evidence-info-hierarchy-001`（自 `origin/main` 9e170c6）
- 目标：让用户进入 `/corpus/evidence` 后知道**先看什么**。当前页面把系统的自我陈述与用户要找的材料放在同一视觉层级，四屏「当前未知」淹没了三条真实内容。
- 明确非目标：不采集新材料、不改数据合同、不改完整度口径、不改状态枚举字面值、不新增页面/字段/权限/动作、不部署运行时、不迁移 `shell.rs` 与 `evidence_page.rs` 的空态路径（见「未做」）。

| 来源 | 状态 | 本次用途 | 已核对 |
|---|---|---|---|
| Mog 的 9 张截图与走查结论 | 用户结果授权 | 定义「不知道看什么」这一用户结果 | 是 |
| `LIDS-LANG-001` §LANG-05 / §6 | 权威当前 | 英文旁注三类判据；本次是其运行时迁移的一部分 | 是 |
| `evidence-candidate-and-boundary-patterns.md` PAT-003 | 权威当前 | 边界带是「区域前交代一次」，不是逐区块重复 | 是 |
| `EVIDENCE-V9-001` 完整度口径 | 权威当前 | 五维分母含未知，本次**不改** | 是 |
| `docs/data-contracts/local-001-*.md` | 权威当前 | 判定哪些英文是合同字面取值 | 是 |
| 当前 Rust 读模型与前端渲染 | 当前事实 | 逐行核对 19 处英文标签与三处缺陷 | 是 |

## 表面地图

| 表面 | 入口 | 本次是否触碰 |
|---|---|---|
| `/corpus/evidence` 结果列表（研读/表格/封面） | `evidence_library.js` | 是 |
| Inspector 四 Tab（概览/证据/材料/来源轨迹） | `evidence_library.js`、`evidence_observation.js` | 是 |
| 左栏页脚只读声明 | `local_web.rs:2884` | 是（仅作用域文案） |
| 来源轨迹 provenance 读模型 | `material_social_read.rs` | 是（修覆盖缺陷） |
| 共享壳层导航、`/collection` 等其他页面 | `shell.rs` 等 | 否 |

## 状态词典（本次涉及的用户含义与禁止替代）

| 状态 | 用户含义 | 不得替代为 |
|---|---|---|
| `UNKNOWN` | 尚未取得 / 当前未知 | `0`、空白、不存在、失败 |
| `NOT_OBSERVED` | 未观察到 | 已确认为零 |
| `KNOWN_EMPTY` | 已处理但未识别到内容 | 未取得 |
| `SOURCE INCOMPLETE` | 当前详情没有返回该信息 | 该信息不存在 |
| `NOT_VERIFIED` | 尚未证明监控目标就是作品作者 | 作者未知、目标无效 |
| 折叠后的「尚未取得 N 项」 | 与逐条 `UNKNOWN` **完全等价**，只是不再逐行占位 | 已取得、不适用 |

**结论行的措辞边界**：结论行只复述已有 lane 状态，不新增事实。允许写「正文通道尚未取得」，禁止写「这条作品没有正文」。

## 依赖地图

- 共享依赖：`lids_tokens.css`（只读用，不新增 token）、`.v7-tech-key` 类（不改语义）
- 跨事项依赖：`shell.rs` 的 LANG-05 欠账属 DESIGN-010 登记的另一事项，本次不动
- 数据合同：不改；`primary_limitation` 的语义缺陷登记为 DECISION_REQUIRED，不在本次修改

## 分类、停止条件

- 分类：**展示 + 状态/语义混合**。最高风险为状态/语义（结论行、边界带收口、provenance 修复）。
- 停止条件：任何需要新增事实、改写状态枚举、放宽访问范围或替用户下结论的做法，一律停下并标记 `DECISION_REQUIRED`。

## 变更清单

### A · LANG-05 Mono 预算迁移

按 LANG-05 判据逐条对照数据合同：**是可枚举状态值、机器标识或编号则保留，否则删除英文只留中文。**

删除（第四类描述性标签，19 处）：

| 位置 | 英文标签 | 判定 |
|---|---|---|
| `evidence_library.js:1182` | `MATERIAL SUMMARY` | 描述性 |
| `evidence_library.js:1194` | `WORK MATERIAL` | 描述性 |
| `evidence_library.js:1218` | `SEPARATE SOURCE FACTS` | 描述性 |
| `evidence_library.js:1227` | `LATEST KNOWN PER METRIC` | 描述性 |
| `evidence_library.js:1239` | `FIELD-WISE PROVENANCE` | 描述性 |
| `evidence_library.js:1251` | `PLATFORM VS LOCAL` | 描述性 |
| `evidence_library.js:1264` | `LANE STATUS` | 描述性 |
| `evidence_library.js:1272` | `VERSIONED CONTEXT` | 描述性 |
| `evidence_library.js:1299` | `ROW EXCERPT` | 描述性 |
| `evidence_library.js:1320` | `MACHINE READ` | 描述性；语义改由中文承担（见 D3） |
| `evidence_library.js:1633` | `LOCAL MEDIA` / `PROJECTED / NN OBJECTS` | 标题描述性；编号部分保留为中文＋数字 |
| `evidence_library.js:1655` | `PROCESSING LEDGER` | 描述性 |
| `evidence_library.js:1058,1797` | `BOUNDED CHANNEL` | 描述性 |
| `evidence_library.js:1805` | `PROVENANCE` | 描述性 |
| `evidence_library.js:1833` | `LIMITATIONS` | 描述性 |
| `evidence_library.js:961` | `SELECTION REQUIRED` | 描述性 |
| `evidence_library.js:990` | `DETAIL READ` | 描述性 |
| `evidence_library.js:1640` | `NO LOCAL MEDIA` | 描述性 |
| `evidence_library.js:1647` | `ACQUISITION NOT AUTHORIZED HERE` | 描述性 |
| `evidence_observation.js:149` | `REOBSERVATION` | 描述性 |
| `evidence_observation.js:255` | `ENGAGEMENT TIMELINE` | 描述性 |
| `evidence_observation.js:276` | `ATTEMPT-LEVEL COVERAGE` | 描述性 |
| `evidence_observation.js:383` | `COVERAGE` | 描述性 |
| `evidence_observation.js:395` | `LOCAL AUTHORIZED RESEARCH` | 描述性 |
| `evidence_observation.js:398` | `IDENTITY WITHHELD` | 描述性；**不在任何数据合同中**，初判有误已更正 |

英文 fallback 字面量改中文（这些不是合同取值，是前端自造的兜底文案）：
`MATERIAL DETAIL UNAVAILABLE`(1005)、`SLOT UNKNOWN`(1329,1671)、`TARGET-LINKED AUTHORIZATION`(obs:156)、`EXISTING ASSETS REUSED`(obs:174)、`NO RETURNED MATERIAL`(obs:423)。
**注意**：这些位置的 `x || 'FALLBACK'` 中，`x` 本身是合同取值时原样保留，只改兜底串。

保留（第 1/2/3 类）：`UNKNOWN`、`PARTIAL`、`ACQUIRED`、`OBSERVED`、`NOT_OBSERVED`、`KNOWN_EMPTY`、`NOT_VERIFIED`、`SOURCE INCOMPLETE`、`DISCOVERY`、`EXISTING_ASSETS_REUSED`、`OTHER_LANES_NOT_EVALUATED`、`slotKey`、`sourceRef`、作品 ref、时间戳、`POSITION #n`。

**按 LANG-001 §6：不得借本次迁移改写任何状态词、字段名或数据含义。** `SOURCE INCOMPLETE` 的空格写法维持原样。

### B · 边界带收口（PAT-003）

现状：`sourceIncompleteBlock()` 被无差别调用，一个 Inspector 内出现 5 次以上近乎同文的米色框。PAT-003 规定边界带是「在可能被误读的区域**前**交代一次」。

- Inspector 顶部新增**一条**边界带，承载现在被重复 5 次的那句话。
- 区块内的 `sourceIncompleteBlock()` 降级为一行弱化灰字，不再使用米色 `tone="warning"` 框。
- 保留但**不弱化**的例外：`tone="danger"`（真实失败）与 `tone="restricted"`（访问边界）。二者当前同色，本次将 `restricted` 与 `danger` 分色，让「失败」和「受限」不再共用一个视觉。

### C · 信息层级四层

| 层 | 内容 | 默认 |
|---|---|---|
| 0 · 结论行 | 由真实 lane 状态复述：已取得什么、哪些通道尚未取得 | 常驻 |
| 1 · 内容 | 正文、评论原声、媒体、图片文字 | 展开 |
| 2 · 状态 | 完整度、通道状态、互动数字 | 展开（压缩排布） |
| 3 · 查证 | 来源轨迹、字段级溯源、技术 ID、边界说明 | 折叠 |

- 连续的 `UNKNOWN` 事实行折叠为「尚未取得 N 项 ▸」，展开后逐条仍是原文案与原状态码。**折叠是排布，不是改写**。
- 互动时间线中「本时点互动字段均为当前未知」的连续同值记录合并为一条并注明观察次数与时间范围；展开后仍可见每次观察。

### D · 三处会让用户形成错误信念的缺陷

**D1 · 来源轨迹的 Target / Work Order 恒为 `SOURCE INCOMPLETE`（真数据丢失）**

`material_social_read.rs:118` 用 `inspector.insert("provenance", …)` **整体替换**了 provenance 对象，把上游 `enrich_collection_context`（`material_projection.rs:382-392`）已写入的 `targetRefs` / `workOrderRefs` 一并抹掉。前端读到 undefined，永远渲染「来源信息不完整」。

修法：改为**合并写入**，保留上游已有键。同时 `read_provenance` 的 producer 列表去重（`material_social_read.rs:466` 无 dedup，同一 Producer 产出两个 package 时重复显示）。

影响面：这是本次唯一的读模型改动。它不新增字段、不改字段语义，只是停止丢弃已有数据。

**D2 · 左栏「不会触发平台采集」的作用域名不副实**

`local_web.rs:2884` 是**页面级**声明，但同一页面的 Inspector 里，「立即复观测」会真的创建准入决定、工单与租约（`content_reobservation.rs:119-247`），由 Browser Producer 去平台抓详情与评论。

修法：把声明收窄到它真实成立的范围——列表与详情的读取不触发采集；复观测是独立授权动作。**只改文案，不改任何行为。**

**D3 · 封面 OCR 在证据 Tab 被当作可引用证据**

`material_evidence_fragment.rs:224-247` 已明文把封面 OCR 排除在行级原声引用之外，理由写在注释里：封面是设计图，OCR 出来是噪声（本库真实产出如 `oy 六 字 =] Li`）。但证据 Tab 的「图片文字与转录原文」（`evidence_library.js:1289-1335`）取全部 derivatives，不区分 `:cover:`，且每条挂绿色「已取得」。

修法：证据 Tab 对 `slotKey` 含 `:cover:` 的条目标注来源为封面并说明其为机器识别、不宜直接引用；正文图片 OCR 不受影响。**不删除数据、不改状态码**，只让页面表达出后端已有的判断。

### E · 列表卡片的作者与监控目标关系

`evidence_library.js:616-632` 在卡片上把「作品作者」与「监控目标」并排，只挂一个 8px 的裸 `NOT_VERIFIED`；详情页那句「尚未证明监控目标就是作品作者」在卡片上没有等价表达。卡片是扫读面，误读代价最高。

修法：卡片上为 `NOT_VERIFIED` 补中文短表达（符合 LANG-01：中文必须独立承载含义）。

## 验收矩阵

| 表面/状态 | 覆盖方式 |
|---|---|
| LANG-05 迁移无遗漏 | 扫描渲染产物中 `.v7-tech-key` 内容，逐条比对合同字面取值集合 |
| 状态语义未被改写 | `cargo test --workspace` 既有断言全绿；不修改任何状态字符串 |
| provenance 合并写入 | 新增 Rust 测试：上游 targetRefs 在 social 读取后仍存在 |
| 折叠不改写事实 | 展开后文案与状态码与折叠前逐字一致 |
| 边界带收口 | Inspector 内 `tone="warning"` 米色框数量由 5+ 降为 1 |
| 真实数据走查 | 本机 `:3100` 开发实例对同一作品（`80685d45`）逐 Tab 比对 |

## 未做与已知边界

- **`primary_limitation` 是写死的默认值**：`material_projection.rs:633` 把 `OTHER_LANES_NOT_EVALUATED` 设为初始常量，全代码库只有 `enrich_media_material`（`:446-448`）在媒体侧三种限制下会覆盖它；评论、详情、作者、OCR 通道跑没跑过从不参与计算。因此它**不等价于**「只跑过发现通道」，一篇采全的作品照样显示「尚未评估」。修正它需要先定义「主要限制」的产品语义，属 Mog 决定 → **本次登记为 `DECISION_REQUIRED`，UI 侧停止把它当结论展示，结论行改用真实 lane 状态计算。**
- **`sort=relevance` 点选后报错**：`material_projection.rs`(local_web) 接受该参数，但 `crates/evidence/src/material_projection.rs:40-42` 对非 `LatestDiscovery` 直接返回 `UnsupportedSort`。与 EVIDENCE-V9-001 「只保留最近观察与相关度」的记载不符。本次不修，登记为待开卡。
- **系统视图会静默覆盖用户手选筛选**：`evidence_library.js:303-308` 用 `params.set()` 覆盖同名参数；后端本身支持四维 AND 组合（`material_projection.rs:463-499`）。属交互设计变更，需另立事项。
- **`shell.rs` / `evidence_page.rs` 的 LANG-05 欠账**：DESIGN-010 §6 已登记，须各自另立受控事项，本次只动 `local_web.rs:2884` 一处作用域文案（因其构成 D2 的错误信念）。
- 未部署：`:3000` 常驻服务指向冻结快照，本次只在开发实例验证。
