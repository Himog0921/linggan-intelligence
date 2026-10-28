//! Comment problem definitions and membership, admitted only after independent review.
use super::{comment_research, store::FoundationError};
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;
pub const CONTRACT: &str = "comment-problem-stage.v1";
mod contract;
pub(crate) use contract::validate;
use contract::{Problem, Resolution, schema};
fn denied(code: &str) -> FoundationError {
    FoundationError::AdmissionDenied(vec![code.into()])
}
fn bounded(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.chars().count() <= max
}
async fn semantic_in(
    tx: &mut Transaction<'_, Postgres>,
    request: Uuid,
) -> Result<Value, FoundationError> {
    sqlx::query_scalar("SELECT result FROM linggan_comment_research_result WHERE request_ref=$1 AND result IS NOT NULL").bind(request).fetch_optional(&mut **tx).await?.ok_or_else(||denied("comment_dependency_failed"))
}
fn eligible(semantic: &Value) -> Vec<(usize, Value)> {
    semantic["signals"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
        .filter(|(_, s)| {
            s["eligibilityState"] == "eligible" && (s["kind"] == "problem" || s["kind"] == "need")
        })
        .map(|(i, s)| (i, s.clone()))
        .collect()
}
async fn definitions_in(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
) -> Result<Value, FoundationError> {
    let rows=sqlx::query("SELECT definition_ref,source_request_ref,jsonb_build_object('definitionRef',definition_ref,'problemRef',problem_ref,'label',label,'actor',actor,'goal',goal,'barrier',barrier,'context',context,'identityHash',identity_hash,'sourceRequestRef',source_request_ref,'sourceSignalOrdinal',source_signal_ordinal) AS definition FROM linggan_comment_problem_definition WHERE domain_ref=$1 ORDER BY definition_ref").bind(domain).fetch_all(&mut **tx).await?;
    let mut qualified = std::collections::HashMap::new();
    let mut result = Vec::new();
    for row in rows {
        let request: Uuid = row.get("source_request_ref");
        if !qualified.contains_key(&request) {
            let okay = match comment_research::frozen_in(tx, request).await {
                Ok(_) => true,
                Err(FoundationError::AdmissionDenied(_)) => false,
                Err(e) => return Err(e),
            };
            qualified.insert(request, okay);
        }
        if qualified[&request] {
            let definition: Uuid = row.get("definition_ref");
            let supporters:Vec<Uuid>=sqlx::query_scalar("SELECT m.request_ref FROM linggan_comment_problem_membership m JOIN linggan_comment_problem_definition d USING(definition_ref) WHERE m.definition_ref=$1 AND m.review_invocation_ref=d.review_invocation_ref").bind(definition).fetch_all(&mut **tx).await?;
            let mut authors = std::collections::HashSet::new();
            for supporter in supporters {
                match comment_research::frozen_in(tx, supporter).await {
                    Ok(_) => {
                        authors.insert(author_in(tx, supporter).await?);
                    }
                    Err(FoundationError::AdmissionDenied(_)) => {}
                    Err(e) => return Err(e),
                }
            }
            if authors.len() >= 2 {
                result.push(row.get::<Value, _>("definition"));
            }
        }
    }
    Ok(json!(result))
}
fn problem_identity(p: &Problem) -> String {
    let normalize = |s: &str| {
        s.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase()
    };
    crate::research_text::content_hash(
        &json!([
            normalize(&p.actor),
            normalize(&p.goal),
            normalize(&p.barrier),
            p.context.as_deref().map(normalize)
        ])
        .to_string(),
    )
}
async fn proposals_in(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
) -> Result<Vec<(Uuid, Resolution)>, FoundationError> {
    let rows=sqlx::query("SELECT s.request_ref,result.output FROM linggan_comment_research_stage s JOIN linggan_comment_problem_stage_result result USING(item_ref) JOIN linggan_comment_research_request r USING(request_ref) WHERE s.stage_number=1 AND r.domain_ref=$1 AND result.output IS NOT NULL AND NOT EXISTS(SELECT 1 FROM linggan_comment_research_cancellation cancelled WHERE cancelled.request_ref=r.request_ref) AND NOT EXISTS(SELECT 1 FROM linggan_comment_research_stage cancelled_stage JOIN linggan_ai_batch_item cancelled_item USING(item_ref) JOIN linggan_ai_batch_control cancelled_batch USING(batch_ref) JOIN linggan_ai_batch_item_control cancelled_control USING(item_ref) WHERE cancelled_stage.request_ref=r.request_ref AND (cancelled_batch.cancel_requested OR cancelled_control.state='cancelled')) AND EXISTS(SELECT 1 FROM jsonb_array_elements(result.output->'decisions') decision WHERE decision->>'status'='new' AND NOT EXISTS(SELECT 1 FROM linggan_comment_problem_membership membership WHERE membership.request_ref=r.request_ref AND membership.signal_ordinal=(decision->>'signalOrdinal')::integer)) ORDER BY r.created_at,r.request_ref").bind(domain).fetch_all(&mut **tx).await?;
    let mut proposals = Vec::new();
    for row in rows {
        let request = row.get("request_ref");
        match comment_research::frozen_in(tx, request).await {
            Ok(_) => {}
            Err(FoundationError::AdmissionDenied(_)) => continue,
            Err(e) => return Err(e),
        };
        let proposal: Resolution =
            serde_json::from_value(row.get("output")).map_err(|_| FoundationError::Invalid)?;
        proposals.push((request, proposal));
    }
    Ok(proposals)
}
async fn author_in(
    tx: &mut Transaction<'_, Postgres>,
    request: Uuid,
) -> Result<String, FoundationError> {
    sqlx::query_scalar::<_,Option<String>>("SELECT NULLIF(btrim(source.author_external_id),'') FROM linggan_comment_research_request r JOIN linggan_material_comment source ON source.material_ref=r.source_ref WHERE r.request_ref=$1").bind(request).fetch_one(&mut **tx).await?.ok_or_else(||denied("ai_source_unavailable"))
}
async fn pair_support_in(
    tx: &mut Transaction<'_, Postgres>,
    request: Uuid,
    domain: Uuid,
    proposal: &Value,
) -> Result<(Value, Value), FoundationError> {
    let own: Resolution =
        serde_json::from_value(proposal.clone()).map_err(|_| FoundationError::Invalid)?;
    let author = author_in(tx, request).await?;
    let proposals = proposals_in(tx, domain).await?;
    let mut supports = serde_json::Map::new();
    let mut evidence = Vec::new();
    for decision in own.decisions.iter().filter(|d| d.status == "new") {
        let identity = problem_identity(decision.problem.as_ref().ok_or(FoundationError::Invalid)?);
        let mut found = None;
        for (other, prior) in &proposals {
            if *other == request || author_in(tx, *other).await? == author {
                continue;
            }
            let Some(candidate) = prior.decisions.iter().find(|d| {
                d.status == "new"
                    && d.problem
                        .as_ref()
                        .is_some_and(|p| problem_identity(p) == identity)
            }) else {
                continue;
            };
            let (source, _) = comment_research::frozen_in(tx, *other).await?;
            let semantic = semantic_in(tx, *other).await?;
            let signal = &semantic["signals"][candidate.signal_ordinal];
            let start = signal["evidenceSpan"]["start"]
                .as_u64()
                .ok_or(FoundationError::Invalid)? as usize;
            let end = signal["evidenceSpan"]["end"]
                .as_u64()
                .ok_or(FoundationError::Invalid)? as usize;
            let quote: String = source
                .raw_text
                .chars()
                .skip(start)
                .take(end.saturating_sub(start))
                .collect();
            evidence.push(json!({"signalOrdinal":decision.signal_ordinal,"independentSource":true,"supportingSignal":signal,"supportingQuote":quote}));
            found = Some(
                json!({"requestRef":other,"signalOrdinal":candidate.signal_ordinal,"identity":identity}),
            );
            break;
        }
        supports.insert(
            decision.signal_ordinal.to_string(),
            found.ok_or_else(|| denied("comment_problem_support_pending"))?,
        );
    }
    Ok((Value::Object(supports), json!(evidence)))
}
pub(crate) struct Packet {
    pub payload: Value,
    pub context: Value,
    pub schema: Value,
}
pub(crate) async fn prepare(
    tx: &mut Transaction<'_, Postgres>,
    request: Uuid,
    stage: u8,
) -> Result<Packet, FoundationError> {
    let semantic = semantic_in(tx, request).await?;
    let signals = eligible(&semantic);
    if signals.is_empty() {
        return Err(denied("comment_no_problem_signal"));
    }
    let (source, _) = comment_research::frozen_in(tx, request).await?;
    let domain: Uuid = sqlx::query_scalar(
        "SELECT domain_ref FROM linggan_comment_research_request WHERE request_ref=$1",
    )
    .bind(request)
    .fetch_one(&mut **tx)
    .await?;
    let (proposal, candidates) = if stage == 1 {
        (Value::Null, definitions_in(tx, domain).await?)
    } else {
        let row=sqlx::query("SELECT result.output,dispatch.context FROM linggan_comment_research_stage s JOIN linggan_comment_problem_stage_result result USING(item_ref) JOIN linggan_comment_problem_dispatch dispatch USING(item_ref) WHERE s.request_ref=$1 AND s.stage_number=1").bind(request).fetch_optional(&mut **tx).await?.ok_or_else(||denied("comment_dependency_failed"))?;
        (
            row.get::<Option<Value>, _>("output")
                .ok_or_else(|| denied("comment_dependency_failed"))?,
            row.get::<Value, _>("context")["candidates"].clone(),
        )
    };
    if stage == 2 {
        let current = definitions_in(tx, domain).await?;
        if candidates.as_array().into_iter().flatten().any(|c| {
            !current
                .as_array()
                .unwrap()
                .iter()
                .any(|q| q["definitionRef"] == c["definitionRef"])
        }) {
            return Err(denied("ai_source_unavailable"));
        }
    }
    let pending = if stage == 1 {
        json!(
            proposals_in(tx, domain)
                .await?
                .iter()
                .flat_map(|(_, p)| p.decisions.iter().filter_map(|d| d.problem.as_ref()))
                .collect::<Vec<_>>()
        )
    } else {
        json!([])
    };
    let (supports, pair_evidence) = if stage == 2 {
        pair_support_in(tx, request, domain, &proposal).await?
    } else {
        (json!({}), json!([]))
    };
    let context = json!({"candidates":candidates,"signalOrdinals":signals.iter().map(|(i,_)|i).collect::<Vec<_>>(),"proposal":proposal,"supports":supports});
    let payload = json!({"contract":if stage==1{"comment-problem-resolution.v1"}else{"comment-problem-review.v1"},"targetComment":crate::comment_cleaning::clean(&source.raw_text).text,"signals":signals.iter().map(|(ordinal,s)|json!({"signalOrdinal":ordinal,"signal":s})).collect::<Vec<_>>(),"candidates":candidates,"pendingProposals":pending,"pairedEvidence":pair_evidence,"proposal":proposal});
    Ok(Packet {
        payload,
        context,
        schema: schema(stage),
    })
}
mod publish;
pub(crate) use publish::{lock_publication_in, publish_in};
mod read;
pub(super) use read::pending_supported_in;
pub use read::read_problems;
