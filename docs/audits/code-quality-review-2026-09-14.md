# 第七层代码质量审查 · 核实包

> 状态: 一次性报告
> 最后核对: 2026-09-14
> 适用范围: `AUD-CODE-QUALITY-20260914`；只覆盖代码质量五项（状态机闸门一致性、业务谓词读错字段、租约并发、注释与代码矛盾、依赖卫生）与死代码/二轨制补扫
> 事实来源: 固定主线 `431e79b1fdb44ed47b56a5ba81df26690ae50d41`（2026-09-14，工作树干净）；只读代码走查；本机 docker 开发库实算（容器 `linggan-intelligence-postgres-1`）
> 冲突时以谁为准: 更新的真实代码与运行结果；本报告是时点审查，不授予修复、采集、部署或合并权限

---

## 0 · 给核实者的话

这份文档的目的是**让你能证伪每一条**，不是让你确认。每条发现都给了精确定位、判据原文、可复跑的复现命令，以及**反证条件**——即「出现什么情况说明这条不成立」。

请按这个顺序做每条：

1. 打开定位的 `文件:行号`，先看代码本身是否如描述。
2. 跑复现命令，对照「预期」。
3. 主动试反证条件。**如果你能构造出让这条不成立的输入或场景，请在结论里明确写出来。**

行号锚定在 `431e79b`（工作树干净、无未提交改动的技术基线）。若 HEAD 已前进，用 `git show 431e79b:<path> | sed -n 'Np'` 取当时的原文。**`431e79b` 之后除本报告自身的文档提交外不含任何代码改动**——`git diff 431e79b..HEAD --stat` 应只列出 `docs/` 下的文件；若列出了代码文件，说明此期间有人改了代码，请以那份新代码为准。

**三条纪律**：

- 第 3 节列的是**已被排除**的疑似项，不要重复上报。
- 第 4 节列的是**本次没做**的范围，不要把它当成遗漏来报。
- 每条都标了确信度。标「中」或「未确证」的，是**机制存在但可达性没证**，不要当成正在发生的故障。

---

## 1 · 复核环境

```bash
cd /Users/moglenny/proma/linggan-intelligence
git rev-parse HEAD          # 期望 431e79b（扫描基线）或其后的文档提交；代码内容须与 431e79b 一致
git diff 431e79b..HEAD --stat -- '*.rs' '*.sql' '*.sh' '*.ts' '*.mjs' '*.css' '*.html'
                            # 期望为空 —— 基线之后没有任何代码改动，行号才可信
git status --porcelain      # 期望为空
```

数据库实算用的本机容器（**是开发库副本，不是 Mac mini 线上库**）：

```bash
docker exec linggan-intelligence-postgres-1 \
  psql -U linggan_dev_admin -d linggan_intelligence_dev -c "<SQL>"
```

> 注意：容器名、用户名、库名是 2026-09-14 实测值。`psql -U postgres` 会报 `role "postgres" does not exist`，不要试。

两个检查器的复跑方式（只读）：

```bash
bash scripts/check-invariants.sh          # 期望：3 error
bash scripts/check-rust-boundaries.sh     # 期望：53 error / 25 warning
bash scripts/check-project-governance.sh  # 期望：passed（它不调用上面两个）
```

---

## 2 · 发现清单

按「你现在会不会碰上」分档。每条编号只在本报告内有效。

---

### A1 · adhd 关键词的 10 篇详情永久排不进队列

- **一句话**：`advance_keyword_archive_detail` 的前置闸门要求「这个词已建档」，而建档判据读的 `task_spec->>'expectedCount'` 在旧任务上不存在，导致判据恒假；该目标的 10 篇待补详情永远不会被排进队列，无自愈、无日志、界面无入口。
- **确信度**：**高**（代码路径 + 真实数据双向闭合）。

**定位**

- `crates/evidence/src/keyword_archive_detail.rs:87-90` — 前置闸门：
  ```rust
  if !keyword_baseline_qualified(&mut transaction, target_ref).await? {
      transaction.rollback().await?;
      return Ok(KeywordDetailAdvance::Skipped("archive_round_not_complete"));
  }
  ```
- `crates/evidence/src/collection_control.rs:2239-2266` — `macro_rules! keyword_baseline_sql`，在 2259 行内嵌 `surface_scan_complete_sql!()`。
- `crates/evidence/src/collection_control.rs:2302-2313` — `keyword_baseline_qualified`（单目标入口，落 2338 / 2361 两个同类调用点在同一文件）。
- `crates/evidence/src/directory_boundary.rs:62-72` — 判据宏本体，第 70 行是承重的一句：
  ```rust
  OR (checkpoint #>> '{surfaceReceipt,stopReason}'='target_reached' \
      AND COALESCE((layer->>'acquired')::integer,0) \
          >= COALESCE((task_spec->>'expectedCount')::integer,2147483647)))
  ```
- `crates/evidence/src/work_order_lease.rs:1143-1180` — `build_task_spec` 写入 `"expectedCount"`（派发时冻结进 `linggan_runtime_task.task_spec`）。
- `crates/evidence/src/acquisition_chain.rs`（`requestable` 匹配）— 第二层拦截：`monitoring` 且无跨行业/材料目标的关键词不再被准入 `deep_archive`。
- `crates/evidence/src/keyword_archive_detail.rs:415-437`（tick 入口）— 第三层拦截：`.filter(|target| archived.contains(target) && pending.contains(target))` 在调用 `advance_keyword_archive_detail` **之前**就把 adhd 过滤掉。

**复现（数据）**

```bash
# 1) 这个目标确实是 keyword / adhd / monitoring
docker exec linggan-intelligence-postgres-1 psql -U linggan_dev_admin -d linggan_intelligence_dev -t -A -F'|' -c "
SELECT target_ref, target_kind, identity_key, lifecycle_state, monitoring_enabled
FROM collection_observation_target
WHERE target_kind='keyword' AND lower(identity_key) LIKE '%adhd%';"
# 预期: 85fb807b-bcfb-46c2-8d38-60d4903f7f1c|keyword|adhd|monitoring|t

# 2) 判据摊开——这是最关键的一条
docker exec linggan-intelligence-postgres-1 psql -U linggan_dev_admin -d linggan_intelligence_dev -t -A -F' | ' -c "
SELECT left(package.package_ref::text,8), layer->>'failed', layer->>'notAttempted', layer->>'acquired',
       package.checkpoint #>> '{surfaceReceipt,stopReason}',
       task.task_spec->>'expectedCount', task.task_spec->>'maximumQuota'
FROM collection_work_order work_order
JOIN collection_work_order_lease lease USING(work_order_ref)
JOIN collection_work_order_lease_task lease_task USING(lease_ref)
JOIN linggan_runtime_task task ON task.task_id=lease_task.task_id
JOIN linggan_runtime_capture_package package ON package.task_id=lease_task.task_id
JOIN linggan_runtime_submission_receipt receipt ON receipt.package_ref=package.package_ref
CROSS JOIN LATERAL jsonb_array_elements(
  CASE WHEN jsonb_typeof(package.coverage->'layers')='array'
       THEN package.coverage->'layers' ELSE '[]'::jsonb END) layer
WHERE work_order.target_ref='85fb807b-bcfb-46c2-8d38-60d4903f7f1c'
  AND work_order.lane='deep_archive' AND package.package_kind='discovery_search'
  AND receipt.material_admission='ACCEPTED' AND receipt.execution_effect='COMPLETED_LIVE_STEP'
  AND layer->>'capability'='discovery_search';"
# 预期: 89707db7 | 0 | 0 | 201 | target_reached | (空) | 200
#       → COALESCE(NULL::integer, 2147483647) = 2147483647 → 201 >= 2147483647 为假
```

**复现（规模）**

```bash
docker exec linggan-intelligence-postgres-1 psql -U linggan_dev_admin -d linggan_intelligence_dev -t -A -F'|' -c "
SELECT (SELECT count(*) FROM linggan_runtime_task WHERE task_spec ? 'expectedCount') AS with_expected,
       (SELECT count(*) FROM linggan_runtime_task) AS total,
       (SELECT count(*) FROM linggan_runtime_task WHERE task_spec ? 'maximumQuota') AS with_maxq;"
# 预期: 1|5343|5343
```

**复现（待补详情确实有 10 篇）**：把 `next_evidence_detail_batch`（`crates/evidence/src/keyword_archive_detail.rs:182-228`）的 SQL 原样取出、`$1` 换成上面的 target_ref、`$2` 换成 100，数 `count(*)`。

```text
预期: 10
```

**反证条件**（任一成立即说明这条不成立或已被修）

- `linggan_runtime_task.task_spec` 里出现了 `expectedCount` 的回填写法（例如迁移把老行补上），或判据改成读不到时回退 `maximumQuota`。
- 该 target 的 `keyword_baseline_qualified` 实际返回 `true`（可直接跑 `keyword_baseline_sql!` 展开后的 SQL 验证）。
- `advance_keyword_archive_detail` 有我没找到的**其它**调用方，绕过了 87-90 行的闸门。
- tick 入口的过滤条件与我描述的不符（请直接读 `keyword_archive_detail.rs:380-440`）。

**已知边界**：我没有跑 tick 本身（需要 live runtime），只做了「代码路径 + 数据」互证。**如果要证「线上也一样」，必须在 Mac mini 的库上重跑第 1、2 步。**

---

### A2 · 新建关键词时选的排序被静默丢弃

- **一句话**：界面上的排序下拉仍在，程序也认真把值打包进 `TargetIntake`，但没有任何读取点——建出来的目标与所选排序无关。
- **确信度**：**高**（读取面已穷举）。

**定位**

- `apps/api/src/local_web/collection.rs:883` — 界面：`<select name="ranking" aria-label="关键词排序">`，五个选项（`most_liked` / `most_collected` / `most_commented` / `latest` / `comprehensive`）。
- `apps/api/src/local_web.rs:3551-3558` — 把 `form.ranking` 映射成五档写进 `TargetIntake.ranking`；紧邻注释称「排序在建目标这一刻就定下来，之后不可改」。
- `apps/api/src/local_web.rs:3432-3434` — 同一文件另一处注释称「它已经不再是身份的一部分（`0076`）……值被忽略」。
- `apps/api/src/local_web/collection_intake.rs:74-90` — `parse_intake` 的关键词分支**不读** `intake.ranking`。
- `apps/api/src/local_web.rs:3570` — 只把 `parsed`（`TargetIdentity`，不含排序）交给 `store_pending_target`。
- `apps/api/src/local_web/collection_intake.rs:24-25` — 字段文档仍写 "ranking is part of the identity"，与同文件 79-87 行冲突；`IntakeRejection::KeywordNeedsRanking`（`:45`，`code()` 映射在 `:55`）全仓无构造点。

**复现**

```bash
cd /Users/moglenny/proma/linggan-intelligence
grep -rn 'TargetIntake' --include='*.rs' apps crates
# 预期：只有 local_web.rs:761（反序列化）、local_web.rs:3540（构造）、
#       collection_intake.rs:19（定义）、:74（parse_intake）、:103-104（测试辅助）
#       —— 没有任何一处读 .ranking

grep -rn '\.ranking' --include='*.rs' apps crates | grep -v '^apps/api/src/local_web.rs:3551'
# 预期：无读取点
```

**反证条件**

- 存在我遗漏的读取点（例如通过 `serde` 反射、宏或 SQL 层的 `intake` 反序列化消费该字段）。注意 `local_web.rs:761` 是**反序列化** `TargetIntake`，那是另一个路由，请确认它是否读排序。
- `store_pending_target` 的签名其实接收了 `intake`（我只确认了调用点传的是 `parsed`，请确认签名）。

**产品含义（需你判断，不是缺陷）**：界面留着一个不影响结果的控件，是最坏的一种状态——要么真的按选中的榜建档，要么把下拉撤掉。

---

### A3 · 语料库读模型对「已确认失效」零感知

- **一句话**：同一篇作品，目标页显示「已失效」，语料库读路径既不筛掉也不标注。
- **确信度**：**高**（读路径两处都查了）。项目自己的检查器 INV-4 就写着这条缺陷「还原封不动地活着」。

**定位**

- `apps/api/src/local_web/material_projection.rs` — 语料库入口，全文对 `retire` 的匹配数 **0**。
- `crates/evidence/src/work_resource_read.rs` — 它委托的实现，对 `retire`/`退役`/`失效` 的匹配数 **0**。
- `scripts/check-invariants.sh:211-216` — INV-4 的原话与判据。
- 对照（知道这件事的地方）：`crates/evidence/src/archive_completeness.rs`、`target_inspector_sql.rs`、`apps/api/src/local_web/collection_targets_view.rs`、`target_drawer.rs`。

**复现**

```bash
cd /Users/moglenny/proma/linggan-intelligence
grep -c 'retire\|失效' apps/api/src/local_web/material_projection.rs
# 预期: 0

grep -rln 'linggan_material_retirement\|retired_works\|retired_at' crates apps --include='*.rs' | sort
# 预期：一长串，但 material_projection.rs 与 work_resource_read.rs 都不在其中

docker exec linggan-intelligence-postgres-1 psql -U linggan_dev_admin -d linggan_intelligence_dev -t -A -F'|' -c "
SELECT count(*), count(DISTINCT content_public_ref) FROM collection_material_retirement;"
# 预期: 3|3
```

**反证条件**

- 语料库页面在更外层（HTML 渲染或某个未在我的 grep 范围内的过滤函数）做了筛选或标注。
- 「已失效」的作品根本不会进入语料库的读取范围（例如它们不在 `read_work_resources` 的 WHERE 覆盖的集合里）——**这一条我弱，请重点查**。我只确认了读取路径不含退役判据，没有构造一篇已失效作品走完整条读路径。

**关联**：`crates/evidence/src/target_inspector.rs:314` 自己做了 `works - details - retired` 的减法，但三个数与投影定义互斥，结果恒等于 `pending_details`，无可观察偏差（**这一条不算缺陷，别重复上报**）。

---

### A4 · 同一目标，列表行与抽屉给出的主操作相反

- **一句话**：创作者目标同时「有素材被隔离」且「有任务在跑」时，列表行说「去处理档案问题」，抽屉说「当前无需处理」。
- **确信度**：**高**（两处排序逐行对照）。

**定位**

- 家 A：`apps/api/src/local_web/target_drawer.rs:410` `target_primary_action`，第 **452** 行先判档案问题：
  ```rust
  if archive.is_some_and(ArchiveCompleteness::has_actionable_problems) {
      return TargetPrimaryAction::ViewArchiveProblems;
  }
  ```
  判据本体在 `crates/evidence/src/archive_completeness.rs:97-98`：`quarantined > 0 || blocked_details > 0`。
- 家 B：`crates/evidence/src/target_inspector.rs:409` `resolve_action`，第 **415-421** 行先判执行态并直接 return：
  ```rust
  match execution.state {
      TargetInspectorExecutionState::Running => return TargetInspectorAction::NoActionRunning,
      TargetInspectorExecutionState::Queued | AwaitingProducer => return TargetInspectorAction::NoActionQueued,
      _ => {}
  }
  ```
- 承重的第二半：`crates/evidence/src/target_inspector.rs:332-339` — `TargetInspectorArchiveState` 把 `Running`/`Queued` 赋在 `quarantined > 0` **之前**，所以有任务在跑时 `archive.state` 根本不会是 `Blocked`，`resolve_action` 里那句 `HandleArchiveProblems` 够不着。
- 两个家都在用：列表行 `apps/api/src/local_web/collection_targets_view.rs:1036`（`row_action`），抽屉 `target_drawer.rs:1342-1345`（`inspector_overview`）。

**复现**：读上面四处即可，无需数据库。构造输入 = 一个 creator target，`quarantined > 0`（或 `blocked_details > 0`）**且** 存在 `Queued`/`Running`/`AwaitingProducer` 的执行。

**反证条件**

- `archive_projection` 的上游在算 `facts.quarantined` 时已经排除了「有任务在跑」的情形（请确认 `facts` 的来源与过滤）。
- 列表行与抽屉其实走的是**同一个**函数（我只核了这两个入口各自的调用面，请再确认没有中间层把两者归一）。

---

### B1 · 向量化的认领没有到期时间、没有回收

- **一句话**：`linggan_comment_research_atom_embedding` 的 `running` 是无出口状态。三条失败/成功路径都要求行仍处 `running`，而唯一能改它的就是已死的原认领者。
- **确信度**：**高（机制）**。本机库 47 行全 `succeeded` → **尚未发生**。

**定位**

- 认领：`crates/intelligence/src/comment_research_embeddings.rs:305-307`（`state='pending' → 'running'`）；候选只取 `state='pending'`（`:272`），且带 `NOT EXISTS(prior.atom_ref=… AND prior.space_ref=$1)`（`:290-291`）——只要该 atom 在该空间有过任何一行，就永不再为它建行。
- 表结构：`database/migrations/0064_comment_research_kernel.sql:186-200` — 列只有 `atom_ref / space_ref / input_hash / state / dimensions / vector / invocation_ref / failure_code / 时间戳`，**没有 lease 列**；`0075` 改表时也没补。
- 唯一的改写点：`:165/:178`（accept）与 `:317/:326`（failure），都要求 `state='running'`。
- 兜底也救不了：`crates/intelligence/src/comment_research_kernel.rs:1195` `fail_active_runs_without_embedding_config` 只把 `state='pending'` 置为 `failed`。
- 后果落在 Run 上：`comment_research_kernel.rs:1059-1072` 统计 `unassigned_atoms` / `embedding_failed_atoms`（后者带 `AND NOT EXISTS(… state IN ('pending','running'))`），`:1126` 于是永久提前返回，Run 永远 `running`；`settle_run_completion`（`:1108`）是唯一的 Run 终局写入点。
- **对照**：同 worker 的另两条认领协议都有租约与回收——run item 有 `lease_until` + `recover_expired_run_items_in`（迁移 `0067_comment_research_run_item_lease.sql`），问题归并有 `recover_problem_resolution_leases`（`comment_research_worker.rs:1319`，认领处 `:1263`）。

**复现**

```bash
docker exec linggan-intelligence-postgres-1 psql -U linggan_dev_admin -d linggan_intelligence_dev -c "\d linggan_comment_research_atom_embedding"
# 预期：无 lease_until / attempts / next_attempt_at 三列

docker exec linggan-intelligence-postgres-1 psql -U linggan_dev_admin -d linggan_intelligence_dev -t -A -F'|' -c "
SELECT state, count(*) FROM linggan_comment_research_atom_embedding GROUP BY state;"
# 预期: succeeded|47

docker exec linggan-intelligence-postgres-1 psql -U linggan_dev_admin -d linggan_intelligence_dev -t -A -c "
SELECT column_name FROM information_schema.columns
WHERE table_name='linggan_comment_research_run_item'
  AND (column_name LIKE '%lease%' OR column_name LIKE '%attempt%');"
# 预期: attempts / next_attempt_at / lease_until（三条都有）→ 对照出不对称
```

**反证条件**

- 存在我没找到的 `running → pending/failed` 的回收路径（例如某个定时任务直接 `UPDATE … WHERE state='running'`）。请 `grep -rn "atom_embedding" crates apps` 穷举所有写点。
- `NOT EXISTS(prior…)` 的含义与我理解的不同（例如它带时间窗或只比 `input_hash`）——**这一条请重点核**，它决定了卡住后是否真的永不重建。

---

### B2 · `release_work_order_lease` 只做了释放的前一半

- **一句话**：它只 `UPDATE released_at`，不把工单交还队列，违反同文件自己写下的不变量。
- **确信度**：**高（代码）**。当前全仓唯一调用点是测试 → **埋伏，不是现患**。

**定位**

- `crates/evidence/src/work_order_lease.rs:323-343` — `pub async fn release_work_order_lease`，只执行 `UPDATE collection_work_order_lease SET released_at=…`。
- `crates/evidence/src/work_order_lease.rs:572-583` — 被违反的不变量原话：「释放一份租约之后，把它的工单交还给共享队列。**每一条释放租约的路径都必须走这里。**……只做前一半，工单就永远停在 `leased`、名下却没有活租约」。同段记录了 2026-09-06 工单 `f7942504` 卡死的实测后果。
- 正确实现（对照）：`crates/evidence/src/work_order_lease.rs:588-620` `requeue_work_orders_after_release`。
- 其它释放点都已合规：`execution_station.rs:650`+`:660`（走 requeue）、`dispatch.rs:501`+`:509`（置 `completed`）、`dispatch.rs:580`+`:587`（就地重排队）。
- 调用面：`crates/evidence/src/lib.rs:201` 公开导出；唯一调用点 `crates/evidence/tests/collection_control_postgres.rs:1203`（测试）。API 只有签发路由（`apps/api/src/local_web.rs:260` 路由 / `:3362` 调用），**没有释放路由**。
- 下游后果的依据：派发候选要求 `queue_state='queued'`（`crates/evidence/src/dispatch.rs:1337`/`:1362`），且该工单仍占 `collection_work_order_active_dedupe_idx`（迁移 `0036`，`dedupe_key WHERE queue_state IN ('queued','leased')`）。救回路径是下一台工位 poll 时的 `recover_released_orphaned_work_orders_in_transaction`（`dispatch.rs:649`）。

**复现**

```bash
cd /Users/moglenny/proma/linggan-intelligence
grep -rn 'release_work_order_lease' --include='*.rs' crates apps
# 预期：定义 1 处 + lib.rs 导出 1 处 + tests 调用 1 处；无生产调用
```

**反证条件**

- 有一个我漏掉的调用方在释放后补了 requeue。
- `requeue_work_orders_after_release` 其实在别处被无条件调用（例如每次派发都跑一次全量 requeue）——`dispatch.rs:649` 的孤儿回收确实会覆盖这种情况，**请确认它的条件是否足以覆盖「再也没有工位 poll」之外的场景**。

---

### B3 · run item 的认领缺 attempt 围栏

- **一句话**：`claim` 结构里带着 `attempt`，但写回时五处全部只判 `state='running'`，于是租约到期重领后旧认领者仍能写入。
- **确信度**：**中**。缺围栏已证；**可达性未确证**——需要「第二个认领者」+「模型调用 > 120 秒」同时成立。
- **前置判断**：本仓库内 `claim_next_run_item` 的生产调用者只有那一个顺序执行的 worker 循环（`model_runner.rs:34`），单进程下不触发。**请先证伪这一句**。

**定位**

- 认领：`crates/intelligence/src/comment_research_kernel.rs:861-897`（`:881` 设 `lease_until=+120 seconds`、`attempts=attempts+1`，并把 `attempts` 作为 `claim.attempt` 返回）。
- 到期回收：`crates/intelligence/src/comment_research_kernel.rs:943-975`（`WHERE state='running' AND lease_until<=now()` → `retryable`/`model_failed`，并把该 invocation 标成 `'recovered',true`）。
- 写回路径（**五处，全部只判 `state='running'`**）：`crates/intelligence/src/comment_research_atoms.rs:226`、`:352`、`:437`，`crates/intelligence/src/comment_research_kernel.rs:1001`、`:1026`。
- 对照（有围栏的实现）：`crates/evidence/src/producer_runtime.rs:1093` `assert_attempt_owner`；媒体侧 `claim_generation`。

**复现**

```bash
cd /Users/moglenny/proma/linggan-intelligence
grep -rn '\.attempt\b' --include='*.rs' crates/intelligence/src/ | grep -v 'attempts' | grep -v next_attempt
# 预期：claim.attempt 被赋值的行有，被读取来判断归属的行没有
```

**反证条件**

- 任一处写回其实带了我没看到的归属判断（例如通过 `invocation_ref` 与当前 claim 比对）。**这一条我弱，请重点核**——我只 grep 了 `.attempt`，没穷举 `invocation_ref` 的比对。
- 部署形态保证永远只有一个认领者（需要你确认线上 worker 的进程数与滚动重启行为）。

---

### B4 · 详情补采的候选判据，证据侧不检查「有没有可用链接」

- **一句话**：同一个「还等着补详情」的判据在文件里写了两份，跨行业那份有 `sample.source_url IS NOT NULL`，证据侧那份没有。
- **确信度**：**高（不对称）**。当前 **545/545 全有可用链接 → 无害**，属潜伏。

**定位**

- 跨行业侧（有链接条件）：`crates/evidence/src/keyword_archive_detail.rs:305-310` `macro_rules! pending_detail_sql`，正文第一句即 `"sample.source_url IS NOT NULL AND NOT "`。
- 证据侧（无链接条件，手抄了两遍）：
  - `crates/evidence/src/keyword_archive_detail.rs:182-228` `next_evidence_detail_batch`（单篇挑选）
  - `crates/evidence/src/keyword_archive_detail.rs:268-295`（`keyword_targets_pending_detail` 的 UNION 证据分支，列表页批量）
- 注释与事实不符：`:177` 写「与跨行业那一侧同形」，`:303-304` 的宏文档写「单篇挑选与列表页批量共用它」——**但该宏只服务跨行业侧**。
- 失败链：`crates/evidence/src/dispatch.rs:1406-1428` `execution_source_url_for_task` 要求记录 payload 里存在 `https://www.xiaohongshu.com/…` 且含 `xsec_token=`；取不到则 `ExecutionLocatorUnavailable` 并进入冷却重试（`dispatch.rs:758-785`）。

**复现**

```bash
docker exec linggan-intelligence-postgres-1 psql -U linggan_dev_admin -d linggan_intelligence_dev -t -A -F'|' -c "
WITH accepted AS (
  SELECT f.content_public_ref, f.package_ref, f.record_ordinal
  FROM linggan_material_discovery_finding f
  JOIN linggan_runtime_submission_receipt r ON r.package_ref=f.package_ref
  JOIN linggan_runtime_record_disposition d ON d.package_ref=f.package_ref AND d.record_ordinal=f.record_ordinal
  WHERE f.discovery_kind='discovery_search'
    AND r.material_admission='ACCEPTED' AND d.disposition='accepted_for_library_discovery'
), cal AS (
  SELECT a.content_public_ref, EXISTS (
    SELECT 1 FROM linggan_material_content c
    JOIN linggan_runtime_capture_package p ON p.package_ref=a.package_ref
    CROSS JOIN LATERAL jsonb_array_elements(p.payload->'records') WITH ORDINALITY AS rec(value,ordinality)
    WHERE c.public_ref=a.content_public_ref AND rec.ordinality=a.record_ordinal+1
      AND rec.value->'payload'->>'url' LIKE 'https://www.xiaohongshu.com/%'
      AND position('xsec_token=' IN rec.value->'payload'->>'url')>0) AS ok
  FROM accepted a)
SELECT count(*), count(*) FILTER (WHERE ok), count(*) FILTER (WHERE NOT ok) FROM cal;"
# 预期: 545|545|0
```

**反证条件**

- 证据侧的入料在别处已保证必有链接（例如 `material_admission` 有 url 校验）。已知：`crates/evidence/src/cross_industry_admission.rs:258` 对跨行业侧要求 `payload.url` 以 https 开头，而 `crates/evidence/src/material_admission.rs` **没有**此要求——请确认这一句。

---

### B5 · 「动态巡查节奏」只以测试形态存在，且用的是已被判定恒假的旧词表

- **一句话**：`read_dynamic_cadence_for_rule` 全仓零调用方；它内部的判据用旧词表，对真实数据 100% 不可满足，永远返回 `Unavailable`。它同时是唯一逃过 `check-invariants.sh` INV-1 的判据副本。
- **确信度**：**高**。

**定位**

- `crates/evidence/src/patrol_scheduler.rs:425` — `pub async fn read_dynamic_cadence_for_rule`，约 110 行 SQL，**全仓零调用方**（只有 `crates/evidence/src/lib.rs:161` 导出）。
- `crates/evidence/src/patrol_scheduler.rs:478-479` — 旧判据原文：
  ```sql
  AND COALESCE((layer->>'unknown')::integer,0)=0 \
  AND layer->>'stoppedReason' IN ('surface_ended','maximum_quota') \
  ```
- `crates/evidence/src/directory_boundary.rs:9-18` — 项目自己给出的实测表：`unknown = 0` 对主页发现恒不成立（插件恒报 4）；停止原因要求 `{surface_ended, maximum_quota}` 也对不上（插件报 `surface_read_complete`）。
- `crates/evidence/src/collection_control.rs:2850` — `dynamic_cadence()`，唯一调用方在 `#[cfg(test)] mod tests {`（`:2925` 起，落在 `:3068/:3084/:3121/:3155`）。
- **守卫空档**：`scripts/check-invariants.sh:80` 的正则只查三个宏名，`:44-62` 的 INV-1 字面量只查 `'surfaceReceipt'` 与 `'bottom_confirmed'` 两个词——第 478 行用的是 `layer->>'unknown'`，不在词表里，所以逃过检查。

**复现**

```bash
cd /Users/moglenny/proma/linggan-intelligence
grep -rn 'read_dynamic_cadence_for_rule\|dynamic_cadence' --include='*.rs' crates apps
# 预期：read_dynamic_cadence_for_rule 只有定义 + lib.rs 导出；
#       dynamic_cadence 的调用方全部落在 #[cfg(test)] 区间内

# 判据是否可满足（用全库真实包）
docker exec linggan-intelligence-postgres-1 psql -U linggan_dev_admin -d linggan_intelligence_dev -t -A -F'|' -c "
SELECT count(*) AS total,
       count(*) FILTER (WHERE COALESCE((layer->>'unknown')::integer,0)=0
                          AND layer->>'stoppedReason' IN ('surface_ended','maximum_quota')) AS satisfies
FROM linggan_runtime_capture_package package
CROSS JOIN LATERAL jsonb_array_elements(
  CASE WHEN jsonb_typeof(package.coverage->'layers')='array'
       THEN package.coverage->'layers' ELSE '[]'::jsonb END) layer;"
# 预期: satisfies = 0
```

**反证条件**

- 有我没找到的动态调用（例如通过字符串派发或外部配置选择入口）。
- `coverage->'layers'` 之外的字段里其实带着这两个旧词（我只查了 `layers`）——**请确认 `unknown` 与 `stoppedReason` 是否有其它写入路径会产出旧词**。

---

### B6 · 「哪些迁移构成完整数据库」有 3 份清单，`full_schema_fixture.rs` 短 11 条

- **一句话**：名字叫 FULL 的夹具缺 11 条迁移，其中只有 `0075` 建的对象是活的。
- **确信度**：**高**（三份清单逐条计数 + 差集实算）。

**定位**

- `apps/api/src/local_web/full_schema_fixture.rs` — **71** 条。
- `crates/evidence/tests/support/material_fixture.rs:10-187` — **82** 条（= 磁盘全部）。
- `scripts/local-runtime.sh` — 83 处 `apply_migration_once`（82 调用 + 1 处函数定义）。
- 缺的 11 条：`0049 0051 0053 0054 0055 0056 0057 0058 0059 0060 0075`。
- 唯一活的：`0075_local_embedding_001` 建 `linggan_comment_research_embedding_profile`，被 `crates/intelligence/src/local_embedding_profile.rs:56` 查询（经 `embedding_settings::read` 与 `apps/api/src/local_web/model_settings.rs:161/170/180`、`comment_research.rs:273` 可达）。
- 漂移的 git 证据：`d9f6c78`（新建 0053-0060）、`8cc7b29`（新建 0075），两次都只登记进后两处。

**复现**

```bash
cd /Users/moglenny/proma/linggan-intelligence
grep -o '00[0-9][0-9]_[a-z_0-9]*\.sql' apps/api/src/local_web/full_schema_fixture.rs | sort -u | wc -l   # 71
grep -o '00[0-9][0-9]_[a-z_0-9]*\.sql' crates/evidence/tests/support/material_fixture.rs | sort -u | wc -l  # 82
ls database/migrations/*.sql | wc -l                                                                      # 82
comm -13 <(grep -o '00[0-9][0-9]_[a-z_0-9]*\.sql' apps/api/src/local_web/full_schema_fixture.rs | sort -u) \
         <(ls database/migrations/*.sql | sed 's|.*/||' | sort)
# 预期：上列 11 条
```

**反证条件**

- 另外 10 条建的表确实已被后续迁移 DROP 或改成 VIEW（我按上游 agent 的结论转述，**未逐条核实这 10 条**——请补这一步）。
- `full_schema_fixture.rs` 的用途本就允许子集（名字叫 FULL 与注释是否如此声明，请读文件头）。

---

### B7 · 同一条规则 Rust 写一遍、SQL 手抄一遍（4 处，今天等价）

- **一句话**：四处同一判断两处实现，今天答案完全一致，漂了不会报错。
- **确信度**：**高（重复与等价性）**；严重度按各处单独判断。

**定位**

| # | 家 A（程序） | 家 B（SQL 手抄） | 今天 | 漂移后果 |
|---|---|---|---|---|
| 1 | `crates/intelligence/src/comment_research_kernel.rs:784-797` `research_fingerprint`（写入：`:494`） | `comment_research_kernel.rs:813-828`（`select_eligible_derivations` 的 `NOT EXISTS`） | 等价（`content_hash` 为小写 hex sha256，字段序与 `concat_ws` 参数一一对应） | `NOT EXISTS` 匹配不到任何东西 → **每轮把所有 derivation 全量重新研究**，静默无报错 |
| 2 | `crates/intelligence/src/comment_research_results.rs:228-245`（`meets_partial_threshold` + `percentage_at_least`，常量在 `:18-19`） | `crates/intelligence/src/comment_research_worker.rs:773-777`（选 run 的 SQL） | 等价（共用常量，零分母处理一致） | 真正选 run 的是 SQL——不一致会让 worker 选中一个随后被拒的 run（`Ok(false)`），该 run 因 `ORDER BY run.finished_at` 永远排队首，**后续 run 全堵住** |
| 3 | `crates/evidence/src/dispatch.rs:490-495`（谓词），收尾 `:499-511`，**写死 `release_reason='partial'`** | `crates/evidence/src/work_order_lease.rs:373-379`（逐字节相同的谓词），收尾 `:392-401`，按 `has_non_completed_terminal`（`:384-389`）在 `'completed'`/`'partial'` 间选 | 等价（A 只在终态失败路径触发，写 `partial` 碰巧总对） | 加一个终态词要同时改多处 |
| 4 | 见 B6 | 见 B6 | 71 vs 82 vs 82 | 已计入 B6 |

- 都在用：1 分别在每次 run-item 写入与每次选择；2 见上表；3 家 A 在 `dispatch.rs:433`，家 B 在 `crates/evidence/src/producer_runtime.rs:925`。

**复现**：逐个打开上表定位，人工核对两侧表达式是否仍逐字段一致。

**反证条件**

- 哪一处其实已经不一致（那就要升级成真缺陷，不是伏笔）。
- 哪一处其实有第三方权威实现（三处共用同一函数），那就不是手抄。

---

### C1 · 两个检查器都是红的，且提交门不调用它们

- **一句话**：结构边界检查报 **53 error / 25 warning**，不变量检查报 **3 error**；而提交时真正跑的 `check-project-governance.sh` 只管目录与索引，**不调用上面任何一个**，且它是绿的。
- **确信度**：**高**（实跑 + 调用链核查）。

**复现**

```bash
cd /Users/moglenny/proma/linggan-intelligence
bash scripts/check-rust-boundaries.sh 2>&1 | tail -3   # failed with 53 error(s) and 25 warning(s)
bash scripts/check-invariants.sh 2>&1 | tail -3        # failed with 3 error(s)
bash scripts/check-project-governance.sh               # passed
grep -n 'check-invariants\|check-rust-boundaries' scripts/check-project-governance.sh
# 预期：无匹配（它不调用）——这是「红着也不拦人」的直接证据
```

**3 条红的逐条定性**（请独立复核这三条定性，它们决定修复顺序）

| INV | 错误原文 | 我的定性 | 依据 |
|---|---|---|---|
| INV-1 | `collection_control.rs 的判据调用点从 2 变成 4` | **真信号 + 噪音** | 真实调用点是 **3** 处（`:2259` / `:2338` / `:2361`），第 4 处是 `:2299` 一行**文档注释**里提到的宏名被 `grep -o` 一起数了进去。真信号 = 有人新增了调用点（`:2259`，在 `keyword_baseline_sql` 宏内）却没更新登记表 |
| INV-2 | `有后续迁移把租约的 expires_at 改回可空` | **误报** | 命中的是 `database/migrations/0064_account_observation_normalization.sql:16`，表为 `platform_observation_account_eligibility_observation`（**观察记录不是租约**），改成可空是该迁移刻意的设计决定（见其 12-14 行注释）。真正的租约表 `collection_work_order_lease` 未被触碰。判据 `scripts/check-invariants.sh:126-128` 是满仓库 grep，过宽 |
| INV-4 | `语料库读模型对「已确认失效」零感知` | **真** | 即 A3 |

**53 条的性质**：里面混着「一个文件导出太多东西」这类评审阈值提示（`runtime_capacity.rs` 16 个、`comment_research_kernel.rs` 16 个、`intelligence/src/lib.rs` 19 个，阈值 15），也混着真问题（7 条是 `apps/api` 的 "contains business SQL"）。**我未逐条定性**——这不在本次范围内，我只确认了它红着且不拦人。

**反证条件**

- `check-project-governance.sh` 通过某种间接方式调用了它们（我 grep 过脚本正文，请用 `bash -x` 再确认一次）。
- 有 CI / git hook 在别处调用它们（已知 `.github/` 下只有 PR 模板与 issue 模板，无 workflows；请确认本机 `.git/hooks/`）。

---

### C2 · 两个守真实事故的测试，没有任何脚本会跑

- **一句话**：25 个集成测试文件里 6 个没被任何脚本按名字跑到；其中 4 个被 `cargo test --workspace` 覆盖，剩下两个既 `#[ignore]` 又需要库，从写下那天起就没响过。
- **确信度**：**高**（脚本与文档全量检索）。

**复现**

```bash
cd /Users/moglenny/proma/linggan-intelligence
grep -ho -- '--test [a-z0-9_]*' scripts/*.sh | awk '{print $2}' | sort -u > /tmp/in_scripts.txt
for f in $(find crates -path '*/tests/*.rs' -not -path '*/tests/support/*' | sed 's|.*/||;s|\.rs$||' | sort -u); do
  grep -qxF "$f" /tmp/in_scripts.txt || echo "$f"
done
# 预期 6 个：capture_contract capture_golden discovery_boundary_contract model_keychain producer_task_risk_policy session_timezone_postgres

grep -n 'cargo test' scripts/verify-development-environment.sh
# 预期：:126 是 cargo test --workspace --locked —— 它覆盖前 4 个纯逻辑测试（非 ignore 的部分）
```

**结论落点**

- `crates/evidence/tests/session_timezone_postgres.rs` — `#[tokio::test]` + `#[ignore = "requires the isolated PostgreSQL 16 proof harness"]`（`:11-13`）。守的是页头时间显示事故（文件头 1-6 行自述：「此前会话是 UTC，而页头写着中国标准时间 UTC+08——一个还有一小时才到的巡检时间会显示成早已过去，且不会有任何报错」）。**全仓检索：没有任何脚本、文档或流水线给出它的运行命令**（只出现在 `docs/progress/2026-09.md:438` 的变更记录里）。
- `crates/intelligence/tests/model_keychain.rs` — 1 个 `#[test]`，`#[ignore]`。只在 `docs/runbooks/model-pi-runtime.md:39` 有一条手工命令。

**反证条件**

- 这两个文件其实被 `cargo test --workspace -- --ignored` 之类的方式覆盖到（我确认过 `verify-development-environment.sh:126` **没有** `--ignored`；请确认没有别的入口）。

---

### C3 · 18 个公开函数没有任何调用方，含整套目标生命周期状态机

- **一句话**：静态扫描发现 18 个零调用方的 `pub` 函数；其中最有代表性的一条是「生产代码里没有任何东西校验生命周期合法迁移」。
- **确信度**：**高（逐条 `rg` 验证过）**。方法限制见文末。

**定位（分 crate）**

- `crates/evidence`（12）：`collection_target.rs:564` `transition_target`、`patrol_scheduler.rs:425` `read_dynamic_cadence_for_rule`、`patrol_scheduler.rs:564` `set_monitoring_for_many`、`patrol_scheduler.rs:621` `target_monitoring_enabled`、`patrol_scheduler.rs:636` `set_target_monitoring`（`interval_seconds` 被 `let _ = interval_seconds;` 丢弃）、`acquisition_chain.rs:529` `request_admit_material_targets_and_lease`、`acquisition_chain.rs:627` `request_and_admit_material_targets_under_authorization`、`ingress/mod.rs:139` `ingest_capture_package`、`runtime_capacity.rs:56` `LaneVerdict::verdict_label`、`runtime_capacity.rs:119` `PlatformDispatchCapacity::remaining`、`runtime_capacity.rs:173` `RuntimeCapacityOverview::any_lane_available`、`runtime_capacity.rs:181` `RuntimeCapacityOverview::blocking_reasons`。
- `crates/intelligence`（4）：`model_runner.rs:30` `run_model_worker`、`model_invocation.rs:277` `checkpoint_invocation_usage`、`pi_adapter.rs:178` `WeMMResponse::single_document_values`、`embedding_settings.rs:114` `active_config`。
- `crates/contracts`（2）：`capture/target.rs:41` `KnownTargetResult::target_ordinal`、`discovery/query.rs:63` `EvidenceQuery::scope`（它返回的 `EvidenceQueryScope`，`:222`，全仓无构造点与匹配点，只被反序列化）。

**最重的链**

```text
LifecycleState::may_move_to_for (crates/contracts/src/collection.rs:86)
  ├── may_move_to (collection.rs:79) ── 唯一调用方在 #[cfg(test)] (collection.rs:254-264)
  └── transition_target (crates/evidence/src/collection_target.rs:564) ── 死
→ 生产代码里没有任何东西校验生命周期合法迁移；
  活代码是裸 SQL 直接写 lifecycle_state：
  work_order_lease.rs:467、patrol_scheduler.rs:576/588/646、collection_control.rs:1916
```

**`#[allow(dead_code)]` 逐条判定**（共 4 处 `allow(dead_code)`，另有 1 处 `cfg_attr`）

- `crates/intelligence/src/embedding_settings.rs:19` — **真死代码**。`ready_in_transaction`（`:20`）全仓零调用方；`comment_research_worker.rs:949/1044` 与 `comment_research_kernel.rs:432` 调的是**另一个** `local_embedding_profile::ready_in_transaction`。`git log -S` 显示该 attribute 由 `8cc7b29`（"add local WeMM embedding profile"，即取代它的那个提交）加上。
- `crates/evidence/tests/target_inspector_postgres.rs:1`、`crates/evidence/tests/observation_target_dossier_postgres.rs:1` — 正当（`#[path]` 共享 fixture）。
- `crates/evidence/tests/support/material_fixture.rs:231` `submit_custom_package` — 正当但原因特殊：该函数有 29 个真实调用点，attribute 存在是因为另有 7 个测试文件也 `#[path]` 包含同一模块且不调用它，每个包含点独立编译。
- `apps/api/src/local_web/target_drawer.rs:878` `#[cfg_attr(not(test), allow(dead_code))]` — 有注释说明测试专用，正当。

**复现**：对每个名字 `grep -rn '<name>' --include='*.rs' crates apps`，确认除定义与 `lib.rs` 再导出外没有调用点。

**注意**：`crates/evidence/src/lib.rs:46,48` 与相邻 `pub use` 块再导出了其中若干（`transition_target`、`ingest_capture_package`、四个 `patrol_scheduler` 辅助、两个 `acquisition_chain` 包装），所以对有些名字做朴素「有没有被引用」grep 会返回非空。

**反证条件**

- 某个名字通过 trait 方法、宏展开或字符串派发被调用（静态 grep 看不到）。
- `pub use` 是给**下游 crate** 用的公开 API 表面（本项目是 workspace 内闭环，请确认没有外部消费者）。

---

## 3 · 已排除项（**不要重复上报**）

这些都是本轮查过、判定**不成立或不算缺陷**的：

| 项 | 判定 | 依据 |
|---|---|---|
| INV-2 的「租约 expires_at 被改回可空」 | 误报 | 命中的是观察记录表，见 C1 |
| 模型语义就绪判据的「三个家」 | **等价且失败模式不可达** | `model_settings_read.rs:62-70` 虽漏 `invocation.connection_version_ref=version.version_ref` 过滤，但 `linggan_model_entry` 有 raise-only 触发器（`database/migrations/0040_model_pi.sql:90` + `0004_plugin_runtime_all_capabilities.sql:172-174`），该列终生不可改；且 `model_invocation.rs:76-86` 写入探针时校验 `(model_ref, connection_version_ref)` 成对存在 |
| `dispatch.rs` 的 `release_reason='partial'` 违反 CHECK | 不成立 | 迁移 `0045_deep_archive_recovery.sql:29` 已把 `partial` 加进允许集合 |
| `dispatch.rs:590` 的 `retry_not_before_at` 用直接赋值而非 `GREATEST` | 不成立 | 退避秒数随 `dispatch_failure_count` 单调不减（`:598-601`），且只在上一个截止时刻之后才可能再次失败，构造不出「把重试提前」 |
| 「同一个工单被派两次」 | 未发现 | 四层叠加：`collection_dispatch_lane_fairness … FOR UPDATE` 全库串行化（`dispatch.rs:818` 起）、候选 `FOR UPDATE OF work_order SKIP LOCKED`（`:1337`/`:1362`）、`queue_state='queued'→'leased'` 以 `rows_affected==1` 为闸（`work_order_lease.rs:302-318`）、`collection_work_order_lease_live_idx`（0010）+ `collection_work_order_active_dedupe_idx`（0036） |
| 工位/安装顶替与媒体租约 | 未发现 | `execution_station.rs:618/232/594` 释放时同时 requeue；`media_acquisition.rs:298`、`material_processing.rs:49` 都有 `lease_expires_at` + 尝试计数 + 认领头回收 |
| 「过期租约把队列堵死」 | 未发现新的永久路径 | `decide_dispatch` 在任何闸门之前、同一事务里先跑过期回收（`dispatch.rs:645`）再跑孤儿回收（`:649`），之后每条 return 都显式 commit（`:658-661` 注明「return 而不 commit 会把回收悄悄滚回去」）；额度与在途判定一律带 `expires_at>scope_001_now()` |
| `target_inspector.rs:314` 自己做 `works - details - retired` 减法 | 无可观察偏差 | 三个数与投影定义互斥（`archive_completeness.rs:305-308`），结果恒等于 `pending_details` |
| `material_contract_validation.rs:19-23` 称重复 `@>` 已删除 | 成立 | 核对 `creator_lifecycle.rs:516-522`，确已删净 |
| `station_read.rs`「配额的权威实现只有这一个函数」 | 成立 | `DAILY_NOTE_USAGE_SQL` 单一定义，3 处准入 + 1 处展示共用 |
| `work_order_lease.rs:745-756` `SAMPLING_DIRECTIVE_KEYS`「唯一真源」 | 成立 | 两侧列表 + 守门测试 `sampling_directives_are_all_declared_exempt`（`:1599-1632`）会拦漏改 |
| 重复的 `to_regclass(...)` 探针（5 组） | 刻意 | 每处带「本工位只装了哪段 schema」的理由，逐点探测 |
| `crates/domain`、`crates/observation` 是空壳 | 有意占位 | SCOPE-001 bootstrap |
| 依赖重复 7 组 | 上游分裂，非仓库自造 | 见下 |

**依赖卫生结论（供核实者直接引用）**：`cargo tree -d` 共 14 条重复条目 = 7 个 crate 各两版（`block-buffer` `cpufeatures` `crypto-common` `digest` `hashbrown` `sha2` `syn`），全部是上游/传递分裂。`sha2 0.10.9` ← `sqlx-core`，`sha2 0.11.0` ← 仓库 5 处直接声明**以及** `sqlx-postgres v0.9.0`——是 sqlx 0.9 内部的分裂。无 `*` 通配版本，`default-features = false` 与实际用法一致，`unicode-properties = "=0.1.4"` 是精确锁定。**唯一小毛病**：`serde`（4 处）与 `sha2`（5 处）没有进 `[workspace.dependencies]`，版本号散在字面量里；`crates/contracts/Cargo.toml:12-17` 自己重写了 `serde_json`/`thiserror`/`uuid` 的版本与 features。

---

## 4 · 本次未覆盖

1. **只读**：未改动任何代码、未删除任何文件、未推送。
2. 数据库结论来自**本机开发库副本**，不是 Mac mini 线上库。**任何「线上也这样」的结论都必须重跑。**
3. **二轨制全量扫描没有完成**（并行 agent 跑到中途无输出）。`references/current-v2/plugin/source/` 468 文件中有 320 个与 `plugins/linggan-intelligence-browser/` 同名且已漂移（AGENTS.md 差 179 行、package.json 25 行、manifest.json 18 行）——这一条沿用第六层结论，**未在本轮重跑**。
4. **53 条结构边界错误未逐条定性。**
5. **未做**：函数长度（60 行警告 / 100 行硬限，走 `clippy::too_many_lines`）、SQLx/Axum/CLI 类型不跨深层模块公开接口、异常需 ADR——这三项 `check-rust-boundaries.sh` 自己声明「not automated by this script, and therefore NOT VERIFIED here」。
6. **死代码扫描的方法限制**：按名字扫描，未做完整传递闭包。一个死函数体内调用的另一个函数会被误判成「活的」。已识别的这类链只有 `may_move_to_for` 一条。

---

## 5 · 建议的核实顺序

如果要省时间，按这个顺序核，前三条是承重的：

1. **A1** —— 一条 SQL 就能证伪或证实，且后果最大。
2. **C1 的三条定性** —— 它决定「哪些红要修、哪些是误报」，进而决定后续所有修复的优先级。
3. **B3 与 B1 的可达性** —— 这两条目前标着「未确证」，需要你判断线上部署形态（几个 worker、是否滚动重启）。
4. A2 / A3 / A4 —— 纯代码判断，不需要库。
5. B2 / B4 / B6 / B7 / C2 / C3 —— 结构性，无时间压力。
