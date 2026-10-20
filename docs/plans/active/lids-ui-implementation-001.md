# LIDS-UI-001 · 真实前端统一实施

> 状态: 活跃计划
> 最后核对: 2026-10-20
> 适用范围: LIDS-UI-001 当前单 Agent 交付包
> 事实来源: Mog 当前对话直接派定、Q01–Q12 已确认、origin/main@612dcbea
> 冲突时以谁为准: 用户最新确认、实际代码/产品/数据合同与当前 SCOPE；设计读取 LIDS 唯一参数源

Mog 2026-10-20 明确允许开始优化真实代码/UI，并接受正式 LIDS 目标语义色板。Claim：当前 root Agent，专属分支 codex/lids-ui-implementation-001，工作树同名；基线 612dcbea。未创建衍生任务、并发或新的派单。本包只本地实施/验证；合并、部署、生产写动作另行授权。

## 本轮完整结果

全站共享基础（颜色、真实字体、B 字级、C 焦点、M2 控件）、A 共享壳及图标折叠、Z02 导航、采集任务 L1 单行账本与现有详情/新建/停止交互。现有 17 路由继承基础，不将继承等同全部业务状态的逐页验收。用户问题卡片结构、登录插画与 V8 工位例外保持。

## 实施顺序与完成条件

1. 确认来源、授权、范围和四图；完成变更清单后进入代码。
2. 已采用九源加颜色单源衔接当前 main；保护新版 React/业务合同。
3. 唯一运行 Token、原始字体/许可与共享基础接入，自动检查无漂移。
4. 共享壳与任务账本、字段配置、只读/编辑/请求状态按已有合同；不新增后台投影。
5. 类型、lint、单测、构建、治理和浏览器宽窄核对；保存证据，不伪装真实写动作验证。

详见 [变更清单](../../design/changes/lids-ui-implementation-001-ui-change-manifest.md) 与 [验收](../../design/acceptance/lids-ui-implementation-001-acceptance.md)。

## 本轮状态

共享基础、A壳与采集任务第一轮候选已落地，自动/只读浏览器核对回执见验收。等待Mog视觉签收；后续逐页状态、CMP认证和主线/部署保持未完成，不把本轮交付声明为全站完工。


## 创作者页续修（2026-10-20，当前已派范围）

Mog直接派定创作者页真实代码候选，先不合并部署；继续同一Claim/工作树，不另派任务。按P05:list的B检索工具合带，统一FIND/范围选择、筛选/排序、结果与表格及详情抽屉。领域仍由CorpusFrame的Z02拥有。保留当前搜索250ms/IME、URL/返回、同篇作品交集、作者与作品判断分责、观察关系未知、写回执与权限合同。

- 表面：Z04-T查询/筛选/排序/刷新；宽屏T/V同带，窄屏T→V→R→B→P；Z04-R紧凑结果/条件/范围；连续八列表格；Z05身份/对象Tab/证据/当前对象动作；筛选、设置、判断和加入确认为现有模态表单。
- 状态：读中/首次失败/刷新失败保留旧内容/无命中/未知数值/关系不可用/领域暂停/无爆款阈值/媒体失败；筛选为未应用草稿，误关闭保护；详情只读遮罩与Esc关闭/焦点返回，已有提交上下文不串对象。
- 依赖：复用ActionButton、CorpusFrame、DomainPicker、ModalDialog（仅新增可选遮罩回调）、既有query/API/数据投影。页面选择适配器留在creators目录，不认证CMP，不替其他页面引入组件库。CreatorTable/DrawerPanels的业务定义保留，写命令不扩展。
- 验收：现有创作者全套回归；选择器键盘/禁用/Escape/外部关闭；搜索IME/防抖/清空取消迟到提交；筛选应用/取消/关闭保护；排序/分页/URL返回；抽屉Tab/焦点/遮罩/原回执上下文；Token/类型/lint/build/治理。真实浏览器GET宽屏/820/390，查询/筛选/表格/抽屉；真实写命令、模型与采集不执行，Mog验收单列。
- 文件：creators组件/CSS/测试，corpus/ModalDialog可选基础交互、上述文档及页面规格/验收/索引/进度；不改后端/API字段/数据库/插件/运行目录，不改用户问题卡片/采集账本。
- 读取回执：本项目AGENTS/README/docs索引/current-state/governance；UI执行合同、LIDS入口/system/typography/tokens/regions/patterns/primitives/materials/data-boundaries/language/interaction/component-promotion；PAGE-CORPUS-CREATORS-V1-001及当前creators API/query/写操作源码与测试。已有Q01–Q12取代历史页规格中的旧视觉参数，业务合同不变。

本地证据固定implementation-20-creators；更改前源码hash和页面基线与新截图分别保存，当前实现/剩余边界追加到同一验收文件。本阶段未提交/合并/部署，先给Mog审阅。

详情动作依赖补记：同页已有设置/判断/加入表单也消费统一ActionButton，关闭时按S1区分脏稿与提交中。最小局部useCreatorLeave钩子只管关闭确认/页面离开，原请求仍由既有React Query mutation及useWriteGuard跟踪，不改业务身份/接口或新增后台操作；取消请求不作为关闭的副作用。

创作者续修状态：B组合/选择与分页/八列表格/对象抽屉/同页S1表单已作为真实候选实现。全量1026与最后270项定向检查通过；1280直接页、820/390真实iframe及横滚/遮罩/焦点核对通过，具体边界见验收。本包仍未提交、合并或部署，等待Mog验收。

## 证据库续修（2026-10-20，Mog直接派定）

沿用本包/工作树，交付真实React候选，先不合并部署。按P03检索B组合统一搜索、筛选、作品卡与详情；FIND、Z02领域、B排版/C焦点/M2/E1白场保持。

- 表面地图：Z04-T手动查询/清空/筛选，宽屏T与V合带，工作区≤1000px恢复T→V→R→B→P；V承载五个集合预设与三种排版；R仅紧凑已载数/读取边界；B保留研读/表格/封面与独立勾选；Z05对象身份/五个对象Tab/宽度与关闭，窄屏只读抽屉。对象Tab不移入全局Z02。
- 状态地图：未选领域/加载/首次与后续失败/空结果/无效范围/未知≠0；手动提交与IME、URL前后退；筛选即选即读保留现有合同，无未保存草稿；卡片选中与勾选、键盘/触屏翻面、封面加载/未取得/失败；详情只读关闭/返回焦点/嵌套媒体逐层Esc，白场contain、图像不裁色；现有受控写入口保留但不触发。
- 依赖地图：CorpusFrame/DomainPicker/ActionButton/ModalDialog与既有API/query/选择/详情hooks。将CreatorSelect的相同基本选择实现提取为shared/SelectField并由原CreatorSelect薄重导出，保留行为/测试，不作CMP认证或新增库。媒体受控URL、深采交接、底稿/复观测命令与原请求身份不改。
- 验收地图：证据库现有回归+查询IME/清空/外部状态/筛选键盘、卡片媒体失败与选中/翻面、只读抽屉焦点/关闭与嵌套Esc、表格局部横滚；共享选择器与创作者回归；类型/lint/全量前端/build/Token/手册/治理；真实GET页面1280/820/390核查及DOM边界，真实写/采集/provider不执行，Mog视觉签收单列。
- 文件范围：evidence UI组件/CSS/必要测试与PAGE规格；shared/SelectField基本复用、creators/CreatorSelect薄导出；同包计划/manifest/验收/索引/进度/生成物登记。API/types/query编解码/hooks/后端/数据库/插件/3000运行目录、用户问题卡片/任务账本与creator业务合同不动。
- 读取回执：项目AGENTS、治理/UI执行合同、LIDS单源与Q01–Q12、PAGE-EVIDENCE-001、当前EvidencePage/Controls/query/Inspector/CoverCard/媒体/列表及测试。历史PAGE的旧视觉/几何测量场按已确认Q12替代，不推翻材料/资格/状态语义。历史溢出修复只作检查线索，按当前DOM重新验证。

证明目录固定implementation-21-evidence（先登记后生成），更改前hash基于当前已优化候选，不以HEAD冒充页面基线。保留创作者续修的全部未提交改动。本阶段无提交/合并/部署。

证据库续修状态：真实B候选完成本地实现与有界核查。91文件/1038项、类型/lint/build/Token/手册/治理通过；1280/820/390只读页面与窄表列容量、卡板、对象/媒体逐层关闭已核对，详见同包验收。未合并部署，等待Mog签收。创作者原基本选择器在本阶段提取为shared唯一实现，其兼容导出保留原API与测试。

## 创作者/证据库发布授权（2026-10-20）

Mog已直接要求使用GPT-5.6 Terra subagent提交、合并并部署两页候选，刷新3000；并明确允许在备份/短暂停服后应用主线已合并的0129/0130两条schema迁移。运行库只读账本当前0128，main当前8dd00108，原API612dcbea；不授权历史回填、模型/平台调用或启用当前disabled的巡检/媒体worker。执行使用本机Codex CLI独立子代理显式`-m gpt-5.6-terra`；内置subagent只做过只读核查，其自述模型不作为实际模型证据。

本轮从当前main整合两页与直接共享依赖，原implementation树和其中采集/其他页未发布候选保留；共享字体/颜色/排版/壳层影响需跨路由回归，不能宣称两页CSS完全隔离。迁移/发布按runbook，exact commit/PR/CI与备份/前后策略/计数和服务身份写release-22回执，未发生的层不提前填完成。
