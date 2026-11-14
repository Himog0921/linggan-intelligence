use super::*;
use crate::content_understanding::Generation;
use std::collections::BTreeSet;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryReceipt {
    pub request_ref: Uuid,
    pub original_invocation_ref: Uuid,
    pub work_ref: Uuid,
    pub domain_ref: Uuid,
    pub already_saved: bool,
}

pub async fn recover_saved_replies(
    db: &Database,
    requests: &[Uuid],
    validator_revision: &str,
    apply: bool,
) -> Result<Vec<RecoveryReceipt>, FoundationError> {
    if !(1..=100).contains(&requests.len())
        || requests.iter().copied().collect::<BTreeSet<_>>().len() != requests.len()
        || validator_revision.len() != 40
        || !validator_revision
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(FoundationError::Invalid);
    }
    let mut ordered = requests.to_vec();
    ordered.sort_unstable();
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET LOCAL lock_timeout = '10s'")
        .execute(&mut *tx)
        .await?;
    let mut receipts = Vec::with_capacity(requests.len());
    for request in ordered {
        receipts.push(recover_in(&mut tx, request, validator_revision).await?);
    }
    if apply {
        tx.commit().await?;
    } else {
        tx.rollback().await?;
    }
    Ok(receipts)
}

async fn recover_in(
    tx: &mut Transaction<'_, Postgres>,
    request: Uuid,
    validator_revision: &str,
) -> Result<RecoveryReceipt, FoundationError> {
    let row = sqlx::query("SELECT r.work_ref,r.domain_ref,r.material_snapshot_identity,o.invocation_ref,o.raw_output,
        COALESCE(t.skill_version_ref::text,'none') AS skill,
        i.result->>'promptVersion' AS prompt,t.model_config_ref::text AS model,
        i.result#>>'{diagnostic,providerReturnedModel}' AS provider,
        to_char(o.created_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS generation_at
        FROM linggan_content_understanding_request r
        JOIN linggan_content_understanding_result o USING(request_ref)
        JOIN linggan_ai_batch_item_control b ON b.item_ref=r.item_ref AND b.invocation_ref=o.invocation_ref
        JOIN linggan_model_invocation i ON i.invocation_ref=o.invocation_ref AND i.batch_ref=r.batch_ref
        JOIN linggan_ai_task_config t ON t.task_config_ref=r.task_config_ref
        WHERE r.request_ref=$1 AND r.method_version=$2 AND o.failure_code='ai_output_invalid'
          AND o.result IS NULL AND o.raw_output IS NOT NULL AND b.state='failed'
          AND i.state='failed' AND i.failure_code='ai_output_invalid' AND i.dispatch_state='started'
          AND i.result->'ok'='true'::jsonb AND i.result#>'{diagnostic,terminalReceived}'='true'::jsonb
          AND i.result#>>'{diagnostic,finishReason}'='stop'
          AND i.result#>>'{diagnostic,limitKind}' IS NULL")
        .bind(request).bind(METHOD).fetch_optional(&mut **tx).await?.ok_or(FoundationError::NotFound)?;
    let work: Uuid = row.get("work_ref");
    let domain = row.get("domain_ref");
    let identity: String = row.get("material_snapshot_identity");
    let invocation = row.get("invocation_ref");
    lock_work_snapshot(tx, work, &identity).await?;
    let packet = frozen_in(tx, request).await?;
    let existing: Option<(Uuid, bool)> = sqlx::query_as("SELECT request_ref,offline_recovery
        FROM linggan_content_understanding_saved_core WHERE work_ref=$1 AND material_snapshot_identity=$2 AND method_version=$3")
        .bind(work).bind(&identity).bind(METHOD).fetch_optional(&mut **tx).await?;
    if let Some((current, recovered)) = existing {
        if current == request && recovered {
            return Ok(RecoveryReceipt {
                request_ref: request,
                original_invocation_ref: invocation,
                work_ref: work,
                domain_ref: domain,
                already_saved: true,
            });
        }
        return Err(FoundationError::Conflict);
    }
    let busy: bool = sqlx::query_scalar("SELECT EXISTS(
        SELECT 1 FROM linggan_content_understanding_request r JOIN linggan_ai_batch_item_control b USING(item_ref)
        WHERE r.work_ref=$1 AND r.material_snapshot_identity=$2 AND r.method_version=$3 AND b.state IN ('pending','running'))")
        .bind(work).bind(&identity).bind(METHOD).fetch_one(&mut **tx).await?;
    if busy {
        return Err(FoundationError::Conflict);
    }
    let generation = Generation {
        analysis_ref: request,
        skill_version_ref: row.get("skill"),
        prompt_version_ref: row.try_get("prompt")?,
        model_config_ref: row.try_get("model")?,
        provider_returned_model: row.try_get("provider")?,
        created_at: row.get("generation_at"),
    };
    let raw: String = row.get("raw_output");
    let draft = serde_json::from_str(&raw).map_err(|_| FoundationError::Invalid)?;
    let result = contract::accept(draft, &packet.snapshot, &generation)
        .map_err(|code| FoundationError::AdmissionDenied(vec![code.to_owned()]))?;
    sqlx::query(
        "INSERT INTO linggan_content_understanding_offline_recovery
        (request_ref,work_ref,material_snapshot_identity,method_version,result,validator_revision)
        VALUES($1,$2,$3,$4,$5,$6)",
    )
    .bind(request)
    .bind(work)
    .bind(&identity)
    .bind(METHOD)
    .bind(result)
    .bind(validator_revision)
    .execute(&mut **tx)
    .await?;
    Ok(RecoveryReceipt {
        request_ref: request,
        original_invocation_ref: invocation,
        work_ref: work,
        domain_ref: domain,
        already_saved: false,
    })
}
