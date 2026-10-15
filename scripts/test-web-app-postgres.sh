#!/usr/bin/env bash
# WEB-REACT-001 阶段 B 真实链路证明：一次性隔离 PostgreSQL + 本机 Axum 路由 + 已构建的 React 前端
# + 无界面浏览器。不连接开发库或本机常驻服务；只用合成模型凭据，模型连接指向本机回环端口，
# 模型调用组件被指到不存在的路径，因此不会启动任何模型进程或外发请求。
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

# 前端：锁定依赖、重新构建，证明用的就是当前源码。
if [[ ! -d apps/web/node_modules ]]; then
  "$npm_bin" ci --prefix apps/web --ignore-scripts --no-audit --no-fund
fi
"$npm_bin" --prefix apps/web run build >/dev/null

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
"$project_root/scripts/runtime/build-pgvector-image.sh" --ensure

cleanup() {
  local result=$?
  trap - EXIT
  docker rm -f "$proof_container" >/dev/null 2>&1 || true
  docker volume rm "$proof_volume" >/dev/null 2>&1 || true
  rm -rf "$support_dir"
  echo "evidence directory (screenshots and report.json): $proof_dir"
  exit "$result"
}
trap cleanup EXIT
docker volume create "$proof_volume" >/dev/null
docker run -d --name "$proof_container" \
  --mount "type=volume,source=$proof_volume,target=/var/lib/postgresql/data" \
  --env POSTGRES_DB="$proof_database" --env POSTGRES_USER="$proof_user" \
  --env POSTGRES_PASSWORD="$proof_password" --publish 127.0.0.1::5432 \
  "$postgres_image" >/dev/null
for _ in {1..30}; do
  docker exec "$proof_container" pg_isready -U "$proof_user" -d "$proof_database" >/dev/null 2>&1 && break
  sleep 1
done
docker exec "$proof_container" pg_isready -U "$proof_user" -d "$proof_database" >/dev/null
proof_port="$(docker inspect --format '{{(index (index .NetworkSettings.Ports "5432/tcp") 0).HostPort}}' "$proof_container")"
mkdir -p "$proof_dir"

LOCAL_001_PROOF_DATABASE_URL="postgresql://${proof_user}:${proof_password}@127.0.0.1:${proof_port}/${proof_database}" \
WEB_APP_BROWSER_PROOF=1 \
WEB_APP_PROOF_DIR="$proof_dir" \
WEB_APP_PROOF_NODE="$node_bin" \
LINGGAN_WEB_DIST="$project_root/apps/web/dist" \
LINGGAN_MODEL_SYNTHETIC_PREVIEW=SYNTHETIC-NOT-EVIDENCE \
LINGGAN_SUPPORT_DIR="$support_dir" \
LINGGAN_PI_NODE="$support_dir/no-model-process" \
RUST_TEST_THREADS=1 \
  cargo test -p linggan-api --bins --locked \
  local_web::web_app_tests::settings_page_real_chain_browser_proof \
  -- --ignored --nocapture --test-threads=1
printf '%s\n' 'WEB-REACT-001 settings page real-chain proof passed (isolated database; synthetic credential only)'
