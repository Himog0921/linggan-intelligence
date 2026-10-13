-- COLLECTION-JTBD-001: user intent and immutable scope, over the existing execution chain.
-- No historical jobs are started or reconstructed by this migration.
CREATE TABLE collection_run (
 run_ref uuid PRIMARY KEY,
 idempotency_key uuid NOT NULL UNIQUE,
 request_hash text NOT NULL CHECK(length(request_hash)=64),
 domain_ref uuid NOT NULL REFERENCES observation_domain,
 source_kind text NOT NULL CHECK(source_kind IN('temporary_keyword','observation_target','selected_works','archive','patrol','supplement')),
 -- Historical source choice survives removal of the long-term control target. Creation
 -- validates and locks the live target; deletion stops its runs without erasing that choice.
 source_target_ref uuid,
 execution_target_ref uuid REFERENCES collection_observation_target,
 parent_run_ref uuid REFERENCES collection_run,
 name text NOT NULL CHECK(length(name) BETWEEN 1 AND 240),
 search_query text,
 ranking text CHECK(ranking IN('comprehensive','latest','most_liked','most_collected','most_commented')),
 work_limit integer NOT NULL CHECK(work_limit BETWEEN 1 AND 200),
 comment_mode text NOT NULL CHECK(comment_mode IN('sample_30','all_public')),
 authorization_ref uuid REFERENCES collection_acquisition_authorization,
 control_intent text NOT NULL DEFAULT 'running' CHECK(control_intent IN('running','paused','stopped')),
 revision integer NOT NULL DEFAULT 1 CHECK(revision>0),
 phase text NOT NULL DEFAULT 'discovery' CHECK(phase IN('discovery','materials','ended','blocked')),
 scope_frozen_at timestamptz,
 discovery_reason text,
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 updated_at timestamptz NOT NULL DEFAULT scope_001_now(),
 ended_at timestamptz,
 CHECK(source_kind<>'temporary_keyword' OR (search_query IS NOT NULL AND length(btrim(search_query)) BETWEEN 1 AND 80)),
 CHECK(source_kind<>'observation_target' OR source_target_ref IS NOT NULL)
);
CREATE INDEX collection_run_domain_page ON collection_run(domain_ref,created_at DESC,run_ref);
CREATE TABLE collection_run_work (
 run_ref uuid NOT NULL REFERENCES collection_run,
 content_public_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),
 ordinal integer NOT NULL CHECK(ordinal BETWEEN 1 AND 200),
 discovery_package_ref uuid REFERENCES linggan_runtime_capture_package(package_ref),
 reused_detail_package_ref uuid REFERENCES linggan_runtime_capture_package(package_ref),
 checkpoint jsonb NOT NULL DEFAULT '{}'::jsonb CHECK(jsonb_typeof(checkpoint)='object'),
 gap_reason text,
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 PRIMARY KEY(run_ref,content_public_ref), UNIQUE(run_ref,ordinal)
);
CREATE INDEX collection_run_work_content ON collection_run_work(content_public_ref,run_ref);
CREATE TABLE collection_run_order (
 run_ref uuid NOT NULL REFERENCES collection_run,
 request_ref uuid NOT NULL UNIQUE REFERENCES collection_acquisition_request,
 work_order_ref uuid UNIQUE REFERENCES collection_work_order,
 content_public_ref uuid REFERENCES linggan_material_content(public_ref),
 stage text NOT NULL CHECK(stage IN('discovery','detail','comments','supplement')),
 segment integer NOT NULL DEFAULT 1 CHECK(segment>0),
 created_at timestamptz NOT NULL DEFAULT scope_001_now(),
 PRIMARY KEY(run_ref,request_ref)
);
CREATE INDEX collection_run_order_run ON collection_run_order(run_ref,stage,content_public_ref,segment);
CREATE TABLE collection_run_command (
 command_ref uuid PRIMARY KEY,
 run_ref uuid NOT NULL REFERENCES collection_run,
 command_hash text NOT NULL CHECK(length(command_hash)=64),
 receipt jsonb NOT NULL CHECK(jsonb_typeof(receipt)='object'),
 created_at timestamptz NOT NULL DEFAULT scope_001_now()
);
CREATE TRIGGER collection_run_command_immutable BEFORE UPDATE OR DELETE ON collection_run_command
 FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE TRIGGER collection_run_order_immutable BEFORE UPDATE OR DELETE ON collection_run_order
 FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();
CREATE FUNCTION collection_run_scope_frozen() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF ROW(NEW.run_ref,NEW.idempotency_key,NEW.request_hash,NEW.domain_ref,NEW.source_kind,
        NEW.source_target_ref,NEW.search_query,NEW.ranking,NEW.work_limit,NEW.comment_mode,NEW.created_at)
    IS DISTINCT FROM ROW(OLD.run_ref,OLD.idempotency_key,OLD.request_hash,OLD.domain_ref,OLD.source_kind,
        OLD.source_target_ref,OLD.search_query,OLD.ranking,OLD.work_limit,OLD.comment_mode,OLD.created_at)
 THEN RAISE EXCEPTION 'collection run intent is frozen' USING ERRCODE='23514'; END IF;
 IF OLD.control_intent='stopped' AND NEW.control_intent<>'stopped'
 THEN RAISE EXCEPTION 'stopped collection run cannot restart' USING ERRCODE='23514'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER collection_run_scope_frozen BEFORE UPDATE ON collection_run FOR EACH ROW EXECUTE FUNCTION collection_run_scope_frozen();
CREATE FUNCTION collection_run_work_frozen() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE bound integer; frozen timestamptz;
BEGIN
 IF TG_OP='DELETE' THEN RAISE EXCEPTION 'collection run scope cannot be removed' USING ERRCODE='23514'; END IF;
 SELECT work_limit,scope_frozen_at INTO bound,frozen FROM collection_run WHERE run_ref=NEW.run_ref FOR UPDATE;
 IF TG_OP='INSERT' AND (frozen IS NOT NULL OR NEW.ordinal>bound)
 THEN RAISE EXCEPTION 'collection run scope already frozen or outside bound' USING ERRCODE='23514'; END IF;
 IF TG_OP='UPDATE' AND ROW(NEW.run_ref,NEW.content_public_ref,NEW.ordinal,NEW.discovery_package_ref,NEW.created_at)
 IS DISTINCT FROM ROW(OLD.run_ref,OLD.content_public_ref,OLD.ordinal,OLD.discovery_package_ref,OLD.created_at)
 THEN RAISE EXCEPTION 'collection run member identity is frozen' USING ERRCODE='23514'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER collection_run_work_frozen BEFORE INSERT OR UPDATE OR DELETE ON collection_run_work FOR EACH ROW EXECUTE FUNCTION collection_run_work_frozen();

ALTER TABLE collection_observation_target DROP CONSTRAINT collection_observation_target_purpose_kind_check;
ALTER TABLE collection_observation_target ADD CONSTRAINT collection_observation_target_purpose_kind_check
 CHECK(purpose_kind IN('long_term','research_round','single_run'));
ALTER TABLE collection_observation_target ADD COLUMN purpose_run_ref uuid REFERENCES collection_run;
ALTER TABLE collection_observation_target ADD CONSTRAINT collection_single_run_target_purpose
 CHECK((purpose_kind='single_run')=(purpose_run_ref IS NOT NULL));
CREATE UNIQUE INDEX collection_single_run_execution_target ON collection_observation_target(purpose_run_ref) WHERE purpose_kind='single_run';
CREATE OR REPLACE FUNCTION topic_map_temporary_target_no_monitor() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.purpose_kind IN('research_round','single_run') AND (NEW.monitoring_enabled OR NEW.lifecycle_state='monitoring')
 THEN RAISE EXCEPTION 'temporary execution targets cannot be monitored' USING ERRCODE='23514'; END IF;
 RETURN NEW;
END $$;
CREATE OR REPLACE FUNCTION topic_map_temporary_target_no_rule() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF EXISTS(SELECT 1 FROM collection_observation_target WHERE target_ref=NEW.target_ref AND purpose_kind IN('research_round','single_run'))
 THEN RAISE EXCEPTION 'temporary execution targets cannot have monitoring rules' USING ERRCODE='23514'; END IF;
 RETURN NEW;
END $$;
CREATE FUNCTION collection_single_run_target_identity_frozen() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF OLD.purpose_kind='single_run' AND ROW(NEW.purpose_kind,NEW.purpose_run_ref,NEW.identity_key,NEW.platform,NEW.target_kind)
 IS DISTINCT FROM ROW(OLD.purpose_kind,OLD.purpose_run_ref,OLD.identity_key,OLD.platform,OLD.target_kind)
 THEN RAISE EXCEPTION 'single run source identity is frozen' USING ERRCODE='23514'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER collection_single_run_target_identity_frozen BEFORE UPDATE ON collection_observation_target FOR EACH ROW EXECUTE FUNCTION collection_single_run_target_identity_frozen();
CREATE FUNCTION collection_source_target_deleted() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE linked uuid;
BEGIN
 FOR linked IN SELECT run_ref FROM collection_run WHERE source_target_ref=OLD.target_ref AND control_intent<>'stopped' ORDER BY run_ref LOOP
  -- Target deletion already holds the target lock; do not invert the run→target
  -- scheduler lock order. A competing tick makes this reversible write retry.
  IF NOT pg_try_advisory_xact_lock(hashtextextended(linked::text,400))
  THEN RAISE EXCEPTION 'collection run is busy; retry target deletion' USING ERRCODE='40001'; END IF;
  UPDATE collection_run SET control_intent='stopped',revision=revision+1,ended_at=scope_001_now(),updated_at=scope_001_now() WHERE run_ref=linked;
  UPDATE collection_work_order SET queue_state='cancelled' WHERE queue_state='queued' AND work_order_ref IN(SELECT work_order_ref FROM collection_run_order WHERE run_ref=linked);
 END LOOP;
 RETURN OLD;
END $$;
CREATE TRIGGER collection_source_target_deleted BEFORE DELETE ON collection_observation_target FOR EACH ROW EXECUTE FUNCTION collection_source_target_deleted();

-- Identity-only first-admission index, over the existing shared comment observations.
-- It stores no text and prevents concurrent/replayed observations from each claiming "new".
CREATE TABLE linggan_material_comment_first_admission (
 content_public_ref uuid NOT NULL REFERENCES linggan_material_content(public_ref),
 comment_external_id text NOT NULL,
 first_package_ref uuid NOT NULL REFERENCES linggan_runtime_capture_package(package_ref),
 PRIMARY KEY(content_public_ref,comment_external_id)
);
INSERT INTO linggan_material_comment_first_admission
 SELECT DISTINCT ON(c.content_public_ref,c.comment_external_id) c.content_public_ref,c.comment_external_id,c.package_ref
 FROM linggan_material_comment c JOIN linggan_runtime_capture_package p USING(package_ref)
 ORDER BY c.content_public_ref,c.comment_external_id,p.accepted_at,c.material_ref;
CREATE FUNCTION collection_comment_first_admitted() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 INSERT INTO linggan_material_comment_first_admission(content_public_ref,comment_external_id,first_package_ref)
 VALUES(NEW.content_public_ref,NEW.comment_external_id,NEW.package_ref) ON CONFLICT DO NOTHING;
 RETURN NEW;
END $$;
CREATE TRIGGER collection_comment_first_admitted AFTER INSERT ON linggan_material_comment FOR EACH ROW EXECUTE FUNCTION collection_comment_first_admitted();
CREATE TRIGGER collection_comment_first_admission_immutable BEFORE UPDATE OR DELETE ON linggan_material_comment_first_admission FOR EACH ROW EXECUTE FUNCTION linggan_plugin_runtime_forbid_mutation();

-- Membership is derived from immutable accepted deliveries, never current cumulative totals.
CREATE VIEW collection_run_package AS
 SELECT DISTINCT relation.run_ref,package.package_ref,package.accepted_at,package.package_kind
 FROM collection_run_order relation
 JOIN collection_work_order_lease lease USING(work_order_ref)
 JOIN collection_work_order_lease_task task USING(lease_ref)
 JOIN linggan_runtime_capture_package package ON package.task_id=task.task_id
 JOIN linggan_runtime_submission_receipt receipt USING(package_ref)
 WHERE receipt.material_admission='ACCEPTED';
CREATE VIEW collection_run_summary AS
 SELECT run.*,domain.name AS domain_name,domain.status AS domain_status,
   (SELECT count(*) FROM collection_run_order o JOIN collection_work_order w USING(work_order_ref) WHERE o.run_ref=run.run_ref AND w.queue_state='queued') AS queued_orders,
   (SELECT count(*) FROM collection_run_order o JOIN collection_work_order_lease l USING(work_order_ref) WHERE o.run_ref=run.run_ref AND l.released_at IS NULL AND l.expires_at>scope_001_now()) AS live_leases,
   (SELECT count(*) FROM collection_run_work w WHERE w.run_ref=run.run_ref) AS selected_works,
   (SELECT count(*) FROM collection_run_work w WHERE w.run_ref=run.run_ref AND COALESCE((checkpoint->>'commentsComplete')::boolean,false)) AS completed_discussions,
   (SELECT count(*) FROM collection_run_work w WHERE w.run_ref=run.run_ref AND gap_reason IS NOT NULL) AS gap_works,
   (SELECT count(*) FROM collection_run_work w WHERE w.run_ref=run.run_ref AND
       (w.reused_detail_package_ref IS NOT NULL OR EXISTS(
        SELECT 1 FROM linggan_material_content_detail d JOIN collection_run_package p USING(package_ref)
        WHERE p.run_ref=run.run_ref AND d.content_public_ref=w.content_public_ref))) AS available_details,
   (SELECT count(*) FROM collection_run_work w WHERE w.run_ref=run.run_ref AND w.reused_detail_package_ref IS NOT NULL) AS reused_details,
   (SELECT count(DISTINCT (c.content_public_ref,c.comment_external_id)) FROM linggan_material_comment c
    JOIN collection_run_package p USING(package_ref) WHERE p.run_ref=run.run_ref AND NOT EXISTS(SELECT 1 FROM linggan_material_comment_restriction r WHERE r.content_public_ref=c.content_public_ref AND r.comment_external_id=c.comment_external_id)) AS delivered_comments,
   (SELECT count(*) FROM linggan_material_comment_first_admission first
    JOIN collection_run_package p ON p.package_ref=first.first_package_ref WHERE p.run_ref=run.run_ref AND NOT EXISTS(SELECT 1 FROM linggan_material_comment_restriction r WHERE r.content_public_ref=first.content_public_ref AND r.comment_external_id=first.comment_external_id)) AS new_comments,
   (SELECT count(DISTINCT (c.content_public_ref,c.comment_external_id)) FROM linggan_material_comment c
    WHERE EXISTS(SELECT 1 FROM collection_run_work w WHERE w.run_ref=run.run_ref AND w.content_public_ref=c.content_public_ref) AND NOT EXISTS(SELECT 1 FROM linggan_material_comment_restriction r WHERE r.content_public_ref=c.content_public_ref AND r.comment_external_id=c.comment_external_id)) AS cumulative_comments,
   (SELECT count(DISTINCT (rec.value->'sourceObject'->>'externalId',COALESCE(rec.value->'payload'->>'commentId',rec.value->'payload'->>'id')))
    FROM collection_run_package p JOIN linggan_runtime_capture_package raw USING(package_ref)
    CROSS JOIN LATERAL jsonb_array_elements(raw.payload->'records') rec(value)
    WHERE p.run_ref=run.run_ref AND raw.package_kind IN('comments','replies') AND COALESCE(rec.value->'payload'->>'commentId',rec.value->'payload'->>'id','')<>'') AS received_comments,
   (SELECT max(p.accepted_at) FROM collection_run_package p WHERE p.run_ref=run.run_ref) AS latest_delivery_at,
   COALESCE((SELECT jsonb_agg(DISTINCT jsonb_build_object('stationRef',station.station_ref,'name',station.display_name))
     FROM collection_run_order o JOIN collection_work_order_lease lease USING(work_order_ref)
     JOIN execution_station station USING(station_ref)
     WHERE o.run_ref=run.run_ref),'[]'::jsonb) AS stations,
   (run.control_intent='stopped' OR run.phase='ended') AS ended,
   (run.phase='blocked' OR EXISTS(SELECT 1 FROM collection_run_work w WHERE w.run_ref=run.run_ref AND w.gap_reason IS NOT NULL)) AS needs_attention,
   EXISTS(SELECT 1 FROM collection_run_work w WHERE w.run_ref=run.run_ref AND
      (w.reused_detail_package_ref IS NOT NULL OR EXISTS(SELECT 1 FROM collection_run_package p WHERE p.run_ref=run.run_ref AND p.package_kind IN('content_detail','comments','replies')))) AS has_results
 FROM collection_run run JOIN observation_domain domain USING(domain_ref);

-- Legacy grouping uses only durable order identity and a verified progressive root relation.
-- It invents no historical run, comment mode, rank, control intent, or retrospective request.
CREATE VIEW collection_user_order AS
 SELECT relation.run_ref,usage.domain_ref,relation.work_order_ref,false AS legacy
 FROM collection_run_order relation JOIN collection_work_order_domain_usage usage USING(work_order_ref)
 UNION
 SELECT COALESCE(parent.work_order_ref,root.work_order_ref,w.work_order_ref),usage.domain_ref,w.work_order_ref,true
 FROM collection_work_order w JOIN collection_work_order_domain_usage usage USING(work_order_ref)
 LEFT JOIN collection_work_order root ON root.work_order_ref::text=w.stop_conditions #>> '{progressiveArchive,rootWorkOrderRef}'
   AND root.target_ref=w.target_ref AND root.stop_conditions #>> '{progressiveArchive,rootWorkOrderRef}'=root.work_order_ref::text
   AND EXISTS(SELECT 1 FROM collection_work_order_domain_usage u WHERE u.work_order_ref=root.work_order_ref AND u.domain_ref=usage.domain_ref)
 LEFT JOIN collection_work_order parent ON parent.work_order_ref::text=w.stop_conditions->>'collectionUserParentRef'
   AND parent.target_ref=w.target_ref AND parent.lane IN('patrol','deep_archive')
   AND EXISTS(SELECT 1 FROM collection_work_order_domain_usage u WHERE u.work_order_ref=parent.work_order_ref AND u.domain_ref=usage.domain_ref)
   AND EXISTS(SELECT 1 FROM jsonb_array_elements(COALESCE(w.stop_conditions->'collectionUserOrigins','[]'::jsonb)) origin WHERE origin->>'workOrderRef'=parent.work_order_ref::text)
 WHERE NOT EXISTS(SELECT 1 FROM collection_run_order r WHERE r.work_order_ref=w.work_order_ref);
CREATE VIEW collection_user_package AS
 SELECT DISTINCT relation.run_ref,relation.domain_ref,package.package_ref,package.accepted_at,package.package_kind
 FROM collection_user_order relation JOIN collection_work_order_lease lease USING(work_order_ref)
 JOIN collection_work_order_lease_task task USING(lease_ref)
 JOIN linggan_runtime_capture_package package ON package.task_id=task.task_id
 JOIN linggan_runtime_submission_receipt receipt USING(package_ref) WHERE receipt.material_admission='ACCEPTED';
CREATE VIEW collection_user_member AS
 SELECT r.run_ref,r.domain_ref,w.content_public_ref FROM collection_run r JOIN collection_run_work w USING(run_ref)
 UNION
 SELECT u.run_ref,u.domain_ref,m.content_public_ref FROM collection_user_order u JOIN collection_work_order_material_target m USING(work_order_ref) WHERE u.legacy
 UNION
 SELECT u.run_ref,u.domain_ref,f.content_public_ref FROM collection_user_package u JOIN linggan_material_discovery_finding f USING(package_ref)
 WHERE EXISTS(SELECT 1 FROM collection_user_order o WHERE o.run_ref=u.run_ref AND o.domain_ref=u.domain_ref AND o.legacy);
CREATE VIEW collection_user_summary AS
 SELECT r.run_ref,r.domain_ref,r.created_at,r.name,r.ended,r.needs_attention,r.has_results,to_jsonb(r)||jsonb_build_object('legacy',false) AS body FROM collection_run_summary r
 UNION ALL
 SELECT grouped.run_ref,grouped.domain_ref,root.created_at,COALESCE(t.display_name,t.identity_key),grouped.ended,false,
   EXISTS(SELECT 1 FROM collection_user_package p WHERE p.run_ref=grouped.run_ref AND p.domain_ref=grouped.domain_ref AND p.package_kind IN('content_detail','comments','replies')),
   jsonb_build_object('run_ref',grouped.run_ref,'domain_ref',grouped.domain_ref,'domain_name',d.name,'name',COALESCE(t.display_name,t.identity_key),
    'source_kind',CASE WHEN root.lane='patrol' THEN 'patrol' WHEN EXISTS(SELECT 1 FROM collection_work_order_material_target m WHERE m.work_order_ref=root.work_order_ref) THEN 'supplement' ELSE 'archive' END,
    'comment_mode',NULL,'work_limit',root.max_works,'scope_frozen_at',NULL,'legacy',true,'revision',NULL,'control_intent',NULL,
    'phase',CASE WHEN grouped.ended THEN 'ended' ELSE 'discovery' END,'ended',grouped.ended,'created_at',root.created_at,'updated_at',COALESCE((SELECT max(p.accepted_at) FROM collection_user_package p WHERE p.run_ref=grouped.run_ref AND p.domain_ref=grouped.domain_ref),root.created_at),
    'selected_works',(SELECT count(*) FROM collection_user_member m WHERE m.run_ref=grouped.run_ref AND m.domain_ref=grouped.domain_ref),
    'available_details',(SELECT count(DISTINCT detail.content_public_ref) FROM linggan_material_content_detail detail JOIN collection_user_package p USING(package_ref) WHERE p.run_ref=grouped.run_ref AND p.domain_ref=grouped.domain_ref),
    'reused_details',NULL,'new_comments',NULL,'discovery_reason','历史参数按原执行记录核对',
    'delivered_comments',(SELECT count(DISTINCT (c.content_public_ref,c.comment_external_id)) FROM linggan_material_comment c JOIN collection_user_package p USING(package_ref) WHERE p.run_ref=grouped.run_ref AND p.domain_ref=grouped.domain_ref AND NOT EXISTS(SELECT 1 FROM linggan_material_comment_restriction r WHERE r.content_public_ref=c.content_public_ref AND r.comment_external_id=c.comment_external_id)))
 FROM (SELECT relation.run_ref,relation.domain_ref,bool_and(w.queue_state IN('completed','cancelled')) AS ended
       FROM collection_user_order relation JOIN collection_work_order w USING(work_order_ref) WHERE relation.legacy GROUP BY relation.run_ref,relation.domain_ref) grouped
 JOIN collection_work_order root ON root.work_order_ref=grouped.run_ref
 JOIN collection_observation_target t ON t.target_ref=root.target_ref JOIN observation_domain d ON d.domain_ref=grouped.domain_ref;
