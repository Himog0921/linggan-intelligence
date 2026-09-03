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
# 不配置任何远端环境。

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")/../.." && pwd)"
support_dir="${LINGGAN_SUPPORT_DIR:-$HOME/Library/Application Support/Linggan Intelligence}"
runtime_dir="${LINGGAN_RUNTIME_DIR:-$support_dir/runtime-main}"
log_dir="$support_dir/runtime-logs"
agents_dir="$HOME/Library/LaunchAgents"

log() { print -r -- "[install] $*"; }

[[ -f "$repo_root/.env" ]] || { print -r -- "缺少 $repo_root/.env，先照 env.example 建好" >&2; exit 1; }

mkdir -p "$log_dir" "$agents_dir"

# 运行目录是一个 detached worktree，跟随 origin/main。detached 是必须的：main 分支已由
# 开发目录 checkout，同一个分支不能在两处。
git -C "$repo_root" fetch --quiet origin main
if [[ -d "$runtime_dir/.git" ]] || git -C "$repo_root" worktree list --porcelain | grep -qxF "worktree $runtime_dir"; then
  log "运行目录已存在：$runtime_dir"
else
  log "创建运行目录：$runtime_dir"
  git -C "$repo_root" worktree add --detach "$runtime_dir" origin/main
fi

# 装之前必须先把运行目录同步到 origin/main。plist 指向的是运行目录里的 launch.sh，
# 而把它带进来的正是运行目录自己的同步——先写 plist 再指望服务自己同步是循环依赖。
# 首次安装时新建的 worktree 已经在 origin/main 上，这一步是空转；在已有的旧运行目录上
# 重装时它是必需的：2026-09-03 就因为漏了这一步，三个服务以 127 全挂
# （`can't open input file: .../scripts/runtime/launch.sh`）。
log "同步运行目录到 origin/main"
git -C "$runtime_dir" reset --quiet --hard origin/main
[[ -x "$runtime_dir/scripts/runtime/launch.sh" ]] \
  || { print -r -- "同步后仍找不到 $runtime_dir/scripts/runtime/launch.sh" >&2; exit 1; }

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
    [[ -n "$port" ]] && print -r -- "    <key>LINGGAN_LOCAL_PORT</key><string>$port</string>"
    print -r -- '  </dict>'
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
