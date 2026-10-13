#!/usr/bin/env bash
set -euo pipefail
# Only synthetic fixtures in a disposable database; no API, worker or producer is started.
export LINGGAN_COLLECTION_UPGRADE_PHASE=governance

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

proof_suffix="$(date -u +%Y%m%d%H%M%S)_$$_$(openssl rand -hex 4)"
proof_database="linggan_collection_single_run_${proof_suffix}"
proof_container="linggan-collection-single-run-proof-${proof_suffix}"
proof_volume="linggan-collection-single-run-proof-${proof_suffix}-data"
proof_user="collection_single_run_proof_admin"
proof_password="$(openssl rand -hex 24)"
postgres_image="linggan-intelligence-postgres-pgvector:16.14-v0.8.0-r1"

[[ "$proof_database" =~ ^linggan_collection_single_run_[a-zA-Z0-9_]+$ ]] || { echo "unsafe proof database name" >&2; exit 1; }
[[ "$proof_container" =~ ^linggan-collection-single-run-proof-[a-zA-Z0-9_-]+$ ]] || { echo "unsafe proof container name" >&2; exit 1; }
[[ "$proof_volume" =~ ^linggan-collection-single-run-proof-[a-zA-Z0-9_-]+-data$ ]] || { echo "unsafe proof volume name" >&2; exit 1; }

if ! docker info >/dev/null 2>&1; then
  echo "focused collection single run proof was not started: Docker is unavailable" >&2
  exit 1
fi
"$project_root/scripts/runtime/build-pgvector-image.sh" --ensure

cleanup() {
  task_exit=$?
  trap - EXIT
  cleanup_failed=0
  [[ "${proof_database_created:-0}" -eq 0 ]] || docker exec "$proof_container" dropdb --if-exists -U "$proof_user" "$proof_database" || cleanup_failed=1
  [[ "${proof_container_created:-0}" -eq 0 ]] || docker rm -f "$proof_container" >/dev/null || cleanup_failed=1
  [[ "${proof_volume_created:-0}" -eq 0 ]] || docker volume rm "$proof_volume" >/dev/null || cleanup_failed=1
  [[ "$cleanup_failed" -eq 0 ]] || { echo "focused collection single run proof cleanup failed" >&2; exit 1; }
  echo "focused collection single run proof cleanup verified"
  exit "$task_exit"
}
trap cleanup EXIT

docker volume create "$proof_volume" >/dev/null
proof_volume_created=1
docker run -d --name "$proof_container" --mount "type=volume,source=$proof_volume,target=/var/lib/postgresql/data" --env POSTGRES_DB=postgres --env POSTGRES_USER="$proof_user" --env POSTGRES_PASSWORD="$proof_password" --publish 127.0.0.1::5432 "$postgres_image" >/dev/null
proof_container_created=1
for _ in {1..30}; do docker exec "$proof_container" pg_isready -U "$proof_user" -d postgres >/dev/null 2>&1 && break; sleep 1; done
docker exec "$proof_container" pg_isready -U "$proof_user" -d postgres >/dev/null
proof_port="$(docker port "$proof_container" 5432/tcp | sed -n 's/^127\.0\.0\.1:\([0-9][0-9]*\)$/\1/p')"
[[ "$proof_port" =~ ^[0-9]+$ ]] || { echo "isolated proof PostgreSQL did not expose a safe local port" >&2; exit 1; }
docker exec "$proof_container" createdb -U "$proof_user" "$proof_database"
proof_database_created=1
export LOCAL_001_PROOF_DATABASE_URL="postgresql://${proof_user}:${proof_password}@127.0.0.1:${proof_port}/${proof_database}"
export COLLECTION_CONTROL_PROOF_DATABASE_URL="$LOCAL_001_PROOF_DATABASE_URL"
if [[ "${COLLECTION_RUN_CORE_PROOF:-1}" == "1" ]]; then
  cargo test -p linggan-evidence --test collection_single_run_postgres --locked -- --ignored --test-threads=1
fi
if [[ "${COLLECTION_RUN_REGRESSION_PROOF:-0}" == "1" ]]; then
  for proof_target in collection_control_postgres collection_control_runtime_postgres keyword_archive_postgres material_social_postgres material_projection_postgres creator_lifecycle_postgres local_producer_postgres scheduler_tick_postgres; do
    cargo test -p linggan-evidence --test "$proof_target" --locked -- --ignored --test-threads=1
  done
fi
if [[ "${COLLECTION_RUN_BROWSER_PROOF:-0}" == "1" ]]; then
  cargo test -p linggan-api --locked collection_single_run_browser_with_persisted_data -- --ignored --test-threads=1
fi
echo "focused collection single run PostgreSQL proof passed"
