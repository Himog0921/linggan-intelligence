#!/bin/zsh
set -euo pipefail

# 把本机三个常驻服务装好：建运行目录、写 launchd 配置、启动。
#
#   ./scripts/runtime/install.sh
#
# 幂等：已装好时重跑只会刷新 plist 并重启服务。重装机器或换机器时，这一条命令加上
# 一次 `./scripts/local-runtime.sh migrate` 就是全部步骤。
#
# 它不做的事：不跑迁移（服务不该改数据库结构，见 sync.sh），不装 Docker/Rust/PostgreSQL，
# 不配置任何远端环境。MODEL-PI-001 要求先按 .nvmrc 装 Node；首次同步会按 lock 安装 Pi SDK。

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")/../.." && pwd)"
support_dir="${LINGGAN_SUPPORT_DIR:-$HOME/Library/Application Support/Linggan Intelligence}"
runtime_dir="${LINGGAN_RUNTIME_DIR:-$support_dir/runtime-main}"
log_dir="$support_dir/runtime-logs"
agents_dir="$HOME/Library/LaunchAgents"
drain_dir="$support_dir/runtime-drain"
worker_drain_ack="$drain_dir/worker-drain-ack"
worker_update_permit="$drain_dir/worker-update-permit"
worker_label="com.linggan-intelligence.patrol-worker"
allow_legacy_bootstrap=0

log() { print -r -- "[install] $*"; }

if (( $# > 1 )) || { (( $# == 1 )) && [[ "$1" != "--bootstrap-no-drain" ]]; }; then
  print -r -- "usage: ./scripts/runtime/install.sh [--bootstrap-no-drain]" >&2
  exit 1
fi
(( $# == 0 )) || allow_legacy_bootstrap=1

write_worker_update_permit() {
  local state="$1" from_revision="$2" target_revision="$3"
  mkdir -p "$drain_dir"
  print -r -- "state=$state" > "$worker_update_permit"
  print -r -- "from=$from_revision" >> "$worker_update_permit"
  print -r -- "target=$target_revision" >> "$worker_update_permit"
}

runtime_supports_worker_drain() {
  [[ -f "$runtime_dir/crates/intelligence/src/model_worker_drain.rs" ]] \
    && grep -q 'run_model_worker_with_drain' "$runtime_dir/apps/worker/src/main.rs"
}

no_running_model_invocations() {
  local running
  set -a
  source "$repo_root/.env"
  set +a
  running="$(docker exec -e PGPASSWORD="$POSTGRES_PASSWORD" \
    "${LINGGAN_POSTGRES_CONTAINER:-linggan-intelligence-postgres-1}" \
    psql -X -At -U "$POSTGRES_USER" -d "$POSTGRES_DB" \
    -c "SELECT count(*) FROM linggan_model_invocation WHERE state='running'" 2>/dev/null)" \
    || return 1
  [[ "$running" == "0" ]]
}

request_worker_drain() {
  local from_revision="$1" target_revision="$2" worker_pid ack_pid ack_state waited=0
  worker_pid="$(launchctl print "gui/$(id -u)/$worker_label" 2>/dev/null | awk '$1 == "pid" { print $3; exit }')"
  if [[ ! "$worker_pid" =~ '^[0-9]+$' ]]; then
    log "没有运行中的巡检 worker；记录无需 drain 的更新许可"
    write_worker_update_permit "no_worker" "$from_revision" "$target_revision"
    return
  fi
  if ! runtime_supports_worker_drain; then
    if (( ! allow_legacy_bootstrap )); then
      print -r -- "现有 worker 不支持 drain 回执；未停止服务也未切换 revision。先在已授权维护窗口运行 install.sh --bootstrap-no-drain。" >&2
      exit 1
    fi
    if ! no_running_model_invocations; then
      print -r -- "旧 worker 仍有 running model invocation 或无法核验；未停止服务也未切换 revision" >&2
      exit 1
    fi
    log "旧 worker 已预检无 running invocation；执行一次性 bootstrap 停止，不把它写成 drained"
    launchctl bootout "gui/$(id -u)/$worker_label"
    if ! no_running_model_invocations; then
      print -r -- "旧 worker 停止后仍有 running model invocation；保留未知费用/恢复链，未切换 revision" >&2
      exit 1
    fi
    write_worker_update_permit "no_inflight_bootstrap" "$from_revision" "$target_revision"
    log "bootstrap 已证明无 running model invocation，允许切换但不声明 drain"
    return
  fi
  mkdir -p "$drain_dir"
  rm -f -- "$worker_drain_ack"
  log "请求巡检 worker（pid $worker_pid）drain 后退出"
  if ! launchctl bootout "gui/$(id -u)/$worker_label"; then
    print -r -- "无法请求 $worker_label 停止；未切换运行 revision" >&2
    exit 1
  fi
  while (( waited < 90 )); do
    if [[ -f "$worker_drain_ack" ]]; then
      ack_pid="$(awk -F= '$1 == "pid" { print $2; exit }' "$worker_drain_ack")"
      ack_state="$(awk -F= '$1 == "state" { print $2; exit }' "$worker_drain_ack")"
      if [[ "$ack_pid" == "$worker_pid" && "$ack_state" == "drained" ]]; then
        write_worker_update_permit "drained" "$from_revision" "$target_revision"
        log "巡检 worker 已确认 drain"
        return
      fi
    fi
    sleep 1
    waited=$((waited + 1))
  done
  print -r -- "巡检 worker 未在 90 秒内确认 drain；未切换运行 revision" >&2
  exit 1
}

[[ -f "$repo_root/.env" ]] || { print -r -- "缺少 $repo_root/.env，先照 env.example 建好" >&2; exit 1; }

mkdir -p "$log_dir" "$agents_dir" "$drain_dir"

# 运行目录是一个 detached worktree，跟随 origin/main。detached 是必须的：main 分支已由
# 开发目录 checkout，同一个分支不能在两处。
git -C "$repo_root" fetch --quiet origin main
target_revision="$(git -C "$repo_root" rev-parse origin/main)"
if [[ -d "$runtime_dir/.git" ]] || git -C "$repo_root" worktree list --porcelain | grep -qxF "worktree $runtime_dir"; then
  log "运行目录已存在：$runtime_dir"
else
  log "创建运行目录：$runtime_dir"
  git -C "$repo_root" worktree add --detach "$runtime_dir" origin/main
fi
runtime_revision="$(git -C "$runtime_dir" rev-parse HEAD)"
request_worker_drain "$runtime_revision" "$target_revision"

# 启动时由 sync.sh 消费上面的 target-specific drain permit，再把运行目录同步到
# origin/main。安装入口不能先 reset 再去停 worker，否则旧进程可能在已被替换的工作树上
# 继续运行，且无法证明其在途模型调用已经落账。
[[ -x "$runtime_dir/scripts/runtime/launch.sh" ]] \
  || { print -r -- "运行目录缺少 $runtime_dir/scripts/runtime/launch.sh" >&2; exit 1; }

# .env 不进版本库，因此从开发目录复制一份。两边必须是同一个数据库。
cp "$repo_root/.env" "$runtime_dir/.env"
log "已同步 .env 到运行目录"

write_plist() {
  local label="$1" binary="$2" logname="$3" port="${4:-}"
  local plist="$agents_dir/com.linggan-intelligence.$label.plist"
  {
    print -r -- '<?xml version="1.0" encoding="UTF-8"?>'
    print -r -- '<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">'
    print -r -- '<plist version="1.0"><dict>'
    print -r -- "  <key>Label</key><string>com.linggan-intelligence.$label</string>"
    print -r -- '  <key>ProgramArguments</key><array>'
    print -r -- '    <string>/bin/zsh</string>'
    print -r -- "    <string>$runtime_dir/scripts/runtime/launch.sh</string>"
    print -r -- "    <string>$binary</string>"
    print -r -- '  </array>'
    # WorkingDirectory 必须指向一个真实存在的目录，否则 launchd 在执行脚本**之前**就以
    # EX_CONFIG(78) 失败，而且不产生任何日志——2026-09-03 删旧快照时真实踩过这个坑。
    print -r -- "  <key>WorkingDirectory</key><string>$runtime_dir</string>"
    print -r -- '  <key>EnvironmentVariables</key><dict>'
    print -r -- "    <key>LINGGAN_RUNTIME_DIR</key><string>$runtime_dir</string>"
    print -r -- "    <key>LINGGAN_SUPPORT_DIR</key><string>$support_dir</string>"
    if [[ "$binary" == "linggan-worker" ]]; then
      print -r -- "    <key>LINGGAN_WORKER_DRAIN_ACK_PATH</key><string>$worker_drain_ack</string>"
    fi
    [[ -n "$port" ]] && print -r -- "    <key>LINGGAN_LOCAL_PORT</key><string>$port</string>"
    print -r -- '  </dict>'
    [[ "$binary" == "linggan-worker" ]] && print -r -- '  <key>ExitTimeOut</key><integer>90</integer>'
    print -r -- '  <key>RunAtLoad</key><true/>'
    print -r -- '  <key>KeepAlive</key><true/>'
    print -r -- "  <key>StandardOutPath</key><string>$log_dir/$logname.out.log</string>"
    print -r -- "  <key>StandardErrorPath</key><string>$log_dir/$logname.err.log</string>"
    print -r -- '</dict></plist>'
  } > "$plist"
  plutil -lint "$plist" >/dev/null
  launchctl bootout "gui/$(id -u)/com.linggan-intelligence.$label" 2>/dev/null || true
  launchctl bootstrap "gui/$(id -u)" "$plist"
  log "已安装并启动 com.linggan-intelligence.$label"
}

write_plist local-runtime linggan-api          api           3000
write_plist patrol-worker linggan-worker       worker
write_plist media-worker  linggan-media-worker media-worker

log "完成。三个服务已在跟随 origin/main 运行。"
log "若有未应用的迁移，服务会拒绝启动；届时执行：./scripts/local-runtime.sh migrate"
