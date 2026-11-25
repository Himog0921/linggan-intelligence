//! One worker scan handles at most sixteen targets from one authorized selection.
//! Authorization, the ordinary start command and its per-target receipts commit together.
use super::super::{Selection, Start, start};
use crate::ai_foundation::batch_store::BatchKind;
use crate::{ai_foundation::store::FoundationError, comment_study_source::StudySourceRole};
use linggan_storage_postgres::Database;
use serde_json::Value;
use sqlx::{Acquire, Postgres, Transaction};
use std::collections::BTreeMap;
use uuid::Uuid;

const LIMIT: i64 = 16;

pub(super) async fn advance(db: &Database) -> Result<(), FoundationError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET LOCAL lock_timeout='3s'")
        .execute(&mut *tx)
        .await?;
    let next: Option<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT t.domain_ref,t.selection_ref FROM linggan_comment_research_selection_target t
         JOIN linggan_comment_research_automation a USING(domain_ref)
         JOIN observation_domain d USING(domain_ref)
         WHERE a.enabled AND a.auto_submit AND d.status='active' AND t.next_check_at<=scope_001_now()
         ORDER BY t.next_check_at,t.domain_ref,t.selection_ref,t.position
         LIMIT 1 FOR UPDATE OF a SKIP LOCKED",
    ).fetch_optional(&mut *tx).await?;
    let Some((domain, selection)) = next else {
        return Ok(());
    };
    // Manual and scheduled starts serialize on the same domain lock. A manual send cannot
    // slip between the historical-dispatch check and start's locked plan validation.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("comment-understanding-start:{domain}"))
        .execute(&mut *tx)
        .await?;
    let targets: Vec<(i32,Uuid,String,Option<String>)> = sqlx::query_as(
        "SELECT t.position,t.work_ref,t.comment_external_id, (
           SELECT CASE WHEN bool_or(c.state IN ('pending','running')) THEN 'already_queued'
                       WHEN bool_or(c.failure_code='ai_dispatch_unknown' OR i.dispatch_state='unknown') THEN 'dispatch_unresolved'
                       WHEN bool_or(x.result IS NOT NULL) THEN 'already_researched'
                       WHEN bool_or(i.dispatch_state='started') THEN 'previous_dispatch' END
           FROM linggan_comment_research_request r
           JOIN linggan_material_comment m ON m.material_ref=r.source_ref
           JOIN linggan_comment_research_unit_target u USING(request_ref)
           JOIN linggan_comment_research_unit unit USING(unit_ref)
           JOIN linggan_ai_batch_item_control c ON c.item_ref=unit.item_ref
           LEFT JOIN linggan_model_invocation i ON i.invocation_ref=c.invocation_ref
           LEFT JOIN linggan_comment_research_result x ON x.request_ref=r.request_ref
           WHERE r.domain_ref=t.domain_ref AND r.work_ref=t.work_ref
             AND m.comment_external_id=t.comment_external_id AND r.method_version='comment-understanding.v1')
         FROM linggan_comment_research_selection_target t
         WHERE t.selection_ref=$1 AND t.next_check_at<=scope_001_now()
         ORDER BY t.next_check_at,t.position LIMIT $2"
    ).bind(selection).bind(LIMIT).fetch_all(&mut *tx).await?;
    let current: Value = sqlx::query_scalar(include_str!("../automation_selection.sql"))
        .bind(selection)
        .bind(domain)
        .bind(
            targets
                .iter()
                .map(|(position, ..)| *position)
                .collect::<Vec<_>>(),
        )
        .fetch_one(&mut *tx)
        .await?;
    let mut sources = BTreeMap::new();
    for item in current["selections"]
        .as_array()
        .ok_or(FoundationError::Invalid)?
    {
        let work = uuid(&item["workRef"])?;
        let id = item["commentExternalId"]
            .as_str()
            .ok_or(FoundationError::Invalid)?;
        sources.insert((work, id), uuid(&item["sourceRef"])?);
    }
    let mut candidates = Vec::new();
    let mut positions = BTreeMap::new();
    for (position, work, id, prior_reason) in targets {
        let source = sources.get(&(work, id.as_str()));
        if let Some(reason) = prior_reason
            .as_deref()
            .or_else(|| source.is_none().then_some("source_unavailable"))
        {
            record(&mut tx, selection, position, "skipped", Some(reason), None).await?;
        } else if let Some(&source_ref) = source {
            candidates.push(Selection {
                work_ref: work,
                source_ref,
                observation_role: StudySourceRole::Primary,
            });
            positions.insert(source_ref, position);
        }
    }
    if !candidates.is_empty() {
        let preview = start::preview_in(&mut tx, domain, &candidates, false).await?;
        for excluded in preview["excludedTargets"]
            .as_array()
            .ok_or(FoundationError::Invalid)?
        {
            let reason = excluded["reason"]
                .as_str()
                .ok_or(FoundationError::Invalid)?;
            let position = positions
                .remove(&uuid(&excluded["sourceRef"])?)
                .ok_or(FoundationError::Invalid)?;
            let state = if reason == "waiting_l1" {
                "waiting_l1"
            } else {
                "skipped"
            };
            record(&mut tx, selection, position, state, Some(reason), None).await?;
        }
        if !positions.is_empty() {
            let request = Start {
                domain_ref: domain,
                command_ref: Uuid::new_v4(),
                selections: candidates,
                knowledge_cutoff: preview["knowledgeCutoff"]
                    .as_str()
                    .ok_or(FoundationError::Invalid)?
                    .to_owned(),
                expected_plan_hash: preview["planHash"]
                    .as_str()
                    .ok_or(FoundationError::Invalid)?
                    .to_owned(),
                expected_task_revision: preview["taskRevision"]
                    .as_i64()
                    .ok_or(FoundationError::Invalid)?,
                task_config_ref: uuid(&preview["taskConfigRef"])?,
                rerun: false,
            };
            // A denied admission rolls back only its savepoint; retain its reason and retry time.
            let (state, reason, batch) =
                match start::start_in(tx.begin().await?, &request, BatchKind::Automatic).await {
                    Ok(receipt) => ("submitted", None, Some(receipt.batch_ref)),
                    Err(FoundationError::AdmissionDenied(reasons)) => {
                        ("admission_blocked", Some(reasons.join(",")), None)
                    }
                    Err(error) => return Err(error),
                };
            for position in positions.into_values() {
                record(
                    &mut tx,
                    selection,
                    position,
                    state,
                    reason.as_deref(),
                    batch,
                )
                .await?;
            }
        }
    }
    tx.commit().await?;
    Ok(())
}

fn uuid(value: &Value) -> Result<Uuid, FoundationError> {
    serde_json::from_value(value.clone()).map_err(|_| FoundationError::Invalid)
}

async fn record(
    tx: &mut Transaction<'_, Postgres>,
    selection: Uuid,
    position: i32,
    state: &str,
    reason: Option<&str>,
    batch: Option<Uuid>,
) -> Result<(), FoundationError> {
    sqlx::query("UPDATE linggan_comment_research_selection_target
        SET submission_state=$3,submission_reason=$4,submitted_batch_ref=$5,
        next_check_at=CASE WHEN $3 IN ('waiting_l1','admission_blocked') THEN scope_001_now()+interval '5 minutes' END
        WHERE selection_ref=$1 AND position=$2")
        .bind(selection).bind(position).bind(state).bind(reason).bind(batch).execute(&mut **tx).await?;
    Ok(())
}
