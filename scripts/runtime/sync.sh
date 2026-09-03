#!/bin/zsh
# 本机常驻服务的同步步骤。三个服务（API / 巡检 worker / 媒体 worker）启动前都先跑这一段，
# 因此它们永远跑同一个 revision。
#
# 取代了按 commit 冻结快照的旧做法：那个模式累积到 28 份共 21GB，而且三个服务各自指向
# 自己那次部署的快照——2026-09-03 实测 API 在 ccf7cca、两个 worker 还停在 7a792c9，
# 同一台机器上跑着两个版本的代码。
#
# 整段包在大括号里：zsh 逐行读取脚本文件，而这个脚本会 reset --hard 覆盖包括它自己在内的
# 工作树。大括号让 zsh 先完整解析再执行，文件在执行中被替换也不会读到半新半旧的内容。
{
set -euo pipefail

runtime_main="${LINGGAN_RUNTIME_DIR:?LINGGAN_RUNTIME_DIR 未设置}"
lock_file="${runtime_main}/.sync.lock"

log() { print -r -- "[sync] $*"; }

cd "$runtime_main"

# 上一次同步崩溃会把锁留在原地。超过 5 分钟的锁一定是死的——正常同步只要几秒。
if [[ -d "$lock_file" ]] && [[ -z "$(find "$lock_file" -maxdepth 0 -mmin -5 2>/dev/null)" ]]; then
  log "清理超过 5 分钟的残留锁"
  rmdir "$lock_file" 2>/dev/null || true
fi

# 三个服务同时启动时必须**等**，不能跳过。跳过会让服务在别人 cargo 正链接到一半时去 exec
# 二进制——那一瞬间文件不存在，三个服务会一起以 78 退出。等到锁释放时构建已由持锁者完成。
waited=0
while ! mkdir "$lock_file" 2>/dev/null; do
  waited=$((waited + 1))
  if (( waited > 600 )); then
    log "等待同步锁超过 10 分钟，放弃"
    exit 1
  fi
  sleep 1
done
trap 'rmdir "$lock_file" 2>/dev/null || true' EXIT

previous="$(git rev-parse --short HEAD)"

# 拿不到网络时不要让服务起不来：继续用现有 revision，并把这件事说出来。
if git fetch --quiet origin main 2>/dev/null; then
  git reset --quiet --hard origin/main
  current="$(git rev-parse --short HEAD)"
  [[ "$previous" == "$current" ]] && log "已是最新 origin/main（${current}）" \
                                  || log "同步 ${previous} → ${current}"
else
  log "fetch 失败（离线？），继续使用当前 revision ${previous}"
  current="$previous"
fi

# 迁移不由服务执行——一个开机自启的服务不该顺手改数据库结构。但代码现在会自动跟到 main，
# 因此必须主动检查：否则新代码撞上没跑的迁移，报出来的是一堆 SQL 错误而不是「该迁移了」。
set -a
source ./.env
set +a

pending=()
if applied="$(docker exec -e PGPASSWORD="$POSTGRES_PASSWORD" \
      "${LINGGAN_POSTGRES_CONTAINER:-linggan-intelligence-postgres-1}" \
      psql -X -At -U "$POSTGRES_USER" -d "$POSTGRES_DB" \
      -c 'SELECT migration_id FROM linggan_local_schema_migration' 2>/dev/null)"; then
  # 循环变量不能叫 path：zsh 的 $path 与 $PATH 绑定，赋值会把整个 PATH 冲掉，表现是后面
  # 每个外部命令都 "command not found"，而迁移检查会把全部迁移误判为未应用。
  for migration_file in database/migrations/*.sql; do
    id="${migration_file:t:r}"
    print -r -- "$applied" | grep -qxF "$id" || pending+=("$id")
  done
  if (( ${#pending} > 0 )); then
    log "拒绝启动：有 ${#pending} 个迁移未应用 —— ${pending[*]}"
    log "请先执行：./scripts/local-runtime.sh migrate"
    exit 1
  fi
  log "迁移台账已是最新"
else
  # 读不到台账与「没有待应用迁移」不是一回事。开机时数据库常常还没起来，硬拦会让服务
  # 永远起不来，因此这里放行——代价是开机首次启动不保证迁移检查生效。
  log "读不到迁移台账（数据库未启动？），跳过迁移检查"
fi

# 构建在锁内做一次，三个二进制一起。放在各自的启动脚本里会让三个 cargo 争同一个 target
# 目录：其中一个正在链接、二进制被临时移除时，另一个恰好 exec 它就会失败。
log "构建 revision ${current}"
cargo build --quiet --bin linggan-api --bin linggan-worker --bin linggan-media-worker

for binary in linggan-api linggan-worker linggan-media-worker; do
  [[ -x "target/debug/${binary}" ]] || { log "构建后仍找不到 target/debug/${binary}"; exit 1; }
done

log "revision ${current} 就绪"
}
