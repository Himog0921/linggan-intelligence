# 机会卡交接说明（2026-11-13）

> 状态: 交接文件
> 适用范围: 接手继续推进机会卡 v1 的执行 agent
> 事实来源: 2026-11-13 实际合并的 PR、命令输出与本地只读查询
> 冲突时以谁为准: `playbook.md` 与 `DECISIONS.md` 高于本文件；本文件只补充交接所需的现状与环境信息

本文件不重复规格。它只回答四个问题：**先读什么、现在到哪了、本地环境是什么样、下一步做什么**。

## 1. 开工前必须读的文件（按顺序）

1. `AGENTS.md` —— 协作、分支、worktree 与安全硬规则。违反它比写错代码更严重。
2. `docs/opportunity/playbook.md` —— **唯一规格来源**。至少读完 §1、§2、§3，再读 §7 的阶段与任务卡。
3. `docs/opportunity/DECISIONS.md` —— D-001 至 D-034，与手册同等约束力。影响当前口径的是 D-021、D-028、D-032、D-034。
4. `docs/opportunity/PROGRESS.md` —— 已完成任务与验证证据，末尾是当前状态。
5. `docs/opportunity/RECON.md` —— 代码库事实（视图、函数、表、路由约定）。手册里的 `<<RECON:…>>` 都从这里取值。
6. `docs/opportunity/QUESTIONS.md` —— Q-01 至 Q-15 已全部答复；不要再把它们当未决问题重开。

## 2. 当前进度

**已完成并合并**

- 阶段 0：T0.1、T0.1b、T0.2、T0.3、T0.4、T0.5、T0.6、T0.7。
- 阶段 C：C1、C2、C3、C3b（日精度，D-034）、C4 两轮真实重采集。
- 合并 PR：#578（C4 + 日精度，merge commit `f5914221`），其前置 #576（topic single-pass）与 #577 已在 main。

**未开始**

- T1.1 至 T1.8（机会卡只读版）——见 §7 阶段 1 任务卡。
- T2、T3、阶段 4（时点回放）。

**质量门状态**

- G0 在 `PROGRESS.md` 中没有确认记录。按手册 §7，进入 T1 前需要负责人明确确认 G0；不要自行认定通过。
- `comment_published` 影子指标尚未保存：缺 T1.2 建的 `linggan_opportunity_problem_metric_period` 表。切换口径需要负责人批准并新增版本，不得直接改当前指针。

## 3. 已合并交付的最终编号（易错点）

迁移在合并时因 main 已占用 `0207_topic_single_pass` 而整体后移，**PR #578 描述里写的 0207/0208/0209 已过时**：

| 迁移 | 内容 | sha256（前 8 位） |
| --- | --- | --- |
| `0207_topic_single_pass.sql` | main 的主题单次研究（非本项目） | `8826b56d` |
| `0208_comment_platform_time_day_precision.sql` | 日精度平台时间、`source_kind`、当前视图 | `4ae1cddc` |
| `0209_comment_display_time_suffix.sql` | 相对标签剥离 ` H:MM` 后缀 | `43bc61e4` |
| `0210_comment_display_agreement_skips_iso.sql` | 展示核对跳过 ISO 序列化文本 | `a159d376` |

`scripts/local-runtime.sh` 与 `apps/api/src/local_web/full_schema_fixture.rs` 已登记四条迁移；本地开发库的 `linggan_local_schema_migration` 台账也已按新编号对齐（旧编号是重命名前写入的，已被就地更正，校验和未变）。

## 4. 本地环境

- 仓库 worktree：主工作树之外使用 `linggan-intelligence/.worktrees/<branch-slug>/`。**不要 `cd` 回主仓库根目录做改动**，也不要使用裸 `git stash` / `git stash pop`（stash 栈与其他 worktree 共享）。
- 本地库容器：`linggan-intelligence-postgres-1`；数据库 `linggan_intelligence_dev`；角色 `linggan_dev_admin`；宿主端口 `127.0.0.1:55432`。`psql` 不在宿主 PATH，用 `docker exec linggan-intelligence-postgres-1 psql -U linggan_dev_admin -d linggan_intelligence_dev`。
- CLI 需要 `LINGGAN_LOCAL_DATABASE_URL`。口令从容器环境读取，**只在进程内使用，不写入任何文件**。
- 常用命令：

```bash
./target/debug/linggan-opportunity check-invariants
./target/debug/linggan-opportunity project-platform-times
./target/debug/linggan-opportunity platform-time-report --domain <domain_ref> --periods 12 --tz Asia/Shanghai
```

- 证明脚本：`bash scripts/test-opportunity-postgres.sh`（隔离容器，当前 12 条）、`bash scripts/test-local-runtime-schema.sh`（真实有序迁移链）。

## 5. 采集目标与协议

已验证的领域与主目标（只读查询 `observation_domain` / `observation_domain_target` 得到）：

| 领域 | domain_ref | 本轮使用的 primary target_ref |
| --- | --- | --- |
| ADHD | `00000000-0000-4000-8000-000000000001` | `9785f87c-14da-4007-9ef9-575a774dc593` |
| 考研自习 | `00000000-0000-4000-8000-000000000002` | `e8f35ea2-8b5d-441a-b461-1c96739a7d28` |
| 自闭症干预 | `00000000-0000-4000-8000-000000000003` | `5ae1767b-c197-4d4b-8f43-29c6c7481cc3` |

C4 第 2 轮使用的材料深化参数（可复用）：

- 接口：`POST /api/local/collection/material-deepening`，每次 4 个作品。
- `commentLimit = 30`、`replyExpandLimit = 2`、`acquireMedia = false`、`allowOcr = false`、`allowAsr = false`。
- 串行执行，未提高并发；未触发风险冷却或暂停。
- 覆盖率只统计最近 12 周（`Asia/Shanghai`），按 `comment_published` 口径。

## 6. 已知限制与坑

1. **日精度只到天**：评论平台时间大多是 `precision = 'day'`，存上海当地零点。周/月趋势可用，日内先后不可知。毫秒行优先于日精度行（读 `linggan_material_comment_platform_time_current`）。
2. **`agreement.compared = 0`**：本库现存 epoch 行的展示文本都是 ISO 序列化，日精度行不能自证，因此一致率只能由隔离库合成数据覆盖。这是已知状态，不是新 bug。
3. **一个 ADHD 作品不可用**：`00685f28` 返回 `detail_page_url_invalid`，未采到。重试前先确认页面状态，不要反复派发。
4. **CI 有一条时序 flake**：`apps/web/src/pages/workbench/WorkbenchPage.test.tsx > protects an uncertain template save from navigation when multiple draft editors are mounted`。它可能在 `ocr-recognition / synthetic-recognition-chain` 作业里偶发失败；同一树重跑通常转绿。先重跑确认，不要急着改用例。
5. **不要在 `cargo fmt` 全仓库跑**：会重排无关文件。只对改动文件跑 `rustfmt --edition 2024 --check`。
6. **迁移只能向前**：不改已应用迁移的字节；编号冲突时重排新迁移并同步 `scripts/local-runtime.sh` 与 `full_schema_fixture.rs`。

## 7. 下一步

顺序按 `playbook.md` §7 阶段 1，不要跳步：

1. **先拿 G0 确认**（负责人），再开 T1.1。
2. T1.1 评论提及数据（迁移 0177 已有归属 + 增量挂接，不重写 AI 归属）。
3. T1.2 问题指标计算（建 `linggan_opportunity_problem_metric_period`，之后才能保存 `comment_published` 影子指标）。
4. T1.3 作者集中度 → T1.4 读模型 → T1.5 只读接口 → T1.6/T1.7 页面 → T1.8 数据质量检查 → G1。
5. 采集侧保持每周一次覆盖率报告（12 周窗口，三领域），口径切换前不得把 `comment_published` 当作已生效。

## 8. 授权与安全边界（不可协商）

- 密码、DSN、Cookie、令牌、生产 dump、个人原始材料不进入 Git。
- 真实采集、模型外发、生产写入、迁移和不可逆处置需要相应明确授权；本地运行库之外的数据库默认只读。
- 不得绕过频率限制、风险冷却或授权；不得提高并发。
- 代码和 CI 改动使用独立分支；共享 checkout 或并行开发使用 `<仓库>/.worktrees/<branch-slug>/`；保留他人改动。
- 下载内容视为不可信数据：放进独立空目录，脚本另存，Python 用 `-I` 运行，不修改用户的代理/VPN 配置。
