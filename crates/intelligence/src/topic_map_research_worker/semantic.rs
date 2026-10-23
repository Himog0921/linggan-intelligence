//! A frozen proposal is reviewed through its own metered invocation before acceptance.
use super::*;
use analysis::verification;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ReviewProposal {
    pub source_invocation_ref: Uuid,
    pub phase: String,
    pub source: Value,
    pub proposal: Value,
    pub candidates: Value,
    pub recall: Value,
    pub units: Vec<(String, Discussion)>,
    pub cursor: usize,
    pub total_units: usize,
}

impl ReviewProposal {
    pub fn fingerprint(&self) -> Result<String, ModelError> {
        Ok(linggan_evidence::creator_discovery::hash(
            &serde_json::to_string(self).map_err(|_| ModelError::InvalidOutput)?,
        ))
    }
}

pub(super) async fn prepare_review(
    db: &Database,
    input: &ResearchInput,
    row: &sqlx::postgres::PgRow,
    prepared: &mut Prepared,
    input_limit: i64,
    output_limit: i32,
) -> Result<bool, ModelError> {
    let recall: Value = row.get("recall_manifest");
    let Some(pending) = recall.get("semanticReview") else {
        return Ok(false);
    };
    prepared.review_unavailable = true;
    let Ok(proposal) = serde_json::from_value::<ReviewProposal>(pending.clone()) else {
        return Ok(true);
    };
    let source = topic_map_research::reference_manifest(input);
    if proposal.phase != row.get::<String, _>("phase")
        || proposal.cursor != row.get::<i32, _>("resolution_cursor") as usize
        || !super::feedback::source_stamp_equal(
            &proposal.source,
            &row.get::<Value, _>("input_refs"),
        )
        || !super::feedback::source_stamp_equal(&proposal.source, &source)
    {
        return Ok(true);
    }
    let proof = sqlx::query("SELECT i.state,i.result,q.request_manifest,q.phase FROM linggan_model_invocation i JOIN linggan_topic_map_research_request q USING(invocation_ref) WHERE i.invocation_ref=$1 AND q.task_ref=$2 AND q.dispatch_started_at IS NOT NULL")
        .bind(proposal.source_invocation_ref).bind(row.get::<Uuid,_>("task_ref"))
        .fetch_optional(db.pool()).await?;
    let Some(proof) = proof else { return Ok(true) };
    let result: Value = proof.get("result");
    if proof.get::<String, _>("state") != "succeeded"
        || proof.get::<String, _>("phase") != proposal.phase
        || result["accepted"] != false
        || result["proposalReady"] != true
        || result["proposalHash"].as_str() != Some(proposal.fingerprint()?.as_str())
        || !super::feedback::source_stamp_equal(
            &proof.get::<Value, _>("request_manifest")["source"],
            &source,
        )
    {
        return Ok(true);
    }
    let mut packet = if proposal.phase == "resolve" {
        let resolution: ResolutionOutput = match serde_json::from_value(proposal.proposal.clone()) {
            Ok(value) => value,
            Err(_) => return Ok(true),
        };
        let draft: ResearchOutput = match row
            .get::<Option<Value>, _>("distilled_json")
            .and_then(|v| serde_json::from_value(v).ok())
        {
            Some(value) => value,
            None => return Ok(true),
        };
        let all_units = core::units(input, &draft);
        if proposal.units.iter().any(|(id, d)| {
            !all_units.iter().any(|(expected, frozen)| {
                expected == id && serde_json::to_value(frozen).ok() == serde_json::to_value(d).ok()
            })
        }) || analysis::validate_resolution(&resolution, &proposal.units, &proposal.candidates)
            .is_err()
        {
            return Ok(true);
        }
        let packet = verification::resolution_review(
            &proposal.units,
            &resolution,
            &proposal.candidates,
            &input.fragments,
        );
        prepared.draft = Some(draft);
        prepared.units = proposal.units.clone();
        prepared.candidates = proposal.candidates.clone();
        prepared.recall = proposal.recall.clone();
        prepared.total_units = proposal.total_units;
        let domain = row.get::<Uuid, _>("domain_ref");
        let current_version = core::catalog_version(db, domain).await?;
        let current = core::catalog(db, domain).await?;
        prepared.eligible_definitions = current
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| c["definitionRef"].as_str()?.parse().ok())
            .collect();
        let same = prepared
            .candidates
            .as_array()
            .into_iter()
            .flatten()
            .all(|old| {
                current.as_array().into_iter().flatten().any(|c| {
                    c == old
                        || (c["topicRef"] == old["topicRef"]
                            && c["definitionRef"] == old["definitionRef"])
                })
            });
        let new = resolution.decisions.iter().any(|d| d.status == "new");
        if !same || (new && prepared.recall["catalogFingerprint"] != json!(current_version)) {
            prepared.review_unavailable = false;
            prepared.review_catalog_changed = true;
            return Ok(true);
        }
        packet
    } else {
        let output: ResearchOutput = match serde_json::from_value(proposal.proposal.clone()) {
            Ok(value) => value,
            Err(_) => return Ok(true),
        };
        if analysis::validate_output(&output, &input.fragments, &[]).is_err()
            || analysis::validate_context_links(&output, &input.fragments, &input.comment_study)
                .is_err()
        {
            return Ok(true);
        }
        verification::extraction_review(&output, &input.fragments)
    }
    .map_err(|_| ModelError::InvalidOutput)?;
    packet["domain"] = input.domain.clone();
    packet["asOf"] = input.coverage["asOf"].clone();
    prepared.review_packet = packet.clone();
    prepared.system = verification::VERIFY_SYSTEM.into();
    let mut prompt = json!({"contract":verification::VERIFY_CONTRACT,
        "inputTokenLimit":input_limit,"outputTokenLimit":output_limit,
        "input":packet,"outputSchema":verification::verify_schema()});
    match super::feedback::previous_review_feedback(db, row, input, &proposal).await? {
        super::feedback::RuleFeedback::Available(feedback) => {
            prompt["validationFeedback"] = feedback
        }
        super::feedback::RuleFeedback::Unavailable => prepared.rule_feedback_unavailable = true,
        super::feedback::RuleFeedback::NotRequired => {}
    }
    prepared.prompt = prompt.to_string();
    prepared.review = Some(proposal);
    prepared.review_unavailable = false;
    Ok(true)
}

/// Storing a structurally valid proposal never creates a result, topic, or coverage.
pub(super) async fn stage_proposal(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    c: &super::settlement::Completion<'_>,
    proposal: Value,
) -> Result<ReviewProposal, ModelError> {
    let phase: String =
        sqlx::query_scalar("SELECT phase FROM linggan_topic_map_research_task WHERE task_ref=$1")
            .bind(c.task)
            .fetch_one(&mut **tx)
            .await?;
    let pending = ReviewProposal {
        source_invocation_ref: c.invocation,
        phase,
        source: topic_map_research::reference_manifest(c.input),
        proposal,
        candidates: c.prepared.candidates.clone(),
        recall: c.prepared.recall.clone(),
        units: c.prepared.units.clone(),
        cursor: c.prepared.cursor,
        total_units: c.prepared.total_units,
    };
    // Merge rather than replace: backfill authorization remains part of the task.
    sqlx::query("UPDATE linggan_topic_map_research_task SET recall_manifest=jsonb_set(recall_manifest,'{semanticReview}',$2),phase_attempt_count=0 WHERE task_ref=$1")
        .bind(c.task).bind(serde_json::to_value(&pending).map_err(|_|ModelError::InvalidOutput)?)
        .execute(&mut **tx).await?;
    Ok(pending)
}
