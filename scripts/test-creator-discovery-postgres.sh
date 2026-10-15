#!/usr/bin/env bash
# CREATOR-DISCOVERY-001 disposable PostgreSQL proof for the creator worker (turn-taking between
# work and author analysis, leases, budget, fingerprints) and the creator read model. Synthetic
# data and a local synthetic model process only: no shared database and no provider call.
#
# Usage: scripts/test-creator-discovery-postgres.sh [worker-test-name ...]
#   no arguments  every creator worker test, then the creator read-model suite
#   test names    only those creator worker tests (cargo name filters), for a focused rerun
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

for worker_filter in "$@"; do
  [[ "$worker_filter" =~ ^[a-zA-Z0-9_]+$ ]] || { echo "worker test filters must be plain test names: $worker_filter" >&2; exit 1; }
done

proof_suffix="$(date -u +%Y%m%d%H%M%S)_$$_$(openssl rand -hex 4)"
proof_database="linggan_creator_proof_${proof_suffix}"
proof_container="linggan-creator-proof-${proof_suffix}"
proof_volume="linggan-creator-proof-${proof_suffix}-data"
proof_user="creator_proof_admin"
proof_password="$(openssl rand -hex 24)"
# The material fixture requires vector; reuse the pinned local proof image.
postgres_image="linggan-intelligence-postgres-pgvector:16.14-v0.8.0-r1"

[[ "$proof_database" =~ ^linggan_creator_proof_[a-zA-Z0-9_]+$ ]] || { echo "unsafe proof database name" >&2; exit 1; }
[[ "$proof_container" =~ ^linggan-creator-proof-[a-zA-Z0-9_-]+$ ]] || { echo "unsafe proof container name" >&2; exit 1; }
[[ "$proof_volume" =~ ^linggan-creator-proof-[a-zA-Z0-9_-]+-data$ ]] || { echo "unsafe proof volume name" >&2; exit 1; }

# Proof migrations and WAL need headroom on the host volume that backs Docker.
# Fail before creating resources rather than relying on a sparse Docker disk to grow.
proof_free_kib="$(df -Pk "$project_root" | awk 'NR == 2 {print $4}')"
if [[ ! "$proof_free_kib" =~ ^[0-9]+$ || "$proof_free_kib" -lt 2097152 ]]; then
  echo "CREATOR-DISCOVERY-001 proof requires at least 2 GiB free on the host; no resources were created" >&2
  exit 1
fi

if ! docker info >/dev/null 2>&1; then
  echo "CREATOR-DISCOVERY-001 PostgreSQL proof was not started: the Docker daemon is unavailable. No proof database, container, or volume was created." >&2
  exit 1
fi
"$project_root/scripts/runtime/build-pgvector-image.sh" --ensure

cleanup() {
  task_exit=$?
  trap - EXIT
  cleanup_failed=0
  if [[ "${proof_database_created:-0}" -eq 0 && "${proof_container_created:-0}" -eq 0 && "${proof_volume_created:-0}" -eq 0 ]]; then
    echo "CREATOR-DISCOVERY-001 PostgreSQL proof created no resources; cleanup was not required"
    exit "$task_exit"
  fi
  if [[ "${proof_database_created:-0}" -eq 1 ]]; then
    docker exec "$proof_container" dropdb --if-exists -U "$proof_user" "$proof_database" || cleanup_failed=1
    remaining="$(docker exec "$proof_container" psql -X -v ON_ERROR_STOP=1 -U "$proof_user" -d postgres -Atc "SELECT datname FROM pg_database WHERE datname = '$proof_database';")" || cleanup_failed=1
    [[ -z "$remaining" ]] || cleanup_failed=1
  fi
  [[ "${proof_container_created:-0}" -eq 0 ]] || docker rm -f "$proof_container" >/dev/null || cleanup_failed=1
  [[ "${proof_volume_created:-0}" -eq 0 ]] || docker volume rm "$proof_volume" >/dev/null || cleanup_failed=1
  [[ "$cleanup_failed" -eq 0 ]] || { echo "CREATOR-DISCOVERY-001 PostgreSQL proof cleanup failed" >&2; exit 1; }
  echo "CREATOR-DISCOVERY-001 PostgreSQL proof cleanup verified; isolated database, container, and volume were removed"
  exit "$task_exit"
}
trap cleanup EXIT
printf 'CREATOR-DISCOVERY-001 proof resources: database=%s container=%s volume=%s\n' "$proof_database" "$proof_container" "$proof_volume"

docker volume create "$proof_volume" >/dev/null
proof_volume_created=1
docker run -d --name "$proof_container" --mount "type=volume,source=$proof_volume,target=/var/lib/postgresql/data" --env POSTGRES_DB=postgres --env POSTGRES_USER="$proof_user" --env POSTGRES_PASSWORD="$proof_password" --publish 127.0.0.1::5432 "$postgres_image" -c max_wal_size=128MB -c min_wal_size=32MB >/dev/null
proof_container_created=1
# initdb temporarily accepts Unix-socket connections before restarting the final server.
# Wait on TCP so that temporary readiness cannot make createdb race that restart.
for _ in {1..30}; do docker exec "$proof_container" pg_isready -h 127.0.0.1 -U "$proof_user" -d postgres >/dev/null 2>&1 && break; sleep 1; done
docker exec "$proof_container" pg_isready -h 127.0.0.1 -U "$proof_user" -d postgres >/dev/null
proof_port="$(docker port "$proof_container" 5432/tcp | sed -n 's/^127\.0\.0\.1:\([0-9][0-9]*\)$/\1/p')"
[[ "$proof_port" =~ ^[0-9]+$ ]] || { echo "isolated proof PostgreSQL did not expose a safe local port" >&2; exit 1; }
docker exec "$proof_container" createdb -U "$proof_user" "$proof_database"
proof_database_created=1
export LOCAL_001_PROOF_DATABASE_URL="postgresql://${proof_user}:${proof_password}@127.0.0.1:${proof_port}/${proof_database}"

# The synthetic model adapter is a local Node child process; it never opens a network connection.
export CREATOR_PROOF_NODE="${CREATOR_PROOF_NODE:-$(command -v node)}"
# Proof suites share one database; keep them serial.
export RUST_TEST_THREADS=1
cargo test -p linggan-intelligence --test creator_discovery_worker_postgres --locked -- --ignored --nocapture --test-threads=1 "$@"
if [[ "$#" -eq 0 ]]; then
  cargo test -p linggan-evidence --test creator_discovery_postgres --locked -- --ignored --nocapture --test-threads=1
fi
echo "CREATOR-DISCOVERY-001 PostgreSQL proof passed"
