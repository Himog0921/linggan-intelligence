-- EVIDENCE-INSPECTOR-RECORDS-001（Mog 2026-10-23）：评论深采可以指定条数。
-- 新评论范围 up_to：每篇作品采到 comment_target 条不重复评论（含回复）就停；评论区先走到尽头也结束。
-- 只加列、扩约束，并把新列纳入冻结的采集意图与汇总视图；既有采集行的 comment_target 为 NULL，意图与历史不变。
ALTER TABLE collection_run ADD COLUMN comment_target integer;
-- 推进调度的轮转位置独立于业务更新时间；等待工位不阻塞后来的任务，也不刷新一分钟准入重试时钟。
ALTER TABLE collection_run ADD COLUMN last_advanced_at timestamptz;
ALTER TABLE collection_run DROP CONSTRAINT collection_run_comment_mode_check;
ALTER TABLE collection_run ADD CONSTRAINT collection_run_comment_mode_check
 CHECK(comment_mode IN('sample_30','all_public','up_to'));
ALTER TABLE collection_run ADD CONSTRAINT collection_run_comment_target_check
 CHECK((comment_mode='up_to')=(comment_target IS NOT NULL) AND (comment_target IS NULL OR comment_target BETWEEN 1 AND 1000));
CREATE OR REPLACE FUNCTION collection_run_scope_frozen() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF ROW(NEW.run_ref,NEW.idempotency_key,NEW.request_hash,NEW.domain_ref,NEW.source_kind,
        NEW.source_target_ref,NEW.search_query,NEW.ranking,NEW.work_limit,NEW.comment_mode,NEW.comment_target,NEW.created_at)
    IS DISTINCT FROM ROW(OLD.run_ref,OLD.idempotency_key,OLD.request_hash,OLD.domain_ref,OLD.source_kind,
        OLD.source_target_ref,OLD.search_query,OLD.ranking,OLD.work_limit,OLD.comment_mode,OLD.comment_target,OLD.created_at)
 THEN RAISE EXCEPTION 'collection run intent is frozen' USING ERRCODE='23514'; END IF;
 IF OLD.control_intent='stopped' AND NEW.control_intent<>'stopped'
 THEN RAISE EXCEPTION 'stopped collection run cannot restart' USING ERRCODE='23514'; END IF;
 RETURN NEW;
END $$;
-- 汇总视图原用 run.* 展开（列清单在创建时固定）：替换时逐列写出原有列、原有计算列不变，新列只追加在末尾，
-- 依赖它的 collection_user_summary（to_jsonb 整行）随之带出 comment_target。
CREATE OR REPLACE VIEW collection_run_summary AS
 SELECT run.run_ref,run.idempotency_key,run.request_hash,run.domain_ref,run.source_kind,run.source_target_ref,run.execution_target_ref,run.parent_run_ref,run.name,run.search_query,run.ranking,run.work_limit,run.comment_mode,run.authorization_ref,run.control_intent,run.revision,run.phase,run.scope_frozen_at,run.discovery_reason,run.created_at,run.updated_at,run.ended_at,domain.name AS domain_name,domain.status AS domain_status,
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
      (w.reused_detail_package_ref IS NOT NULL OR EXISTS(SELECT 1 FROM collection_run_package p WHERE p.run_ref=run.run_ref AND p.package_kind IN('content_detail','comments','replies')))) AS has_results,
   run.comment_target
 FROM collection_run run JOIN observation_domain domain USING(domain_ref);
