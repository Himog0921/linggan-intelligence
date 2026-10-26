#!/bin/zsh
set -euo pipefail

# 更新四个服务的配置并保留禁用状态：建运行目录、写 launchd 配置、启动。
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
agents_dir="${LINGGAN_LAUNCH_AGENTS_DIR:-$HOME/Library/LaunchAgents}"
drain_dir="$support_dir/runtime-drain"
worker_drain_ack="$drain_dir/worker-drain-ack"
worker_update_permit="$drain_dir/worker-update-permit"
worker_label="com.linggan-intelligence.patrol-worker"
ai_drain_ack="$drain_dir/ai-drain-ack"
ai_drain_proof="$drain_dir/ai-drain-proof.json"
disabled_snapshot="$drain_dir/disabled-before.json"
patrol_drain_state=""
guard_source_root="$repo_root"
runtime_env="$repo_root/.env"
[[ ! -f "$runtime_dir/.env" ]] || runtime_env="$runtime_dir/.env"
source "$repo_root/scripts/runtime/drain.zsh"
source "$repo_root/scripts/runtime/plist.zsh"
drain_grace_seconds="$(python3 "$ai_guard" grace "$repo_root/crates/contracts/src/ai_limits.json" "$runtime_dir/crates/contracts/src/ai_limits.json")"
allow_legacy_bootstrap=0
prepare_migration=0

log() { print -r -- "[install] $*"; }

if (( $# > 1 )) || { (( $# == 1 )) && [[ "$1" != "--bootstrap-no-drain" && "$1" != "--prepare-migration" ]]; }; then
  print -r -- "usage: ./scripts/runtime/install.sh [--prepare-migration | --bootstrap-no-drain]" >&2
  exit 1
fi
if (( $# == 1 )); then
  if [[ "$1" == "--bootstrap-no-drain" ]]; then
    allow_legacy_bootstrap=1
  else
    prepare_migration=1
  fi
fi

set_patrol_drain_state() {
  patrol_drain_state="$1"
}

runtime_supports_worker_drain() {
  [[ -f "$runtime_dir/apps/worker/src/shutdown.rs" ]] \
    && grep -q 'write_drain_ack' "$runtime_dir/apps/worker/src/shutdown.rs"
}

no_running_model_invocations() {
  local running
  running="$(runtime_sql "SELECT count(*) FROM linggan_model_invocation WHERE state='running'")" || return 1
  [[ "$running" == "0" ]]
}

request_worker_drain() {
  local worker_pid worker_inspection ack_pid ack_state waited=0
  if ! worker_inspection="$(launchctl print "gui/$(id -u)/$worker_label" 2>&1)"; then
    if [[ "$worker_inspection" == *"Could not find service \"$worker_label\" "* ]]; then
      log "没有运行中的巡检worker；记录巡检状态，最终许可仍需四服务及数据库复核"
      set_patrol_drain_state "not_running"
      return
    fi
    print -r -- "无法查询 $worker_label 的 drain 状态；未切换运行 revision" >&2
    print -r -- "$worker_inspection" >&2
    exit 1
  fi
  worker_pid="$(print -r -- "$worker_inspection" | awk '$1 == "pid" { print $3; exit }')"
  if [[ ! "$worker_pid" =~ '^[0-9]+$' ]]; then
    log "没有运行中的巡检worker；记录巡检状态，最终许可仍需四服务及数据库复核"
    set_patrol_drain_state "not_running"
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
    set_patrol_drain_state "no_inflight_bootstrap"
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
  while (( waited < drain_grace_seconds )); do
    if [[ -f "$worker_drain_ack" ]]; then
      ack_pid="$(awk -F= '$1 == "pid" { print $2; exit }' "$worker_drain_ack")"
      ack_state="$(awk -F= '$1 == "state" { print $2; exit }' "$worker_drain_ack")"
      if [[ "$ack_pid" == "$worker_pid" && "$ack_state" == "drained" ]] && python3 "$ai_guard" process-gone "$worker_pid" 2>/dev/null; then
        python3 "$ai_guard" check-patrol "$worker_drain_ack" "$worker_pid" || return 1
        set_patrol_drain_state "drained"
        log "巡检 worker 已确认 drain"
        return
      fi
    fi
    if [[ "${ack_pid:-}" == "$worker_pid" && "${ack_state:-}" != "drained" ]]; then
      print -r -- "巡检worker返回失败收条；未继续切换代码" >&2
      return 1
    fi
    sleep 1
    waited=$((waited + 1))
  done
  print -r -- "巡检 worker 未在共同宽限内确认 drain；未切换运行 revision" >&2
  exit 1
}

[[ -f "$repo_root/.env" ]] || { print -r -- "缺少 $repo_root/.env，先照 env.example 建好" >&2; exit 1; }
source "$repo_root/.env"
runtime_port="${LINGGAN_LOCAL_PORT:-3000}"
[[ "$runtime_port" =~ '^[1-9][0-9]{0,4}$' ]] && (( runtime_port <= 65535 )) \
  || { print -r -- "LINGGAN_LOCAL_PORT must be a valid non-zero TCP port" >&2; exit 1; }

mkdir -p "$log_dir" "$agents_dir" "$drain_dir"
install_lock="$drain_dir/install.lock"
mkdir "$install_lock" 2>/dev/null || { print -r -- "已有安装占用锁；未操作服务" >&2; exit 1; }
trap 'rmdir "$install_lock" 2>/dev/null || true' EXIT
rm -f -- "$worker_update_permit"
runtime_capture_disabled

# 运行目录是一个 detached worktree，跟随 origin/main。detached 是必须的：main 分支已由
# 开发目录 checkout，同一个分支不能在两处。
git -C "$repo_root" fetch --quiet origin main
target_revision="$(git -C "$repo_root" rev-parse origin/main)"
[[ "$(git -C "$repo_root" rev-parse HEAD)" == "$target_revision" ]] \
  || { print -r -- "安装入口必须来自目标origin/main；未操作服务" >&2; exit 1; }
git -C "$repo_root" diff --quiet HEAD -- scripts/runtime crates/contracts/src/ai_limits.json \
  || { print -r -- "安装入口或资源合同存在未提交改动；未操作服务" >&2; exit 1; }
if [[ -e "$runtime_dir" ]]; then
  # Linked worktrees keep .git as a file, so -d "$runtime_dir/.git" would
  # wrongly treat a healthy existing runtime as absent.  Never overwrite an
  # arbitrary existing directory: it must still be a worktree for this origin.
  git -C "$runtime_dir" rev-parse --is-inside-work-tree >/dev/null 2>&1 \
    || { print -r -- "运行目录已存在，但不是 Git 工作树；未覆盖：$runtime_dir" >&2; exit 1; }
  runtime_origin="$(git -C "$runtime_dir" remote get-url origin 2>/dev/null || true)"
  repo_origin="$(git -C "$repo_root" remote get-url origin)"
  [[ "$runtime_origin" == "$repo_origin" ]] \
    || { print -r -- "运行目录不属于当前仓库；未覆盖：$runtime_dir" >&2; exit 1; }
  log "运行目录已存在：$runtime_dir"
else
  log "创建运行目录：$runtime_dir"
  git -C "$repo_root" worktree add --detach "$runtime_dir" origin/main
fi
runtime_revision="$(git -C "$runtime_dir" rev-parse HEAD)"
request_worker_drain
runtime_drain_ai "$runtime_revision"
runtime_stop_all
if (( prepare_migration )); then
  log "巡检/AI已核对排空，四服务已停止；未切换代码或启动服务，可在授权窗口单独迁移。"
  exit 0
fi
runtime_write_permit "$runtime_revision" "$target_revision" "$patrol_drain_state"

# 启动时由新 revision 的 sync.sh 消费上面的 target-specific drain permit，再把运行目录同步到
# origin/main。安装入口不能先 reset 再去停 worker，否则旧进程可能在已被替换的工作树上
# 继续运行，且无法证明其在途模型调用已经落账。
[[ -x "$runtime_dir/scripts/runtime/launch.sh" ]] \
  || { print -r -- "运行目录缺少 $runtime_dir/scripts/runtime/launch.sh" >&2; exit 1; }

# .env 不进版本库，因此从开发目录复制一份。两边必须是同一个数据库；当安装入口
# 本身就在 runtime worktree 中时，源和目标相同，不能让 cp 把这个幂等场景当错误。
if [[ "$repo_root/.env" != "$runtime_dir/.env" ]]; then
  cp "$repo_root/.env" "$runtime_dir/.env"
  log "已同步 .env 到运行目录"
else
  log "运行目录已使用当前 .env"
fi

# The existing runtime may predate a retired binary. Run the target revision's synchronizer from
# the source checkout, not the runtime copy that is about to be replaced.
sync_copy="$(mktemp "${TMPDIR:-/tmp}/linggan-sync.XXXXXX")"
cp "$repo_root/scripts/runtime/sync.sh" "$sync_copy"
LINGGAN_RUNTIME_SOURCE_DIR="$repo_root" LINGGAN_RUNTIME_DIR="$runtime_dir" LINGGAN_SUPPORT_DIR="$support_dir" /bin/zsh "$sync_copy"
rm -f "$sync_copy"

[[ "$(git -C "$runtime_dir" rev-parse HEAD)" == "$target_revision" ]] \
  || { print -r -- "运行目录未同步到目标 revision；未重启服务" >&2; exit 1; }

write_plist local-runtime linggan-api          api           "$runtime_port"
write_plist patrol-worker linggan-worker       worker
write_plist media-worker  linggan-media-worker media-worker
write_plist ai-executor   linggan-ai-worker    ai-worker

log "配置更新完成；已启用服务跟随origin/main，原禁用服务与新AI初始禁用保持。"
log "若有未应用的迁移，服务会拒绝启动；届时执行：./scripts/local-runtime.sh migrate"
