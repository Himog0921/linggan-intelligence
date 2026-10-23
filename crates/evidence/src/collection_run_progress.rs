//! Bounded advancement over accepted original packages. A normal tick never visits a platform.
use crate::collection_run::{RunError, admit_stage};
use linggan_storage_postgres::Database;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

pub async fn advance_runs(db: &Database) -> Result<usize, RunError> {
    if !crate::collection_governance_enabled() {
        return Ok(0);
    }
    let ready: bool = sqlx::query_scalar("SELECT to_regclass('collection_run') IS NOT NULL")
        .fetch_one(db.pool())
        .await?;
    if !ready {
        return Ok(0);
    }
    let runs:Vec<Uuid>=sqlx::query_scalar("SELECT run_ref FROM collection_run r JOIN observation_domain d USING(domain_ref) WHERE ((r.control_intent='running' AND r.phase IN('discovery','materials') AND d.status='active') OR (r.phase='discovery' AND EXISTS(SELECT 1 FROM collection_run_order o JOIN collection_work_order w USING(work_order_ref) WHERE o.run_ref=r.run_ref AND o.stage='discovery' AND w.queue_state='completed')))  ORDER BY r.updated_at,r.run_ref LIMIT 20").fetch_all(db.pool()).await?;
    let mut advanced = 0;
    for run in runs {
        let mut tx = db.pool().begin().await?;
        let locked: bool =
            sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(hashtextextended($1,400))")
                .bind(run.to_string())
                .fetch_one(&mut *tx)
                .await?;
        if !locked {
            continue;
        }
        let r=sqlx::query("SELECT phase,comment_mode,comment_target,control_intent,(SELECT status FROM observation_domain d WHERE d.domain_ref=collection_run.domain_ref) AS domain_status FROM collection_run WHERE run_ref=$1 FOR UPDATE").bind(run).fetch_one(&mut *tx).await?;
        if r.get::<String, _>("phase") == "discovery" {
            freeze_discovery(&mut tx, run).await?;
        }
        let phase: String = sqlx::query_scalar("SELECT phase FROM collection_run WHERE run_ref=$1")
            .bind(run)
            .fetch_one(&mut *tx)
            .await?;
        if phase == "materials"
            && r.get::<String, _>("control_intent") == "running"
            && r.get::<String, _>("domain_status") == "active"
        {
            queue_materials(&mut tx, run, &r.get::<String, _>("comment_mode")).await?;
            reconcile_materials(
                &mut tx,
                run,
                &r.get::<String, _>("comment_mode"),
                r.get::<Option<i32>, _>("comment_target"),
            )
            .await?;
        }
        tx.commit().await?;
        advanced += 1;
    }
    Ok(advanced)
}

async fn freeze_discovery(tx: &mut Transaction<'_, Postgres>, run: Uuid) -> Result<(), RunError> {
    let order:Option<(Option<Uuid>,Option<String>)>=sqlx::query_as("SELECT r.work_order_ref,w.queue_state FROM collection_run_order r LEFT JOIN collection_work_order w USING(work_order_ref) WHERE r.run_ref=$1 AND r.stage='discovery' ORDER BY r.created_at DESC LIMIT 1").bind(run).fetch_optional(&mut **tx).await?;
    match order {
        Some((Some(_), Some(ref state))) if state == "completed" => {}
        Some((Some(_), Some(ref state))) if state == "cancelled" => {
            sqlx::query("UPDATE collection_run SET phase='blocked',discovery_reason='discovery_execution_stopped',updated_at=scope_001_now() WHERE run_ref=$1").bind(run).execute(&mut **tx).await?;
            return Ok(());
        }
        Some((None, _)) => {
            // Admission capacity is transient. Retain its decision, retry at most once/minute.
            let due:bool=sqlx::query_scalar("SELECT updated_at<scope_001_now()-interval '1 minute' FROM collection_run WHERE run_ref=$1").bind(run).fetch_one(&mut **tx).await?;
            if due {
                admit_stage(tx, run, None, "discovery", 1).await?;
            }
            return Ok(());
        }
        _ => return Ok(()),
    }
    // Ranking comes from the original result order. Known materials still occupy their slot.
    // DISTINCT ON selects the first accepted observation of an identity, never UUID rank.
    sqlx::query(r#"INSERT INTO collection_run_work(run_ref,content_public_ref,ordinal,discovery_package_ref)
     SELECT $1,content_public_ref,row_number() OVER(ORDER BY accepted_at,result_position,record_ordinal,package_ref),package_ref FROM (
      SELECT DISTINCT ON(f.content_public_ref) f.content_public_ref,p.accepted_at,f.result_position,f.record_ordinal,p.package_ref
      FROM collection_run_package p JOIN linggan_material_discovery_finding f USING(package_ref)
      WHERE p.run_ref=$1 AND f.result_position IS NOT NULL
      ORDER BY f.content_public_ref,p.accepted_at,f.result_position,f.record_ordinal,p.package_ref) first_seen
     ORDER BY accepted_at,result_position,record_ordinal,package_ref LIMIT (SELECT work_limit FROM collection_run WHERE run_ref=$1)"#).bind(run).execute(&mut **tx).await?;
    let reason:Option<String>=sqlx::query_scalar("SELECT p.coverage #>> '{layers,0,stoppedReason}' FROM collection_run_package r JOIN linggan_runtime_capture_package p USING(package_ref) WHERE r.run_ref=$1 AND p.package_kind IN('discovery_search','profile_discovery') ORDER BY p.accepted_at DESC LIMIT 1").bind(run).fetch_optional(&mut **tx).await?.flatten();
    sqlx::query("UPDATE collection_run SET phase='materials',scope_frozen_at=scope_001_now(),discovery_reason=COALESCE($2,'accepted_discovery_scope'),updated_at=scope_001_now() WHERE run_ref=$1").bind(run).bind(reason).execute(&mut **tx).await?;
    Ok(())
}

async fn queue_materials(
    tx: &mut Transaction<'_, Postgres>,
    run: Uuid,
    mode: &str,
) -> Result<(), RunError> {
    // One small request per work preserves fairness and independent failures in the common queue.
    let works:Vec<Uuid>=sqlx::query_scalar("SELECT content_public_ref FROM collection_run_work w WHERE run_ref=$1 AND NOT EXISTS(SELECT 1 FROM collection_run_order o WHERE o.run_ref=w.run_ref AND o.content_public_ref=w.content_public_ref) ORDER BY ordinal LIMIT 3").bind(run).fetch_all(&mut **tx).await?;
    for work in works {
        let reused:Option<Uuid>=sqlx::query_scalar("SELECT d.package_ref FROM linggan_material_content_detail d JOIN linggan_runtime_capture_package p USING(package_ref) JOIN linggan_runtime_submission_receipt r USING(package_ref) JOIN collection_run run ON run.run_ref=$2 WHERE d.content_public_ref=$1 AND p.accepted_at<run.created_at AND r.material_admission='ACCEPTED' ORDER BY p.accepted_at DESC LIMIT 1").bind(work).bind(run).fetch_optional(&mut **tx).await?;
        sqlx::query("UPDATE collection_run_work SET reused_detail_package_ref=$3 WHERE run_ref=$1 AND content_public_ref=$2").bind(run).bind(work).bind(reused).execute(&mut **tx).await?;
        admit_stage(
            tx,
            run,
            Some(work),
            if mode == "sample_30" || reused.is_some() {
                "comments"
            } else {
                "detail"
            },
            1,
        )
        .await?;
    }
    // A rejected admission is a durable decision, not an admitted stage. Retry the latest
    // missing order at most once per minute while preserving its exact work and segment.
    let waiting=sqlx::query("SELECT * FROM (SELECT DISTINCT ON(content_public_ref) content_public_ref,stage,segment,work_order_ref,created_at FROM collection_run_order WHERE run_ref=$1 AND content_public_ref IS NOT NULL ORDER BY content_public_ref,CASE WHEN stage='comments' THEN 2 ELSE 1 END DESC,segment DESC,created_at DESC,request_ref DESC) latest WHERE work_order_ref IS NULL AND created_at<scope_001_now()-interval '1 minute' ORDER BY created_at LIMIT 3").bind(run).fetch_all(&mut **tx).await?;
    for row in waiting {
        admit_stage(
            tx,
            run,
            Some(row.get("content_public_ref")),
            &row.get::<String, _>("stage"),
            row.get("segment"),
        )
        .await?;
    }
    // Completion still belongs to receipt/coverage evaluation, never the fact of queueing.
    Ok(())
}

async fn reconcile_materials(
    tx: &mut Transaction<'_, Postgres>,
    run: Uuid,
    mode: &str,
    target: Option<i32>,
) -> Result<(), RunError> {
    let latest=sqlx::query("SELECT * FROM (SELECT DISTINCT ON(o.content_public_ref) o.content_public_ref,o.stage,o.segment,o.work_order_ref,w.queue_state,member.checkpoint FROM collection_run_order o JOIN collection_run_work member ON member.run_ref=o.run_ref AND member.content_public_ref=o.content_public_ref LEFT JOIN collection_work_order w USING(work_order_ref) WHERE o.run_ref=$1 AND o.content_public_ref IS NOT NULL AND member.gap_reason IS NULL AND NOT COALESCE((member.checkpoint->>'commentsComplete')::boolean,false) ORDER BY o.content_public_ref,CASE WHEN o.stage='comments' THEN 2 ELSE 1 END DESC,o.segment DESC,o.created_at DESC,o.request_ref DESC) latest WHERE queue_state IN('completed','cancelled') LIMIT 3").bind(run).fetch_all(&mut **tx).await?;
    for row in latest {
        let work: Uuid = row.get("content_public_ref");
        let order: Option<Uuid> = row.get("work_order_ref");
        let stage: String = row.get("stage");
        let segment: i32 = row.get("segment");
        let checkpoint: serde_json::Value = row.get("checkpoint");
        let state: Option<String> = row.get("queue_state");
        if order.is_none() {
            continue;
        }
        if state.as_deref() == Some("cancelled") {
            sqlx::query("UPDATE collection_run_work SET gap_reason='execution_stopped_or_unavailable' WHERE run_ref=$1 AND content_public_ref=$2").bind(run).bind(work).execute(&mut **tx).await?;
            continue;
        }
        if state.as_deref() != Some("completed") {
            continue;
        }
        let packages=sqlx::query("SELECT p.package_kind,p.coverage,p.payload FROM collection_run_order o JOIN collection_work_order_lease l USING(work_order_ref) JOIN collection_work_order_lease_task t USING(lease_ref) JOIN linggan_runtime_capture_package p ON p.task_id=t.task_id JOIN linggan_runtime_submission_receipt r USING(package_ref) WHERE o.work_order_ref=$1 AND r.material_admission='ACCEPTED'").bind(order).fetch_all(&mut **tx).await?;
        if stage == "detail" {
            if packages
                .iter()
                .any(|p| p.get::<String, _>("package_kind") == "content_detail")
            {
                admit_stage(tx, run, Some(work), "comments", 1).await?;
            } else {
                sqlx::query("UPDATE collection_run_work SET gap_reason='detail_not_accepted' WHERE run_ref=$1 AND content_public_ref=$2").bind(run).bind(work).execute(&mut **tx).await?;
            }
            continue;
        }
        let discussion = packages
            .iter()
            .find(|p| p.get::<String, _>("package_kind") == "comments");
        if discussion.is_some_and(|p| {
            p.get::<serde_json::Value, _>("coverage")["target"]["commentCollection"]["stopReason"]
                == "risk_control"
        }) {
            sqlx::query("UPDATE collection_run_work SET gap_reason='access_restricted' WHERE run_ref=$1 AND content_public_ref=$2").bind(run).bind(work).execute(&mut **tx).await?;
            continue;
        }
        let replies = packages
            .iter()
            .any(|p| p.get::<String, _>("package_kind") == "replies");
        let captured:i64=sqlx::query_scalar("SELECT count(DISTINCT COALESCE(rec.value->'payload'->>'commentId',rec.value->'payload'->>'id')) FROM collection_run_package p JOIN linggan_runtime_capture_package raw USING(package_ref) CROSS JOIN LATERAL jsonb_array_elements(raw.payload->'records') rec(value) JOIN linggan_material_content c ON c.platform=raw.platform AND c.content_external_id=rec.value->'sourceObject'->>'externalId' WHERE p.run_ref=$1 AND c.public_ref=$2 AND raw.package_kind IN('comments','replies') AND COALESCE(rec.value->'payload'->>'commentId',rec.value->'payload'->>'id','')<>''").bind(run).bind(work).fetch_one(&mut **tx).await?;
        let complete = discussion.is_some_and(|p| {
            let coverage: serde_json::Value = p.get("coverage");
            let receipt = &coverage["target"]["commentCollection"];
            receipt["state"] == "complete"
                && replies
                && (mode == "sample_30"
                    || (receipt["traversal"]["mainExhausted"] == true
                        && receipt["traversal"]["repliesExhausted"] == true
                        && receipt["traversal"]["identityReliable"] == true))
        });
        // 指定条数：这篇已采到的不重复评论（含回复）够数即完成；不够数时仍按「尽量采全」的遍历判据收尾。
        let reached = target.is_some_and(|target| captured >= i64::from(target));
        if complete
            || (mode == "sample_30" && captured >= 30 && discussion.is_some() && replies)
            || (reached && discussion.is_some() && replies)
        {
            sqlx::query("UPDATE collection_run_work SET checkpoint=checkpoint||jsonb_build_object('commentsComplete',true,'lastOrder',$3::uuid,'lastSegment',$4::integer) WHERE run_ref=$1 AND content_public_ref=$2").bind(run).bind(work).bind(order).bind(segment).execute(&mut **tx).await?;
            continue;
        }
        if discussion.is_none() || !replies || discussion.is_some_and(|p|p.get::<serde_json::Value,_>("coverage")["target"]["commentCollection"]["targetIdentity"]=="mismatched") {
            sqlx::query("UPDATE collection_run_work SET gap_reason='discussion_partial_or_not_delivered' WHERE run_ref=$1 AND content_public_ref=$2").bind(run).bind(work).execute(&mut **tx).await?;continue;
        }
        // Raw stable identities include observations removed by the shared text cleaner.
        // Replayed rows do not make progress and a 30-record sample is never refilled.
        let no_progress = if captured <= checkpoint["capturedUnique"].as_i64().unwrap_or(0) {
            checkpoint["emptySegments"].as_i64().unwrap_or(0) + 1
        } else {
            0
        };
        if no_progress >= 3 {
            sqlx::query("UPDATE collection_run_work SET gap_reason='no_progress_after_bounded_retries' WHERE run_ref=$1 AND content_public_ref=$2").bind(run).bind(work).execute(&mut **tx).await?;
            continue;
        }
        sqlx::query("UPDATE collection_run_work SET checkpoint=checkpoint||jsonb_build_object('emptySegments',$3::integer,'lastOrder',$4::uuid,'lastSegment',$5::integer,'capturedUnique',$6::bigint) WHERE run_ref=$1 AND content_public_ref=$2").bind(run).bind(work).bind(no_progress as i32).bind(order).bind(segment).bind(captured).execute(&mut **tx).await?;
        admit_stage(tx, run, Some(work), "comments", segment + 1).await?;
    }
    let done:bool=sqlx::query_scalar("SELECT NOT EXISTS(SELECT 1 FROM collection_run_work WHERE run_ref=$1 AND gap_reason IS NULL AND NOT COALESCE((checkpoint->>'commentsComplete')::boolean,false)) AND scope_frozen_at IS NOT NULL FROM collection_run WHERE run_ref=$1").bind(run).fetch_one(&mut **tx).await?;
    if done {
        sqlx::query("UPDATE collection_run SET phase='ended',ended_at=scope_001_now(),updated_at=scope_001_now() WHERE run_ref=$1").bind(run).execute(&mut **tx).await?;
    }
    Ok(())
}
