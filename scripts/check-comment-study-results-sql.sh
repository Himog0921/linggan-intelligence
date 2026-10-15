#!/usr/bin/env bash
# Isolated synthetic SQL proof. No production tables, raw data, or schema migrations.
set -euo pipefail
: "${COMMENT_RESULTS_TEST_DATABASE_URL:?Set an explicitly disposable PostgreSQL test database URL.}"
command -v psql >/dev/null || { echo 'psql is required' >&2; exit 2; }
root="$(cd "$(dirname "$0")/.." && pwd)"
fixture="$(mktemp "${TMPDIR:-/tmp}/comment-results-sql.XXXXXX")"
trap 'rm -f "$fixture"' EXIT
cat >"$fixture" <<'SQL'
BEGIN;
SET LOCAL search_path=pg_temp;
CREATE TEMP TABLE linggan_comment_study_problem(problem_ref uuid,domain_ref uuid,state text,created_at timestamptz,current_revision_ref uuid) ON COMMIT DROP;
CREATE FUNCTION pg_temp.uid(i int) RETURNS uuid IMMUTABLE LANGUAGE sql AS $$ SELECT ('00000000-0000-0000-0000-'||lpad(i::text,12,'0'))::uuid $$;
CREATE TEMP TABLE linggan_comment_study_problem_revision(revision_ref uuid,domain_ref uuid,identity_version int,created_at timestamptz,title text,definition text,core_frame jsonb,seed_signal_refs uuid[]) ON COMMIT DROP;
CREATE TEMP TABLE linggan_comment_study_problem_membership(problem_ref uuid,signal_ref uuid,created_at timestamptz) ON COMMIT DROP;
CREATE TEMP TABLE linggan_comment_study_signal(signal_ref uuid,target_ref uuid) ON COMMIT DROP;
CREATE TEMP TABLE linggan_comment_study_target(target_ref uuid,run_ref uuid,source_ref uuid,parent_source_ref uuid) ON COMMIT DROP;
CREATE TEMP TABLE linggan_comment_study_run(run_ref uuid,policy_ref uuid) ON COMMIT DROP;
CREATE TEMP TABLE linggan_comment_study_policy(policy_ref uuid,domain_ref uuid) ON COMMIT DROP;
CREATE TEMP TABLE linggan_material_comment(material_ref uuid,content_public_ref uuid,comment_external_id text,body_state text,body_text text,author_external_id text,author_display_name text) ON COMMIT DROP;
CREATE TEMP VIEW linggan_comment_study_current_comment AS SELECT * FROM linggan_material_comment;
CREATE TEMP TABLE linggan_material_comment_restriction(content_public_ref uuid,comment_external_id text) ON COMMIT DROP;
CREATE TEMP TABLE linggan_material_content(public_ref uuid,platform text) ON COMMIT DROP;
CREATE TEMP TABLE linggan_comment_study_effective_signal(signal_ref uuid,target_ref uuid,domain_ref uuid,content_public_ref uuid,comment_external_id text,eligibility_state text) ON COMMIT DROP;
INSERT INTO linggan_comment_study_policy VALUES (pg_temp.uid(1),pg_temp.uid(1)),(pg_temp.uid(2),pg_temp.uid(2));
INSERT INTO linggan_comment_study_run VALUES (pg_temp.uid(1),pg_temp.uid(1)),(pg_temp.uid(2),pg_temp.uid(2));
INSERT INTO linggan_comment_study_problem VALUES
 (pg_temp.uid(10),pg_temp.uid(1),'active','2026-09-01',pg_temp.uid(100)),
 (pg_temp.uid(20),pg_temp.uid(1),'active','2026-09-01',pg_temp.uid(200)),
 (pg_temp.uid(30),pg_temp.uid(2),'active','2026-09-01',pg_temp.uid(300)),
 (pg_temp.uid(40),pg_temp.uid(1),'support_insufficient','2026-09-01',pg_temp.uid(400));
INSERT INTO linggan_comment_study_problem_revision VALUES
 (pg_temp.uid(100),pg_temp.uid(1),2,'2026-10-14 11:00Z','Alpha','Recorded definition','{}',ARRAY[pg_temp.uid(1001)]),
 (pg_temp.uid(200),pg_temp.uid(1),1,'2026-09-01','Beta','Existing definition','{}',ARRAY[pg_temp.uid(2000)]),
 (pg_temp.uid(300),pg_temp.uid(2),1,'2026-09-01','Other domain','Not in scope','{}',ARRAY[pg_temp.uid(3000)]),
 (pg_temp.uid(400),pg_temp.uid(1),1,'2026-09-01','SECRET-TITLE','SECRET-DEFINITION','{}',ARRAY[pg_temp.uid(4000)]);
INSERT INTO linggan_material_content VALUES (pg_temp.uid(90),'test'),(pg_temp.uid(92),'test'),(pg_temp.uid(93),'test'),(pg_temp.uid(94),'test');
INSERT INTO linggan_material_comment VALUES
 (pg_temp.uid(9000),pg_temp.uid(90),'a','KNOWN','Synthetic original A','account-a','Synthetic A'),
 (pg_temp.uid(9020),pg_temp.uid(92),'b','KNOWN','Synthetic original B','account-b','Synthetic B'),
 (pg_temp.uid(9030),pg_temp.uid(93),'c','KNOWN','Other domain original','account-c','Synthetic C'),
 (pg_temp.uid(9040),pg_temp.uid(94),'secret','KNOWN','SECRET-VOICE','account-d','Synthetic D');
INSERT INTO linggan_comment_study_target VALUES
 (pg_temp.uid(1000),pg_temp.uid(1),pg_temp.uid(9000),NULL),(pg_temp.uid(1001),pg_temp.uid(1),pg_temp.uid(9000),NULL),
 (pg_temp.uid(2000),pg_temp.uid(1),pg_temp.uid(9020),NULL),(pg_temp.uid(3000),pg_temp.uid(2),pg_temp.uid(9030),NULL),(pg_temp.uid(4000),pg_temp.uid(1),pg_temp.uid(9040),NULL);
INSERT INTO linggan_comment_study_signal VALUES (pg_temp.uid(1000),pg_temp.uid(1000)),(pg_temp.uid(1001),pg_temp.uid(1001)),(pg_temp.uid(2000),pg_temp.uid(2000)),(pg_temp.uid(3000),pg_temp.uid(3000)),(pg_temp.uid(4000),pg_temp.uid(4000));
INSERT INTO linggan_comment_study_effective_signal VALUES
 (pg_temp.uid(1001),pg_temp.uid(1001),pg_temp.uid(1),pg_temp.uid(90),'a','eligible'),
 (pg_temp.uid(2000),pg_temp.uid(2000),pg_temp.uid(1),pg_temp.uid(92),'b','eligible'),
 (pg_temp.uid(3000),pg_temp.uid(3000),pg_temp.uid(2),pg_temp.uid(93),'c','eligible'),
 (pg_temp.uid(4000),pg_temp.uid(4000),pg_temp.uid(1),pg_temp.uid(94),'secret','eligible');
INSERT INTO linggan_comment_study_problem_membership VALUES
 (pg_temp.uid(10),pg_temp.uid(1000),'2026-09-01'),(pg_temp.uid(10),pg_temp.uid(1001),'2026-10-14'),
 (pg_temp.uid(20),pg_temp.uid(2000),'2026-10-14 09:00Z'),(pg_temp.uid(30),pg_temp.uid(3000),'2026-10-14'),(pg_temp.uid(40),pg_temp.uid(4000),'2026-10-14');
INSERT INTO linggan_material_comment_restriction VALUES (pg_temp.uid(94),'secret');
PREPARE cards(uuid,bigint,text,text,text,bigint,uuid,bigint) AS
SQL
cat "$root/apps/api/src/local_web/comment_study_problem_cards.sql" >>"$fixture"
cat >>"$fixture" <<'SQL'
;
CREATE TEMP TABLE all_results AS EXECUTE cards(pg_temp.uid(1),1792022400000000,'','all','recent',NULL,NULL,20);
DO $$
DECLARE b jsonb; p jsonb;
BEGIN
 SELECT jsonb_build_object INTO b FROM all_results;
 ASSERT (b->>'totalCount')::int=3,'domain scope';
 ASSERT jsonb_array_length(b->'problems')=3,'full result count';
 SELECT value INTO p FROM jsonb_array_elements(b->'problems') WHERE value->>'problemRef'=pg_temp.uid(10)::text;
 ASSERT (p->>'supportCommentCount')::int=1,'reanalysis must not count twice';
 ASSERT (p->>'recentAddedSupportCommentCount')::int=0,'old comment reanalysis is not recent evidence';
 ASSERT p->>'changeKind'='definition_revision','stored revision classification';
 SELECT value INTO p FROM jsonb_array_elements(b->'problems') WHERE value->>'problemRef'=pg_temp.uid(20)::text;
 ASSERT p->>'changeKind'='evidence_added','materials distinct from definitions';
 SELECT value INTO p FROM jsonb_array_elements(b->'problems') WHERE value->>'problemRef'=pg_temp.uid(40)::text;
 ASSERT p->'title'='null'::jsonb AND p->'definition'='null'::jsonb AND p->'preview'='null'::jsonb,'restriction propagation';
 ASSERT (p->>'supportCommentCount')::int=0,'restricted voice excluded';
END $$;
CREATE TEMP TABLE changes AS EXECUTE cards(pg_temp.uid(1),1792022400000000,'','changes','recent',NULL,NULL,20);
DO $$ BEGIN ASSERT (SELECT (jsonb_build_object->>'totalCount')::int FROM changes)=2,'global changes filtering'; END $$;
CREATE TEMP TABLE filtered AS EXECUTE cards(pg_temp.uid(1),1792022400000000,'Beta','all','recent',NULL,NULL,1);
DO $$ BEGIN ASSERT (SELECT (jsonb_build_object->>'totalCount')::int FROM filtered)=1,'search before pagination'; END $$;
CREATE TEMP TABLE restricted_search AS EXECUTE cards(pg_temp.uid(1),1792022400000000,'SECRET','all','recent',NULL,NULL,20);
DO $$ BEGIN ASSERT (SELECT (jsonb_build_object->>'totalCount')::int FROM restricted_search)=0,'no restricted text search'; END $$;
CREATE TEMP TABLE first_page AS EXECUTE cards(pg_temp.uid(1),1792022400000000,'','all','recent',NULL,NULL,2);
DO $$
DECLARE last_row jsonb; b jsonb;
BEGIN
 SELECT jsonb_build_object INTO b FROM first_page;
 ASSERT (b->>'totalCount')::int=3 AND jsonb_array_length(b->'problems')=2,'total independent of page';
 last_row=b->'problems'->1;
 EXECUTE format('CREATE TEMP TABLE next_page AS EXECUTE cards(pg_temp.uid(1),1792022400000000,%L,%L,%L,%s,%L,2)', '', 'all','recent',last_row->>'sortKey',last_row->>'problemRef');
 ASSERT (SELECT jsonb_array_length(jsonb_build_object->'problems') FROM next_page)=1,'keyset continuation';
END $$;
-- Live source eligibility changes the ordering revision; old cursors are rejected in the adapter.
INSERT INTO linggan_material_comment_restriction VALUES (pg_temp.uid(92),'b');
CREATE TEMP TABLE after_restriction AS EXECUTE cards(pg_temp.uid(1),1792022400000000,'','all','recent',NULL,NULL,20);
DO $$ BEGIN
 ASSERT (SELECT jsonb_build_object->>'scopeRevision' FROM all_results) IS DISTINCT FROM
        (SELECT jsonb_build_object->>'scopeRevision' FROM after_restriction),'scope revision follows ordering changes';
END $$;
ROLLBACK;
\echo PASS: synthetic SQL assertions; all temporary objects rolled back.
SQL
psql -X --dbname="$COMMENT_RESULTS_TEST_DATABASE_URL" -v ON_ERROR_STOP=1 --file="$fixture"
