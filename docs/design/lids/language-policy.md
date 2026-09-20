# LIDS-LANG-001 · 中文优先的界面语言规则

> 状态: 权威当前
> 最后核对: 2026-09-02
> 适用范围: Linggan Intelligence 全部面向用户的运行时 Web UI；首个落地页面为 `PAGE-EVIDENCE-001 /corpus/evidence`
> 事实来源: Mog 对 Evidence Library 的明确确认「中文为主，英文只用来装饰或作为注释」、Mog 于 2026-09-02 指定的 `linggan-design-system-v7.html`（§03 Mono 预算、§04 壳层违规样本）、AGENTS.md、UI 协作 Agent 执行合同、LIDS-SYS-001、LIDS-PRI-001、PAGE-EVIDENCE-001 与 LOCAL-001C0 数据合同
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实运行/代码/合同、ACCEPTED 决策与当前 SCOPE；本规则只约束表达，不改写事实、权限、数据或行动

> **DESIGN-010（2026-09-02）收窄了本规则的执行口径。** 原 LANG-02 允许英文技术键紧邻任何中文出现，实践中扩散为满屏中英成对。v7 把「中英对照并列」明确列为违规，并给出 Mono 预算：只有三类内容可以是英文。新增的 LANG-05 是本规则现在的执行口径，与 LANG-01/03/04 一并生效。

## 1. 规则与目的

`LIDS-LANG-001` 固定 Linggan 的用户界面采用**中文主表达、英文技术旁注**的语言结构。

它的目的不是机械翻译，而是让用户在不认识任何英文状态码的情况下，仍能理解页面现在知道什么、尚不知道什么、能做什么以及不能做什么。英文在 Linggan 中保留其审计与工程可追溯价值，但不能再单独承担用户理解或操作责任。

## 2. 强制规则

### LANG-01 · 中文独立承载用户含义

所有用户可见的页面主标题、导航、分组、按钮、筛选、状态、空态、提示、错误解释和可读数据标签，必须由中文独立表达含义。用户不阅读英文时，也能理解：

- 当前页面的任务和材料边界；
- 某个动作是否可用；
- 某个事实是已知、未知、未获得、受限、部分还是失败；
- 当前数据不能证明什么。

英文原文不能被用作唯一按钮名、唯一状态名、唯一筛选值、唯一空态或唯一错误处置。

### LANG-02 · 英文的许可用途

英文只可用于下列三类场景：

1. **固有名与品牌**：例如 `Linggan`、`Linggan Intelligence`、`XHS`；
2. **紧邻中文的技术键或小型等宽旁注**：例如 `发布时间未知（PUBLISHED_AT UNKNOWN）`；
3. **明确的短技术代码**：例如可追溯的 `surface_read_complete`，但必须有中文字段名或解释相邻出现。

技术键应使用 Mono、小于或等于同层中文正文的视觉权重；它是注释，不是第二语言版页面。不得把大段英文、全大写英文或机器字段名做成比中文更显眼的标题、按钮或状态标签。

**本条自 DESIGN-010 起受 LANG-05 的 Mono 预算约束**：满足 LANG-02 的形式要求（紧邻、Mono、权重更低）只是必要条件，还必须落在 LANG-05 的三类之内。

### LANG-03 · 数据诚实性优先于翻译整齐

本规则不得改变原始材料、数据合同或状态资格：

- 用户采集到的标题、正文、评论、创作者展示名和原始时间文本保持原样，不由 UI 翻译；
- `UNKNOWN` 必须表达为「未知」「尚未获得」或合同指定的准确中文边界，不能变成 `0`、空白、成功或失败；
- `NOT_ACQUIRED` 必须表达为「尚未采集/尚未取得」，不能暗示不存在、已删除或媒体已本地化；
- `DISCOVERY_ONLY` 必须表达为「仅发现面材料」，不能暗示详情、评论、作者画像、媒体、OCR/ASR 或市场判断已经存在；
- 原有 English code 保留为旁注时，不得因此推断其含义比合同所允许的更强。

### LANG-04 · 组合格式

默认格式为：

```text
中文主表达（TECHNICAL_KEY）
```

空间过窄时可采用同一视觉组内的两行/两列形式：中文在前或上，技术键紧邻其后或下。技术键不得脱离对应中文留在另一张卡、另一条列表行、Tooltip 或只能鼠标悬停才能看到的位置。

例如：

| 用户表达 | 允许的技术旁注 | 禁止的孤立表达 |
|---|---|---|
| 发布时间未知 | `PUBLISHED_AT UNKNOWN` | `PUBLISHED_AT UNKNOWN` |
| 媒体尚未采集 | `MEDIA NOT ACQUIRED` | `MEDIA NOT ACQUIRED` |
| 创作者未知 | `CREATOR UNKNOWN` | `CREATOR UNKNOWN` |
| 已接纳的本机发现材料 | ~~`ACCEPTED RUNTIME MATERIAL`~~ 见下 | `ACCEPTED RUNTIME MATERIAL` |
| 搜索位置 #17 | `POSITION #17` | `POSITION #17` |
| 观察到 / 配额 | ~~`OBSERVED / QUOTA`~~ 见下 | `OBSERVED / QUOTA` |

> **DESIGN-010 对本表的修正**：`ACCEPTED RUNTIME MATERIAL` 与 `OBSERVED / QUOTA` 是**描述性标签**，不是状态枚举值，因此按 LANG-05 应整条删除英文，只保留中文——它们在本表中作为"允许的旁注"是 v2.0 时代的口径，已被收窄。`PUBLISHED_AT UNKNOWN`、`MEDIA NOT ACQUIRED`、`CREATOR UNKNOWN` 保留，因为 `UNKNOWN` / `NOT_ACQUIRED` 是数据合同中的字面取值。`POSITION #17` 保留，因为它是结构编号。

### LANG-05 · Mono 预算：三类允许，其余一律中文

英文技术旁注**不是"可以随处加的注释"**，它有预算。它扩散的速度比背景纹理更快，代价是中文用户的扫读成本——每一对中英并列都让用户多读一遍同一件事。

**只有这三类可以是英文：**

| 类别 | 内容 | 例 |
|---|---|---|
| 1 · 机器事实 | ID、时间戳、版本号、配额、数值读数 | `WORK-8D0EB837` · `09:42:11` · `UTC+08` |
| 2 · 系统状态枚举 | **有限且稳定**的状态词 | `PATROLLING` · `PARTIAL` · `INTERRUPTED` · `UNKNOWN` |
| 3 · 结构编号 | 章节序号、卡片序号、图注编号 | `01` · `FIG 01` · `ES-24-001` |

**第四类——描述性标签——禁止。** 它必须只用中文，不配英文对照：

| 禁止 | 必须写成 |
|---|---|
| `MONITOR TARGET` | 监控目标 |
| `BODY / COMMENT / MEDIA / OCR` | 正文 · 评论 · 媒体 · OCR |
| `SINCE LAST PATROL` | 距上次巡逻 |
| `LOCAL MATERIAL PROJECTION` | 本机材料投影 |
| `NO LEGACY FALLBACK` | （属系统策略，移出界面到设置页） |

判据机械且不需要讨论：**这个英文词，是不是一个可枚举的状态值、一个机器标识、或一个编号？不是，就删掉它，只留中文。**

### 状态枚举的边界

第 2 类之所以允许，是因为状态词是**闭集**——`PARTIAL` 在系统里始终是同一个值，用户学一次就够了，且它与日志、API、审计中出现的是同一个字符串。

这条不适用于"看起来像状态词的描述"。`ACCEPTED DISCOVERY` 不是一个状态枚举值，它是对一批材料的描述，因此走第四类：只写中文。

判断方法：**这个词在数据合同里是不是一个字面取值？** 是则允许，否则禁止。这让规则可以对着合同核对，而不是靠语感。

#### 已登记的单点例外（2026-09-20）

Mog 于 2026-09-20 对执行工位页（`/collection/runtime`）复刻 v7.2 稿时，明确要求保留仪器表头右侧的 `INSTALLATION_STATE`。按上述判据它是**描述性标签**，落在被禁的第四类，因此这是一条**用户授权的例外**，不是规则本身的放宽：

- 作用范围：仅 `/collection/runtime` 接单仪器面的表头一格；
- 它**不**改变 LANG-05 在其它任何页面的效力，也不构成「以后可以照此追加描述性英文」的先例；
- 决策留痕见 [../changes/runtime-station-v7-2-001-ui-change-manifest.md](../changes/runtime-station-v7-2-001-ui-change-manifest.md) §5d-5；
- 该标记描述的是稿中的「安装状态」，与本块实际判定的「通道能不能接活」并非同一件事，语义错配已在该清单同节留证。

### 时区与单位

同一事实不得同时写全称与代号。`本机时区 / 中国标准时间 / UTC+08` 三写同一件事，**界面里永远只留代号**。见 [shell-zones.md](shell-zones.md) 的 D 锚点区。

## 3. 不适用与非目标

- 不强迫翻译用户原始材料、来源 URL、平台固有名称、内容 ID、哈希、模型名或外部协议字段；
- 不把英文技术键删除为不可审计的自然语言；
- 不授权新增页面、动作、筛选、状态、数据字段、翻译服务或前端国际化框架；
- 不以本规则改变 API、数据库、采集、媒体、OCR/ASR、权限、Coverage、发布时间、查询或按钮行为；
- 不要求历史页面自动迁移。每个页面须在独立受控 UI 事项中按本规则审查。

## 4. 实现与验收

运行时页面应以语义元素标识技术旁注（当前 Evidence Library 使用 `.v7-tech-key`），并维持 LIDS 的中文 Sans / 技术 Mono 分工。实现时至少核对：

1. 全部用户可读操作、状态和空态是否有中文独立表达；
2. 英文技术键是否紧邻对应中文，且视觉权重不高于中文；
3. **每一处英文是否落在 LANG-05 的三类之内**——逐条对照数据合同确认它是字面取值、机器标识或编号；不是则删除英文；
4. 同一事实是否只写了一次（时区不得全称与代号并存，状态不得中英各写一遍）；
5. `UNKNOWN`、`NOT_ACQUIRED`、`DISCOVERY_ONLY`、部分 Coverage 等是否仍保留准确合同含义；
6. 原始用户材料是否未被翻译或改写；
7. 默认读取、严格发布时间窗口、空态和已有材料卡片是否仍通过原有数据边界测试。

第 3 条建议由自动检查执行：扫描渲染产物中的 `.v7-tech-key`（或其后继类名）内容，对照数据合同的字面取值集合与编号正则，不在集合内即失败。当前尚未实现该检查，属 DESIGN-010 欠账。

截图或字符串检查只能证明当前工作条件下的文案与呈现；它们不证明原始材料真实性、媒体取得、平台采集、详情/评论、趋势或业务结论。

## 5. 首次落地与后续迁移

- 首次落地：`DESIGN-007 / Issue #65`，仅 `/corpus/evidence`；
- 页面组合：`PAGE-EVIDENCE-001`，L1 Corpus Explorer + 受限 L2 Inspector；
- 数据语义来源：`LOCAL-001C0-DISCOVERY-BOUNDARY-V1`；
- 视觉来源：`LIDS-SYS-001`、`LIDS-PRI-001` 和既有 Token；不新增 Token、CMP 或页面骨架；
- 后续页面迁移须另立 Issue、Change Manifest 与验收记录。不得因本规则已建立，就对其他页面进行无界扫改。

## 6. LANG-05 的迁移欠账（DESIGN-010，2026-09-02）

LANG-05 是**规则层**收窄，运行时**尚未迁移**。现有页面中大量 `v7-tech-key` 属于被禁的第四类描述性标签。已知欠账：

| 位置 | 已知违规样例 | 处置 |
|---|---|---|
| `apps/api/src/local_web/shell.rs` | 一级导航中英并列、`LOCAL MATERIAL PROJECTION`、时区全称与代号并存 | 按 [shell-zones.md](shell-zones.md) 的区位表一次性收敛 |
| `apps/api/src/local_web/evidence_page.rs` | `ACCEPTED DISCOVERY`、`LOCAL READ ONLY`、`NO MATERIAL READ`、`LOCAL HOST` 等描述性标签 | 逐条对照数据合同判定，非字面取值者删除英文 |
| `apps/api/src/local_web/station_view.rs`、`collection_control_surface_view.rs` 的执行工位区块 | ~~`准入第 5 问`、`有界控制事实`、`同一评估器`、`Eligibility`、`lane 上限`、`WorkOrder`、`Attempt`、裸 station UUID~~ | **已清偿**：RUNTIME-STATION-TABLE-001（2026-09-13）。保留的英文只剩合同字面取值（`account_unknown`、`author_profile` 等原因码与能力名），且每一处都有中文相邻；由 `the_page_never_speaks_in_internal_engineering_names` 自动执行 |

两处**不在本次范围内**，须各自另立受控事项。在迁移完成前：

- 新写的界面文案一律直接按 LANG-05 执行，不得以"和旁边保持一致"为由继续新增中英对照；
- 已存在的违规不得被引用为先例；
- 不得为消除违规而顺手改写状态词、字段名或数据含义——删除的只是英文**描述性标签**，`UNKNOWN` 一类字面取值必须原样保留。

命名冲突提醒：运行时 CSS 类前缀 `v7-*` 来自 `REF-V7-001`（Evidence Library 页面 Gold Master），**与本次的设计系统 v7.0 无关**。迁移时不要把两者当成同一件事。
