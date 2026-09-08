# ACC-CORPUS-CROSS-DOMAIN-RENDER-001 · 外部领域样本与领域菜单

> 状态: 一次性报告
> 最后核对: 2026-09-08
> 适用范围: #130 的既有外部领域读取展示与领域切换器；不含领域自助配置能力
> 事实来源: 当前候选分支、focused Rust/DOM contract tests、候选 loopback 浏览器走查与本机只读样本接口
> 冲突时以谁为准: 用户最新确认、真实代码/读取回执、AGENTS.md 与接受的领域/设计合同
> 页面: `GET /corpus/evidence?domain=00000000-0000-4000-8000-000000000002`
> 关联: `PAGE-EVIDENCE-001` §5.4、`CORPUS-CROSS-DOMAIN-RENDER-001`、LIDS v7

## 1. 验收对象与前提

- 候选源码：`codex/corpus-cross-domain-render`，基线 `829fe22e60a5cfa60a0e130ae58d87f79fb3cba3`。
- 候选服务：本机 loopback `http://localhost:3107`；用当前本机数据库只读启动，未运行 migration、未写入领域、未触发采集、详情、评论或媒体动作。
- 数据前提：`/api/local/cross-industry/samples?domain=...0002` 在本次验收返回 21 个列表级样本；这不证明详情、原文、评论、媒体或本领域 Evidence 已取得。
- 视觉条件：in-app Browser 候选页，约 1280×720 桌面视口；未将截图或录屏写入仓库。

## 2. 场景矩阵

| 场景 | 用户任务 | 预期状态含义 | 实际证据 | 结果 |
|---|---|---|---|---|
| 外部领域有样本 | 选择“考研自习”并阅读结果 | 21 是跨行业列表级参照样本数，不是 Evidence/详情数量 | HTTP 200、`items.length=21`；浏览器列表渲染 21 行，首行标题为“我真忍不下去了…” | VERIFIED |
| 选中外部样本 | 查看右侧 Inspector | `sampleRef` 只能承载列表级事实；未读取字段不是失败 | Inspector 显示“跨行业列表级参照样本”“列表级字段”“不参与本领域判断”；打开原文/补采禁用 | VERIFIED |
| 外部样本未知字段 | 看见未知作者、发布时间、互动或封面 | `UNKNOWN` 不写成 0 或已取得 | 浏览器逐项显示“当前未知”“发布时间当前未知”；无远程封面回退 | VERIFIED |
| 不支持的工作台控件 | 尝试检索、材料筛选、状态视图或排序 | 外部列表模型尚未提供这些查询合同 | 相关控件 disabled，并提供“当前只提供列表级字段”的原因 | VERIFIED |
| 领域切换器 | 展开领域菜单 | 当前观察对象可切换；不提供领域配置 | 页面无原生 `<select>`；展开为方形白色链接菜单，当前“考研自习 / 21 条样本”反白，三个领域链接可达 | VERIFIED |

## 3. 视觉与可访问性检查

- LIDS：菜单触发器为方形 Ink Surface；菜单是 1px Ink 边界、白色 Canvas、`--lgi-shadow-brutal` 单层硬投影；没有圆角蓝色系统菜单、渐变或 Pill。
- 中文主语义：当前领域、样本数、列表级边界和不可用原因均以中文传达；英文只作紧邻技术注释。
- 键盘/结构：领域选择使用原生 `details/summary` + 可聚焦链接；辅助技术读取到“切换当前观察领域”“考研自习 21 条样本”及三个导航链接。行仍保留 option 选择与 Inspector 更新。
- 数据边界：Inspector 的“详情、正文、评论、媒体尚未读取”是直接可见文本，不由 tooltip 或 toast 独占。

## 4. 分层结论

| 完成层 | 结论 | 证据 | 仍有限制 |
|---|---|---|---|
| 设计规格一致 | VERIFIED | `PAGE-EVIDENCE-001` §5.4、变更清单与候选浏览器走查 | 领域自助配置未实现 |
| 前端/组件实现 | VERIFIED | `corpus_domain_picker`、`shell.css`、`evidence_library.js/css` 的候选渲染 | 未改变跨行业/证据 API 合同 |
| 自动检查 | VERIFIED | `node --check`、`cargo fmt --check`、3 个 focused Rust/DOM contract tests | 未运行全仓回归 |
| 真实读取链路 | VERIFIED（只读列表层） | `:3107` health database `READY`；真实本机外部样本接口返回 21 条 | 不证明采集、详情或材料链路 |
| 部署 | NOT VERIFIED | 候选仅运行在 :3107 | `origin/main`、共享 runtime-main 与 :3000 未变更 |
| Mog / 业务验收 | NOT VERIFIED | 等待 Mog 在集成后验收 | 本记录不替代用户业务判断 |

## 5. 后续与非结论

- 本次没有增加“向外看的领域”的配置入口。现有领域来自数据库 `observation_domain` 读取；新增、改名、暂停、排序、权限和采集关联需要作为 #130 的单独领域治理交付先冻结合同。
- 未把外部 `sampleRef` 写入 `?work=`，也没有调用 Work Resource detail 路由；因此不会把参照样本误写成“来源信息不完整”。
- 候选服务及浏览器只用于本次只读验证，完成后应关闭；不将候选端口当作部署或生产证明。
