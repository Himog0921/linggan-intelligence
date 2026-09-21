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
support_dir="${LINGGAN_SUPPORT_DIR:-$HOME/Library/Application Support/Linggan Intelligence}"
worker_update_permit="${LINGGAN_WORKER_UPDATE_PERMIT_PATH:-$support_dir/runtime-drain/worker-update-permit}"

log() { print -r -- "[sync] $*"; }

has_worker_update_permit() {
  local expected_from="$1" expected_target="$2" state from target
  [[ -f "$worker_update_permit" ]] || return 1
  state="$(awk -F= '$1 == "state" { print $2; exit }' "$worker_update_permit")"
  from="$(awk -F= '$1 == "from" { print $2; exit }' "$worker_update_permit")"
  target="$(awk -F= '$1 == "target" { print $2; exit }' "$worker_update_permit")"
  [[ "$from" == "$expected_from" && "$target" == "$expected_target" \
    && ( "$state" == "drained" || "$state" == "no_worker" || "$state" == "no_inflight_bootstrap" ) ]]
}

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
  target="$(git rev-parse origin/main)"
  current_full="$(git rev-parse HEAD)"
  if [[ "$current_full" != "$target" ]] && ! has_worker_update_permit "$current_full" "$target"; then
    log "拒绝切换 ${current_full:0:7} → ${target:0:7}：没有匹配的 worker drain 回执"
    exit 1
  fi
  git reset --quiet --hard "$target"
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
migration_head=""
# 按 id 排序读：下面的循环只关心「某个 id 在不在」，排序是为了让**最后一行就是台账头**
# （写进 runtime-identity.json 的那个值）。迁移 id 是零填充的（`0098`），字典序即顺序。
if applied="$(docker exec -e PGPASSWORD="$POSTGRES_PASSWORD" \
      "${LINGGAN_POSTGRES_CONTAINER:-linggan-intelligence-postgres-1}" \
      psql -X -At -U "$POSTGRES_USER" -d "$POSTGRES_DB" \
      -c 'SELECT migration_id FROM linggan_local_schema_migration ORDER BY migration_id' 2>/dev/null)"; then
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
  migration_head="$(print -r -- "$applied" | tail -n 1)"
else
  # 读不到台账与「没有待应用迁移」不是一回事。开机时数据库常常还没起来，硬拦会让服务
  # 永远起不来，因此这里放行——代价是开机首次启动不保证迁移检查生效。
  log "读不到迁移台账（数据库未启动？），跳过迁移检查"
fi

# 构建在锁内做一次，三个二进制一起。放在各自的启动脚本里会让三个 cargo 争同一个 target
# 目录：其中一个正在链接、二进制被临时移除时，另一个恰好 exec 它就会失败。
#
# 构建输出另落一份日志，不与服务日志混在一起：从这一版起服务日志里跑的是**结构化事件**
# （一行一个 JSON，见 `crates/evidence/src/runtime_event.rs`），一行 cargo 警告混进去之后，
# 「哪一行是这次进程说的」就要靠猜。失败时把最后 20 行抄回这里，看服务日志的人不必先去
# 找另一个文件。
build_dir="${support_dir}/runtime-build"
build_log="${build_dir}/sync-build.log"
mkdir -p "$build_dir"
print -r -- "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] build revision ${current}" >> "$build_log"
log "构建 revision ${current}（完整输出：${build_log}）"
if ! cargo build --quiet --bin linggan-api --bin linggan-worker --bin linggan-media-worker \
     >>"$build_log" 2>&1; then
  log "构建失败；最近 20 行如下，完整输出见 ${build_log}"
  tail -n 20 "$build_log" | while IFS= read -r line; do log "  ${line}"; done
  exit 1
fi

for binary in linggan-api linggan-worker linggan-media-worker; do
  [[ -x "target/debug/${binary}" ]] || { log "构建后仍找不到 target/debug/${binary}"; exit 1; }
done

# The independent comment loop runs inside linggan-worker. It needs the real SDK even
# when launchd has no nvm shell initialization. This installs only fixed local packages.
./scripts/runtime/prepare-pi-adapter.sh --install

# 这次部署是谁：写一份身份文件，`launch.sh` 把路径导出给三个服务，进程只读它（见
# `crates/evidence/src/runtime_event.rs`）。此前日志里没有 revision，一次故障要说清
# 「是哪个部署干的」只能靠比对启动时刻。
#
# **先写临时文件再改名**：三个服务会同时启动并读这个文件，写一半被读到的话，读到的会是一个
# 残缺的 JSON——进程按约定报 `unknown`，于是「刚部署完却看不出 revision」。同一个目录里的
# 改名是原子的，读到的要么是旧的一份、要么是新的完整一份。
identity_file="${LINGGAN_RUNTIME_IDENTITY_PATH:-$support_dir/runtime-identity.json}"
identity_tmp="${identity_file}.tmp.$$"
mkdir -p "${identity_file:h}"
{
  print -r -- '{'
  print -r -- "  \"revision\": \"${current}\","
  print -r -- "  \"builtAt\": \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\","
  if [[ -n "$migration_head" ]]; then
    print -r -- "  \"migrationHead\": \"${migration_head}\""
  else
    # 读不到台账时如实写 null，不写「大概是最新的」：身份文件是给排查用的，不是给信心用的。
    print -r -- '  "migrationHead": null'
  fi
  print -r -- '}'
} > "$identity_tmp"
mv -f "$identity_tmp" "$identity_file"

log "revision ${current} 就绪（身份文件：${identity_file}）"
}
