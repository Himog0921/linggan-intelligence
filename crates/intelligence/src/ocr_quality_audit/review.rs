//! Human commands append a new adoption version; raw OCR and platform titles never change.
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
    pub field: String,
    pub text: Option<String>,
    pub actor: String,
    pub reason: String,
    pub metrics: HumanMetrics,
}
pub async fn review(db: &Database, r: &Review) -> Result<Value, FoundationError> {
    if !matches!(r.decision.as_str(), "adopt" | "reject" | "pending")
        || !matches!(r.field.as_str(), "adopted_body" | "cover_headline")
        || !valid_reason(&r.actor, 120)
        || !valid_reason(&r.reason, 300)
        || !r.metrics.validate(r.decision == "pending")
        || (r.decision == "adopt") != r.text.is_some()
        || r.text.as_ref().is_some_and(|s| s.chars().count() > 16_000)
    {
        return Err(FoundationError::Invalid);
    }
    let mut c = Command::begin(db, r.command_ref, "ocr_audit_review", r).await?;
    if let Some(v) = c.replay::<Value>()? {
        let id: Uuid = serde_json::from_value(v["adoptionRef"].clone())
            .map_err(|_| FoundationError::Invalid)?;
        if source::load_in(&mut c.tx, id).await?.is_none() {
            return Err(FoundationError::NotFound);
        }
        return Ok(v);
    }
    let id: Option<Uuid> =
        sqlx::query_scalar("SELECT adoption_ref FROM linggan_ocr_audit_sample WHERE sample_ref=$1")
            .bind(r.sample_ref)
            .fetch_optional(&mut *c.tx)
            .await?
            .flatten();
    let id = id.ok_or(FoundationError::NotFound)?;
    sqlx::query(
        "SELECT adoption_ref FROM linggan_media_ocr_adoption WHERE adoption_ref=$1 FOR UPDATE",
    )
    .bind(id)
    .fetch_one(&mut *c.tx)
    .await?;
    if let Some(issue) = r.issue_ref {
        let matches:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_ocr_audit_issue WHERE issue_ref=$1 AND sample_ref=$2)").bind(issue).bind(r.sample_ref).fetch_one(&mut *c.tx).await?;
        if !matches {
            return Err(FoundationError::Invalid);
        }
    }
    let source = source::load_in(&mut c.tx, id)
        .await?
        .ok_or(FoundationError::NotFound)?;
    let revision: i64 = sqlx::query_scalar(
        "SELECT COALESCE(max(revision),0) FROM linggan_media_ocr_review WHERE adoption_ref=$1",
    )
    .bind(id)
    .fetch_one(&mut *c.tx)
    .await?;
    if revision != r.expected_revision {
        return Err(FoundationError::Conflict);
    }
    if r.field == "cover_headline"
        && !matches!(
            source.assessment.image_role,
            linggan_evidence::ocr_adoption::ImageRole::FirstImage
                | linggan_evidence::ocr_adoption::ImageRole::ExplicitCover
        )
    {
        return Err(FoundationError::Invalid);
    }
    let previous:Option<String>=sqlx::query_scalar("SELECT after_text FROM linggan_media_ocr_review WHERE adoption_ref=$1 AND field=$2 AND decision='adopt' ORDER BY revision DESC LIMIT 1").bind(id).bind(&r.field).fetch_optional(&mut *c.tx).await?;
    let before = previous.unwrap_or_else(|| {
        if r.field == "cover_headline" {
            source
                .assessment
                .cover_title
                .text
                .clone()
                .unwrap_or_default()
        } else {
            linggan_evidence::ocr_adoption::project_work(std::slice::from_ref(&source.assessment))
                .ok()
                .and_then(|p| p.images.into_iter().next())
                .map(|i| i.text)
                .unwrap_or_default()
        }
    });
    let literal = r.text.as_ref().filter(|s| !s.is_empty()).and_then(|text| {
        let matches: Vec<_> = source
            .input
            .layout
            .blocks
            .iter()
            .flat_map(|b| b.text.match_indices(text).map(move |(n, _)| (b, n)))
            .collect();
        if matches.len() != 1 {
            return None;
        }
        let (b, n) = matches[0];
        let start = b.text[..n].chars().count();
        Some((
            b.block_ref.clone(),
            start as i32,
            (start + text.chars().count()) as i32,
        ))
    });
    let kind = if r.decision == "adopt" {
        Some(if literal.is_some() {
            "ocr_block"
        } else {
            "manual_transcription"
        })
    } else {
        None
    };
    let record = json!({"reviewRef":r.command_ref,"adoptionRef":id,"revision":revision+1,"decision":r.decision,"field":r.field,"beforeText":before,"afterText":r.text,"sourceKind":kind,"blockRef":literal.as_ref().map(|l|&l.0),"sourceStart":literal.as_ref().map(|l|l.1),"sourceEnd":literal.as_ref().map(|l|l.2),"imageRef":source.slot_key,"blobSha256":source.input.image.blob_sha256});
    sqlx::query("INSERT INTO linggan_media_ocr_review(review_ref,sample_ref,issue_ref,adoption_ref,revision,decision,field,before_text,after_text,source_kind,block_ref,source_start,source_end,actor,reason,metrics) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16)")
 .bind(r.command_ref).bind(r.sample_ref).bind(r.issue_ref).bind(id).bind(revision+1).bind(&r.decision).bind(&r.field).bind(before).bind(&r.text).bind(kind).bind(literal.as_ref().map(|l|&l.0)).bind(literal.as_ref().map(|l|l.1)).bind(literal.as_ref().map(|l|l.2)).bind(&r.actor).bind(&r.reason).bind(json!(r.metrics)).execute(&mut *c.tx).await?;
    c.finish(record).await
}
