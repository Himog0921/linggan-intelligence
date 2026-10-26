# Sourced by install/sync. Every database operation here is a fixed, read-only query.
ai_guard="$guard_source_root/scripts/runtime/ai_guard.py"
ai_label="com.linggan-intelligence.ai-executor"

runtime_sql() (
  set -a
  source "$runtime_env"
  set +a
  PGOPTIONS="-c default_transaction_read_only=on -c statement_timeout=5000 -c lock_timeout=5000" \
    PGCONNECT_TIMEOUT=5 PGPASSWORD="$POSTGRES_PASSWORD" \
    docker exec --env PGPASSWORD --env PGOPTIONS --env PGCONNECT_TIMEOUT \
    "${LINGGAN_POSTGRES_CONTAINER:-linggan-intelligence-postgres-1}" \
    psql -X -qAt -v ON_ERROR_STOP=1 -U "$POSTGRES_USER" -d "$POSTGRES_DB" -c "$1" 2>/dev/null
)

runtime_ai_snapshot() {
  local presence snapshot_sql
  presence="$(runtime_sql "SELECT (to_regclass('linggan_ai_executor_current') IS NOT NULL)::int::text||','||(to_regclass('linggan_model_invocation') IS NOT NULL)::int::text")" \
    || { print -r -- "database_snapshot_unavailable；未继续切换代码" >&2; return 1; }
  snapshot_sql="$(python3 "$ai_guard" snapshot-query "$presence")" || return 1
  runtime_sql "$snapshot_sql" || { print -r -- "database_snapshot_unavailable；未继续切换代码" >&2; return 1; }
}

runtime_no_binary_processes() {
  local binary result sockets runtime_port
  for binary in linggan-api linggan-worker linggan-media-worker linggan-ai-worker; do
    if pgrep -x "$binary" >/dev/null 2>&1; then
      print -r -- "$binary 仍在运行；未继续切换代码" >&2
      return 1
    else
      result=$?
      (( result == 1 )) || return 1
    fi
  done
  runtime_port="$(source "$runtime_env"; print -r -- "${LINGGAN_LOCAL_PORT:-3000}")" || return 1
  [[ "$runtime_port" =~ '^[1-9][0-9]{0,4}$' ]] && (( runtime_port <= 65535 )) || return 1
  if sockets="$(lsof -nP -iTCP:${runtime_port} -sTCP:LISTEN 2>&1)"; then return 1; else
    result=$?
    [[ "$result" == "1" && -z "$sockets" ]]
  fi
}

runtime_capture_disabled() {
  local raw="$drain_dir/disabled-before.txt"
  launchctl print-disabled "gui/$(id -u)" > "$raw" || return 1
  python3 "$ai_guard" disabled "$raw" "$disabled_snapshot"
}

runtime_stop_service() {
  local label="$1" inspection
  if inspection="$(launchctl print "gui/$(id -u)/$label" 2>&1)"; then
    launchctl bootout "gui/$(id -u)/$label"
  elif [[ "$inspection" != *"Could not find service \"$label\" "* ]]; then
    print -r -- "无法确认 $label 状态；未继续切换代码" >&2
    return 1
  fi
}

runtime_stop_all() {
  local label inspection busy deadline=$((SECONDS + 30))
  for label in com.linggan-intelligence.local-runtime com.linggan-intelligence.patrol-worker \
    com.linggan-intelligence.media-worker "$ai_label"; do
    runtime_stop_service "$label" || return 1
  done
  while (( SECONDS < deadline )); do
    busy=0
    for label in com.linggan-intelligence.local-runtime com.linggan-intelligence.patrol-worker \
      com.linggan-intelligence.media-worker "$ai_label"; do
      if inspection="$(launchctl print "gui/$(id -u)/$label" 2>&1)"; then
        busy=1
      elif [[ "$inspection" != *"Could not find service \"$label\" "* ]]; then
        return 1
      fi
    done
    if (( ! busy )) && runtime_no_binary_processes; then return 0; fi
    sleep 1
  done
  print -r -- "运行服务尚未全部退出；未继续切换代码或迁移" >&2
  return 1
}

runtime_drain_ai() {
  local before="$1" inspection pid snapshot deadline=$((SECONDS + drain_grace_seconds))
  if inspection="$(launchctl print "gui/$(id -u)/$ai_label" 2>&1)"; then
    pid="$(print -r -- "$inspection" | awk '$1 == "pid" { print $3; exit }')"
  elif [[ "$inspection" == *"Could not find service \"$ai_label\" "* ]]; then
    pid=""
  else
    print -r -- "无法读取 AI 服务状态；未继续切换代码" >&2
    return 1
  fi
  if [[ -z "$pid" ]]; then
    if pgrep -x linggan-ai-worker >/dev/null 2>&1; then
      print -r -- "存在未归属此label的AI进程；未干预该进程或切换代码" >&2
      return 1
    else
      [[ "$?" == "1" ]] || return 1
    fi
    snapshot="$(runtime_ai_snapshot)" || return 1
    print -r -- "$snapshot" | python3 "$ai_guard" no-ai > "$ai_drain_proof"
    return
  fi
  [[ "$pid" =~ '^[1-9][0-9]*$' ]] || return 1
  rm -f -- "$ai_drain_ack"
  log "请求 AI 执行器（pid $pid）排空；上限 ${drain_grace_seconds}s"
  launchctl bootout "gui/$(id -u)/$ai_label" || return 1
  while (( SECONDS < deadline )); do
    if [[ -f "$ai_drain_ack" ]] && python3 "$ai_guard" process-gone "$pid" 2>/dev/null; then
      snapshot="$(runtime_ai_snapshot)" || return 1
      print -r -- "$snapshot" | python3 "$ai_guard" check-ack "$ai_drain_ack" "$pid" "$before" > "$ai_drain_proof" || return 1
      return 0
    fi
    sleep 1
  done
  print -r -- "AI执行器未在共同宽限内确认DB与文件排空；未切换代码" >&2
  return 1
}

runtime_write_permit() {
  local before="$1" after="$2" patrol="$3" snapshot
  runtime_no_binary_processes || return 1
  snapshot="$(runtime_ai_snapshot)" || return 1
  print -r -- "$snapshot" | python3 "$ai_guard" permit "$worker_update_permit" \
    "$before" "$after" "$patrol" "$ai_drain_proof" "$disabled_snapshot"
}

runtime_check_permit() {
  local before="$1" after="$2" snapshot
  runtime_no_binary_processes || return 1
  snapshot="$(runtime_ai_snapshot)" || return 1
  print -r -- "$snapshot" | python3 "$ai_guard" check-permit "$worker_update_permit" "$before" "$after"
}
