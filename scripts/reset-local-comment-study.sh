#!/usr/bin/env bash
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

if [[ "${1:-}" != "--confirm-local-comment-study-derived-reset" || $# -ne 1 ]]; then
  echo "refusing reset: pass exactly --confirm-local-comment-study-derived-reset" >&2
  exit 1
fi

"$project_root/scripts/setup-local-env.sh" >/dev/null
set -a
source "$project_root/.env"
set +a

if [[ "${POSTGRES_USER:-}" != "linggan_dev_admin" || -z "${POSTGRES_DB:-}" || ! "${POSTGRES_PORT:-}" =~ ^[1-9][0-9]{0,4}$ ]]; then
  echo "refusing reset: local Linggan runtime environment is not configured" >&2
  exit 1
fi

if [[ "${LINGGAN_CONFIRM_COMMENT_STUDY_RESET:-}" != "DELETE_DERIVED_COMMENT_STUDY" ]]; then
  echo "refusing reset: set LINGGAN_CONFIRM_COMMENT_STUDY_RESET=DELETE_DERIVED_COMMENT_STUDY for this one command" >&2
  exit 1
fi

"$project_root/scripts/dev-db.sh" up >/dev/null
published_address="$(docker compose port postgres 5432)"
if [[ "$published_address" != "127.0.0.1:${POSTGRES_PORT}" ]]; then
  echo "refusing reset: Docker PostgreSQL port does not match the configured local runtime" >&2
  exit 1
fi

{
  printf 'BEGIN;\n'
  cat "$project_root/database/bootstrap/comment-study-reset.sql"
  cat "$project_root/database/bootstrap/comment-study-001.sql"
  cat <<'SQL'
INSERT INTO linggan_comment_study_reset_receipt(
  receipt_ref,reset_scope,old_relation_counts,preserved_relation_counts,requested_by
)
SELECT gen_random_uuid(),'local_comment_study_derived_only',old_relation_counts,
  jsonb_build_object(
    'content',(SELECT count(*) FROM linggan_material_content),
    'comment',(SELECT count(*) FROM linggan_material_comment),
    'capturePackage',(SELECT count(*) FROM linggan_runtime_capture_package),
    'restriction',(SELECT count(*) FROM linggan_material_comment_restriction),
    'mediaDisposition',(SELECT count(*) FROM linggan_current_material_media_disposition)
  ),'explicit_local_reset' FROM comment_study_reset_counts;
COMMIT;
SQL
} | docker compose exec -T postgres psql -X -v ON_ERROR_STOP=1 -U "$POSTGRES_USER" -d "$POSTGRES_DB"

echo "local comment-study derived layer reset completed; raw Evidence and qualification facts were retained"
