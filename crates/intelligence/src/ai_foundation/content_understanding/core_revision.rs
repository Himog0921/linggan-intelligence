use super::*;
use crate::content_understanding::{Generation, Snapshot, model_draft};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewedCore {
    pub base_request_ref: Uuid,
    pub source_request_ref: Uuid,
    pub source_snapshot: Snapshot,
    pub source_raw_output: String,
    pub source_failure: Value,
    pub reviewed_draft: String,
    pub validator_revision: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreRevisionReceipt {
    pub revision_ref: Uuid,
    pub base_request_ref: Uuid,
    pub already_saved: bool,
}

pub async fn publish_reviewed_core(
    db: &Database,
    reviewed: &ReviewedCore,
    apply: bool,
) -> Result<CoreRevisionReceipt, FoundationError> {
    let failure = &reviewed.source_failure;
    let diagnostic = &failure["diagnostic"];
    if reviewed.base_request_ref == reviewed.source_request_ref
        || reviewed.source_request_ref.is_nil()
        || reviewed.source_request_ref.to_string() != failure["requestRef"]
        || failure["requestReceipt"]["requestRef"] != failure["requestRef"]
        || failure["state"] != "failed"
        || failure["failureCode"] != "ai_output_invalid"
        || failure["draftProtocol"] != model_draft::PROTOCOL
        || diagnostic["requestSent"] != true
        || diagnostic["terminalReceived"] != true
        || diagnostic["finishReason"] != "stop"
        || !diagnostic["limitKind"].is_null()
        || reviewed.validator_revision.len() != 40
        || !reviewed
            .validator_revision
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        return Err(FoundationError::Invalid);
    }
    let provider = diagnostic["providerReturnedModel"]
        .as_str()
        .ok_or(FoundationError::Invalid)?;
    let rejected: Value =
        serde_json::from_str(&reviewed.source_raw_output).map_err(|_| FoundationError::Invalid)?;
    let draft: Value =
        serde_json::from_str(&reviewed.reviewed_draft).map_err(|_| FoundationError::Invalid)?;

    let mut tx = db.pool().begin().await?;
    sqlx::query("SET LOCAL lock_timeout = '10s'")
        .execute(&mut *tx)
        .await?;
    let source = &reviewed.source_snapshot;
    lock_work_snapshot(&mut tx, source.work_ref, &source.material_snapshot_identity).await?;
    // Deep publication takes the same Core lock; without it a revision can land
    // between the "still current" check and the Deep insert.
    super::deep::lock_core(&mut tx, reviewed.base_request_ref).await?;
    let base = sqlx::query(
        "SELECT core.work_ref, core.material_snapshot_identity, r.domain_ref
        FROM linggan_content_understanding_saved_core core
        JOIN linggan_content_understanding_request r USING(request_ref)
        WHERE core.request_ref=$1 AND core.method_version=$2",
    )
    .bind(reviewed.base_request_ref)
    .bind(METHOD)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(FoundationError::NotFound)?;
    if base.get::<Uuid, _>("work_ref") != source.work_ref
        || base.get::<String, _>("material_snapshot_identity") != source.material_snapshot_identity
    {
        return Err(FoundationError::Conflict);
    }
    let cutoff = linggan_evidence::research_dossier::cutoff_in(&mut tx, None).await?;
    let current = source::load(&mut tx, base.get("domain_ref"), source.work_ref, &cutoff).await?;
    if current.coverage["materialReadiness"]["state"] != "ready" {
        return Err(FoundationError::AdmissionDenied(vec![
            "ai_source_unavailable".into(),
        ]));
    }
    let mut frozen = current.snapshot;
    frozen.material_as_of.clone_from(&source.material_as_of);
    if serde_json::to_value(frozen).map_err(|_| FoundationError::Invalid)?
        != serde_json::to_value(source).map_err(|_| FoundationError::Invalid)?
    {
        return Err(FoundationError::Conflict);
    }
    let revision_ref = Uuid::new_v4();
    let created_at: String = sqlx::query_scalar(
        "SELECT to_char(scope_001_now() AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')",
    )
    .fetch_one(&mut *tx)
    .await?;
    let generation = Generation {
        analysis_ref: revision_ref,
        skill_version_ref: "none".into(),
        prompt_version_ref: "offline-reviewed".into(),
        model_config_ref: "offline-reviewed".into(),
        provider_returned_model: provider.into(),
        created_at,
    };
    let original_error = model_draft::accept_model_draft(rejected, source, &generation)
        .err()
        .ok_or(FoundationError::Conflict)?;
    if failure["validationCode"] != original_error {
        return Err(FoundationError::Conflict);
    }
    let result = model_draft::accept_model_draft(draft, source, &generation)
        .map_err(|code| FoundationError::AdmissionDenied(vec![code.into()]))?;

    let inserted: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO linggan_content_understanding_core_revision
          (revision_ref,base_request_ref,source_request_ref,source_raw_output,source_failure,
           reviewed_draft,validator_revision,result,created_at)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9::text::timestamptz)
         ON CONFLICT (source_request_ref) DO NOTHING RETURNING revision_ref",
    )
    .bind(revision_ref)
    .bind(reviewed.base_request_ref)
    .bind(reviewed.source_request_ref)
    .bind(&reviewed.source_raw_output)
    .bind(failure)
    .bind(&reviewed.reviewed_draft)
    .bind(&reviewed.validator_revision)
    .bind(result)
    .bind(&generation.created_at)
    .fetch_optional(&mut *tx)
    .await?;
    let (revision_ref, already_saved) = if let Some(id) = inserted {
        (id, false)
    } else {
        let id: Uuid = sqlx::query_scalar(
            "SELECT revision_ref
            FROM linggan_content_understanding_core_revision
            WHERE source_request_ref=$1 AND base_request_ref=$2 AND source_raw_output=$3
            AND source_failure=$4 AND reviewed_draft=$5 AND validator_revision=$6",
        )
        .bind(reviewed.source_request_ref)
        .bind(reviewed.base_request_ref)
        .bind(&reviewed.source_raw_output)
        .bind(failure)
        .bind(&reviewed.reviewed_draft)
        .bind(&reviewed.validator_revision)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(FoundationError::Conflict)?;
        (id, true)
    };
    if apply {
        tx.commit().await?;
    } else {
        tx.rollback().await?;
    }
    Ok(CoreRevisionReceipt {
        revision_ref,
        base_request_ref: reviewed.base_request_ref,
        already_saved,
    })
}
