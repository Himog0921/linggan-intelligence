# DESIGN-010 · LIDS 升级到 v7.0 的 UI 变更清单

> 状态: 权威当前
> 最后核对: 2026-09-02
> 适用范围: LIDS 表达标准从 v2.0 升级到 v7.0 的治理文档变更；不含任何运行时代码改动
> 事实来源: Mog 于 2026-09-02 指定的 `linggan-design-system-v7.html`（SHA-256 `1093462bcea81c10584e18119a92ae6b51645d1b480667e161d993e863ca4e36`）与同日的范围裁定「只改治理文档 + 记录迁移欠账」
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实运行/代码/合同、ACCEPTED 决策与当前 SCOPE

## 1. 变更分类

| 项 | 判定 |
|---|---|
| 变更类型 | **展示**（规则层）。不触及交互、状态语义或权限/行动 |
| 影响级 | 最高——它改变全项目未来 UI 工作的选值依据与验收标准 |
| 运行时影响 | **零**。本次不修改任何 `.rs`、`.css`、`.js` 或测试 |
| 停止条件 | 若发现某条 v7 规则与已确认数据合同或产品事实冲突，停止该条并记 `DECISION_REQUIRED` |

## 2. 范围

### 做了什么

**新增四份规则文件：**

| 文件 | 承担 |
|---|---|
| `lids/materials.md` | `LIDS-MAT-001` 8px 采样点阵六态、材料预算 70/20/10、区位限制 |
| `lids/shell-zones.md` | `LIDS-SHELL-001` 页头 3 区 + Context Bar 4 区的容量上限与静默区保护 |
| `lids/data-boundaries.md` | `LIDS-BOUND-001` 超长/缺失/零值与极大值/未知四态渲染契约 |
| `lids/decisions.md` | `LIDS-ADR-001` 11 条 v7 ADR + 2 条项目 ADR，规则状态与运行时状态分列 |

**修订八份既有文件：**

| 文件 | 主要修订 |
|---|---|
| `lids/tokens.md` | 重构为两层：§2 v7 三层架构（目标态）+ §3 `--lgi-*` 运行时基线（原样保留）+ §4 差异表与分步迁移顺序 |
| `lids/system.md` | 标题升 v7.0；材料预算；密度 T0–T3；页面模式；字重 3 档、字号下限 11px、线宽四档、边框禁 1.5px；信号色双角色；动效四档与量化位移 |
| `lids/primitives.md` | 排版表重建（含 Serif 原声角色）；线宽由两级改四档并说明理由；按钮 7 变体；图标系统；文字 Tab 取代分段控件；全局焦点环；三种空态；证据标本卡 |
| `lids/patterns.md` | 页面模式四种；壳层区位指向 `shell-zones.md`；Pattern 验收补四条 |
| `lids/language-policy.md` | **新增 LANG-05 Mono 预算**（三类允许、描述性标签禁止）；修正 LANG-04 表中两条被收窄的样例；验收补两条；第 6 节登记文案迁移欠账 |
| `lids/README.md` | 升 v7.0；v7 来源回执与不继承清单；文件地图补五行；新增「v7.0 的落地边界」 |
| `lids/agent-execution-guide.md` | 决策树补材料/文字/切换/信号色四支；交付前检查补七条 |
| `design/README.md`、`design-governance.md`、`reference-register.md` | 权威地图、读取路径、LIDS 治理地位、`REF-DS-V7-001` 来源登记与两个 "V7" 的命名冲突说明 |

**同步更新：** `docs/README.md` 索引、`scripts/verify-ui-design-handbook.sh` 门禁、`lids/migration-log.md`。

### 明确非目标

本次**不做**下列任何一项，它们各自需要独立受控事项与 Mog 的单独授权：

- 不修改 `apps/api/src/local_web/lids_tokens.css` 的任何 token 值；127 项 `--lgi-*` 与 Rust 逐项校验保持不变；
- 不修改 `shell.css`、`evidence_library.css`、`collection_workspace.css`；
- 不修改 `shell.rs`、`evidence_page.rs` 或任何页面模板的文案；
- 不删除或改写现有的 `v7-tech-key` 中英对照；
- 不引入密度机制、材料实现、8px 点阵资产或深色主题；
- 不改动路由、权限、状态判定、数据合同或任何 API；
- 不新增页面、组件、CMP 或 PAGE 规格。

## 3. 逐条差异与欠账

完整差异表在 [`../lids/tokens.md`](../lids/tokens.md) 第 4 节。摘要：

| 项 | 运行时现状 | v7 目标 | 对应 ADR |
|---|---|---|---|
| 主信号色 | `#ef4f25` 单值双用 | `#c63214` 承字 / `#f24a23` 仅色块 | `ADR-04` |
| 字重 | 含 500 | 仅 400/600/700 | `ADR-02` |
| 字号下限 | 9px | 11px | `ADR-03` |
| 线宽 | 1/2px 两级 | 1/2/3/5 四档，禁 1.5px | — |
| 圆角 | 0/2/4/8 | 0/4/8/pill | — |
| 材料 | 双层点阵 | 8px 采样点阵六态 | `ADR-08` `ADR-09` |
| 密度 | 无 | T0–T3 驱动信息量 | `ADR-06` |
| 壳层 | 有已知违规 | 3 区 + 4 区冻结 | `ADR-10` |
| 视图切换 | — | 文字 Tab | `ADR-11` |
| 界面英文 | 满屏中英对照 | 收窄到 Mono 三类 | `ADR-P01` |

已知运行时违规位置：`shell.rs`（导航中英并列、`LOCAL MATERIAL PROJECTION`、时区全称与代号并存）、`evidence_page.rs`（`ACCEPTED DISCOVERY`、`LOCAL READ ONLY`、`NO MATERIAL READ`、`LOCAL HOST` 等描述性标签）。两处各需独立事项。

## 4. 冲突处理

| 冲突 | 处置 |
|---|---|
| v7 收窄英文预算 vs `LIDS-LANG-001` 原口径 | 不推翻原规则。原则相同（中文独立承载含义），v7 是执行口径收紧，记为 `ADR-P01`，写入 LANG-05 |
| v7 三层 token vs 运行时 `--lgi-*` 与 Rust 校验 | 两层并存，明确标注各自适用面，记为 `ADR-P02`。§3 镜像与运行时源逐字节保持一致，校验未受影响 |
| v7 线宽四档 vs DESIGN-003「全站只有两级线宽」 | 采纳四档并在 `primitives.md` 就地说明理由（3px/5px 各绑唯一语义，不是放宽）；旧表述保留在同一节可查 |
| v7 `M-05` 低亮传感面 vs「不使用黑底终端」 | 维持为**局部**深色仪器面例外，绑定真实实时事件；`ADR-07` 不支持深色模式的结论不变 |
| 两个 "V7" 命名冲突 | 在 `reference-register.md` 与 `language-policy.md` 分别登记，不改类名 |

## 5. 验证

| 层 | 方法 | 结果 |
|---|---|---|
| Token 镜像一致性 | 逐项比对 `lids_tokens.css` 与 `tokens.md` §3 的 `--lgi-*` 名称与值 | 127 / 127，完全相等 |
| 手册结构门禁 | `scripts/verify-ui-design-handbook.sh` | 通过 |
| 文档索引 | 新增文件在 `docs/README.md` 与 `design/README.md` 权威地图中可达 | 已同步 |
| 状态头 | 四份新文件均含五项必填状态头字段 | 已核对 |

## 6. 本次不能证明什么

- 不证明任何页面在浏览器中的实际呈现发生了变化——**没有一行运行时代码被修改**；
- 不证明 v7 的材料、密度、壳层区位在真实页面上可行；这些只有在各自迁移事项中实测后才成立；
- 不证明现有页面符合 v7；恰恰相反，本清单登记了它们不符合；
- 不证明 v7 的对比度数值在本项目的实际字号/背景组合下全部达标——数值取自来源文件的实测标注，本次未复算；
- 不构成 Mog 对任何视觉结果的验收。LIDS 成熟度仍为 `PROPOSED`。

## 7. 回退

本次全部改动限于 `docs/` 与 `scripts/verify-ui-design-handbook.sh`。回退方式为撤销该提交；无数据迁移、无运行时状态、无外部副作用。
