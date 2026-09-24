# DOMAIN-UNIFICATION-001 · 平级 Domain 与统一材料链 UI 变更清单

> 状态: 权威当前
> 最后核对: 2026-09-24
> 适用范围: Collection 领域管理、Observation Target role、Evidence Library 与 Comment Study 的 Domain 隔离
> 事实来源: Mog 2026-09-23 的决定、DEC-0008、PAGE-DOMAIN-MANAGEMENT-001、现行 UI 执行合同与 LIDS
> 冲突时以谁为准: 用户最新决定、AGENTS.md、DEC-0008、真实合同与代码；源码实现记录不代表浏览器验收、部署或用户接受
> LIDS 页面层级 / Pattern: L1 Operations / Collection Control

## 0. 读取回执与变更分类

| 来源 | 状态 | 本次采用的依据 |
|---|---|---|
| `AGENTS.md`、`docs/README.md`、`docs/current-state.md` | 已核对 | Domain 统一计划与当前交付边界 |
| `docs/agents/ui-execution-contract.md` | 已核对 | 混合变更按状态/语义与受控写入类别执行；UI 页面、清单和分层验收必须同步 |
| `PAGE-DOMAIN-MANAGEMENT-001`、`PAGE-COLLECTION-001` | 已核对并对齐 | 页面责任、Collection 六面顺序、空/失败状态和逐通道事实 |
| `DEC-0008`、领域数据与行动合同 | 已核对 | 多领域用途、暂停、Request/WorkOrder/材料历史边界 |
| LIDS Language / Data Boundaries / Collection Control | 已核对 | 中文优先、未知不补零、动作按服务端结果表达 |
| Rust SSR 路由、迁移与隔离 PostgreSQL proof | 已核对 | 写入回执和状态读数来自当前候选源码/存储事实 |

- 分类：混合（交互、状态/语义、受控写入）；最高风险：状态/语义与权限/行动。
- 表面：Collection rail、领域列表/设置、Domain–Target 关系、Evidence Library、Comment Study。
- 依赖：领域/关系读取 → 表单 POST 与数据库回执 → Collection、Targets、Corpus、Comment Study 的显式 Domain 读取；共享材料身份不按领域复制。
- 文件边界：本计划列出的 Collection / Evidence / Comment Study / schema / fixture / docs 所有者；不触及插件、外部平台访问、共享数据库迁移、runtime-main 或部署。
- 停止条件：来源状态无法确定时显示 UNKNOWN；规格与真实合同冲突时停止受影响语义；不把实现回执升级成浏览器/运行时/业务验收。

## 1. 用户结果与边界

所有正式 Domain 以相同基础材料能力运行。用户能在 Collection 管理 Domain 和 Target 的 `primary/reference` 用途；Evidence Library 与 Comment Study 显式按 Domain 读取。reference 可完整查看和人工研究选择，但不自动进入本领域默认研究与结论。

本变更不新增跨 Domain 材料身份，不复制 Work/Comment/Media，不发起平台访问，不改插件，不部署。Domain 配置只描述研究上下文；实际访问仍走已有授权和 WorkOrder 链。

## 2. 表面与交互

| Surface | 变化 | 必须保持 |
|---|---|---|
| Collection rail | 追加“领域管理”作为第六个子面 | 保留原五面顺序；保持 SSR + POST/Redirect |
| Domain list/detail | 查看、创建、说明/研究目标、active/paused、Target 关系与 role | 不支持删除；创建不触发平台工作 |
| Observation Targets | 一个 Target 可关联多个 Domain 并分别设置 role | 不复制 Target；未关联者不排活 |
| Evidence Library | 所有 Domain 走同一 Work Resource 与材料 Inspector | Domain 显式；无效范围失败；paused 历史可读；reference 标记 |
| Comment Study | Policy、Run、source selection 与结果显式携带 Domain | 默认 primary；reference 人工选入；Run/结果冻结 role |
| Work lanes | 按每篇作品最新的已接纳通道回执汇总 discovery/detail/comments/replies/media slots/OCR/ASR；媒体字节按本地物化与远程候选事实单独汇总 | UNKNOWN、NOT_REQUESTED、PARTIAL、FAILED、REMOTE_ONLY、LOCALIZED 不压缩；没有请求范围证据时不伪造 NOT_REQUESTED |

## 3. 状态和写入后果

- active Domain 可接收新关系、Request 和研究写入；提交不等于排活或成功。
- paused Domain 不接受新 Request、未开始 WorkOrder claim 或 Comment Study Policy/Run；历史仍可读，已 claim Attempt 有界完成。
- role 变更影响之后的默认用途，不改写已冻结执行和已保存结果。
- 数据不可读显示 UNKNOWN 或失败；不能显示 0、空库或自动回落 ADHD。
- 操作冲突或数据库不可用须说明写入没有成功。

## 4. 验收 ID 与 UI 检查

页面和接口使用 [`domain-management-page.md`](../pages/domain-management-page.md) 的 `DM-01`–`DM-07`，并覆盖：1440 CSS px、键盘与焦点、无横向溢出、空/paused/无关系/读失败/POST 冲突、reference 标记、媒体本地化真相。实现状态可按源码记录；浏览器结果只在实际交付验收后登记，规格本身不证明运行或用户验收。

## 5. 实施与验收状态（2026-09-24）

| 范围 | 源码证据 | 状态 |
|---|---|---|
| Collection rail 与领域页面 | `apps/api/src/local_web/collection.rs`、`apps/api/src/local_web.rs` | SSR 路由与配置/关联操作已实现 |
| Target 领域上下文 | `apps/api/src/local_web/collection_targets_view.rs`、`apps/api/src/local_web/target_drawer.rs` | 新建与既有目标操作显式携带 Domain |
| Evidence Library | `apps/api/src/local_web/evidence_library.js`、`material_projection.rs` | 显式 Domain 读取与统一 Material UI 已实现 |
| Comment Study | `apps/api/src/local_web/comment_study.js`、`comment_study.rs` | Domain、primary/reference 选择与结果 role 已接入 |
| 自动检查 | `scripts/verify-ui-design-handbook.sh`、完整 LOCAL-001 隔离 PostgreSQL 套件 | 通过 |
| 领域管理浏览器交互 | 隔离 PostgreSQL 浏览器流程 | create/edit/pause/resume、同一 Target 在两个 Domain 分设 primary/reference、重复名称冲突保留表单并明确未写入，均已验证 |
| 视觉、响应式与焦点 | `docs/design/acceptance/domain-unification-001-acceptance.md` | 隔离浏览器 1440×1000、390×844 目视与键盘焦点通过；本机部署后 1440/1085/390 的 `scrollWidth === innerWidth`；完整共享 shell LIDS 合规未声明 |
| runtime 部署与 Mog 验收 | `runtime-main@46e10837`、共享迁移 0104、本机 API/浏览器 smoke | 本机部署 VERIFIED；真实媒体/跨 Domain 业务数据与 Mog 验收 NOT VERIFIED |

通道读数仅总结来源事实，不把未出现的通道回执推断成“未申请”；页面提供通往目标/材料读取面的链接以检查原始来源。写入失败通过可见错误反馈说明未写入，并保留当前领域读取上下文。视觉规则沿用 L1 Operations 的 Collection Control；本次不新增 Token、Primitive、CMP、Scene 或 Motion。
