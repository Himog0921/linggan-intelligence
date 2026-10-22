# ACC-COMMENT-WORD-CLOUD-V23 · 验收记录

> 状态: 执行中 / 结果以精确 HEAD CI 为准
> 最后核对: 2026-10-22
> 适用范围: 紧凑评论概览、候选短语、来源回查与只读 API
> 事实来源: 候选分支和真实 GitHub Action 回执
> 冲突时以谁为准: CI、浏览器实际测量、真实 SQL 与用户复审

## 已编写的验证入口

- Vitest：DomainWordCloud 的真实 DTO、时间切换、词频分类、空数据和错误状态、点击词语到评论详情；ObservationSeries 的独立尺度和覆盖语义。
- Rust：word_cloud 的逐评论去重、只在精确种子分类、噪声排除；隔离 PostgreSQL 包含 reader/creator 区分、跨作品支持数与稳定源引用。
- 检查：npm --prefix apps/web run verify；npm --prefix apps/web run lids:verify；cargo test -p linggan-intelligence -p linggan-api --locked；浏览器隔离复现。
- 真实敏感评论正文、生产数据与灰度部署没有进行，也不能由合成测试代替。

## 待 CI/独立复核的判据

| 问题 | 必须观察到的结果 |
|---|---|
| 默认态 | 56 天图表，黑柱与红曲线位于同一横轴上方，面积由曲线下渐变至基线 |
| 视觉密度 | 1440、1791、1920 下无不规则换行/页面横向溢出，桌面主模块接近用户红框；390 自然滚动不截词 |
| 日期切换 | 图表与词云同窗口发起独立只读查询；响应换领域不污染旧数据 |
| 统计范围 | domain 和来源资格一致；评论重复词只计一次，跨作品统计正确 |
| 失败和上限 | 不返回合成数据；>20,000 样本边界如实提示 |
| 原声回查 | 只返回稳定 CommentKey，原声通过既有受限详情读取；不泄漏失效来源 |
| 回归 | 四视图、FIND、选择模式、Run 冻结方法、设置、证据库不受本 PR 破坏 |

## 未完成

尚未记录成功的 exact-head CI、可核实的桌面截图、私有 PostgreSQL 和完整用户视觉验收；不能把实现提交等同发布。
