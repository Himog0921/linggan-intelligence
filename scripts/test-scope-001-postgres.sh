#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

proof_suffix="$(date -u +%Y%m%d%H%M%S)_$$_$(openssl rand -hex 4)"
proof_database="linggan_intelligence_proof_${proof_suffix}"
proof_container="linggan-intelligence-scope-001-proof-${proof_suffix}"
proof_volume="linggan-intelligence-scope-001-proof-${proof_suffix}-data"
proof_user="scope_001_proof_admin"
proof_password="$(openssl rand -hex 24)"
postgres_image="postgres:16.14-bookworm@sha256:64154d0babcb1741988719e703419af0382b19953706149f9872fbd0f438efa8"

if [[ ! "$proof_database" =~ ^linggan_intelligence_proof_[0-9]{14}_[0-9]+_[0-9a-f]{8}$ ]] \
  || [[ ! "$proof_container" =~ ^linggan-intelligence-scope-001-proof-[0-9]{14}_[0-9]+_[0-9a-f]{8}$ ]] \
  || [[ ! "$proof_volume" =~ ^linggan-intelligence-scope-001-proof-[0-9]{14}_[0-9]+_[0-9a-f]{8}-data$ ]]; then
  echo "unsafe proof resource name" >&2
  exit 1
fi

container_created=0
proof_database_created=0
volume_created=0

cleanup() {
  task_exit=$?
  trap - EXIT
  cleanup_failed=0

  if [[ "$proof_database_created" -eq 1 ]]; then
    if ! docker exec "$proof_container" dropdb --if-exists -U "$proof_user" "$proof_database"; then
      echo "proof database cleanup failed" >&2
      cleanup_failed=1
    fi

    if ! remaining_proof_database="$(docker exec "$proof_container" psql -X -v ON_ERROR_STOP=1 -U "$proof_user" -d postgres -Atc "SELECT datname FROM pg_database WHERE datname = '$proof_database';")"; then
      echo "proof database cleanup verification failed" >&2
      cleanup_failed=1
    elif [[ -n "$remaining_proof_database" ]]; then
      echo "proof database still exists after cleanup" >&2
      cleanup_failed=1
    fi
  fi

  if [[ "$container_created" -eq 1 ]] && ! docker rm -f "$proof_container" >/dev/null; then
    echo "proof container cleanup failed" >&2
    cleanup_failed=1
  fi

  if [[ "$volume_created" -eq 1 ]] && ! docker volume rm "$proof_volume" >/dev/null; then
    echo "proof volume cleanup failed" >&2
    cleanup_failed=1
  fi

  if [[ "$cleanup_failed" -ne 0 ]]; then
    exit 1
  fi

  echo "SCOPE-001 PostgreSQL proof cleanup verified; isolated proof database, container, and volume were removed"
  exit "$task_exit"
}
trap cleanup EXIT

docker volume create "$proof_volume" >/dev/null
volume_created=1

docker run -d \
  --name "$proof_container" \
  --mount "type=volume,source=$proof_volume,target=/var/lib/postgresql/data" \
  --env "POSTGRES_DB=postgres" \
  --env "POSTGRES_USER=$proof_user" \
  --env "POSTGRES_PASSWORD=$proof_password" \
  --publish 127.0.0.1::5432 \
  "$postgres_image" >/dev/null
container_created=1

for attempt in {1..30}; do
  if docker exec "$proof_container" pg_isready -U "$proof_user" -d postgres >/dev/null 2>&1; then
    break
  fi
  sleep 1
done
if ! docker exec "$proof_container" pg_isready -U "$proof_user" -d postgres >/dev/null 2>&1; then
  echo "isolated proof PostgreSQL did not become ready" >&2
  exit 1
fi

proof_port="$(docker port "$proof_container" 5432/tcp | sed -n 's/^127\.0\.0\.1:\([0-9][0-9]*\)$/\1/p')"
if [[ ! "$proof_port" =~ ^[0-9]+$ ]]; then
  echo "isolated proof PostgreSQL did not expose a safe local port" >&2
  exit 1
fi

docker exec "$proof_container" createdb -U "$proof_user" "$proof_database"
proof_database_created=1
export SCOPE_001_PROOF_DATABASE_URL="postgresql://${proof_user}:${proof_password}@127.0.0.1:${proof_port}/${proof_database}"

cargo test -p linggan-evidence --test ingress_postgres --locked -- --ignored

echo "SCOPE-001 PostgreSQL proof passed"
