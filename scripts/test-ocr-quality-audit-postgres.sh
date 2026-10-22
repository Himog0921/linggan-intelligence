#!/usr/bin/env bash
# Own disposable PostgreSQL, synthetic images and loopback SDK only. No runtime DSN.
set -euo pipefail
project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"
audit_tmp_root="${TMPDIR:-/tmp}"
audit_tmp_root="${audit_tmp_root%/}"
export AI_FOUNDATION_PROOF_NODE="$(command -v node)"
case "${1:-}" in
 '') ;;
 --browser)
  npm --prefix apps/web run build
  export LINGGAN_WEB_DIST="$project_root/apps/web/dist"
  export LINGGAN_MODEL_SYNTHETIC_PREVIEW=SYNTHETIC-NOT-EVIDENCE
  binary_manifest="$(mktemp "${TMPDIR:-/tmp}/linggan-ocr-audit-binary.XXXXXX")"
  cargo test -p linggan-api --no-run --locked --message-format=json > "$binary_manifest"
  OCR_AUDIT_BROWSER_PROOF_BINARY="$(python3 - "$binary_manifest" <<'PYCODE'
import sys,json
for line in open(sys.argv[1]):
 d=json.loads(line)
 if d.get('reason')=='compiler-artifact' and d.get('profile',{}).get('test') and d.get('executable'): print(d['executable'])
PYCODE
)"
  rm -- "$binary_manifest"
  export OCR_AUDIT_BROWSER_PROOF_BINARY
  [[ -x "$OCR_AUDIT_BROWSER_PROOF_BINARY" ]] || exit 2
  ;;
 *) echo 'usage: test-ocr-quality-audit-postgres.sh [--browser]' >&2;exit 2;;
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
  if [[ -n "${OCR_AUDIT_SAVE_DIR:-}" && "$OCR_AUDIT_SAVE_DIR" == "${audit_tmp_root}/linggan-ocr-audit-"* && -d "${OCR_AUDIT_PROOF_OUTPUT:-}" ]];then
   mkdir -p "$OCR_AUDIT_SAVE_DIR"
   cp -R "$OCR_AUDIT_PROOF_OUTPUT/." "$OCR_AUDIT_SAVE_DIR/"
  fi
  if [[ "$code" != 0 && "${container_created:-0}" == 1 ]]; then docker logs --tail 6 "$proof_container" 2>&1; fi
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
export LINGGAN_PADDLE_OCR_PYTHON="$(command -v python3)"
export LINGGAN_PADDLE_VL_SCRIPT="$proof_assets/synthetic-bridge.py"
export OCR_AUDIT_PROOF_OUTPUT="$proof_assets/receipts"
cargo test -p linggan-worker --test ocr_quality_audit_postgres synthetic_audit_blind_read_review_report_and_withdrawal --locked -- --ignored --nocapture
if [[ -n "${OCR_AUDIT_SAVE_DIR:-}" ]];then
 [[ "$OCR_AUDIT_SAVE_DIR" == "${audit_tmp_root}/linggan-ocr-audit-"* ]] || exit 2
 mkdir -p "$OCR_AUDIT_SAVE_DIR"
 cp -R "$OCR_AUDIT_PROOF_OUTPUT/." "$OCR_AUDIT_SAVE_DIR/"
fi
