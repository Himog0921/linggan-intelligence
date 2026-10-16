# WEB-REACT-001-F · 主题图谱与 Topic 工作区迁移

> 状态: 活跃计划
> 最后核对: 2026-10-16
> 适用范围: WEB-REACT-001 阶段F / Issue #440，parent #430
> 事实来源: Mog当前会话派定、主计划、当前main与3000 `82d48a72`、Topic Map HTTP/产品手册及紧凑工作区规格
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实运行/代码/合同、DEC-0010

## 1. 授权与责任

Mog要求按WEB-REACT-001继续迁移主题图谱，UI与当前实际页面一致，完成后截图对照。Codex root / task-id WEB-REACT-001-F-TOPICS-20261016。base `82d48a72b95d905bffd0def9314fdee004893ddb`，branch `codex/web-react-001-f-topics`，独立 `.worktrees/web-react-001-f-topics`。主计划阶段F是唯一推进来源；本分册只细化这个已派定包。Mog已明确允许本包最多2个实施代理：topics_main负责主图谱React/DTO/组件测试；topic_workspace负责独立工作区React/DTO/组件及独立浏览器模块；root负责共享接缝、真实Axum/隔离PG证明、实际截图对照与交付。不得再派生实施代理，提交前review按AGENTS进行。

本包只交付React候选与证明、Draft PR；不合并/部署，不执行共享库migration/写命令，不恢复研究或worker、不调用平台/模型。现有Rust API/领域规则保留。此前E已按Mog授权随PR439合并、3000为82d48a72；其业务签收与C/D遗留不并入F。

## 2. 来源与分类

读取回执：AGENTS.md、docs/README.md/current-state.md、governance三册、UI执行合同、design README/design-governance、LIDS README/tokens/shell-zones/data-boundaries/language-policy，DEC-0010、主计划F、Topic Map HTTP、topic-map-handbook、topic-map-compact-382、旧topic_map/topic_workspace及相关回归。

混合展示/交互/状态/命令，最高风险为显式研究/补采/结构与保存命令；本包保持其原语义，真实回执仅在隔离环境验证。L1/L2既有工作区，唯一LIDS token + shell.css；页面CSS Modules。绘图几何与聚合/匹配算法保留，不借迁移改产品。

## 3. 表面地图与能力盘点

| 表面 | 必须保留能力 | 关键状态/验收 |
|---|---|---|
| 共享页头/上下文 | 当前领域、双视图、研究就绪、方法/我方/备选/变化入口 | 领域ID、研究暂停/失败/派发未知分开；无自动写命令 |
| 单侧栏主题树 | 概览/旅程入口、主题层级与折叠、旧候选分组、计数列、窄屏展开 | 双视图共享选择；侧栏位置、滚动与布局保真 |
| 材料与讨论 | 关键词、阶段、平台/时间、最新/高表现/常规、材料卡/列表、原文/评论/讨论 | 未知/不可读/仅评论可读分别呈现；搜索不声称全库全文 |
| 结构与表现 | 结构表/散点、主题选择、最多三项对比、矩阵、定义边界 | 对象身份以ref为准，统计全范围不随加载条数改变 |
| 生命旅程 | 地图/列表、主阶段/涉及、family/adult路径与overlay、阶段材料、其他分类 | 原算法与分母；main/involved不混同，reduced motion |
| 内容格局/来源/候选/我方 | 场景检索、讨论组、来源/研究覆盖、候选、我方复用与爆款 | 名称相同不混同主题；没有证据不制造结论 |
| 材料/精选/阅读弹窗 | 完整主题读取、精选10篇、替换、嵌套阅读、评论分页、精确引用高亮与来源 | 当前资格独立复核，晚到回答不覆盖/重开已关闭弹窗；失败不泄漏旧原文 |
| 研究弹窗 | 当前方法/历史账本、阶段用量、诊断、批次、配置/预算、启停/恢复与按需 | 申请/排队/外发/接纳分开；完整请求身份与异常/冲突回执 |
| 结构编辑/移动/定义 | 定义、关系、合并/拆分preview/apply、位置移动 | 预览不是保存；版本/hash冲突不报成功；业务规则留Rust |
| 我方/备选/产品 | 账号范围、规则、人工/研究备选保存、来源重开、Markdown导出 | 成对定义引用与来源冻结；限制后不显示失效保存内容 |
| 补采弹窗 | 原有search/detail/comments边界、独立round、停止/刷新 | 真实命令只隔离证明；明确授权不是已执行 |
| `/topics/{canonical_key}` | 定义/版本/冻结材料/支持挑战边界筛选/Inspector/JSON回链 | key合法性、404/503、当前来源；无合成占位成功 |

状态词典：读取中、未读取/读取失败、空结果、未知/null/0、暂定/旧方法候选/正式、Coverage部分/完整、来源可用/不可用/受限、研究暂停/可调度/失败、排队/运行/派发未知/接纳、请求未确认/冲突/服务器确认分别保留，不压为总completed。

## 4. 依赖与文件责任

Exclusive：`apps/web/src/pages/topics/**`；旧topic_map/topic_workspace的页面输出/JS/CSS及只服务旧DOM的测试；本包visual/real-chain/e2e与分册验收。
Shared由root：App.tsx/pages.ts/登记测试，local_web.rs/web_app.rs/Router测试，隔离证明入口/CI，文档索引/计划/规格/migration-log/progress。API层仅删除页面职责与必要窄同源读取；原Rust命令组合保留。Forbidden为Rust业务/schema、其他页面/工作树/运行目录、Token值和外壳视觉改版。遇并行接缝冲突报告Mog，不合入他人分支。

## 5. 验收矩阵与证据

1. 确认当前main/runtime，再在四视口拍完整页面、双视图/地图列表/选择主题/主要弹窗/完整滚动，保存private基线；只允许同源GET/HEAD，页面原有自动viewed回执若发生由捕获器阻断并单独记录，不伪造成功回包。
2. 以正常JSON、字段缺失/类型错、未知与0、逆序/切换/关闭/卸载、失去来源资格等组件反例验证React实现；不存材料原文进浏览器存储。
3. 正式Axum + 一次性PostgreSQL + React合成fixture验证读取、写命令回执、409/失败、不重复、下载和导航。既有Rust/PG内核回归，旧asset404/未知路径/API边界负例。
4. 当前实际数据候选仅只读桥接；与基线同数据/场景完整RGB对照，无裁切/遮罩/容差掩盖差异；性能按相同加载条件多次对照。它不代替正式Axum链路。
5. 同PR删除旧页面输出/资产/纯DOM测试，更新CI到正式React测试；TS/lint/test/build/governance/UI-handbook通过。独立审核有效发现逐项处理，再交付Draft。

证据：工作树`artifacts/private/web-react-001-f/`，真实原文/截图/快照不上传GitHub或CI。合成测试证据可按现有隔离链路登记。

性能测量固定条件：当前实际材料录制输入、原生页面入口同样冻结、1440×900、fresh browser context、交替顺序各7轮；记录全材料就绪，以及结构/散点/来源/材料切换和阅读器。前四项点击至两次动画帧，阅读器另含读取稳定等待。阶段F未在开工时约定数值预算，故报告绝对毫秒、median/p95和比值，明显退化时继续修正，不补写虚构S0预算；C阶段A13不挪作F的数值门槛。模型/平台耗时和现场SQL延迟不在此渲染比较中。收尾7轮出现阅读器单次尾部波动后，在同条件追加21轮核实，保留原7轮，不改场景或倒填预算；结果与局限见验收报告。

## 6. 完成层级与停止条件

Design/Code已形成可审查候选；前端、正式浏览器链路与完整截图已实跑，具体计数及性能见 [F验收](../../design/acceptance/web-react-001-f-acceptance.md)。原有Topic内核PG有2项未修改代码的失败，不能称自动检查全绿。指定Sonnet因会话额度未完成审查后，Mog明确允许本包用1个GPT只读复核后提交Draft；唯一代理topics_commit_review已核实并复验4项产品兼容问题和捕获等待缺口，最终源码指纹5a545dfb审查PASS。前端490测与最终F链路通过；最终截图、Draft head及CI状态以F验收/PR回执为准。Deploy与Mog签收NOT VERIFIED。产品语义/权限/真实外发/并行共享冲突停止受影响部分；技术实现自行处理。需要回退时以整个PR版本回退，不保留新旧开关。
