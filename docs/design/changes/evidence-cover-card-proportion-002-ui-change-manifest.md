# EVIDENCE-COVER-CARD-PROPORTION-002 · 证据库封面卡比例校准

> 状态: 权威当前
> 最后核对: 2026-09-15
> 适用范围: `/corpus/evidence?layout=cover` 的既有作品卡视觉比例、分组间距与固定数据铭牌
> 事实来源: Mog 2026-09-15 的截图复核与明确推进授权、`PAGE-EVIDENCE-001`、`EVIDENCE-COVER-FLIP-CARD-001`、LIDS、现行 `evidence_library.js/css`
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实运行/代码/API 合同；本清单不扩大 Work Resource、媒体或状态语义

## 1. 事项与表面地图

- Work Package：`EVIDENCE-COVER-CARD-PROPORTION-002`；分支 `codex/corpus-cover-card-proportion-002`；专属 worktree `.worktrees/corpus-cover-card-proportion-002`。
- 用户可见结果：保留封面视图六列扫描密度，让每个作品成为一组有主从尺度的「可翻转档案主卡 + 低矮横向数据铭牌」；组内紧凑，换行后的作品组留出更大的垂直空白。
- 明确非目标：页面壳、筛选/排序/分页、Inspector、详情选择、Work Resource API、数据模型、媒体 URL 资格、completeness/observation 规则、采集、插件、数据库、全局 Token 与运行服务。

| 表面 | 责任 | 复用事实 | 禁止替代 |
|---|---|---|---|
| 封面主视觉 | 固定两行标题、同一 3:4 Stage、几何/真实封面翻转 | title、既有受控本地 cover、已有媒体形态 | 远程封面、AI 图像或新数据分类 |
| 数据铭牌 | 作者/日期、互动数、部分取得与最近观察 | author、publishAt、engagement、material summary、observedAt | 新的总分/进度、伪造 0、第二状态模型 |
| 作品组间距 | 区分同一作品的两块与下一行作品组 | CSS layout only | 新分页、虚拟化或排序语义 |
| 作品行 | 继续选择当前 Work 并打开既有 Inspector | 既有 row selection handler | 平行详情路由或点击链路 |

## 2. 读取回执与方向锁

| 来源 | 状态 | 本次解决的问题 | 已核对 |
|---|---|---|---|
| AGENTS.md / docs README / current-state | 权威当前 | UI 交付、独立 worktree、变更前文档与审查边界 | 是 |
| UI execution contract | 权威当前 | 展示+局部交互的表面、状态、依赖与验收矩阵 | 是 |
| `PAGE-EVIDENCE-001` / 当前 Work Row | 权威当前 / 真实代码 | 保留同一读取集、选择和 Inspector，不能另建卡片体系 | 是 |
| LIDS Token / Materials / Data Boundaries / Language | 权威当前 | token-only、3:4、真实数据四态、中文主表达与无渐变/玻璃 | 是 |
| `EVIDENCE-COVER-FLIP-CARD-001` 与用户 V5 原型 | 已合并实现 / 用户视觉目标 | 保留翻转合同；修正主卡/铭牌的尺度关系而非减少列数 | 是 |

- **Visual thesis**：温暖米白的技术目录以高竖向档案页承载作品，低矮横向铭牌承载证据读数；同组紧密，作品组之间留白，Signal 橙红只标记当前对象。
- **Content plan**：固定标题区 → 共享 3:4 Stage → 中文类型/封面提示；下方铭牌依次为作者与日期 → 四项互动 → 取得状态与最近观察。
- **Interaction thesis**：主视觉保持现有 hover/tap/键盘翻面；数据铭牌不翻面，信息区点击继续触发现有作品选择。
- CSS 策略：仅扩展现有 `apps/api/src/local_web/evidence_library.css`，只引用既有 `--lgi-*` token。
- 几何策略：视频、多图、已知评论材料仍优先映射 wave、stack、cluster；当当前投影只诚实地证明为「单显式封面」而无法再区分媒体类型时，使用既有稳定 Work Reference 在 grid / cone / frame 三种通用图形间选择。它不写回数据、不生成内容、不表示新的内容或质量分类，也不会随排序或翻页随机变化。
- 铭牌文字策略：顶部已知发布日期仍显示完整 `YYYY-MM-DD`，以受限日期轨道为作者保留剩余宽度；只有 `SOURCE_TEXT_ONLY` 显示「来源时间」，完整来源文字仍在元素 `title` / Inspector。底部只显示 `MM-DD HH:mm` 观察时刻，元素 `title` 保留「最近观察 + 完整时刻」。日期、作者、状态与观察时间均必须可收缩，不能越出切角外框或挤压成重叠文本；跨行业样本将其边界说明收束为底带第一格，永远不生成隐式第二行。
- 封面铭牌状态策略：卡面只显示中文状态词（例如「部分取得」）；`PARTIAL` 等技术枚举继续留在已有 tooltip 与 Inspector，不占用第三条横带的阅读空间。

## 3. 分类、状态与边界

- 分类：展示 + 既有局部交互的视觉校准；最高风险为展示。
- L1 / Pattern：L1 Corpus Explorer 的连续工作面；本变更只重排现有 cover layout 内的 Card Group，不新增 CMP、Scene 或全局 Motion。
- 状态：保留 `PARTIAL` / `UNKNOWN` 等既有状态词与 tooltip 来源；从卡面撤去多色 material rail，不改变 rail 计算、tooltip、Inspector 或状态含义。
- 选中态：已有 `aria-selected` 的当前对象改为整组 Signal 色面；这只是同一选择状态的视觉表达，不是风险、完成或数据质量。
- DECISION_REQUIRED：无。用户已明确六列正确、需要低矮数据铭牌、组间留白与目标原型一致的尺度关系。

## 4. 尺度合同与验收矩阵

| 合同 | 实现约束 | 验收方法 |
|---|---|---|
| Stage | 正反面的单一 Stage 继续 `aspect-ratio: 3/4`，两行标题不改变 Stage 起点 | 文本合同 + 不同标题长度的浏览器核对 |
| 主从比例 | 六列下数据铭牌是低横向面；最小高度 112px，内部固定为作者/日期、四项读数、状态/观察三条横带；主视觉:铭牌高度接近 5:1 | CSS contract + 2048px 实测截图/DOM 尺寸 |
| 成组节奏 | 主视觉→铭牌使用 `space-3`；每个作品组→下一行使用至少 `space-6` | Grid row/column gap 走读 + 浏览器核对 |
| 真实读数 | 作者、日期、互动、取得状态、最近观察仍来自既有投影；0/未知不互换 | 既有 runtime tests + DOM walk |
| 色彩纪律 | 默认米白/墨线；整组选择为 Signal；卡面不再显示多色大进度轨 | CSS/浏览器核对 |

停止条件：需要改变 `completeness` 计算、取数/API、媒体资格、行选择契约、全局 token，或需要新增状态/字段时立即停止并报告 Mog。

## 5. 验证与交接

- 修改文件：`evidence_library.css`、必要时 `evidence_library.js` 与局部文本合同测试；本清单、验收记录、设计索引和当月 progress。
- 计划验证：focused cover contract、`evidence_runtime`、`node --check`、`git diff --check`、设计/治理检查、1280/1440/1920/390px 浏览器核对、键盘/hover/reduced-motion。
- 发布回执（2026-09-15）：Mog 已明确授权提交、推送、合并和刷新。本包提交 `a4e7f7a` 已推送 `origin/codex/corpus-cover-card-proportion-002`；`main` 合并提交 `36ee8bb` 已推送 `origin/main`；受控 `scripts/runtime/install.sh` 已将本机 `runtime-main` 同步至该 revision。无 migration、采集、模型、平台或数据库写入。
