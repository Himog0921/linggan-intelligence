#!/usr/bin/env bash
# Isolated synthetic proof only; does not migrate or access the development database.
set -euo pipefail
project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"
command -v cargo >/dev/null || { echo "Cargo is unavailable" >&2; exit 1; }
docker info >/dev/null 2>&1 || { echo "Docker is unavailable" >&2; exit 1; }
# The formal page is now React; both API browser proofs use the same built entry.
npm --prefix apps/web ci --ignore-scripts --no-audit --no-fund
npm --prefix apps/web run build >/dev/null
export LINGGAN_WEB_DIST="$project_root/apps/web/dist"
export WEB_APP_PROOF_NODE="$(command -v node)"

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
  local task_exit=$?
  trap - EXIT
  local cleanup_failed=0 remaining
  if [[ "${proof_container_created:-0}" -eq 1 ]]; then
    docker rm -f "$proof_container" >/dev/null || cleanup_failed=1
    remaining="$(docker ps -a --filter "name=$proof_container" --format '{{.Names}}')" || cleanup_failed=1
    [[ -z "$remaining" ]] || cleanup_failed=1
  fi
  if [[ "${proof_volume_created:-0}" -eq 1 ]]; then
    docker volume rm "$proof_volume" >/dev/null || cleanup_failed=1
    remaining="$(docker volume ls --filter "name=$proof_volume" --format '{{.Name}}')" || cleanup_failed=1
    [[ -z "$remaining" ]] || cleanup_failed=1
  fi
  [[ "$cleanup_failed" -eq 0 ]] || { echo "Comment Study owned proof cleanup failed" >&2; exit 1; }
  echo "Comment Study owned proof container and volume cleanup verified"
  exit "$task_exit"
}
trap cleanup EXIT
printf 'Comment Study owned proof resources: container=%s volume=%s\n' "$proof_container" "$proof_volume"
docker volume create "$proof_volume" >/dev/null
proof_volume_created=1
docker run -d --name "$proof_container" \
  --mount "type=volume,source=$proof_volume,target=/var/lib/postgresql/data" \
  --env POSTGRES_DB="$proof_database" --env POSTGRES_USER="$proof_user" \
  --env POSTGRES_PASSWORD="$proof_password" --publish 127.0.0.1::5432 \
  "$postgres_image" >/dev/null
proof_container_created=1
for _ in {1..30}; do
  docker exec "$proof_container" pg_isready -h 127.0.0.1 -p 5432 -U "$proof_user" -d "$proof_database" >/dev/null 2>&1 && break
  sleep 1
done
docker exec "$proof_container" pg_isready -h 127.0.0.1 -p 5432 -U "$proof_user" -d "$proof_database" >/dev/null
proof_port="$(docker inspect --format '{{(index (index .NetworkSettings.Ports "5432/tcp") 0).HostPort}}' "$proof_container")"
export LOCAL_001_PROOF_DATABASE_URL="postgresql://${proof_user}:${proof_password}@127.0.0.1:${proof_port}/${proof_database}"
RUST_TEST_THREADS=1 cargo test -p linggan-intelligence \
  --test comment_study_catalog_postgres \
  --test comment_study_catalog_read_postgres \
  --test comment_study_inspection_postgres \
  --test comment_study_work_catalog_postgres \
  --test comment_study_p1_closure_postgres \
  --test comment_study_start_postgres --locked \
  -- --ignored --nocapture --test-threads=1
# Current three-stage workflow is exercised through the shared executor and an owned loopback provider.
AI_FOUNDATION_PROOF_NODE="$(command -v node)" RUST_TEST_THREADS=1 cargo test -p linggan-intelligence --test comment_research_executor_postgres --locked -- --ignored --nocapture --test-threads=1
# Patrol now owns only local maintenance. The AI owner/signal/usage proofs run in
# test-research-dossier-postgres.sh --ai-service with its explicit pinned Node/SDK setup.
RUST_TEST_THREADS=1 cargo test -p linggan-worker --test startup_contract --locked \
  patrol_maintains_local_catalog_without_instantiating_model_transport \
  -- --ignored --nocapture --test-threads=1
RUST_TEST_THREADS=1 cargo test -p linggan-worker --test startup_contract --locked \
  sigterm_drains_even_while_the_collection_tick_is_blocked \
  -- --ignored --nocapture --test-threads=1
printf '%s\n' 'Current comment catalog, three-stage workflow and patrol ownership proofs passed'
