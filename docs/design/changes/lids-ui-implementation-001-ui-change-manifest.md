# LIDS-UI-001 · UI 变更清单

> 状态: 活跃计划
> 最后核对: 2026-10-20
> 适用范围: LIDS-UI-001 当前单 Agent 交付包
> 事实来源: Mog 当前对话直接派定、Q01–Q12 已确认、origin/main@612dcbea
> 冲突时以谁为准: 用户最新确认、实际代码/产品/数据合同与当前 SCOPE；设计读取 LIDS 唯一参数源

## 授权、读取与边界

Mog 当前直接派定的同一交付包；Claim root / codex/lids-ui-implementation-001 / main@612dcbea。已读项目 AGENTS、docs/README、current-state、governance、UI 执行合同、LIDS 入口/参数/regions/patterns/materials/typography、React DEC-0010 与当前运行实现。类别为展示和既有交互适配；不新增状态、资格、权限、API 字段或业务动作。

## 表面地图

| 表面 | 所有权与本轮工作 | 验收 |
| --- | --- | --- |
| AppShell + 各 Frame | 品牌/一级导航、Z02 路径领域与已有三级 Tab、Z03 折叠图标；A 容量/M2 | 宽窄、键盘、保留 URL/领域 |
| 登录、模型设置、创作 | 共享字体/语义；登录几何与 V8 创作布局保持；创作的动作按钮随共享 ActionButton 采用 M2（ALIGN-WORKBENCH-015，Mog 2026-10-20 决定） | 回归测试/构建；创作浏览器证明的按钮断言改读 M2 源，独立场景不冒称改版 |
| 创作者、证据库、评论四视图 | 共享字体、数字、输入/FIND/M2；已有 B/C 保持，用户问题卡片保护 | 测试及只读代表页 |
| 采集目标、任务、领域、工位 | 共享基础；任务 L1/44/144、字段配置、局部横滚、详情交互适配 | 单测与宽窄实际页面 |
| 运营概览、发布、账号、每日、IO | 共享基础继承，现有功能不改 | 现有回归/构建；未逐状态验收单列 |
| 主题图谱、主题详情 | 共享基础继承，已有 Z02 三级导航保持 | 现有回归/构建 |

共 17 路由入口（comments 含四视图，collection 四页，ops 五页）以 app/pages.ts 实际注册为准；不能按截图张数宣称业务全面通过。

## 状态词典

正常内容来自既有 GET；加载/空/读取失败分别呈现。null=未知，不改为0；已有部分成果不等于完整完成。草稿关闭经未保存确认；提交中离开保留原请求身份/内容，不重发；回执以服务端为准。停止后不可恢复；领域/工位暂停为独立合同。拒绝与结果未确认分开，不乐观成功。只读预览拒绝写请求，其错误不代表正式API失败。

## 依赖与复用地图

LIDS JSON → 唯一 lids_tokens.css → shell.css / 页面 CSS Modules；原字体和许可 → public/fonts/lids。为让 Vite `dist` 内嵌套字体和可见许可在正式同源路径可达，`apps/api/src/local_web/web_app.rs` 仅增加精确白名单静态资源路由、MIME 与缓存策略；不放宽目录、接口或页面路由。AppShell 只负责壳，Frame 声明业务路径/领域。新 Sidebar 为 shared 路由，不自动晋升 CMP；任务 ledger 为业务模式，列仅消费 tasksApi.Run 已有投影。runRequests/collectionCommand 保留幂等与回执；其余后端、数据库、worker、插件、providers 不在修改范围。问题卡片 JSX/CSS、登录素材保持原件。

## 验收矩阵

| 范围 | 自动 | 浏览器 | 未证明 |
| --- | --- | --- | --- |
| 设计源/Token/字体许可 | 源镜像、文件hash、token检查、typecheck/build | 实际加载/计算样式 | 冷网络/跨平台 |
| 壳与导航 | 现有路由/新折叠键盘回归 | 1440/820/390、折叠/领域/Z02 | Mog 审美签收 |
| 任务账本 | 固定核心字段/字段配置/筛选URL、未知值与真实合同回归 | 单行、44px、144px、局部滚动 | 新API字段、真实任务执行 |
| 详情/新建/停止 | 既有请求/dirty/pending/stop 回归 | 只读/本地草稿 | 真实采集/停止副作用 |
| 其他页面 | 全套现有测试 | 代表页面只读核对 | 全部业务状态逐页验收 |

## 文件/停止条件

apps/web、`apps/api/src/local_web/web_app.rs` 及其测试 `web_app_tests.rs` 的上述精确字体静态服务例外、docs/design/lids、项目 Agent 路由、相关治理/索引/日志与源同步脚本；不改 crates、migration、plugins、运行目录或共享 checkout。未确认设计/业务意义、外部副作用或跨任务并发则停止受影响项。M2 当前批准替代旧“全站禁 hover lift”在主按钮的约束；Collection 冻结样式仅为迁移保真历史，本次新授权允许任务独立 Token 化样式，不改其余旧页面语义。提交前独立审查由项目规则另触发；本轮未承诺提交/推送/合并。

字体许可的可见使用标记：复用 FontNotice 在个人菜单、紧凑工位菜单与登录既有页脚提供 OPPO Sans 使用说明和原许可入口；登录仅补标记/继承字族，不改冻结的插画、几何或表单业务。

采集任务领域控件复用既有Z02 picker，链接留在tasks，读取已有domain参数；未选时明确“全部领域”，不把shell默认领域伪装为实际列表范围。FIND与LI保持brand角色，不随普通数字字族替换。


## 创作者页续修清单（2026-10-20）

用户结果、四图、文件与读取回执见[当前计划创作者续修](../../plans/active/lids-ui-implementation-001.md)。展示+交互混合；不改变真实写命令/模型资格。采用P05:list/B、C焦点、M2、B排版与F2、A壳、Z02领域和对象局部Tab、S1只读关闭/草稿保护。八列表格继续同构比较，作者倾向/作品表达/观察关系不合并；未知不为零。复用shared ActionButton，新增creators局部选择适配器，未晋升CMP。

必要共享依赖仅ModalDialog增加显式可选遮罩关闭策略，默认行为保持，调用者决定是否确认；不重写全站浮层。不扩展页面/导航/字段/后台动作。发现数据或产品语义冲突停止对应部分，其他独立部分继续；真实写副作用和Mog审美验收保持NOT VERIFIED。

## 证据库续修（2026-10-20，Mog直接派定）

沿用本包/工作树，交付真实React候选，先不合并部署。按P03检索B组合统一搜索、筛选、作品卡与详情；FIND、Z02领域、B排版/C焦点/M2/E1白场保持。

- 表面地图：Z04-T手动查询/清空/筛选，宽屏T与V合带，工作区≤1000px恢复T→V→R→B→P；V承载五个集合预设与三种排版；R仅紧凑已载数/读取边界；B保留研读/表格/封面与独立勾选；Z05对象身份/五个对象Tab/宽度与关闭，窄屏只读抽屉。对象Tab不移入全局Z02。
- 状态地图：未选领域/加载/首次与后续失败/空结果/无效范围/未知≠0；手动提交与IME、URL前后退；筛选即选即读保留现有合同，无未保存草稿；卡片选中与勾选、键盘/触屏翻面、封面加载/未取得/失败；详情只读关闭/返回焦点/嵌套媒体逐层Esc，白场contain、图像不裁色；现有受控写入口保留但不触发。
- 依赖地图：CorpusFrame/DomainPicker/ActionButton/ModalDialog与既有API/query/选择/详情hooks。将CreatorSelect的相同基本选择实现提取为shared/SelectField并由原CreatorSelect薄重导出，保留行为/测试，不作CMP认证或新增库。媒体受控URL、深采交接、底稿/复观测命令与原请求身份不改。
- 验收地图：证据库现有回归+查询IME/清空/外部状态/筛选键盘、卡片媒体失败与选中/翻面、只读抽屉焦点/关闭与嵌套Esc、表格局部横滚；共享选择器与创作者回归；类型/lint/全量前端/build/Token/手册/治理；真实GET页面1280/820/390核查及DOM边界，真实写/采集/provider不执行，Mog视觉签收单列。
- 文件范围：evidence UI组件/CSS/必要测试与PAGE规格；shared/SelectField基本复用、creators/CreatorSelect薄导出；同包计划/manifest/验收/索引/进度/生成物登记。API/types/query编解码/hooks/后端/数据库/插件/3000运行目录、用户问题卡片/任务账本与creator业务合同不动。
- 读取回执：项目AGENTS、治理/UI执行合同、LIDS单源与Q01–Q12、PAGE-EVIDENCE-001、当前EvidencePage/Controls/query/Inspector/CoverCard/媒体/列表及测试。历史PAGE的旧视觉/几何测量场按已确认Q12替代，不推翻材料/资格/状态语义。历史溢出修复只作检查线索，按当前DOM重新验证。

证明目录固定implementation-21-evidence（先登记后生成），更改前hash基于当前已优化候选，不以HEAD冒充页面基线。保留创作者续修的全部未提交改动。本阶段无提交/合并/部署。

## 2026-10-20 · Mog授权#457/#458依赖整合与3000发布

当前对话Mog已接受采集候选并明确授权核对两PR、无问题合并并部署。#457@6fdbe9a1与main@2dc1f520冲突，本轮由root在专属lids-release-integration-001整合现有来源：保留主线AI-GEN1离线评审/策略读写，套用本包LIDS UI；不创造第三种产品语义。冲突涉及CreatorPolicyDialog/CreatorWrites回归、creators/settings浏览器证明与文档索引/迁移日志/注册表/进度。保留两边断言和已确认语义，先前审查仅历史依据，最终树另做commit-reviewer与自动检查。

表面/状态/权限沿两包既定合同；新增依赖图为main→#457→#458，验收精确head、CI、隔离真实库/浏览器、实际main/runtime/PID/3000页面。部署只用当前main installer，保留禁用worker，未提供数据库不可逆处置、真实模型/平台/采集授权。证据沿已登记/tmp/collection-lids-ui-001/release-01/，不入Git，不改原#457工作树。
