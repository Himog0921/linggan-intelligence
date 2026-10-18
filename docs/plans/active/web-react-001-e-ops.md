# WEB-REACT-001-E · 运营台账五页迁移

> 状态: 活跃计划
> 最后核对: 2026-10-18
> 适用范围: WEB-REACT-001 阶段 E / Issue #438
> 事实来源: Mog 当前会话派定、主计划、PAGE-OPS-001/002、OPS-HTTP-001、main 712286c3 的代码与 3000 实际页面
> 冲突时以谁为准: 用户最新确认、AGENTS.md、真实合同与运行证据

## 2026-10-18 当前状态补记

E 已随 [PR #439](https://github.com/Himog0921/linggan-intelligence/pull/439) 合入 `82d48a72`；运营概览后续改版 #444 已合入 `07d6c6ca`，两者已包含在正式 3000 的 `acdfbb87` 中。下文截图与测试属于原 E 迁移候选；不能用它们冒充 #444 的视觉证明。Mog 业务验收独立保留。

## 授权与边界

Mog 明确要求开始运营台账五页。按 G 后 E → F 顺序实施 E；C/D 的剩余验收独立保留，不宣称前序全部闭环。UI 保持当前实际页面，每次完成需实际截图核对。单执行者 Codex root；基线 712286c39bd0fc57ca71097a3abba1d5aca3aa90；分支 `codex/web-react-001-e-ops`；唯一工作树为仓库 `.worktrees/web-react-001-e-ops/`。

Issue 与 Claim：[438](https://github.com/Himog0921/linggan-intelligence/issues/438)。完整能力、读写 API、状态、旧文件、文件所有权及验收矩阵以该 Issue 为盘点入口。

不做运营登录/台账写入/自动发现新功能，不改业务规则、schema、导入数据、worker 或插件；不写共享库、不调用平台/模型；合并部署另行派定。

## 表面与能力地图

| 表面 | 必须迁移的能力 | 接口 |
|---|---|---|
| 共用外壳 | 业务选择含 all、失效选择纠正、五页侧栏、导出时点、错误重试、键盘/焦点/减少动态效果；/ops 仍默认 creation | businesses + 各页 asOf |
| 总览 | 昨日/周/月/自定义、加微目标、漏斗、长趋势粒度、每日产出指标/人、待处理展开、排行、平台、矩阵 | overview、overview/trend、targets GET/POST |
| 发布 | 五读数、运营人/平台/账号/日期/待补筛选、URL、分页、16 列、重复说明、封面大图、禁用写按钮 | summary、publications、covers |
| 账号 | 人员筛选、计数、12 列、主页、手填健康未开放 | accounts |
| 每日 | 人员筛选、最近 200 行、12 列、状态、业务不拆分提示 | daily |
| 导入导出 | 三类六种下载、数据时点、导入禁用 | export |

## 状态词典

未知 — 与真实 0 分开；未接入/未设定/无迁入覆盖/当期为空分开。人工迁入不等于平台确认。抖音选填且不判爆款。读取失败不显示空数据、不保留其他筛选的旧数字。目标仅服务器回执后确认；stale 显示最新版本，409 不冒成功；无回执重试沿用原请求号和原 payload。快速切换、关闭及路由卸载隔离旧请求和回执。

## 依赖与实施

L1，Token → 已有 Primitive → 本包表格/图表/对话框 → Page。样式 CSS Modules；AppShell/shell.css 与唯一 lids_tokens.css 复用，不改 Token。不合入 C/D 或旧分支候选。

顺序：能力盘点及文档 → 3000 只读截图 → JSON 解码/React 五页 → 同 PR 删除旧页面/资产/DOM 测试 → 隔离 PostgreSQL + Axum + React → 同数据截图对照 → 类别自审/提交前审查 → PR 交付。

必要 shared seams：App.tsx/pages.ts、web_app.rs/ops.rs/shell.rs、local_web.rs 测试登记、test-web-app-postgres.sh、文档索引/主计划/migration log/progress/生成物登记。本包负责有界修改，遇到并行冲突报告 Mog。其他页面、内核、runtime-main 与共享 root 禁止改动。

## 验收矩阵

| 类别 | 证明 | 当前结果 |
|---|---|---|
| 功能 | 五页功能盘点逐项核对，筛选/深链/分页/封面/图表/对话框 | 已通过，详见验收记录 |
| 状态 | unknown/0/覆盖/失败、旧请求隔离、目标 stale/409/无回执重试 | 已通过，详见验收记录 |
| 真实链路 | 隔离 PG + 正式 Router + React，目标回执/DB、六导出 bytes/asOf | 已通过，详见验收记录 |
| 视觉 | 当前 3000 与同数据候选，1440/1280/1024/390，完整滚动与关键状态 | 已通过，详见验收记录 |
| 删除 | 旧 HTML/JS/CSS/Rust 渲染/纯 DOM 测试及旧 assets 路由无残留 | 已通过，详见验收记录 |
| 自动检查 | TS/lint/Vitest/build、Rust/PG、治理、独立提交审查 | 自动检查已通过；GPT 只读独立审查 PASS，两项发现已闭合 |
| 部署与 Mog 验收 | 另行授权和签收 | NOT VERIFIED |

## 风险、停止与回退

目标并发/无回执与业务范围优先保真。图表保留当前算法与几何，生命周期归 React。真实截图 private，不上传。业务/权限变更、设计未定、共享接缝冲突停止受影响部分。候选不替换 3000；代码通过整 PR 回退，不留双轨开关。

## 当前候选证明

实现、自动检查、隔离链路与实际 UI 对照已形成：[阶段 E 验收](../../design/acceptance/web-react-001-e-acceptance.md)。提交前 Sonnet 独立复核遇到账号会话额度限制后，Mog 于当前会话明确允许改用 1 个 GPT 只读 reviewer，按同一规范复核后提交 Draft PR。审查 task-id 为 WEB-REACT-001-E-GPT-REVIEW-20261016；源码候选完成，独立审查已 PASS（tree e0e9921ef9931dfd88272249dfbe8cc843800a7e）；两项有效发现已修正，前端410项及实际256组截图/隔离链路复核通过，进入提交与 Draft PR 交付。部署/Mog 签收分别记录，不由测试推断。
