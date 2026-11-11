#!/usr/bin/env bash
# Real ordered migration body and repeat-run proof; transport is bound to an owned container.
# Does not exercise launchd/Compose credentials and never repairs the shared database role.
set -euo pipefail
populated_upgrade=0
case "${1:-}" in
  '') [[ $# == 0 ]] || exit 2 ;;
  --content-understanding-upgrade) [[ $# == 1 ]] || exit 2; populated_upgrade=1 ;;
  *) printf 'Usage: %s [--content-understanding-upgrade]\n' "$0" >&2; exit 2 ;;
esac
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
# The image's init server accepts Unix-socket clients before it is stopped and restarted; only the final
# server listens on TCP, so readiness is judged there (same rule as scripts/test-web-app-postgres.sh).
for _ in {1..30};do docker exec "$proof_container" pg_isready -h 127.0.0.1 -p 5432 -U "$proof_user" -d "$proof_database" >/dev/null 2>&1 && break;sleep 1;done
docker exec "$proof_container" pg_isready -h 127.0.0.1 -p 5432 -U "$proof_user" -d "$proof_database" >/dev/null 2>&1 || { echo 'runtime schema proof PostgreSQL did not become TCP-ready within 30 seconds' >&2; exit 1; }
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
 if name=='migrate':
  boundary='  apply_migration_once "0196_content_understanding" '
  if boundary not in body:raise SystemExit('missing content understanding migration boundary')
  prefix=body[:body.index(boundary)]
  if '  apply_migration_once "0195_observation_transition_immutable" ' not in prefix:
   raise SystemExit('missing pre-upgrade migration')
  print(prefix.replace('migrate() {','migrate_before_content_understanding() {',1)+'}')
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
# The ordered chain must reach the last migration the runtime registers, whichever that is.
expected_head="$(sed -n 's/^  apply_migration_once "\([^"]*\)" .*/\1/p' "$project_root/scripts/local-runtime.sh" | tail -n 1)"
[[ -n "$expected_head" && "$head" == "$expected_head" ]] || { echo "Migration head $head is not the last registered migration $expected_head" >&2; exit 1; }
printf '{"head":"%s","retiredTables":%s,"freshAndRepeatPassed":true,"scope":"ordered schema migration only"}\n' "$head" "$legacy" > "$proof_dir/result.json"
echo 'Fresh and repeat runtime schema migration passed; retired tables absent.'
if [[ "$populated_upgrade" == 1 ]]; then
  upgrade_database="${proof_database}_upgrade"
  psql_in_container -c "CREATE DATABASE $upgrade_database" >/dev/null
  proof_database="$upgrade_database"
  migrate_before_content_understanding > "$proof_dir/upgrade-baseline.log" 2>&1
  [[ "$(psql_in_container -Atc 'SELECT max(migration_id) FROM linggan_local_schema_migration')" == 0195_observation_transition_immutable ]] || exit 1
  psql_in_container > "$proof_dir/upgrade-seed.log" 2>&1 <<'SQL'
BEGIN;
DO $$
DECLARE connection uuid := gen_random_uuid(); version uuid := gen_random_uuid();
 command uuid; batch uuid; config uuid; target record;
BEGIN
 INSERT INTO linggan_model_connection(connection_ref) VALUES(connection);
 INSERT INTO linggan_model_connection_version(version_ref,connection_ref,revision,name,api,base_url,local_endpoint,secret_ref)
 VALUES(version,connection,1,'Synthetic upgrade only','openai-responses','http://127.0.0.1:9',true,gen_random_uuid());
 FOR target IN SELECT * FROM (VALUES
  ('note_interpretation','note_material'),('note_interpretation','comment_material'),
  ('creation_title','selected_writing_material'),('model_probe','synthetic_probe')
 ) AS pairs(task,material) LOOP
  INSERT INTO linggan_model_egress_event(event_ref,endpoint_ref,task_key,material_class,allowed,actor_kind,reason)
  VALUES(gen_random_uuid(),version,target.task,target.material,true,'human','Synthetic pre-upgrade permission');
 END LOOP;
 FOR target IN SELECT * FROM (VALUES
  ('note_interpretation','interactive'),('creation_title','interactive'),('model_probe','probe')
 ) AS pairs(task,kind) LOOP
  command := gen_random_uuid(); batch := gen_random_uuid();
  SELECT current_config_ref INTO STRICT config FROM linggan_ai_task_setting WHERE task_key=target.task;
  INSERT INTO linggan_ai_command(command_ref,operation,payload,receipt)
  VALUES(command,'synthetic_upgrade_seed','{}','{}');
  INSERT INTO linggan_ai_batch(batch_ref,task_key,task_config_ref,kind,inputs_sha256,item_count,command_ref)
  VALUES(batch,target.task,config,target.kind,repeat('a',64),1,command);
  INSERT INTO linggan_ai_batch_item(item_ref,batch_ref,position,item_key,input_ref)
  VALUES(gen_random_uuid(),batch,0,'synthetic-upgrade','{"kind":"synthetic_upgrade"}');
 END LOOP;
 command := gen_random_uuid();
 INSERT INTO linggan_ai_command(command_ref,operation,payload,receipt)
 VALUES(command,'synthetic_upgrade_controls','{}','{}');
 UPDATE linggan_ai_control SET enabled=true,revision=revision+1,actor_kind='human',
  reason='Synthetic pre-upgrade control',command_ref=command WHERE scope_key='global';
 command := gen_random_uuid();
 INSERT INTO linggan_ai_command(command_ref,operation,payload,receipt)
 VALUES(command,'synthetic_upgrade_controls','{}','{}');
 UPDATE linggan_ai_control SET enabled=true,revision=revision+1,actor_kind='human',
  reason='Synthetic pre-upgrade control',command_ref=command WHERE task_key='note_interpretation';
END $$;
COMMIT;
CREATE SCHEMA upgrade_proof;
CREATE TABLE upgrade_proof.baseline_table AS
 SELECT tablename FROM pg_tables WHERE schemaname='public' AND tablename<>'linggan_local_schema_migration';
CREATE FUNCTION upgrade_proof.snapshot() RETURNS TABLE(table_name text,row_count bigint,row_hash text)
 LANGUAGE plpgsql AS $$
DECLARE entry record; predicate text;
BEGIN
 FOR entry IN SELECT tablename FROM upgrade_proof.baseline_table ORDER BY tablename LOOP
  predicate := CASE
   WHEN entry.tablename IN ('linggan_ai_task','linggan_ai_control','linggan_ai_task_round_state',
    'linggan_ai_prompt_version','linggan_ai_task_config','linggan_ai_task_setting','linggan_skill_binding')
    THEN 'WHERE task_key IS DISTINCT FROM ''content_understanding'' AND task_key IS DISTINCT FROM ''content_understanding_deep'''
   WHEN entry.tablename='linggan_ai_failure_code'
    THEN 'WHERE code NOT IN (''content_understanding_publication_withheld'',''content_understanding_deep_insufficient_evidence'')'
   ELSE '' END;
  RETURN QUERY EXECUTE format(
   'SELECT %L::text,count(*),md5(COALESCE(string_agg(to_jsonb(row)::text,E''\n'' ORDER BY to_jsonb(row)::text),'''')) FROM public.%I row %s',
   entry.tablename,entry.tablename,predicate);
 END LOOP;
END $$;
DO $$ BEGIN
 IF (SELECT count(*) FROM linggan_ai_batch)<>3
  OR (SELECT count(*) FROM linggan_ai_batch_item)<>3
  OR (SELECT count(*) FROM linggan_model_egress_event)<>4
  OR to_regclass('public.linggan_content_understanding_request') IS NOT NULL
 THEN RAISE EXCEPTION 'populated_upgrade_baseline_invalid'; END IF;
END $$;
SQL
  psql_in_container -Atc 'SELECT row_to_json(snapshot) FROM upgrade_proof.snapshot()' > "$proof_dir/upgrade-before.jsonl"
  migrate > "$proof_dir/upgrade-apply.log" 2>&1
  psql_in_container -Atc 'SELECT row_to_json(snapshot) FROM upgrade_proof.snapshot()' > "$proof_dir/upgrade-after.jsonl"
  cmp "$proof_dir/upgrade-before.jsonl" "$proof_dir/upgrade-after.jsonl"
  psql_in_container > "$proof_dir/upgrade-assertions.log" 2>&1 <<'SQL'
DO $$ BEGIN
 IF (
  SELECT count(*) FROM linggan_ai_task_setting setting
  JOIN linggan_ai_task_config config ON config.task_config_ref=setting.current_config_ref
  JOIN linggan_ai_control control ON control.task_key=setting.task_key
  WHERE setting.task_key IN ('content_understanding','content_understanding_deep') AND NOT control.enabled
   AND config.model_config_ref IS NULL AND config.skill_version_ref IS NULL
   AND config.budget_mode='unconfigured'
 ) <> 2 THEN RAISE EXCEPTION 'new_task_was_enabled_or_configured'; END IF;
 IF EXISTS(SELECT 1 FROM linggan_model_egress_event WHERE task_key IN ('content_understanding','content_understanding_deep'))
  OR EXISTS(SELECT 1 FROM linggan_ai_batch WHERE task_key IN ('content_understanding','content_understanding_deep'))
  OR EXISTS(SELECT 1 FROM linggan_content_understanding_request)
  OR EXISTS(SELECT 1 FROM linggan_content_understanding_result)
  OR EXISTS(SELECT 1 FROM linggan_content_understanding_deep_request)
  OR EXISTS(SELECT 1 FROM linggan_content_understanding_deep)
  OR EXISTS(SELECT 1 FROM linggan_model_invocation)
 THEN RAISE EXCEPTION 'upgrade_created_work_or_egress'; END IF;
END $$;
SQL
  migrate > "$proof_dir/upgrade-repeat.log" 2>&1
  psql_in_container -Atc 'SELECT row_to_json(snapshot) FROM upgrade_proof.snapshot()' > "$proof_dir/upgrade-repeat.jsonl"
  cmp "$proof_dir/upgrade-before.jsonl" "$proof_dir/upgrade-repeat.jsonl"
  [[ "$(psql_in_container -Atc 'SELECT max(migration_id) FROM linggan_local_schema_migration')" == "$expected_head" ]] || exit 1
  python3 - "$proof_dir/result.json" <<'PY'
import json,sys
path=sys.argv[1]
with open(path) as f:result=json.load(f)
result.update(populatedUpgradePassed=True,baselineHead='0195_observation_transition_immutable',
    syntheticBatches=3,syntheticItems=3,syntheticEgressPermissions=4,
    scope='fresh, repeat and populated synthetic upgrade; no production data or external calls')
with open(path,'w') as f:json.dump(result,f,ensure_ascii=False,indent=2);f.write('\n')
PY
  echo 'Populated content-understanding upgrade and repeat passed; prior rows unchanged, new tasks disabled and no work dispatched.'
fi
