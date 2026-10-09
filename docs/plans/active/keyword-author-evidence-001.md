# KEYWORD-AUTHOR-EVIDENCE-001 · 关键词作品作者归属与同页头像证据

> 状态: 活跃计划
> 当前实施层: 隔离候选，未集成
> 最后核对: 2026-10-08
> 适用范围: Issue #369 的关键词作品作者读取、详情页作者主页观察与现有头像媒体链
> 事实来源: 用户 2026-10-08 授权、只读 351 篇现状核对、当前源码与隔离测试
> 冲突时以谁为准: 用户最新确认、AGENTS.md、当前主线代码与真实材料回执

## UI Change Manifest

- Issue: #369；独立分支 `codex/keyword-author-evidence-001`；worktree `.worktrees/keyword-author-evidence-001`。
- 用户可见结果：关键词「作品」列表对已接纳详情优先显示该篇作者昵称；稳定作者 ID 可打开对应平台主页。未知作者继续显示待取得。头像仍由独立媒体状态决定。
- 变更分类：状态语义与展示。页面 L1 连续工作表；维持现有表格结构、按钮和 Token。作者主页链接由该篇详情的稳定作者 ID 构造，属于导航呈现，不表示主页资料已采集。原始观察到的主页 URL 保存在详情来源包内，不把推导链接伪装成平台原始观察。
- 读取回执：AGENTS.md、docs/README.md、docs/current-state.md、docs/agents/ui-execution-contract.md、docs/design/design-governance.md、docs/design/lids/README.md、docs/design/lids/agent-execution-guide.md、docs/design/lids/data-boundaries.md、docs/design/lids/language-policy.md、docs/design/pages/collection-workspace-page.md；当前 `target_catalog.rs`、`target_drawer.rs`、`noteCollector.js`、媒体包路径与数据库样本。
- Surface：仅关键词目标抽屉「作品」表的「创作者」单元格。Data Truth：详情已接纳且作者名/ID有值才显示；发现卡的作者名作后备；两者均未知则待取得。主页链接仅在稳定 ID 已知时启用；无 ID 时仍显示文字。头像缺失、媒体包未接纳、媒体物化缺失各依旧按现有媒体链表述。
- 依赖：合格详情证据 → 作者名及稳定 ID → 列表；详情 DOM 原始 href → `content_detail` 原始包；头像 URL → 现有 `author.avatar` 媒体槽位 → 接纳/物化。无新增平台访问、数据库迁移或权限。
- 验收计划：数据库回归验证发现作者未知而详情作者已知；插件单测验证相对主页 href 保留、外部/错配 href 拒绝；UI 测试验证链接转义、未知保持；`npm run verify`、Rust 定向测试、治理检查。运行时/真实浏览器/历史媒体字节需合并和另行授权部署后验证。
- 停止条件：若现有详情作者字段不能可信对应笔记，则停止该条归属；不利用关键词目标或同名匹配补作者。
- 文件边界补充：本次需修改 `apps/api/src/local_web/target_drawer.rs`、`target_drawer.css` 与其定向测试，以展示稳定 ID 导航；这些文件不属于 #367 当前 worktree 改动。其余边界沿用 Issue 正文。
- 文件边界补充：插件源码改动需要同步 `package.json`、`package-lock.json`、`manifest.json`、`releases/release-manifest.json` 与 `releases/linggan-intelligence-browser-v0.8.60.zip`。本地生成且不代表浏览器已加载。

## 验收矩阵

| 层级 | 验收方法 | 当前结果 | 未证明边界 |
|---|---|---|---|
| 任务可用 | 关键词合格详情作者 SQL 回归、UI 链接单测 | 隔离 PostgreSQL 1/1、UI 单测 1/1 | 合并后真实运行未验 |
| 状态诚实 | 未知作者、ID 错配、媒体状态分开断言 | 插件定向 24/24、完整 verify 通过；缺失媒体不改写 | 历史缺失头像不回填 |
| 视觉一致 | 复用列表现有单元格链接样式，页面尺寸走查 | 待验 | 真实浏览器未验 |
| 真实后果 | 独立 PostgreSQL / 真实插件采集与媒体回执 | 独立 PostgreSQL 通过并清理 | 无共享库写入、无平台访问 |
