#!/usr/bin/env bash
# WEB-REACT-001 真实链路证明：一次性隔离 PostgreSQL + 本机 Axum 路由 + 已构建的 React 前端
# + 无界面浏览器。依次运行：阶段 B 的设置页证明；阶段 D1 的语料外壳接口读库证明与创作者页证明；
# 阶段 D2 的证据库接口证明（DR-1 失效控件、两条写路径的库内后果）与证据库页证明；阶段 E 的运营台账五页证明
# （合成作者与作品，真实写入只落在隔离库）。不连接开发库或本机常驻服务；只用合成模型凭据，
# 普通页面证明禁用模型进程；做法试跑专项仅使用固定 SDK 和自有回环端点，
# 不连接真实供应商，也不读取共享服务凭据。
set -euo pipefail
project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"
command -v cargo >/dev/null || { echo "Cargo is unavailable" >&2; exit 1; }
docker info >/dev/null 2>&1 || { echo "Docker is unavailable" >&2; exit 1; }

node_bin="${WEB_APP_PROOF_NODE:-$HOME/.nvm/versions/node/v$(cat .nvmrc)/bin/node}"
[[ -x "$node_bin" ]] || node_bin="$(command -v node || true)"
[[ -x "$node_bin" ]] || { echo "Node is unavailable" >&2; exit 1; }
npm_bin="$(dirname "$node_bin")/npm"
[[ -x "$npm_bin" ]] || npm_bin="$(command -v npm)"
export PATH="$(dirname "$node_bin"):$PATH"

# Native Ops proofs exercise only Axum and PostgreSQL.  Do not make that focused backend proof
# depend on a generated browser build (or its unrelated design-system drift); every browser scope
# still builds the current frontend before starting its isolated database.
proof_scope="${WEB_APP_PROOF_SCOPE:-all}"
# This focused proof intentionally exercises a synthetic exact publication through a new shared
# lease.  It must declare the same governance phase as the dedicated Operations proof runner;
# missing phase remains fail-closed for every other invocation.
if [[ "$proof_scope" == "ops-native" ]]; then
  export LINGGAN_COLLECTION_UPGRADE_PHASE=governance
fi
# These focused scopes exercise Axum + isolated PostgreSQL only.  Do not make
# their security/model proofs fail on unrelated generated frontend drift.
if [[ "$proof_scope" != "ops-native" && "$proof_scope" != "ops-auth" ]]; then
  if [[ ! -d apps/web/node_modules ]]; then
    "$npm_bin" ci --prefix apps/web --ignore-scripts --no-audit --no-fund
  fi
  "$npm_bin" --prefix apps/web run build
fi

suffix="$(date -u +%Y%m%d%H%M%S)_$$_$(openssl rand -hex 4)"
proof_database="linggan_web_app_proof_${suffix}"
proof_container="linggan-web-app-proof-${suffix//_/-}"
proof_volume="${proof_container}-data"
proof_user="web_app_proof_admin"
proof_password="$(openssl rand -hex 24)"
postgres_image="linggan-intelligence-postgres-pgvector:16.14-v0.8.0-r1"
[[ "$proof_database" =~ ^linggan_web_app_proof_[a-zA-Z0-9_]+$ ]] || exit 1
[[ "$proof_container" =~ ^linggan-web-app-proof-[a-zA-Z0-9-]+$ ]] || exit 1
proof_dir="${WEB_APP_PROOF_DIR:-${TMPDIR:-/tmp}/linggan-web-app-proof-${suffix}}"
support_dir="$(mktemp -d "${TMPDIR:-/tmp}/linggan-web-app-support.XXXXXX")"

cleanup() {
  local result=$?
  trap - EXIT
  local cleanup_failed=0
  if [[ "${proof_container_attempted:-0}" -eq 1 ]]; then
    local remaining_container
    if remaining_container="$(docker ps -a --filter "name=^/${proof_container}$" --format '{{.Names}}')"; then
      if [[ "$remaining_container" == "$proof_container" ]]; then docker rm -f "$proof_container" >/dev/null 2>&1 || cleanup_failed=1;
      elif [[ -n "$remaining_container" ]]; then cleanup_failed=1; fi
    else cleanup_failed=1; fi
  fi
  if [[ "${proof_volume_created:-0}" -eq 1 ]]; then docker volume rm "$proof_volume" >/dev/null 2>&1 || cleanup_failed=1; fi
  rm -rf "$support_dir" || cleanup_failed=1
  if [[ "$cleanup_failed" -ne 0 ]]; then
    echo "WEB-REACT-001 isolated PostgreSQL proof cleanup failed" >&2
    [[ "$result" -ne 0 ]] || result=1
  else
    echo "WEB-REACT-001 isolated PostgreSQL proof cleanup verified; controlled container, volume and support directory absent"
  fi
  echo "evidence directory (screenshots and report.json): $proof_dir"
  exit "$result"
}
trap cleanup EXIT
"$project_root/scripts/runtime/build-pgvector-image.sh" --ensure
docker volume create "$proof_volume" >/dev/null
proof_volume_created=1
proof_container_attempted=1
docker run -d --name "$proof_container" \
  --mount "type=volume,source=$proof_volume,target=/var/lib/postgresql/data" \
  --env POSTGRES_DB="$proof_database" --env POSTGRES_USER="$proof_user" \
  --env POSTGRES_PASSWORD="$proof_password" --publish 127.0.0.1::5432 \
  "$postgres_image" >/dev/null
for _ in {1..30}; do
  # The image's init server accepts Unix sockets before it is stopped; only the
  # final server listens on TCP, which is the connection path used by the proof.
  docker exec "$proof_container" pg_isready -h 127.0.0.1 -p 5432 -U "$proof_user" -d "$proof_database" >/dev/null 2>&1 && break
  sleep 1
done
if ! docker exec "$proof_container" pg_isready -h 127.0.0.1 -p 5432 -U "$proof_user" -d "$proof_database" >/dev/null 2>&1; then
  echo "WEB-REACT-001 isolated PostgreSQL did not become TCP-ready within 30 seconds" >&2
  # Diagnostic failure must not hide the original readiness failure. Never print Env/DSN.
  docker inspect --format 'container state={{.State.Status}} exit={{.State.ExitCode}}' "$proof_container" >&2 || echo 'container state unavailable' >&2
  docker logs --tail 80 "$proof_container" >&2 || echo 'container startup logs unavailable' >&2
  exit 1
fi
proof_port="$(docker inspect --format '{{(index (index .NetworkSettings.Ports "5432/tcp") 0).HostPort}}' "$proof_container")"
mkdir -p "$proof_dir"

# 在隔离库上运行一个被忽略的证明测试。第一个参数是证据目录，其余是 cargo 的参数。
run_proof() {
  local evidence_dir="$1"
  shift
  local -a proof_environment=(
    "LOCAL_001_PROOF_DATABASE_URL=postgresql://${proof_user}:${proof_password}@127.0.0.1:${proof_port}/${proof_database}"
    "WEB_APP_BROWSER_PROOF=1" "P1_BROWSER_PROOF=1"
    "WEB_APP_PROOF_DIR=$evidence_dir" "WEB_APP_PROOF_NODE=$node_bin"
    "LINGGAN_WEB_DIST=$project_root/apps/web/dist"
    "LINGGAN_MODEL_SYNTHETIC_PREVIEW=SYNTHETIC-NOT-EVIDENCE"
    "LINGGAN_SUPPORT_DIR=$support_dir" "LINGGAN_LOCAL_MEDIA_ROOT=$support_dir/media"
    "LINGGAN_PI_NODE=$support_dir/no-model-process" "RUST_TEST_THREADS=1"
  )
  local selected_tests
  selected_tests="$(env "${proof_environment[@]}" "$@" --list)" || return "$?"
  if ! grep -Eq '^[^[:space:]].*: test$' <<< "$selected_tests"; then
    echo 'Browser proof selected no tests; refusing an empty success' >&2
    return 1
  fi
  env "${proof_environment[@]}" "$@"
}

# Focused local reruns use the same disposable environment; CI defaults to every stage.
if [[ "${WEB_APP_PROOF_SCOPE:-all}" == ops ]]; then
  run_proof "$proof_dir" cargo test -p linggan-api --bins --locked local_web::ops_browser_tests -- --ignored --nocapture --test-threads=1
  exit 0
fi
if [[ "${WEB_APP_PROOF_SCOPE:-all}" == ops-auth ]]; then
  run_proof "$proof_dir" cargo test -p linggan-api --bins --locked local_web::ops_auth_tests -- --ignored --nocapture --test-threads=1
  exit 0
fi
if [[ "${WEB_APP_PROOF_SCOPE:-all}" == ops-native ]]; then
  run_proof "$proof_dir" cargo test -p linggan-api --bins --locked local_web::ops_native_ -- --ignored --nocapture --test-threads=1
  exit 0
fi
if [[ "${WEB_APP_PROOF_SCOPE:-all}" == workbench ]]; then
  mkdir -p "$support_dir/creation-assets"
  run_proof "$proof_dir" env -u CW_BROWSER_REAL_SAMPLES -u CW_BROWSER_MANUAL_REVIEW LINGGAN_CREATION_ASSET_ROOT="$support_dir/creation-assets" CW_BROWSER_PROOF=1 CW_BROWSER_PROOF_DIR="$proof_dir" CW_BROWSER_PROOF_NODE="$node_bin" cargo test -p linggan-api --bins --locked content_workbench_real_chain_browser_proof -- --ignored --nocapture --test-threads=1
  exit 0
fi
if [[ "${WEB_APP_PROOF_SCOPE:-all}" == ai-trials ]]; then
  run_proof "$proof_dir" env LINGGAN_PI_NODE="$node_bin" cargo test -p linggan-api --bins --locked ai_settings_skill_trial_real_chain_browser_proof -- --ignored --nocapture --test-threads=1
  exit 0
fi
if [[ "${WEB_APP_PROOF_SCOPE:-all}" == settings ]]; then
  run_proof "$proof_dir" env LINGGAN_PI_NODE="$node_bin" cargo test -p linggan-api --bins --locked settings_page_real_chain_browser_proof -- --ignored --nocapture --test-threads=1
  exit 0
fi
if [[ "${WEB_APP_PROOF_SCOPE:-all}" == creators ]]; then
  run_proof "$proof_dir" cargo test -p linggan-api --bins --locked local_web::creators_browser_tests -- --ignored --nocapture --test-threads=1
  run_proof "$proof_dir" env LINGGAN_COLLECTION_UPGRADE_PHASE=governance cargo test -p linggan-api --bins --locked local_web::creator_preview_proof_tests -- --ignored --nocapture --test-threads=1
  exit 0
fi
if [[ "${WEB_APP_PROOF_SCOPE:-all}" == topics ]]; then
  mkdir -p "$proof_dir/topics"
  run_proof "$proof_dir/topics" cargo test -p linggan-api --bins --locked local_web::topic_workspace_tests -- --ignored --nocapture --test-threads=1
  run_proof "$proof_dir/topics" cargo test -p linggan-api --bins --locked local_web::topic_browser_tests -- --ignored --nocapture --test-threads=1
  exit 0
fi
if [[ "${WEB_APP_PROOF_SCOPE:-all}" == comments-contracts || "${WEB_APP_PROOF_SCOPE:-all}" == comments || "${WEB_APP_PROOF_SCOPE:-all}" == research ]]; then
  mkdir -p "$proof_dir/research"
  run_proof "$proof_dir/research" env LINGGAN_PI_NODE="$node_bin" cargo test -p linggan-api --bins --locked \
    local_web::research_browser_tests -- --ignored --nocapture --test-threads=1
  exit 0
fi
[[ "${WEB_APP_PROOF_SCOPE:-all}" == all ]] || { echo 'Unknown proof scope' >&2; exit 1; }

# 阶段 B：设置页。
run_proof "$proof_dir" \
  env LINGGAN_PI_NODE="$node_bin" cargo test -p linggan-api --bins --locked \
  local_web::web_app_tests::settings_page_real_chain_browser_proof \
  -- --ignored --nocapture --test-threads=1
printf '%s\n' 'WEB-REACT-001 settings page real-chain proof passed (isolated database; synthetic credential only)'

# AI-GEN1: owned service and synthetic SDK endpoint; deliberately distinct from zero-call settings.
mkdir -p "$proof_dir/ai-trials"
run_proof "$proof_dir/ai-trials" env LINGGAN_PI_NODE="$node_bin" cargo test -p linggan-api --bins --locked ai_settings_skill_trial_real_chain_browser_proof -- --ignored --nocapture --test-threads=1


# 阶段 D1：语料外壳接口读真实库 + 创作者页（证据放在 creators/ 子目录）。
mkdir -p "$proof_dir/creators"
run_proof "$proof_dir/creators" \
  cargo test -p linggan-api --bins --locked \
  local_web::creators_browser_tests \
  -- --ignored --nocapture --test-threads=1
# 创作者主页浅看与精选研究：走真实派发，须与常驻服务一样声明 governance 阶段（其余证明保持原样）。
run_proof "$proof_dir/creators" \
  env LINGGAN_COLLECTION_UPGRADE_PHASE=governance cargo test -p linggan-api --bins --locked \
  local_web::creator_preview_proof_tests \
  -- --ignored --nocapture --test-threads=1
printf '%s\n' 'WEB-REACT-001 D1 corpus shell API and creators page real-chain proofs passed (isolated database; synthetic data only)'

# 阶段 D2：证据库。先是接口证明（失效控件的真实调用记录、两条写路径在库里留下的后果），再是页面证明
# （合成作品含长文＋多图＋评论＋图片文字、视频、风险停止、部分取得、字节已清理、撤回或受限、可复观测；
# 受控媒体字节写在支持目录的 media/ 下，只经本机媒体句柄读取）。证据放在 evidence/ 子目录。
mkdir -p "$proof_dir/evidence" "$support_dir/media"
run_proof "$proof_dir/evidence" \
  cargo test -p linggan-api --bins --locked \
  local_web::evidence_api_proof_tests \
  -- --ignored --nocapture --test-threads=1
run_proof "$proof_dir/evidence" \
  cargo test -p linggan-api --bins --locked \
  local_web::evidence_browser_tests \
  -- --ignored --nocapture --test-threads=1
printf '%s\n' 'WEB-REACT-001 D2 evidence API proofs and evidence page real-chain proof passed (isolated database; synthetic data only)'

# 阶段 E：运营台账五页、目标写入与六种导出；全部使用独立合成数据。
mkdir -p "$proof_dir/ops"
run_proof "$proof_dir/ops" \
  cargo test -p linggan-api --bins --locked \
  local_web::ops_browser_tests \
  -- --ignored --nocapture --test-threads=1
printf '%s\n' 'WEB-REACT-001 E operations five-page real-chain proof passed (isolated database only)'

# 阶段 F：完整主题图谱 + 独立 Workspace；本机真实 UI 对照另存 private，不上传 CI。
mkdir -p "$proof_dir/topics"
run_proof "$proof_dir/topics" cargo test -p linggan-api --bins --locked local_web::topic_workspace_tests -- --ignored --nocapture --test-threads=1
run_proof "$proof_dir/topics" \
  cargo test -p linggan-api --bins --locked \
  local_web::topic_browser_tests \
  -- --ignored --nocapture --test-threads=1
printf '%s\n' 'WEB-REACT-001 F Topics and Workspace real-chain proof passed (isolated database only)'

mkdir -p "$proof_dir/research"
run_proof "$proof_dir/research" env LINGGAN_PI_NODE="$node_bin" cargo test -p linggan-api --bins --locked local_web::research_browser_tests -- --ignored --nocapture --test-threads=1
