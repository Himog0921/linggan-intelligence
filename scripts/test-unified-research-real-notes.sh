#!/usr/bin/env bash
# Bounded real-material flow proof. Source DB is read-only; models are owned loopback fixtures.
set -euo pipefail
project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"
source scripts/cargo-proof.sh
[[ -d apps/pi-adapter/node_modules ]] || { echo 'Install the pinned adapter dependencies before this proof.' >&2; exit 1; }
source_container=linggan-intelligence-postgres-1
source_database=linggan_intelligence_dev
source_user=linggan_dev_admin
suffix="$(date -u +%Y%m%d%H%M%S)_$$_$(openssl rand -hex 4)"
proof_container="linggan-unified-real-proof-${suffix}"
proof_volume="${proof_container}-data"
proof_database="linggan_unified_real_proof_${suffix}"
proof_user=unified_real_proof
proof_password="$(openssl rand -hex 24)"
proof_dir="$project_root/artifacts/private/unified-research-automation/real-notes-${suffix}"
mkdir -p "$proof_dir"
[[ "$proof_container" =~ ^linggan-unified-real-proof-[a-zA-Z0-9_-]+$ && "$proof_volume" == "$proof_container-data" ]] || exit 1
cleanup(){
 task_exit=$?;trap - EXIT
 cleanup_failed=0
 [[ "${container_created:-0}" == 0 ]] || docker rm -f "$proof_container" >/dev/null || cleanup_failed=1
 [[ "${volume_created:-0}" == 0 ]] || docker volume rm "$proof_volume" >/dev/null || cleanup_failed=1
 [[ "$cleanup_failed" == 0 ]] || { echo 'Owned proof resource cleanup failed' >&2;exit 1; }
 echo "Real-note proof cleanup verified; evidence directory: $proof_dir"
 exit "$task_exit"
}
trap cleanup EXIT
source_digest(){
 docker exec -i "$source_container" psql -X -U "$source_user" -d "$source_database" -At -v ON_ERROR_STOP=1 <<'SQL'
SELECT jsonb_build_object('works',(SELECT jsonb_build_array(count(*),sum(hashtextextended(to_jsonb(t)::text,0)::numeric)) FROM linggan_material_content t),'comments',(SELECT jsonb_build_array(count(*),sum(hashtextextended(to_jsonb(t)::text,0)::numeric)) FROM linggan_material_comment t),'media',(SELECT jsonb_build_array(count(*),sum(hashtextextended(to_jsonb(t)::text,0)::numeric)) FROM linggan_media_materialization t),'readings',(SELECT jsonb_build_array(count(*),sum(hashtextextended(to_jsonb(t)::text,0)::numeric)) FROM linggan_material_reading t));
SQL
}
source_digest > "$proof_dir/source-before.json"
docker volume create "$proof_volume" >/dev/null
volume_created=1
docker run -d --name "$proof_container" --mount "type=volume,source=$proof_volume,target=/var/lib/postgresql/data" --env POSTGRES_DB="$proof_database" --env POSTGRES_USER="$proof_user" --env POSTGRES_PASSWORD="$proof_password" --publish 127.0.0.1::5432 linggan-intelligence-postgres-pgvector:16.14-v0.8.0-r1 -c max_locks_per_transaction=1024 >/dev/null
container_created=1
for _ in {1..30};do docker exec "$proof_container" pg_isready -h 127.0.0.1 -U "$proof_user" -d "$proof_database" >/dev/null 2>&1 && break;sleep 1;done
docker exec "$proof_container" pg_isready -h 127.0.0.1 -U "$proof_user" -d "$proof_database" >/dev/null
# Neither dump nor restored raw material is written to a host artifact or sent off this machine.
docker exec "$source_container" pg_dump -U "$source_user" -d "$source_database" --format=custom --no-owner --no-acl | docker exec -i "$proof_container" pg_restore -U "$proof_user" -d "$proof_database" --no-owner --no-acl --exit-on-error
for name in 0169_ai_manual_skill_activation 0170_ai_prompt_versions 0171_ai_writing_comparison_prompts 0172_research_method_settings 0173_creator_research_execution 0174_comment_research_execution 0175_creator_account_research 0176_topic_research_execution 0177_comment_problem_workflow 0178_creator_manual_annotations 0179_retire_research_relations;do
 migration="$project_root/database/migrations/$name.sql"
 checksum="$(shasum -a 256 "$migration" | awk '{print $1}')"
 { printf 'BEGIN;\n';cat "$migration";printf "\nINSERT INTO linggan_local_schema_migration(migration_id,migration_sha256) VALUES('%s','%s');\nCOMMIT;\n" "$name" "$checksum"; } | docker exec -i "$proof_container" psql -X -U "$proof_user" -d "$proof_database" -v ON_ERROR_STOP=1 >/dev/null
done
legacy_count="$(docker exec "$proof_container" psql -X -U "$proof_user" -d "$proof_database" -At -c "SELECT count(*) FROM information_schema.tables WHERE table_schema='public' AND (table_name LIKE 'linggan_comment_study_%' OR table_name LIKE 'linggan_topic_map_research_%' OR table_name IN('linggan_creator_discovery_policy','linggan_creator_discovery_work_analysis','linggan_creator_discovery_author_analysis'));")"
[[ "$legacy_count" == 0 ]] || { echo 'Retired relations remain in restored proof' >&2;exit 1; }
port="$(docker port "$proof_container" 5432/tcp | sed -n 's/^127\.0\.0\.1:\([0-9][0-9]*\)$/\1/p')"
[[ "$port" =~ ^[0-9]+$ ]] || exit 1
export LOCAL_001_PROOF_DATABASE_URL="postgresql://${proof_user}:${proof_password}@127.0.0.1:${port}/${proof_database}"
export UNIFIED_RESEARCH_REAL_WORK="${UNIFIED_RESEARCH_REAL_WORK:-31950b3e-6f18-47aa-b073-a6a86dd7c65a}"
export UNIFIED_RESEARCH_REPORT="$proof_dir/result.json"
export AI_FOUNDATION_PROOF_NODE="$(command -v node)"
export LINGGAN_CREATION_ASSET_ROOT="$proof_dir/assets"
mkdir -p "$LINGGAN_CREATION_ASSET_ROOT"
export RUST_TEST_THREADS=1
cargo_proof -p linggan-intelligence --test unified_research_real_notes --locked -- --ignored --nocapture > "$proof_dir/execution.log" 2>&1
source_digest > "$proof_dir/source-after.json"
cmp "$proof_dir/source-before.json" "$proof_dir/source-after.json"
echo 'Real-note flow passed on an isolated restored DB with synthetic model responses; source tables unchanged. This is not model-quality acceptance.'
