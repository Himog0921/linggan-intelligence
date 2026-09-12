#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

node --test apps/api/src/local_web/evidence_observation.test.mjs

# API media proofs intentionally use the same controlled local-media root as the delivery
# contract. Keep this isolated proof serial so one test's fixture cleanup cannot race another
# test's file assertion; callers may still request an even smaller explicit thread count.
export RUST_TEST_THREADS="${RUST_TEST_THREADS:-1}"

proof_suffix="$(date -u +%Y%m%d%H%M%S)_$$_$(openssl rand -hex 4)"
proof_database="linggan_intelligence_local_001_${proof_suffix}"
proof_container="linggan-intelligence-local-001-proof-${proof_suffix}"
proof_volume="linggan-intelligence-local-001-proof-${proof_suffix}-data"
proof_user="local_001_proof_admin"
proof_password="$(openssl rand -hex 24)"
# 0075 起夹具里有 `CREATE EXTENSION vector`，裸 postgres 镜像装不了它，整套证明会在
# 应用迁移那一步就崩。评论研究那个脚本已经换成了带 pgvector 的基础镜像，这一套当时
# 漏了——于是 main 上 LOCAL-001 全套跑不起来。与那边同一个写法。
postgres_image="linggan-intelligence-postgres-pgvector:16.14-v0.8.0-r1"

[[ "$proof_database" =~ ^linggan_intelligence_local_001_[a-zA-Z0-9_]+$ ]] || { echo "unsafe proof database name" >&2; exit 1; }
[[ "$proof_container" =~ ^linggan-intelligence-local-001-proof-[a-zA-Z0-9_-]+$ ]] || { echo "unsafe proof container name" >&2; exit 1; }
[[ "$proof_volume" =~ ^linggan-intelligence-local-001-proof-[a-zA-Z0-9_-]+-data$ ]] || { echo "unsafe proof volume name" >&2; exit 1; }

if ! docker info >/dev/null 2>&1; then
  echo "LOCAL-001 PostgreSQL proof was not started: the Docker daemon is unavailable. No proof database, container, or volume was created." >&2
  exit 1
fi
"$project_root/scripts/runtime/build-pgvector-image.sh" --ensure

cleanup() {
  task_exit=$?
  trap - EXIT
  cleanup_failed=0
  if [[ "${proof_database_created:-0}" -eq 0 && "${proof_container_created:-0}" -eq 0 && "${proof_volume_created:-0}" -eq 0 ]]; then
    echo "LOCAL-001 PostgreSQL proof created no resources; cleanup was not required"
    exit "$task_exit"
  fi
  if [[ "${proof_database_created:-0}" -eq 1 ]]; then
    docker exec "$proof_container" dropdb --if-exists -U "$proof_user" "$proof_database" || cleanup_failed=1
    remaining="$(docker exec "$proof_container" psql -X -v ON_ERROR_STOP=1 -U "$proof_user" -d postgres -Atc "SELECT datname FROM pg_database WHERE datname = '$proof_database';")" || cleanup_failed=1
    [[ -z "$remaining" ]] || cleanup_failed=1
  fi
  [[ "${proof_container_created:-0}" -eq 0 ]] || docker rm -f "$proof_container" >/dev/null || cleanup_failed=1
  [[ "${proof_volume_created:-0}" -eq 0 ]] || docker volume rm "$proof_volume" >/dev/null || cleanup_failed=1
  [[ "$cleanup_failed" -eq 0 ]] || { echo "LOCAL-001 PostgreSQL proof cleanup failed" >&2; exit 1; }
  echo "LOCAL-001 PostgreSQL proof cleanup verified; isolated database, container, and volume were removed"
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
# Collection Control predates the unified LOCAL-001 runner and its focused
# suites retain their explicit proof variable for direct invocation.  Point
# all aliases at this one disposable database so the full harness exercises
# the exact same migration ledger rather than silently skipping them.
export COLLECTION_CONTROL_PROOF_DATABASE_URL="$LOCAL_001_PROOF_DATABASE_URL"
export COLLECTION_DISPATCH_PROOF_DATABASE_URL="$LOCAL_001_PROOF_DATABASE_URL"
cargo test -p linggan-evidence --test local_discovery_postgres --locked -- --ignored
cargo test -p linggan-evidence --test local_producer_postgres --locked -- --ignored
cargo test -p linggan-evidence --test content_reobservation_postgres --locked -- --ignored
cargo test -p linggan-evidence --test material_projection_postgres --locked -- --ignored
cargo test -p linggan-evidence --test cross_industry_admission_postgres --locked -- --ignored
cargo test -p linggan-evidence --test keyword_archive_postgres --locked -- --ignored
cargo test -p linggan-evidence --test queue_position_postgres --locked -- --ignored
cargo test -p linggan-evidence --test material_social_postgres --locked -- --ignored
cargo test -p linggan-evidence --test material_media_postgres --locked -- --ignored
cargo test -p linggan-evidence --test creator_lifecycle_postgres --locked -- --ignored
cargo test -p linggan-evidence --test target_inspector_postgres --locked -- --ignored
cargo test -p linggan-evidence --test observation_target_dossier_postgres --locked -- --ignored
cargo test -p linggan-evidence --test collection_control_postgres --locked -- --ignored
cargo test -p linggan-evidence --test monitor_rule_slots_postgres --locked -- --ignored
cargo test -p linggan-evidence --test collection_control_runtime_postgres --locked -- --ignored
cargo test -p linggan-evidence --test collection_dispatch_sequence_postgres --locked -- --ignored
cargo test -p linggan-intelligence --test topic_workspace_postgres --locked -- --ignored
cargo test -p linggan-api --bin linggan-api --locked -- --ignored
echo "LOCAL-001 discovery PostgreSQL proof passed"
