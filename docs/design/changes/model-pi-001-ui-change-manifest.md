# MODEL-PI-001 UI 变更清单

> 状态: 权威当前
> 最后核对: 2026-09-06
> 适用范围: Issue #169；MODEL-PI-001 已授权模型设置与评论分析包
> 事实来源: 用户当前派定、Issue Claim、LIDS、0040 与当前 API/worker
> 冲突时以谁为准: 用户最新确认、AGENTS、真实数据/权限与 LIDS

读取回执：AGENTS → docs README → current-state → governance/agent-collaboration/file-placement 与 Issue 规则 → UI execution contract → LIDS README、tokens、patterns、materials、shell-zones、language-policy、data-boundaries。预先表面/状态/依赖/验收矩阵在 [实施计划](../../plans/active/model-pi-001.md)。本包是状态、权限/行动与交互的混合变更。

- 表面：`/settings` 到 `/settings/models`；个人菜单；四区设置和必要 dialog；评论原声的当前模型提示/试运行入口/默认版本问题候选。无额外一级页面。
- 分类：L1 Settings / Governance，单一 Form Surface；Token → 现有边界/按钮/表单 primitive → 连接/模型/计划与来源选择 → 四区功能页。共享 shell 只有菜单入口，专用类 `lgi-model-*` 不覆盖壳层。
- shell-zones 适配：过去系统区的状态+未实现 command 两槽，本包保留状态槽，把 command 槽用于用户明确要求的个人菜单及设置入口；仍只有两个全局槽，不加第三排控件。用户当前明确需求优先于旧 command-only 表述。这是局部入口变更，不整改其它导航欠账。
- 状态：保存未测、目录成功、可调用、评论校验成功、未配置、已停用、等待、执行、无信号、失败、用量未知与金额未知分开；派发/输出/当前资格来自后端。
- 集中整改 R1：计划行内显式恢复和确认 dialog，保留来源/配置/次数/额度，修订冲突可操作；重复来源回执链接至真正所属计划，页面按指定计划读取和定位。复用原表格/dialog，不增加历史工作台。R2/R3 修复自动选源和中断回执，使既有数量/状态有真实依据；浏览器最终验收仍由根代理负责。
- 依赖：唯一实施 child；根代理固定 HEAD 一次集中审核。直接共享接缝为 API/Shell、comment work/read、worker 异步组合、Cargo/0040、实际 migration 登记、Node lock/runtime 准备及必要扫描排除安装依赖。无其它包源文件所有权冲突。
- 权限：loopback Host/Origin/no-store 复用；秘密 Keychain，仅有界 stdin；受控 fixture 是显式隔离模式。保存不调用；显式试运行、自动启用、独立历史范围；停用不派新调用，进行中的远端取消不保证。
- 非目标/停止：不部署、迁移共享库、读其他凭据、真实外发材料或擅自启动现有服务；真实外部质量和语义聚类不能用 SDK/fixture 证明。没有新 reviewer；审核 verdict 在 PR，不写入代码分支产生循环。
- 验收矩阵：设置→Pi→工作→严格输出/标注、未知用量/失败/暂停/切模型/重放、Source 资格、HTTP/静态页面、随机 Keychain、真实 SDK 三协议分别在 [验收记录](../acceptance/model-pi-001-acceptance.md) 标明层次。浏览器最终验收由根代理完成。

## 同包返修

用户最新决定为单页面+单连接弹窗，替代四区常驻导航。表面、状态、依赖和验收矩阵见实施计划的同包返修章节。复用 L1 Form Surface、既有原生 dialog、LIDS token 与中文错误；单实施者为根代理，工作树 `model-settings-repair`。
