# WEB-REACT-001 阶段 D 分册 · 语料三页迁移到 React

> 状态: 活跃计划
> 最后核对: 2026-10-16
> 适用范围: WEB-REACT-001 阶段 D 的拆分、各页能力清单（迁移前盘点）、已定的设计改动与验收；D1 已复核并以 Draft PR 呈交（合并与部署待 Mog 授权），D2/D3 未开始
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

_未开始。_

## 5. D3 · 评论研究

_未开始。_

## 6. 执行方式

- 代码由子代理（Sonnet 5.5）在专属工作树实施；协调方（Claude Code）负责拆分、任务说明、审查、验证、提交与 PR。子代理不提交、不推送、不碰其他工作树。
- 每个子项：先盘点能力清单，再实现；完成后协调方逐项核对清单、差异与测试，再提交。
- 验证：`npm --prefix apps/web run verify`、`cargo test -p linggan-api --bins --locked`、隔离 PostgreSQL 真实浏览器证明、治理与 UI 手册检查、云端 `web-app.yml`。
