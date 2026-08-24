# LIDS-LOG-001 · LIDS 迁移与变更记录

> 状态: 权威当前
> 最后核对: 2026-08-21
> 适用范围: Linggan Intelligence LIDS Token、Primitive、Component、Pattern、Page、Motion、Scene 和 Data Truth 规则的实际变更、替代、例外与验证边界
> 事实来源: [system.md](system.md)、[README.md](README.md)、DESIGN-002 Issue #7、项目 progress 记录和实际验证输出
> 冲突时以谁为准: 真实代码/合同/测试、用户最新确认、当前 SCOPE 和 ACCEPTED 决策；本日志不把计划写成已实现事实

任何影响 LIDS 五层或横向约束的事项必须在同一 PR 更新本记录：变更是什么、取代什么、影响页面/组件、验证结果和仍未证明什么。日志不是路线图，更不是运行时真相。

## 2026-08-21 · LIDS v2.0 导入为 Linggan 的权威设计表达标准

- **来源**：Mog 明确指定 `/Users/moglenny/Downloads/Linggan_Intelligence_Design_System_v2.0`；主源文件校验值与不继承清单见 [README.md](README.md)。
- **吸收**：`Token → Primitive → Component → Pattern → Page`、L1/L2/L3、暖灰/煤黑/Signal 的语义、Sans/Mono 分工、状态五轴、`PARTIAL + VALID`、动效/场景/响应式/a11y 边界、Agent 决策树、规格门与变更纪律。
- **项目适配**：把来源包的运行时代码、目标目录、React/Three/Blender 路线、V3 模拟数据/状态和静态原型降级为未来候选；不创建 Web 应用、主题 CSS、组件、真实数据或部署。
- **替代**：DESIGN-002 原有 DS-001–DS-007 的局部“暗色 Acid/新粗野主义”视觉值被 LIDS Token 与 L2 Pattern 取代；原有 Evidence/Boundary 和组件晋升的事实边界仍保留，并与 LIDS Data Truth 对齐。
- **影响**：全项目未来 UI Agent；DESIGN-002 Topic 静态参考页、其 PAGE 规格、执行合同、设计治理、模板、索引和检查脚本。
- **验证目标**：手册链接/状态头、LIDS 检查、Topic 专项检查、项目治理检查、静态浏览器走查与独立审查。
- **未证明**：LIDS 仍为 `PROPOSED`；没有真实 L1/L2/L3 页面、运行时 Token、组件、真实数据/Agent 状态、3D 资产、性能、部署或 Mog 最终视觉验收。

## 2026-08-24 · LOCAL-001A 首个运行时 L1 Evidence Library token 映射

- **来源与事项**：Mog 确认的 LOCAL-001、`REF-V7-001` 页面 Gold Master、Issue #25、`PAGE-EVIDENCE-001`。
- **实际实现**：`apps/api/src/local_web/evidence_library.css` 是当前唯一运行时页面样式文件。它只声明本页实际使用的 `--lgi-*` token，值逐项映射自 `LIDS-TOK-001`；没有引入第二个全局主题、组件库或视觉前缀。
- **页面组合**：本页采用 `LIDS-PAT-001` 的 L1 `Corpus Explorer`，嵌入右侧受限 `Split Evidence Inspector`。原声、材料、搜索、动作和真实状态均未实现；首屏唯一视觉核心是来源不足的材料边界说明。
- **局部例外**：`LOCAL-001-UI-EX-01` 仅为 V7 三栏比例保留 216px 左 rail、440px right inspector 与 2px 中央结构线。例外在 PAGE/Manifest 中可查询，未推广为 token 或跨页组件；以后第二页面复用前必须重新审查。
- **Data Truth**：页面只表达 `SOURCE_INCOMPLETE`、`NOT_CONNECTED`、`UNKNOWN` 与“本页没有可展示的已接纳材料”。这些不表示系统库为 0、平台不存在内容、捕获失败或任何趋势；没有模拟数值、`LIVE`/`FRESH`、假按钮或前端回执。
- **验证目标**：Rust route test、loopback HTTP、指定视口浏览器走查、CSS token/a11y 检查和治理检查。实际结果与未证明边界记录在 `ACC-EVIDENCE-001`。
- **未证明**：LIDS 整体仍为 `PROPOSED`；本项不证明 Materials read model、真实 Evidence/Observation/Capture、数据库、插件/真实平台、媒体、OCR/ASR、跨页组件、部署或 Mog 验收。
