# ACC-COMMENT-STUDY-LAYOUT-001 · 评论研究布局验收记录

> 状态: 一次性报告
> 最后核对: 2026-09-17
> 适用范围: Issue #295 的 `/corpus/comments` 初始研究输入布局；不覆盖 StudyRun、模型、Problem Resolution 或部署验收
> 事实来源: `codex/comment-study-layout-001`、本机临时 loopback 预览、页面静态测试和浏览器可见状态
> 冲突时以谁为准: 当前代码、真实运行回执、用户最新确认与 AGENTS.md

## 本次已验收的页面事实

- 移除了把核心输入挤到首屏以下的视觉 Hero；页面从共享壳层进入紧凑的“准备一次研究”工具栏，再进入作品候选表。
- 已加载的 100 篇 ADHD 作品以连续表格展示：选择框、作品标题与可研究评论数均来自现有 `comment-study.setup.v1` 的 `eligibleWorks`。内部 work ref 只作为 DOM/提交身份，不再挤占用户表格行；没有补造字段或改变 API。
- 桌面表格的表头固定、列表本地滚动；窄屏（390 × 844）释放为自然页面滚动，选择框保持 24px 命中区，窄屏列宽优先留给可换行的作品标题，避免嵌套滚动或标题列被固定数字列挤没。
- 两个区域各自唯一的可用主动作遵循 LIDS Primary：Signal 填充、2px Ink 框与既有 4px 硬投影；不可用时回到禁用的浅面按钮，不伪装为可执行动作。
- 浏览器实际选择第一篇作品、再把筛选条件改为“智商131”后，界面显示“已选择 1 篇”而筛选结果只保留一行。这证明筛选不会从既有的 canonical selected set 中移除隐藏项。
- 本次只进行本地选择与筛选，不点击“保存研究策略”或“创建研究运行”；没有写入 policy、创建 Run、外发模型输入或改动评论研究数据。

## 自动与浏览器证据

| 层次 | 结果 | 证据 |
|---|---|---|
| Rust 页面静态测试 | PASS | `cargo test -p linggan-api local_web::comment_study::tests --locked`：4 passed、0 failed；覆盖共享壳层/L1 表格、筛选保留选择、桌面与窄屏滚动边界 |
| JavaScript 语法 | PASS | `node --check apps/api/src/local_web/comment_study.js` |
| 变更完整性 | PASS | `git diff --check` |
| 本机真实读取 | PASS | 临时 `127.0.0.1:3001` 使用运行库同一数据库配置，`/api/local/comment-study/setup` 返回 100 篇 `eligibleWorks`；页面显示“本机数据库：已连接” |
| 桌面浏览器 | PASS | 真实数据页面首屏可见受控启动工具栏、筛选框和连续表格，无原先的空白 Hero |
| 窄屏浏览器 | PASS | 390 × 844 下配置字段纵向排列，候选表在页面文档流中继续向下，不截断研究输入 |

## 未由本次证明的层次

| 层次 | 状态 | 原因 |
|---|---|---|
| 共享 `main` / PR / 推送 | NOT VERIFIED | 本次只在隔离 worktree 验收，未获本轮提交、推送或合并授权 |
| `:3000` runtime | NOT VERIFIED | 3000 仍是 `origin/main@0a7c940` 的现役快照；本次临时预览为 3001，已关闭 |
| 保存策略与创建 Run | NOT VERIFIED | 这两个动作会写入研究事实，本轮 UI 验收没有触发 |
| 模型 adapter、语义接纳与 Problem Resolution | NOT VERIFIED | 本包不改其合同，也不以页面排版推导其正确性 |
| Mog 业务验收 | 未完成 | 需在获授权合并和 runtime 刷新后由 Mog 在 3000 页面确认 |

本记录不把静态测试、临时浏览器预览或 HTTP 200 写成共享运行、真实模型质量或用户验收。
