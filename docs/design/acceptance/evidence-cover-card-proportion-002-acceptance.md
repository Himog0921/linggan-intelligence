# ACC-EVIDENCE-COVER-CARD-PROPORTION-002 · 封面卡比例候选验收

> 状态: 一次性报告
> 最后核对: 2026-09-15
> 适用范围: `EVIDENCE-COVER-CARD-PROPORTION-002` 的交付分支候选、静态合同与受控本机只读视觉预览
> 事实来源: `codex/corpus-cover-card-proportion-002` 工作树、自动检查输出、临时本地代理截图与既有本机 `:3000` 的 Work Resource 读取
> 冲突时以谁为准: 真实分支/测试输出/运行服务与 Mog 前端验收；本报告不构成合并、部署或业务验收

## 结论

候选已经把封面作品收束为一个有主从尺度的档案组：固定两行标题和共享 3:4 Stage 组成高主视觉；作者、日期、互动、取得状态和最近观察收进低矮横向铭牌。六列扫描密度继续由既有 `minmax(248px, 1fr)` 网格决定，没有通过减少列数来回避比例问题。

## 场景矩阵

| 场景 | 用户任务 | 预期状态含义 | 视觉检查重点 | 真实后果/回执 | 结果 |
|---|---|---|---|---|---|
| 选中作品 | 浏览封面并定位当前 Work | `aria-selected` 仍是当前选择，不是质量等级 | 主视觉与铭牌整组 Signal；Stage 保持白色档案框 | 既有 Inspector 读取同一 Work；无写入 | VERIFIED |
| 一行/两行标题 | 快速扫读不同长度标题 | 截断仍是已有作品标题，非改写 | 固定两行标题格；相邻 Stage 顶边对齐 | N/A | VERIFIED（源码合同 + 1920px 预览） |
| 单封面材料 | 扫读大量同类图文 | 无更细媒体类型时不伪造分类 | 稳定 Work Reference 在 grid / cone / frame 间选择；视频、多图、已知评论仍优先专属图形 | 无数据写回、无新字段 | VERIFIED |
| `PARTIAL` | 看见取得状态与最近观察 | 保留已有状态和 tooltip 材料事实，未知不变成 0 | 低铭牌只保留状态/最近观察，不再显示彩色 material rail | N/A | VERIFIED（运行时合同） |
| 翻面与移动端 | 在主视觉查看本机封面 | 继续使用既有 hover/tap/键盘合同 | 只有主视觉翻转，铭牌不参与 | 不触发采集或媒体写入 | NOT VERIFIED（本轮未做触摸实机复测） |

## 视觉工作条件

- 桌面核对：`1920 × 1400` 无头 Chrome；临时只读代理保留 `:3000` 已加载的基底 CSS/读取接口，并将候选的页面局部 CSS 与 JS 叠加为末级规则。列表实际加载 50 条本机 Work Resource；截图仅用于本次走查，未登记或提交到 Git。
- 观察结果：同一行主视觉的 Stage 顶边固定；数据铭牌为 `aspect-ratio: 7 / 2`、最小高度 112px，并以作者/日期、四项读数、状态/观察三条横带组织。已知发布日期保留 `YYYY-MM-DD`；只有来源文本时间显示「来源时间」，完整来源文字保留在元素 `title` / Inspector。观察时刻为 `MM-DD HH:mm`，卡面状态只显示中文词；跨行业样本的边界说明只占底带第一格，不生成隐式第二行。主视觉→铭牌为 `space-3`，整组→下一行为 `space-8`。因 Inspector 同时打开，2048px 截图可见五列；原有 `auto-fill` 网格未被改为固定五列，宽屏仍可呈现六列。
- LIDS 依据：既有 `--lgi-*` spacing、duration、ease、canvas、signal 与 ink token；无渐变、玻璃、阴影升级或全局 token 改动。
- 未登记临时目录和本地代理均只服务预览；没有 API、数据库、采集、平台、媒体或运行服务写入。

## 分层结论

| 完成层 | VERIFIED / NOT VERIFIED / N/A | 证据 | 仍有限制 |
|---|---|---|---|
| 设计规格一致 | VERIFIED | Manifest 的 3:4、主从比例、组内/组间节奏和整组选择态均有 CSS 合同 | Mog 视觉主观验收未完成 |
| 前端/组件实现 | VERIFIED | 仅 `evidence_library.css/js` 的现有 cover component 路径及其合同测试改动 | 无组件 API 或数据模型重构 |
| 自动检查 | VERIFIED | `node --check`；focused cover contract 1 passed；`evidence_runtime` 9 passed；`git diff --check`；UI handbook 与治理检查通过 | Rust 测试产生 21 条既有 dead-code warning，未由本包引入 |
| 真实链路/回执 | VERIFIED（只读呈现） | 50 条真实本机作品在临时只读叠加预览中加载 | 不证明采集、媒体生成或外部平台状态 |
| 部署 | VERIFIED（本机 runtime） | 提交 `a4e7f7a` 已推送分支；`main` 合并提交 `36ee8bb` 已推送 `origin/main`；受控 install 将 `runtime-main`、`origin/main` 同步到 `36ee8bb`，三个 launchd 服务 running，`/health` schema READY、目标 Evidence URL HTTP 200 | 不证明远端生产环境或 Mog 业务验收 |
| Mog / 业务验收 | NOT VERIFIED | 等待前端实际审阅 | 不得以本截图代替用户验收 |

## 后续边界

- 本轮不需要 `DECISION_REQUIRED`：用户已确认六列是正确的密度，问题是卡片主从比例和换行节奏。
- 若需要更具业务语义的几何分类，必须先在 Work Resource 读取模型中已有可验证媒体类型后再提出；本候选不会从标题或内容猜测分类。
- 提交、推送、合并到 `main` 与刷新本机 `:3000` 已在 Mog 明确授权后完成；Mog 前端验收与真实触摸复测仍是独立未证明层。

## 交付位置

- 分支：`codex/corpus-cover-card-proportion-002`
- Worktree：`/Users/moglenny/proma/linggan-intelligence/.worktrees/corpus-cover-card-proportion-002`
- 关联清单：[evidence-cover-card-proportion-002-ui-change-manifest.md](../changes/evidence-cover-card-proportion-002-ui-change-manifest.md)
