# WEB-REACT-001 阶段 D 分册 · 语料三页迁移到 React

> 状态: 活跃计划
> 最后核对: 2026-10-16
> 适用范围: WEB-REACT-001 阶段 D 的拆分、各页能力清单（迁移前盘点）、已定的设计改动与验收；D1 已合并上线（PR #436，`baaf5a51`，上线后只读核验见 #433）；D2 已实施并复核，以 Draft PR 呈交（§4.10）；D3 未开始
> 事实来源: Mog 2026-10-15「接下来重点推进 D，语料三页」与「实际代码部分交给 Sonnet 5.5 子代理执行」；[WEB-REACT-001](web-react-001.md) 阶段 D；Issue #433；`origin/main@36f3fdf9` 旧页面源码
> 冲突时以谁为准: Mog 最新确认、DEC-0010、WEB-REACT-001 主计划、真实代码与运行证据

本文是主计划阶段 D 的分册，不另立推进入口。规则 R1–R7、目标架构与安全约束以[主计划](web-react-001.md)为准。

## 1. 拆分与顺序

| 子项 | 范围 | 旧实现（切换时删除） |
|---|---|---|
| D1 | 语料共享外壳（左侧二级导航、领域选择器与其只读接口、采集状态词）+ `/corpus/creators` | `creators.rs` 页面部分、`creators.html/.js/.css`、只服务旧页面的测试 |
| D2 | `/corpus/evidence`（含观察记录、研究底稿、媒体） | `evidence_page.rs`、`local_web.rs` 中的证据库页面拼接、`evidence_library.js/.css`、`evidence_observation.js`、`research_dossier*.js` |
| D3 | `/corpus/comments` 四视图（概览、用户评论、研究批次、用户问题），一次切换 | `comment_study.rs` 页面部分、`comment_study.html/.js/.css`、`comment_study_results.js/.css`、`comment_research.css` |

每个子项一个 PR，切换的同一 PR 删除旧实现（R2）。三页全部迁完后，`shell.rs` 的语料侧栏与领域选择器拼接一并删除。

## 2. 已定的设计改动（只做这些，不另做新视觉）

- 照搬现有能力与信息结构；页面样式只用 LIDS Token（CSS Modules，自动检查拦写死数值）。
- 机器语言不外露：普通层不出现 `NO LEGACY FALLBACK`、`SAVED VIEWS NOT CONNECTED`、字段码、UUID 等；诊断信息可展开（基线审计 F06）。「我的视图」等未接通占位删除（F16）。
- 日期统一为 `YYYY-MM-DD HH:MM` 北京时间（`humanMoment`，F15）；未知不是 0。
- 三页外壳一致：同一页头、同一侧栏、领域选择器在面包屑里；侧栏与页头和 `shell.rs` 由测试对照（R4）。
- 证据库阅读优先：标题、作者、正文、图组、讨论在前，技术事实放次层（F08，D2）。
- 创作者「加入观察」需要明确确认后才发出请求，并说明影响（D1）。
- 头像、封面的可访问名称与实际加载状态一致（F04，D1）。
- 评论研究概览保持 #429 已上线的 A 方案·无边框 V2 的布局与读数口径（D3）。
- 领域在三页之间切换不丢（F09）。

## 3. D1 · 共享外壳与创作者页

### 3.1 能力清单（迁移前盘点，由实施方填写并在 PR 中逐项核对）

**盘点依据**：旧 `creators.rs / creators.html / creators.js / creators.css`（`origin/main@36f3fdf9`）逐函数、逐元素通读；接口合同 `crates/contracts/src/creator_discovery.rs`、`crates/evidence/src/creator_discovery.rs`、`creators.rs`；[PR #384](https://github.com/Himog0921/linggan-intelligence/pull/384)（`f7528a58`，未合并）2026-10-10 的调查结论只读核对。表格结构以主线已确认的八列（#386）为准，不采用 #384 的五列。

**路径简写**：`C/` = `apps/web/src/pages/corpus/creators/`；`K/` = `apps/web/src/corpus/`；`A/` = `apps/web/src/app/`。
**标记**：＝ 照旧；△ 有意改动（见 §2 的已定改动，或仅实现方式不同、用户可见行为相同）；＋ 新增（#384 发现或补旧页的洞）；【#384 发现 n】对应 Mog 补充要求的第 n 条。

#### A. 页面入口、外壳与领域

| 编号 | 旧页能力（行为、接口、状态） | React 实现 | 说明 |
|---|---|---|---|
| A01 | 路径 `/corpus/creators`，由服务器返回页面；另有 `/assets/creators.css`、`/assets/creators.js` | `web_app.rs` `REACT_PAGES`、`A/pages.ts`、`A/App.tsx`（lazy 按需加载）；旧页面处理、两条资源路由、`creators.html/.js/.css` 已删除 | ＝；旧资源路径返回 404（路由测试） |
| A02 | 全局页头：一级导航 6 项、系统胶囊「本机服务状态」、个人菜单 | `K/CorpusFrame.tsx` → `A/AppShell.tsx` | ＝；样式沿用 `shell.css` |
| A03 | 一级导航「采集」项的状态词：旧创作者页（与旧评论研究页）调用 `global_header(…, None)`，恒显示「状态未知」；旧证据库页传真实值（观察中 / 无观察目标 / 状态未知） | `AppShell` 新增 `collectionState` 覆盖（与 shell.rs `collection_state` 等价）；`K/CorpusFrame.tsx` 把语料外壳接口给出的状态词传进去，外壳没读到时保持「状态未知」 | △ 有意改动：同一时刻同一份数据，旧三页的「采集」状态却不一致；按 §2「三页外壳一致」改为都取服务器值。若要照搬旧页恒为「状态未知」，只需 `CorpusFrame` 不再传这个值 |
| A04 | 语料侧栏：3 项、序号、当前项 `aria-current`、每项带 `?domain=`、底部文字「作者来自当前领域已有作品」、证据库计数徽标位 | `K/CorpusSideNav.tsx`、`K/corpusPages.ts`；`K/corpusPages.test.ts` 逐项对照 shell.rs 的 `CORPUS_ENTRIES` 与侧栏标记 | ＝ |
| A05 | 面包屑「语料 / 领域选择器 / 创作者」；选择器为 details + 链接；没有领域，或只有一个且已选中时不显示；当前项高亮；暂停显示「已暂停」；作品数未知写「未知」；切换只带 `domain` 并停在本页 | `K/DomainPicker.tsx`（站内链接，选后收起，Esc 收起）；数据取自 `K/useCorpusShell.ts` → `GET /api/local/corpus/shell` | ＝；F09 领域在三页之间不丢 |
| A06 | 选择器不渲染时，旧面包屑出现「语料 /  / 创作者」空段 | 无选择器时写「语料 / 创作者」 | △ 修正旧页遗留的排版错误 |
| A07 | 页头读数 `creator-header-readout`：读取中… / 读取于 {时间} / 读取时间未知；失败与未选领域为空 | `C/CreatorsPage.tsx` `readout`，时间走 `humanMoment` | △ 时间统一写法（F15） |
| A08 | 文档标题「创作者 · 语料」、视觉隐藏的 `h1` | `K/CorpusFrame.tsx` | ＝ |
| A09 | 领域校验：地址里的 `domain` 由服务器按 `resolve_current_domain` 解析；无效或缺省不替用户挑领域，显示「请选择领域」，不发列表请求 | `corpus_shell_api.rs`（`selectedDomain`）、`C/CreatorsPage.tsx`（未选不请求） | ＝；旧页对非 UUID 的 `domain` 直接 400，新页按未选处理 |
| A10 | 一个领域也没有：旧页只写「请选择领域。」，没有出口 | 写明「还没有可选择的研究领域」并给「前往领域管理」链接（与证据库旧页一致） | ＋ 补死路 |

#### B. 地址参数与历史

| 编号 | 旧页能力 | React 实现 | 说明 |
|---|---|---|---|
| B01 | 参数：`domain`、`usageRole`、`platform`、`searchMode`、`query`(≤200)、`sort`、`publishedFrom/To`(YYYY-MM-DD，结束日不含)、`traits`(逗号分隔)、`topicHint`(≤120)、`observation`、`minLikes`、`relevance`、`focus`、`viral`、`highLikes`、`page`(从 0 起)、`pageSize`(25/50/100) | `C/creatorsQuery.ts` `parseQuery / toPageSearch / toApiParams`；`creatorsQuery.test.ts` | ＝；名字与接口、证据库 `creatorFilter` 一致；地址只写非默认值；无法识别的值按默认处理，不把页面带坏 |
| B02 | 每次条件变化新增一条历史；刷新与页码越界替换当前历史 | `C/CreatorsPage.tsx` `go(update, replace)`（连续两次同步改动叠加而不是互相覆盖） | ＝ |
| B03 | 后退/前进恢复真实状态（旧页靠 pageshow/popstate，领域变了就整页重载） | 状态完全由地址驱动，无需重载 | △ 实现方式；用户可见行为相同 |
| B04 | 页码超过现有总数：回到最后一页并替换历史；总数为 0 回第 0 页，不循环请求 | `CreatorsPage` `outOfRange`（期间不显示空表） | ＝ |
| B05 | 切换范围时取消旧请求，晚到的响应不覆盖新范围 | TanStack Query：键含完整参数、`signal` 取消、`gcTime: 0`、不用占位数据 | ＝（缓存不当作新事实） |

#### C. 搜索与排序

| 编号 | 旧页能力 | React 实现 | 说明 |
|---|---|---|---|
| C01 | 搜索范围「创作者资料 / 作品内容」；占位符随之变化 | `C/SearchField.tsx` | ＝ |
| C02 | 停止输入 250ms 才查询；中文输入法组词期间不发送，组词结束再提交；回车立即提交 | `SearchField`（`composing` 标志 + 定时器） | ＝ |
| C03 | 有词时显示「清空」，无词时显示快捷键提示「/」；按「/」（没有打开的对话框、不在输入框内）把焦点放进搜索框 | `SearchField`、`CreatorsPage` 的 keydown 监听 | ＝ |
| C04 | 打开筛选前先提交待发的搜索词 | `SearchFieldHandle.flush`（`CreatorsPage.openFilter`） | ＝ |
| C05 | 地址里的搜索词变化（后退、移除条件、快捷视图）时输入框同步 | `SearchField`（按地址派生，不在 effect 里补渲染） | ＝ |
| C06 | 排序下拉：最近收录 / 命中相关作品数 / 命中最高点赞 / 命中相关爆款数（未设爆款标准时禁用） | `C/CreatorControls.tsx` | ＝ |
| C07 | 表头排序：「相关作品」「相关最高赞」，`aria-sort`；筛选或高赞模式下表头改称「命中相关」「命中最高赞」 | `C/CreatorTable.tsx` | ＝ |
| C08 | 「刷新结果」：回到第一页重读（替换历史） | `CreatorsPage.refresh` | ＝ |

#### D. 筛选

| 编号 | 旧页能力 | React 实现 | 说明 |
|---|---|---|---|
| D01 | 筛选按钮及已选条件数 | `CreatorControls` | ＝ |
| D02 | 筛选对话框，草稿语义（点「应用筛选」才生效，取消不改）：材料用途；平台（小红书）；发表日期（全部 / 近 30 天 / 近 90 天 / 自定义，北京时间自然日、结束日不含）；作品表达 4 项复选；作者倾向；观察关系（关系读不到时禁用）；领域相关性；最低点赞；内容方向短语（精确匹配）；有相关爆款（未设标准时禁用）；有高赞线索 | `C/CreatorFilterDialog.tsx`、`creatorsQuery.presetDates` | ＝；平台只有一项，显示为不可改 |
| D03 | 日期校验「结束日期必须晚于开始日期（结束日不含）」 | `creatorsQuery.dateRangeError` | ＝ |
| D04 | 对话框内「清除额外条件」：只重置草稿，仍需应用；同时清掉搜索词与排序 | `CreatorFilterDialog` | ＝ |
| D05 | 已选条件芯片：逐项移除（表达逐项）、「清除筛选」；移除后回第一页 | `CreatorControls`、`creatorsQuery.conditionChips / removeCondition` | ＝【#384 发现 3】 |
| D06 | 「清除筛选」（条件芯片条）与「清除额外条件」（筛选对话框、无命中提示）——即 #384 所称「重置」——只清列表条件（搜索、筛选、排序）与页码；领域、材料用途、平台、发表日期保持不动 | `creatorsQuery.clearExtras` | ＝【#384 发现 3】 |
| D07 | 快捷视图：全部创作者 / 未加入观察 / 未观察的高赞（爆款）作者；`aria-pressed`；点击先清额外条件再套用；有爆款标准用爆款并按爆款排序，否则用高赞线索；观察关系读不到时后两项禁用 | `creatorsQuery.applyQuick / quickPressed`、`CreatorControls` | ＝ |
| D08 | 条件可组合：不同条件取交集，组内任选其一，作品条件在同一篇作品上核对（服务端谓词）；对话框里写明 | `CreatorFilterDialog` 说明文字 | ＝【#384 发现 3】 |
| D09 | 范围统计与筛选结果分开显示：快捷视图与「材料范围」里的数字只跟领域 / 用途 / 平台 / 日期；结果条的「N 位创作者」跟搜索与筛选 | `CreatorControls`、`CreatorScopeDialog` 说明文字 | ＝；文案补明【#384 发现 3】 |
| D10 | 无命中：「当前条件下没有找到创作者。」+「清除额外条件」 | `CreatorTable` | ＝ |
| D11 | 局限提示：自动分析未开启而用了表达 / 倾向 / 方向条件 →「仅检索已有判断」；领域已暂停 →「历史材料仍可读，写入操作不可用」 | `CreatorsPage.describeStatus` | ＝ |

#### E. 结果条与材料范围

| 编号 | 旧页能力 | React 实现 | 说明 |
|---|---|---|---|
| E01 | 结果计数：「N 位创作者」/ 读取中… / 请选择领域 / 读取失败 | `CreatorsPage.resultText` | ＝ |
| E02 | 范围摘要「主研究 / 参照 · 日期范围」；「材料范围」按钮 | `CreatorControls` | ＝ |
| E03 | 材料范围对话框：5 个统计按钮（点击 = 只看此项并清其他条件）；爆款标准说明；范围内作品数、已确认相关作者数；「作者身份待补 N 篇」链接；有日期窗口时「另有 N 篇发布时间未知」按钮；领域名称 · 用途、领域说明、研究目标（缺失写「尚未填写」，不编）；自动分析是否开启 | `C/CreatorScopeDialog.tsx` | ＝【#384 发现 2】只展示接口已提供的；两个统计按钮的名称改用 #384 的语义用词（旧「持续创作倾向」→「本领域持续创作」，旧「有亲历作品」→「有亲历自述作品」），点击行为不变 |
| E04 | 判断标准与并存关系常驻可见：六项判断的门槛；「作品表达」与「作者倾向」分层、可同时成立；分析状态四种含义 | `CreatorScopeDialog`、`C/creatorsPresent.ts` `CRITERIA / COEXISTENCE` | ＋【#384 发现 1、2】 |

#### F. 八列表格

| 编号 | 旧页能力 | React 实现 | 说明 |
|---|---|---|---|
| F01 | 八列：创作者 / 内容方向 / 作者倾向 / 作品表达 / 相关作品 / 相关最高赞 / 作品预览 / 观察状态；固定表头与首列；局部横向滚动 | `C/CreatorTable.tsx` | ＝（以主线 #386 已确认的八列为准；§3.4 的列名是原手册措辞，对应关系：来源特征 → 作者倾向 + 作品表达，爆款 / 表现 → 相关最高赞，观察关系 + 操作 → 观察状态） |
| F02 | 创作者：头像、名字按钮、粉丝数、名称来源「观察目标名称」；缺名写「作者名称未取得」 | `CreatorTable` `CreatorRow` | ＝ |
| F03 | 内容方向（至多 2 条短语；缺失「内容方向尚未形成」；两行截断） | `CreatorRow` | ＝ |
| F04 | 作者倾向 + 分析状态：排队待分析 / 分析中 / 分析失败·原因 / 分析已暂停·原因 / 已分析，依据不足 / 已有分析结果 / 尚未取得当前分析结果 / 自动分析未开启——文字加色点，含义分开 | `C/creatorsPresent.ts` `analysisStatus`、`CreatorRow` | ＝（失败原因翻译为中文）【#384 发现 2】 |
| F05 | 作品表达：首个标签 +N（不含机构 / 品牌）；点击打开「作品与依据」 | `CreatorRow` | ＝【#384 发现 1：与作者倾向分列，表头给出层次说明】 |
| F06 | 相关作品：数量链接 → 证据库（范围内已判相关）；「待判断 N」链接；筛选时显示「命中」并给出「范围内」 | `CreatorRow`、`creatorsQuery.workLink / scopeWorkLink` | ＝ |
| F07 | 相关最高赞：数字（悬停显示精确值）+ 说明（已判相关作品 / 当前命中作品） | `CreatorTable` `Metric` | △ ≥1 万缩写为「x.x万」（LIDS BOUND-001）；未知显示「—」，不是 0 |
| F08 | 作品预览：封面、标题（作品资源标题优先，缺失「标题未取得」）、相关性 · 赞；链接到证据库精确作品 | `CreatorRow` | ＝ |
| F09 | 观察状态文字 + 行内动作：查看目标 / 查看加入方式 / 加入观察 / 关系待读取 | `CreatorTable` `RowAction` | ＝（「加入观察」改为先确认，见 I01） |
| F10 | 抽屉打开期间该行高亮 | `CreatorTable`（`data-selected`） | ＝ |
| F11 | 作者名按钮带 `aria-haspopup=dialog`；关闭抽屉后焦点回到该作者 | `CreatorTable`、`C/findAuthor.ts`、`K/ModalDialog.tsx` | ＝ |

#### G. 分页

| 编号 | 旧页能力 | React 实现 | 说明 |
|---|---|---|---|
| G01 | 「x / y 页」或「0 页」；上一页 / 下一页；跳转到指定页（超界就地提示「请输入 1 至 y 页。」）；每页 25 / 50 / 100；整页只有一套 | `C/Pager.tsx` | ＝ |
| G02 | 换页或改每页数后回到结果起点 | `CreatorsPage`（表格按范围键重建） | ＝ |
| G03 | 加载 / 失败 / 空结果时翻页控件不可用，不产生旧结果跳转 | `Pager`（总数未知时全部禁用） | ＝ |

#### H. 依据抽屉

| 编号 | 旧页能力 | React 实现 | 说明 |
|---|---|---|---|
| H01 | 原生模态对话框；Esc / 关闭；位置在页头下沿，窄屏全宽，随页头高度变化 | `C/CreatorDrawer.tsx`、`K/ModalDialog.tsx`（`setup`） | ＝ |
| H02 | 头部：头像、名字、「领域 · 用途 · 小红书 · 粉丝」、三项数字（相关作品 / 待判断 / 相关最高赞） | `CreatorDrawer` | ＝ |
| H03 | 三个分区：概览 / 作品与依据 / 作者资料；方向键、Home、End 切换；每区记住滚动位置 | `CreatorDrawer` | ＝ |
| H04 | 概览，结论在先：作者内容倾向（结论、调整判断、判定门槛、可展开的理由、来源与时间、分析状态、取样说明、重试、引文）；内容方向与作品表达；当前条件的作品预览（至多 3 篇）；查看全部命中作品；判断标准与材料范围 | `C/DrawerPanels.tsx` `OverviewPanel`、`C/JudgmentBlock.tsx` | ＝【#384 发现 1、2、4】 |
| H05 | 作品与依据，按作品分组（可折叠，展开状态保留）：来源类型 · 收录于 · 指标观察时间、打开原作品、四项判断（领域关系 / 亲历自述 / 知识与方法讲解 / 推广表达）各带门槛、理由、来源、引文；分析状态；重试；处理信息 | `DrawerPanels` `EvidencePanel` | ＝【#384 发现 4】 |
| H06 | 作者资料：公开简介；公开身份线索（机构 / 品牌自述）；材料与来源；处理信息 | `DrawerPanels` `ProfilePanel` | △ 平台账号标识移入折叠的「处理信息」（F06） |
| H07 | 实际取样篇数：字段有就显示（「本次实际使用 N 篇作品摘要；分析当时已有 M 篇」），缺失写「本次实际取样篇数未记录」，不显示成 0 | `creatorsPresent.sampleText` | ＝【#384 发现 2；依赖 #384 后台（已拆为 PR #435）写入 `sampleWorkCount / availableWorkCount`，主线 worker 尚无该字段，前端字段在才显示】 |
| H08 | 引文用原声字体，标明字段来源，作品引文带「回查原作品」链接 | `JudgmentBlock` `Quote` | ＝ |
| H09 | 提交栏固定：观察关系文字 + 动作（加入 / 查看观察目标 / 关系暂不可用）+ 回执始终可见，内容区自己滚动 | `CreatorDrawer` footer、`C/CreatorDrawer.module.css` | ＝【#384 发现 4】 |
| H10 | 领域已暂停：调整判断、重试、加入不可用 | `DrawerPanels`（`writable`）、`CreatorDrawer` | ＝ |

#### I. 写操作

| 编号 | 旧页能力（接口） | React 实现 | 说明 |
|---|---|---|---|
| I01 | 加入观察（行内与抽屉）：`POST /api/local/creators/{creatorKey}/observation-target`，体 `{domain, usageRole}`。其他领域已有主研究关系时只能按参照用途加入；已有的暂停 / 忽略目标保持原状 | `C/CreatorJoinDialog.tsx`、`C/useCreatorWrites.ts` | △ 先弹确认（说明会 / 不会发生什么），确认后才发请求，回执后才显示结果；失败分「未完成」（服务器拒绝，中文原因 + 错误码）与「未确认」（没有答复；重试发送同一请求体——接口没有请求编号，但按作者键 + 领域天然幂等，不会重复登记） |
| I02 | 发现设置：`PUT /api/local/creator-discovery-policy`——点赞标准、自动分析开关、模型配置（读 `GET /api/local/model-settings`）、每日词元上限；带设置版本号，冲突明确提示；模型配置读不到时只存标准；领域暂停不可改 | `C/CreatorPolicyDialog.tsx` | △ 模型配置显示「连接 / 模型」名，不显示编号（F06） |
| I03 | 调整判断：`PUT /api/local/creator-discovery-overrides`——六类判断；正向判断必须选当前支持材料；原因 ≤300 字；恢复自动判断；支持片段按作品分组 | `C/CreatorCorrectionDialog.tsx` | ＝；分组名取片段的 `workTitle`，缺失时用代表作品标题，再缺失写「作品材料 N」【#384 发现 4；`workTitle` 依赖 #384 后台（已拆为 PR #435）】 |
| I04 | 重试失败 / 暂停的分析：`POST /api/local/creator-discovery-analysis/retry`——作品级、作者级；仅在已开启分析且状态为失败 / 暂停时出现 | `CreatorDrawer` `runRetry` | ＝ |
| I05 | 回执绑定发起时的上下文：每个抽屉 / 对话框实例用自己的上下文发起请求；迟到的旧回执不得关闭或改写之后为另一个作者打开的抽屉 / 对话框，只触发列表重读；重复点击只显示进行中，不重复提交 | `C/useWriteGuard.ts`、`useCreatorWrites.ts`（`useJoinPending`）、各对话框与抽屉 | ＋【#384 发现 5】组件测试：先开 A 再开 B，让 A 的回执晚到 |
| I06 | 写后刷新：列表重读但不清空表格；调整判断后抽屉保持打开 | `useCreatorWrites`（写入后使列表查询失效） | ＝ |

#### J. 状态（加载 / 空 / 错误 / 部分 / 受限 / 未知）

| 编号 | 旧页能力 | React 实现 | 说明 |
|---|---|---|---|
| J01 | 读取中 | `CreatorsPage.describeStatus`、表格 `aria-busy` | ＝ |
| J02 | 空：无命中；范围内没有创作者 | `CreatorTable` | ＝ |
| J03 | 读取失败 + 重试读取；不把失败变成空表 | `describeStatus` | ＝ |
| J04 | 观察关系暂不可用：不列入「未纳入」，不可按观察关系筛选或写入；提示「清除观察条件后重试」 | `creatorsPresent.observedText`、`describeStatus` | ＝ |
| J05 | 未设爆款标准：爆款排序 / 勾选禁用；提示「改看高赞线索」 | 同上 | ＝ |
| J06 | 媒体读取失败：文字、链接与查询仍可用 | `describeStatus` | ＝ |
| J07 | 刷新失败但页面上仍是上一次读到的内容：明说不是最新 | `describeStatus` | ＋ 补洞 |
| J08 | 未知不等于 0：粉丝、点赞、取样篇数等未知显示「—」或「未记录」；0 是已确认为零 | `Metric`、`formatCount`、`sampleText` | ＝（LIDS BOUND-001） |
| J09 | 受限 / 撤回：服务端读取层已阻断，页面不缓存原话 | 前端不缓存内容（只在 sessionStorage 记位置，见 L03） | ＝ |

#### K. 媒体

| 编号 | 旧页能力 | React 实现 | 说明 |
|---|---|---|---|
| K01 | 批量读取作品资源（标题、头像、封面），一次至多 100 个：`GET /api/local/work-resources?domain&publicRefs=` | `C/creatorsApi.ts` `fetchWorkResources`、`C/useWorkResources.ts` | ＝ |
| K02 | 图片地址只接受同源的 `/api/local/media/…` 或 `/api/local/derivative/…` 路径；头像只在 `INLINE_SAFE` 时使用 | `creatorsApi.safeAssetUrl` | ＝ |
| K03 | 头像 / 封面的可访问名称与实际加载状态一致：读取中 / 已显示 / 未取得（含加载失败） | `C/Media.tsx` | △ F04 |
| K04 | 封面按 3:4 完整显示（contain），不裁切 | `C/CreatorsPage.module.css` `.cover` | △ 已定改动 |
| K05 | 无头像显示名字首字，无封面显示符号占位 | `Media.tsx` | ＝ |

#### L. 跨页链接与返回

| 编号 | 旧页能力 | React 实现 | 说明 |
|---|---|---|---|
| L01 | 去证据库的精确作品集合：`/corpus/evidence?domain&creatorFilter&returnTo&creatorKey&work`（创作者身份、用途、发表范围、当前作品条件） | `creatorsQuery.workLink / scopeWorkLink / evidenceLink / unknownAuthorLink` | ＝ |
| L02 | 去观察目标：`/collection/targets?domain&drawer=<targetRef>`；去模型设置：`/settings/models` | `CreatorTable.targetHref`、`CreatorPolicyDialog` | ＝ |
| L03 | 从证据库返回恢复位置：表格滚动、作者焦点、抽屉分区与展开项（只存位置，不存内容） | `C/returnState.ts`、`CreatorsPage`、`CreatorDrawer`（`onLeave`） | ＝ |
| L04 | 证据库与评论研究仍是旧页面：侧栏链接整页跳转并带领域；旧证据库页的「返回创作者」依旧可用（地址仍以 `/corpus/creators?` 开头） | `CorpusSideNav` | ＝ |

#### M. 键盘、可访问性与响应式

| 编号 | 旧页能力 | React 实现 | 说明 |
|---|---|---|---|
| M01 | 键盘：对话框模态、Esc 关闭、焦点回到触发者；抽屉分区方向键；表头按钮、芯片可键盘操作；焦点环沿用 LIDS | `K/ModalDialog.tsx` 及各组件 | ＝ |
| M02 | 读屏：结果计数、状态行、回执 `aria-live`；分页、表格、分区有名称 | 各组件 | ＝ |
| M03 | 响应式：1440 / 1280（表格局部横滚）/ 1024 / 390；≤900 侧栏变横向导航带、抽屉全宽；≤640 搜索折行；低矮屏压缩 | `K/CorpusFrame.module.css`、`C/CreatorsPage.module.css` 等 | ＝；浏览器证明量 4 个视口无页面级横向溢出 |
| M04 | 减少动态效果 | 各样式模块 | ＝ |

#### N. 已定的设计改动（§2）落实

| 编号 | 设计改动 | 落实位置 |
|---|---|---|
| N01 | 「加入观察」先确认，说明影响 | I01 `CreatorJoinDialog` |
| N02 | 失败区分「未完成」与「未确认」 | I01 `creatorsFeedback.ts`、`FeedbackLine.tsx` |
| N03 | 头像 / 封面可访问名称随加载状态同步（F04） | K03 `Media.tsx` |
| N04 | 封面 3:4 完整显示 | K04 |
| N05 | 普通层不露机器语言与 UUID，诊断放次层 | 平台账号标识入「处理信息」（H06）；模型配置显示名称（I02）；错误码只作旁注；不外露 `fragmentId / targetRef / creatorKey` |
| N06 | 时间统一 `humanMoment`；未知不是 0 | A07、H05（`provenanceText`）、J08 |
| N07 | 三页外壳一致 | A02–A05（`CorpusFrame`） |

#### O. 「#384 发现」逐条落实

| # | 发现 | 落实 |
|---|---|---|
| 1 | 语义分开：「亲历自述」是作品表达，「本领域持续创作」是作者倾向；依据门槛与可并存关系直接可见 | 表格两列分列且表头给出层次说明（F04、F05）；抽屉「作者内容倾向」与「作品表达」分层（H04、H05）；六项门槛与并存说明常驻在材料范围、筛选、调整判断与抽屉里（E04、D08、H04、I03） |
| 2 | 分析状态区分；显示实际取样作品数；领域说明 / 目标、分析开关有则展示 | 排队 / 失败 / 已分析但依据不足分别显示（F04、H04、H05）；取样篇数（H07）；领域说明、研究目标、分析开关（E03，缺失写「尚未填写」） |
| 3 | 条件可组合、可单独移除；「重置」只清列表条件；范围统计与筛选结果分开 | D05–D09 |
| 4 | 依据抽屉先结论、再按作品分组的作品与引文；提交栏固定可见 | H04、H05、H09；调整判断对话框同样按作品分组、保存栏固定（I03） |
| 5 | 写回执绑定发起时的上下文；重复点击只显示进行中 | I05（含组件测试） |
| 6 | 依赖 #384 后台（已拆为 PR #435）、主线接口没有的字段只记为依赖 | `authorAnalysis.automatic.sampleWorkCount / availableWorkCount`（H07）、支持片段 `workTitle`（I03）：前端字段存在才用，缺失如实说明，不实现后台语义 |

#### P. 建议删除 / 不搬的内容

无能力被删除。仅有两处实现细节不再保留，均不影响用户可见能力：① 旧页为跟上 SSR 页头而在领域变化时整页重载（B03）；② 旧页加入成功后把焦点移到状态行（回执改在确认对话框里显示，I01）。

### 3.2 回执（D1 实施方交付）

> 本回执只陈述实施方在专属工作树内做了什么、验证了什么。协调方的审查、提交、PR、合并与部署另记，此处不预先宣称。验收详情见 [ACC-WEB-REACT-001-D1](../../design/acceptance/web-react-001-d1-acceptance.md)，页面规格见 [PAGE-CORPUS-CREATORS-V1-001](../../design/pages/creators-page.md)。

**状态**：候选实现完成（交付时未提交）；提交与 PR 见下方「协调方复核」。工作树 `.worktrees/web-react-001-d1`，分支 `feat/web-react-001-d1`，基线 `origin/main@36f3fdf9`。

**交付内容**

| 范围 | 内容 |
|---|---|
| 共享外壳（`apps/web/src/corpus/`） | 侧栏 `CorpusSideNav`（与 shell.rs 的 `CORPUS_ENTRIES` 与侧栏标记由测试对照）、领域选择器 `DomainPicker`、页面框架 `CorpusFrame`、模态对话框 `ModalDialog`、外壳读取 `useCorpusShell`；`AppShell` 增加 `collectionState` |
| 只读接口 | `GET /api/local/corpus/shell?domain=`（`corpus_shell_api.rs`，合同见 §3.3） |
| 创作者页（`apps/web/src/pages/corpus/creators/`） | 列表、筛选、搜索、八列表格、分页、依据抽屉、调整判断、发现设置、加入观察（先确认）、材料范围、媒体、返回位置恢复；38 个文件含测试与样式 |
| 页面登记 | `pages.ts`、`App.tsx`（lazy）、`web_app.rs` 的 `REACT_PAGES` |
| 删除旧实现 | `creators.html/.js/.css`、`creators.rs` 里的页面处理、两条 `/assets/creators.*` 路由、页面单元测试 `page_tests`、`scripts/test-creator-approved-ui.py`；JSON 接口全部保留；`shell.rs` 的 `corpus_side_nav` 保留（证据库、评论研究仍用） |
| 测试与证明 | 15 个新增前端测试文件；Rust 路由测试；`creators_browser_tests.rs` 与 `creators-real-chain.mjs`（隔离 PostgreSQL 真实链路，4 个视口）；`scripts/test-web-app-postgres.sh` 串接设置页与创作者页两套证明；`web-app.yml` 补路径与格式清单 |
| 文档 | 本分册 §3.1/§3.2/§3.3、页面规格、验收记录、`docs/README.md`、进度、LIDS 迁移记录、生成物登记；`docs/audits/uiux-content-workbench-evidence-2026-10-14.md` 里两处指向已删文件的链接改为纯文字（历史审计原意不变） |

**验证（全部在实施工作树内实跑）**

| 项目 | 结果 |
|---|---|
| `npm --prefix apps/web run verify` | 类型检查、lint、测试、构建通过；30 个测试文件 328 项通过（基线 129 项 + 新增 199 项） |
| `cargo test -p linggan-api --bins --locked` | 362 通过 / 0 失败 / 67 忽略；基线 356 / 0 / 65：新增 7（外壳接口 6、路由 1）、删除 1（旧页面单元测试）、忽略项新增 2（两个隔离证明）；编译警告 53 条，与基线相同 |
| `scripts/test-web-app-postgres.sh` | 通过：设置页 15 步、外壳接口读库 1 项、创作者页 17 步；一次性容器、数据卷与临时目录已删除；采集请求、工单、模型调用均为 0 |
| rustfmt（`--edition 2024 --config skip_children=true --check`） | 本模块 Rust 文件通过；`local_web.rs` 的既有偏差未动，本次新增行不在偏差之内 |
| `./scripts/check-project-governance.sh origin/main`、`./scripts/verify-ui-design-handbook.sh`、`git diff --check` | 见验收记录 §4 |

**与能力清单的差异**：没有能力被静默删除。有意改动均在 §3.1 以「△」标明（时间写法、≥1 万缩写、先确认再加入、平台账号标识移入「处理信息」、采集状态词取服务器值、头像／封面名称随加载状态、封面 3:4 完整显示等）；「＋」为补旧页缺口或 #384 发现。

**待 Mog / 协调方裁定**

1. **采集状态词（A03）**：旧创作者页与评论研究页恒显示「状态未知」，旧证据库页显示真实值。本次让三页共用外壳都取服务器值（创作者页现在显示「观察中」等）。若要照搬旧页，只需 `CorpusFrame` 不再传该值。
2. **加入观察的结果显示在确认对话框里**（I01）：旧页是关闭后焦点落到状态行；新页的「已完成／未完成／未确认」在对话框内显示，关闭后行内立即变成「已加入」。
3. **#384 后台字段**（H07、I03）：`authorAnalysis.automatic.sampleWorkCount / availableWorkCount` 与支持片段 `workTitle` 主线接口尚无；前端有就显示，没有就写「本次实际取样篇数未记录」「作品材料 N」，没有造数字。这部分后台已拆为 PR #435（Draft）；合入后，建议补一条真实链路断言（合成数据没有自动分析结果，这两项只能由组件测试证明）。
4. **共享文件接缝**：`shared/api.ts` 增加 `PUT`（含一项测试）；`AppShell` 增加可选 `collectionState`（设置页与 `/ops/creation` 的行为不变，既有测试全部通过）；模态对话框放在 `corpus/` 而不是 `ui/`；`FeedbackLine` 在创作者页单独一份，没有为此改设置页。
5. **「FIND」装饰标记**沿用旧页，未新设计。
6. **完整迁移会预置 3 个默认领域**（ADHD、考研自习、自闭症干预）：证明按编号找自己播种的领域，不依赖「恰好 2 个领域」。
7. **本回执之外的主计划表项**：[主计划](web-react-001.md)里「PR #384 …或关闭并入阶段 D」一行的状态词，待 Mog 对 #384 的处置确定后由协调方同步，本次未改主计划。（协调方已同步：见下方协调方复核。）

**协调方复核（2026-10-16）**

- 在同一工作树独立复跑：`npm --prefix apps/web run verify`（30 个文件 328 项通过）、`cargo test -p linggan-api --bins --locked`（362 / 0 / 67）、`scripts/test-web-app-postgres.sh`（设置页 15/15、创作者页 17/17，容器已清理）、治理检查、UI 手册检查、`git diff --check`、本模块 rustfmt，结果与上表一致。`creators.rs` 在主线上已有 4 处 rustfmt 偏差，本次删掉页面处理后剩 3 处，不在 CI 格式清单内，未动。
- 抽查截图发现一处样式回退：「刷新结果」主按钮平时显示成普通描边按钮（`.primary` 的优先级低于 `.page button`），只有悬停才变色；旧页平时是墨色实底。已改为 `.page .primary`，修后重跑前端检查与浏览器证明。
- 核对「加入观察重试沿用同一请求体」的依据：后台按（平台、作者键）`ON CONFLICT DO NOTHING` 加 `FOR UPDATE`，领域关系同样 `ON CONFLICT DO NOTHING`，重复提交得到同一目标与同一关系，成立。
- 第 7 项已同步：Mog 2026-10-15 决定 #384 界面部分作废、调查发现由本阶段落实，后台修复拆为 PR #435（Draft，Refs #367），#384 已关闭；主计划 §7 已改。
- 未执行独立代码审查：审查时机由 Mog 决定。
- 提交状态：提交推送至 `feat/web-react-001-d1`，以 Draft PR 呈交（Refs #433、#430）；未合并、未部署，合并与部署由 Mog 授权。
- 后续（2026-10-16）：Mog 合并 PR #436 为 `baaf5a51`，3000 运行实例随主线更新；协调方上线后只读核验（零写请求、旧资源 404、外壳接口与「采集」状态词、真实数据 1,516 位创作者可读）见 [#433 回执](https://github.com/Himog0921/linggan-intelligence/issues/433)。

### 3.3 接口合同 · 语料外壳只读接口

`GET /api/local/corpus/shell?domain=<领域编号，可选>`（`apps/api/src/local_web/corpus_shell_api.rs`）。

| 项 | 约定 |
|---|---|
| 目的 | 给语料三页的页头面包屑选择器和一级导航「采集」项提供事实：有哪些领域、哪一个被选中、采集有没有观察目标。取代此前由各旧页面在服务器端拼进 HTML 的两件事（`corpus_domain_picker`、证据库页的 `count_targets`），前端不重算 |
| 访问控制 | 与其他本机接口同一道 `local_comment_study_guard`：Host/Origin 必须是本机，跨源或外来 Host 返回 403 `{"error":"local_comment_study_origin_rejected"}`；响应 `Cache-Control: no-store`、`X-Content-Type-Options: nosniff` |
| 方法 | 只有 GET；POST/PUT/DELETE 返回 405 |
| 参数 | `domain` 可选。缺省、空、不是 UUID、不存在，一律「不选中任何领域」（`selectedDomain: null`），不是错误，也不替用户挑第一个 |
| 成功 200 | `{"domains": [{"domainRef", "name", "status", "targetCount", "sampleCount"}], "selectedDomain": "<uuid>\|null", "collectionState": "观察中"\|"无观察目标"\|"状态未知"}` |
| 字段语义 | `status` 为 `active` 或 `paused`；`targetCount` 为本领域观察目标数；`sampleCount` 为本领域作品数，**`null` 表示未知，不是 0**（页面显示「未知」）；`collectionState` 数的是全库的观察目标（不分领域，一级导航「采集」是全局状态）：至少有一个 = 观察中，读到零个 = 无观察目标，读不到数量 = 状态未知（**读不到不能推断为空**） |
| 失败 | 503，体为本机读取错误形状 `{"operation":"local_read","outcome":"unavailable","code":…}`：`read_model_not_connected`（没有连接数据库）、`domain_read_unavailable`（领域读取失败）。页面据此显示「读取失败」，不回退到别的数据 |
| 副作用 | 无。不触发采集、模型或任何写入；隔离库证明里采集请求、工单、模型调用计数保持 0 |
| 测试 | 单元与路由：状态词三种取值、字段与选中解析、无效领域、无数据库 503、来源拒绝、写动词 405（`corpus_shell_api.rs`，6 项）；真实库：`creators_browser_tests::corpus_shell_api_reads_domains_selection_and_collection_state`（隔离 PostgreSQL，含暂停领域与观察目标计数） |

## 4. D2 · 证据库

> **进度**：阶段 1（能力盘点与风险扫描）已完成并写入本节；**尚未写任何实现代码**。阶段 2（实现）等协调方放行；放行前下文标「DR-n」的待决项（§4.8）需要先裁定或接受其「建议」。盘点基线：`origin/main@baaf5a51`（D1 已合并）。

### 4.1 能力清单（迁移前盘点，由实施方填写并在 PR 中逐项核对）

**盘点依据**：旧实现逐文件、逐函数通读——`L/evidence_library.js`（2,661 行）、`L/evidence_observation.js`（490）、`L/research_dossier.js`（217）、`L/research_dossier_batch.js`（72）、`L/evidence_library.css`（600）、`L/evidence_page.rs`（257，仅测试编译）、`L/local_web.rs` 中的 `evidence_library`／`evidence_library_html`／`corpus_domain_choice_dialog`／资源路由、`L/research_dossier.rs`、`L/material_projection.rs`、`L/local_media_routes.rs`、`L/shell.rs`；接口合同 `crates/contracts/src/discovery/query.rs`、`crates/evidence/src/{material_projection,material_projection_types,material_detail_read,work_resource_read,content_reobservation,research_dossier}.rs` 与 `research_dossier/relayer.rs`；规格与走查 [PAGE-EVIDENCE-001](../../design/pages/evidence-library-page.md)、[EVIDENCE-RUNTIME-001](../../design/changes/evidence-runtime-001-ui-change-manifest.md)、[DESIGN-007](../../design/changes/design-007-chinese-first-evidence-library-ui-change-manifest.md)、[DESIGN-012](../../design/changes/design-012-evidence-information-hierarchy-ui-change-manifest.md)、[封面翻转卡](../../design/changes/evidence-cover-flip-card-001-ui-change-manifest.md)、[RESEARCH-DOSSIER-001](research-dossier-001.md) 及其[数据合同](../../data-contracts/research-dossier.md)与[页面清单](../../design/changes/research-dossier-001-ui-change-manifest.md)、[基线审计](../../audits/uiux-content-workbench-baseline-2026-10-14.md)的 F 编号。**以运行中的代码为准**：规格与历史走查文档里与代码不符的地方，以代码为事实并在相应行注明。

**路径简写**：`E/` = `apps/web/src/pages/corpus/evidence/`（本页新建）；`K/` = `apps/web/src/corpus/`（D1 共享外壳）；`A/` = `apps/web/src/app/`；`L/` = `apps/api/src/local_web/`。「React 实现」一栏里的文件名是计划名，实现时可按需要拆合，但能力不得缺项。
**标记**：＝ 照旧；△ 有意改动（理由与来源在行内：§2 已定改动、或仅实现方式不同而用户可见行为相同）；＋ 补旧页的洞；**DR-n** 本行取决于待决项（§4.8），裁定前按其「建议」实施。

#### A. 页面入口、外壳与领域

| 编号 | 旧页能力（行为、接口、状态） | React 实现 | 说明 |
|---|---|---|---|
| A01 | 路径 `/corpus/evidence`：服务器 `evidence_library` 读库后拼整页 HTML；另有 3 条页面资源路由（`/assets/evidence-library.css`、`/assets/evidence-library.js`、`/assets/evidence-observation.js`）和 1 条底稿脚本路由（`/assets/research-dossier.js`） | `L/web_app.rs` 的 `REACT_PAGES`、`A/pages.ts`、`A/App.tsx`（lazy 按需加载）；旧页面处理与 4 条资源路由删除 | ＝；旧资源路径返回 404、`/corpus/evidence/unknown` 仍 404（路由测试）；`/corpus` 的 307 跳转保留 |
| A02 | 全局页头：一级导航 6 项、「本机服务状态」胶囊、个人菜单（`evidence_library_header` → `shell::global_header`） | `K/CorpusFrame` → `A/AppShell`（D1） | ＝；与 `shell.rs` 由 `shellParity.test.ts` 对照（R4） |
| A03 | 页头上下文行读数：静态「事实层 只读／材料入口 默认投影／本机时区 UTC+08」 | 读取成功后「读取于 {humanMoment(asOf)}」，读取中「读取中…」，失败与未选领域为空 | △ F06／F15（机器词不外露、时间统一）；「只读、不触发采集」的声明留在侧栏页脚（A06） |
| A04 | 面包屑：有领域选择器时「语料 / ［选择器］ / 证据库」；没有时「语料 / 证据库 / 作品材料集合」 | `CorpusFrame` 的 `crumb` | △ 没有选择器时与 D1 一致写「语料 / 证据库」，去掉与页面标题重复的第三段 |
| A05 | 领域选择器（details＋链接；只有一个领域且已选中、或没有领域时不渲染；暂停显示「已暂停」；作品数未知写「未知」；切换只带 `domain` 并停在本页） | `K/DomainPicker` | ＝；F09 |
| A06 | 语料侧栏：3 项、序号、`aria-current`、每项带 `?domain=`；页脚「只读本机材料投影／列表与详情不触发采集」（DESIGN-012 把「不触发采集」收窄到读取） | `K/CorpusSideNav` 的 `foot` | ＝ |
| A07 | 侧栏「证据库」项的计数徽标 `#ev-rail-count`：显示本次读取的作品数；悬停说明「本次读取 N 个作品 · 已留存评论 X · 本地材料 Y · 部分或缺口 Z」；未选领域或零作品时隐藏。徽标样式只存在于 `evidence_library.css` | `CorpusSideNav`／`CorpusFrame` 增加可选的徽标参数，样式搬进 `K/CorpusFrame.module.css`；标记在无计数时与 `shell.rs` 逐字相同 | **DR-5**（D1 共享文件接缝） |
| A08 | 文档标题「证据库 · Linggan Intelligence」；视觉隐藏的 `h1`「证据库」 | `CorpusFrame` | △ 标题改为「证据库 · 语料」，与 D1 一致 |
| A09 | 领域校验：地址里的 `domain` 由服务器 `resolve_current_domain` 解析，缺省、无效、不存在一律「未选」，不替用户挑领域 | `K/useCorpusShell` 的 `selectedDomain` | ＝ |
| A10 | 未选领域（且有领域可选）：外壳保持完整，自动弹出模态「选择研究领域」（说明「选择后读取该领域的作品材料；也可随后在页头切换。」）；每项为名称＋「运行中 · N 个目标 · M 篇作品」／「作品数未知」／「已暂停 · 历史材料可读」＋箭头；关闭只回到「请选择研究领域」空态，不自动代选、不读材料；关闭后焦点回到面包屑选择器 | `E/DomainChoiceDialog.tsx`（`K/ModalDialog`），数据取自 `useCorpusShell` | ＝ |
| A11 | 一个领域也没有：对话框写「还没有可选择的研究领域。」＋「前往领域管理 →」（`/collection/domains`）；结果区另有一条同样的内联链接 | 同上 | ＝ |
| A12 | 地址带合法 `work` 但未选领域：对话框里每个领域链接都追加 `&work=<uuid>`，选定领域后继续定位该作品 | `DomainChoiceDialog` 的 `pendingWork`（只接受 UUID） | ＝；F09（入口见 C04） |
| A13 | 数据库不可用／领域读取失败：旧页整页返回无外壳的纯文字页（「Corpus 当前不可读」「领域范围当前不可读」） | 页内显示「读取失败」＋「重试读取」（外壳读取失败同 D1 J03），不把失败显示成空列表 | △ 保持外壳；失败仍如实，不回退到别的数据 |
| A14 | 暂停领域：选择器标「已暂停」，历史材料照常可读；页面不预判写入是否被拒（复观测是否被拒由服务器准入回执给出，见 M05） | 同 `DomainPicker`；不新增提示 | ＝（PAGE-EVIDENCE-001 §5.4） |

#### B. 地址参数与历史（页面自己读写的每一个参数）

| 编号 | 旧页能力（参数、取值、读写规则） | React 实现 | 说明 |
|---|---|---|---|
| B01 | `domain`：读＝服务器按 A09 解析；写＝任何列表刷新、选择、标签切换都原样写回（否则领域「选一次、下一次交互即失效」） | `E/evidenceQuery.ts` 的 `toPageSearch(domainParam, state)` 总是写回 | ＝ |
| B02 | `q`：检索词；只在提交（Enter 或「读取材料」）时进入地址，空则不写；无领域时忽略 | 同 | ＝ |
| B03 | `sort`：`latest_discovery`（默认，不写）｜`relevance` | 同 | **DR-1**（`relevance` 服务器返回 400） |
| B04 | `window`：`latest_accepted_discovery`（默认，不写）｜`last_7_days`｜`last_30_days` | 同 | **DR-1**（后两项读模型恒返回空） |
| B05 | `lane`（9 个通道之一）、`laneState`（14 个状态之一；旧页另外动态追加一项 `RETIRED`）、`mediaKind`（`cover`｜`image`｜`video`｜`live_photo`）：读写；值不在选项里按默认处理，不会上送 | 选项表常量；非法值回默认 | ＝；`RETIRED` 见 **DR-1** |
| B06 | `view`：`all`（默认，不写）｜`partial`｜`risk`｜`cleaned`｜`restricted`；上送时展开为 `laneState=PARTIAL`／`laneState=RISK_CONTROL`／`lane=media_bytes&laneState=BYTES_CLEANED`／`restriction=WITHDRAWN_OR_RESTRICTED`，并**覆盖同名的手选筛选** | `VIEW_FILTERS` 常量 | ＝（覆盖手选筛选是既有行为，DESIGN-012「未做」登记为另立事项，D2 不改） |
| B07 | `layout`：`research`（默认，不写）｜`table`｜`cover`；只重排已读到的集合，不重新请求 | 同 | ＝ |
| B08 | `work`：作品稳定引用（只采用合法 UUID）；有领域才有意义；列表加载后自动选中第一项时也会写回（替换历史）；`work` 不在已读页内时直接按详情读取，不回退到首项 | `EvidencePage`（`directWorkItem` 等价物） | ＝；原由 `creator_lifecycle_tests::corpus_work_deep_link_does_not_fall_back_when_the_work_is_off_page` 的字符串断言守，改由组件测试守（§4.5） |
| B09 | `tab`：`overview`（默认，不写）｜`evidence`｜`dossier`｜`materials`｜`trace`；替换历史 | `E/Inspector.tsx` | ＝；五个取值是深链接合同（RESEARCH-DOSSIER-001 页面清单与 `manual_dossier_frontend_preview` 都用 `tab=dossier`），D2 重排标签内容但不改取值 |
| B10 | `panel`：`closed`｜`normal`｜`wide`｜`focus`；缺省取本机偏好（P01），再缺省 `normal` | 同 | ＝ |
| B11 | `creatorKey`／`creatorFilter`／`returnTo`：创作者作品集合（D1 创作者页生成）；读后原样写回，并随列表请求上送（`returnTo` 只是导航状态，接口忽略它） | 同 | ＝；`returnTo` 不再上送接口（接口本来就忽略） |
| B12 | `collectionRunRef`：本次采集的固定作品集合；原样写回；列表、详情（含按 `work` 直读）、评论、复观测请求都带同一个值 | 同 | ＝ |
| B13 | `cursor`：只在接口层使用，不进地址；「继续读取作品」时用上一次回执里的游标 | `E/evidenceApi.ts` | ＝ |
| B14 | 不在页面地址里的接口参数：`restriction`（只由 `view=restricted` 展开）、`publicRefs`（创作者页批量读取用，本页不用） | — | ＝ |
| B15 | 历史写法：运行查询、切换系统视图或排版、改通道／状态／窗口／媒体类型、用户点选作品＝新增一条历史；改排序＝新增一条并保留选择；检查器宽度与开合、标签页、自动选中首项、从地址还原的选中、追加读取＝替换当前历史 | `go(update, replace)`（D1 同模式：连续两次同步改动叠加而不是互相覆盖） | ＝ |
| B16 | 后退／前进：从地址整体还原查询、视图、排版、标签、宽度、选中并重新读取，保持所选作品 | 状态完全由地址驱动，不需要 popstate 监听 | △ 实现方式；用户可见行为相同 |
| B17 | 地址是唯一依据，缺省＝默认值：后退到不带参数的地址时不保留旧筛选 | 同 | ＝ |
| B18 | 无领域：控件置灰、不发列表请求、不改写地址（登录页真实检查要求落地后 `location.search` 为空） | `EvidencePage` | ＝ |
| B19 | 无法识别的 `layout`／`tab`／`panel`／`view`／`sort`／`window`／`lane`／`laneState`／`mediaKind` 按默认处理；`work` 非 UUID 忽略 | `parseQuery` | ＝ |
| B20 | `collectionRunRef` 不是 UUID、`creatorFilter` 不是合法 JSON：旧页原样上送，服务器 400 后显示成「本机材料读取暂时不可用」 | 校验后改为「地址里的筛选条件无效」＋清除链接，不当成读取失败 | ＋ |

#### C. 入站深链与出站交接

入站链接在切换后必须全部继续有效；下表是它们的确切形状（已对全仓库 `corpus/evidence` 的生成点逐一核对）。**阶段 F／G 迁移各自页面时，须继续生成这些形状**（在本表登记，不靠记忆）。

| 编号 | 来源与旧链接形状 | React 页的处理 | 说明 |
|---|---|---|---|
| C01 | `/` → `/login`；`/corpus` → 307 → `/corpus/evidence`；登录页 `location.assign('/corpus')` | 路由表不变（`corpus_entry`、`login::page`） | ＝ |
| C02 | 语料侧栏与本页领域选择器：`/corpus/evidence`、`/corpus/evidence?domain=<uuid>` | `K/CorpusSideNav`、`K/DomainPicker` | ＝ |
| C03 | 创作者页（D1，React）：`/corpus/evidence?domain=<d>&creatorFilter=<JSON>&returnTo=<创作者页地址>&creatorKey=<key>`，可再带 `&work=<uuid>`；「作者身份待补」集合没有 `creatorKey`（`workLink`／`scopeWorkLink`／`evidenceLink`／`unknownAuthorLink`） | B11；`returnTo` 须为同源路径且以 `/corpus/creators?` 开头才作为「返回创作者」，否则回 `/corpus/creators?domain=<d>` | ＝；D1 e2e 依赖的文字「当前为创作者作品筛选集合」「返回创作者」保持不变 |
| C04 | 观察目标检查器（旧，`L/target_drawer.rs`）：作品行 `/corpus/evidence?domain=<d>&work=<uuid>`；生命周期选中点「到语料页查看这篇作品」`/corpus/evidence?work=<uuid>`（无领域） | A12／B08 | ＝；阶段 G 须继续生成这两种形状 |
| C05 | 采集运行详情（旧，`L/collection_run_workspace.js`）：「查看本次作品与评论」`/corpus/evidence?domain=<d>&collectionRunRef=<run>&returnTo=/collection/tasks?domain=<d>&run=<run>`；「查看作品／评论」在其后加 `&work=<publicRef>` | B12；C11 横幅 | ＝；阶段 G 须继续生成这两种形状 |
| C06 | 主题图谱（旧，`L/topic_map.js`「在材料检查器定位」）：`/corpus/evidence?work=<ref>&domain=<d>` | B08 | ＝；阶段 F 沿用 |
| C07 | 领域管理页（旧，`L/local_web.rs` 领域卡片「查看该领域材料」）：`/corpus/evidence?domain=<d>` | B01 | ＝；阶段 G 沿用 |
| C08 | 浏览器插件：`{LINGGAN_LOCAL_ORIGIN}/corpus/evidence`（`plugins/linggan-intelligence-browser/src/linggan/background.js`，无参数） | B18 | ＝ |
| C09 | 已核对**没有**直接链到证据库的页面：评论研究（旧）、运营（旧）、内容工作台（React）、设置（React） | — | 无需处理 |
| C10 | 「返回创作者」横幅（地址带 `creatorKey` 或 `creatorFilter`）：「当前为创作者作品筛选集合 · 返回创作者」 | `E/ReturnBanners.tsx` | ＝ |
| C11 | 「返回采集任务」横幅（地址带 `collectionRunRef`）：「当前为本次采集的固定作品；详情与评论按本次取得／明确复用版本回查；媒体为共享当前资源。 返回采集任务」；`returnTo` 须以 `/collection/tasks?` 开头，否则回 `/collection/tasks?domain=<d>&run=<run>` | 同上 | ＝ |
| C12 | 出站交接「评论深采」：先把所选作品稳定引用的 JSON 数组写进 `sessionStorage['collection-selected:<领域>']`，再整页跳转 `/collection/tasks?domain=<d>&selected=1&returnTo=<本页路径与查询>`；旧采集页读同一个键，弹窗关闭后按 `returnTo`（须是 `/collection/targets` 或 `/corpus/evidence`）回到本页 | `E/useCollectionSelection.ts` | ＝；键名、数组格式与跳转形状冻结，直到阶段 G 完成（**DR-6**） |
| C13 | 出站：「前往领域管理 →」`/collection/domains`；「整理图片文字」是页内弹窗（L15），不离开页面 | — | ＝ |
| C14 | 页面入口现在经 `local_comment_study_guard`（与 D1 创作者页、设置页同）：Host 必须是 `localhost`／`127.0.0.1`，跨站发起的导航（Sec-Fetch-Site 为 cross-site，例如从外部网页或聊天软件点开本机链接）返回 403；旧入口没有这道闸门 | `L/web_app.rs` 统一加闸 | △ 随页面登记产生的收紧，与已迁移的页面一致 |

#### D. 检索、筛选、排序、系统视图与排版

| 编号 | 旧页能力 | React 实现 | 说明 |
|---|---|---|---|
| D01 | 检索框 FIND：`name=q`、`aria-label=检索作品与证据`、占位「标题、作者或原声片段……」；只在 Enter 或「读取材料」时提交，没有自动搜索和防抖；提交会把系统视图复位成「全部材料」；只检索本机已接纳的材料，不触发平台搜索 | `E/EvidenceControls.tsx`（原生 `<form>`） | ＝ |
| D02 | 「FIND」装饰标记 | 沿用（D1 创作者页有同样先例） | ＝ |
| D03 | 「筛选」按钮＋已选条数徽标（四个下拉中非默认者的个数；系统视图与排序不计）；展开浮层，Esc 或点击外部关闭，打开一个会关掉另一个浮层 | `E/FilterPanel.tsx` | ＝ |
| D04 | 「读取窗口」：最新已接纳发现（默认）／近 7 天来源发布时间／近 30 天来源发布时间 | 原生下拉 | **DR-1**（后两项在当前读模型里恒返回空） |
| D05 | 「材料通道」：全部材料通道＋发现、详情、评论、回复、作者、媒体槽位、媒体字节、图片文字、视频转录 | 同上 | ＝ |
| D06 | 「通道状态」：全部状态＋当前未知、尚未请求、已排队、尚未观察、已观察、部分取得、已取得、处理中、处理器未启用、可检索、执行失败、风险控制停止、字节已清理、撤回或受限；旧页另外动态追加「旧 OCR 已停用」 | 同上 | ＝；「旧 OCR 已停用」见 **DR-1**（服务器不认这个值，选了即 400） |
| D07 | 「媒体类型」：全部类型／封面／正文图片／视频／实况图片 | 同上 | ＝ |
| D08 | 下拉的实现与键盘：旧页自绘 listbox（↑↓ Home End 移动、Esc 收起并还焦点、选后立即重新读取并新增历史） | 原生 `<select>`：键盘与读屏由浏览器承担，变更即重新读取并新增历史 | △ 实现方式（D1 筛选对话框与设置页同做法）；用户可见行为相同 |
| D09 | 排序：弹出菜单，2 项「最近观察」「相关度」，每项带英文技术键 `LATEST DISCOVERY`／`RELEVANCE`；选后保留当前选择并重新读取 | 原生下拉「排序」，不带英文技术键 | △ LANG-05；「相关度」见 **DR-1**（服务器 400） |
| D10 | 「清空筛选」（筛选浮层内）：把窗口、通道、状态、媒体类型置回默认，系统视图复位为全部并立即重新读取；不动检索词、排序、排版 | `FilterPanel` | ＝ |
| D11 | 系统视图 5 个：全部材料／部分取得／风险停止／媒体已清理／撤回或受限（`role=group`「按材料状态快速筛选」，`aria-pressed`）；点选即重新读取并新增历史 | `E/EvidenceControls.tsx` | ＝ |
| D12 | 「我的视图 · 暂无已保存视图」不可交互标签（保存视图能力未接通） | 删除 | △ F16／§2 已定；[PAGE-EVIDENCE-001](../../design/pages/evidence-library-page.md) §5.3、§12 同步修改（规格写的是「保留位置、显示空态」） |
| D13 | 排版切换：研读／表格／封面（`role=group`「结果排版」，`aria-pressed`），不重新请求、不改选择，新增历史 | 同 | ＝ |
| D14 | 读取回执条：只在「扫描预算已触发，当前结果不是全部匹配；可继续读取」或「本次读取已截断，但来源没有给出继续读取的游标」时出现，右侧「扫描 N」；平时读取时间只放在列表容器的 `title` 里 | 回执条保留；读取时间改为可见读数（A03） | ＝；＋读取时间可见（旧页只在 `title` 里，键盘与触屏用户读不到） |
| D15 | 未选领域：检索、筛选、排序、读取、清空、系统视图、下拉全部禁用，`title`「请先选择研究领域，再读取统一作品材料。」；结果区「请选择研究领域」＋说明 | `EvidencePage` | ＝ |
| D16 | 空结果：「当前查询没有匹配的作品材料」＋「读取已经成功；这个结果只描述当前本地查询，不证明平台或现实中没有相关内容。」；空结果时地址里的 `work` 仍按详情直读 | `describeStatus` | ＝ |

#### E. 作品列表（研读／表格／封面三种排版）

| 编号 | 旧页能力 | React 实现 | 说明 |
|---|---|---|---|
| E01 | 列表语义：容器 `role=listbox`「作品材料集合」，每行 `role=option`、`aria-selected`、roving tabindex；行内还含「勾选」复选框（`option` 的子孙在 ARIA 里属装饰性，读屏可能读不到这个复选框） | 语义列表（`ul`／`li`／`article`；表格排版用真 `<table>`）；选择用一个可聚焦的标题按钮（`aria-current`），勾选框是独立控件 | △ 可访问性修正（PAGE-EVIDENCE-001 §11.3「结果使用语义列表／文章」）；行为不变：点击、Enter、Space 选中，↑↓ Home End 在行间移动 |
| E02 | 研读行预览：3:4，只用受控本地句柄（`/api/local/media/`、`/api/local/derivative/`），没有时显示封面状态词；图片 `object-fit: cover` | `E/WorkRow.tsx`、`E/Media.tsx` | △ 封面完整显示（contain，不裁切）：主计划 §4.5 与 D1 K04 已定；PAGE-EVIDENCE-001 §5.1 写的是 cover，规格同步更新 |
| E03 | 研读行标题区：平台（大写）；标题来自封面 OCR 时标「封面 OCR」；参照材料标「reference · 对照」；标题 `h2`（无标题写「标题当前未知」）；作者（头像＋名）与发布日期（紧凑到日） | 同 | ＝；「reference · 对照」改写为中文「参照」（LANG-05；与领域管理的「主研究／参照」同词） |
| E04 | 原声块：来源标签（正文／评论／图片文字／图片实质文字／视频画面文字／视频转录）＋引文；检索命中按字符偏移高亮（`Array.from` 切片，兼容中文与表情）；没有原声写「无原声：尚未取得可引用的原文、评论或图片文字」 | `E/QuoteBlock.tsx` | ＝ |
| E05 | 互动行：点赞／评论／收藏／分享四个实心图标（`role=img`，带中文名）＋千分位数字；未知写「未知」，不是 0 | `E/Engagement.tsx` | ＝ |
| E06 | 材料完整度导轨：5 个维度（正文←详情通道、评论、媒体←媒体字节、图片文字、转录），色块表示已就绪／部分／失败／待处理／未知／不适用；**不适用的维度不进分母**（图文作品不因没有视频而缺一格；尚未看过媒体时仍按未知计入）；悬停提示每格状态；后跟详情通道状态线（点＋中文状态）和「最近观察 YYYY-MM-DD HH:MM」（`title` 给全文） | `E/MaterialRail.tsx` + `E/evidencePresent.ts` | ＝；状态线旁的英文技术码不外露（F06），只留在 `data-state` 与技术事实里 |
| E07 | 作者规则：详情作者名优先；其次 creator 型监控目标的名称（`authorIdentityMatchState=MISMATCH` 时不回退）；关键词型目标永不填作者；都没有写「当前未知」。头像只用 `INLINE_SAFE` 的受控本地副本，没有则显示「作者」占位，可访问名称写头像状态 | `effectiveAuthor` 的 TS 等价物 | ＝（作者合并决定，2026-09-04）；头像可访问名称随加载状态同步（F04，同 D1 K03） |
| E08 | 表格排版：列＝作品／作者／材料／互动（列头放四个图标，行内只有数字并带 `aria-label`）／发布时间／最近观察；旧页列头 `aria-hidden` | 真 `<table>`＋`<th scope="col">`；互动列头保留图标并给文字名 | △ 可访问性（列头不再对读屏隐藏）；列与内容不变 |
| E09 | 封面排版：卡片栅格（≤640px 两列，≤420px 一列），每张卡由「翻转面」＋「铭牌」组成 | `E/CoverCard.tsx`、`E/CoverGeometry.tsx` | ＝ |
| E10 | 封面正面：线稿标记（视频／多图／讨论／三种通用图文，按稳定引用哈希取其一；只是目录标记，不推断内容含义）、标题（至多两行）、「图文／多图／视频／讨论 · 已观察」、「查看封面 →」 | 同 | ＝ |
| E11 | 封面背面：原始封面（3:4）或封面状态词；「原始封面」「受控本地副本」「3:4 COVER」 | 同 | ＝；「3:4 COVER」是英文装饰 → △ 改为「3:4」（LANG-05）；封面完整显示同 E02 |
| E12 | 翻转行为：支持悬停的设备上鼠标悬停即翻；触屏（`(hover: none)`）点击卡面翻转并**不**触发行选择；键盘 Enter／Space 翻转；`role=button`、`aria-pressed`、名称「标题，查看封面／收起封面」；减少动态效果时不动画 | `CoverCard` | ＝（EVIDENCE-COVER-FLIP-CARD-001）；翻转是旋转不是上浮，保留 |
| E13 | 封面铭牌：作者＋发布日（仅有来源文本时写「来源时间」）、互动、状态线（无英文码）＋悬停提示（五维状态；评论维度写「评论 保留数 / 平台数」）、观察时刻「MM-DD HH:MM」（`title` 给全文） | 同 | ＝ |
| E14 | 选中：点击行、Enter、Space；↑↓ Home End 移动并选中且聚焦；选中行高亮 | `E/WorkList.tsx` | ＝ |
| E15 | 自动选中：地址没有 `work` 时选中第一项并写回地址（替换历史）；窄屏（抽屉模式）不因自动选中展开抽屉 | 同 | ＝ |
| E16 | 继续读取：有游标时显示「继续读取作品」，追加在末尾，保持当前选择；追加失败时旧页清空整张表并显示读取失败 | `useInfiniteQuery` 或手动累积 | ＝＋ 追加失败时保留已读到的作品，在列表下方给内联错误与重试 |
| E17 | 读取中：「正在读取本机材料投影／只读取 Linggan 已接纳的作品级材料；不会触发平台搜索或采集。」 | `describeStatus` | ＝ |
| E18 | 读取失败：「本机材料读取暂时不可用／当前没有读取任何作品材料；页面不会回退到旧卡片、远程数据库或平台 CDN。」＋错误码；回执条「当前未读取任何材料，不能据此判断库为空或来源不存在。」 | 同；错误码作旁注，中文说明按码映射 | ＝＋ 加「重试读取」（旧页没有） |
| E19 | 范围一变就取消旧请求，晚到的响应不覆盖新范围，不拿上一个领域的数据占位 | TanStack Query：键含完整参数、`signal`、`gcTime: 0`、不用占位数据 | ＝ |
| E20 | `work` 在已读页内→选中；不在→按 `detailUrl` 直读（保持所选领域与 `collectionRunRef`），不回退首项、不新增历史；`material_not_found` 且来源是地址→「当前领域无法读取这篇作品」＋「这篇作品可能未收录于所选领域，或当前缺少可读取的材料；请切换领域或从作品列表重新选择」 | `EvidencePage` | ＝ |
| E21 | 无总数：读模型不给总数，页面不编造「共 N 个」，只写「本次读取 N 个作品」（徽标与 `title`） | A07 | ＝ |

#### F. 勾选与批量动作

| 编号 | 旧页能力 | React 实现 | 说明 |
|---|---|---|---|
| F01 | 「勾选本页」：勾选当前已加载的全部作品 | `E/SelectionBar.tsx` | ＝ |
| F02 | 「已勾选 N 篇作品」 | 同 | ＝ |
| F03 | 「评论深采」：有领域且勾选 1–200 篇时可用，其余禁用；点击执行 C12 的交接 | 同 | ＝＋ 超过 200 篇时就地说明「最多 200 篇」（旧页只是禁用） |
| F04 | 「整理图片文字」：同条件；打开批量整理弹窗（L15） | 同 | ＝ |
| F05 | 「继续上次整理」：仅在 sessionStorage 里有未完成的批量时出现 | 同 | ＝ |
| F06 | 勾选集合：每次变化写入 `sessionStorage['collection-selected:<领域>']`；刷新、「继续读取」、换检索条件都保留；只存稳定引用，不存内容 | `useCollectionSelection` | ＝（键名与格式冻结，见 C12） |
| F07 | 勾选与检查器选中互相独立；点勾选框不触发行选择 | 同 | ＝ |
| F08 | 复选框名称「勾选作品用于批量操作」 | 同 | ＝（采集页脚本与 e2e 按此名称取） |

#### G. 检查器外壳

| 编号 | 旧页能力 | React 实现 | 说明 |
|---|---|---|---|
| G01 | 桌面两栏：列表＋检查器；宽度三档（标准 408／加宽 640／专注 min(920px, 72vw)；≤1500px 标准降为 390）；「检查器宽度」组（标准／加宽／专注）与「×」；选择记入本机偏好，地址 `panel` 优先 | `E/Inspector.tsx` | ＝ |
| G02 | 关闭检查器（桌面）：结果区占满；再选作品时自动重开；关闭后焦点回到所选行 | 同 | ＝ |
| G03 | 抽屉模式（≤1180px）：检查器变成右侧固定抽屉（宽 min(680px, 94vw)），仅在用户点选或地址还原时展开；Esc 关闭并还焦点；「← 返回当前作品」把所选行滚入视野并聚焦；宽度控件与「×」在此模式隐藏 | 同（抽屉非模态，旧页亦非模态） | ＝ |
| G04 | 头部：作品标题（读屏标题）、作品引用前 8 位（大写）、「打开原文」「补采」两个常驻禁用按钮（`title` 写明原因） | 前 8 位引用移入「技术事实」；两个按钮见 DR-2 | △ F06；**DR-2** |
| G05 | 反馈行：「尚未选择作品」「正在读取当前作品详情」「当前作品详情读取失败」＋错误码；失败时五个标签面板都写「当前详情未读取，列表通道仍可查看，但不能据此补造 Inspector。」 | 同；错误码作旁注 | ＝＋ 加「重试读取」 |
| G06 | 标签页：5 个 `tab`（概览／证据／研究底稿／材料／来源轨迹），roving tabindex，←→ Home End，`tab` 写入地址（替换历史） | 同 | ＝ |
| G07 | 选了另一件作品：取消在途详情读取，重置底稿、复观测、评论状态，面板内容整体替换 | 各面板组件以 `key=publicRef` 重建 | ＝ |
| G08 | 详情地址校验：`detailUrl` 必须同源、以 `/api/local/work-resources/` 开头、不以 `/comments` 结尾，否则写「来源信息不完整：列表没有提供可用的同源 detailUrl」，不猜路由 | `E/evidenceApi.ts` | ＝ |
| G09 | 复观测进入终态后自动重读详情；重读失败写「复观测状态已更新，但作品当前事实未重新读取」 | `useReobservation` | ＝ |
| G10 | 研究底稿标签激活时才读取底稿 | `E/dossier/DossierPanel.tsx` | ＝ |

#### H. 概览：阅读主序与技术事实

旧概览的顺序是：材料完整度 → 「作品字段与来源」事实网格 → 立即复观测 → 来源与溯源 → 互动 → 详情字段当前事实 → 平台计数与本地留存 → 材料通道状态 → 互动时间线 → 作者上下文。正文只出现在「详情字段当前事实」里的一行（`正文字段`，后面跟来源引用）；图片、讨论要切到别的标签。D2 按 §2 的阅读优先（F08）重排，**不新增数据、不新增接口、一项事实都不删**；标签页取值与五个标签的归属不变（B09），标签文字仍用「概览」。

| 编号 | 旧页能力 | React 实现 | 说明 |
|---|---|---|---|
| H01 | 现状：首屏先是完整度导轨，再是字段码、UUID、时间来源字段与精度（见上） | 概览首屏改为 H02–H06 的阅读主序；其余下沉到「技术事实」折叠区（H08） | △ F08（§2 已定） |
| H02 | 阅读主序之一：标题（`display.title`，来自封面 OCR 时照旧标「封面 OCR」）＋作者（头像＋名，规则同 E07）＋发布时间（按精度，H14）＋平台＋四项互动 | `E/panels/ReadingPanel.tsx` | △ F08 |
| H03 | 阅读主序之二：正文。详情 `inspector.detailCurrent.body` 在状态为已知时带完整正文（服务器已给，不需要新接口）；长文折叠为「展开全文」；未取得写「正文通道尚未取得」（不写「没有正文」，DESIGN-012 措辞边界） | 同 | △ F08（旧页把整段正文塞在事实网格的一行里） |
| H04 | 阅读主序之三：图组。受控本地图片的缩略轨道（同 J01），点击进入原尺寸查看器 | 同 | △ F08 |
| H05 | 阅读主序之四：讨论。评论／回复状态与数量、「平台显示 X 条，本地留存 Y 条」并列的一行（两种事实不合并）、「读取评论原文」按钮和评论列表（同 I07–I11）；评论原文仍要用户点击才读取 | 同 | △ F08；不比旧页多暴露 |
| H06 | 材料缺口就近提示：正文、评论、图片、图片文字、转录各维用同一套状态词（N01），缺口写在对应块上，不集中放页尾 | 同 | △ F08 |
| H07 | 阅读主序里的操作：F08 提到「引用操作」，旧页没有「复制引用」一类动作；D2 只把已有动作（预览 AI 输入、对照原图、导出 Markdown／JSON，见 L04、L10）放到阅读主序里就近可达，不新增动作 | 同 | **DR-7** |
| H08 | 「技术事实」折叠区（默认收起），一项不删：稳定引用、平台、标题与状态、时间来源字段／类型／精度／解析器版本、最近观察、当前主要限制、互动当前值·前值·变化、详情字段来源（标题／作者／发布时间／正文各带来源引用）、材料通道九宫格（保留 N／数量未知＋状态线）、互动数据观察时间线、作者上下文 | `E/panels/TechnicalFacts.tsx`、`E/FactGrid.tsx` | △ F06／F08：字段码与 UUID 只在这里；内容与状态码逐字一致，折叠是排布不是改写（DESIGN-012 原则） |
| H09 | 材料完整度：5 维色块＋格下维度名；颜色是状态唯一编码，文字回答「这一格是谁」 | `MaterialRail`（阅读块下方一行） | ＝（位置下移，不删） |
| H10 | 连续的「当前未知」事实行 ≥3 项时折叠成「尚未取得 N 项」，展开后逐条原文案；判据是「技术键为 UNKNOWN 且值读作未知」，已知值不会被折进去 | `FactGrid` | ＝ |
| H11 | 互动数据观察时间线：连续同值观察合并为一条（写次数与首末时间），展开仍可见每次观察；没有观察时点写「当前详情没有互动数据观察时点；不把未知写成 0。」 | `TechnicalFacts` | ＝；时间走 `humanMoment` |
| H12 | 平台计数与本地留存：平台点赞／评论／收藏／分享（状态化）、本地留存评论、已去重评论身份、本地留存边界（旧页直接写停止原因的原始码，如 `risk_control`） | 阅读层写中文：已达到本次配额／当前页面内容已结束／当前页面读取完成／平台风险控制导致停止／用户手动停止／停止原因未知／停止原因未归类，原始码作旁注 | △＋ 词表来自 `evidence_page.rs::stopped_reason_markup`（该函数随旧页删除，词表搬进 `evidencePresent.ts` 并带测试） |
| H13 | 作者上下文：显示名、简介（是否已形成状态化上下文）、关注者数、观察时间；没有合格上下文写「当前详情没有合格作者上下文；这不表示作者不存在。」 | 技术事实＋阅读层作者行 | ＝ |
| H14 | 时间显示：列表层的 `publishedAt`、`lastObservedAt` 服务器已按 `linggan_human_moment` 给出北京时间 `YYYY-MM-DD HH:MM`；互动时间线、覆盖历史、租约到期、作者观察时间等是数据库原串。发布时间精度为 `millisecond｜second｜minute｜day｜relative｜unknown`：日精度只显示日期，`SOURCE_TEXT_ONLY` 写「来源时间：<来源原文>」，未知写「当前未知」 | `evidencePresent.ts` 的 `momentText` | △ F15（原串统一为 `humanMoment`）；不把低精度时间补成整分钟 |
| H15 | 「当前主要限制」：读模型把 `primaryLimitation` 写死为 `OTHER_LANES_NOT_EVALUATED`，不等价于「只跑过发现通道」（DESIGN-012「未做与已知边界」） | 只在技术事实里显示，不作标题结论 | **DR-8**（继续待决，不展示成结论） |
| H16 | 「来源信息不完整」说明：弱化的单行灰字，中文说明＋英文 `SOURCE INCOMPLETE` 旁注 | 中文说明；英文码不外露 | △ F06 |

#### I. 证据：原声、图片文字、讨论

| 编号 | 旧页能力 | React 实现 | 说明 |
|---|---|---|---|
| I01 | 「列表引用的原声」：同 E04 的原声块＋来源类型、选取依据（包含当前检索词／当前最具作者性的可读材料）、是否截断、来源引用、所在媒体对象；没有原声写「这个作品还没有可引用的原文、评论或图片文字。这只描述当前已接纳的材料，不表示平台上没有内容。」 | `E/panels/EvidencePanel.tsx` | ＝；来源引用与所在媒体对象下沉到折叠的技术行 |
| I02 | 「图片文字与转录原文」：说明「以下为机器识别结果，与原图可能有出入。」；每条＝类型·槽位＋引文＋状态线；封面图（按槽位 `purpose==='cover'`，不看键名）标「封面图 · 不宜直接引用」并说明「封面是设计排版，机器识别结果常为噪声；列表引用的原声不采用它。」；没有则写「当前详情没有已识别的图片文字或转录文本；不把缺少记录写成处理成功或失败。」 | 同 | ＝ |
| I03 | OCR 分层信息：OCR 版面记录、分层状态、判定来源（本地规则／视觉选择器）、封面主文案、干净语料资格；已接纳的图片实质文字 | 折叠的「分层详情」 | △ F06（版面记录引用是机器 ID，下沉） |
| I04 | 评论与回复覆盖：评论状态／数量、回复状态／数量（数量未知写「当前未知」） | 同 | ＝ |
| I05 | 评论／回复复观测历史：每个 Package／回执一条，不跨尝试合计；按范围分三组（详情窗口（最多 30 条）／独立全量或其他范围／范围未完整表达）；字段＝范围、请求上限、页面评论数、预期／去重保留、采集／保留／失败、Task／Attempt、Package／Receipt、观察时间；没有历史写「当前没有 评论／回复 覆盖历史；这不表示该通道未曾执行。」 | 技术事实内的折叠区 | ＝（内容）；△ F06（引用下沉）、F15（时间） |
| I06 | 评论研究通道回执：总数／本次返回／是否截断／下一游标；已截断但详情没给下一页地址时写「继续读取入口尚未提供」，不猜后端路由 | 技术事实 | ＝ |
| I07 | 「本机授权评论研究」块：说明「原文只在本机授权详情中按页读取」「作者只显示匿名上下文；页面不会渲染平台用户标识，也不会把评论原文放回普通列表」；按钮「读取评论原文」（通道地址缺失时禁用并写「来源信息不完整」） | `E/DiscussionBlock.tsx` | ＝ |
| I08 | 评论项：匿名评论／匿名回复＋来源引用＋正文（正文状态已知才显示，否则「评论正文当前未知」）＋「本条正文已在读取边界截断／未在通道内截断」 | 同 | ＝；来源引用下沉到 `title` |
| I09 | 评论分页：每页 20 条；回执行「总数 N · 当前 M · 本次 K；本页已截断，可继续读取」；「继续读取评论」 | 同 | ＝ |
| I10 | 评论失败：「评论研究通道读取失败」＋错误码；评论为空：「当前通道没有返回评论材料／这只描述本次授权读取，不表示平台评论为 0。」 | 同；错误码作旁注 | ＝ |
| I11 | 评论文本只在内存：换作品或卸载即清空并取消在途请求；不进地址、不进浏览器存储 | `useCommentChannel` | ＝ |

#### J. 材料：媒体轨道、原尺寸查看器、台账

| 编号 | 旧页能力 | React 实现 | 说明 |
|---|---|---|---|
| J01 | 「本机媒体材料 NN」轨道：说明「采集顺序 · 平台排列顺序未经验证」；卡片是按钮：图片（同源受控路径＋`blob.deliveryState=INLINE_SAFE`＋`image/*`）；有句柄但类型不可内联写「本地副本已验证，但 <类型> 不在可内联集合中」（视频与实况图片的动态部分旧页从不播放，只显示这句状态说明）；否则写字节状态词；序号 NN/MM；元信息 `MED-XXXXXXXX`、「文字 <状态>」／「文字未处理」 | `E/MediaRail.tsx` | ＝；`MED-…` 机器编号下沉到 `title`／技术行（F06） |
| J02 | 轨道交互：上一个／下一个按钮、位置 NN/MM、进度条；鼠标拖拽（位移＞4px 才进入拖拽，纯点击仍是点击）；纵向滚轮转为横向（到边让位）；键盘 ← → Home End；「跳到命中」（所选作品的原声来自某个槽位时出现）；滚动平滑（减少动态效果时直接跳）；单张图片读取失败只在原位提示，其余不受影响 | 同 | ＝ |
| J03 | 没有本地媒体：「当前没有本地媒体对象／这只说明本机还没有留存这个作品的媒体副本；这不表示作品没有媒体。」＋禁用的「补采媒体」 | 同；禁用按钮见 DR-2 | ＝；**DR-2**（禁用占位） |
| J04 | 原尺寸查看器：模态对话框；标题「用途 · 第 NN / MM 个」；副标题「MED-… · 采集序号 N · 平台排列顺序未经验证」；缩放 1×–6×（按钮 −／100%／＋，滚轮以指针为锚缩放，双击在适合与 2.5× 之间切换，放大后拖动平移且平移被夹在溢出范围内）；键盘 ← → 换图、`+`／`-`／`0` 缩放、Esc 关闭；Tab 只在对话框自己的控件间循环；点遮罩关闭；关闭后焦点回到打开它的卡片；打开期间页面不滚动；图注「字节 <状态> · 文字 <状态>」；无可内联副本时写原因 | `E/Lightbox.tsx`（`K/ModalDialog`＋`useZoomPan`） | ＝；副标题的机器编号下沉 |
| J05 | 底稿里的「放大查看原图」走同一个查看器（单图模式：不能前后翻） | 同 | ＝ |
| J06 | 材料处理台账：槽位总数／本次返回／是否截断；逐对象（含作者头像，轨道里不含）＝本地对象、字节状态、文字处理、采集序号（平台排列顺序未经验证）、副本状态 | `E/panels/MaterialsPanel.tsx` | ＝；字段码折叠 |

#### K. 来源轨迹与限制

| 编号 | 旧页能力 | React 实现 | 说明 |
|---|---|---|---|
| K01 | 来源轨迹通道回执：总数／本次返回／是否截断 | `E/panels/TracePanel.tsx` | ＝ |
| K02 | 来源与执行血缘：Package／Task／Attempt／Receipt／Target／Work Order／Producer 引用列表（`code`），缺哪项写「来源信息不完整」；「工位与账号镜头」只显示资格状态，不展示平台账号身份 | 同 | ＝；本面板本来就是查证层，保留机器标识 |
| K03 | 限制与访问边界：详情返回的限制逐条「当前限制＋原码」；已翻译的码：其他材料通道尚未评估／来源信息不完整／采集端没有回报实际使用的候选地址／评论尚未评估／媒体尚未评估／正文原文不在本次读取范围内／该通道尚未评估，其余写「来源未完整表达」；没有限制列表写「当前详情没有返回限制列表；页面不据此推断没有限制。」 | 同 | ＝ |

#### L. 研究底稿（RESEARCH-DOSSIER-001）

| 编号 | 旧页能力 | React 实现 | 说明 |
|---|---|---|---|
| L01 | 入口：标签「研究底稿」；面板初始写「查看这篇作品的完整研究文字、来源与图片分层。」＋「读取研究底稿」；标签激活且面板可见时自动读取；读取中「正在整理当前已取得的研究材料…」并置 `aria-busy` | `E/dossier/DossierPanel.tsx` | ＝ |
| L02 | 读取失败：首次失败写「暂时无法完成，请重试。已取得的原始材料仍保留。」＋「重试读取底稿」；已有内容时刷新失败保留上次可读底稿并提示 | 同；错误码作旁注 | ＝ |
| L03 | 头部：平台·作者·发布时间；「已整理 N 个文字片段 · M 字符 · K 张图片部分待确认」；说明「底稿使用当前可读取的共享材料。历史运行继续保留其冻结输入；待确认行不进入正文。」 | 同 | ＝ |
| L04 | 工具栏：「预览 AI 输入」（滚到任务材料）、「刷新底稿」、「导出 Markdown」、「导出 JSON」、「重新分层（N 个版面）」（0 个或超过 100 个时禁用）；说明「重新分层处理已有版面；封面与首图可能是同一图片，送给 AI 时会去重。不重新识图，也不启动 AI 研究。」 | 同 | ＝ |
| L05 | 布局计划一行：「本篇可用布局 N 个 · 待分层 N 个 · 已是当前版本 N 个 · 需先识字 N 个」 | 同 | ＝ |
| L06 | 重新分层：`POST …/research-dossier/relayer`（语义与三态见 §4.3 W1）；成功后文案「本次 N 个版面：更新 N 个，已是当前版本 N 个，跳过 N 个。」并重读底稿 | `E/dossier/useRelayer.ts` | ＝＋ 成功后让列表与详情查询失效重读（旧页预留了 `onUpdated` 回调却没有调用；封面 OCR 标题、原声摘录可能随分层变化）；失败分「未完成」「未确认」 |
| L07 | 完整度与下一步：原标题／正文（已取得／已确认没有／未知／暂不可用）＋媒体处理结果 x/y 可读取；数字标题疑似点赞数时说明「发现记录中的数字与同次点赞数相同，且未取得详情标题，已排除出平台标题…」；补救指引「缺少原文或原图：先在“材料”查看获取状态，再通过现有采集任务补齐…」 | 同 | ＝ |
| L08 | 「送给研究任务的材料」：任务选择（创作者发现·单篇材料／评论研究·作品上下文／主题研究·分窗前材料）＋各自说明（单篇最多 4,000 字符／作品上下文最多 20,000 字符／分窗前完整材料）；已纳入 N 字符及是否有预算截断；片段折叠（字段名·字符数）；标准 Markdown 折叠；任务材料 JSON 折叠 | `E/dossier/TaskInput.tsx` | ＝；JSON 属诊断，保持折叠 |
| L09 | 「按当前模型预览完整请求」（仅创作者任务）：未配置模型→「尚未配置模型，以上为默认材料预览。」；无法准备→「当前无法准备完整请求，请检查模型输入额度或材料范围。」；成功→输入上限与保守估算上界、实际准备的 Markdown 材料、完整请求与来源映射（含系统提示词，折叠）；不调用模型、不预留额度 | 同 | ＝ |
| L10 | 完整文字：按栏目（平台标题／封面与图片大字／正文／话题标签／图片正文／其他图片文字／视频转写／评论）分块；每个片段「展开全文 · N 字符」（不足 500 字符默认展开）、图片类片段有「对照原图」按钮（展开并滚到对应原图）、「查看来源」折叠（片段编号、来源引用、来源版本）；空栏各写自己的说明 | `E/dossier/Fragments.tsx` | ＝；来源编号折叠即诊断层 |
| L11 | 原图与识别对照：每张图一个折叠块（图 N · 状态 · 分层版本）；左图右文（容器宽 ≤560px 时上下排）；点击识别行→在原图上叠框（归一化坐标须在 0–1）；已排除的水印／界面／不清文字行折叠；「放大查看原图」；另有处理结果时提示；原始识别文字、来源与分层版本折叠；没有原图写「本地原图尚不可用。可在“材料”页查看或补采。」，原图读取失败写「本地原图暂时不可读，请在材料页查看获取状态。」，没有识别行写「尚无可对照的识别行。」，整篇没有图片识别结果写「当前没有图片识别结果。」 | `E/dossier/ImageCompare.tsx` | ＝；原图只接受 `/api/local/media/` |
| L12 | 底稿内的「已采评论」：需有领域；「读取前 30 条」／「继续读取 30 条」；主评论下嵌回复，缺父评论写「父评论尚未读到，暂不拼接其内容。」；状态行「已展示 X / Y 条可读取评论；平台曾显示 N 条。数量相同不代表已采全。」 | `E/dossier/DossierComments.tsx` | ＝ |
| L13 | 互动与来源信息：点赞／评论／收藏（未知写「未知」）＋「底稿版本与观察信息」折叠 | 同 | ＝ |
| L14 | 导出：Markdown／JSON 经浏览器下载，文件名「研究底稿-<作品引用>.md／.json」；单篇、用户显式触发 | `E/dossier/exportDossier.ts` | ＝；不新增批量导出 |
| L15 | 批量整理弹窗（≤200 篇固定作品，原生 `<dialog>`）：「预览／刷新范围」「开始／继续整理」「处理完当前篇后暂停」「关闭」；逐篇列表（引用前 8 位 · 更新 N 个布局／已是当前版本／跳过／N 个待分层／需先识字／本篇处理失败，可重新预览后重试）；暂停只发生在篇间，Esc 视为暂停；已完成结果在数据库保留；刷新后可「继续上次整理」 | `E/dossier/RelayerBatchDialog.tsx` | ＝；失败拆成「未完成」「未确认」（§4.3 W2） |
| L16 | 批量状态保存：`sessionStorage['linggan-research-dossier-batch-v1']`＝`{domain, items:[{work, state, plan, result}]}`（引用、计数与修订哈希，无作品内容；≤200 篇；本标签页内有效） | `E/dossier/batchStore.ts` | ＝（见 P01） |
| L17 | 领域校验：底稿所有读写都按领域校验作品属于该领域，否则 404 `material_not_found` | 页面总是带 `domain` | ＝ |

#### M. 观察记录：立即复观测

| 编号 | 旧页能力 | React 实现 | 说明 |
|---|---|---|---|
| M01 | 可用性：仅小红书作品，且详情的 `channels.reobservation` 给出 `eligible` 与同源地址才出现按钮；否则写原因——平台当前不支持／入口未通过同源校验／尚未具备监控目标关联的有效深度归档授权——并附原因码；不提供不可执行的按钮 | `E/reobservation/ReobservationSection.tsx` | ＝ |
| M02 | 范围说明一行（常驻，不放 tooltip）：「范围：详情、评论（最多 30 条）与回复；不下载媒体、不跑 OCR / 转录。」 | 同 | ＝ |
| M03 | 按钮「立即复观测」；点击发一次 `POST`（无请求体）；已有工单／租约后改为「复观测已入队」并禁用；旁注媒体策略 | `useReobservation`：只由点击触发，不由 Effect 触发（StrictMode 安全） | ＝；确认步骤与重试见 **DR-3** |
| M04 | 三种失败块：「未能创建复观测请求」「复观测状态已更新，但作品当前事实未重新读取」「复观测状态读取失败」（各带原因码），都明说没有把结果写成已排队、已执行或已接纳 | 同 | ＝；与 §4.3 W3 的三态对应 |
| M05 | 执行状态块：准入结果（已按关联授权准入／已合并到覆盖当前作品的在途工作／既有材料已满足请求／请求已记录，等待可用资源／请求已记录，但不在当前可执行范围／请求已记录，尚需决定／准入结论当前未知）、准入决定、准入说明、请求、工单、租约、到期时间、媒体策略；无任务时分「工单已进入立即队列，尚未被合格工位认领；这不是平台已读取、执行或采集成功」和「准入已形成记录，但当前没有可读取的执行 lane」 | 同 | ＝；编号类字段下沉到「技术详情」折叠 |
| M06 | 任务行：通道名＋状态线；任务、评论上限、媒体、认领时间、尝试、包／回执 | 同 | ＝；任务状态里 `PAGE_UNAVAILABLE`、`DETAIL_READ_BLOCKED`，以及执行状态 `NOT_STARTED`，旧页词表没有文案（落进「来源未完整表达」） → **DR-3** |
| M07 | 轮询：有状态地址时每 2.5 秒读一次，全部任务进入终态（已接纳回执／租约结束未见回执／任务结束未见回执／输入不可执行，已停止）即停；终态后重读详情。**当前后端在创建请求时从不返回租约**（`lease_ref` 恒为空、`statusUrl` 为 null），这条轮询分支在真实链路上走不到 | `useReobservation`（保留，用合成回执测试） | ＝；如实记录「当前不可达」，不为它新增接口 |
| M08 | 换作品、卸载：取消在途请求与定时器；本次操作结果不跨作品保留；刷新页面后看不到刚才的操作（没有按作品查询在途复观测的接口） | 同 | ＝；记为已知限制，不新增接口 |

#### N. 状态词与诚实性

| 编号 | 旧页能力 | React 实现 | 说明 |
|---|---|---|---|
| N01 | 状态词表（29 个）：当前未知／仅有来源时间文本／尚未请求／已排队／租约已签发／租约有效／已合并到在途工作／租约已释放／工位已认领／执行中／已接纳回执／租约已过期／租约结束，未见回执／任务结束，未见回执／输入不可执行，已停止／尚未形成观察／已观察／部分取得／已取得／处理中／处理器未启用／可检索／已处理，未识别到内容／执行失败／风险控制停止／字节已清理／旧 OCR 已停用／已撤回或限制读取／本地副本可用；词表外的值写「来源未完整表达」并保留原码 | `E/evidencePresent.ts` 的 `STATE_LABELS`（含色调 info／success／warning／danger／unknown） | ＝；状态用文字＋圆点＋颜色，不只靠颜色 |
| N02 | 完整度导轨的状态归类：就绪＝已取得／可检索／完整／已处理未识别到内容；部分＝部分取得／字节已清理；失败＝执行失败／风险控制停止／已撤回或限制读取；待处理＝已排队／处理中／已观察／尚未形成观察／尚未请求／处理器未启用；其余为未知 | 同 | ＝ |
| N03 | 页面级状态：未选领域／读取中／读取成功但无匹配／读取失败／查询参数无效／详情读取失败／详情不在所选领域／来源信息不完整／材料受限 | `describeStatus` | ＝；各自文案见 E16–E20、G05、B20 |
| N04 | 未知不是 0：互动未知写「未知」，通道数量未知写「数量未知」，评论条数未知写「当前未知」，媒体字节未取得写状态词 | `evidencePresent.ts` | ＝ |
| N05 | 部分是一等状态：`PARTIAL` 保留已取得的材料，不进入「全部失败」；`BYTES_CLEANED` 与可检索的转写可并存，并保留清理说明 | 同 | ＝ |
| N06 | 受限／撤回：只显示状态与限制，不泄漏被限制内容；`view=restricted` 直接筛出 | 同 | ＝ |
| N07 | 不显示作品级「完整度百分比」或总完成状态；不把包接纳（ACK）写成完整 | 同 | ＝ |
| N08 | 读取成功但为零只描述当前查询范围，不外推平台没有内容 | 同 | ＝ |
| N09 | 底稿自带三张词表：片段栏目名（原标题／正文／图片大字／图片正文／视频转写／平台标题／视频画面文字）、处理状态（已整理／部分待确认／待确认／处理失败／已取得／排队中／处理中／本次未识别到文字／尚未知晓／未启用／材料受限／旧识别已停用）、识别行角色（图片大字／图片正文／待确认／平台水印／平台界面文字／识别不清／场景文字） | `E/dossier/dossierPresent.ts` | ＝ |
| N10 | 内联提示的色调：失败（danger）、部分／警示（warning）、访问边界（restricted，用于「本机授权评论研究」）三者分色，让「失败」和「受限」不共用一个视觉（DESIGN-012 B）；提示都是内联文字，不用 toast 或原生 alert 承载必读内容 | `E/InlineState.tsx` + 样式模块 | ＝ |

#### O. 键盘、可访问性、响应式与动效

| 编号 | 旧页能力 | React 实现 | 说明 |
|---|---|---|---|
| O01 | 搜索框的焦点环画在框内（不另加外框），FIND 标记反色 | CSS Modules | ＝ |
| O02 | 命中区 ≥40×40px；可见焦点环；不使用 `tabindex>0` | 同 | ＝ |
| O03 | 浮层规则：一次只开一个；Esc 先关最内层（下拉→筛选浮层→抽屉）；查看器打开时 Esc 只关查看器 | 事件按层处理 | ＝ |
| O04 | 模态对话框用原生 `<dialog>`（领域选择、查看器、批量弹窗）：焦点陷阱与背景惰性由浏览器承担，关闭后焦点还给打开者 | `K/ModalDialog` | ＝ |
| O05 | 读屏：结果状态、回执、反馈区 `role=status aria-live=polite`；`tablist` 语义；图标有中文名；封面卡 `aria-pressed`；头像名称随加载状态同步 | 各组件 | ＝（F04） |
| O06 | 响应式断点：≤1500px 检查器标准宽降为 390；≤1180px 检查器变抽屉；≤900px 查询栏单列、页面自身滚动、表格行改堆叠；≤640px 列表行压缩（预览 56px）、封面栅格 2 列；≤420px 封面栅格 1 列 | CSS Modules（同断点） | ＝；浏览器证明量 4 个视口：1440／1280／1024／390，无页面级横向溢出 |
| O07 | 1280×720 首屏：查询栏三行（检索＋工具、系统视图＋排版、勾选条）之后即可看到第一条作品与检查器标题（F07 的验收口径，不重新设计） | e2e 断言 | ＋ 补验收口径 |
| O08 | 减少动态效果：关闭平滑滚动、翻转与抽屉过渡，状态信息不消失 | `@media (prefers-reduced-motion: reduce)` | ＝ |
| O09 | 悬停提示只作补充：封面铭牌的状态提示是 tooltip（旧页如此）；必读信息（受限、失败、部分）另有常驻文字 | 铭牌状态线常驻 | ＝ |
| O10 | 可访问名称清单（读屏、e2e、采集页脚本都依赖，保持不变）：检索区「材料检索与筛选」；检索框「检索作品与证据」；筛选下拉「读取窗口」「材料通道」「通道状态」「媒体类型」；排序「结果排序」；系统视图组「按材料状态快速筛选」；排版组「结果排版」（研读／表格／封面）；工作台「证据审查工作台」；结果「作品材料结果」；列表「作品材料集合」；检查器宽度组「检查器宽度」（标准／加宽／专注）、「关闭检查器」、「返回当前作品」；标签组「作品证据详情」；查看器「缩放」「缩小」「放大」「上一张」「下一张」「关闭查看器」；轨道「本地媒体对象，左右方向键切换」「上一个媒体对象」「下一个媒体对象」；领域对话框「选择研究领域」「选择语料领域」「关闭领域选择」；勾选框「勾选作品用于批量操作」；互动图标「点赞」「评论」「收藏」「分享」；头像「作者头像＋状态」／「<作者>的头像」 | 各组件按此命名 | ＝；E01、E08 的语义改动不改这些名称 |

#### P. 浏览器存储与敏感材料

| 编号 | 旧页能力 | React 实现 | 说明 |
|---|---|---|---|
| P01 | 浏览器存储，旧页共 3 个键：`localStorage['linggan.evidence.inspectorWidth']`（`normal`｜`wide`｜`focus`，界面偏好）；`sessionStorage['collection-selected:<领域>']`（作品稳定引用数组，勾选交接）；`sessionStorage['linggan-research-dossier-batch-v1']`（批量整理的引用、计数与修订哈希）。三者都不含作品内容；后两个存的是作品稳定引用清单（标识，不是内容也不是滚动位置）：前者是与旧采集页的交接合同（C12），后者是批量整理的断点续做（L15） | `usePanelWidthPreference`、`useCollectionSelection`、`batchStore` | ＝；e2e 断言键名与内容形态（沿用 D1 做法）；若要收紧到「只存位置」，前者须等阶段 G，后者会失去断点续做，需 Mog 另定 |
| P02 | 不进存储与地址：作品标题／正文／原声／评论／底稿文字、检索结果；地址里只有标识与可公开筛选值（`q` 与 `creatorFilter` 里的检索词是用户自己输入的筛选条件，沿用现状） | 同 | ＝ |
| P03 | 评论原文：只经本机授权通道（来源闸门＋领域校验），用户点击后按页读取，匿名、不含平台用户标识，仅在内存 | 同 | ＝ |
| P04 | 列表上的原声只是服务器给出的有界摘录（来源是评论时标「评论」）；完整评论不进列表 | 同 | ＝ |
| P05 | 媒体：只用同源 `/api/local/media/…`、`/api/local/derivative/…`；头像还要 `INLINE_SAFE`；不安全或未知类型不内联；撤回／受限／已清理只显示状态；永不回退到平台 CDN 地址；全页不发跨源请求 | D1 的 `safeAssetUrl`＋e2e 拦截所有非同源请求 | ＝ |
| P06 | 渲染：所有用户内容走 React 文本节点；不用 `dangerouslySetInnerHTML`／`innerHTML`；普通层不渲染 `authorExternalId`、平台用户标识、账号镜头（旧页由对 JS 源码的字符串断言守） | 源码扫描测试 | ＝ |
| P07 | 底稿的元数据含 `authorExternalId`（平台作者编号）与作品平台编号，会出现在「底稿版本与观察信息」折叠、JSON 导出和 Markdown 导出里（`metadata` 原样序列化） | 同（折叠诊断与显式导出内），普通层不渲染 | ＝；**DR-9**（是否收紧待定，D2 不增加暴露面） |
| P08 | 创作者预览的完整请求 JSON 含系统提示词与整份输入：只作折叠诊断显示，不写存储、不写日志 | `TaskInput` | ＝ |
| P09 | 缓存不当作新事实：查询缓存 `gcTime: 0`，不用占位数据；离开页面即丢弃 | TanStack Query（主计划 §4.4） | ＝ |
| P10 | 页面本身没有脱敏逻辑：ADHD／儿童／家庭材料的脱敏与限制由读模型、评论通道和处置传播承担；前端对响应做字段白名单解码，未知字段丢弃，因此不会比旧页多显示任何字段 | `E/evidenceApi.ts` 解码 | ＝（不扩权） |
| P11 | 测试、截图、控制台只用合成数据 | e2e fixture | ＝ |

#### Q. §2 已定设计改动的落实

| 编号 | 设计改动 | 落实位置 |
|---|---|---|
| Q01 | 阅读优先（F08）：标题、作者、正文、图组、讨论在前，技术事实放次层 | H01–H09；旧概览首屏的 UUID、字段来源、时间精度、通道状态降到「技术事实」折叠区；标签页取值不变（B09） |
| Q02 | 机器语言不外露（F06）：状态码、`SOURCE INCOMPLETE`、`MED-…`、引用前 8 位、`PACKAGE / RECEIPT`、`TASK / ATTEMPT`、排序英文键、下一游标、OCR 版面记录引用——普通层不出现 | 诊断放「技术事实」「技术详情」折叠区或 `data-*`／`title`；错误码只作旁注（D1 约定）；见 D09、E06、H08、H16、I03、J01、M05 |
| Q03 | 日期统一为 `YYYY-MM-DD HH:MM` 北京时间（F15） | H14、A03、H11、I05 |
| Q04 | 未知不是 0 | N04 |
| Q05 | 三页外壳一致（R4） | A02、A04–A06 |
| Q06 | 领域在三页之间切换不丢（F09） | A05、A12、B01、C02–C08 |
| Q07 | 删除未接通占位（F16） | D12（「我的视图」）；G04／J03 的常驻禁用按钮见 **DR-2** |
| Q08 | 封面 3:4 完整显示 | E02、E11 |
| Q09 | 头像、封面的可访问名称随加载状态同步（F04） | E07、O05 |
| Q10 | 描述性英文装饰按 LANG-05 去掉 | D09（排序英文键）、E03（`reference · 对照`）、E11（`3:4 COVER`） |

### 4.2 接口清单（本页调用的全部接口；D2 不新增、不修改任何接口）

「闸门」指 `local_comment_study_guard`（Host 必须是本机、跨站请求拒绝、响应 `no-store`）。错误体统一为 `{"operation":"local_read","outcome":"unavailable","code":…}`；页面按 `ApiError`（D1 的 `shared/api.ts`）区分「服务器答复了（http）」「没有答复（network）」「答复内容无法解析（invalid_response）」。

| 编号 | 方法与路径 | 参数 | 页面用到的响应字段 | 失败码与页面呈现 | 闸门 |
|---|---|---|---|---|---|
| API01 | `GET /api/local/corpus/shell`（D1） | `domain`（可选） | `domains[]`（`domainRef`、`name`、`status`、`targetCount`、`sampleCount`）、`selectedDomain`、`collectionState` | 503 `read_model_not_connected`／`domain_read_unavailable` →「读取失败」＋重试 | 有 |
| API02 | `GET /api/local/work-resources` | `domain`（必需）、`q`、`window`、`sort`、`lane`、`laneState`、`mediaKind`、`restriction`、`cursor`、`creatorKey`、`creatorFilter`、`collectionRunRef`（`returnTo` 接受但忽略；未知参数或无法解析的值一律 400，体是纯文本而不是 JSON，前端按「有答复、无错误码」处理）。每页 50 条，扫描预算 200 | `items[]`、`cursor`、`truncated`、`scanLimited`、`scannedCount`、`asOf`。条目：`identity.{publicRef,platform}`、`detailUrl`、`display.{title,titleState,titleSource,creatorState,creatorDisplayName,publishedAt,publishedAtState,publishedAtSourceText,engagement.*}`、`collectionContext.{authorIdentityMatchState,targetKind,targetDisplayState,targetDisplayName}`、`domainUsage.role`、`media.{state,cover,coverCandidates,images,video,avatar}`、`laneSummaries[].{lane,state,valueState,retained}`、`evidenceFragment.{text,sourceKind,matchOffset,matchLength}`、`summary.lastObservedAt` | 400 `domain_required`／`invalid_local_evidence_query`／`invalid_material_query_cursor_or_sort`；404 `unknown_domain`；503 `read_model_not_connected`／`domain_read_unavailable`／`material_projection_schema_unavailable`／`material_read_projection_unavailable` → E18（中文说明＋码旁注＋重试） | **无**（DR-4） |
| API03 | `GET /api/local/work-resources/{publicRef}`（即列表项的 `detailUrl`，或 `work` 直读） | `domain`（必需）、`collectionRunRef` | `item`（同上，另有 `display.publishedAtSourceField/Kind/Precision/ParserVersion`、`summary.primaryLimitation`、`inspector.{detailCurrent,engagementCurrent,engagementTimeline,commentsCoverage,repliesCoverage,commentsCoverageHistory,repliesCoverageHistory,authorContext,sharedCurrentAuthorContext,derivatives,mediaSlots,mediaSlotsReceipt,commentsReceipt,provenance,limitations}`）；`channels.{comments.{url,receipt},media.receipt,derivatives.receipt,provenance.receipt,reobservation.{url,available,supported,eligible,reason,requires,mediaPolicy}}` | 400 `invalid_material_ref`／`domain_required`；404 `material_not_found`（地址来源→E20 专用文案）；503 `domain_material_read_unavailable`／`material_detail_unavailable`／`reobservation_eligibility_unavailable`／`run_material_read_unavailable`／`read_model_not_connected` → G05 | **无**（DR-4） |
| API04 | `GET /api/local/work-resources/{publicRef}/comments`（`channels.comments.url`） | `domain`、`collectionRunRef`、`cursor`（旧页不发 `q`，接口支持 ≤200 字的评论内搜索，本阶段不用） | `items[].{relation（ROOT／REPLY）,sourceRef,body,bodyState,bodyTruncated}`、`total`、`returned`、`truncated`、`nextCursor`；每页 20 条 | 400 `invalid_material_channel_cursor`／`material_comment_query_too_long`；404 `material_not_found`；503 `material_comment_channel_unavailable` 等 → I10 | 有 |
| API05 | `GET /api/local/work-resources/{publicRef}/research-dossier` | `domain` | `dossier.{contract,workRef,revision,asOf,fragments,media,coverage,metadata,sections}`、`relayerPlan`、`markdown`、`inputs.{creator,comments,topics}` | 404 `material_not_found`；503 `dossier_read_unavailable`／`read_model_not_connected` → L02 | 有 |
| API06 | `GET …/research-dossier/relayer-plan` | `domain` | `relayerPlan.{workCount,imageSlots,imageJobs,layoutCount,pendingLayouts,alreadyCurrent,needsOcr,maximumLayoutsPerWork,expectedRevision}`（只在批量整理弹窗里用） | 同 API05 | 有 |
| API07 | `GET …/research-dossier/creator-preview` | `domain` | `available`、`reason`、`stage`、`inputTokenLimit`、`estimatedInputTokensUpperBound`、`system`、`input`、`fragments`、`citationMap`、`markdown`、`sourceFingerprint`（只读，不调用模型、不预留额度） | 503 `creator_preview_unavailable` → L09 | 有 |
| API08 | `GET …/research-dossier/comments` | `domain`（必需）、`cursor`；每页 30 条 | `items[].{sourceRef,parentSourceRef,relation,body,likeCount}`、`total`、`nextCursor` | 400 `domain_required`；503 `material_comment_channel_unavailable` → L12 | 有 |
| API09 | `POST …/research-dossier/relayer`（**写**） | 体 `{"expectedRevision","domain"}`，不允许多余字段 | `workRef`、`layerVersion`、`needsOcr`、`scopeCount`、`updated`、`alreadyCurrent`、`skipped`、`results[]` | 404／409／503／403 → §4.3 W1 | 有 |
| API10 | `POST /api/local/work-resources/{publicRef}/reobserve`（**写**） | 查询串 `domain`；无请求体 | `operation.{publicRef,targetRef,requestRef,decisionRef,admission,admissionReason,workOrderRef,leaseRef,expiresAt,execution,media,tasks[]}`、`statusUrl` | 400／404／409／422／503 → §4.3 W3 | **无**（DR-4） |
| API11 | `GET /api/local/work-resources/{publicRef}/reobserve/{leaseRef}`（`statusUrl`；当前不可达，见 M07） | `domain` | `operation.{leaseState,expiresAt,media,tasks[]}` | 404 `reobservation_not_found`；503 `reobservation_status_unavailable` → M04 | **无**（DR-4） |
| API12 | `GET /api/local/media/{materializationRef}/{sha256}`、`GET /api/local/derivative/{derivativeRef}` | — | 图片字节（只作 `<img src>`；前端只接受这两个同源前缀） | 404 `local_media_not_found`／503 → 就地写读取失败（J01） | 无（句柄本身是资格凭证） |

**页面不使用**：`GET /api/local/work-resources?publicRefs=`（创作者页批量读取）、`GET /api/local/model-settings`（创作者预览已在 API07 里带回模型配置结果）。

### 4.3 写路径语义与回执呈现（已读后端实现）

本页有 2 个**服务器写**（W1 重新分层、W3 立即复观测），1 个**批量重复 W1**（W2），1 个**纯浏览器侧交接**（W4）。D1 的约定沿用：回执只在服务器答复之后显示成功；写请求从不自动重试；重复点击只显示进行中；回执绑定发起它的作品与对话框（`useWriteGuard`），晚到的旧回执不得改写之后选中的另一件作品。三态沿用 D1 的字样「已完成／未完成／未确认」，但有一处口径更严：D1 的 `feedbackForError` 把「得到 2xx 却看不懂内容」归入「未完成」；本页的写操作（W1、W3）把它归入「未确认」——写入可能已经发生，不能当成失败。D2 自带一份 `evidenceFeedback.ts`，不改 D1 的文件。

**W1 · 重新分层** `POST /api/local/work-resources/{ref}/research-dossier/relayer`

| 项 | 约定 |
|---|---|
| 请求 | `{"expectedRevision": "<底稿修订哈希>", "domain": "<领域>"}`（驼峰、禁止多余字段）。`expectedRevision` 来自刚读到的底稿或 `relayer-plan`；旧页总是带领域，新页也总是带 |
| 范围与锁 | 作品必须属于该领域，否则 404 `material_not_found`。单事务，按作品取 `pg_advisory_xact_lock`；读取当前底稿，修订哈希与 `expectedRevision` 不同 → 409 `dossier_changed_or_scope_unavailable`（同一个码也用于：作品不存在、可分层版面超过 100 个） |
| 幂等 | **没有请求编号。** 写入是追加「（版面，规则版本）」唯一的分层结果，`ON CONFLICT DO NOTHING`：同一规则版本重复提交不会产生第二条，只计入 `alreadyCurrent`。所以重复提交是安全的。但要注意：第一次真的更新了（`updated>0`）之后，底稿修订哈希变了，第二次带着旧 `expectedRevision` 会得到 409——**这个 409 不表示第一次失败**，它只是说「你手里的修订已经过期」 |
| 回执 | `{workRef, layerVersion, needsOcr, scopeCount, updated, alreadyCurrent, skipped, results[]}`。`updated`＝本次新写入的版面数；`alreadyCurrent`＝已有该规则版本、无变化；`skipped`＝版面缺少可用识别行、未处理（`results[].reason=layout_lines_unavailable`）；`needsOcr`＝没有任何识别结果的图片槽位数，**本动作不处理它们**（需要先识字／补采）。这个接口**没有**「HTTP 200 但被拒」的变体；拒绝一律是非 2xx 的 JSON 错误体——其 `operation` 字段写的是 `local_read`（共用读错误的外形），页面不能因此把一次写入当成读取。请求体不合格时是 axum 默认的纯文本 4xx，不是 JSON，前端按「有答复、无错误码」处理（归入「未完成」） |
| 副作用 | 只追加分层结果；不重新识字、不采集、不调用模型、不改原始识别行；失败时整个事务未提交，**没有任何写入** |
| 已完成 | HTTP 200。文案「本次 N 个版面：更新 a 个，已是当前版本 b 个，跳过 c 个。」`updated=0` 且 `alreadyCurrent>0` 是「已是当前版本」（无变化也算完成）；`skipped>0` 明说这是部分处理（c 个版面缺识别行，没有处理）；`needsOcr>0` 另说「仍有 d 张图片没有识别结果，需先识字或补采，本操作不会识字」。完成后底稿、列表、详情查询一并失效重读 |
| 未完成 | 服务器给了明确答复（404／409／503／403）。一律「本次没有写入」。409 加「材料已变化，请刷新底稿后重试（如果刚才那次其实已经生效，刷新后会显示已是当前版本）」；503 `dossier_relayer_failed` 等「暂时无法完成，已取得的原始材料仍保留」；错误码作旁注 |
| 未确认 | 没有得到答复（断网、超时），或得到 2xx 却看不懂内容（写入可能已经发生，不能当成失败）。文案「请求没有得到答复，结果未确认，可能已经生效」。**不自动重试，也不拿旧的 `expectedRevision` 盲目重发**：主按钮是「刷新底稿」（读到 `pendingLayouts=0` 即已生效）；刷新后仍待分层，用户再点「重新分层」，此时带的是新读到的修订 |
| 进行中 | 按钮置灰（旧页同）；只由点击触发，不由 Effect 触发（StrictMode 安全） |

**W2 · 批量整理**（逐篇重复 W1）：先对每篇 `GET relayer-plan` 取得各自的 `expectedRevision`，再逐篇 `POST relayer`。任何失败都把该篇标成失败并丢弃旧计划，必须「重新预览」后才能重试——这本来就是「先核对再重试」。新页把失败拆成「未完成」（有答复）与「未确认」（无答复），两者的重试路径相同（重新预览）。暂停只发生在篇间；已完成的篇在数据库保留，刷新后可继续。

**W3 · 立即复观测** `POST /api/local/work-resources/{ref}/reobserve?domain=<领域>`

| 项 | 约定 |
|---|---|
| 请求 | 无请求体；领域在查询串里。目标与用途由服务器从该作品关联的有效深度归档授权推导，标题、作者名、URL 从不当作授权 |
| 前置与失败 | 仅小红书。404 `material_not_found`（作品不存在或不在该领域）；422 `reobservation_platform_not_supported`；409 `reobservation_authorization_not_linked`（没有关联授权）、`reobservation_admission_unavailable`、`reobservation_lease_unavailable`；503 `reobservation_unavailable`／`read_model_not_connected` |
| 幂等 | **没有请求编号。** 单事务内持目标锁：同一范围若已有「尚未被工位认领」的工单，则合并（`admission=MERGE`，返回同一个 `workOrderRef`），并发点击不会产生并行排队工单；**但一旦那张工单被工位认领过，之后再点会新建一张工单**。所以：未确认之后的重试**不是无条件安全的**（上一张若已被认领，重试会再形成一张，工位会再访问一次平台） |
| 回执与「被拒」 | **HTTP 200 可以带被拒的回执。** `operation.admission` ∈ `ADMITTED`｜`MERGE`｜`REUSE`｜`DEFERRED`｜`REFUSED`｜`DECISION_REQUIRED`｜`UNKNOWN`：只有 `ADMITTED`、`MERGE` 带 `workOrderRef` 且 `execution=QUEUED`；`REUSE` 是「既有材料已满足请求」，不新建工作；`DEFERRED`／`REFUSED`／`DECISION_REQUIRED`／`UNKNOWN` 是「请求已记录，没有形成工单」（`workOrderRef` 为空、`execution=NOT_STARTED`）。**每次点击（包括被拒的）都会留下一条请求记录与准入决定。** `leaseRef`、`statusUrl` 在请求时恒为空（租约要等工位认领才形成） |
| 副作用 | 只写本机记录（请求、决定、工单）；不访问平台、不下载媒体、不排 OCR／ASR。真正的平台访问要等合格工位认领，之后才有包与回执 |
| 已完成（已受理） | 200 且 `admission` 为 `ADMITTED`／`MERGE`（有 `workOrderRef`）→「已入队：复观测工单已形成，尚未由工位认领。这不是已读取，也不是已刷新。」；`REUSE` →「既有材料已满足请求，没有新建工作。」（沿用旧页：只有出现工单或租约引用时才把按钮改成「复观测已入队」并禁用） |
| 未完成 | 非 2xx 的明确答复，或 200 但 `admission` 为 `DEFERRED`／`REFUSED`／`DECISION_REQUIRED`／`UNKNOWN` →「请求已记录，但没有形成工单」＋准入说明（`admissionReason` 服务器原文）；不得写「已排队」。旧页对这几种 200 回执已经逐条翻译（`admissionCopy`），新页保持 |
| 未确认 | 没有得到答复，或得到 2xx 却看不懂内容 →「请求没有得到答复，结果未确认；可能已经入队」。**不自动重试，不提供一键重试。** 建议提供「重新核对」（重读详情，来源轨迹里的 Work Order 引用出现即说明已入队）；确需再发时用二次确认并写明上述后果。是否采用该确认步骤见 **DR-3** |
| 进行中 | 按钮置灰；只由点击触发（StrictMode 安全）；换作品或卸载时取消在途请求（服务器可能已处理，所以不显示「已取消」） |

**W4 · 勾选交接（「评论深采」）**：本页只做两件浏览器侧的事——把勾选集合写进 `sessionStorage['collection-selected:<领域>']`，再整页跳转 `/collection/tasks?domain=<d>&selected=1&returnTo=<本页地址>`；不调用任何接口，真实动作与回执由采集页负责。键名、数组格式、跳转形状冻结到阶段 G 完成（C12、DR-6）。

### 4.4 删除清单（同一个 PR 删除，R2）与共享核查

**共享核查方法**：对 `apps/`、`crates/`、`plugins/`、`scripts/`、`.github/` 全文检索每个待删资产的文件名、路由、导出符号与 `window.Lingggan*` 全局名；下表「共享核查」一栏是结果。结论：**待删的页面资产没有任何未迁移页面在用**；真正外溢的只有两处（`.ev-rail-count` 徽标标记，与 `collection-selected:` 交接键），它们不是资产共享，而是跨页契约，已在 A07、C12 登记。

| 路径 | 规模 | 处理 | 共享核查 |
|---|---|---|---|
| `L/evidence_library.js` | 2,661 行 | 删除 | 只由本页模板经 `/assets/evidence-library.js` 加载；其他页面不引用 |
| `L/evidence_library.css` | 600 行 | 删除 | 只由本页 `<link>` 加载，`shell.css` 不依赖它。**唯一外溢**：`.ev-rail-count` 徽标样式——`shell.rs::corpus_side_nav` 仍输出（隐藏的）徽标标记，评论研究页在用；样式搬进 `K/CorpusFrame.module.css`，`shell.rs` 一字不动 |
| `L/evidence_observation.js` | 490 行 | 删除 | 仅本页 |
| `L/evidence_observation.test.mjs` | 128 行（3 个测试） | 删除，改写为 Vitest | 由 `scripts/test-local-001-discovery-postgres.sh` 第 8 行调用，该行同时删除 |
| `L/research_dossier.js`、`L/research_dossier_batch.js` | 217＋72 行 | 删除 | 仅经 `/assets/research-dossier.js` 被本页加载；`window.LingganResearchDossier`、`LingganDossierBatch`、`LingganEvidenceObservation` 全仓库只有本页使用 |
| `L/evidence_page.rs` | 257 行 | 删除；`mod evidence_page`（仅测试编译）声明一并删 | 生产代码零引用（渲染函数只被 `#[cfg(any())]` 停用的测试和一个活跃测试用到）；`stopped_reason_markup` 的词表迁到 TS（H12） |
| `L/local_web.rs` 页面部分 | 约 360 行 | 删除：`EVIDENCE_LIBRARY_CSS`／`EVIDENCE_OBSERVATION_JS`／`EVIDENCE_LIBRARY_JS` 三个常量；`/corpus/evidence` 路由与 `evidence_library`、`CorpusSurfaceParams`、`corpus_message_html`、`corpus_domain_choice_dialog`、`evidence_library_header`、`evidence_library_html`（约 200 行 HTML 模板）、`corpus_nav_domain`；`stylesheet`、`evidence_library_script`、`evidence_observation_script` 及其 3 条 `/assets/*` 路由；两个 `#[cfg(test)]` 辅助函数 | **保留**：`corpus_entry`（`/corpus` 跳转）、`evidence_library_json`（API，名字仍叫 evidence，实为列表接口）、`corpus_domain_picker`／`render_domain_picker`（评论研究、主题图谱、采集页在用）、`html_escape`、`LIDS_TOKENS`／`SHELL_CSS`（其他页面样式在用） |
| `L/research_dossier.rs` | `script()` 11 行＋2 个 `include_str!` | 删除 `script()` 与 `/assets/research-dossier.js` 路由 | 其余是 API，保留 |
| `L/tests.rs` | 30 个测试（1,319 行）删除，12 个测试（502 行）改写 | 见 §4.5 | 14 个是 `#[cfg(any())]` 早已停用的旧发现卡测试 |
| `L/creator_lifecycle_tests.rs` | 1 个测试（约 40 行） | 删除，改由组件测试守 | 该文件其余测试（目标检查器的作品链接形状）保留 |
| `L/research_dossier_tests.rs` | 1 个测试删除，1 个改写 | 见 §4.5 | HTTP 测试保留 |
| 文档中指向待删文件的 Markdown 链接 | 3 处（`docs/plans/active/domain-unification-001.md`、`docs/plans/active/intelligence-product-001-implementation-plan.md`、`docs/audits/uiux-content-workbench-evidence-2026-10-14.md`） | 链接改为纯文字并注明已随 D2 删除（治理检查会拦断链；D1 同做法）；其余历史文档里的代码式文件名不动 | 已用 `grep` 对 `docs/`、`database/`、根目录 Markdown 核对，仅此 3 处 |

**不删（共享或 API）**：`L/material_projection.rs`、`L/local_media_routes.rs`、`L/local_asset_delivery.rs`（运营封面也走它）、`L/shell.rs`＋`shell.css`（评论研究页仍用；`corpus_side_nav` 与 `CorpusPage::Evidence` 保留）、`L/lids_tokens.css`、`L/corpus_shell_api.rs`、`crates/evidence` 与 `crates/contracts` 的全部读写代码。

### 4.5 依赖旧证据库 DOM 或资源的脚本与测试

**(a) 删除的测试各自守的契约，以及它们在新实现里由什么来守**（R5：改写，不是丢弃）

| 旧测试（删除） | 它守的契约 | 新的守法 |
|---|---|---|
| `tests.rs`：`evidence_runtime_uses_material_projection_as_its_only_default_read_source` | 只读 `/api/local/work-resources`，不碰旧路由，不发外域请求；复观测与主页面分模块；「立即复观测」「尚未具备…授权」文案；终态后重读详情 | `evidenceApi.test.ts`（只发同源 `/api/local/` 请求）、`Reobservation.test.tsx`；e2e「全程无跨源请求」 |
| `evidence_library_requires_a_domain_and_uses_work_resources_only` | 无领域不读取；参照标记；没有跨行业旧接口 | `EvidencePage.test.tsx` |
| `evidence_runtime_restores_system_and_personal_view_strategy_without_faking_saved_views` | 系统视图展开表；排版参数；不假装有保存视图；表格比研读行紧凑 | `evidenceQuery.test.ts`（`VIEW_FILTERS`、`toApiParams`）、`EvidenceControls.test.tsx`；「我的视图」按 D12 删除，测试断言它不存在 |
| `evidence_runtime_renders_only_controlled_media_handles` | 只渲染受控本地句柄；不渲染远程封面 | `Media.test.tsx`（含 `safeAssetUrl` 用例）；e2e 拦截所有非同源请求 |
| `evidence_runtime_shows_one_author_field_and_renders_avatar_from_media`、`…keeps_collection_lineage_in_the_inspector_only` | 单一作者字段；creator 目标回退、MISMATCH 不回退、关键词永不填作者；来源监控放检查器 | `evidencePresent.test.ts`（`effectiveAuthor`）、`ReadingPanel.test.tsx` |
| `evidence_runtime_uses_the_xhs_portrait_cover_ratio_in_visual_layouts` | 小红书封面 3:4 | e2e 计算样式探针（容器宽高比 0.75±0.02） |
| `evidence_cover_layout_keeps_a_stationary_data_plate_and_one_shared_flip_stage` | 翻转卡行为：触屏点击翻转且不触发行选择、键盘翻转、几何标记由稳定引用哈希选取、观察时刻紧凑、来源时间不撑破铭牌 | `CoverCard.test.tsx`、`coverGeometry.test.ts` |
| `evidence_cover_card_layers_by_surface_and_keeps_its_materials_apart`（31 条 CSS 字符串断言） | 封面卡的层级、材质、标题字号等视觉细节 | **不搬字符串断言**（它们钉的是旧类名与旧写法）；改由 `tokenDiscipline.test.ts`（无写死值、引用的 token 均有定义）＋ 四个视口的静止态截图人工核对＋少数计算样式探针 |
| `evidence_results_band_stays_removed` | 已删除的结果带不回流；读取时间可核对 | 新 DOM 本来没有；读取时间改为可见读数（D14） |
| `evidence_runtime_preserves_unknown_partial_and_restricted_states` | 诚实状态词；「数量未知」；不出现「ACK 完整」 | `evidencePresent.test.ts`（每个状态词一条） |
| `evidence_runtime_exposes_ocr_layering_without_reintroducing_invalid_tokens` | OCR 分层信息可见；样式不引用失效 token | `EvidencePanel.test.tsx`；`tokenDiscipline.test.ts` |
| `evidence_runtime_keeps_sensitive_text_and_media_inside_controlled_detail_reads` | 评论原文只在授权通道；不渲染 `authorExternalId`；`INLINE_SAFE`；不使用 `innerHTML` | `sensitiveSurface.test.ts`（对渲染源码扫描 `dangerouslySetInnerHTML`／`innerHTML`／`authorExternalId`）、`DiscussionBlock.test.tsx` |
| `evidence_runtime_has_bounded_continuation_keyboard_and_mobile_contracts` | 游标续读、方向键、减少动态效果、移动断点、命中区 | `WorkList.test.tsx`、`Inspector.test.tsx`；样式里 `prefers-reduced-motion` 与断点存在性检查；e2e 四视口 |
| `discovery_stop_reasons_keep_raw_codes_but_lead_with_truthful_chinese_meaning` | 停止原因词表 | `evidencePresent.test.ts`（7 条） |
| `creator_lifecycle_tests.rs`：`corpus_work_deep_link_does_not_fall_back_when_the_work_is_off_page` | `work` 不在首页时直读，不回退首项，不新增历史 | `EvidencePage.test.tsx` |
| `evidence_observation.test.mjs`（3 条） | 复观测控制器：不为没有租约的回执编造轮询；换作品取消定时器与请求；`INPUT_BLOCKED` 是终态且终态后重读一次 | `useReobservation.test.tsx`（同样三条） |
| `research_dossier_tests.rs`：`dossier_assets_and_page_tab_are_served_without_a_database` | 旧底稿脚本被服务 | 删除；`web_app_tests` 新增 4 条旧资源路径返回 404 |
| 14 个 `#[cfg(any())]` 停用的旧发现卡测试 | （早已不运行） | 随 `evidence_page.rs` 删除 |

**(b) 其他脚本、测试与流水线**

| 编号 | 对象 | 它检查什么 | 处理 |
|---|---|---|---|
| S01 | `L/tests.rs` 其余 12 个测试 | 把旧证据库页当「一个语料页样本」来核对共享外壳：页头六项、采集状态词、导航不指向子页、已提供页面互相可达、页头字阶、主要导航读数；以及把 `evidence_library.css` 列在页面样式清单里的三条 CSS 规则测试 | **改写，不丢**：样本改为直接渲染 `shell::global_header(PrimarySurface::Corpus, …)` 与 `shell::corpus_side_nav(…)`（断言文本不变）；样式清单去掉 `evidence_library.css` 一项；`corpus_domain_picker…` 改为直接调用 `corpus_domain_picker`；`corpus_without_a_selected_domain…` 只保留侧栏带领域那一段；令牌基线测试去掉 `EVIDENCE_LIBRARY_CSS` 一行；保留 2 个 API 级测试（旧入口 404、未知参数 400） |
| S02 | `scripts/test-local-001-discovery-postgres.sh` 第 8 行 | `node --test …/evidence_observation.test.mjs` | 删除该行；其余 LOCAL-001 接口／数据库证明不受影响 |
| S03 | `scripts/test-research-dossier-postgres.sh` | `--http` 跑 `dossier_http_*`（API，不变）；`--preview` 起合成数据的人工预览（`manual_dossier_frontend_preview`） | `--http` 不变；预览现在服务的是 React 页，需要已构建的前端产物：脚本在 `--preview` 前补构建或明确报错；预览地址形状不变 |
| S04 | `scripts/test-collection-single-run-ui.cjs` 第 107–133 行及其 harness `scripts/test-collection-single-run-postgres.sh` | 采集页点「查看作品」进证据库后，用 `input[data-collection-work-select]` 数 5 个、50 个，刷新后仍勾选 50 个，`#ev-next-list`，按钮「勾选本页」「评论深采」，再核对评论深采弹窗 | 选择器改为角色与可访问名称（复选框「勾选作品用于批量操作」、按钮「继续读取作品」「勾选本页」「评论深采」，F08 已保证名称不变）；harness 起的是 Axum 路由，React 入口需要前端产物，须先构建。**与阶段 G 的分工见 DR-6** |
| S05 | `apps/web/test/e2e/creators-real-chain.mjs`（D1） | 「侧栏进入证据库（仍是旧页面）再回创作者页…」一步用 `.ev-main`，并核对「当前为创作者作品筛选集合」「返回创作者」「collection-selected」存储键 | 改为等待新页的稳定控件；横幅文字与链接名保持不变（C10）；存储断言的正则保持 |
| S06 | `apps/web/test/e2e/settings-real-chain.mjs` | 步骤名「语料仍由旧服务提供」、截图名 `legacy-corpus-…`；等待条件只是一级导航 | 只改文字 |
| S07 | `scripts/login-anatomy/browser-checks.json`（手动登录页检查） | 登录后落地路径 `/corpus/evidence`，且无查询串、无哈希、不存口令 | 不改；B18 保证落地后不改写地址 |
| S08 | `scripts/verify-evidence-library-reference.sh` | 页面规格与静态参考里的必含词（`NOT_REQUESTED`、`RISK_CONTROL`、「评论与回复」、「来源代次」…） | 修改 PAGE-EVIDENCE-001 时保持这些词 |
| S09 | `.github/workflows/web-app.yml` | 路径过滤、rustfmt 清单、API 回归的测试选择 | 补路径：`L/evidence*`、`L/research_dossier*.rs`、`L/material_projection*.rs`、`L/local_media_routes.rs`、`L/local_asset_delivery.rs` 及证据库读取的 `crates/evidence/src/{work_resource_*,material_*,content_reobservation}.rs`；rustfmt 清单加 `evidence_browser_tests.rs`；API 回归加 `local_web::evidence_` |
| S10 | `scripts/test-web-app-postgres.sh` | 依次跑设置页、D1 的证明 | 追加 D2 一段，证据放 `$proof_dir/evidence` |
| S11 | `L/web_app_tests.rs`、`A/pageRegistry.test.ts` | 登记表前后端逐项相同；`/health` 的 `webApp.pages` | 加 `/corpus/evidence`；新增「证据库是登记过的 React 入口、旧资源退役」测试 |
| S12 | `K/CorpusSideNav.test.tsx`、`K/corpusPages.test.ts` | 侧栏标记与 `shell.rs` 逐字相同 | 若采纳 DR-5：无计数时标记仍须逐字相同，增加有计数用例 |

### 4.6 新增后端需求

**无。** 页面需要的每一项读写都已有接口（§4.2）：领域列表与采集状态词来自 D1 的 `GET /api/local/corpus/shell`；正文来自详情 `inspector.detailCurrent.body`；评论、底稿、复观测、媒体都有接口。旧页里「只存在于 Rust 拼接 HTML」的两项（领域选择对话框的数据、采集状态词）已由 D1 的外壳接口覆盖。不新增端点、不改合同、不改迁移。已知的既有接口缺陷（来源闸门缺失、不支持的排序／窗口／状态值、没有按作品查询在途复观测的接口、复观测没有请求编号）列为待决项而不是 D2 的前置条件（DR-1、DR-3、DR-4）。

### 4.7 规模估计与是否可以单个 PR

| 项 | 数 |
|---|---|
| 旧实现 | 页面脚本 3,440 行（约 173 KB）＋样式 600 行（58 KB）＋ Rust 页面部分约 620 行＋待删／改测试约 1,800 行 |
| D1 对照 | 旧实现约 100 KB → React 源码 4,058 行＋样式 1,812 行＋测试 2,724 行；PR 共 81 个文件、11.2k 行新增 |
| D2 估计 | TS／TSX 9,000–11,000 行、样式模块 3,000–3,500 行、单元与组件测试 7,000–9,000 行、e2e 约 700 行、Rust 证明约 500 行、文档约 700 行；约 120–140 个文件、22k–26k 行新增，即 **D1 的 2–2.5 倍** |
| 能否单 PR | **能**，且 R2 要求如此：整个页面是一个路由、一份共享状态（地址、列表、选中作品）；研究底稿与复观测是同一页检查器里的面板，没有哪一块能先于其他部分切换；没有后端迁移或新接口，没有跨 PR 的依赖顺序 |
| 评审体量的缓解 | PR 内按 7 个逻辑提交组织：① 地址／接口／展示纯函数及其测试 ② 列表、查询控件、领域门 ③ 检查器外壳、阅读、证据、技术事实 ④ 媒体轨道、查看器、材料台账 ⑤ 研究底稿与批量整理 ⑥ 复观测 ⑦ 登记、删除旧实现、测试与脚本改写、文档。若 Mog 要更小的评审单元，**唯一不违反 R2／R3 的拆法**是「栈式分支」：在 D2 集成分支上分 3 个子 PR 评审，只有集成分支一次合入 main，main 上不出现任何新旧并存状态。是否拆由 Mog 决定，我不自行拆 |

### 4.8 待决（DECISION_REQUIRED）、风险与接缝

**待决项**（每项给出事实、选项与建议。放行阶段 2 时请一并裁定；未明确裁定的项按「建议」实施，并在 PR 回执里逐条标明）

| 编号 | 事实 | 选项 | 建议 |
|---|---|---|---|
| DR-1 | 旧页提供三个读模型当前**无法给出有效结果**的控件：排序「相关度」→ 400 `invalid_material_query_cursor_or_sort`（`read_material_library` 只认最近观察；DESIGN-012 已记待开卡）；读取窗口「近 7 天／近 30 天来源发布时间」→ 200 但恒返回空集（`queryScope=accepted_typed_material_strict_published_time`，`items=[]`，页面会写成「没有匹配」，其实是读模型没实现）；通道状态「旧 OCR 已停用」（页面脚本动态追加）→ 400 `invalid_local_evidence_query`（后端枚举里没有 `RETIRED`）。PAGE-EVIDENCE-001 §5.3 仍把窗口与排序写成可用 | A 从界面移除（窗口控件只剩一项时整体不显示），规格同步修订；B 保留但禁用并写明「当前读模型暂不支持」；C 先改后端再做（超出 D2） | **A**，与 F16 一致；后端支持后再恢复 |
| DR-2 | 常驻禁用的占位：「打开原文」「补采」（检查器头）、「补采媒体」（无媒体时），连同 D12 的「我的视图」。PAGE-EVIDENCE-001 §5.3／§8 要求它们「disabled＋开放条件」；§2 已定删除未接通占位（F16 的「等」字） | A 全部删除；B 保留为带原因的禁用按钮 | **A**（「我的视图」已定；其余三个按同一原则），规格同步修订 |
| DR-3 | 立即复观测：(a) 没有请求编号，未确认之后重试，若上一张工单已被工位认领，会再形成一张并让工位再访问一次平台（§4.3 W3）；(b) 词表缺三个文案：`PAGE_UNAVAILABLE`（生产方确认页面不在）、`DETAIL_READ_BLOCKED`（详情页打开了但没读成）、执行状态 `NOT_STARTED`，现落进「来源未完整表达」（PAGE-EVIDENCE-001 §7.3 已记为已知缺口） | (a) A 发起前加一次确认（说明「会让工位再访问一次平台」），未确认后的再次发送要二次确认；B 不加确认，只提供「重新核对」；C 后端加请求编号（超出 D2）。(b) 补文案或维持兜底 | (a) **A**，与 D1「加入观察」同类处理；(b) 补文案：「平台页面已不存在（生产方确认）」「详情页已打开但没有读成」「尚未开始」，请产品确认措辞 |
| DR-4 | `GET /api/local/work-resources`、`GET …/{ref}`、`POST …/{ref}/reobserve`、`GET …/reobserve/{lease}` 没有 `local_comment_study_guard`（同族的 comments、research-dossier、corpus shell 都有）。`reobserve` 会创建工单；无体的 `POST` 是浏览器的「简单请求」，跨站网页可以直接发，不经预检 | A D2 补上（四处各一行 `.layer(from_fn(…))`＋两条路由测试，既有测试里不带 Host 头的调用补头）；B 不补，只记风险 | **A**，作为显式接缝；它改变既有接口行为，需协调方／Mog 确认 |
| DR-5 | 侧栏「证据库」计数徽标（A07）：保留需要改 D1 共享文件（`CorpusSideNav`／`CorpusFrame`／样式／测试，约 15–25 行），无计数时标记与 `shell.rs` 逐字相同 | A 保留，作为已列接缝；B 删除徽标（A07 改 △），同时 `shell.rs` 的隐藏标记因其冻结而保留 | **A** |
| DR-6 | 与阶段 G 的交接：(a) 入站形状（C04、C05、C07）与出站交接（C12）在 G 完成前冻结，G 的 React 采集页须原样生成与消费；(b) `scripts/test-collection-single-run-ui.cjs` 的证据库一跳（S04）谁来改——G 在另一个工作树里很可能也要改这份脚本 | (b) A D2 做最小选择器更新并通知 G，后合并者 rebase；B D2 不动，由 G 修，D2 合并时该（仅在专属分支上才跑的）证明会在证据库一跳失败 | **A**；(a) 本节 C 组需转交给 G 的实施方 |
| DR-7 | F08 提到阅读主序里要有「引用操作」，旧页没有「复制引用」类动作 | A 只把现有动作（预览 AI 输入、对照原图、导出）重排到就近；B 另立事项设计新动作 | **A** |
| DR-8 | `summary.primaryLimitation` 在读模型里是写死的默认值（`OTHER_LANES_NOT_EVALUATED`），不等价于「只跑过发现通道」（DESIGN-012「未做」，需先定义「主要限制」的产品语义） | 继续待决 | 只放技术事实层，不作标题结论 |
| DR-9 | 底稿元数据含 `authorExternalId`（平台作者编号）与作品平台编号，会出现在「底稿版本与观察信息」折叠、JSON 导出、Markdown 导出里（旧页如此；而作者外部编号在其他接口里被刻意不返回） | A 保持（不增加暴露面，普通层不渲染）；B 折叠诊断与导出里改为「已隐藏」，需要后端或前端过滤 | **A**，请 Mog 决定是否收紧 |

**风险**

| 编号 | 风险 | 应对 |
|---|---|---|
| R-1 | 体量是 D1 的 2–2.5 倍，评审与回归面大 | 7 个逻辑提交（§4.7）；能力清单逐项核对；每个提交可独立跑 `verify` |
| R-2 | 阶段 G 并行：共同修改点集中在 `local_web.rs` 路由表、`REACT_PAGES`、`pages.ts`、`App.tsx`、`web_app_tests.rs` 的 `/health` 断言、`test-collection-single-run-ui.cjs`、CI 路径 | 先合并者定基线，后者 rebase；D2 不碰任何 `collection_*` 文件，也不重排上述数组的格式 |
| R-3 | 跨页契约被静默破坏：`collection-selected:` 交接键、`returnTo` 白名单、入站链接形状 | 双向用例：D2 e2e 断言写入的键与跳转形状；采集侧脚本断言读取；本节 C 组作为 G 的约束 |
| R-4 | 旧页隐含行为多（键盘、历史写法、`work` 直读、选择保留、查看器缩放平移、浮层层叠 Esc） | 清单逐行对应测试；`work` 直读与历史写法各有专门用例 |
| R-5 | `tests.rs` 改写面广（30 删、12 改），可能弱化「共享外壳一致」的覆盖 | 样本改为直接渲染 `global_header`／`corpus_side_nav`，断言文本不变（S01）；改前后各跑一遍对照 |
| R-6 | 页面入口新增来源闸门（C14）：跨站点击本机链接得到 403 | 与已迁移的设置页、创作者页一致；在验收记录里写明 |
| R-7 | jsdom 不计算层叠，静止态样式回退（D1 教训：`.primary` 被 `.page button` 压过）看不出来 | 作用域化选择器（`.page .primary`）；四视口静止态截图人工核对；计算样式探针 |
| R-8 | 封面翻转卡是悬停触发的旋转，与 LIDS「无悬停上浮」并存；已被 EVIDENCE-COVER-FLIP-CARD-001 接纳 | 保留（旋转不是上浮）；减少动态效果下直接切换；若 Mog 认为违规则改为静态切换 |
| R-9 | 图片与长文性能：50 行起并追加、懒加载图片、底稿长文与多图 | 不引入虚拟化；用长文＋多图合成作品验证；懒加载沿用 |
| R-10 | e2e 夹具复杂：受控媒体文件、OCR 行、关联授权（复观测）、评论与回复 | 复用 `material_fixture.rs`、`material_asset_route_fixture.rs`、`research_dossier_fixture.rs`；复观测若夹具成本过高，退为组件级＋API 级并如实标注 |
| R-11 | D1 的 e2e 依赖旧证据库 DOM（S05） | 同一 PR 改（主计划 R5：针对旧 DOM 的测试随旧代码一起改写） |
| R-12 | CI 路径过滤缺口：现有 `web-app.yml` 不含 `evidence*`、`research_dossier*`、`material_projection*` | S09 补路径 |
| R-13 | 治理检查：3 处断链、当月进度、索引、生成物登记 | 阶段 2 收尾前运行 `check-project-governance.sh` 与 UI 手册检查 |

**接缝（对共享文件的改动，逐项报告）**：`A/pages.ts`、`A/App.tsx`、`L/web_app.rs`（登记）；`L/local_web.rs`（路由与页面删除）；`K/CorpusSideNav.tsx`、`K/CorpusFrame.tsx`、`K/CorpusFrame.module.css`（DR-5）；`shared/*` 预计**零改动**（`requestJson` 已支持无体 `POST`）；`apps/web/test/e2e/{creators,settings}-real-chain.mjs`；`scripts/test-web-app-postgres.sh`、`scripts/test-local-001-discovery-postgres.sh`、`scripts/test-research-dossier-postgres.sh`、`scripts/test-collection-single-run-ui.cjs`、`.github/workflows/web-app.yml`。`shell.rs`、`shell.css`、`lids_tokens.css` 不动。

**协调方裁定（2026-10-16，阶段 2 放行时）**

- DR-1：先用真实调用证明再移除。结果：「相关度」排序带不带检索词都 400；近 7／30 天时间窗由读模型直接返回空集（`crates/evidence/src/material_projection.rs` 对时间窗不读任何数据）；「旧 OCR 已停用」400——三者都不可能给出结果，移除；12 次调用记录见验收记录。读模型支持时间窗后再恢复，空结果须说明「发布时间未知的作品不计入」。
- DR-2：按 §2（F16）删除。
- DR-3：发起前确认；回执分已提交／未受理（HTTP 200 但准入拒绝）／未完成／未确认，未确认不自动重发；三个状态词按建议补齐，措辞待产品确认。
- DR-4：批准。协调方核对过调用方：只有同源页面与测试脚本；浏览器插件只用 `chrome.tabs.create` 打开页面、不调这些接口。插件入口须实测过闸门（已证明：`Sec-Fetch-Site: none`，HTTP 200）。
- DR-5：批准，最小接缝。
- DR-6：D2 改采集脚本里驱动旧证据库 DOM 的一段，跨页链接与交接键形状不变；跨页合同已贴 #430 供阶段 G 对照。
- DR-7、DR-8：按建议。
- DR-9：保持原样，不扩大暴露；是否收紧交 Mog。

### 4.9 阶段 2 计划（放行后执行）

**文件**：`E/` 下新建约 45 个源文件（EvidencePage、evidenceQuery／evidenceApi／evidenceTypes／evidencePresent、EvidenceControls／FilterPanel／SelectionBar、WorkList／WorkRow／CoverCard／CoverGeometry／MaterialRail／QuoteBlock／Engagement／Media、DomainChoiceDialog／ReturnBanners、Inspector 与 Reading／Evidence／Materials／Trace／TechnicalFacts 五个面板、FactGrid／StateLine、MediaRail／Lightbox／useZoomPan、`dossier/*`（面板、片段、图片对照、任务材料、评论、批量弹窗、导出、批量存储）、`reobservation/*`、useCollectionSelection／usePanelWidthPreference）及对应的 CSS Modules；登记 `pages.ts`、`App.tsx`（lazy）、`REACT_PAGES`；同一 PR 删除 §4.4 所列旧实现。样式只用 LIDS Token（CSS Modules，无写死值、无悬停上浮）。

**测试**：约 35 个 Vitest 文件（纯函数：地址、接口解码、状态词、作者规则、停止原因、封面几何；组件：列表三种排版、控件、领域门、检查器各面板、查看器、底稿与批量、复观测、深链与历史、源码扫描）；`apps/web/test/e2e/evidence-real-chain.mjs`＋被忽略的 Rust 证明 `L/evidence_browser_tests.rs`（隔离 PostgreSQL＋真实 Axum＋已构建前端＋无界面浏览器，合成数据），串入 `scripts/test-web-app-postgres.sh`；Rust 侧另含重新分层与复观测的幂等证明（重复提交的库内后果）。

**真实链路证明步骤**：① 打开与外壳（标题、h1、一级导航、侧栏带领域、面包屑选择器、无写请求、普通层无 UUID）② 未选领域弹窗、`work` 随选择保留、无领域时地址不被改写 ③ 三种排版、50 条＋继续读取、排版切换不重新请求、3:4 探针 ④ 检索、四个下拉、系统视图、后退恢复 ⑤ 选中→阅读主序、技术事实折叠、`tab` 深链 ⑥ 媒体轨道与查看器（缩放、Esc、焦点回位、不可内联类型） ⑦ 评论：点击前无评论请求，点击后 20 条、续读、匿名 ⑧ 研究底稿：读取、对照原图叠框、预览 AI 输入（无模型调用）、导出内容、重新分层（已完成／已是当前版本）、批量弹窗（预览、开始、暂停、刷新后继续） ⑨ 复观测：可用时点击→库内恰好 1 个请求、1 张工单、0 个租约；再次提交合并到同一工单；不可用时给原因 ⑩ 入站：创作者页→证据库→返回创作者；`collectionRunRef` 横幅；`work` 不在首页时直读 ⑪ 评论深采交接的存储键与跳转形状 ⑫ 存储只有 §P01 的三类键且无内容 ⑬ 四个视口 1440／1280／1024／390：无页面级横向溢出、1280×720 首屏见第一行与检查器标题、抽屉开合、含静止态主按钮的截图 ⑭ 全程只有同源请求、无脚本与控制台错误、失败请求只有有意拦截的 ⑮ 库内副作用核对（采集请求、工单、模型调用的计数与预期一致）⑯ 负例：带跨站来源头（Sec-Fetch-Site 为 cross-site，或 Origin 非本机）的页面请求返回 403；缺少领域、不存在的领域、非法 `collectionRunRef`、非法游标的读取得到明确错误而不是空列表。

**文档**：`docs/design/acceptance/web-react-001-d2-acceptance.md`（新建）；修订 `docs/design/pages/evidence-library-page.md`（D12、DR-1、DR-2、E02 的 contain、阅读主序）；`docs/README.md`、`docs/design/README.md` 索引；`docs/progress/2026-10.md`；`docs/design/lids/migration-log.md`；`docs/governance/generated-artifacts-registry.md`（证明证据目录）；`docs/runbooks/local-evidence-library.md`（页面描述）；§4.4 所列 3 处断链；本节追加「4.10 回执」。

**验证**：`npm --prefix apps/web run verify`；`cargo test -p linggan-api --bins --locked`；`scripts/test-web-app-postgres.sh`；`scripts/test-local-001-discovery-postgres.sh` 与 `scripts/test-research-dossier-postgres.sh --http`；治理与 UI 手册检查；本模块 rustfmt；`git diff --check`。一次性容器与数据卷用完即删。

### 4.10 回执（D2 实施与协调方复核，2026-10-16）

> 本回执记实施方与协调方各自做了什么、验证了什么；验收详情见 [ACC-WEB-REACT-001-D2](../../design/acceptance/web-react-001-d2-acceptance.md)，页面规格修订见 [PAGE-EVIDENCE-001](../../design/pages/evidence-library-page.md) §14。

**状态**：已实施并复核，以 Draft PR 呈交（分支 `feat/web-react-001-d2`，分支点 `main@baaf5a51`，已并入 `main@82d48a72`）；未合并、未部署。

**实施**：子代理（Sonnet 5.5）按 §4.9 在专属工作树分步本地提交：接口闸门与库内证明、纯函数、列表与控件、侧栏徽标接缝、媒体与评论、立即复观测、研究底稿、检查器与页面编排、整页测试、源码敏感面扫描与样式契约、登记入口并在同一提交删除旧实现、真实浏览器发现的两处缺陷（封面缺多列栅格、查看器换图后焦点丢失）。收尾阶段子代理撞上模型周用量上限，按项目约定改由协调方顺序接手。

**协调方接手后**

- 证明脚本：插件入口证明改为由记录服务器取请求头、页面自读 HTTP 状态（Playwright 不为扩展新开的标签页报网络事件），并等领域对话框出现再判定；评论续读断言改为两页 40 条互不重复（评论按稳定来源编号排序，编号每次播种不同）；两处 lint。
- 并入主线：阶段 G（`712286c3`）与 E（`82d48a72`）已于同日合入主线，以一次合并提交并入。10 个冲突文件逐个裁定：页面登记两边并存且顺序与前端一致；两边各删的旧页面代码都不保留；G 已删的旧采集页测试照删；D2 已删的旧证据库字符串断言照删（离页作品深链由真实浏览器证明覆盖）；测试脚本两边的新增都保留；进度条目都保留。合并后核对阶段 G 的 React 采集页：运行详情、观察目标作品、领域管理三处链接与「评论深采」交接键的读取都与 §4.1 C 组一致。
- 其余：清理合并残留（孤立注释与右括号、重复的 `#[test]`）、合并后不再使用的导入、两个文件末尾空行；在 §4.8 记下裁定。
- 云端首跑后：证明脚本五处计时依赖改为等到真实内容（本机降速复现并验证），每一步加失败现场截图；步骤 9 的一次偶发超时原因未查明，见验收记录 §5。

**验证**（合并后在工作树实跑）

| 项目 | 结果 |
|---|---|
| `npm --prefix apps/web run verify` | 类型检查、lint、构建通过；71 个测试文件 749 项（主线基线 40／410，本包 +31／+339） |
| `cargo test -p linggan-api --bins --locked` | 156 通过 / 0 失败 / 71 忽略（主线基线 169 / 1 / 68；本包删除 18、新增 7；主线那 1 项失败与本包无关，见验收记录 §5） |
| `scripts/test-web-app-postgres.sh` | 全部通过：设置页 15 步、创作者页 17 步、证据库接口证明、证据库页 19 步、插件入口、运营五页；证据库部分另行连续两次独立重跑通过；容器与数据卷已清理 |
| `scripts/test-collection-single-run-postgres.sh`（浏览器模式，与 CI 相同） | 通过（React 采集运行详情 → 证据库 → 勾选、续读、刷新） |
| `scripts/test-local-001-discovery-postgres.sh`、`scripts/test-research-dossier-postgres.sh` | 研究底稿脚本通过。发现脚本首跑：API／库内证明 61 项通过（含 D2 为来源闸门改过的 material_projection／cursor／media／replica 各组），7 个浏览器证明因缺少各自专用环境被拒——脚本跳过清单漏登记了这 7 项（其中 6 项在主线上已漏，含协调方自己在阶段 B、D1 加的两项），已补登记；补登记后整脚本重跑通过（「LOCAL-001 discovery PostgreSQL proof passed」，API 被忽略组 61 / 0，容器已清理）；另预先登记阶段 F（#441）新增的 `topics_and_workspace_real_chain_browser_proof`，两个 PR 谁先合并都不再打断本脚本 |
| 本模块 rustfmt、`./scripts/check-project-governance.sh origin/main`、`./scripts/verify-ui-design-handbook.sh`、`git diff --check` | 通过 |

**待 Mog**：DR-9；复观测三个新状态词的措辞；合并与部署授权；独立代码审查的时机。

## 5. D3 · 评论研究

_未开始。_

## 6. 执行方式

- 代码由子代理（Sonnet 5.5）在专属工作树实施；协调方（Claude Code）负责拆分、任务说明、审查、验证、提交与 PR。子代理不提交、不推送、不碰其他工作树。
- 每个子项：先盘点能力清单，再实现；完成后协调方逐项核对清单、差异与测试，再提交。
- 验证：`npm --prefix apps/web run verify`、`cargo test -p linggan-api --bins --locked`、隔离 PostgreSQL 真实浏览器证明、治理与 UI 手册检查、云端 `web-app.yml`。
