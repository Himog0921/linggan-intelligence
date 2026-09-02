# PLUGIN-REHOME-001 · 完整迁入灵感爆爆爆浏览器插件源码基线

> 状态: 已完成计划
> 最后核对: 2026-09-02
> 适用范围: GitHub Issue #41；将 `linggan-boom v2.0.91` 的完整浏览器端 source / UX 迁入 Linggan，并切断旧内容工作台运行依赖
> 事实来源: Issue #41、`plugins/linggan-intelligence-browser/` 当前 source、构建/隔离验证输出
> 冲突时以谁为准: 用户最新确认、`AGENTS.md`、本计划、当前代码与独立 review；旧插件 source 只为迁入依据

## 用户可见目标

Linggan Intelligence 拥有一份真正属于自己的、可构建与可安装的完整浏览器插件源码包。
它保留用户已经熟悉的 Popup、Dashboard、页面注入控制、任务控制位置、资产主题和
采集器代码结构；不再把简化的 `linggan-browser-producer v0.2.0` 描述成完整迁移答案。

## 允许范围

- 将精确来源 `linggan-boom@8a00cc1` / `v2.0.91` 的浏览器端完整 source、assets、themes、build 和测试基础迁入本仓库独立包；
- 用 Linggan adapter 替换**活跃**后台入口，切断旧内容工作台 host、授权、polling、lease、station dispatch、sync 与 fallback；
- 让尚无 Linggan backend 合同的动作在原 UI 位置显示明确 pending 原因；
- 提供 source 迁入映射、可构建 release、隔离扫描、项目索引与进度记录。

## 不做项

- 不实现 Rust / PostgreSQL / Linggan API 合同，不连接真实平台或账号；
- 不启用详情、评论、媒体、批量、抖音、自动化、OCR、ASR；
- 不下载、保存、处理真实内容或媒体，不修改旧 `linggan-boom` source repo；
- 不把浏览器本地暂存当成 Linggan 数据库，也不把构建证明写成真实采集证明。

## 退出条件

1. 迁入插件能从 source clean build，并生成本仓库自有 release ZIP；
2. Popup、Dashboard、页面注入控制、主题/资产和 XHS/抖音 collector source 均可在 source tree 中核验；
3. Webpack active runtime 是 `src/linggan/background.js`；built artifact 不含旧工作台 host 或 endpoint；
4. 未支持动作有明确 pending 文案，Manifest 未保留 cookies、下载、定时、网络规则与通知等旧权限；
5. 迁入映射、生成物登记、索引、当前状态和当月进度均明确这是 Draft source baseline，不是实际采集完成；
6. Draft PR 已请求独立 review。实施者不自批、不合并、不关闭 Issue。

## PR #42 复审修正（2026-08-25）

- 活跃 `content` 入口只保留 Linggan-owned 页面控制壳和明确 pending 回执；旧的
  collector / workbench / lease / heartbeat / outbox / polling 源码继续留在 source tree，
  但不允许进入活跃内容模块图；
- Dashboard 的“下载媒体”保留原操作位置，但当前直接返回 `media_download` 未接通，
  不读取本机笔记记录、不访问平台也不调用下载器；
- release ZIP 的验证不再通过重新覆盖 ZIP 得出结论：新的 `npm ci → build → temporary
  package` 必须与提交 ZIP 的 SHA-256 完全一致。

这些是迁入包的运行边界修正，不是平台采集、媒体入库或 Linggan backend 已接通的证明。

## 2026-09-02 · 完成记录

PR #42 已于 2026-08-25 合并为 `3ea210dd2bd426750a30c510e233564a9c35a6f9`。已证明范围是 Linggan 自有插件源码、构建与 release 基线，以及旧内容工作台运行路径切断；本计划的 build、隔离扫描和独立审查记录继续作为该结论的证据。

未证明范围保持不变：真实浏览器加载、平台采集、媒体、OCR/ASR、Linggan 后端数据互通、部署和业务验收。本次归档不改变当前 Browser Producer 的后续版本或运行合同。
