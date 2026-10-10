#!/usr/bin/env bash
# TOPIC-MAP-V41-001: disposable synthetic PostgreSQL proof, never the runtime database.
set -Eeuo pipefail
trap 'echo "Topic Map proof failed at script line ${LINENO}" >&2' ERR
project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"
docker info >/dev/null 2>&1 || { echo "Docker is unavailable" >&2; exit 1; }
suffix="$(date -u +%Y%m%d%H%M%S)_$$_$(openssl rand -hex 4)"
proof_database="linggan_topic_map_proof_${suffix}"
proof_container="linggan-topic-map-proof-${suffix}"
proof_volume="${proof_container}-data"
proof_user="topic_map_proof_admin"
proof_password="$(openssl rand -hex 24)"
postgres_image="linggan-intelligence-postgres-pgvector:16.14-v0.8.0-r1"
[[ "$proof_database" =~ ^linggan_topic_map_proof_[a-zA-Z0-9_]+$ ]] || exit 1
[[ "$proof_container" =~ ^linggan-topic-map-proof-[a-zA-Z0-9_-]+$ ]] || exit 1
[[ "$proof_volume" =~ ^linggan-topic-map-proof-[a-zA-Z0-9_-]+-data$ ]] || exit 1
"$project_root/scripts/runtime/build-pgvector-image.sh" --ensure
cleanup() {
  task_exit=$?
  trap - EXIT
  cleanup_failed=0
  [[ "${proof_container_created:-0}" -eq 0 ]] || docker rm -f "$proof_container" >/dev/null || cleanup_failed=1
  [[ "${proof_volume_created:-0}" -eq 0 ]] || docker volume rm "$proof_volume" >/dev/null || cleanup_failed=1
  [[ "$cleanup_failed" -eq 0 ]] || { echo "Topic Map proof cleanup failed" >&2; exit 1; }
  echo "Topic Map isolated container and volume cleanup verified"
  exit "$task_exit"
}
trap cleanup EXIT
docker volume create "$proof_volume" >/dev/null
proof_volume_created=1
docker run -d --name "$proof_container" --mount "type=volume,source=$proof_volume,target=/var/lib/postgresql/data" --env POSTGRES_DB="$proof_database" --env POSTGRES_USER="$proof_user" --env POSTGRES_PASSWORD="$proof_password" --publish 127.0.0.1::5432 "$postgres_image" >/dev/null
proof_container_created=1
# The image initializes through a socket-only server. Wait for the final TCP
# service used by the proof so initialization cannot satisfy this probe early.
for _ in {1..30}; do
  docker exec "$proof_container" pg_isready -h 127.0.0.1 -p 5432 -U "$proof_user" -d "$proof_database" >/dev/null 2>&1 && break
  sleep 1
done
docker exec "$proof_container" pg_isready -h 127.0.0.1 -p 5432 -U "$proof_user" -d "$proof_database" >/dev/null
proof_port="$(docker port "$proof_container" 5432/tcp | sed -n 's/^127\.0\.0\.1:\([0-9][0-9]*\)$/\1/p')"
[[ "$proof_port" =~ ^[0-9]+$ ]] || exit 1
export CREATOR_PROOF_NODE="${CREATOR_PROOF_NODE:-$(command -v node)}"
[[ -x "$CREATOR_PROOF_NODE" ]] || { echo "Synthetic proof Node executable is unavailable" >&2; exit 1; }
export LINGGAN_COLLECTION_UPGRADE_PHASE=governance
export LOCAL_001_PROOF_DATABASE_URL="postgresql://${proof_user}:${proof_password}@127.0.0.1:${proof_port}/${proof_database}"
# Attempt every selected target, preserving failure so later proofs are not hidden.
proof_exit=0
cargo test -p linggan-intelligence --test topic_map_postgres --test topic_map_research_postgres --test topic_map_core_postgres --test topic_map_core_lifecycle_postgres --test topic_map_core_unknown_postgres --test topic_map_core_legacy_postgres --test topic_map_saved_sources_postgres --test topic_map_search_postgres --locked --no-fail-fast -- --ignored --test-threads=1 || proof_exit=$?
cargo test -p linggan-evidence --test topic_map_comment_budget_postgres --locked --no-fail-fast -- --ignored --test-threads=1 || proof_exit=$?
cargo test -p linggan-api --locked --no-fail-fast topic_map -- --ignored --test-threads=1 || proof_exit=$?
if [[ "$proof_exit" -ne 0 ]]; then
  echo "Topic Map proof failed; all selected targets were attempted" >&2
fi
exit "$proof_exit"
