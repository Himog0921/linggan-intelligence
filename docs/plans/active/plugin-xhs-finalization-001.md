# PLUGIN-XHS-FINALIZATION-001：采集执行可靠性与统一媒体资源终包

> 状态: 活跃计划
> 最后核对: 2026-08-31
> 适用范围: Issue #128 的 Browser Producer 执行可靠性、统一媒体身份/读取和现有页面消费
> 事实来源: Mog 当前授权、Issue #128、`origin/main@62a60404`、两路只读 Code Review、统一媒体最终决策、当前代码与真实样本
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实运行/代码/版本化合同；自动检查不替代真实链和用户验收

## 用户结果与完成定义

插件按照服务端 TaskSpec 执行目标采集，不静默缩小评论目标、不并发操纵同一页面、不把缺失回执写成成功，也不因 Service Worker 崩溃窗口为同一任务创建第二 Attempt。采回的头像、封面、多图、视频、评论图片和 OCR/转录只形成一套受控媒体资源，现有业务页面不自行拼远程 URL 或各写一套选图规则。

本卡只在以下六层分别有证据时报告对应完成：代码、自动检查、隔离 PostgreSQL、发行包、真实链、本机部署/用户验收。任一层不得替代另一层。

## 冻结范围与非目标

本卡实现 Issue #128 的十二项固定范围：派发/页面回执 fail-closed、缓存 lane 的幂等 outbox、评论 Attempt 单一执行状态、批量超时取消与 drain、缓存回收、每用途媒体 ordinal、七类媒体关系、统一 MediaResource、唯一封面规则、现有页面只读本地句柄、三个评论计数贯穿和 0.8.16 可复现发行包。人工媒体下载窗口必须保留。

不实现 Intelligence 分析、自动选目标、万能 PresentationResolver、新页面、第二套资产模型、云 OCR/ASR、历史 Package 改写或破坏性回填。不把不存在的 Topic/Actor/Comment 页面写成已迁移。

## 两路审查结论

### Standards 轴

1. outbox 入队与 cache 标记存在崩溃窗口；
2. `mayExecute=true` 的派发体未在运行边界执行完整 TaskSpec 校验；
3. 页面缺失/畸形回执可被写成 `executed=true`；
4. stop 后立即 restart 可形成两个 collector；
5. 详情缓存从不主动回收；
6. 评论 API/DOM 两支重复维护状态，缺少单一 Attempt seam；
7. 仓库保留 0.8.1–0.8.15 全部 ZIP，违反当前发行物登记。

### Spec 轴

1. scheduled 不限评论在页面控制器被改成 1 条；
2. 批量单篇 timeout 不取消旧 collector；
3. `cover:1/cover:8` 分裂且封面选择优先级错误；
4. 当前媒体 schema 只接受 content 的四种 role；
5. Work Resource seam 未形成统一 MediaResource，目标页仍渲染远程头像；
6. 真实样本已有 8 个 slot 但无 Materialization；
7. Resource read 丢失 `requestedLimit`；
8. 搜索/批量数量仍有固定候选和静默 50 上限。

## UI 变更清单

### 读取回执与分类

已读取 AGENTS/current-state、UI execution contract、design governance、LIDS README/Primitive/Pattern/Agent guide、PAGE-EVIDENCE-001、Work Resource Read 与 Media Lifecycle 合同。变更分类为“状态/语义 + 展示”的混合变更；不新增权限/行动。页面为 L1 Corpus Explorer 加既有受限 L2 Inspector，主 Pattern 不变，Token/Primitive/Shell 不变。

### 表面地图

| 表面 | 本卡责任 | 唯一数据来源 | 禁止替代 |
|---|---|---|---|
| Evidence 作品列表 | 显示 MediaResource 选出的本地封面及真实不可用状态 | Work Resource `media` | `preview` 私有猜测、远程 CDN |
| Evidence Inspector | 显示同一资源的图片/视频/派生和关系状态 | Work Resource `media` | 直接拼 Slot/Derivative 选图 |
| Collection Target | 头像仅在统一本地资源可用时内联 | MediaResource avatar | `identity_facts.avatar` 远程 URL |
| 评论任务面板 | 分开显示已取得、页面公开数、请求上限 | Comment receipt/Resource read | 模糊 `current/total`、`130/53` |

### 状态词典

- `AVAILABLE/INLINE_SAFE`：有受控同源句柄，可以内联；
- `OBSERVED`：只看到来源槽位，不等于字节取得；
- `NOT_OBSERVED/NOT_REQUESTED/NOT_ENABLED/FAILED/RESTRICTED`：原样表达，不用远程 URL 回退；
- `selectedBy=explicit_cover/first_body_image/video_poster/none`：封面选择事实；fallback 不是平台显式封面；
- `PARTIAL`：已取得数据仍可用；不冒充 `COMPLETE`。

### 依赖与文件边界

| 依赖 | owner | 本卡用法 | 不得做 |
|---|---|---|---|
| Media Lifecycle V2 | 现有合同 | 继续复用 Slot→Observation→Download→Blob→Materialization→Derivative | 新建第二资产模型 |
| Work Resource Read | 现有共享 seam | 扩为统一 `media` DTO | 页面专用 SQL/API |
| Evidence PAGE/LIDS | 现有页面规范 | 只替换媒体消费和 3:4 展示策略 | 改页面骨架/Token/Shell |
| Browser Producer | 本卡 | 统一执行状态与媒体 identity | 分析/选目标/风险绕过 |

### 验收矩阵

| 层级 | 验收方法 | 通过条件 | 未证明边界 |
|---|---|---|---|
| 任务可用 | 评论生命周期、批量取消、dispatch/outbox 合同测试 | 无静默限额、重叠 collector、假成功或重复 Attempt | 真实平台长期稳定性 |
| 状态诚实 | DTO/UI fixture 与 receipt 断言 | 三个评论数分开；媒体未知/失败不回退远程 URL | 平台隐藏数据 |
| 视觉一致 | Evidence UI 自动检查与桌面/窄屏检查 | 3:4 只是 UI 策略；intrinsic dimensions 保持事实 | Mog 最终审美验收 |
| 真实后果 | 单一真实样本一次验证 | 评论 Package/Receipt 与至少一条媒体 Materialization，或精确失败回执 | 其他作品/账号代表性 |

## 实施步骤

1. 关闭执行合同阻断：claim decoder、page receipt、幂等 outbox、session prune。
2. 建立单一评论 Attempt seam：不限/有限配额、API/DOM、暂停/停止、restart 互斥、batch cancel/drain、receipt。
3. 建立统一媒体身份与 Resource read：每用途 ordinal、历史 cover 兼容、七类关系、封面优先级、尺寸事实、评论计数。
4. 迁移现有消费者：Evidence 和 Collection Target 只使用共享本地资源；保留人工媒体窗口。
5. 升级 0.8.16，清理陈旧 ZIP，运行一次完整自动/隔离数据库/发行验证。
6. 等 Mog 重载精确包后，只对当前授权样本运行一次真实链；冻结 exact head 并提交 PR，等待单独合并授权。

## 当前执行状态

- 步骤 1–5 及终态评审修正已在 `v0.8.28` 完成；231 项插件测试、Rust workspace、49 项隔离
  PostgreSQL proof、发行校验与可复现重建通过，等待冻结 commit/PR。
- 0.8.27 已用授权样本完成一次干净真实详情链；0.8.28 新增评论图片能力目前只有自动与隔离数据库
  正向证明，未把没有非空评论图片的真实样本写成平台正向验收。
- 本机持久数据库和 `:3000` 的 `0030`/runtime exact-head 更新、Chrome 0.8.28 重载与最终业务验收
  继续分层记录，不由测试或 ZIP 存在性替代。

## 2026-09-01 终态评审收口

0.8.27 已完成一次干净真实标准详情链，证明详情、评论、回复、作者头像、封面、正文图与 Live Photo
字节均能进入 Intelligence。最终 Spec Review 只留下两个与原完成定义直接冲突的阻断：标准详情没有
把评论图片交给统一媒体资源；回复队列失败会污染已成功的评论 lane。0.8.28 在同一终包关闭二者，
并以 additive `0030`、JS 合同测试、Rust 接纳/读取 proof 和完整发行门禁收口。没有非空真实评论图片
样本时，只报告自动与隔离链通过，不伪称平台正向样本已验证。

## 2026-08-31 受控后续：0.8.19 作者头像与 Evidence 身份分栏

Mog 已授权对同一作品“ADHD的尽头是成瘾”重新执行详情与评论采集，并要求所有实际取得的媒体在 Evidence Library 可见。范围固定为：详情 `authorId + authorAvatar` 形成独立 `author.avatar` 槽位并走既有媒体链；Work Resource 返回受控本地头像；Evidence 把作品作者和监控目标拆成两个事实区，并在媒体 Inspector 连续列出头像、封面、正文图片、视频和派生资源。评论总数持续增长按平台真实进量处理，最新 Attempt 与历史累计仍保持不同口径。

本后续不允许远程头像/CDN 回退、不把监控目标填成作者、不把头像送入 OCR、不改评论重采与完整性规则，也不新增第二媒体表。自动检查、隔离 PostgreSQL、0.8.19 发行包、本机 migration/runtime、Chrome 重载、真实 Package/Receipt/Materialization 和页面验收继续分层记录；本段不预先宣称真实链完成。

## 停止条件

需要历史破坏性改写、第二资产模型、新页面/产品决定、扩大真实目标、绕过登录/风控、暴露签名 URL/敏感原文、或覆盖并行修改时，停止受影响部分并报告。没有真实来源的 avatar/comment image/OCR/transcript 必须保持 `NOT_OBSERVED`，不能为通过验收伪造。
