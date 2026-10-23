# OCR-MANUAL-HUMAN-ADOPTION-454 · 合并后发布运行手册

> 状态: 草案
> 最后核对: 2026-10-23
> 适用范围: #477 先合并、#479 rebase 后独立合并后的 `0145_ocr_manual_human_adoption.sql` 与本机 3000 更新
> 事实来源: 现行 migration、`scripts/runtime/install.sh`、`docs/runbooks/local-runtime-deployment.md` 与本包隔离证明
> 冲突时以谁为准: Mog 对精确 main head 的发布授权、实际运行身份、备份/恢复回执与 migration ledger

## 前提与禁止事项

本手册只描述获得一次精确 main head 发布授权后的受控操作顺序。它不授权合并 PR、写共享库、重启 3000、启用 patrol/media worker、调用模型、外发图片/文本，或代替 Mog 的 G4 签收。

开始前须已有：#477 已在其当时精确 head 合并；#479 已针对新 `main` rebase、通过新 head 的 CI 与独立审阅并获准合并；目标 `main` 完整 SHA 已记录。#477/#479 的候选 CI、临时集成 PR 或隔离浏览器证明都不能替代这一条件。

## 只读发布预检

在任何停机或写库之前，逐项记录并停止于不一致项：

1. `git fetch origin main` 后记录 `origin/main` 完整 SHA；分别记录 canonical 开发 checkout 和 `/Users/moglenny/Library/Application Support/Linggan Intelligence/runtime-main` 的 SHA、clean/dirty 状态。运行目录不是手工修改目标。
2. 记录 3000 listener 的 PID、cwd、executable 与 `/health`。健康回执中的 `webApp.revision`、`runtimeRevision`、数据库状态和目标 main 必须可解释；HTTP 200 本身不放行。
3. 只读 `launchctl print-disabled`、已加载 label 与 PID。patrol/media 的既有禁用意图必须保持；本包不把 disabled worker 当作可启用对象。
4. 只读检查共享库 migration ledger：列出最后已应用 migration、`0145_ocr_manual_human_adoption.sql` 是否缺失，以及任何比 0145 更新的未应用项。运行服务不得承担 migration。
5. 验证备份位置可写、备份工具可用、恢复演练的独立目标已定义；记录数据库容量、可用磁盘与预计备份空间。不得把“命令存在”记成“可恢复”。

预检记录必须明确实际执行人与时间、目标 SHA、ledger 状态、listener 身份、worker 意图、备份路径（不记凭据）和可用空间；任一未知或不一致即停止，保持现状并请求决定。

## 备份、迁移与恢复验证

1. 在获批维护窗口前完成可验证备份，记录产物 hash/大小、开始/结束时间与访问控制；不把 DSN、口令、原始图片或原文写进仓库或普通日志。
2. 对隔离的恢复目标执行恢复并只读验证：数据库可连接、关键 schema/ledger 与备份时点一致，且恢复过程没有触碰共享库。恢复验证失败时不迁移。
3. 按 `local-runtime-deployment.md` 的 prepare-migration 停止顺序与停止证明执行；确认 3000 和相关服务已按该手册要求退出后，才使用受控 migration 入口应用 `0145`。不得在运行服务上直接迁移。
4. 迁移完成后只读核对 ledger 恰好含 `0145`、SQL 约束存在且没有意外的未应用 migration。任何 ledger 不一致、约束缺失或迁移异常都停止，不尝试手工补写 ledger。

## 更新、回读与失败边界

1. 仅从已经获准的 clean `main` 更新 `runtime-main`，以 `scripts/runtime/install.sh` 的受控路径构建和加载；不从共享 dirty checkout、临时集成分支或 PR worktree 发布。
2. 新 listener 必须来自 `runtime-main`，并以 PID/cwd/executable、完整 revision 和 `/health.webApp` 共同核对。保持 patrol/media 的原禁用状态。
3. 打开真实受控页面并回读：作品详情的原图对照入口、人工采用正文、来源片段、未采用 preview 的隔离提示，以及刷新后的 receipt 状态。此时仍不自动采用其它结果，不调用 provider。
4. 发布记录须区分：代码/CI、迁移/ledger、运行/health、真实页面读回、Mog/G4。只有最后一项可确认真实原图是否应采用、来源片段是否可理解，以及未采用 preview 是否确实隔离。

以下任一情况是回退或停止条件：备份/恢复验证失败；ledger 与目标不符；运行身份或 health revision 不符；页面读取异常；worker 意图被改变；出现任何未预计写入、外发或真实材料副作用。没有新结构写入且已确认兼容时，才依既有整版回退合同回到已验证 revision；不做 down migration、不用新后端配旧页面，也不以回退掩盖 ledger 异常。

## 发布记录模板

```text
release: OCR-MANUAL-HUMAN-ADOPTION-454
authorized main SHA:
operator / maintenance window:
preflight: origin/main | runtime-main | 3000 PID/cwd/executable | health | worker intent:
backup + isolated restore verification:
migration: target 0145 | before ledger | after ledger | constraints:
runtime update: source SHA | listener identity | health revision:
real-page readback: receipt | human body | source fragment | preview isolation:
failure/rollback decision (if any):
Mog/G4 decision: PASS / repair items / not yet reviewed
```
