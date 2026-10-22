#!/usr/bin/env bash
# Own container/volume only. Never reads a runtime database URL or starts the normal worker.
set -euo pipefail
project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"
case "${1:-}" in
  '')
    [[ -z "${OCR_CHAIN_MANIFEST:-}" ]] || { echo "private manifest requires explicit --private-local" >&2; exit 2; }
    test_name=synthetic_worker_persists_only_adopted_text_with_exact_sources
    ;;
  --private-local)
    [[ -z "${GITHUB_ACTIONS:-}" && -f "${OCR_CHAIN_MANIFEST:-}" && -n "${OCR_CHAIN_OUTPUT:-}" && -f "${LINGGAN_PADDLE_VL_SCRIPT:-}" ]] || { echo "private local prerequisites missing or forbidden in CI" >&2; exit 2; }
    test_name=private_unseen_images_use_actual_worker_and_database
    ;;
  *) echo "usage: $0 [--private-local]" >&2; exit 2;;
esac
export LINGGAN_COLLECTION_UPGRADE_PHASE=governance RUST_TEST_THREADS=1
suffix="$(date -u +%Y%m%d%H%M%S)_$$_$(openssl rand -hex 4)"
proof_container="linggan-ocr-proof-${suffix}"
proof_volume="${proof_container}-data"
proof_database="linggan_intelligence_ocr_${suffix}"
proof_user=ocr_proof_admin
proof_password="$(openssl rand -hex 24)"
[[ "$proof_container" =~ ^linggan-ocr-proof-[a-zA-Z0-9_]+$ && "$proof_volume" == "$proof_container-data" ]] || exit 2
docker info >/dev/null
scripts/runtime/build-pgvector-image.sh --ensure
proof_assets="$(mktemp -d "${TMPDIR:-/tmp}/linggan-ocr-proof-assets.XXXXXX")"
cleanup() {
  code=$?;trap - EXIT
  [[ "${container_created:-0}" == 0 ]] || docker rm -f "$proof_container" >/dev/null || code=1
  [[ "${volume_created:-0}" == 0 ]] || docker volume rm "$proof_volume" >/dev/null || code=1
  if [[ "$proof_assets" == "${TMPDIR:-/tmp}/linggan-ocr-proof-assets."* ]]; then rm -rf -- "$proof_assets";else code=1;fi
  if [[ "${container_created:-0}" == 1 ]] && docker container inspect "$proof_container" >/dev/null 2>&1;then code=1;fi
  if [[ "${volume_created:-0}" == 1 ]] && docker volume inspect "$proof_volume" >/dev/null 2>&1;then code=1;fi
  echo "OCR proof cleanup verified for owned container/volume/assets (exit=$code)"
  exit "$code"
}
trap cleanup EXIT
docker volume create "$proof_volume" >/dev/null;volume_created=1
docker run -d --name "$proof_container" --mount "type=volume,source=$proof_volume,target=/var/lib/postgresql/data" --env POSTGRES_DB="$proof_database" --env POSTGRES_USER="$proof_user" --env POSTGRES_PASSWORD="$proof_password" --publish 127.0.0.1::5432 linggan-intelligence-postgres-pgvector:16.14-v0.8.0-r1 >/dev/null
container_created=1
for _ in {1..40};do docker exec "$proof_container" pg_isready -h 127.0.0.1 -U "$proof_user" -d "$proof_database" >/dev/null 2>&1 && break;sleep 1;done
docker exec "$proof_container" pg_isready -h 127.0.0.1 -U "$proof_user" -d "$proof_database" >/dev/null
proof_port="$(docker port "$proof_container" 5432/tcp | sed -n 's/^127\.0\.0\.1:\([0-9][0-9]*\)$/\1/p')"
[[ "$proof_port" =~ ^[0-9]+$ ]] || exit 2
export LOCAL_001_PROOF_DATABASE_URL="postgresql://${proof_user}:${proof_password}@127.0.0.1:${proof_port}/${proof_database}"
export LINGGAN_LOCAL_MEDIA_ROOT="$proof_assets/media"
if [[ "${1:-}" != --private-local ]];then
  export LINGGAN_PADDLE_OCR_PYTHON="$(command -v python3)"
  export LINGGAN_PADDLE_VL_SCRIPT="$proof_assets/synthetic-bridge.py"
fi
cargo test -p linggan-worker --test ocr_vl_postgres "$test_name" --locked -- --ignored --nocapture
