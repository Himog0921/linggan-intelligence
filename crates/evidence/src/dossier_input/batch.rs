//! Explicit frozen scope, one operation, per-work transactions and mutually exclusive outcomes.
use super::{Error, MAX_WORKS, backfill, cutoff, read, validate_works};
use crate::creator_discovery::hash;
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

const PLAN_VERSION: &str = "research-dossier-plan.v2";
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BatchOperation {
    NativeBody,
    Hashtags,
    Relayer,
    Snapshot,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlanEntry {
    pub work_ref: Uuid,
    pub expected_revision: String,
    pub material_fingerprint: String,
    pub body_source_version: Option<String>,
    pub relayer_source_hash: String,
    pub detail_refs: Vec<Uuid>,
    pub pending_tag_details: usize,
    pub source_not_provided: usize,
    pub source_invalid: usize,
    pub source_nonempty: usize,
    pub relayer: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BatchPlan {
    pub contract: String,
    pub domain_ref: Uuid,
    pub as_of: String,
    pub layer_version: String,
    pub maximum_works: usize,
    pub operation: BatchOperation,
    pub entries: Vec<PlanEntry>,
    pub plan_hash: String,
}
impl BatchPlan {
    fn digest(&self) -> String {
        let mut copy = self.clone();
        copy.plan_hash.clear();
        hash(&serde_json::to_string(&copy).expect("typed plan"))
    }
    fn validate(&self) -> Result<(), Error> {
        validate_works(&self.entries.iter().map(|e| e.work_ref).collect::<Vec<_>>())?;
        if self.contract != PLAN_VERSION
            || self.layer_version != crate::ocr_layering::VERSION
            || self.maximum_works != MAX_WORKS
            || self.plan_hash != self.digest()
        {
            return Err(Error::Invalid("plan_contract_or_hash_mismatch"));
        }
        Ok(())
    }
}

pub async fn plan(
    database: &Database,
    domain: Uuid,
    works: &[Uuid],
    as_of: Option<&str>,
    operation: BatchOperation,
) -> Result<BatchPlan, Error> {
    validate_works(works)?;
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let as_of = cutoff(&mut tx, as_of).await?;
    let inputs = read::read_in(&mut tx, domain, works, &as_of).await?;
    let dossiers = crate::research_dossier::read_in(&mut tx, works, &as_of).await?;
    let mut entries = vec![];
    for input in inputs {
        let details = backfill::inventory(&mut tx, input.work_ref, &as_of).await?;
        let parsed: Vec<_> = details
            .iter()
            .map(|(_, p, _)| {
                p.as_object()
                    .map(crate::material_hashtag::parse)
                    .ok_or(Error::Invalid("source_record_invalid"))
            })
            .collect::<Result<_, _>>()?;
        entries.push(PlanEntry {
            work_ref: input.work_ref,
            expected_revision: input.dossier_revision,
            material_fingerprint: input.material_fingerprint,
            body_source_version: dossiers[&input.work_ref]
                .materials
                .iter()
                .find(|m| m["kind"] == "body")
                .and_then(|m| m["sourceVersion"].as_str().map(str::to_owned)),
            relayer_source_hash: relayer_source_hash(&dossiers[&input.work_ref]),
            detail_refs: details.iter().map(|(r, _, _)| *r).collect(),
            pending_tag_details: details.iter().filter(|(_, _, done)| !*done).count(),
            source_not_provided: parsed.iter().filter(|p| p.state == "not_provided").count(),
            source_invalid: parsed.iter().filter(|p| p.state == "invalid").count(),
            source_nonempty: parsed.iter().filter(|p| !p.tags.is_empty()).count(),
            relayer: crate::research_dossier::relayer_plan(&dossiers[&input.work_ref]),
        });
    }
    tx.commit().await?;
    let mut result = BatchPlan {
        contract: PLAN_VERSION.into(),
        domain_ref: domain,
        as_of,
        layer_version: crate::ocr_layering::VERSION.into(),
        maximum_works: MAX_WORKS,
        operation,
        entries,
        plan_hash: String::new(),
    };
    result.plan_hash = result.digest();
    Ok(result)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkReceipt {
    pub work_ref: Uuid,
    pub outcome: String,
    pub result: Value,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchReceipt {
    pub contract: &'static str,
    pub plan_hash: String,
    pub operation: BatchOperation,
    pub work_count: usize,
    pub updated: usize,
    pub already_current: usize,
    pub partial: usize,
    pub failed: usize,
    pub layout_updated: u64,
    pub layout_already_current: u64,
    pub layout_skipped: u64,
    pub needs_ocr: u64,
    pub results: Vec<WorkReceipt>,
}
impl BatchReceipt {
    pub fn complete(&self) -> bool {
        self.failed == 0 && self.partial == 0
    }
}

/// Resume by reapplying the saved plan. Completed writes are immutable/idempotent;
/// changed inputs conflict per work and do not prevent the remaining frozen works.
pub async fn apply_plan(database: &Database, plan: &BatchPlan) -> Result<BatchReceipt, Error> {
    plan.validate()?;
    let mut report = BatchReceipt {
        contract: "research-dossier-receipt.v2",
        plan_hash: plan.plan_hash.clone(),
        operation: plan.operation,
        work_count: plan.entries.len(),
        updated: 0,
        already_current: 0,
        partial: 0,
        failed: 0,
        layout_updated: 0,
        layout_already_current: 0,
        layout_skipped: 0,
        needs_ocr: 0,
        results: vec![],
    };
    for entry in &plan.entries {
        let result = apply_work(database, plan, entry).await;
        let (outcome, result) = match result {
            Ok(result) => {
                let partial = result["state"] == "not_acquired"
                    || result["invalid"].as_u64().unwrap_or(0) > 0
                    || result["skipped"].as_u64().unwrap_or(0) > 0
                    || result["needsOcr"].as_u64().unwrap_or(0) > 0;
                report.layout_updated += result["updated"].as_u64().unwrap_or(0);
                report.layout_already_current += result["alreadyCurrent"]
                    .as_u64()
                    .filter(|_| plan.operation == BatchOperation::Relayer)
                    .unwrap_or(0);
                report.layout_skipped += result["skipped"].as_u64().unwrap_or(0);
                report.needs_ocr += result["needsOcr"].as_u64().unwrap_or(0);
                let changed = match plan.operation {
                    BatchOperation::NativeBody => result["state"] == "updated",
                    BatchOperation::Hashtags => result["projected"].as_u64().unwrap_or(0) > 0,
                    BatchOperation::Relayer => result["updated"].as_u64().unwrap_or(0) > 0,
                    BatchOperation::Snapshot => result["inserted"] == true,
                };
                if partial {
                    report.partial += 1;
                    ("partial", result)
                } else if changed {
                    report.updated += 1;
                    ("updated", result)
                } else {
                    report.already_current += 1;
                    ("already_current", result)
                }
            }
            Err(error) => {
                report.failed += 1;
                ("failed", json!({"reason":error.to_string()}))
            }
        };
        report.results.push(WorkReceipt {
            work_ref: entry.work_ref,
            outcome: outcome.into(),
            result,
        });
    }
    Ok(report)
}

async fn apply_work(
    database: &Database,
    plan: &BatchPlan,
    entry: &PlanEntry,
) -> Result<Value, Error> {
    match plan.operation {
        BatchOperation::NativeBody => native_body(database, plan, entry).await,
        BatchOperation::Hashtags => {
            backfill::apply(
                database,
                plan.domain_ref,
                entry.work_ref,
                &plan.as_of,
                &entry.expected_revision,
            )
            .await
        }
        BatchOperation::Snapshot => Ok(json!(
            super::snapshot::save_planned_snapshot(
                database,
                plan.domain_ref,
                entry.work_ref,
                &plan.as_of,
                &entry.expected_revision,
                Some(&entry.material_fingerprint)
            )
            .await?
        )),
        BatchOperation::Relayer => {
            // Reuse the canonical full-source transaction; all eligible layouts are retained.
            // On replay, a fully current work needs no mutation; a changed work that
            // still has pending layouts must retain the original revision guard.
            let mut tx = database.pool().begin().await?;
            read::scope(&mut tx, plan.domain_ref, &[entry.work_ref], &plan.as_of).await?;
            let now = cutoff(&mut tx, None).await?;
            let current = crate::research_dossier::read_in(&mut tx, &[entry.work_ref], &now)
                .await?
                .remove(&entry.work_ref)
                .ok_or(Error::NotFound)?;
            let status = crate::research_dossier::relayer_plan(&current);
            tx.commit().await?;
            if current.revision != entry.expected_revision {
                // Nothing is written on an already-current replay. Explicitly report
                // revision drift, rather than pretending the frozen source stayed unchanged.
                if relayer_source_hash(&current) == entry.relayer_source_hash
                    && status["pendingLayouts"] == 0
                {
                    return Ok(
                        json!({"updated":0,"alreadyCurrent":status["alreadyCurrent"],"skipped":0,"needsOcr":status["needsOcr"],"layerOutputChanged":true,"readOnlyReplay":true}),
                    );
                }
                return Err(Error::Conflict);
            }
            crate::research_dossier::relayer_work(
                database,
                entry.work_ref,
                &entry.expected_revision,
            )
            .await
            .map_err(Error::from)
        }
    }
}

async fn native_body(
    database: &Database,
    plan: &BatchPlan,
    entry: &PlanEntry,
) -> Result<Value, Error> {
    let mut tx = database.pool().begin().await?;
    read::lock_work(&mut tx, entry.work_ref).await?;
    read::scope(&mut tx, plan.domain_ref, &[entry.work_ref], &plan.as_of).await?;
    let now = cutoff(&mut tx, None).await?;
    let current = crate::research_dossier::read_in(&mut tx, &[entry.work_ref], &now)
        .await?
        .remove(&entry.work_ref)
        .ok_or(Error::NotFound)?;
    let body = current
        .materials
        .iter()
        .find(|m| m["kind"] == "body")
        .ok_or(Error::NotFound)?;
    if body["sourceVersion"].as_str() != entry.body_source_version.as_deref() {
        return Err(Error::Conflict);
    }
    if current.revision != entry.expected_revision {
        if body["reading"]["ruleVersion"] == crate::research_dossier::content::RULE_VERSION
            && body["currentRevisionRef"].is_string()
        {
            tx.commit().await?;
            return Ok(json!({"state":"already_current","readOnlyReplay":true}));
        }
        return Err(Error::Conflict);
    }
    let result =
        crate::research_dossier::native::publish_current_body_in(&mut tx, entry.work_ref).await?;
    tx.commit().await?;
    Ok(result)
}

// Outputs change during normalization; source membership and restrictions may not.
fn relayer_source_hash(dossier: &crate::research_dossier::ResearchDossier) -> String {
    let media: Vec<_> = dossier
        .media
        .iter()
        .map(|m| {
            json!([
                m["slotKey"],
                m["sourceVersion"],
                m["layoutRef"],
                m["humanProtected"]
            ])
        })
        .collect();
    let native: Vec<_> = dossier
        .fragments
        .iter()
        .filter(|f| matches!(f.field.as_str(), "title" | "body" | "tags"))
        .collect();
    hash(&json!([native, dossier.media_inventory, media]).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_scope_and_tampered_plan_are_rejected() {
        let mut p = BatchPlan {
            contract: PLAN_VERSION.into(),
            domain_ref: Uuid::nil(),
            as_of: "x".into(),
            layer_version: crate::ocr_layering::VERSION.into(),
            maximum_works: MAX_WORKS,
            operation: BatchOperation::Hashtags,
            entries: vec![],
            plan_hash: String::new(),
        };
        p.plan_hash = p.digest();
        assert!(p.validate().is_err());
        assert!(serde_json::from_value::<BatchPlan>(json!({"unexpected":true})).is_err());
    }
}
