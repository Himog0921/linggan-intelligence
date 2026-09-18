//! Filling the embedding cache for eligible Signals.
//!
//! Vectors only ever serve recall: nothing here admits a Signal to a Problem, and a missing vector
//! degrades recall rather than blocking extraction or display. Encoding is deliberately the least
//! privileged step in the module.
//!
//! Two limits are not ours to invent. The local runtime is a single resident process behind a
//! mutex, so encoding is already serial; and a machine that is busy running local OCR or ASR is
//! busy with the same scarce resource, so this step stands down instead of competing with it.

use crate::comment_study_canonical::PREPROCESSING_REVISION;
use crate::model_settings::ModelError;
use crate::pi_adapter::PiAdapter;
use linggan_storage_postgres::Database;
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::Row;
use thiserror::Error;
use uuid::Uuid;

/// Must match `MODEL_ID` in `apps/pi-adapter/src/wemm_runtime.py`; the runtime reports it back on
/// its ready line, and a profile that named a different model would describe vectors nobody
/// produced.
pub const WEMM_MODEL_ID: &str = "Tencent/WeMM-Embedding-2B";
pub const WEMM_DIMENSION: i32 = 512;

/// One tick encodes a bounded slice. The point is not throughput but staying a background
/// citizen on a machine that also serves the API, the database and media processing.
const MAX_TEXTS_PER_TICK: usize = 16;

#[derive(Debug, Error)]
pub enum EmbeddingError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Model(#[from] ModelError),
    #[error("the local runtime returned a vector that is not one finite 512-dimension unit vector")]
    InvalidVector,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "outcome")]
pub enum EmbeddingOutcome {
    /// Nothing was encoded, and the reason is named rather than reported as "nothing to do":
    /// an absent profile and an idle queue are different facts.
    Stood {
        reason: &'static str,
    },
    Encoded {
        encoded: usize,
        pending_after: i64,
    },
}

/// Registers the fixed encoding identity, or returns the existing one with the same fingerprint.
///
/// `qualification` is the evidence that this exact combination was measured on this machine — the
/// manual refuses to accept an upstream CUDA example as proof that MPS works here. A profile is
/// only `qualified` when such evidence is supplied; without it the cache stays empty and recall
/// honestly reports itself incomplete.
pub async fn register_embedding_profile(
    database: &Database,
    model_revision: &str,
    runtime_manifest: Value,
    qualification: Option<Value>,
) -> Result<Uuid, EmbeddingError> {
    let identity = json!({
        "modelId": WEMM_MODEL_ID,
        "modelRevision": model_revision,
        "dimension": WEMM_DIMENSION,
        "encodingMode": "document",
        "preprocessingRevision": PREPROCESSING_REVISION,
        "runtimeManifest": runtime_manifest,
    });
    let fingerprint = sha256_json(&identity);
    if let Some(existing) = sqlx::query_scalar::<_, Uuid>(
        "SELECT profile_ref FROM linggan_comment_study_embedding_profile WHERE fingerprint=$1",
    )
    .bind(&fingerprint)
    .fetch_optional(database.pool())
    .await?
    {
        return Ok(existing);
    }
    let profile_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_comment_study_embedding_profile( \
           profile_ref,fingerprint,model_id,model_revision,dimension,encoding_mode, \
           preprocessing_revision,runtime_manifest,qualification,state,qualified_at \
         ) VALUES($1,$2,$3,$4,$5,'document',$6,$7,COALESCE($8,'{}'::jsonb), \
                  CASE WHEN $8 IS NULL THEN 'unverified' ELSE 'qualified' END, \
                  CASE WHEN $8 IS NULL THEN NULL ELSE scope_001_now() END) \
         ON CONFLICT(fingerprint) DO NOTHING",
    )
    .bind(profile_ref)
    .bind(&fingerprint)
    .bind(WEMM_MODEL_ID)
    .bind(model_revision)
    .bind(WEMM_DIMENSION)
    .bind(PREPROCESSING_REVISION)
    .bind(&runtime_manifest)
    .bind(&qualification)
    .execute(database.pool())
    .await?;
    sqlx::query_scalar(
        "SELECT profile_ref FROM linggan_comment_study_embedding_profile WHERE fingerprint=$1",
    )
    .bind(&fingerprint)
    .fetch_one(database.pool())
    .await
    .map_err(EmbeddingError::Database)
}

/// Encodes eligible Signals whose canonical text has no vector under the active profile.
///
/// Work is chosen by canonical hash, not by Signal: two Signals that reduce to the same canonical
/// sentence share one vector, which is also why re-running a Run costs nothing here.
pub async fn embed_pending_signals(
    database: &Database,
    adapter: &PiAdapter,
) -> Result<EmbeddingOutcome, EmbeddingError> {
    let Some(profile_ref) = active_profile(database).await? else {
        return Ok(EmbeddingOutcome::Stood {
            reason: "no_qualified_profile",
        });
    };
    if heavy_local_work_in_flight(database).await? {
        return Ok(EmbeddingOutcome::Stood {
            reason: "heavy_local_work_in_flight",
        });
    }
    let pending = pending_canonical_texts(database, profile_ref).await?;
    if pending.is_empty() {
        return Ok(EmbeddingOutcome::Stood {
            reason: "no_pending_canonical_text",
        });
    }
    let mut encoded = 0;
    for (canonical_hash, canonical_text) in pending {
        let response = adapter.embed_wemm_document(&canonical_text).await?;
        let vector = single_unit_vector(&response)?;
        sqlx::query(
            "INSERT INTO linggan_comment_study_embedding_cache(profile_ref,canonical_hash,embedding) \
             VALUES($1,$2,$3::text::public.vector) ON CONFLICT DO NOTHING",
        )
        .bind(profile_ref)
        .bind(&canonical_hash)
        .bind(pgvector_literal(&vector))
        .execute(database.pool())
        .await?;
        encoded += 1;
    }
    Ok(EmbeddingOutcome::Encoded {
        encoded,
        pending_after: pending_count(database, profile_ref).await?,
    })
}

pub async fn active_profile(database: &Database) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT profile_ref FROM linggan_comment_study_embedding_profile WHERE state='qualified'",
    )
    .fetch_optional(database.pool())
    .await
}

/// A leased media job is the same machine doing the same kind of expensive local work. The
/// concurrency registry is the existing definition of what counts as heavy; this step reads it
/// rather than keeping a second opinion about it.
async fn heavy_local_work_in_flight(database: &Database) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS( \
           SELECT 1 FROM linggan_media_processing_work work \
           JOIN linggan_media_processing_job job USING(job_ref) \
           JOIN linggan_media_processing_concurrency concurrency \
             ON concurrency.processor_kind=job.processor_kind \
           WHERE work.state='leased')",
    )
    .fetch_one(database.pool())
    .await
}

async fn pending_canonical_texts(
    database: &Database,
    profile_ref: Uuid,
) -> Result<Vec<(String, String)>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT DISTINCT ON (signal.canonical_hash) signal.canonical_hash,signal.canonical_text \
         FROM linggan_comment_study_signal signal \
         WHERE signal.canonical_hash IS NOT NULL \
           AND NOT EXISTS( \
             SELECT 1 FROM linggan_comment_study_embedding_cache cache \
             WHERE cache.profile_ref=$1 AND cache.canonical_hash=signal.canonical_hash) \
         ORDER BY signal.canonical_hash,signal.created_at \
         LIMIT $2",
    )
    .bind(profile_ref)
    .bind(i64::try_from(MAX_TEXTS_PER_TICK).unwrap_or(16))
    .fetch_all(database.pool())
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| (row.get("canonical_hash"), row.get("canonical_text")))
        .collect())
}

async fn pending_count(database: &Database, profile_ref: Uuid) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT count(DISTINCT signal.canonical_hash) FROM linggan_comment_study_signal signal \
         WHERE signal.canonical_hash IS NOT NULL \
           AND NOT EXISTS( \
             SELECT 1 FROM linggan_comment_study_embedding_cache cache \
             WHERE cache.profile_ref=$1 AND cache.canonical_hash=signal.canonical_hash)",
    )
    .bind(profile_ref)
    .fetch_one(database.pool())
    .await
}

/// The runtime already normalizes twice, so this is not a second opinion about the maths: it is a
/// refusal to persist anything that would make cosine distance mean something else. A stored
/// non-unit or non-finite vector would rank silently rather than fail.
fn single_unit_vector(response: &crate::pi_adapter::WeMMResponse) -> Result<Vec<f64>, EmbeddingError> {
    if !response.ok {
        return Err(EmbeddingError::InvalidVector);
    }
    let values = response
        .values
        .as_ref()
        .filter(|values| values.len() == 1)
        .ok_or(EmbeddingError::InvalidVector)?;
    let vector = &values[0];
    if vector.len() != usize::try_from(WEMM_DIMENSION).unwrap_or(512)
        || !vector.iter().all(|value| value.is_finite())
    {
        return Err(EmbeddingError::InvalidVector);
    }
    let norm = vector.iter().map(|value| value * value).sum::<f64>().sqrt();
    if !(0.999..=1.001).contains(&norm) {
        return Err(EmbeddingError::InvalidVector);
    }
    Ok(vector.clone())
}

fn pgvector_literal(vector: &[f64]) -> String {
    let mut literal = String::with_capacity(vector.len() * 12 + 2);
    literal.push('[');
    for (index, value) in vector.iter().enumerate() {
        if index > 0 {
            literal.push(',');
        }
        literal.push_str(&format!("{value}"));
    }
    literal.push(']');
    literal
}

fn sha256_json(value: &Value) -> String {
    Sha256::digest(
        serde_json::to_string(value)
            .expect("JSON values serialize")
            .as_bytes(),
    )
    .iter()
    .map(|byte| format!("{byte:02x}"))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pi_adapter::WeMMResponse;

    fn response(values: Option<Vec<Vec<f64>>>, ok: bool) -> WeMMResponse {
        WeMMResponse {
            version: "linggan.wemm.v1".into(),
            id: None,
            ok,
            values,
            failure_code: None,
            elapsed_ms: None,
            backend: Some("mps".into()),
            dimension: Some(512),
            response_type: None,
            model_id: Some(WEMM_MODEL_ID.into()),
            model_revision: None,
            encoding_mode: Some("document".into()),
        }
    }

    fn unit_vector() -> Vec<f64> {
        let mut vector = vec![0.0; 512];
        vector[0] = 1.0;
        vector
    }

    #[test]
    fn a_unit_vector_of_the_declared_dimension_is_accepted() {
        assert_eq!(
            single_unit_vector(&response(Some(vec![unit_vector()]), true)).unwrap(),
            unit_vector()
        );
    }

    #[test]
    fn a_vector_that_is_not_normalised_is_refused_rather_than_stored() {
        // A stored non-unit vector does not fail: it ranks, quietly and wrongly, because cosine
        // distance over the cache assumes every row is already a unit vector.
        let mut scaled = unit_vector();
        scaled[0] = 2.0;
        assert!(single_unit_vector(&response(Some(vec![scaled]), true)).is_err());
    }

    #[test]
    fn a_short_or_non_finite_vector_is_refused() {
        assert!(single_unit_vector(&response(Some(vec![vec![1.0; 256]]), true)).is_err());
        let mut broken = unit_vector();
        broken[5] = f64::NAN;
        assert!(single_unit_vector(&response(Some(vec![broken]), true)).is_err());
    }

    #[test]
    fn a_failed_or_batched_response_never_yields_a_vector() {
        assert!(single_unit_vector(&response(Some(vec![unit_vector()]), false)).is_err());
        assert!(
            single_unit_vector(&response(Some(vec![unit_vector(), unit_vector()]), true)).is_err(),
            "one request encodes one text; two vectors means the pairing with a hash is unknown"
        );
        assert!(single_unit_vector(&response(None, true)).is_err());
    }

    #[test]
    fn the_literal_keeps_every_dimension() {
        let literal = pgvector_literal(&unit_vector());
        assert!(literal.starts_with("[1,0,"), "{}", &literal[..16]);
        assert_eq!(literal.matches(',').count(), 511);
    }
}
