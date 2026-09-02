# AUTHOR-TARGET-SYNC-001：作者资料到观察目标与本地头像闭环

> 状态: 活跃计划
> 最后核对: 2026-09-02
> 适用范围: Browser Producer 已缓存作者资料到 Linggan 观察目标、独立作者头像本地物化与目标页展示
> 事实来源: 当前候选代码、隔离 PostgreSQL 证明、插件发行候选与 Mog 当前交付授权
> 冲突时以谁为准: Mog 最新授权、`AGENTS.md`、版本化合同、当前代码、数据库及运行时事实
>
> 当前阶段: 实现与隔离证明已完成；现按 Mog 于 2026-09-02 的上线授权，执行合入、共享 migration、运行时切换、真实 Chrome 链和业务验收。
> 交付 checkout: `codex/author-target-delivery-001`，从 `origin/main@26f5e4b805906497e97510988fa8fa5d4f0b9897` 创建。

## Claim

一次人工采集或从 Browser Producer Dashboard 明确重投递的 XHS 博主资料，必须先通过既有
`TaskSpec → Attempt → immutable CapturePackage → Submission Receipt` 本机 outbox；仅在服务端
接纳该 `author_profile` Package 后，按 `(platform, creator, stable author id)` 幂等创建或补全
观察目标。公开资料（昵称、简介、账号、计数、头像来源）来自 Package，绝不以监控目标或 URL
猜作者身份。头像只有在 Producer 采集、既有媒体管道本地物化并产生受控本机资产时才以内联图像
显示；尚未物化时页面显示诚实状态，绝不回退渲染平台 CDN URL。

## 范围与非目标

范围：

- XHS `author_profile` 被接纳后的观察目标创建/资料合并与可见回执；
- Dashboard 已缓存、已选中博主的显式重投递，复用既有本机 outbox；
- 独立作者头像的 `media_slots` 合同、接纳、采集工作、Blob/Materialization 与观察目标本地读取；
- 观察目标页面的头像及“本地物化中/不可用”状态；
- 单元、路由、隔离 PostgreSQL、插件构建和发行候选核验，以及本月进度记录。

非目标：

- 不读取平台新页面、不绕过登录或风控；
- 不把 Dashboard 缓存直接当 Evidence，也不新建第二个同步 HTTP 通道；
- 不把作者头像伪造为某篇作品的封面或正文媒体，不创建虚构 Work；
- 不改共享 checkout、现有运行 snapshot、已安装 Chrome 扩展、远程 PR 或部署；
- 不把 HTTP 200、构建或孤立数据库证明表述成 Mog 的真实业务验收。

## 验收矩阵

| 层 | 通过证据 | 不替代 |
|---|---|---|
| 合同/代码 | 独立作者头像必须有稳定 author id；现有 detail-context avatar 继续兼容 | 真实 Blob |
| 数据库 | 新迁移可在隔离 PostgreSQL 应用；无虚构 content；目标按稳定 id 去重 | 共享库已迁移 |
| API/回执 | 接纳 Package 后才创建/补全目标；目标同步失败使 outbox 保持可重试 | 前端按钮文案 |
| 插件/UI | 手工采集自动入队；已选缓存作者可入队重投递；界面只报告 queued/receipt 状态 | 服务端接纳 |
| 媒体 | 目标页只用本地 `/api/local/...` 资产；未物化不远程回退 | 所有历史头像已回填 |
| 真实链 | Mog 授权的单个作者形成 Package、Receipt、Target、Blob/Materialization 与目标页 | 自动测试 |

## 停止条件

如果实现需要跨仓改内容工作台、改历史不可变 Package、无授权的平台访问、伪造内容上下文、覆盖并行
修改、部署/重载/合并，停止对应动作并单独报告。新 Chrome 发行包仅在本地构建为候选，除非 Mog
明确授权安装/重载。

## 本次完成证据

- `0032_author_profile_avatar_media` 以 additive 迁移放宽 author-owned `media_slots` 的合法
  上下文；普通内容媒体与既有详情上下文头像继续保留，独立头像不创建虚构作品。
- 接纳 author Profile 后，服务端以稳定作者 ID 幂等创建/合并观察目标；API receipt 返回
  `targetSync`，失败不会被当成已完成。
- Dashboard 的已选作者会复用同一 durable outbox 重投递；按钮和消息只陈述 queued，不等同
  receipt、目标可见或本地头像。
- 观察目标只呈现本机 materialization URL；远程头像 URL 仅是证据来源，不是页面 fallback。
- 已通过 Rust workspace、完整受控 PostgreSQL proof、插件 232 项验证、Dashboard focused tests、
  候选 ZIP 完整性和可复现性。所有 PostgreSQL proof 资源已由脚本清理。

## 仍待的独立门

- 共享数据库应用 `0032`，从该候选源码构建并切换 API、scheduler、media worker；
- 安装/重载候选 Browser Producer `0.8.29`，并在真实 Chrome 以一位缓存作者走出 Package、Receipt、
  Target、媒体 Materialization 和目标页本机头像；
- Mog 在 `:3000` 的观察目标页面做业务/UX 验收；
- `/collection/tasks` 与 `/collection/operations` 的独立 read-model 缺口需另立范围，不属于本卡。
