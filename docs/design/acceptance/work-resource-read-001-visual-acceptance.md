# ACC-WORK-RESOURCE-READ-001 · 共享作品资源与三排版验收记录

> 状态: 一次性报告
> 最后核对: 2026-08-31
> 适用范围: Issue #110 / WORK-RESOURCE-READ-001 的共享作品资源读取、Evidence Library 三排版与自动验证
> 事实来源: 当前分支 Rust/SQL/HTML/CSS/JS、Browser Producer 源码、聚焦自动检查、隔离 PostgreSQL proof 与 127.0.0.1:3300 in-app Browser 实际检查
> 冲突时以谁为准: 用户最新确认、当前代码/API 合同与真实运行证据；本报告不替代合并、部署、真实 XHS 探针或 Mog 业务验收
> Issue: #110
> 页面: `/corpus/evidence`

## 验收对象与条件

- PAGE / Pattern：`PAGE-EVIDENCE-001`；L1 Corpus Explorer + embedded L2 Split Evidence Inspector。
- 数据前提：合同/状态 Oracle 使用隔离 PostgreSQL 16 合成 Package；本轮 UI 走查另以只读方式连接本机数据库，显示 13 个作品集合，不写入、不迁移、不访问真实 XHS。
- 关联合同：`WORK-RESOURCE-READ-001`、Media V2、Material Projection。
- 声明视口：1440×900 桌面与 375×812 窄屏已实际截图/几何检查；900/390px 未在本轮复拍。隔离实例没有接入当前运行快照的独立媒体根，个别图片采用诚实缺图替代，不据此判断媒体资格或生产呈现。

## 场景矩阵

| 场景 | 用户任务 | 预期事实 | 自动结果 | 视觉结果 |
|---|---|---|---|---|
| 精确详情时间 | 查看作品发布时间 | platform epoch 显示 `KNOWN`，带 field/kind/precision/parser | PostgreSQL proof 通过 | NOT VERIFIED |
| 相对时间文本 | 避免把“3小时前”当精确历史时间 | `publishedAt=null`、`SOURCE_TEXT_ONLY`、保留参照时间 | PostgreSQL 负向 proof 通过 | NOT VERIFIED |
| 创作者目标发现 | 看出内容从哪个监控目标来 | 显示目标；作品作者在没有 ID 时仍未知，关系 `NOT_VERIFIED` | collection dispatch proof 通过 | NOT VERIFIED |
| 三种排版 | 在研读、表格、封面间切换 | 同一 items、同一选择、同一 Inspector；不重新 fetch | JS/source test 通过 | VERIFIED：实际点击写入 `layout`，三种 DOM 布局切换且选择不变 |
| 排版选择器 | 在三种排版间快速辨认当前项 | 单条文字 tab rail；无独立方格、无黑底选中块；signal 下划线只标当前项 | CSS 正负断言通过 | VERIFIED：1440/375 实拍；上下边线 1px、当前下划线 4px、三个命中区高度 44px；点击仍写入 `layout` |
| 状态快速筛选 | 理解并使用全部/部分取得/风险停止/媒体已清理/撤回或受限 | 它们是 `view` 预设查询，不是页面导航或 layout | Rust/source test 通过 | VERIFIED：横向工具条实际点击写入 `view` 并返回读取回执 |
| 系统/我的视图 | 区分系统预设与个人保存视图 | 系统区有五项真实查询；个人保存合同缺失时只有禁用空态，无假按钮/假视图 | Rust 正负断言通过 | VERIFIED：1440/375 均显示双区；`data-ev-saved-view=0`，空态 `aria-disabled=true` |
| 表格可读性 | 长时间比较作品、作者、时间和状态 | 标题 14px、作者上下文 12px、辅助正文 11px、状态 10px；桌面行高 92px | CSS/source test 通过 | VERIFIED：1440 computed style 与 row geometry 实测一致；375 堆叠后字号保持 |
| 首屏层级 | 直接进入材料工作面 | 无重复页面宣言、无独立状态侧栏；列表 + Inspector 为主体 | Rust 负向文案断言通过 | VERIFIED：1440×900 中结果 784px、Inspector 440px；无 retired copy |
| 375px 作品行 | 阅读 lane、主要限制与最近观察 | 页面宽度不溢出；两个底部字段不重叠 | CSS/source test 通过 | VERIFIED：`scrollWidth=clientWidth=375`，字段 bounding boxes 不相交 |
| 读取失败/受限 | 不制造空库或远程 fallback | 原有 inline 状态和受控媒体边界保留 | Rust/source test 通过 | NOT VERIFIED |

## 分层结论

| 完成层 | 结论 | 证据 | 限制 |
|---|---|---|---|
| 设计规格一致 | VERIFIED（branch） | PAGE、Manifest、LIDS migration log、CSS/JS 静态检查、1440/375 实际渲染 | 未做 Chrome/900/390，多媒体根未接入隔离实例 |
| 前端/组件实现 | VERIFIED（branch） | 三 layout selector、响应式 CSS、共享 API root | 未合并、未发布 |
| 自动检查 | VERIFIED | Node 语法、125 项插件测试、Rust compile/test、PostgreSQL proof、新增状态预设/重复文案聚焦测试 | 自动检查不替代视觉走查 |
| 本机 UI 走查 | VERIFIED（隔离实例） | 13 个作品集合；1440/375 几何、双视图区、表格字号、截图、真实系统视图点击与 URL 状态 | 不是 `:3000` 部署，媒体根未接入，不是 Chrome/Mog 验收 |
| 真实链路/回执 | PARTIAL | Target/WorkOrder/Package 合成链；真实已有目标数据只做先前诊断 | 签名详情探针未获确认，未执行 |
| 部署 | NOT VERIFIED | 无 | 共享 migration/API/插件均未发布 |
| Mog / 业务验收 | NOT VERIFIED | 无 | 等待 PR、运行页与用户验收 |

## 不得据此推断

不得把本报告解释为真实 XHS 发布时间字段已核实、12 条历史笔记已回填、共享数据库已迁移、Chrome 已加载新插件、新 UI 已部署到 `:3000`、本轮视觉已由 Mog 最终接受或任务可合并。真实签名详情探针仍需要当次外部请求确认。
