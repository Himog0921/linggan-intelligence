#!/usr/bin/env bash
set -euo pipefail
export LINGGAN_COLLECTION_UPGRADE_PHASE=governance

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"


# Statement counters are database-wide. Keep all proof suites serial; a caller
# cannot raise this and introduce races between reset/measurement windows.
export RUST_TEST_THREADS=1

case "${1:-}" in
  ""|--dossier-ui|--dossier-input|--http|--ai-foundation|--ai-ledger|--ai-writing|--ai-service|--ai-install|--ai-readonly|--ai-compatibility) ;;
  *) echo "Unsupported proof selection: ${1:-}" >&2; exit 2 ;;
esac

proof_suffix="$(date -u +%Y%m%d%H%M%S)_$$_$(openssl rand -hex 4)"
proof_database="linggan_intelligence_rd_${proof_suffix}"
proof_container="linggan-intelligence-research-dossier-proof-${proof_suffix}"
proof_volume="linggan-intelligence-research-dossier-proof-${proof_suffix}-data"
proof_user="research_dossier_proof_admin"
proof_password="$(openssl rand -hex 24)"
# The material fixture requires vector; reuse the pinned local proof image.
postgres_image="linggan-intelligence-postgres-pgvector:16.14-v0.8.0-r1"

[[ "$proof_database" =~ ^linggan_intelligence_rd_[a-zA-Z0-9_]+$ ]] || { echo "unsafe proof database name" >&2; exit 1; }
[[ "$proof_container" =~ ^linggan-intelligence-research-dossier-proof-[a-zA-Z0-9_-]+$ ]] || { echo "unsafe proof container name" >&2; exit 1; }
[[ "$proof_volume" =~ ^linggan-intelligence-research-dossier-proof-[a-zA-Z0-9_-]+-data$ ]] || { echo "unsafe proof volume name" >&2; exit 1; }

# Proof migrations and WAL need headroom on the host volume that backs Docker.
# Fail before creating resources rather than relying on a sparse Docker disk to grow.
proof_free_kib="$(df -Pk "$project_root" | awk 'NR == 2 {print $4}')"
if [[ ! "$proof_free_kib" =~ ^[0-9]+$ || "$proof_free_kib" -lt 2097152 ]]; then
  echo "RESEARCH-DOSSIER-V2 proof requires at least 2 GiB free on the host; no resources were created" >&2
  exit 1
fi

if ! docker info >/dev/null 2>&1; then
  echo "RESEARCH-DOSSIER-V2 PostgreSQL proof was not started: the Docker daemon is unavailable. No proof database, container, or volume was created." >&2
  exit 1
fi
"$project_root/scripts/runtime/build-pgvector-image.sh" --ensure

cleanup() {
  task_exit=$?
  trap - EXIT
  cleanup_failed=0
  if [[ -n "${ledger_asset_root:-}" ]]; then
    if [[ "$ledger_asset_root" == "$project_root"/artifacts/private/ai-gen1-pr1b/ledger-assets.* ]]; then
      rm -rf -- "$ledger_asset_root" || cleanup_failed=1
      [[ ! -e "$ledger_asset_root" ]] || cleanup_failed=1
    else
      echo "AI ledger proof cleanup refused an unowned asset path" >&2
      cleanup_failed=1
    fi
  fi
  if [[ "${proof_database_created:-0}" -eq 0 && "${proof_container_created:-0}" -eq 0 && "${proof_volume_created:-0}" -eq 0 ]]; then
    echo "RESEARCH-DOSSIER-V2 PostgreSQL proof created no resources; cleanup was not required"
    exit "$task_exit"
  fi
  if [[ "${proof_database_created:-0}" -eq 1 ]]; then
    docker exec "$proof_container" dropdb --if-exists -U "$proof_user" "$proof_database" || cleanup_failed=1
    remaining="$(docker exec "$proof_container" psql -X -v ON_ERROR_STOP=1 -U "$proof_user" -d postgres -Atc "SELECT datname FROM pg_database WHERE datname = '$proof_database';")" || cleanup_failed=1
    [[ -z "$remaining" ]] || cleanup_failed=1
  fi
  [[ "${proof_container_created:-0}" -eq 0 ]] || docker rm -f "$proof_container" >/dev/null || cleanup_failed=1
  [[ "${proof_volume_created:-0}" -eq 0 ]] || docker volume rm "$proof_volume" >/dev/null || cleanup_failed=1
  [[ "$cleanup_failed" -eq 0 ]] || { echo "RESEARCH-DOSSIER-V2 PostgreSQL proof cleanup failed" >&2; exit 1; }
  echo "RESEARCH-DOSSIER-V2 PostgreSQL proof cleanup verified; isolated database, container, volume, and any owned ledger assets were removed"
  exit "$task_exit"
}
trap cleanup EXIT
printf 'RESEARCH-DOSSIER-V2 proof resources: database=%s container=%s volume=%s\n' "$proof_database" "$proof_container" "$proof_volume"

docker volume create "$proof_volume" >/dev/null
proof_volume_created=1
docker run -d --name "$proof_container" --mount "type=volume,source=$proof_volume,target=/var/lib/postgresql/data" --env POSTGRES_DB=postgres --env POSTGRES_USER="$proof_user" --env POSTGRES_PASSWORD="$proof_password" --publish 127.0.0.1::5432 "$postgres_image" -c shared_preload_libraries=pg_stat_statements -c max_wal_size=128MB -c min_wal_size=32MB -c max_locks_per_transaction=1024 >/dev/null
proof_container_created=1
# The image's temporary init server is Unix-only and stops before normal startup.
# A successful socket probe cannot authorize createdb against the final server.
for _ in {1..30}; do docker exec "$proof_container" pg_isready -h 127.0.0.1 -p 5432 -U "$proof_user" -d postgres >/dev/null 2>&1 && break; sleep 1; done
docker exec "$proof_container" pg_isready -h 127.0.0.1 -p 5432 -U "$proof_user" -d postgres >/dev/null
proof_port="$(docker port "$proof_container" 5432/tcp | sed -n 's/^127\.0\.0\.1:\([0-9][0-9]*\)$/\1/p')"
[[ "$proof_port" =~ ^[0-9]+$ ]] || { echo "isolated proof PostgreSQL did not expose a safe local port" >&2; exit 1; }
docker exec "$proof_container" createdb -U "$proof_user" "$proof_database"
proof_database_created=1
docker exec "$proof_container" psql -X -v ON_ERROR_STOP=1 -U "$proof_user" -d "$proof_database" -c "CREATE EXTENSION pg_stat_statements" >/dev/null
export LOCAL_001_PROOF_DATABASE_URL="postgresql://${proof_user}:${proof_password}@127.0.0.1:${proof_port}/${proof_database}"
if [[ "${1:-}" == "--ai-foundation" || "${1:-}" == "--ai-ledger" || "${1:-}" == "--ai-writing" || "${1:-}" == "--ai-service" || "${1:-}" == "--ai-install" || "${1:-}" == "--ai-readonly" || "${1:-}" == "--ai-compatibility" ]]; then
  ledger_asset_root="$(mktemp -d "$project_root/artifacts/private/ai-gen1-pr1b/ledger-assets.XXXXXX")"
  export LINGGAN_CREATION_ASSET_ROOT="$ledger_asset_root"
  export LINGGAN_LOCAL_MEDIA_ROOT="$ledger_asset_root/source-media"
  export LINGGAN_PI_NODE="$(command -v node)"
  if [[ "${1:-}" == "--ai-readonly" ]]; then
    cargo test -p linggan-intelligence --test topic_map_postgres --locked -- --ignored --nocapture
    echo "AI-GEN1 historical Topic Map source/read qualification proof passed"
    exit 0
  fi
  if [[ "${1:-}" == "--ai-install" ]]; then
    python3 -B scripts/runtime/tests/test_ai_guard.py
    python3 -B scripts/runtime/tests/test_install_flow.py
    python3 -B scripts/runtime/tests/test_local_children.py
    AI_FOUNDATION_PROOF_NODE="$(command -v node)" cargo test -p linggan-worker --test ai_worker_contract --locked -- --include-ignored --nocapture
    cargo test -p linggan-worker --test startup_contract --locked -- --include-ignored --nocapture
    echo "AI-GEN1 release handshake and patrol ownership proof passed"
    exit 0
  fi
  if [[ "${1:-}" == "--ai-service" ]]; then
    AI_FOUNDATION_PROOF_NODE="$(command -v node)" cargo test -p linggan-intelligence --test ai_foundation_postgres executor_probe:: --locked -- --ignored --nocapture
    AI_FOUNDATION_PROOF_NODE="$(command -v node)" cargo test -p linggan-intelligence --test ai_executor_service_postgres --locked -- --ignored --nocapture
    AI_FOUNDATION_PROOF_NODE="$(command -v node)" cargo test -p linggan-worker --test ai_worker_contract --locked -- --include-ignored --nocapture
    echo "AI-GEN1 dedicated executor service proof passed"
    exit 0
  fi
  if [[ "${1:-}" != "--ai-writing" && "${1:-}" != "--ai-compatibility" ]]; then
    cargo test -p linggan-intelligence --test ai_pool_cancellation_postgres --locked -- --ignored --nocapture
    AI_FOUNDATION_PROOF_NODE="$(command -v node)" cargo test -p linggan-intelligence --test ai_foundation_postgres --locked -- --ignored --nocapture
    cargo test -p linggan-api --bin linggan-api ai_settings_http_ --locked -- --ignored --nocapture
    AI_FOUNDATION_PROOF_NODE="$(command -v node)" cargo test -p linggan-intelligence --test ai_adapter_transport --locked -- --ignored --nocapture
  fi
  if [[ "${1:-}" != "--ai-compatibility" ]]; then
    AI_FOUNDATION_PROOF_NODE="$(command -v node)" cargo test -p linggan-intelligence --test ai_writing_executor_postgres --locked -- --ignored --nocapture
  fi
  export LINGGAN_PI_NODE="$ledger_asset_root/no-model-process"
  if [[ "${1:-}" == "--ai-writing" ]]; then
    cargo test -p linggan-intelligence --test content_workbench_ai_postgres --locked -- --ignored --nocapture
  fi
  if [[ "${1:-}" == "--ai-ledger" || "${1:-}" == "--ai-compatibility" ]]; then
    export CREATOR_PROOF_NODE="$(command -v node)"
    ledger_consumers=(creator_discovery_input_postgres content_workbench_ai_postgres comment_study_start_postgres topic_map_research_postgres)
    if [[ $# -gt 1 ]]; then
      ledger_consumers=()
      for ledger_selection in "${@:2}"; do
        case "$ledger_selection" in
          creator_discovery_input_postgres|content_workbench_ai_postgres|comment_study_start_postgres|comment_study_rebuild_postgres|topic_map_research_postgres) ledger_consumers+=("$ledger_selection") ;;
          *) echo "unsupported AI ledger consumer proof" >&2; exit 1 ;;
        esac
      done
    fi
    ledger_failures=0
    for ledger_consumer in "${ledger_consumers[@]}"; do
      if ! cargo test -p linggan-intelligence --test "$ledger_consumer" --locked -- --ignored --nocapture; then
        ledger_failures=$((ledger_failures + 1))
      fi
    done
    [[ "$ledger_failures" -eq 0 ]] || { echo "AI ledger consumer proof failed: $ledger_failures suites" >&2; exit 1; }
  fi
  if [[ "${1:-}" == "--ai-compatibility" ]]; then
    echo "AI-GEN1 selected historical consumer contracts passed; foundation suite was not rerun"
  else
    echo "AI-GEN1 PR-1B foundation PostgreSQL proof passed"
  fi
  exit 0
fi
if [[ "${1:-}" == "--dossier-ui" ]]; then
  command -v node >/dev/null || { echo "Node is required for the research dossier UI proof" >&2; exit 1; }
  [[ -d apps/web/node_modules ]] || npm ci --prefix apps/web --ignore-scripts --no-audit --no-fund
  npm --prefix apps/web run build
  export LINGGAN_WEB_DIST="$project_root/apps/web/dist"
  export WEB_APP_PROOF_DIR="${WEB_APP_PROOF_DIR:-$project_root/artifacts/private/research-dossier-v2/browser}"
  export WEB_APP_PROOF_NODE="$(command -v node)"
  export DOSSIER_UI_PROOF=1
  export LINGGAN_LOCAL_MEDIA_ROOT="$WEB_APP_PROOF_DIR/media"
  export LINGGAN_PI_NODE="$project_root/artifacts/private/research-dossier-v2/no-model-process"
  cargo test -p linggan-api --bin linggan-api dossier_v2_browser_proof --locked -- --ignored --nocapture
  exit 0
fi
if [[ "${1:-}" == "--dossier-input" ]]; then
  cargo test -p linggan-evidence --test local_producer_postgres every_declared_capability_admits_one_typed_package --locked -- --ignored --nocapture
  cargo test -p linggan-evidence --test dossier_input_postgres --locked -- --ignored --nocapture
  cargo test -p linggan-evidence --test research_dossier_postgres --locked -- --ignored --nocapture
  cargo test -p linggan-evidence --test material_reading_postgres --locked -- --ignored --nocapture
  cargo test -p linggan-api --bin linggan-api dossier_http_ --locked -- --ignored --nocapture
  echo "RESEARCH-DOSSIER-V2 canonical inputs, immutable readings and HTTP proof passed"
  exit 0
fi
if [[ "${1:-}" == "--http" ]]; then
  cargo test -p linggan-api --bin linggan-api dossier_http_ --locked -- --ignored --nocapture
  exit 0
fi
cargo test -p linggan-evidence --test research_dossier_postgres --locked -- --ignored --nocapture
cargo test -p linggan-evidence --test creator_discovery_postgres --locked -- --ignored --nocapture
for test in source_context_admits_only_the_latest a_reobserved_media_slot source_gate_excludes_withdrawn_ocr; do
  cargo test -p linggan-intelligence --test comment_study_rebuild_postgres "$test" --locked -- --ignored --nocapture
done
for test in dossier_partial_image_body_reaches_v2_frozen_context concurrent_ preview_and_two_runs_cover_different_comments_from_688 new_run_is_frozen_and_method_bound_dispatch missing_parent_reaches_model; do
  cargo test -p linggan-intelligence --test comment_study_start_postgres "$test" --locked -- --ignored --nocapture
done
export CREATOR_PROOF_NODE="$(command -v node)"
cargo test -p linggan-intelligence --test creator_discovery_input_postgres --locked -- --ignored --nocapture
cargo test -p linggan-api --bin linggan-api dossier_http_ --locked -- --ignored --nocapture
echo "RESEARCH-DOSSIER-V2 PostgreSQL proof passed"
