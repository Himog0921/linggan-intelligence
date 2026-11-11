use super::{Command, Database, FoundationError, METHOD, Uuid, Value, source};
use crate::ai_foundation::store::valid_reason;
use crate::content_understanding::local_id;
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Transaction};
use std::collections::BTreeSet;

const MANUAL_METHOD: &str = "content-understanding.l2.manual.v1";

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum FindingType {
    Hook,
    Structure,
    ValueDelivery,
    ExpressionMechanism,
    EngagementHypothesis,
    TransferableMethod,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Finding {
    pub(super) id: String,
    pub(super) finding_type: FindingType,
    pub(super) statement: String,
    pub(super) basis_unit_ids: Vec<String>,
    pub(super) evidence_refs: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublishDeep {
    command_ref: Uuid,
    domain_ref: Uuid,
    work_ref: Uuid,
    core_ref: Uuid,
    question: String,
    findings: Vec<Finding>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepReceipt {
    analysis_ref: Uuid,
    core_ref: Uuid,
}

fn refs_match(refs: &[String], available: &[Value]) -> bool {
    refs.len() <= 10
        && refs.iter().collect::<BTreeSet<_>>().len() == refs.len()
        && refs
            .iter()
            .all(|id| available.iter().any(|row| row["id"].as_str() == Some(id)))
}

pub(super) fn valid_findings(findings: &[Finding], card: &Value) -> bool {
    let (Some(units), Some(evidence)) = (
        card["core"]["knowledgeUnits"].as_array(),
        card["evidence"].as_array(),
    ) else {
        return false;
    };
    let mut ids = BTreeSet::new();
    (1..=20).contains(&findings.len())
        && findings.iter().all(|finding| {
            local_id(&finding.id)
                && ids.insert(&finding.id)
                && valid_reason(&finding.statement, 600)
                && refs_match(&finding.basis_unit_ids, units)
                && !finding.evidence_refs.is_empty()
                && refs_match(&finding.evidence_refs, evidence)
        })
}

pub async fn publish_deep(
    db: &Database,
    request: &PublishDeep,
) -> Result<DeepReceipt, FoundationError> {
    if request.command_ref.is_nil() || !valid_reason(&request.question, 300) {
        return Err(FoundationError::Invalid);
    }
    let mut command = Command::begin(
        db,
        request.command_ref,
        "content_understanding_manual_deep",
        request,
    )
    .await?;
    if let Some(receipt) = command.replay()? {
        return Ok(receipt);
    }
    lock_core(&mut command.tx, request.core_ref).await?;
    let card = current_card_in(
        &mut command.tx,
        request.domain_ref,
        request.work_ref,
        request.core_ref,
    )
    .await?
    .ok_or(FoundationError::Conflict)?;
    if !valid_findings(&request.findings, &card) {
        return Err(FoundationError::Invalid);
    }
    let findings = serde_json::to_value(&request.findings).map_err(|_| FoundationError::Invalid)?;
    sqlx::query("INSERT INTO linggan_content_understanding_deep(analysis_ref,core_ref,domain_ref,method_version,question,findings) VALUES($1,$2,$3,$4,$5,$6)")
        .bind(request.command_ref).bind(request.core_ref).bind(request.domain_ref).bind(MANUAL_METHOD)
        .bind(request.question.trim()).bind(findings)
        .execute(&mut *command.tx).await?;
    command
        .finish(DeepReceipt {
            analysis_ref: request.command_ref,
            core_ref: request.core_ref,
        })
        .await
}

pub(super) async fn lock_core(
    tx: &mut Transaction<'_, Postgres>,
    core: Uuid,
) -> Result<(), FoundationError> {
    // The revision order follows publication order for the same Core.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,197))")
        .bind(core.to_string())
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(super) async fn current_card_in(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    work: Uuid,
    core: Uuid,
) -> Result<Option<Value>, FoundationError> {
    let cutoff = linggan_evidence::research_dossier::cutoff_in(tx, None).await?;
    let material = source::load(tx, domain, work, &cutoff).await?;
    Ok(sqlx::query_scalar("SELECT result->'card' FROM linggan_content_understanding_result
        WHERE request_ref=$1 AND work_ref=$2 AND material_snapshot_identity=$3 AND method_version=$4 AND result IS NOT NULL")
        .bind(core).bind(work).bind(material.snapshot.material_snapshot_identity).bind(METHOD)
        .fetch_optional(&mut **tx).await?)
}
