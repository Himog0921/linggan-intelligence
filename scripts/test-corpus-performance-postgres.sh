#!/usr/bin/env bash
set -euo pipefail
export LINGGAN_COLLECTION_UPGRADE_PHASE=governance

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"


# Statement counters are database-wide. Keep all proof suites serial; a caller
# cannot raise this and introduce races between reset/measurement windows.
export RUST_TEST_THREADS=1

proof_suffix="$(date -u +%Y%m%d%H%M%S)_$$_$(openssl rand -hex 4)"
proof_database="linggan_intelligence_corpus_perf_${proof_suffix}"
proof_container="linggan-intelligence-corpus-performance-proof-${proof_suffix}"
proof_volume="linggan-intelligence-corpus-performance-proof-${proof_suffix}-data"
proof_user="corpus_perf_proof_admin"
proof_password="$(openssl rand -hex 24)"
# The material fixture requires vector; reuse the pinned local proof image.
postgres_image="linggan-intelligence-postgres-pgvector:16.14-v0.8.0-r1"

[[ "$proof_database" =~ ^linggan_intelligence_corpus_perf_[a-zA-Z0-9_]+$ ]] || { echo "unsafe proof database name" >&2; exit 1; }
[[ "$proof_container" =~ ^linggan-intelligence-corpus-performance-proof-[a-zA-Z0-9_-]+$ ]] || { echo "unsafe proof container name" >&2; exit 1; }
[[ "$proof_volume" =~ ^linggan-intelligence-corpus-performance-proof-[a-zA-Z0-9_-]+-data$ ]] || { echo "unsafe proof volume name" >&2; exit 1; }

# Proof migrations and WAL need headroom on the host volume that backs Docker.
# Fail before creating resources rather than relying on a sparse Docker disk to grow.
proof_free_kib="$(df -Pk "$project_root" | awk 'NR == 2 {print $4}')"
if [[ ! "$proof_free_kib" =~ ^[0-9]+$ || "$proof_free_kib" -lt 2097152 ]]; then
  echo "CORPUS-PERFORMANCE-089 proof requires at least 2 GiB free on the host; no resources were created" >&2
  exit 1
fi

if ! docker info >/dev/null 2>&1; then
  echo "CORPUS-PERFORMANCE-089 PostgreSQL proof was not started: the Docker daemon is unavailable. No proof database, container, or volume was created." >&2
  exit 1
fi
"$project_root/scripts/runtime/build-pgvector-image.sh" --ensure

cleanup() {
  task_exit=$?
  trap - EXIT
  cleanup_failed=0
  if [[ "${proof_database_created:-0}" -eq 0 && "${proof_container_created:-0}" -eq 0 && "${proof_volume_created:-0}" -eq 0 ]]; then
    echo "CORPUS-PERFORMANCE-089 PostgreSQL proof created no resources; cleanup was not required"
    exit "$task_exit"
  fi
  if [[ "${proof_database_created:-0}" -eq 1 ]]; then
    docker exec "$proof_container" dropdb --if-exists -U "$proof_user" "$proof_database" || cleanup_failed=1
    remaining="$(docker exec "$proof_container" psql -X -v ON_ERROR_STOP=1 -U "$proof_user" -d postgres -Atc "SELECT datname FROM pg_database WHERE datname = '$proof_database';")" || cleanup_failed=1
    [[ -z "$remaining" ]] || cleanup_failed=1
  fi
  [[ "${proof_container_created:-0}" -eq 0 ]] || docker rm -f "$proof_container" >/dev/null || cleanup_failed=1
  [[ "${proof_volume_created:-0}" -eq 0 ]] || docker volume rm "$proof_volume" >/dev/null || cleanup_failed=1
  [[ "$cleanup_failed" -eq 0 ]] || { echo "CORPUS-PERFORMANCE-089 PostgreSQL proof cleanup failed" >&2; exit 1; }
  echo "CORPUS-PERFORMANCE-089 PostgreSQL proof cleanup verified; isolated database, container, and volume were removed"
  exit "$task_exit"
}
trap cleanup EXIT
printf 'CORPUS-PERFORMANCE-089 proof resources: database=%s container=%s volume=%s\n' "$proof_database" "$proof_container" "$proof_volume"

docker volume create "$proof_volume" >/dev/null
proof_volume_created=1
docker run -d --name "$proof_container" --mount "type=volume,source=$proof_volume,target=/var/lib/postgresql/data" --env POSTGRES_DB=postgres --env POSTGRES_USER="$proof_user" --env POSTGRES_PASSWORD="$proof_password" --publish 127.0.0.1::5432 "$postgres_image" -c shared_preload_libraries=pg_stat_statements -c max_wal_size=128MB -c min_wal_size=32MB >/dev/null
proof_container_created=1
for _ in {1..30}; do docker exec "$proof_container" pg_isready -U "$proof_user" -d postgres >/dev/null 2>&1 && break; sleep 1; done
docker exec "$proof_container" pg_isready -U "$proof_user" -d postgres >/dev/null
proof_port="$(docker port "$proof_container" 5432/tcp | sed -n 's/^127\.0\.0\.1:\([0-9][0-9]*\)$/\1/p')"
[[ "$proof_port" =~ ^[0-9]+$ ]] || { echo "isolated proof PostgreSQL did not expose a safe local port" >&2; exit 1; }
docker exec "$proof_container" createdb -U "$proof_user" "$proof_database"
proof_database_created=1
docker exec "$proof_container" psql -X -v ON_ERROR_STOP=1 -U "$proof_user" -d "$proof_database" -c "CREATE EXTENSION pg_stat_statements" >/dev/null
export LOCAL_001_PROOF_DATABASE_URL="postgresql://${proof_user}:${proof_password}@127.0.0.1:${proof_port}/${proof_database}"
for suite in corpus_performance_postgres material_projection_postgres material_social_postgres material_media_postgres domain_unification_postgres creator_discovery_postgres; do
  cargo test -p linggan-evidence --test "$suite" --locked -- --ignored --nocapture
done
for filter in material_projection_tests material_cursor_tests material_media_delivery_tests material_replica_fallback_tests; do
  cargo test -p linggan-api --bin linggan-api "$filter" --locked -- --ignored --nocapture
done
echo "CORPUS-PERFORMANCE-089 PostgreSQL proof passed"
