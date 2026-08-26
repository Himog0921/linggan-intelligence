# ACC-DESIGN-008 · 共享壳层中文优先验收

> 状态: 一次性报告
> 最后核对: 2026-08-26
> 适用范围: Issue #68 / DESIGN-008 的 shared local-web shell 文案与排版层
> 事实来源: Draft PR 的实际 diff、focused Rust render test、LIDS checks 与安全本地页面走查
> 冲突时以谁为准: 真实运行结果、代码与合同；本报告不替代 LIDS-LANG-001、页面规格或数据合同

## 1. 验收对象

- 共享页面: `/corpus/evidence`、`/collection/*`
- 共享实现: `apps/api/src/local_web/shell.rs`、`shell.css`
- 数据前提: 使用安全本地 synthetic render；不读取、输出或写入真实平台材料。
- 关联: `LIDS-LANG-001`（PR #67 的项目级语言规则）、`PAGE-EVIDENCE-001`、`PAGE-COLLECTION-001`、`LIDS-PRI-001`

## 2. 核对矩阵

| 场景 | 预期 | 结果 |
|---|---|---|
| Corpus shared header | 中文说明读模型、来源与时区；原技术码紧邻且更小 | 已通过：本地 `/corpus/evidence` DOM 与截图 |
| Collection shared header | 中文说明调度器、观察目标、运行时与时区；原技术码紧邻且更小 | 已通过：本地 `/collection/operations?mode=now` DOM 与截图 |
| 一级导航 | 中文为主；`RADAR` / `TOPIC MAP` / `CORPUS` / `INSIGHTS` / `COLLECTION` 为小型旁注 | 已通过：中文 13px Sans；英文技术键 9px Mono |
| 共享导轨 | Corpus / Collection 不再以英文读数单独显示 | 已通过：共用导轨显示“语料库”或“采集” |
| `UNKNOWN` | 显示“未知”并保留 `UNKNOWN` 技术码；不改变实际 unknown 语义 | 已通过：两页均显示中文主语义与原码 |
| `UTC+08` | 显示“中国标准时间”并保留 `UTC+08` 技术码 | 已通过：两页均显示中文主语义与原码 |

## 3. 分层结论

| 层级 | 结论 | 证据 | 未证明 |
|---|---|---|---|
| 设计规则采用 | IMPLEMENTER VERIFIED | 本事项仅应用 LIDS-LANG-001，不新建 Token / Primitive / Pattern / CMP | PR #67 尚未合并前，项目级规则的最终整合状态 |
| 前端实现 | IMPLEMENTER VERIFIED | shared shell 将中文主文案与 `.v7-tech-key` 技术旁注分离；Collection 模式短标签中文化 | Evidence Library 页面局部模板中的文案，由 #67 单独验收 |
| 自动检查 | IMPLEMENTER VERIFIED | `cargo test -p linggan-api` 通过；shared shell 单元测试覆盖 Corpus 与 Collection 两条输入 | 被忽略的隔离 PostgreSQL 证明路径与本事项无关 |
| 安全本地 DOM / 视觉 | IMPLEMENTER VERIFIED | 以 `127.0.0.1:3010` 的无材料安全本地运行时走查 `/corpus/evidence` 与 `/collection/operations?mode=now`；DOM 记录中文主语义为 11–13px Sans、英文技术旁注为 9px Mono，并检查截图 | 不读取真实素材、不开平台采集；不替代非实现者复审 |
| 真实链路 / 部署 | N/A | 本项不产生 API、DB、插件、媒体或外部副作用 | 不证明产品链路或用户业务验收 |

## 4. 明确边界

- 本卡不会把 `UNKNOWN` 改写为 0、空值、成功、失败、完整或不存在。
- 本卡不会把 `UTC+08` 改为其他时区，也不会依据浏览器时钟推断数据事件时间。
- 本卡不负责 Evidence Library page-local 的搜索、筛选、卡片、Inspector 或动态状态文案；该范围由 #67 / PR #67 负责。
- 本卡不证明任何真实采集、Evidence 接纳、媒体获取、OCR/ASR、数据库读写或部署。
