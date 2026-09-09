//! P3 continuation keeps every retry in the replay shadow domain and under its first run's
//! authorization envelope. A successor is a new immutable sample set; completed old pairs are
//! copied as provenance-linked receipts and are never sent to a provider again.
use crate::{
    comment_execution_budget::{BudgetPurpose, check_budget_in},
    comment_replay::{
        CreateCandidateReplay, ReplayModelSnapshot, authorization_usage_in,
        create_replay_successor, read_model_snapshot,
    },
    comment_research::comment_source_hash,
    comment_research_sampling::replay_sources,
    comment_runtime::ContextPolicy,
    model_invocation::{connection_request, finish_invocation_in},
    model_secrets::ModelSecretStore,
    model_settings::ModelError,
    pi_adapter::{PiAdapter, PiResponse, safe_result},
};
use linggan_storage_postgres::Database;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use std::collections::{BTreeSet, HashSet};
use uuid::Uuid;

const CONTINUATION_TTL_SECONDS: i64 = 86_400;

#[derive(Clone)]
struct PriorPair {
    source_ref: Uuid,
    work_ref: Uuid,
}

/// Records one restartable intent for every insufficient replay. A `no_new_samples` receipt is
/// revisited when a later sampler sees a previously unselected readable source; it is not a
/// permanent terminal state.
pub async fn enqueue_replay_follow_ups(db: &Database) -> Result<usize, ModelError> {
    let rows = sqlx::query(
        "SELECT r.run_ref,COALESCE(r.authorization_root_run_ref,r.run_ref) AS root, \
                r.context_policy_hash,r.policy_revision,r.candidate_rule_revision_ref,r.seed, \
                candidate.canonical_hash \
         FROM linggan_comment_replay_run r \
         JOIN linggan_comment_research_rule_revision candidate \
           ON candidate.rule_revision_ref=r.candidate_rule_revision_ref \
         LEFT JOIN linggan_comment_replay_follow_up follow_up \
           ON follow_up.prior_run_ref=r.run_ref AND follow_up.reason='required_new_samples' \
         WHERE r.state='insufficient_evidence' \
           AND (follow_up.follow_up_ref IS NULL OR follow_up.state='no_new_samples') \
         ORDER BY follow_up.updated_at NULLS FIRST,r.finished_at,r.run_ref \
         LIMIT 20",
    )
    .fetch_all(db.pool())
    .await?;
    let mut changed = 0;
    for row in rows {
        let prior_run_ref: Uuid = row.get("run_ref");
        let prior_sources = prior_member_sources(db, prior_run_ref).await?;
        let selected = replay_sources(db, row.get::<i64, _>("seed") + 1).await?;
        let state = if selected
            .iter()
            .any(|source| !prior_sources.contains(source))
        {
            "queued"
        } else {
            "no_new_samples"
        };
        let request_hash = comment_source_hash(
            &json!({
                "priorRunRef":prior_run_ref,
                "root":row.get::<Uuid,_>("root"),
                "reason":"required_new_samples",
                "selector":row.get::<String,_>("context_policy_hash"),
                "policyRevision":row.get::<i64,_>("policy_revision"),
                "candidate":row.get::<Uuid,_>("candidate_rule_revision_ref"),
            })
            .to_string(),
        );
        let result = sqlx::query(
            "INSERT INTO linggan_comment_replay_follow_up( \
                 follow_up_ref,prior_run_ref,authorization_root_run_ref,reason,selector_hash, \
                 policy_revision,candidate_rule_revision_ref,candidate_rule_hash,request_hash,state,failure_code \
             ) VALUES($1,$2,$3,'required_new_samples',$4,$5,$6,$7,$8,$9, \
                       CASE WHEN $9='no_new_samples' THEN 'insufficient_new_sources' ELSE NULL END) \
             ON CONFLICT(prior_run_ref,reason) DO UPDATE \
                SET state=EXCLUDED.state,failure_code=EXCLUDED.failure_code,updated_at=scope_001_now() \
              WHERE linggan_comment_replay_follow_up.state='no_new_samples'",
        )
        .bind(Uuid::new_v4())
        .bind(prior_run_ref)
        .bind(row.get::<Uuid, _>("root"))
        .bind(row.get::<String, _>("context_policy_hash"))
        .bind(row.get::<i64, _>("policy_revision"))
        .bind(row.get::<Uuid, _>("candidate_rule_revision_ref"))
        .bind(row.get::<String, _>("canonical_hash"))
        .bind(request_hash)
        .bind(state)
        .execute(db.pool())
        .await?;
        changed += result.rows_affected() as usize;
    }
    Ok(changed)
}

/// Builds at most one continuation. It retains every reusable old pair and fills only the
/// missing denominator from fresh sources. Ten completed old pairs plus twenty fresh sources is
/// therefore a valid thirty-pair run, without re-paying for the ten old pairs.
pub async fn run_next_replay_follow_up(db: &Database) -> Result<bool, ModelError> {
    let row = sqlx::query(
        "SELECT f.follow_up_ref,f.prior_run_ref,f.authorization_root_run_ref, \
                r.baseline_rule_revision_ref,r.candidate_rule_revision_ref,r.config_ref, \
                r.context_policy,r.thresholds,r.seed,r.repeat_member_limit, \
                root.authorized_token_budget \
         FROM linggan_comment_replay_follow_up f \
         JOIN linggan_comment_replay_run r ON r.run_ref=f.prior_run_ref \
         JOIN linggan_comment_replay_run root ON root.run_ref=f.authorization_root_run_ref \
         WHERE f.state='queued' AND f.reason='required_new_samples' \
         ORDER BY f.created_at,f.follow_up_ref LIMIT 1",
    )
    .fetch_optional(db.pool())
    .await?;
    let Some(row) = row else { return Ok(false) };
    let prior_run_ref: Uuid = row.get("prior_run_ref");
    let thresholds: crate::comment_replay_metrics::ReplayThresholds =
        serde_json::from_value(row.get("thresholds")).map_err(|_| ModelError::Invalid)?;
    let prior_pairs = completed_primary_pairs(db, prior_run_ref).await?;
    let prior_sources = prior_member_sources(db, prior_run_ref).await?;
    let selected = select_continuation_sources(
        db,
        row.get::<i64, _>("seed") + 1,
        &prior_pairs,
        &prior_sources,
        thresholds.minimum_pairs,
        thresholds.minimum_works,
    )
    .await?;
    let Some(source_refs) = selected else {
        sqlx::query("UPDATE linggan_comment_replay_follow_up SET state='no_new_samples',failure_code='insufficient_new_sources',updated_at=scope_001_now() WHERE follow_up_ref=$1 AND state='queued'")
            .bind(row.get::<Uuid, _>("follow_up_ref")).execute(db.pool()).await?;
        return Ok(true);
    };
    let command = CreateCandidateReplay {
        run_ref: Uuid::new_v4(),
        sample_set_ref: Uuid::new_v4(),
        baseline_rule_revision_ref: row.get("baseline_rule_revision_ref"),
        candidate_rule_revision_ref: row.get("candidate_rule_revision_ref"),
        config_ref: row.get("config_ref"),
        source_refs,
        seed: row.get::<i64, _>("seed") + 1,
        ttl_seconds: CONTINUATION_TTL_SECONDS,
        context_policy: ContextPolicy::parse(row.get("context_policy"))?,
        thresholds,
        repeat_member_limit: row.get::<i16, _>("repeat_member_limit") as u8,
        // The creation transaction replaces this with the root's remaining envelope. This
        // placeholder can never become an independent grant.
        authorized_token_budget: row.get("authorized_token_budget"),
    };
    let follow_up_ref: Uuid = row.get("follow_up_ref");
    let root: Uuid = row.get("authorization_root_run_ref");
    match create_replay_successor(
        db,
        &command,
        root,
        Some(prior_run_ref),
        Some(follow_up_ref),
        "continuity",
        true,
    )
    .await
    {
        Ok(_) => Ok(true),
        Err(ModelError::Conflict) => {
            // Root authorization is durable. It does not reset merely because the daily ledger
            // moves to another day.
            sqlx::query("UPDATE linggan_comment_replay_follow_up SET state='waiting_daily_budget',failure_code='replay_authorized_budget_exhausted',updated_at=scope_001_now() WHERE follow_up_ref=$1 AND state='queued'")
                .bind(follow_up_ref).execute(db.pool()).await?;
            Ok(true)
        }
        Err(error) => Err(error),
    }
}

/// After an automatic adoption, queue a new-sample health comparison against the active rule's
/// immutable parent. The model and context must still equal the adoption snapshot; otherwise the
/// result cannot be attributed to the prompt and no health replay is created.
pub async fn enqueue_active_rule_health_replays(db: &Database) -> Result<usize, ModelError> {
    let rows = sqlx::query(
        "SELECT adopted.run_ref,adopted.config_ref,adopted.context_policy,adopted.thresholds, \
                adopted.seed,adopted.repeat_member_limit,adopted.model_snapshot, \
                adopted.candidate_rule_revision_ref,rule.parent_rule_revision_ref, \
                adopted.authorized_token_budget \
         FROM linggan_comment_replay_run adopted \
         JOIN linggan_comment_research_rule_revision rule \
           ON rule.rule_revision_ref=adopted.candidate_rule_revision_ref \
         JOIN linggan_comment_research_rule_active active ON active.singleton \
         JOIN linggan_comment_auto_upgrade_policy policy ON policy.singleton \
         WHERE adopted.state='pass' AND adopted.adoption_receipt IS NOT NULL \
           AND active.rule_revision_ref=adopted.candidate_rule_revision_ref \
           AND policy.enabled AND policy.selected_candidate_rule_revision_ref=adopted.candidate_rule_revision_ref \
           AND rule.parent_rule_revision_ref IS NOT NULL \
           AND NOT EXISTS( \
             SELECT 1 FROM linggan_comment_replay_run health \
             WHERE health.authorization_root_run_ref=adopted.run_ref \
               AND health.run_kind='active_health' \
           )",
    )
    .fetch_all(db.pool())
    .await?;
    let mut created = 0;
    for row in rows {
        let frozen: ReplayModelSnapshot =
            serde_json::from_value(row.get("model_snapshot")).map_err(|_| ModelError::Invalid)?;
        let current = read_model_snapshot(db, row.get("config_ref")).await?;
        if current != frozen || !current.connection_enabled {
            continue;
        }
        let prior_run_ref: Uuid = row.get("run_ref");
        let prior_sources = prior_member_sources(db, prior_run_ref).await?;
        let thresholds: crate::comment_replay_metrics::ReplayThresholds =
            serde_json::from_value(row.get("thresholds")).map_err(|_| ModelError::Invalid)?;
        let fresh = select_fresh_sources(
            db,
            row.get::<i64, _>("seed") + 2,
            &prior_sources,
            thresholds.minimum_pairs,
            thresholds.minimum_works,
        )
        .await?;
        let Some(source_refs) = fresh else { continue };
        let command = CreateCandidateReplay {
            run_ref: Uuid::new_v4(),
            sample_set_ref: Uuid::new_v4(),
            baseline_rule_revision_ref: row.get("parent_rule_revision_ref"),
            candidate_rule_revision_ref: row.get("candidate_rule_revision_ref"),
            config_ref: row.get("config_ref"),
            source_refs,
            seed: row.get::<i64, _>("seed") + 2,
            ttl_seconds: CONTINUATION_TTL_SECONDS,
            context_policy: ContextPolicy::parse(row.get("context_policy"))?,
            thresholds,
            repeat_member_limit: row.get::<i16, _>("repeat_member_limit") as u8,
            authorized_token_budget: row.get("authorized_token_budget"),
        };
        if create_replay_successor(
            db,
            &command,
            prior_run_ref,
            None,
            None,
            "active_health",
            false,
        )
        .await
        .is_ok()
        {
            created += 1;
        }
    }
    Ok(created)
}

async fn prior_member_sources(db: &Database, run_ref: Uuid) -> Result<HashSet<Uuid>, ModelError> {
    Ok(sqlx::query_scalar(
        "SELECT member.source_ref FROM linggan_comment_replay_run run \
         JOIN linggan_comment_replay_member member USING(sample_set_ref) WHERE run.run_ref=$1",
    )
    .bind(run_ref)
    .fetch_all(db.pool())
    .await?
    .into_iter()
    .collect())
}

async fn completed_primary_pairs(
    db: &Database,
    run_ref: Uuid,
) -> Result<Vec<PriorPair>, ModelError> {
    let rows = sqlx::query(
        "WITH baseline AS ( \
            SELECT DISTINCT ON (member_ordinal) member_ordinal,state \
            FROM linggan_comment_replay_item \
            WHERE run_ref=$1 AND side='baseline' AND repeat_ordinal=0 \
            ORDER BY member_ordinal,attempt_ordinal DESC \
          ), candidate AS ( \
            SELECT DISTINCT ON (member_ordinal) member_ordinal,state \
            FROM linggan_comment_replay_item \
            WHERE run_ref=$1 AND side='candidate' AND repeat_ordinal=0 \
            ORDER BY member_ordinal,attempt_ordinal DESC \
          ) \
          SELECT member.source_ref,member.work_ref \
          FROM linggan_comment_replay_run run \
          JOIN linggan_comment_replay_member member USING(sample_set_ref) \
          JOIN baseline ON baseline.member_ordinal=member.ordinal AND baseline.state='succeeded' \
          JOIN candidate ON candidate.member_ordinal=member.ordinal AND candidate.state='succeeded' \
          WHERE run.run_ref=$1 ORDER BY member.ordinal",
    )
    .bind(run_ref)
    .fetch_all(db.pool())
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| PriorPair {
            source_ref: row.get("source_ref"),
            work_ref: row.get("work_ref"),
        })
        .collect())
}

async fn source_work_ref(db: &Database, source_ref: Uuid) -> Result<Option<Uuid>, ModelError> {
    sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_comment WHERE material_ref=$1",
    )
    .bind(source_ref)
    .fetch_optional(db.pool())
    .await
    .map_err(ModelError::from)
}

async fn select_continuation_sources(
    db: &Database,
    seed: i64,
    reusable: &[PriorPair],
    all_prior_sources: &HashSet<Uuid>,
    minimum_pairs: usize,
    minimum_works: usize,
) -> Result<Option<Vec<Uuid>>, ModelError> {
    let readable = replay_sources(db, seed).await?;
    let readable_set: HashSet<_> = readable.iter().copied().collect();
    let mut selected: Vec<Uuid> = reusable
        .iter()
        .filter(|pair| readable_set.contains(&pair.source_ref))
        .map(|pair| pair.source_ref)
        .collect();
    selected.truncate(120);
    let mut works: BTreeSet<Uuid> = reusable
        .iter()
        .filter(|pair| selected.contains(&pair.source_ref))
        .map(|pair| pair.work_ref)
        .collect();
    let mut candidates = Vec::new();
    for source in readable {
        if all_prior_sources.contains(&source) {
            continue;
        }
        if let Some(work) = source_work_ref(db, source).await? {
            candidates.push((source, work));
        }
    }
    // Prefer fresh works until coverage is met, then use the frozen sampler order to fill the
    // pair denominator. This remains bounded by the original 120-member policy.
    for (source, work) in candidates.iter().copied() {
        if selected.len() >= 120 {
            break;
        }
        if works.len() < minimum_works && !works.contains(&work) {
            selected.push(source);
            works.insert(work);
        }
    }
    for (source, work) in candidates {
        if selected.len() >= 120
            || (selected.len() >= minimum_pairs && works.len() >= minimum_works)
        {
            break;
        }
        if !selected.contains(&source) {
            selected.push(source);
            works.insert(work);
        }
    }
    Ok((selected.len() >= minimum_pairs && works.len() >= minimum_works).then_some(selected))
}

async fn select_fresh_sources(
    db: &Database,
    seed: i64,
    excluded: &HashSet<Uuid>,
    minimum_pairs: usize,
    minimum_works: usize,
) -> Result<Option<Vec<Uuid>>, ModelError> {
    select_continuation_sources(db, seed, &[], excluded, minimum_pairs, minimum_works).await
}

const EXPLANATION_OUTPUT_TOKENS: i32 = 512;
const EXPLANATION_RESERVATION_TOKENS: i64 = 4_608;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DifferenceExplanation {
    kind: String,
    stability_not_accuracy: bool,
    summary: String,
    citations: Vec<DifferenceCitation>,
    uncertainty: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DifferenceCitation {
    side: String,
    item_ref: Uuid,
}

/// Enqueues bounded, sanitized explanations only for a completed pair whose retained shadow
/// summaries differ. The prompt contains no comment text, no raw provider output, and no formal
/// analysis/atom rows; it contains just paired result summaries and stable item/source references.
pub async fn enqueue_difference_explanations(db: &Database) -> Result<usize, ModelError> {
    let rows = sqlx::query(
        "WITH baseline AS ( \
            SELECT DISTINCT ON (run_ref,member_ordinal) run_ref,member_ordinal,item_ref, \
                   structure_accepted,no_signal,safe_summary \
            FROM linggan_comment_replay_item \
            WHERE side='baseline' AND repeat_ordinal=0 AND state='succeeded' \
            ORDER BY run_ref,member_ordinal,attempt_ordinal DESC \
          ), candidate AS ( \
            SELECT DISTINCT ON (run_ref,member_ordinal) run_ref,member_ordinal,item_ref, \
                   structure_accepted,no_signal,safe_summary \
            FROM linggan_comment_replay_item \
            WHERE side='candidate' AND repeat_ordinal=0 AND state='succeeded' \
            ORDER BY run_ref,member_ordinal,attempt_ordinal DESC \
          ) \
          SELECT run.run_ref,member.source_ref,baseline.item_ref AS baseline_item_ref, \
                 candidate.item_ref AS candidate_item_ref,baseline.structure_accepted AS baseline_structure_accepted, \
                 candidate.structure_accepted AS candidate_structure_accepted,baseline.no_signal AS baseline_no_signal, \
                 candidate.no_signal AS candidate_no_signal,baseline.safe_summary AS baseline_summary, \
                 candidate.safe_summary AS candidate_summary \
          FROM linggan_comment_replay_run run \
          JOIN linggan_comment_replay_member member USING(sample_set_ref) \
          JOIN baseline ON baseline.run_ref=run.run_ref AND baseline.member_ordinal=member.ordinal \
          JOIN candidate ON candidate.run_ref=run.run_ref AND candidate.member_ordinal=member.ordinal \
          WHERE run.state IN ('pass','degraded','insufficient_evidence') \
            AND baseline.safe_summary IS DISTINCT FROM candidate.safe_summary \
            AND EXISTS(SELECT 1 FROM linggan_comment_research_readable readable WHERE readable.material_ref=member.source_ref) \
          ORDER BY run.finished_at,run.run_ref,member.ordinal LIMIT 20",
    )
    .fetch_all(db.pool())
    .await?;
    let mut created = 0;
    for row in rows {
        let safe_input = json!({
            "version":"comment-replay-difference-input.v1",
            "sourceRef":row.get::<Uuid,_>("source_ref"),
            "baseline":{
                "itemRef":row.get::<Uuid,_>("baseline_item_ref"),
                "structureAccepted":row.get::<Option<bool>,_>("baseline_structure_accepted"),
                "noSignal":row.get::<Option<bool>,_>("baseline_no_signal"),
                "summary":row.get::<Value,_>("baseline_summary"),
            },
            "candidate":{
                "itemRef":row.get::<Uuid,_>("candidate_item_ref"),
                "structureAccepted":row.get::<Option<bool>,_>("candidate_structure_accepted"),
                "noSignal":row.get::<Option<bool>,_>("candidate_no_signal"),
                "summary":row.get::<Value,_>("candidate_summary"),
            },
        });
        let pair_hash = comment_source_hash(&safe_input.to_string());
        let request_hash = comment_source_hash(
            &json!({
                "runRef":row.get::<Uuid,_>("run_ref"),"pairHash":pair_hash,
                "contract":"comment-replay-difference-explanation.v1"
            })
            .to_string(),
        );
        let result = sqlx::query("INSERT INTO linggan_comment_replay_explanation(explanation_ref,run_ref,pair_hash,baseline_item_ref,candidate_item_ref,request_hash,state,safe_input) VALUES($1,$2,$3,$4,$5,$6,'queued',$7) ON CONFLICT(run_ref,pair_hash) DO NOTHING")
            .bind(Uuid::new_v4()).bind(row.get::<Uuid,_>("run_ref")).bind(pair_hash)
            .bind(row.get::<Uuid,_>("baseline_item_ref")).bind(row.get::<Uuid,_>("candidate_item_ref"))
            .bind(request_hash).bind(safe_input).execute(db.pool()).await?;
        created += result.rows_affected() as usize;
    }
    Ok(created)
}

/// Read-only P3 continuation status for the existing comment-research settings surface. It does
/// not expose stored prompts, source body, or raw provider output, and never starts a call.
pub async fn read_replay_continuity(db: &Database) -> Result<Value, ModelError> {
    let follow_ups = sqlx::query(
        "SELECT follow_up_ref,prior_run_ref,authorization_root_run_ref,reason,state,failure_code, \
                successor_run_ref,created_at::text AS created_at,updated_at::text AS updated_at \
         FROM linggan_comment_replay_follow_up \
         ORDER BY created_at DESC,follow_up_ref DESC LIMIT 100",
    )
    .fetch_all(db.pool())
    .await?;
    let explanations = sqlx::query(
        "SELECT explanation_ref,run_ref,invocation_ref,state,reserved_tokens,failure_code,result, \
                created_at::text AS created_at,finished_at::text AS finished_at,expires_at::text AS expires_at \
         FROM linggan_comment_replay_explanation \
         ORDER BY created_at DESC,explanation_ref DESC LIMIT 100",
    )
    .fetch_all(db.pool())
    .await?;
    let rollbacks = sqlx::query(
        "SELECT receipt_ref,triggering_run_ref,policy_revision,candidate_rule_revision_ref, \
                restored_rule_revision_ref,active_revision_before,active_revision_after, \
                health_evidence,created_at::text AS created_at \
         FROM linggan_comment_rule_rollback_receipt \
         ORDER BY created_at DESC,receipt_ref DESC LIMIT 100",
    )
    .fetch_all(db.pool())
    .await?;
    Ok(json!({
        "followUps":follow_ups.into_iter().map(|row|json!({
            "followUpRef":row.get::<Uuid,_>("follow_up_ref"),
            "priorRunRef":row.get::<Uuid,_>("prior_run_ref"),
            "authorizationRootRunRef":row.get::<Uuid,_>("authorization_root_run_ref"),
            "reason":row.get::<String,_>("reason"),"state":row.get::<String,_>("state"),
            "failureCode":row.get::<Option<String>,_>("failure_code"),
            "successorRunRef":row.get::<Option<Uuid>,_>("successor_run_ref"),
            "createdAt":row.get::<String,_>("created_at"),"updatedAt":row.get::<String,_>("updated_at"),
        })).collect::<Vec<_>>(),
        "explanations":explanations.into_iter().map(|row|json!({
            "explanationRef":row.get::<Uuid,_>("explanation_ref"),"runRef":row.get::<Uuid,_>("run_ref"),
            "invocationRef":row.get::<Option<Uuid>,_>("invocation_ref"),"state":row.get::<String,_>("state"),
            "reservedTokens":row.get::<i64,_>("reserved_tokens"),"failureCode":row.get::<Option<String>,_>("failure_code"),
            "result":row.get::<Option<Value>,_>("result"),"createdAt":row.get::<String,_>("created_at"),
            "finishedAt":row.get::<Option<String>,_>("finished_at"),"expiresAt":row.get::<String,_>("expires_at"),
        })).collect::<Vec<_>>(),
        "rollbackReceipts":rollbacks.into_iter().map(|row|json!({
            "receiptRef":row.get::<Uuid,_>("receipt_ref"),"triggeringRunRef":row.get::<Uuid,_>("triggering_run_ref"),
            "policyRevision":row.get::<i64,_>("policy_revision"),
            "candidateRuleRevisionRef":row.get::<Uuid,_>("candidate_rule_revision_ref"),
            "restoredRuleRevisionRef":row.get::<Uuid,_>("restored_rule_revision_ref"),
            "activeRevisionBefore":row.get::<i32,_>("active_revision_before"),
            "activeRevisionAfter":row.get::<i32,_>("active_revision_after"),
            "healthEvidence":row.get::<Value,_>("health_evidence"),"createdAt":row.get::<String,_>("created_at"),
        })).collect::<Vec<_>>(),
        "stabilityNotAccuracy":true,
    }))
}

/// Uses the normal local Pi adapter and the shared replay ledger. The explanation itself remains
/// shadow-only and is accepted only when every citation names exactly the paired stored receipts.
pub async fn run_next_difference_explanation(
    db: &Database,
    store: &dyn ModelSecretStore,
    adapter: &PiAdapter,
) -> Result<bool, ModelError> {
    let row = sqlx::query(
        "SELECT explanation.explanation_ref,explanation.run_ref,explanation.safe_input, \
                explanation.baseline_item_ref,explanation.candidate_item_ref, \
                run.model_snapshot,run.config_ref \
         FROM linggan_comment_replay_explanation explanation \
         JOIN linggan_comment_replay_run run USING(run_ref) \
         WHERE explanation.state='queued' AND explanation.expires_at>scope_001_now() \
         ORDER BY explanation.created_at,explanation.explanation_ref LIMIT 1",
    )
    .fetch_optional(db.pool())
    .await?;
    let Some(row) = row else { return Ok(false) };
    let model: ReplayModelSnapshot =
        serde_json::from_value(row.get("model_snapshot")).map_err(|_| ModelError::Invalid)?;
    let safe_input: Value = row.get("safe_input");
    let source_ref = safe_input
        .get("sourceRef")
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or(ModelError::Invalid)?;
    let source_readable: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_comment_research_readable WHERE material_ref=$1)",
    )
    .bind(source_ref)
    .fetch_one(db.pool())
    .await?;
    if !source_readable {
        sqlx::query("UPDATE linggan_comment_replay_explanation SET state='expired',failure_code='source_unavailable',finished_at=scope_001_now() WHERE explanation_ref=$1 AND state='queued'")
            .bind(row.get::<Uuid,_>("explanation_ref")).execute(db.pool()).await?;
        return Ok(true);
    }
    let prompt = difference_prompt(&safe_input)?;
    if prompt.len() > 16_384 || !model.connection_enabled {
        return Ok(false);
    }
    let explanation_ref: Uuid = row.get("explanation_ref");
    let run_ref: Uuid = row.get("run_ref");
    let baseline_item_ref: Uuid = row.get("baseline_item_ref");
    let candidate_item_ref: Uuid = row.get("candidate_item_ref");
    let request_hash = comment_source_hash(&prompt);
    let reserved =
        reserve_difference_explanation(db, explanation_ref, run_ref, &model, &request_hash).await?;
    let Some(invocation_ref) = reserved else {
        return Ok(false);
    };
    let mut request = connection_request(db, store, model.connection_version_ref).await?;
    request.operation = "analyze".into();
    request.model_id = model.model_id.clone();
    request.timeout_ms = u64::try_from(model.timeout_seconds)
        .map_err(|_| ModelError::Invalid)?
        .saturating_mul(1000);
    request.max_output_tokens = EXPLANATION_OUTPUT_TOKENS.min(model.output_token_limit);
    request.system = "你只解释两个已脱敏的影子研究回执为何不同。它们只能说明工程稳定性与分布差异，绝不说明任何结果更准确。不得推断、复原或要求评论原文。只返回 JSON。".into();
    request.prompt = prompt;
    let response = adapter.call(&request).await;
    finish_difference_explanation(
        db,
        explanation_ref,
        invocation_ref,
        baseline_item_ref,
        candidate_item_ref,
        response,
    )
    .await?;
    Ok(true)
}

fn difference_prompt(safe_input: &Value) -> Result<String, ModelError> {
    if !safe_input.is_object() {
        return Err(ModelError::Invalid);
    }
    Ok(json!({
        "task":"解释影子回执的工程差异；稳定性不等于准确率。",
        "output":{
            "kind":"shadow_difference_explanation",
            "stabilityNotAccuracy":true,
            "summary":"不含原文的有限工程解释",
            "citations":[
                {"side":"baseline","itemRef":"输入 baseline.itemRef"},
                {"side":"candidate","itemRef":"输入 candidate.itemRef"}
            ],
            "uncertainty":"不能从这项检查得到的结论"
        },
        "pairedSanitizedResult":safe_input,
    })
    .to_string())
}

async fn reserve_difference_explanation(
    db: &Database,
    explanation_ref: Uuid,
    run_ref: Uuid,
    model: &ReplayModelSnapshot,
    request_hash: &str,
) -> Result<Option<Uuid>, ModelError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SELECT singleton FROM linggan_model_workspace WHERE singleton FOR UPDATE")
        .fetch_one(&mut *tx)
        .await?;
    let explanation = sqlx::query("SELECT state,expires_at>scope_001_now() AS readable FROM linggan_comment_replay_explanation WHERE explanation_ref=$1 FOR UPDATE")
        .bind(explanation_ref).fetch_optional(&mut *tx).await?;
    let Some(explanation) = explanation else {
        tx.commit().await?;
        return Ok(None);
    };
    if explanation.get::<String, _>("state") != "queued" {
        tx.commit().await?;
        return Ok(None);
    }
    if !explanation.get::<bool, _>("readable") {
        sqlx::query("UPDATE linggan_comment_replay_explanation SET state='expired',failure_code='retention_expired',finished_at=scope_001_now() WHERE explanation_ref=$1")
            .bind(explanation_ref).execute(&mut *tx).await?;
        tx.commit().await?;
        return Ok(None);
    }
    let run = sqlx::query("SELECT authorized_token_budget,reserved_tokens,COALESCE(authorization_root_run_ref,run_ref) AS root FROM linggan_comment_replay_run WHERE run_ref=$1 FOR UPDATE")
        .bind(run_ref).fetch_optional(&mut *tx).await?.ok_or(ModelError::NotFound)?;
    let root: Uuid = run.get("root");
    let root_budget: i64 = sqlx::query_scalar(
        "SELECT authorized_token_budget FROM linggan_comment_replay_run WHERE run_ref=$1",
    )
    .bind(root)
    .fetch_one(&mut *tx)
    .await?;
    let root_used = authorization_usage_in(&mut tx, root).await?;
    let run_budget: i64 = run.get("authorized_token_budget");
    let run_reserved: i64 = run.get("reserved_tokens");
    if EXPLANATION_RESERVATION_TOKENS > root_budget.saturating_sub(root_used)
        || EXPLANATION_RESERVATION_TOKENS > run_budget.saturating_sub(run_reserved)
    {
        sqlx::query("UPDATE linggan_comment_replay_explanation SET state='failed',failure_code='replay_authorized_budget_exhausted',finished_at=scope_001_now() WHERE explanation_ref=$1")
            .bind(explanation_ref).execute(&mut *tx).await?;
        tx.commit().await?;
        return Ok(None);
    }
    let daily = check_budget_in(
        &mut tx,
        BudgetPurpose::Replay,
        EXPLANATION_RESERVATION_TOKENS,
        false,
    )
    .await?;
    if !daily.allowed {
        tx.commit().await?;
        return Ok(None);
    }
    let invocation_ref = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens,result) VALUES($1,$2,$3,$4,'analyze',$5,'running',$6,$6,$7)")
        .bind(invocation_ref).bind(model.connection_version_ref).bind(model.model_ref).bind(model.config_ref)
        .bind(request_hash).bind(EXPLANATION_RESERVATION_TOKENS)
        .bind(json!({"budgetPurpose":"replay","shadowDifferenceExplanation":true}))
        .execute(&mut *tx).await?;
    sqlx::query("UPDATE linggan_comment_replay_explanation SET state='running',invocation_ref=$2,reserved_tokens=$3 WHERE explanation_ref=$1 AND state='queued'")
        .bind(explanation_ref).bind(invocation_ref).bind(EXPLANATION_RESERVATION_TOKENS).execute(&mut *tx).await?;
    sqlx::query(
        "UPDATE linggan_comment_replay_run SET reserved_tokens=reserved_tokens+$2 WHERE run_ref=$1",
    )
    .bind(run_ref)
    .bind(EXPLANATION_RESERVATION_TOKENS)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Some(invocation_ref))
}

async fn finish_difference_explanation(
    db: &Database,
    explanation_ref: Uuid,
    invocation_ref: Uuid,
    baseline_item_ref: Uuid,
    candidate_item_ref: Uuid,
    response: Result<PiResponse, ModelError>,
) -> Result<(), ModelError> {
    let provider = response.as_ref().ok();
    let parsed = provider
        .filter(|response| response.ok)
        .and_then(|response| response.text.as_deref())
        .and_then(|text| serde_json::from_str::<DifferenceExplanation>(text).ok());
    let valid = parsed.as_ref().is_some_and(|value| {
        valid_difference_explanation(value, baseline_item_ref, candidate_item_ref)
    });
    let failure = if valid {
        None
    } else if let Some(response) = provider {
        response
            .failure_code
            .as_deref()
            .or(Some("invalid_explanation_citation"))
    } else {
        response.as_ref().err().map(ModelError::code)
    };
    let safe_output = parsed.filter(|value| valid_difference_explanation(value, baseline_item_ref, candidate_item_ref)).map(|value| {
        json!({
            "kind":value.kind,
            "stabilityNotAccuracy":value.stability_not_accuracy,
            "summary":value.summary,
            "citations":value.citations.into_iter().map(|citation|json!({"side":citation.side,"itemRef":citation.item_ref})).collect::<Vec<_>>(),
            "uncertainty":value.uncertainty,
        })
    });
    let mut tx = db.pool().begin().await?;
    finish_invocation_in(
        &mut tx,
        invocation_ref,
        provider,
        valid,
        failure,
        &json!({
            "callStarted":true,
            "shadowDifferenceExplanation":true,
            "stabilityNotAccuracy":true,
            "safe":provider.map(safe_result).unwrap_or(Value::Null),
        }),
    )
    .await?;
    sqlx::query("UPDATE linggan_comment_replay_explanation SET state=$2,result=$3,failure_code=$4,finished_at=scope_001_now() WHERE explanation_ref=$1 AND state='running'")
        .bind(explanation_ref).bind(if valid {"succeeded"} else {"failed"}).bind(safe_output).bind(failure)
        .execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

fn valid_difference_explanation(
    value: &DifferenceExplanation,
    baseline_item_ref: Uuid,
    candidate_item_ref: Uuid,
) -> bool {
    value.kind == "shadow_difference_explanation"
        && value.stability_not_accuracy
        && !value.summary.trim().is_empty()
        && value.summary.chars().count() <= 1_000
        && !value.uncertainty.trim().is_empty()
        && value.uncertainty.chars().count() <= 1_000
        && value.citations.len() == 2
        && value
            .citations
            .iter()
            .any(|citation| citation.side == "baseline" && citation.item_ref == baseline_item_ref)
        && value
            .citations
            .iter()
            .any(|citation| citation.side == "candidate" && citation.item_ref == candidate_item_ref)
}

/// A rollback is attributable only when one `active_health` successor compares the current
/// active candidate against its parent, with program-produced same-input/model evidence and no
/// supplier failure. It changes only the future active pointer and records one immutable receipt.
pub async fn rollback_future_default_if_unhealthy(db: &Database) -> Result<usize, ModelError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SELECT singleton FROM linggan_model_workspace WHERE singleton FOR UPDATE")
        .fetch_one(&mut *tx)
        .await?;
    let row = sqlx::query(
        "SELECT r.run_ref,r.candidate_rule_revision_ref,r.baseline_rule_revision_ref, \
                r.policy_revision,r.comparison,active.revision \
         FROM linggan_comment_replay_run r \
         JOIN linggan_comment_research_rule_active active ON active.singleton \
         WHERE r.run_kind='active_health' AND r.state='degraded' AND r.failure_code IS NULL \
           AND r.comparison->>'sameInputSameModel'='true' \
           AND r.comparison->>'supplierFailureCount'='0' \
           AND r.comparison->>'promptAttributableHealthRegression'='true' \
           AND r.candidate_rule_revision_ref=active.rule_revision_ref \
           AND NOT EXISTS( \
             SELECT 1 FROM linggan_comment_rule_rollback_receipt receipt \
             WHERE receipt.triggering_run_ref=r.run_ref \
           ) \
         ORDER BY r.finished_at DESC,r.run_ref DESC LIMIT 1 FOR UPDATE OF r,active",
    )
    .fetch_optional(&mut *tx)
    .await?;
    let Some(row) = row else {
        tx.commit().await?;
        return Ok(0);
    };
    let before: i32 = row.get("revision");
    let target: Uuid = row.get("baseline_rule_revision_ref");
    sqlx::query("UPDATE linggan_comment_research_rule_active SET rule_revision_ref=$2,revision=revision+1,updated_at=scope_001_now() WHERE singleton AND revision=$1")
        .bind(before).bind(target).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO linggan_comment_rule_rollback_receipt(receipt_ref,triggering_run_ref,policy_revision,candidate_rule_revision_ref,restored_rule_revision_ref,active_revision_before,active_revision_after,health_evidence) VALUES($1,$2,$3,$4,$5,$6,$7,$8)")
        .bind(Uuid::new_v4()).bind(row.get::<Uuid,_>("run_ref")).bind(row.get::<i64,_>("policy_revision"))
        .bind(row.get::<Uuid,_>("candidate_rule_revision_ref")).bind(target).bind(before).bind(before+1)
        .bind(row.get::<Value,_>("comparison")).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(1)
}
