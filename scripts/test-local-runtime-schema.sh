#!/usr/bin/env bash
# Real ordered migration body and repeat-run proof; transport is bound to an owned container.
# Does not exercise launchd/Compose credentials and never repairs the shared database role.
set -euo pipefail
project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"
suffix="$(openssl rand -hex 8)"
proof_container="linggan-runtime-schema-proof-$suffix"
proof_volume="$proof_container-data"
proof_database="linggan_intelligence_runtime_proof_$suffix"
proof_user=linggan_dev_admin
proof_password="$(openssl rand -hex 24)"
proof_dir="$project_root/artifacts/private/unified-research-automation/runtime-schema-$suffix"
mkdir -p "$proof_dir"
cleanup(){
 task_exit=$?;trap - EXIT
 [[ "${created:-0}" == 0 ]] || docker rm -f "$proof_container" >/dev/null || exit 1
 [[ "${volume_created:-0}" == 0 ]] || docker volume rm "$proof_volume" >/dev/null || exit 1
 echo "Owned runtime schema proof resources removed: $proof_dir"
 exit "$task_exit"
}
trap cleanup EXIT
"$project_root/scripts/runtime/build-pgvector-image.sh" --ensure
docker volume create "$proof_volume" >/dev/null;volume_created=1
docker run -d --name "$proof_container" --mount "type=volume,source=$proof_volume,target=/var/lib/postgresql/data" --env POSTGRES_DB="$proof_database" --env POSTGRES_USER="$proof_user" --env POSTGRES_PASSWORD="$proof_password" linggan-intelligence-postgres-pgvector:16.14-v0.8.0-r1 -c max_locks_per_transaction=1024 >/dev/null
created=1
for _ in {1..30};do docker exec "$proof_container" pg_isready -U "$proof_user" -d "$proof_database" >/dev/null 2>&1 && break;sleep 1;done
psql_in_container(){ docker exec -i "$proof_container" psql -X -v ON_ERROR_STOP=1 -U "$proof_user" -d "$proof_database" "$@"; }
# Evaluate the source functions, with only the migration body's connection/startup
# preamble omitted. Production ordering, historical scaffold and checksums are unchanged.
python3 - "$project_root/scripts/local-runtime.sh" > "$proof_dir/migration-functions.sh" <<'PY'
import re,sys
source=open(sys.argv[1]).read()
for name in ['prepare_historical_comment_schema','apply_migration_once','migrate']:
 match=re.search(r'^'+name+r'\(\) \{\n.*?^\}',source,re.M|re.S)
 if not match:raise SystemExit('missing runtime function '+name)
 body=match.group()
 if name=='migrate':
  start=body.index('  ensure_migration_ledger\n')
  body='migrate() {\n'+body[start:]
 print(body)
PY
ensure_migration_ledger(){ psql_in_container >/dev/null <<'SQL'
CREATE TABLE IF NOT EXISTS linggan_local_schema_migration(migration_id text PRIMARY KEY,migration_sha256 text NOT NULL CHECK(migration_sha256 ~ '^[0-9a-f]{64}$'),applied_at timestamptz NOT NULL DEFAULT clock_timestamp());
SQL
}
# shellcheck disable=SC1090
source "$proof_dir/migration-functions.sh"
migrate > "$proof_dir/first.log" 2>&1
migrate > "$proof_dir/repeat.log" 2>&1
legacy="$(psql_in_container -Atc "SELECT count(*) FROM information_schema.tables WHERE table_schema='public' AND (table_name LIKE 'linggan_comment_study_%' OR table_name LIKE 'linggan_topic_map_research_%' OR table_name IN('linggan_creator_discovery_policy','linggan_creator_discovery_work_analysis','linggan_creator_discovery_author_analysis'))")"
[[ "$legacy" == 0 ]] || { echo 'Retired research tables remain' >&2;exit 1; }
head="$(psql_in_container -Atc "SELECT max(migration_id) FROM linggan_local_schema_migration")"
[[ "$head" == 0179_retire_research_relations ]] || exit 1
printf '{"head":"%s","retiredTables":%s,"freshAndRepeatPassed":true,"scope":"ordered schema migration only"}\n' "$head" "$legacy" > "$proof_dir/result.json"
echo 'Fresh and repeat runtime schema migration passed; retired tables absent.'
