# EVIDENCE-COVER-CARD-MATERIAL-004 · 封面作品卡表面层级与材质

> 状态: 权威当前
> 最后核对: 2026-09-15
> 适用范围: `/corpus/evidence?layout=cover` 既有作品卡的表面分层、边框依赖、卡面材质、舞台测量场、选中态材质、标题字阶，以及结果区横条的删除
> 事实来源: Mog 2026-09-15 的规格、真机尺寸候选页与选型回复（候选 B）、「登记为第七种」的指示、`DEC-0005`、`EVIDENCE-COVER-FLIP-CARD-001`、`EVIDENCE-COVER-CARD-PROPORTION-002`、LIDS、现行 `evidence_library.js/css`
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实运行/代码/API 合同；本清单不扩大 Work Resource、媒体、状态或选择语义

## 1. 事项与表面地图

- Work Package：`EVIDENCE-COVER-CARD-MATERIAL-004`；分支 `codex/corpus-cover-card-material-004`；专属 worktree `.worktrees/corpus-cover-card-material-004`。
- 用户可见结果：一排仍是 6 张卡。三处表面不再靠黑框区分，而是靠底色层级区分（页面白 → 卡面 `#eef0f1` → 舞台纯白 → 铭牌 `#f7f8f8`）；卡面多一层极淡的干燥纸面颗粒（`LIDS-MAT-001` 新登记的 `M-06` 纸面残留）；舞台点阵真正铺满 3:4 区域并回到规格的 8px / 1px；选中对象仍是整卡 Signal 橙，但橙色面本身有材质；标题回到系统字阶而不是页面大标题字阶。
- 用户明确删除：「图片主导 · 视觉供给研究」所在的结果区横条整条移除（含「打开检查器」按钮）。
- 明确非目标：页面壳、搜索、Filter、排序、System Views、Saved Views、Inspector 的开关逻辑与宽度、Detail 抽屉、分页/无限滚动、Work Resource API、数据模型、媒体 URL 资格、completeness/observation 规则、采集、插件、数据库、全局 Token 与运行服务。

| 表面 | 责任 | 复用事实 | 禁止替代 |
|---|---|---|---|
| 卡片表面（L1） | 让作品成为一张独立的印刷档案卡，可从页面底色中分离 | `--lgi-canvas-sunken`、`--lgi-border-strong` | 新底色、新阴影、玻璃或渐变 |
| 卡面材质 | 给「印刷卡」以纸面密度，不参与表达任何状态 | `LIDS-MAT-001` 新登记的 `M-06` 纸面残留（同一点阵的第七种状态，见 `DEC-0005`）；一个容器至多一种材料 | 方格纹理、噪点图、状态或质量暗示 |
| 舞台测量场（L2） | 3:4 主视觉区作为测量面，正反面共用同一物理footprint | 既有 `aspect-ratio:3/4`、既有几何模板 | 改变 3:4、改变正反面尺寸或位置 |
| 数据铭牌（L3） | 作者/日期、互动数、部分取得与最近观察 | 既有 author、publishAt、engagement、material summary、observedAt | 新的总分/进度、伪造 0、第二状态模型 |
| 选中态 | 当前对象整卡 Signal 橙 | 既有 `aria-selected` | 风险、完成或质量等级 |
| 标题字阶 | 卡片标题使用卡片字阶，不再借用页面大标题 | `--lgi-text-longform`、`--lgi-lh-title`、`--lgi-weight-bold` | 逐张不同的字号 |

## 2. 读取回执与方向锁

| 来源 | 状态 | 本次解决的问题 | 已核对 |
|---|---|---|---|
| AGENTS.md / CLAUDE.md / docs README | 权威当前 | UI 交付、专属 worktree、变更前文档与提交前独立审核 | 是 |
| `docs/agents/ui-execution-contract.md` / `docs/design/README.md` / `docs/design/lids/README.md` | 权威当前 | 展示+局部交互的表面、状态、依赖与验收矩阵 | 是 |
| LIDS Token / Materials / Data Boundaries | 权威当前 | token-only、8px 采样栅格、一个容器至多一种材质、正文不压纹理 | 是 |
| `EVIDENCE-COVER-FLIP-CARD-001` / `-PROPORTION-002` / `-OVERFLOW-003` | 已合并实现 | 保留翻转合同、六列密度、低横向铭牌与宽度约束 | 是 |
| Mog 规格与真机尺寸候选页 | 用户视觉目标 | 四级表面层级、减弱黑线依赖、卡面材质、整卡橙色带材质 | 是 |

- **Visual thesis**：温暖但仍在系统内的四级灰白台面，卡片像一张有纸面的档案卡；黑线退到只保留证据主视觉的一条 1px 描边，其余关系由底色深浅与留白承担；Signal 橙只标记当前对象，而橙色面本身也是印刷面而非色块。
- **Content plan**：与 002 相同，不新增字段：固定两行标题区 → 共享 3:4 Stage → 类型/封面提示；下方铭牌依次为作者与日期 → 四项互动 → 取得状态与最近观察。
- **Interaction thesis**：主视觉保持既有 hover/tap/键盘翻面；铭牌不翻面；选择继续走既有 row selection handler。结果区横条移除后，检查器仍由选中作品打开，键盘与抽屉布局不受影响。
- CSS/JS 策略：只修改 `evidence_library.css`、`evidence_library.js` 的既有 cover component 路径，以及 `local_web.rs` 中该横条的 HTML。
- 几何策略：六个既有模板（grid / cone / frame / wave / stack / cluster）全部重画在同一个安全区内 —— viewBox `0 0 300 400` 中 x 48..252（宽 68%）、y 80..320（高 60%），几何容器回到 100%×100%。模板只是目录记号，不推断主题、状态或质量。

## 3. 分类、状态与边界

- 分类：展示 + 既有局部交互的视觉校准；最高风险为展示。
- L1 / Pattern：L1 Corpus Explorer 的连续工作面；不新增 CMP、Scene 或全局 Motion。
- 状态：`PARTIAL` / `UNKNOWN` / `SOURCE_TEXT_ONLY` 等既有状态词、tooltip 与 Inspector 内容完全不变。
- 材质纪律：一个容器至多一种材质。卡面颗粒只挂在 `.ev-cover-face`，且被 `.ev-cover-face-inner` 的内容层覆盖，因此标题、正文、数据永不被纹理压住；舞台点阵只挂在 `.ev-cover-stage-frame--diagram`。两者图案不同（不排成线的 24px 残留 vs 均匀的 8px 点阵），不会读成同一种图案。
- 取值纪律（如实列出写死值）：本包新增的值里，除既有 `--lgi-*` token 外只有两组，都是材质密度参数——卡面颗粒（点半径 `.6px`、周期 `24px`，常态取格点 `(0,0)`、`(0,8)`、`(8,16)`，选中态在同一格里多留一个 `(8,8)`，α 分别为 `.15/.11/.13` 与 `.20/.14/.20/.14`）与舞台点阵（半径 `1px`、周期 `8px`，来自用户规格「8px 间距、1px 点」）。颗粒墨色用与 `--lgi-ink` 完全相同的 RGB（17,19,21），只变透明度；α 是密度参数不是新颜色，选中态橙面颗粒同理。LIDS 的「不得写死间距值」按其字面在这两组上不成立：它们是材质自身的参数，不是布局间距。
- LIDS-MAT-001 关系（已登记为第七种）：卡面颗粒按 Mog 2026-09-15「登记为第七种」的指示进入材料家族，编号 `M-06` 纸面残留，长期依据是 `docs/decisions/0005-lids-material-06-paper-residue.md`，规则落在 `docs/design/lids/materials.md` §3。登记的前提是格点归属，因此候选稿的 20px 周期、偏移 13/6 改为 24px 周期的全格点取法：每一个点与重复周期都是 8px 的整数倍。格点归属不足以单独守住它 —— 第一版取的 `(0,0)`、`(8,16)`、`(16,8)` 全在格点上，却构成点阵的一个子群，场在 45° 方向排成每 11.31px 一个点的连续排线（加密态的 16px 格同样如此）；所以判据 2「排不成等距点列」（场中不存在三个等距共线的点、间距小于 17px）与判据 1 并列。三条判据（格点归属、不排成等距点列、密度封顶）都进测试合同。常态取 9 个格点中的 3 个，加密态在同一格里多留一个 `(8,8)`：状态是密度，不是第二种图案或第二种颜色。**同面积点数因此比候选稿少约三成**（原来 3 点/20px 格、现在 3 点/24px 格），纸面比 Mog 看过的候选稿再淡一档，α 与点半径不变。它的允许区位只有有 1px 边界的独立卡板，连续阅读区背后禁用；`M-00`…`M-05` 的规则与运行时位置不变，全局 token 未动。
- 删除项：`.ev-results-head` 横条、`.ev-results-legend`、`.ev-inspector-reopen`（含其 CSS、focus ring 条目与 1180px 抽屉断点条目）、结果区图例的文本写入。读取时间戳不再占据一行，改为挂在作品列表容器上，保持可核对。
- DECISION_REQUIRED：结果区读取时间戳改挂列表容器后，悬停结果区任意位置都会浮出它（浏览器对没有自己 `title` 的后代取最近祖先的），是否保留这个可核对入口。用户已确认改动范围（含标题 CSS 一并修正）；原先并列的「卡面颗粒是否进入材料家族」已由 Mog 2026-09-15 的指示结案，改为登记（见上条与 `DEC-0005`）。

## 4. 尺度合同与验收矩阵

| 合同 | 实现约束 | 验收方法 |
|---|---|---|
| 六列 | 继续使用 `repeat(auto-fill,minmax(248px,1fr))`，不通过减少列数回避问题 | CSS 合同 + 1920px 实测截图/DOM 尺寸 |
| 表面分层 | 页面白 / 卡面 `--lgi-canvas-sunken` / 舞台 `--lgi-canvas-hi` / 铭牌 `--lgi-canvas-low`，四层全部为既有 token | 计算样式取值 + 截图 |
| 边框减负 | 主视觉 1px `--lgi-border-strong`；舞台 1px `--lgi-border-strong`；铭牌无整圈黑框，只有顶部关系线与底部 hairline | CSS 合同 |
| Stage | 正反面单一 Stage 继续 `aspect-ratio:3/4`，两行标题不改变 Stage 起点；舞台尺寸六张完全一致 | 文本合同 + 不同标题长度的浏览器核对 |
| 标题区 | 固定两行格，高度 = `--lgi-text-longform × --lgi-lh-title × 2`，一行与两行标题的 Stage 顶边完全一致 | CSS 合同 + DOM 尺寸 |
| 标题字阶 | 卡片单一字阶 `--lgi-text-longform`，不再按行数或 `data-two-line` 变化 | CSS 合同 + 逐张 DOM 取值 |
| 测量场 | 舞台点阵以 CSS 铺满整个 3:4 区域（8px 间距、1px 点），不再出现底部空白 | DOM 尺寸 + 截图 |
| 材料格点 | 卡面颗粒的每一个点与重复周期都是 8px 的整数倍（`M-06` 与其它六种同属一套点阵），场中不存在三个**等距**共线的点、间距小于 17px（因此不读作方格或斜纹），密度封顶 `.20` / `.6px` / 墨色 `(17,19,21)`，且与舞台点阵不是同一图案 | 三条性质断言 + 计算样式取值 |
| 几何权重 | 六个模板全部落在同一安全区内（宽 68% / 高 60%） | JS 合同 + 360px 宽度下逐张实测包围盒 |
| 真实读数 | 作者、日期、互动、取得状态、最近观察仍来自既有投影；0/未知不互换 | 既有 runtime tests + DOM walk |
| 翻面 | 保持 `perspective:1400px`、`preserve-3d`、`backface-visibility:hidden` 与既有 duration/ease；不产生布局位移 | 翻转前后 stage/meta/row 尺寸差为 0 |

停止条件：需要改变 `completeness` 计算、取数/API、媒体资格、行选择契约、全局 token，或需要新增状态/字段时立即停止并报告 Mog。

**标题字阶为什么取 `--lgi-text-longform`**：参考稿 V6 的卡片标题是 `.head h3{font-size:17px;line-height:1.08;font-weight:700;letter-spacing:-.025em}`，并在 `max-width:1450px` 时降到 16px。17px 不在 token 表内，而 277px 宽、六列并排正是 V6 自己定义的窄卡情形，因此取它已降级后的那一档：`--lgi-text-longform`（16px）＋`--lgi-lh-title`（1.1）＋`--lgi-weight-bold`（700）＋`--lgi-ls-tight`。原实现把页面大标题字阶 `--lgi-text-page-title` 搬进了卡片（一行 28px，两行再 ×0.86 = 24.08px），且因 `font` 简写缺字号而丢掉字重，这正是用户判断「太大、完全没有美感」的地方：243px 内容宽在 24px 下一行只放得下约 10 个字，绝大多数标题被截断，标题的视觉权重也压过了 3:4 主视觉。字阶变化只影响卡高（−25.7px），舞台尺寸不受影响 —— 两面的标题格高度相同，在共享 footprint 里相互抵消。

## 5. 顺带修正的既有缺陷

1. **舞台点阵没有铺满**：`coverDotField()` 输出的 SVG 只有 `viewBox` 没有尺寸，作为替换元素按 1:1 固有比例取高，实际只覆盖 3:4 区域的上方 `237.6 × 237.6`，底部约 80px 没有点阵；同时等效间距被拉到约 18.85px、点半径约 1.77px，与规格的 8px / 1px 不符。修正为 CSS 点阵。
2. **六处 `font` 简写缺少字号**：`font:var(--lgi-weight-X)/var(--lgi-lh-Y) var(--lgi-font-Z)` 缺字号是无效声明，浏览器整条丢弃，导致字重与行高回落到继承值。受影响：`.ev-cover-title`、`.ev-cover-back-label/.ev-cover-index/.ev-cover-micro-label/.ev-cover-flip-cue`、`.ev-cover-unavailable`、`.ev-cover-date/.ev-cover-observed`、`.ev-cover-cross-boundary`、`.ev-cover-status-tooltip`。修正后标题按设计值加粗，其余小字从回落的 400 回到设计值 600。
3. **逐张检测两行标题的脚本**：`syncCoverTitleSizes()` 在每次渲染与每次窗口 resize 时对每张标题做 `getComputedStyle` + `scrollHeight` 强制重排。标题改为单一字阶后该机制不再需要，随 `data-two-line` 一起移除。
4. **背面原始封面没有真正填满 3:4 舞台**：`.ev-cover-stage-frame--cover img` 只有 `width:100%;height:100%;object-fit:cover`。舞台盒的高度由共享 sizer 推出，这个百分比高度因此解析不稳定：部署中的构建上，多数封面恰好填满（六列 47/50、五列 49/50），但比例偏离 3:4 较多的封面按自身固有比例取高、挂在舞台下方——640×1138 的图片盒在五列下溢出 93px（舞台内容盒 281.6）、六列下溢出 104px（舞台内容盒 314.8），640×1422 溢出 209px；`object-fit` 无从生效，只显示图片上方约 75%。修正为脱离文档流（`position:absolute;inset:0`，与本文件 `.ev-preview img` 同一写法），正面几何与背面封面现在占据完全相同的 3:4 盒，`object-fit` 从中心裁切。修正后在新构建上逐张实测 50 张封面（含 640×1138 / 640×853 / 640×640 / 640×480 / 640×400 等比例）与舞台内容盒的宽高差全部为 0。**勘误**：本卡早期草稿曾写「640×480 / 640×400 的封面会矮于舞台、留出约 140px 空带」，那是由机制推出的预判而非实测，五次测量都没有复现（矮封面同样是 0 差），已删除该断言。

## 6. 验证与交接

- 修改文件：`evidence_library.css`、`evidence_library.js`、`local_web.rs`（仅删除横条 HTML）、`tests.rs`（封面卡合同）；设计系统侧 `docs/design/lids/materials.md`（`M-06` 行与专章、§1、验收清单 8）、`docs/design/lids/README.md`、`docs/design/lids/migration-log.md`、`docs/decisions/0005-lids-material-06-paper-residue.md`（新增）；本清单、验收记录、设计索引和当月 progress。
- 已完成的验证（2026-09-15，1920×929 视口、检查器关闭、共 50 张卡）：
  - `cargo test -p linggan-api local_web::tests::evidence` → 12 passed；其中封面卡合同与「结果区横条保持删除」两项为本包新增/改写。
  - 断言回填验证：六个模板的每条坐标字面量都进了安全区合同，把 stack 的矩形从 `160×170` 改成 `260×260`（越出 x 48..252 / y 80..320）→ 只有该合同变红；把 `.ev-results-head` 的样式规则加回 CSS → 只有横条合同变红；把 `M-06` 的格点从 `(8,16)` 改成 `(13,16)`（离开 8px 点阵）→ 只有格点归属断言变红；把 `M-06` 常态改回子群取法、或让三个点等距排成一行、或让选中态在同一 24px 格上排成一行 → 只有「排不成等距点列」断言变红，报出 `draws three dots on one line …px apart`；把 α 从 `.15` 提到 `.6`、把点半径从 `.6px` 提到 `4px`、或把墨色换成 `rgba(255,0,0,…)` → 只有密度封顶断言变红。CSS 断言覆盖的是独立 asset，所以横条合同同时负向断言 HTML 与 CSS 两侧。
  - `M-06` 的实测：`cargo test` 12 passed 后，在真实页面上逐张读取计算样式 —— 常态卡面 `24px 24px` × 3 层（`(0,0)`/`(0,8)`/`(8,16)`，α `.15/.11/.13`），选中卡面 `24px 24px` × 4 层（`(0,0)`/`(0,8)`/`(8,8)`/`(8,16)`，α `.20/.14/.20/.14`），舞台点阵仍为 `8px 8px`，内容层 `z-index:2` 压在颗粒 `z-index:1` 之上；同一视口下六列、卡宽 277.33px、卡高 544.64px、舞台 240.03×320.05、背面封面盒与舞台内容盒差 0 —— 材质变化没有动任何几何。
  - 六列：列表宽 1704px，卡片宽 277.33px；前 18 张卡高完全一致（544.64px），含一张标题与两种超短标题。
  - 舞台：六张正面舞台全部 `240.02|240.03 × 320.04|320.05`（仅亚像素取整差），顶边全部 304.2px；标题格全部 `214.13 × 35.2`。
  - 翻面：真实 hover 触发 `matrix3d(-1,…)`；`perspective:1400px`、`preserve-3d`、`backface-visibility:hidden`；正反面舞台盒位移差 0/0/0/0。
  - 封面填充：新构建逐张实测 50 张封面，与舞台内容盒的宽高差全部为 0（含改动前溢出的 640×1138）。
  - 响应式：1280px → 4 列、舞台仍为精确 3:4、标题仍 16px；390px → 1 列、无横向滚动、舞台 352.7×470.27（3:4）。
  - reduced-motion：`.ev-main *` 与 `shell.css .v7-app *` 两道 `prefers-reduced-motion:reduce` 规则均覆盖卡面；键盘 Enter/Space 走既有 `data-flipped` 切换。
- 已知偏差（需 Mog 决定是否改）：翻面的时长与缓动仍取系统 token（`--lgi-duration-panel + --lgi-duration-state` = 400ms，`--lgi-ease-ui` = `cubic-bezier(.2,.75,.2,1)`），与规格里写的 `.45s cubic-bezier(.2,.72,.18,1)` 相差 50ms 与一条几乎同形的曲线；本包按「不改动既有翻转」执行，未把规格中的字面值写死进 CSS。
- 未完成：远端生产、真实触摸设备、Mog 前端验收。
