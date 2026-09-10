#!/usr/bin/env bash
# 不变量层：把已经踩过的坑固化成「不可能再犯」的自动检查。
#
# 这里每一条都对应一次真实事故，不是设想出来的风险。检查全部是静态的（读代码、读
# migration），不需要数据库——它才能挂在每次提交上跑。需要真实数据库才能证明的行为，
# 本脚本只断言「那个测试还在」，真正的证明由 scripts/test-*-postgres.sh 给出。
#
# 加新检查的规矩：先有一次真实故障，再有这里的一条。
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

errors=0

report_error() {
  echo "invariant error: $1" >&2
  errors=$((errors + 1))
}

# 只数非测试的源码。测试夹具本来就要构造各种形态，拿它当违规是误报。
source_files() {
  grep -rl "$1" crates apps --include='*.rs' 2>/dev/null \
    | grep -v '/tests/' \
    | grep -v '_tests\.rs$' \
    | grep -v '/tests\.rs$' \
    | grep -v '_fixture\.rs$' \
    || true
}

# ---------------------------------------------------------------------------
# INV-1 · 「这一步做成了没有」的判据只有一份定义
#
# 事故：同一个判据被抄成六份且彼此不一致，六份全都永远为假——图妈 201 条作品链接躺在
# 库里没有任何东西会去取，巡检天天跑却四天没被记成成功。1972d86 把它收口成
# crates/evidence/src/directory_boundary.rs 里的三个宏。
#
# 这条守的是「不再出现第七份」：谁想再写一遍判据逻辑，必须先动这里的登记表。
# ---------------------------------------------------------------------------
boundary="crates/evidence/src/directory_boundary.rs"

if [[ ! -f "$boundary" ]]; then
  report_error "判据的唯一定义文件不见了：$boundary"
else
  for macro_name in surface_scan_complete_sql directory_proven_sql profile_read_complete_sql; do
    if ! grep -q "macro_rules! $macro_name" "$boundary"; then
      report_error "INV-1 判据宏 $macro_name 不再定义在 $boundary"
    fi
  done
fi

# 判据内部的原始字面量不许出现在别的源码里——出现即意味着有人在别处重写判据。
for needle in 'surfaceReceipt' 'bottom_confirmed'; do
  while IFS= read -r file; do
    [[ -z "$file" ]] && continue
    [[ "$file" == "$boundary" ]] && continue
    report_error "INV-1 判据字面量 '$needle' 出现在 $file —— 判据只能在 $boundary 里定义，调用方用宏"
  done < <(source_files "$needle")
done

# 调用点登记表。新增调用点必须同时改这里，否则本检查失败。
# 格式：文件:该文件里的宏调用次数
expected_call_sites=(
  "crates/evidence/src/acquisition_chain.rs:2"
  "crates/evidence/src/archive_completeness.rs:2"
  "crates/evidence/src/archive_ledger.rs:2"
  "crates/evidence/src/collection_control.rs:2"
  "crates/evidence/src/creator_lifecycle.rs:1"
  "crates/evidence/src/target_inspector_sql.rs:1"
  "crates/evidence/src/work_order_lease.rs:1"
)
macro_call_re='\(surface_scan_complete_sql\|directory_proven_sql\|profile_read_complete_sql\)!()'

for entry in "${expected_call_sites[@]}"; do
  file="${entry%:*}"
  want="${entry##*:}"
  if [[ ! -f "$file" ]]; then
    report_error "INV-1 登记的判据调用点文件不存在：$file"
    continue
  fi
  got="$(grep -c "$macro_call_re" "$file" || true)"
  if [[ "$got" != "$want" ]]; then
    report_error "INV-1 $file 的判据调用点从 $want 变成 $got —— 改动调用点必须同步更新 scripts/check-invariants.sh 的登记表"
  fi
done

# 反向：登记表之外的文件不许调用判据宏（`use` 那行不算调用）。
while IFS= read -r file; do
  [[ -z "$file" ]] && continue
  [[ "$file" == "$boundary" ]] && continue
  if ! printf '%s\n' "${expected_call_sites[@]}" | grep -q "^$file:"; then
    report_error "INV-1 未登记的判据调用点：$file —— 先在 scripts/check-invariants.sh 登记再用"
  fi
done < <(grep -rl "$macro_call_re" crates apps --include='*.rs' 2>/dev/null || true)

# ---------------------------------------------------------------------------
# INV-2 · 过期的租约必须能被回收，不能把整条队列堵死
#
# 事故：一张过期却没被释放的租约会一直占着工单（live 唯一索引让工单拿不到新租约），
# 整条派发队列停在那里。
#
# 数据库层：租约必须有到期时间，且一张工单同时只能有一份未结束的租约。
# 代码层：认领路径必须先回收过期租约，且释放后必须把工单放回队列。
# ---------------------------------------------------------------------------
lease_migration="database/migrations/0010_work_order_lease.sql"
lease_source="crates/evidence/src/work_order_lease.rs"

if ! grep -q "expires_at timestamptz NOT NULL" "$lease_migration"; then
  report_error "INV-2 租约的 expires_at 不再是 NOT NULL —— 没有到期时间的租约等于永久授权"
fi
if ! grep -q "collection_work_order_lease_live_idx" "$lease_migration"; then
  report_error "INV-2 租约 live 唯一索引不见了 —— 一张工单会被同时派两次"
fi
if ! grep -q "expire_lapsed_leases_in_transaction(&mut transaction)" "$lease_source"; then
  report_error "INV-2 认领路径不再先回收过期租约 —— 过期租约会把队列堵死"
fi
if ! grep -q "requeue_work_orders_after_release" "$lease_source"; then
  report_error "INV-2 释放租约后不再把工单放回队列 —— 工单会永久卡在 leased"
fi
# 回收必须把 released_at 记成 expires_at（真实到期时刻），不是「发现它过期的时刻」。
if ! grep -q "SET released_at = expires_at, release_reason = 'expired'" "$lease_source"; then
  report_error "INV-2 过期回收不再把 released_at 记成真实到期时刻"
fi
if ! grep -rq "expire_lapsed_leases(&database)" crates/evidence/tests/; then
  report_error "INV-2 模拟租约过期的测试不见了（应在 collection_dispatch_sequence_postgres.rs）"
fi

# ---------------------------------------------------------------------------
# INV-3 · 重复任务在数据库层被拦掉，不靠应用逻辑自觉
#
# 应用层的去重只要有一条路径忘了走就失效；这三个唯一索引是最后一道，不许悄悄删。
# ---------------------------------------------------------------------------
declare -a unique_indexes=(
  "database/migrations/0036_monitor_scheduling_clarity.sql:collection_work_order_active_dedupe_idx:同一个 dedupe_key 在排队/已租出时只能有一张工单"
  "database/migrations/0036_monitor_scheduling_clarity.sql:collection_work_order_scheduled_rule_once_idx:同一条规则版本 + 同一个计划时点只能生成一张工单"
  "database/migrations/0010_work_order_lease.sql:collection_work_order_lease_live_idx:一张工单同时只能有一份未结束的租约"
  "database/migrations/0061_material_retirement.sql:collection_material_retirement_once_idx:同一目标下同一篇作品只确认失效一次"
)
for entry in "${unique_indexes[@]}"; do
  file="${entry%%:*}"
  rest="${entry#*:}"
  index="${rest%%:*}"
  why="${rest#*:}"
  if ! grep -q "CREATE UNIQUE INDEX $index" "$file"; then
    report_error "INV-3 唯一索引 ${index} 不见了（${file}）—— ${why}"
  fi
done

# ---------------------------------------------------------------------------
# INV-4 · 「这篇在平台上已经没了」必须传到每一个依赖方
#
# 事故：确认失效之后，七处展示层各自做 works_listed - details_captured，只有一处知道
# 要再减一次已失效的，于是木可可确认了 3 篇已删除，界面照旧催他去补那 3 篇。
#
# 解法是投影出三个互斥的数（已取得／已确认失效／待取得），展示层没有可减的东西。
# 这里把依赖方显式列出来：新增一个读档案完整度的地方，必须登记。
# ---------------------------------------------------------------------------
completeness="crates/evidence/src/archive_completeness.rs"
for field in retired_works pending_details; do
  if ! grep -q "$field" "$completeness"; then
    report_error "INV-4 档案完整度投影不再产出 $field —— 展示层又要自己做减法了"
  fi
done

# 失效结论只能由人下：数据库层写死 decided_by='person'。
if ! grep -q "decided_by text NOT NULL CHECK (decided_by = 'person')" database/migrations/0061_material_retirement.sql; then
  report_error "INV-4 失效结论不再限定由人下 —— 机器自动判定失效这条路必须在数据库层走不通"
fi

# 依赖方登记表：这些地方都必须读投影字段，而不是自己减。
expected_dependents=(
  "crates/evidence/src/target_inspector.rs"
  "crates/evidence/src/target_inspector_sql.rs"
  "apps/api/src/local_web/collection_targets_view.rs"
  "apps/api/src/local_web/target_drawer.rs"
)
for file in "${expected_dependents[@]}"; do
  if [[ ! -f "$file" ]]; then
    report_error "INV-4 登记的依赖方文件不存在：$file"
  elif ! grep -q "retired_works\|pending_details" "$file"; then
    report_error "INV-4 依赖方 $file 不再读失效投影 —— 它要么漏掉了已失效作品，要么自己做了减法"
  fi
done

# 语料库读模型：一篇已被确认失效的作品，在语料库里也该看得出来。
if ! grep -q "retire" apps/api/src/local_web/material_projection.rs; then
  report_error "INV-4 语料库读模型 apps/api/src/local_web/material_projection.rs 对「已确认失效」零感知 —— 同一篇作品在观察目标页显示已失效，在语料库页仍显示为正常材料"
fi

# ---------------------------------------------------------------------------
# INV-5 · 并发正确性用确定性同步证明，不用睡眠
#
# 睡眠证明不了并发正确：机器慢一点它就假绿，机器快一点它就假红。
# 另：macOS 没有 GNU timeout，任何脚本都不许依赖它来诊断卡死。
# ---------------------------------------------------------------------------
# 两种睡眠要分开看：
#
# - **循环里的短退避**（<50ms，反复查一个条件、条件成立就 break）：它等的是条件不是时间，
#   慢机器上只是多转几圈，结论不变。放行。
# - **定位式睡眠**（>=50ms，「条件成立了，再睡一会儿，赌现在正好在执行中」）：它把一个
#   时间点当成了状态。机器快一点，干扰就落在操作**完成之后**——测试照样变绿，但证明的
#   不是它声称的那件事。这是假绿，比红更贵。
#
# 判据用时长，不用上下文：grep 看不出循环结构，而时长恰好把两类分得很干净。
while IFS= read -r hit; do
  [[ -z "$hit" ]] && continue
  millis="$(printf '%s' "$hit" | sed -n 's/.*from_millis(\([0-9]*\)).*/\1/p')"
  [[ -z "$millis" ]] && millis=9999
  if [[ "$millis" -ge 50 ]]; then
    report_error "INV-5 定位式睡眠（${millis}ms）：$hit —— 它赌的是「此刻正好在执行中」，机器快一点干扰就落在操作完成之后，测试会为错误的原因变绿。改用确定性汇合点（barrier/oneshot/tokio::join!）"
  fi
done < <(grep -rn "sleep(" crates/evidence/tests crates/intelligence/tests 2>/dev/null || true)

while IFS= read -r hit; do
  [[ -z "$hit" ]] && continue
  report_error "INV-5 脚本依赖 GNU timeout：$hit —— macOS 上不存在这个命令，它会把「命令找不到」伪装成「超时」"
done < <(grep -rn '^\s*timeout \|[^-a-z_]timeout [0-9]' scripts --include='*.sh' 2>/dev/null || true)

# 并发认领的证明必须还在。
if ! grep -rq "concurrent_station_claims_create_at_most_one_live_lease_for_one_account" crates/evidence/tests/; then
  report_error "INV-5 并发认领只产生一份租约的证明不见了"
fi

if [[ "$errors" -ne 0 ]]; then
  echo "invariant check failed with $errors error(s)" >&2
  exit 1
fi

echo "invariant check passed"
