# ACC-WORK-RESOURCE-READ-001 · 共享作品资源与三排版验收记录

> 状态: 一次性报告
> 最后核对: 2026-08-31
> 适用范围: Issue #110 / WORK-RESOURCE-READ-001 的共享作品资源读取、Evidence Library 三排版与自动验证
> 事实来源: 当前分支 Rust/SQL/HTML/CSS/JS、Browser Producer 源码、聚焦自动检查与隔离 PostgreSQL proof
> 冲突时以谁为准: 用户最新确认、当前代码/API 合同与真实运行证据；本报告不替代合并、部署、真实 XHS 探针或 Mog 业务验收
> Issue: #110
> 页面: `/corpus/evidence`

## 验收对象与条件

- PAGE / Pattern：`PAGE-EVIDENCE-001`；L1 Corpus Explorer + embedded L2 Split Evidence Inspector。
- 数据前提：隔离 PostgreSQL 16 合成 Package；不使用共享库、不访问真实 XHS、不保存真实正文/媒体。
- 关联合同：`WORK-RESOURCE-READ-001`、Media V2、Material Projection。
- 声明视口：桌面工作台、900px 断点、375/390px 窄屏；本轮没有实际浏览器截图，因此均为 `NOT VERIFIED`。

## 场景矩阵

| 场景 | 用户任务 | 预期事实 | 自动结果 | 视觉结果 |
|---|---|---|---|---|
| 精确详情时间 | 查看作品发布时间 | platform epoch 显示 `KNOWN`，带 field/kind/precision/parser | PostgreSQL proof 通过 | NOT VERIFIED |
| 相对时间文本 | 避免把“3小时前”当精确历史时间 | `publishedAt=null`、`SOURCE_TEXT_ONLY`、保留参照时间 | PostgreSQL 负向 proof 通过 | NOT VERIFIED |
| 创作者目标发现 | 看出内容从哪个监控目标来 | 显示目标；作品作者在没有 ID 时仍未知，关系 `NOT_VERIFIED` | collection dispatch proof 通过 | NOT VERIFIED |
| 三种排版 | 在研读、表格、封面间切换 | 同一 items、同一选择、同一 Inspector；不重新 fetch | JS/source test 通过 | NOT VERIFIED |
| 读取失败/受限 | 不制造空库或远程 fallback | 原有 inline 状态和受控媒体边界保留 | Rust/source test 通过 | NOT VERIFIED |

## 分层结论

| 完成层 | 结论 | 证据 | 限制 |
|---|---|---|---|
| 设计规格一致 | VERIFIED（source） | PAGE、Manifest、LIDS migration log、CSS/JS 静态检查 | 未做真实渲染 |
| 前端/组件实现 | VERIFIED（branch） | 三 layout selector、响应式 CSS、共享 API root | 未合并、未发布 |
| 自动检查 | VERIFIED | Node 语法、125 项插件测试、Rust compile/test、PostgreSQL proof | 自动检查不证明真实页面 |
| 真实链路/回执 | PARTIAL | Target/WorkOrder/Package 合成链；真实已有目标数据只做先前诊断 | 签名详情探针未获确认，未执行 |
| 部署 | NOT VERIFIED | 无 | 共享 migration/API/插件均未发布 |
| Mog / 业务验收 | NOT VERIFIED | 无 | 等待 PR、运行页与用户验收 |

## 不得据此推断

不得把本报告解释为真实 XHS 发布时间字段已核实、12 条历史笔记已回填、共享数据库已迁移、Chrome 已加载新插件、三种布局已由 Mog 接受或任务可合并。真实签名详情探针仍需要当次外部请求确认。
