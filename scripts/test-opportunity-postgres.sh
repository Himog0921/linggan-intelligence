#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

proof_suffix="$(date -u +%Y%m%d%H%M%S)_$$_$(openssl rand -hex 4)"
proof_database="linggan_intelligence_proof_${proof_suffix}"
proof_container="linggan-intelligence-opportunity-proof-${proof_suffix}"
proof_volume="linggan-intelligence-opportunity-proof-${proof_suffix}-data"
proof_user="opportunity_proof_admin"
proof_password="$(openssl rand -hex 24)"
postgres_image="linggan-intelligence-postgres-pgvector:16.14-v0.8.0-r1"
proof_support_dir=""

if [[ ! "$proof_database" =~ ^linggan_intelligence_proof_[0-9]{14}_[0-9]+_[0-9a-f]{8}$ ]] \
  || [[ ! "$proof_container" =~ ^linggan-intelligence-opportunity-proof-[0-9]{14}_[0-9]+_[0-9a-f]{8}$ ]] \
  || [[ ! "$proof_volume" =~ ^linggan-intelligence-opportunity-proof-[0-9]{14}_[0-9]+_[0-9a-f]{8}-data$ ]]; then
  echo "unsafe proof resource name" >&2
  exit 1
fi

"$project_root/scripts/runtime/build-pgvector-image.sh" --ensure

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

  if [[ -n "$proof_support_dir" ]]; then
    rm -f "$proof_support_dir/opportunity/commenter-digest.key" || cleanup_failed=1
    rmdir "$proof_support_dir/opportunity" "$proof_support_dir" || cleanup_failed=1
  fi

  if [[ "$cleanup_failed" -ne 0 ]]; then
    exit 1
  fi

  echo "Opportunity PostgreSQL proof cleanup verified; isolated proof database, container, and volume were removed"
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
export OPPORTUNITY_PROOF_DATABASE_URL="postgresql://${proof_user}:${proof_password}@127.0.0.1:${proof_port}/${proof_database}"

cargo test -p linggan-intelligence --test opportunity_postgres --locked -- --ignored --test-threads=1
cargo test -p linggan-intelligence --test opportunity_mentions_postgres --locked -- --ignored --test-threads=1
cargo test -p linggan-intelligence --test opportunity_asof_postgres --locked -- --ignored --test-threads=1
cargo test -p linggan-intelligence --test opportunity_metrics_postgres --locked -- --ignored --test-threads=1
LOCAL_001_PROOF_DATABASE_URL="$OPPORTUNITY_PROOF_DATABASE_URL" \
  cargo test -p linggan-evidence --test creator_discovery_postgres --locked \
    -- --ignored --exact profile_only_author_uses_the_shared_as_of_owner --test-threads=1

# Exercise the real 0177 publication path with an owned loopback model stub. The synthetic digest
# key exists only in this temporary private directory and is never printed or committed.
[[ -d apps/pi-adapter/node_modules ]] || { echo "pi-adapter dependencies are required for the synthetic publication proof" >&2; exit 1; }
proof_support_dir="$(mktemp -d "${TMPDIR:-/tmp}/linggan-opportunity-support.XXXXXXXX")"
chmod 700 "$proof_support_dir"
mkdir -m 700 "$proof_support_dir/opportunity"
openssl rand 32 > "$proof_support_dir/opportunity/commenter-digest.key"
chmod 600 "$proof_support_dir/opportunity/commenter-digest.key"
for proof_mode in enabled missing_key disabled locked_release locked_drain; do
  proof_lock=""
  proof_periodic=0
  case "$proof_mode" in
    enabled) proof_flag=enabled; proof_missing=0; proof_periodic=1 ;;
    missing_key)
      proof_flag=enabled; proof_missing=1
      rm "$proof_support_dir/opportunity/commenter-digest.key"
      ;;
    disabled) proof_flag=disabled; proof_missing=0 ;;
    locked_release) proof_flag=enabled; proof_missing=0; proof_lock=release ;;
    locked_drain) proof_flag=enabled; proof_missing=0; proof_lock=drain ;;
  esac
  LOCAL_001_PROOF_DATABASE_URL="$OPPORTUNITY_PROOF_DATABASE_URL" \
    LINGGAN_SUPPORT_DIR="$proof_support_dir" LINGGAN_OPPORTUNITY_CARD="$proof_flag" \
    OPPORTUNITY_TEST_MISSING_KEY="$proof_missing" OPPORTUNITY_TEST_LOCK_MODE="$proof_lock" \
    OPPORTUNITY_TEST_PERIODIC="$proof_periodic" \
    AI_FOUNDATION_PROOF_NODE="$(command -v node)" \
    cargo test -p linggan-intelligence --test comment_research_executor_postgres --locked \
      -- --ignored --exact comment_uses_target_dossier_and_both_permissions --test-threads=1
done

LINGGAN_LOCAL_DATABASE_URL="$OPPORTUNITY_PROOF_DATABASE_URL" \
  cargo run -q -p linggan-worker --bin linggan-opportunity --locked -- \
  check-invariants --schema opportunity_t07_invariants

LINGGAN_LOCAL_DATABASE_URL="$OPPORTUNITY_PROOF_DATABASE_URL" \
  cargo run -q -p linggan-worker --bin linggan-opportunity --locked -- \
  platform-time-report --domain 00000000-0000-4000-8000-000000000001 \
  --as-of 2026-11-10T00:00:00Z --tz Asia/Shanghai --periods 12 \
  --schema opportunity_c3_basis

echo "Opportunity PostgreSQL proof passed"
