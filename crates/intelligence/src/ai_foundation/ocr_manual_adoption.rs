//! Human adoption for an already-saved cloud OCR result. This never dispatches a model,
//! creates an audit sample, or modifies the machine observation.
use super::store::{Command, FoundationError, valid_reason};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Adopt {
    pub command_ref: Uuid,
    pub adoption_ref: Uuid,
    pub expected_revision: i64,
    pub decision: String,
    pub field: String,
    pub text: Option<String>,
    pub actor: String,
    pub reason: String,
}

struct Source {
    adoption_ref: Uuid,
    assessment: Value,
    prior_body: String,
}

fn valid(request: &Adopt) -> bool {
    matches!(request.decision.as_str(), "adopt" | "reject" | "pending")
        && request.field == "adopted_body"
        && request.expected_revision >= 0
        && valid_reason(&request.actor, 120)
        && valid_reason(&request.reason, 300)
        && (request.decision == "adopt") == request.text.is_some()
        && request
            .text
            .as_ref()
            .is_none_or(|text| !text.is_empty() && text.chars().count() <= 16_000)
}

async fn source_in(
    conn: &mut sqlx::PgConnection,
    work: Uuid,
    adoption: Uuid,
) -> Result<Option<Source>, FoundationError> {
    let row = sqlx::query(
        "SELECT a.adoption_ref,a.assessment,a.image_text
         FROM linggan_ocr_recognition_item i
         JOIN linggan_ocr_recognition_run r USING(run_ref)
         JOIN linggan_media_ocr_adoption a ON a.adoption_ref=i.adoption_ref
         JOIN linggan_media_ocr_layout l USING(layout_ref)
         WHERE r.content_public_ref=$1 AND i.adoption_ref=$2
           AND l.content_public_ref=$1
           AND i.state='succeeded' AND a.rule_version='cloud-ocr-adoption.v1'
           AND l.engine='cloud_manual'
           AND NOT EXISTS(SELECT 1 FROM linggan_current_material_media_disposition d
             WHERE d.slot_key=i.slot_key OR d.blob_sha256=i.blob_sha256
                OR d.materialization_ref=i.materialization_ref)
           AND NOT EXISTS(SELECT 1 FROM linggan_media_processing_job j
             JOIN linggan_media_ocr_retirement retired ON retired.retired_job_ref=j.job_ref
             WHERE j.job_ref=(SELECT job_ref FROM linggan_media_derivative WHERE derivative_ref=a.derivative_ref))",
    )
    .bind(work)
    .bind(adoption)
    .fetch_optional(conn)
    .await?;
    Ok(row.map(|row| Source {
        adoption_ref: row.get("adoption_ref"),
        assessment: row.get("assessment"),
        prior_body: row.get("image_text"),
    }))
}

fn literal(assessment: &Value, text: &str) -> Option<(String, i32, i32)> {
    let mut found = Vec::new();
    for region in assessment["regions"].as_array()? {
        let (Some(reference), Some(original)) =
            (region["regionRef"].as_str(), region["text"].as_str())
        else {
            continue;
        };
        for (byte_offset, _) in original.char_indices() {
            if !original[byte_offset..].starts_with(text) {
                continue;
            }
            let start = original[..byte_offset].chars().count();
            found.push((
                reference.to_owned(),
                start as i32,
                (start + text.chars().count()) as i32,
            ));
        }
    }
    (found.len() == 1).then(|| found.remove(0))
}

/// Appends a manual decision only after the saved result is still visible in this exact work.
pub async fn adopt(db: &Database, work: Uuid, request: &Adopt) -> Result<Value, FoundationError> {
    if !valid(request) {
        return Err(FoundationError::Invalid);
    }
    let mut command = Command::begin(
        db,
        request.command_ref,
        "manual_ocr_human_adoption",
        request,
    )
    .await?;
    if let Some(receipt) = command.replay::<Value>()? {
        let adoption: Uuid = serde_json::from_value(receipt["adoptionRef"].clone())
            .map_err(|_| FoundationError::Invalid)?;
        if source_in(&mut command.tx, work, adoption).await?.is_none() {
            return Err(FoundationError::NotFound);
        }
        return Ok(receipt);
    }
    let source = source_in(&mut command.tx, work, request.adoption_ref)
        .await?
        .ok_or(FoundationError::NotFound)?;
    sqlx::query(
        "SELECT adoption_ref FROM linggan_media_ocr_adoption WHERE adoption_ref=$1 FOR UPDATE",
    )
    .bind(source.adoption_ref)
    .fetch_one(&mut *command.tx)
    .await?;
    let revision: i64 = sqlx::query_scalar(
        "SELECT COALESCE(max(revision),0) FROM linggan_media_ocr_review WHERE adoption_ref=$1",
    )
    .bind(source.adoption_ref)
    .fetch_one(&mut *command.tx)
    .await?;
    if revision != request.expected_revision {
        return Err(FoundationError::Conflict);
    }
    let previous: Option<String> = sqlx::query_scalar(
        "SELECT after_text FROM linggan_media_ocr_review
         WHERE adoption_ref=$1 AND field=$2 AND decision='adopt'
         ORDER BY revision DESC LIMIT 1",
    )
    .bind(source.adoption_ref)
    .bind(&request.field)
    .fetch_optional(&mut *command.tx)
    .await?;
    let before = previous.unwrap_or(source.prior_body);
    let literal = request
        .text
        .as_deref()
        .and_then(|text| literal(&source.assessment, text));
    let source_kind = (request.decision == "adopt").then(|| {
        if literal.is_some() {
            "ocr_block"
        } else {
            "manual_transcription"
        }
    });
    let receipt = json!({
        "reviewRef":request.command_ref,"adoptionRef":source.adoption_ref,
        "revision":revision + 1,"decision":request.decision,"field":request.field,
        "beforeText":before,"afterText":request.text,"sourceKind":source_kind,
        "blockRef":literal.as_ref().map(|value| &value.0),
        "sourceStart":literal.as_ref().map(|value| value.1),
        "sourceEnd":literal.as_ref().map(|value| value.2),"auditSampleRef":Value::Null,
    });
    sqlx::query(
        "INSERT INTO linggan_media_ocr_review
         (review_ref,sample_ref,issue_ref,adoption_ref,revision,decision,field,before_text,after_text,source_kind,block_ref,source_start,source_end,actor,reason,metrics)
         VALUES($1,NULL,NULL,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,'{}'::jsonb)",
    )
    .bind(request.command_ref)
    .bind(source.adoption_ref)
    .bind(revision + 1)
    .bind(&request.decision)
    .bind(&request.field)
    .bind(&receipt["beforeText"])
    .bind(&request.text)
    .bind(source_kind)
    .bind(literal.as_ref().map(|value| &value.0))
    .bind(literal.as_ref().map(|value| value.1))
    .bind(literal.as_ref().map(|value| value.2))
    .bind(&request.actor)
    .bind(&request.reason)
    .execute(&mut *command.tx)
    .await?;
    command.finish(receipt).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn literal_requires_one_saved_region_range() {
        let assessment = json!({"regions":[
          {"regionRef":"a","text":"甲乙丙"},{"regionRef":"b","text":"丁戊"}
        ]});
        assert_eq!(literal(&assessment, "乙丙"), Some(("a".into(), 1, 3)));
        assert_eq!(literal(&assessment, "甲"), Some(("a".into(), 0, 1)));
        assert_eq!(literal(&assessment, "不存在"), None);
    }
    #[test]
    fn literal_refuses_overlapping_ranges() {
        let assessment = json!({"regions":[{"regionRef":"a","text":"aaaa"}]});
        assert_eq!(literal(&assessment, "aaa"), None);
    }
}
