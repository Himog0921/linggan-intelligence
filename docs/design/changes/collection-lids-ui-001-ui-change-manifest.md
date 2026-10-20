# COLLECTION-LIDS-UI-001 · 采集三页 UI 变更清单

> 状态: 活跃计划
> 最后核对: 2026-10-20
> 适用范围: 观察目标、采集任务、工位管理的真实 React 前端
> 事实来源: Mog 当前对话直接派定；LIDS Q01–Q12；PR #457@4700352b；既有 Collection API
> 冲突时以谁为准: 用户最新确认、真实产品/数据/权限合同；LIDS 已确认参数源

## 用户结果与责任

单 Agent root，Claim codex/collection-lids-ui-001，专属 worktree collection-lids-ui-001。一个三页交付包；不派生任务或启动其他 Agent。基于 #457 已提交设计基础，未复制其未提交壳层修复；合并发布前须解决 #457 依赖及 main 对齐。展示与既有交互适配，不改 Rust/API/数据库/权限/插件/模型/正式3000。

视觉：连续白场、黑色结构、少量橙色主动作；OPPO Sans 正文、Manrope 数据、JetBrains Mono 机器编号。B 字级、M2 控件、C 焦点。CSS策略为现有CSS Modules消费唯一Token；页面定位沿Z02/侧栏，不新增重复标题或仪表盘。交互沿既有按压/焦点/浮层规则，reduced-motion保持。

## 表面与依赖地图

| 表面 | 组合与实现 | 依赖与限制 |
| --- | --- | --- |
| CollectionFrame | Z01/Z02复用AppShell，Z03复用Sidebar；紧凑结果披露 | 仅改CollectionFrame，不修改全站壳层/字体/Token；管理入口及返回领域保留 |
| 观察目标 | P08/B，分类/排序/批量合带，新建字段回所属表单，creator/keyword两张比较表 | 所有抽屉/规则/命令身份保留，不添加伪查询或新数据 |
| 采集任务 | P09/B，FIND/筛选/集合动作；L1 44px/名称144px单行账本；字段自定义/局部横滚 | 使用既有Run字段，rank未投影则明示；进展/成果保留，旧技术诊断按下文截图反馈退役 |
| 工位管理 | P11/C，常驻工具，白场工位列表与通道判定，概览沿已有抽屉 | 认领/账号/能力/改名/接活/停用入口完整；M-02E仅原许可位置 |
| 浮层 | 详情及表单就近，使用既有S1与原请求保护 | 关闭不冒充取消，回执不冒充执行成功；不新增命令 |

## 状态词典

加载、已知空、读取失败、保留旧值、部分成果、未知、受限、处理中和结果未确认分开。null不是0，材料入库不是采集全部结束，心跳过期不是正在执行。工位恢复接收不等于恢复停止任务。真实动作继续依赖服务端回执，不用本地状态宣称成功。

## 验收矩阵

| 类别 | 验证 |
| --- | --- |
| 回归 | 既有目标/任务/管理测试，必要更新布局/入口；strict类型/lint/构建 |
| 数据边界 | 长名称/编号、0/null、大数、空/读失败/部分成果、字段显隐；一个任务一行 |
| 交互 | 筛选/URL/搜索组词、键盘、详情及返回、新建/批量入口；命令原身份/回执保持 |
| 浏览器 | 私有GET桥+候选真实React；1440/1280/820/390及缩放等价宽度，无根溢出/局部横滚 |
| 标准 | Token同步、LIDS对齐/手册、治理、diff；截图日志私有不入Git |

正式写请求、真实采集/平台/模型、部署及Mog视觉业务签收均NOT VERIFIED。只读预览在转发前拒绝非GET/HEAD，其拒绝不代表业务API失败。产品语义/权限/共享并行冲突停止受影响部分报告，不修改设计系统绕过。

## 2026-10-20 · Mog截图反馈续修（同一交付包）

来源为Mog本轮七张截图及明确修订要求，沿当前#458@1a92405e继续；不创建新任务/代理/分支。采用现有LIDS Token/C焦点/M2及本轮页面具体裁定：标题向上收紧约5–7px（按8px Token阶梯中的组合实现，实测报告）、编号和正向新增采用signal-ink；所有吸附格与整行hover同源背景。creator/keyword分别拥有在本表范围内有效的冻结标题/字段，横向位置与该表数据同步，不把前一张表字段带入下一张表。

任务页删除旧“历史执行记录与诊断”及无调用者的旧前端诊断组件/适配器，后端历史数据/API不删除；详情删除原始JSON/版本/UUID/租约等技术诊断展示，保留进展、成果、缺口、来源/范围、工位入口、停止确认及未确认原请求核实。详情默认紧凑宽度，顶沿不高于实际共享页头含面包屑下沿；去除大占位和无业务价值的重复解释。搜索复用现有检索组合的完整FIND边界/输入/C焦点与M2提交按钮，保留手动提交、IME、URL/清空语义，不新增查询规则。

文件边界为本包CollectionFrame/目标表/任务页/详情CSS Modules及必要回归；唯一Token、共享AppShell、后端、数据库、插件不改。验收新增：真实hover各格颜色一致，creator与keyword各自纵向冻结/横向对齐且换表不串字段，标题位移，任务详情实际top/width/滚动/返回，旧诊断DOM与请求不存在，搜索完整边界/IME/空结果/清空及原命令保护。全部在GET-only预览验证；证明写入已登记的/tmp/collection-lids-ui-001/feedback-02/，不入Git。

## 2026-10-20 · 工位管理弹窗与导航收口（第三轮）

Mog直接要求主新建按钮右对齐、删去侧栏“采集与观察”、工位管理改居中弹窗，核心只保留新增工位、工位及能力、今天在跑什么、自动观察排程与插件认领。该最新确认局部替代PAGE-COLLECTION-001“管理面整体复用原页面与全部能力”的呈现要求：运行大看板/运行概览与独立管理页面退役为历史；工位的改名/接活/账号/能力/认领窗口/停用仍归所属工位，不新增后端能力。

表面图：Tasks工具栏主动作右对齐；CollectionFrame删导航分组字样，Z02工位入口用URL-owned runtime=1打开居中原生ModalDialog，旧runtime深链转到安全返回页并打开弹窗；弹窗含工位/今天在跑什么/自动观察排程/插件认领四页签，新增工位为所属表单。工位用紧凑对象行及能力/设置展开区代替十列横向宽表；插件最近报到优先、历史安装折叠，原生required阻止未选工位提交。

状态图：未知/已知空/读取失败及保留旧值独立；连接/能力/执行/额度分别呈现，不把已有插件数当在线数。只展示真实running当前执行，不伪造今日累计。原CommandForm请求身份/回执/未确认状态保留；关闭弹窗遇未保存/在途表单先明确继续或放弃/收起原请求，迟到回执不跳页。

依赖图：复用现有ModalDialog、ActionButton、CommandForm、managementApi与唯一Token，不改共享壳、后端/DB/插件；新增RuntimeManagementDialog.tsx/RuntimeManagementDialog.module.css为Collection专属适配，RuntimeContent.tsx及StationManagement.tsx拆分现有领域呈现，测试仍在management/CollectionFrame相关同目录。旧RuntimePage及runtimeDestination.ts只做兼容重定向，运行概览组件由Git保留可追溯历史。私有截图日志仍在已登记/tmp/collection-lids-ui-001/feedback-03/。

验收图：按钮右缘/无导航分组；两个日常页打开关闭弹窗保留完整URL/分页/焦点；旧runtime含station与return_to的深链安全恢复；四页签真实与未知状态、新增及未保存/请求收起保护、插件空选择不POST；真实浏览器1440/1280及窄屏对话框/内容局部滚动/各页签/能力/认领/关闭返回，GET-only不触发写；全量verify、Token/LIDS/手册/治理/diff及AGENTS要求提交前只读commit-reviewer复核。无main/3000发布。

提交前复核：按本树AGENTS新增明确要求执行一次commit-reviewer只读子代理（与原单实施Agent声明区分）；定义中的Sonnet本会话不可选，使用继承模型。两项P2经隔离复现：URL后退绕过脏稿、受控认领下拉无法承接原请求草稿恢复，均已修复并加回归；改用项目现有useBlocker/beforeunload及非受控required下拉。复核四文件106项通过、最终未处理发现0，仅本地提交前复核，不替代远端CI/整包独立发布复审。

浏览器追加依赖：真实popstate在useBlocker回退时会暂时改变window.location，现useManagementRead以原始window.search作queryKey，导致弹窗内表单重挂载。必要依赖明确纳入useManagementRead.ts与managementApi.ts：使用React Router确认的search为同一读取key/请求上下文，不新增接口/数据语义；原APIload仍支持默认search以兼容既有调用。验收真实浏览器后退被拦截后继续，表单和输入均不丢。

## 2026-10-20 · #457/#458 授权整合与本机发布

Mog明确接受候选，并授权核对两PR依赖/检查，无问题合并并更新3000。当前#457@6fdbe9a1与main@2dc1f520冲突，#458@e99e18d8基于旧4700352b且为Draft；最新head无远端checks，旧自动review额度耗尽不作PASS。先在专属`.worktrees/lids-release-integration-001`整合#457与main，保留两边既有能力，提交前按AGENTS独立只读复核；先#457后#458转main并合并已集成依赖。必要共享冲突限既有来源合并，不新增功能。四图沿原表面/状态，新增验收精确head/远端CI或缺失明示/本地隔离回归/合并main/运行PID与前端身份/实际3000页面。

部署仅从干净的目标main使用现行installer，复用canonical runtime的.env不展示或提交；先只读核对migration与在途调用，原禁用worker不启用。若需不在既有授权范围的不可逆迁移则停止该部分报告，不因旧PR正文授权而执行。私有发布证据为已登记/tmp/collection-lids-ui-001/release-01/；不改共享开发目录未提交文件，不调用真实模型/平台/采集。Mog视觉接受仅针对4196候选，实际3000需复验。

发布检查兼容性：项目现存Collection隔离浏览器证明仍定位旧表格/诊断/工位大页面。为验证本轮用户已批准呈现，必要纳入scripts/test-collection-single-run-ui.cjs、test-collection-management-json.cjs、test-collection-targets-json.cjs；只调整真实UI入口/选择器/业务文案与退役诊断断言，保留所有领域/工位/请求幂等/材料/分页/跨域/真实库后果检查，新增布局和无诊断回退断言，不降低HTTP/DB/请求验证。不在CI调用的旧同数据视觉对比脚本保留历史，不伪称本次运行。

本地整合验收回执：97文件/1063测试及types/lint/build通过，治理213项通过，独立commit-reviewer无未处理发现。Collection隔离PG核心14项通过、5条route合同通过；真实browser任务/分页/跨领域/原请求GET回执/51唯一作品通过，目标11合同通过，工位6合同/18次隔离UI命令通过，未访问外部平台，临时容器与卷清理verified。目标脚本回执status定位收窄到原alertdialog，避免新选择计数status导致严格匹配重复；仍核完整lostRef。远端CI/两PR最终整合与3000运行证明继续待后续精确head确认。
