//! Audit opinions do not modify material. Corrections use the common reading command.
use super::source;
use crate::ai_foundation::store::{Command, FoundationError, valid_reason};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Count {
    pub numerator: u32,
    pub denominator: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HumanMetrics {
    pub title_false_adoption: Count,
    pub title_missing: Count,
    pub body_missing: Count,
    pub hallucinated_text: Count,
    pub disputed_recovery: Count,
}
impl HumanMetrics {
    pub fn pairs(&self) -> [(&'static str, &Count); 5] {
        [
            ("titleFalseAdoption", &self.title_false_adoption),
            ("titleMissing", &self.title_missing),
            ("bodyMissing", &self.body_missing),
            ("hallucinatedText", &self.hallucinated_text),
            ("disputedRecovery", &self.disputed_recovery),
        ]
    }
    fn validate(&self, pending: bool) -> bool {
        self.pairs().iter().all(|(_, c)| {
            c.numerator <= c.denominator
                && c.denominator <= 10_000
                && (!pending || c.denominator == 0)
        })
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Review {
    pub command_ref: Uuid,
    pub sample_ref: Uuid,
    pub issue_ref: Option<Uuid>,
    pub expected_revision: i64,
    pub decision: String,
    pub material_review_command_ref: Option<Uuid>,
    pub actor: String,
    pub reason: String,
    pub metrics: HumanMetrics,
}
pub async fn review(db: &Database, r: &Review) -> Result<Value, FoundationError> {
    if !matches!(r.decision.as_str(), "confirmed" | "dismissed" | "pending")
        || !valid_reason(&r.actor, 120)
        || !valid_reason(&r.reason, 300)
        || !r.metrics.validate(r.decision == "pending")
    {
        return Err(FoundationError::Invalid);
    }
    let mut c = Command::begin(db, r.command_ref, "ocr_audit_review", r).await?;
    if let Some(value) = c.replay::<Value>()? {
        return Ok(value);
    }
    let reading: Option<Uuid> = sqlx::query_scalar(
        "SELECT reading_ref FROM linggan_ocr_audit_sample WHERE sample_ref=$1 FOR UPDATE",
    )
    .bind(r.sample_ref)
    .fetch_optional(&mut *c.tx)
    .await?
    .flatten();
    let reading = reading.ok_or(FoundationError::NotFound)?;
    let source = source::load_in(&mut c.tx, reading)
        .await?
        .ok_or(FoundationError::NotFound)?;
    if let Some(issue) = r.issue_ref {
        let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_ocr_audit_issue WHERE issue_ref=$1 AND sample_ref=$2)")
            .bind(issue).bind(r.sample_ref).fetch_one(&mut *c.tx).await?;
        if !valid {
            return Err(FoundationError::Invalid);
        }
    }
    if let Some(command) = r.material_review_command_ref {
        let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_material_reading_command WHERE command_ref=$1 AND unit_ref=$2 AND (candidate_reading_ref=$3 OR expected_reading_ref=$3))")
            .bind(command).bind(source.unit_ref).bind(reading).fetch_one(&mut *c.tx).await?;
        if !valid {
            return Err(FoundationError::Invalid);
        }
    }
    let revision: i64 = sqlx::query_scalar(
        "SELECT COALESCE(max(revision),0) FROM linggan_ocr_audit_review WHERE sample_ref=$1",
    )
    .bind(r.sample_ref)
    .fetch_one(&mut *c.tx)
    .await?;
    if revision != r.expected_revision {
        return Err(FoundationError::Conflict);
    }
    sqlx::query("INSERT INTO linggan_ocr_audit_review(review_ref,sample_ref,issue_ref,reading_ref,revision,decision,material_review_command_ref,actor,reason,metrics) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)")
        .bind(r.command_ref).bind(r.sample_ref).bind(r.issue_ref).bind(reading).bind(revision+1).bind(&r.decision)
        .bind(r.material_review_command_ref).bind(&r.actor).bind(&r.reason).bind(json!(r.metrics)).execute(&mut *c.tx).await?;
    c.finish(json!({"reviewRef":r.command_ref,"sampleRef":r.sample_ref,"readingRef":reading,"revision":revision+1,"decision":r.decision,"materialReviewCommandRef":r.material_review_command_ref})).await
}
