#!/usr/bin/env bash
# Isolated synthetic proof only; does not migrate or access the development database.
set -euo pipefail
project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"
command -v cargo >/dev/null || { echo "Cargo is unavailable" >&2; exit 1; }
docker info >/dev/null 2>&1 || { echo "Docker is unavailable" >&2; exit 1; }
suffix="$(date -u +%Y%m%d%H%M%S)_$$_$(openssl rand -hex 4)"
proof_database="linggan_comment_study_proof_${suffix}"
proof_container="linggan-comment-study-productization-${suffix}"
proof_volume="${proof_container}-data"
proof_user="comment_study_proof_admin"
proof_password="$(openssl rand -hex 24)"
postgres_image="linggan-intelligence-postgres-pgvector:16.14-v0.8.0-r1"
[[ "$proof_database" =~ ^linggan_comment_study_proof_[a-zA-Z0-9_]+$ ]] || exit 1
[[ "$proof_container" =~ ^linggan-comment-study-productization-[a-zA-Z0-9_-]+$ ]] || exit 1
[[ "$proof_volume" =~ ^linggan-comment-study-productization-[a-zA-Z0-9_-]+-data$ ]] || exit 1
"$project_root/scripts/runtime/build-pgvector-image.sh" --ensure
cleanup() {
  local result=$?
  trap - EXIT
  docker rm -f "$proof_container" >/dev/null 2>&1 || true
  docker volume rm "$proof_volume" >/dev/null 2>&1 || true
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
export LOCAL_001_PROOF_DATABASE_URL="postgresql://${proof_user}:${proof_password}@127.0.0.1:${proof_port}/${proof_database}"
RUST_TEST_THREADS=1 cargo test -p linggan-intelligence \
  --test comment_study_productization_postgres \
  --test comment_study_catalog_postgres \
  --test comment_study_catalog_read_postgres \
  --test comment_study_inspection_postgres \
  --test comment_study_work_catalog_postgres \
  --test comment_study_p1_closure_postgres \
  --test comment_study_policy_postgres \
  --test comment_study_start_postgres --locked \
  -- --ignored --nocapture --test-threads=1
# These are actual Axum requests over the same disposable PostgreSQL proof database.
# Run the ignored group explicitly; absence of a test file is never treated as a pass.
RUST_TEST_THREADS=1 cargo test -p linggan-api --locked \
  local_web::comment_study::catalog_api::command_api::postgres_tests:: \
  -- --ignored --nocapture --test-threads=1
printf '%s\n' 'Implemented Comment Study productization PostgreSQL subset passed; not the complete T01-T54 suite'
