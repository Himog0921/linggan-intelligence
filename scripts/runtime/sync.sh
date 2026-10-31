#!/bin/zsh
# 本机常驻服务的同步步骤。四个服务启动前都先跑这一段，使用同一固定部署目标。
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

guard_source_root="${LINGGAN_RUNTIME_SOURCE_DIR:-$runtime_main}"
runtime_env="$runtime_main/.env"
source "$guard_source_root/scripts/runtime/drain.zsh"

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

# Installation passes a frozen SHA. Restarts reuse the last installed SHA and never fetch main.
# A legacy installation without a pin stays on its current checkout until explicitly installed.
target="${LINGGAN_RUNTIME_TARGET_REVISION:-}"
if [[ -z "$target" ]]; then
  if [[ -e "$support_dir/runtime-target-revision" ]]; then
    target="$(cat "$support_dir/runtime-target-revision")"
  else
    target="$(git rev-parse HEAD)"
  fi
fi
[[ "$target" =~ '^[0-9a-f]{40}$' ]] || { log "invalid installed target SHA; refusing startup"; exit 1; }
git cat-file -e "${target}^{commit}" || { log "installed target commit is unavailable"; exit 1; }
current_full="$(git rev-parse HEAD)"
if [[ "$current_full" != "$target" ]] && ! runtime_check_permit "$current_full" "$target"; then
  log "拒绝切换 ${current_full:0:7} → ${target:0:7}：没有匹配的巡检/AI与数据库排空许可"
  exit 1
fi
if [[ "$current_full" != "$target" ]]; then git reset --quiet --hard "$target"; fi
current="$(git rev-parse --short HEAD)"
[[ "$previous" == "$current" ]] && log "已是固定部署版本（${current}）" \
                                || log "同步 ${previous} → ${current}"

# 迁移不由服务执行；固定部署目标仍必须检查台账，避免新代码撞上未应用的迁移。
set -a
source ./.env
set +a
if [[ -n "${LINGGAN_RUSTUP_BIN_DIR:-}" ]]; then
  [[ -x "$LINGGAN_RUSTUP_BIN_DIR/cargo" ]] \
    || { log "LINGGAN_RUSTUP_BIN_DIR has no cargo executable"; exit 1; }
  export PATH="$LINGGAN_RUSTUP_BIN_DIR:$PATH"
fi

pending=()
migration_head=""
# 按 id 排序读：下面的循环只关心「某个 id 在不在」，排序是为了让**最后一行就是台账头**
# （写进 runtime-identity.json 的那个值）。迁移 id 是零填充的（`0098`），字典序即顺序。
if applied="$(PGPASSWORD="$POSTGRES_PASSWORD" docker exec --env PGPASSWORD \
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
if ! cargo build --quiet --bin linggan-api --bin linggan-worker --bin linggan-media-worker --bin linggan-ai-worker \
     >>"$build_log" 2>&1; then
  log "构建失败；最近 20 行如下，完整输出见 ${build_log}"
  tail -n 20 "$build_log" | while IFS= read -r line; do log "  ${line}"; done
  exit 1
fi

for binary in linggan-api linggan-worker linggan-media-worker linggan-ai-worker; do
  [[ -x "target/debug/${binary}" ]] || { log "构建后仍找不到 target/debug/${binary}"; exit 1; }
done

# 网页前端（WEB-REACT-001 / DEC-0010）与三个二进制同一个 revision：同一把锁内、同一个 checkout
# 里构建，API 读这个 checkout 的 apps/web/dist。已为该 revision 构建过就直接复用，离线重启不需要
# 网络；构建失败与 cargo 失败同样拒绝启动，不让新后台配旧界面半新半旧地运行。
log "准备网页前端 revision ${current}"
if ! ./scripts/runtime/prepare-web.sh --build "$current" >>"$build_log" 2>&1; then
  log "网页前端构建失败；最近 20 行如下，完整输出见 ${build_log}"
  tail -n 20 "$build_log" | while IFS= read -r line; do log "  ${line}"; done
  exit 1
fi

# The independent AI executor needs the pinned SDK even
# when launchd has no nvm shell initialization. This installs only fixed local packages.
./scripts/runtime/prepare-pi-adapter.sh --install

# 这次部署是谁：写一份身份文件，`launch.sh` 把路径导出给三个服务，进程只读它（见
# `crates/evidence/src/runtime_event.rs`）。此前日志里没有 revision，一次故障要说清
# 「是哪个部署干的」只能靠比对启动时刻。
#
# **先写临时文件再改名**：三个服务会同时启动并读这个文件，写一半被读到的话，读到的会是一个
# 残缺的 JSON——进程按约定报 `unknown`，于是「刚部署完却看不出 revision」。同一个目录里的
# 改名是原子的，读到的要么是旧的一份、要么是新的完整一份。
identity_revision="$(git rev-parse HEAD)"
identity_file="${LINGGAN_RUNTIME_IDENTITY_PATH:-$support_dir/runtime-identity.json}"
identity_tmp="${identity_file}.tmp.$$"
mkdir -p "${identity_file:h}"
{
  print -r -- '{'
  print -r -- "  \"revision\": \"${identity_revision}\","
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
