#!/usr/bin/env bash
set -euo pipefail
export LINGGAN_COLLECTION_UPGRADE_PHASE=governance
case "${1:-}" in ""|--http|--browser) ;; *) echo "usage: $0 [--http|--browser]" >&2; exit 2 ;; esac

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"


# Statement counters are database-wide. Keep all proof suites serial; a caller
# cannot raise this and introduce races between reset/measurement windows.
export RUST_TEST_THREADS=1
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
export CARGO_PROFILE_DEV_DEBUG=0
export CARGO_PROFILE_TEST_DEBUG=0
export CARGO_INCREMENTAL=0
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/tmp/linggan-uiux-workbench-target}"

if [[ "${1:-}" == "--browser" ]]; then
  node_bin="${CW_BROWSER_PROOF_NODE:-$HOME/.nvm/versions/node/v$(cat .nvmrc)/bin/node}"
  [[ -x "$node_bin" ]] || node_bin="$(command -v node || true)"
  [[ -x "$node_bin" ]] || { echo "Node is unavailable" >&2; exit 1; }
  export PATH="$(dirname "$node_bin"):$PATH"
  if [[ ! -d apps/web/node_modules ]]; then npm ci --prefix apps/web --ignore-scripts --no-audit --no-fund; fi
  npm --prefix apps/web run build
  export CW_BROWSER_PROOF=1
  export CW_BROWSER_PROOF_NODE="$node_bin"
  export CW_BROWSER_PROOF_DIR="${CW_BROWSER_PROOF_DIR:-$project_root/artifacts/private/uiux-content-workbench-001/implementation/browser-$(date -u +%Y%m%dT%H%M%SZ)}"
  export LINGGAN_WEB_DIST="$project_root/apps/web/dist"
  export LINGGAN_MODEL_SYNTHETIC_PREVIEW=SYNTHETIC-NOT-EVIDENCE
fi

proof_suffix="$(date -u +%Y%m%d%H%M%S)_$$_$(openssl rand -hex 4)"
proof_database="linggan_intelligence_cw_${proof_suffix}"
proof_container="linggan-intelligence-content-workbench-proof-${proof_suffix}"
proof_volume="linggan-intelligence-content-workbench-proof-${proof_suffix}-data"
proof_user="content_workbench_proof_admin"
proof_password="$(openssl rand -hex 24)"
# The material fixture requires vector; reuse the pinned local proof image.
postgres_image="linggan-intelligence-postgres-pgvector:16.14-v0.8.0-r1"

[[ "$proof_database" =~ ^linggan_intelligence_cw_[a-zA-Z0-9_]+$ ]] || { echo "unsafe proof database name" >&2; exit 1; }
[[ "$proof_container" =~ ^linggan-intelligence-content-workbench-proof-[a-zA-Z0-9_-]+$ ]] || { echo "unsafe proof container name" >&2; exit 1; }
[[ "$proof_volume" =~ ^linggan-intelligence-content-workbench-proof-[a-zA-Z0-9_-]+-data$ ]] || { echo "unsafe proof volume name" >&2; exit 1; }

# Proof migrations and WAL need headroom on the host volume that backs Docker.
# Fail before creating resources rather than relying on a sparse Docker disk to grow.
proof_free_kib="$(df -Pk "$project_root" | awk 'NR == 2 {print $4}')"
if [[ ! "$proof_free_kib" =~ ^[0-9]+$ || "$proof_free_kib" -lt 2097152 ]]; then
  echo "UIUX-CONTENT-WORKBENCH-001 proof requires at least 2 GiB free on the host; no resources were created" >&2
  exit 1
fi

if ! docker info >/dev/null 2>&1; then
  echo "UIUX-CONTENT-WORKBENCH-001 PostgreSQL proof was not started: the Docker daemon is unavailable. No proof database, container, or volume was created." >&2
  exit 1
fi
"$project_root/scripts/runtime/build-pgvector-image.sh" --ensure

cleanup() {
  task_exit=$?
  trap - EXIT
  cleanup_failed=0
  if [[ -n "${proof_asset_root:-}" && "$proof_asset_root" == /tmp/linggan-content-workbench-assets.* ]]; then
    rm -rf -- "$proof_asset_root" || cleanup_failed=1
  fi
  if [[ "${proof_database_created:-0}" -eq 0 && "${proof_container_attempted:-0}" -eq 0 && "${proof_volume_created:-0}" -eq 0 && "$cleanup_failed" -eq 0 ]]; then
    echo "UIUX-CONTENT-WORKBENCH-001 PostgreSQL proof created no resources; cleanup was not required"
    exit "$task_exit"
  fi
  if [[ "${proof_database_created:-0}" -eq 1 ]]; then
    docker exec "$proof_container" dropdb --if-exists -U "$proof_user" "$proof_database" || cleanup_failed=1
    remaining="$(docker exec "$proof_container" psql -X -v ON_ERROR_STOP=1 -U "$proof_user" -d postgres -Atc "SELECT datname FROM pg_database WHERE datname = '$proof_database';")" || cleanup_failed=1
    [[ -z "$remaining" ]] || cleanup_failed=1
  fi
  if [[ "${proof_container_attempted:-0}" -eq 1 ]]; then
    if remaining_container="$(docker ps -a --filter "name=^/${proof_container}$" --format '{{.Names}}')"; then
      if [[ "$remaining_container" == "$proof_container" ]]; then docker rm -f "$proof_container" >/dev/null || cleanup_failed=1;
      elif [[ -n "$remaining_container" ]]; then cleanup_failed=1; fi
    else cleanup_failed=1; fi
  fi
  [[ "${proof_volume_created:-0}" -eq 0 ]] || docker volume rm "$proof_volume" >/dev/null || cleanup_failed=1
  if [[ "$cleanup_failed" -ne 0 ]]; then
    echo "UIUX-CONTENT-WORKBENCH-001 PostgreSQL proof cleanup failed" >&2
    [[ "$task_exit" -ne 0 ]] || task_exit=1
    exit "$task_exit"
  fi
  echo "UIUX-CONTENT-WORKBENCH-001 PostgreSQL proof cleanup verified; controlled database, container, and volume absent"
  exit "$task_exit"
}
trap cleanup EXIT
proof_asset_root="$(mktemp -d /tmp/linggan-content-workbench-assets.XXXXXX)"
export LINGGAN_CREATION_ASSET_ROOT="$proof_asset_root"
export LINGGAN_LOCAL_MEDIA_ROOT="$proof_asset_root/source-media"
if [[ "${1:-}" == "--browser" ]]; then
  export LINGGAN_SUPPORT_DIR="$proof_asset_root/support"
  export LINGGAN_PI_NODE="$proof_asset_root/no-model-process"
fi
if [[ -n "${DOSSIER_PROOF_IMAGE:-}" ]]; then
  mkdir -p "$LINGGAN_LOCAL_MEDIA_ROOT/blobs"
  cp "$DOSSIER_PROOF_IMAGE" "$LINGGAN_LOCAL_MEDIA_ROOT/blobs/dossier.jpg"
fi
printf 'UIUX-CONTENT-WORKBENCH-001 proof resources: database=%s container=%s volume=%s\n' "$proof_database" "$proof_container" "$proof_volume"

docker volume create "$proof_volume" >/dev/null
proof_volume_created=1
proof_container_attempted=1
docker run -d --name "$proof_container" --mount "type=volume,source=$proof_volume,target=/var/lib/postgresql/data" --env POSTGRES_DB=postgres --env POSTGRES_USER="$proof_user" --env POSTGRES_PASSWORD="$proof_password" --publish 127.0.0.1::5432 "$postgres_image" -c shared_preload_libraries=pg_stat_statements -c max_wal_size=128MB -c min_wal_size=32MB >/dev/null
# The temporary init server is Unix-only and is stopped before normal startup.
# Wait for the final TCP listener used by the proof, not the temporary socket.
for _ in {1..30}; do docker exec "$proof_container" pg_isready -h 127.0.0.1 -p 5432 -U "$proof_user" -d postgres >/dev/null 2>&1 && break; sleep 1; done
if ! docker exec "$proof_container" pg_isready -h 127.0.0.1 -p 5432 -U "$proof_user" -d postgres >/dev/null 2>&1; then
  echo "UIUX-CONTENT-WORKBENCH-001 isolated PostgreSQL did not become TCP-ready within 30 seconds" >&2
  # Best-effort diagnostics do not change the mandatory failure exit or reveal Env/DSN.
  docker inspect --format 'container state={{.State.Status}} exit={{.State.ExitCode}}' "$proof_container" >&2 || echo 'container state unavailable' >&2
  docker logs --tail 80 "$proof_container" >&2 || echo 'container startup logs unavailable' >&2
  exit 1
fi
proof_port="$(docker port "$proof_container" 5432/tcp | sed -n 's/^127\.0\.0\.1:\([0-9][0-9]*\)$/\1/p')"
[[ "$proof_port" =~ ^[0-9]+$ ]] || { echo "isolated proof PostgreSQL did not expose a safe local port" >&2; exit 1; }
docker exec "$proof_container" createdb -U "$proof_user" "$proof_database"
proof_database_created=1
docker exec "$proof_container" psql -X -v ON_ERROR_STOP=1 -U "$proof_user" -d "$proof_database" -c "CREATE EXTENSION pg_stat_statements" >/dev/null
export LOCAL_001_PROOF_DATABASE_URL="postgresql://${proof_user}:${proof_password}@127.0.0.1:${proof_port}/${proof_database}"
if [[ "${1:-}" == "--browser" ]]; then
  cargo test -p linggan-api --bin linggan-api content_workbench_real_chain_browser_proof --locked -- --ignored --nocapture
  exit 0
fi
if [[ "${1:-}" == "--http" ]]; then
  cargo test -p linggan-api --bin linggan-api content_workbench_http_ --locked -- --ignored --nocapture
  cargo test -p linggan-api --bin linggan-api creation_assets_http_ --locked -- --ignored --nocapture
  exit 0
fi
cargo test -p linggan-intelligence --test content_workbench_recovery_postgres --locked -- --ignored --nocapture
cargo test -p linggan-intelligence --test content_workbench_postgres --locked -- --ignored --nocapture
cargo test -p linggan-intelligence --test content_workbench_sources_postgres --locked -- --ignored --nocapture
cargo test -p linggan-intelligence --test content_workbench_ai_postgres --locked -- --ignored --nocapture
cargo test -p linggan-intelligence --test creation_assets_postgres --locked -- --ignored --nocapture
cargo test -p linggan-intelligence --lib --locked -- creation_assets::
cargo test -p linggan-api --bin linggan-api content_workbench_http_ --locked -- --ignored --nocapture
cargo test -p linggan-api --bin linggan-api creation_assets_http_ --locked -- --ignored --nocapture
echo "UIUX-CONTENT-WORKBENCH-001 isolated PostgreSQL checks passed; real provider, shared runtime and Mog acceptance are not proved"
