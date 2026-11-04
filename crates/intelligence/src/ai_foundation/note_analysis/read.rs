use super::*;
use crate::note_analysis::{self, Output};
pub async fn read(db: &Database, domain: Uuid, request: Uuid) -> Result<Value, FoundationError> {
    let mut tx = db.pool().begin().await?;
    let row=sqlx::query("SELECT r.*,r.created_at::text AS created,c.state,c.failure_code,c.invocation_ref,result.result,cfg.skill_version_ref,cfg.prompt_version_ref,cfg.model_config_ref FROM linggan_note_analysis_request r JOIN linggan_ai_batch_item_control c USING(item_ref) JOIN linggan_ai_task_config cfg USING(task_config_ref) LEFT JOIN linggan_note_analysis_result result USING(request_ref) WHERE r.request_ref=$1 AND r.domain_ref=$2").bind(request).bind(domain).fetch_optional(&mut *tx).await?.ok_or(FoundationError::NotFound)?;
    let p = frozen_in(&mut tx, request).await?;
    let cols = source::columns(&mut tx, row.get("task_config_ref")).await?;
    let now = linggan_evidence::research_dossier::cutoff_in(&mut tx, None).await?;
    let latest = source::load(&mut tx, domain, p.work_ref, p.scope.clone(), &now, &cols).await?;
    let raw: Option<Value> = row.get("result");
    let result = raw
        .map(|v| serde_json::from_value::<Output>(v).map_err(|_| FoundationError::Invalid))
        .transpose()?;
    if let Some(ref r) = result {
        note_analysis::validate(r, &p).map_err(|_| FoundationError::Invalid)?;
    }
    let v = json!({"contract":"note-analysis.read.v1","requestRef":request,"workRef":p.work_ref,"scope":p.scope,"run":{"state":row.get::<String,_>("state"),"batchRef":row.get::<Uuid,_>("batch_ref"),"itemRef":row.get::<Uuid,_>("item_ref"),"invocationRef":row.get::<Option<Uuid>,_>("invocation_ref"),"failureCode":row.get::<Option<String>,_>("failure_code")},"result":result,"coverage":p.coverage,"freshness":if latest.fingerprint()==p.fingerprint(){"current"}else{"source_changed"},"versions":{"methodVersion":METHOD,"taskConfigRef":row.get::<Uuid,_>("task_config_ref"),"skillVersionRef":row.get::<Option<Uuid>,_>("skill_version_ref"),"promptVersionRef":row.get::<Option<Uuid>,_>("prompt_version_ref"),"modelConfigRef":row.get::<Option<Uuid>,_>("model_config_ref"),"createdAt":row.get::<String,_>("created")}});
    tx.commit().await?;
    Ok(v)
}
pub async fn latest(
    db: &Database,
    domain: Uuid,
    work: Uuid,
    scope: Scope,
) -> Result<Value, FoundationError> {
    // Check access even when no analysis has been created.
    let mut tx = db.pool().begin().await?;
    let now = linggan_evidence::research_dossier::cutoff_in(&mut tx, None).await?;
    source::load(
        &mut tx,
        domain,
        work,
        Scope::Note,
        &now,
        &["title".into(), "body".into()],
    )
    .await?;
    tx.commit().await?;
    let newest:Option<Uuid>=sqlx::query_scalar("SELECT request_ref FROM linggan_note_analysis_request WHERE domain_ref=$1 AND work_ref=$2 AND scope=$3 ORDER BY created_at DESC,request_ref DESC LIMIT 1").bind(domain).bind(work).bind(scope.as_str()).fetch_optional(db.pool()).await?;
    let successful:Vec<Uuid>=sqlx::query_scalar("SELECT r.request_ref FROM linggan_note_analysis_request r JOIN linggan_note_analysis_result result USING(request_ref) JOIN linggan_ai_batch_item_control c USING(item_ref) WHERE r.domain_ref=$1 AND r.work_ref=$2 AND r.scope=$3 AND result.result IS NOT NULL AND c.state='succeeded' ORDER BY r.created_at DESC,r.request_ref DESC").bind(domain).bind(work).bind(scope.as_str()).fetch_all(db.pool()).await?;
    let newest = match newest {
        Some(id) => match read(db, domain, id).await {
            Ok(v) => v,
            Err(FoundationError::AdmissionDenied(_) | FoundationError::NotFound) => Value::Null,
            Err(e) => return Err(e),
        },
        None => Value::Null,
    };
    let mut success = Value::Null;
    for id in successful {
        if newest["requestRef"] == json!(id) {
            success = newest.clone();
            break;
        }
        match read(db, domain, id).await {
            Ok(v) => {
                success = v;
                break;
            }
            Err(FoundationError::AdmissionDenied(_) | FoundationError::NotFound) => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(json!({"contract":"note-analysis.latest.v1","latestRun":newest,"latestSuccessful":success}))
}
pub async fn read_context(
    db: &Database,
    domain: Uuid,
    work: Uuid,
    material: &str,
    purpose: &str,
) -> Result<Option<Value>, FoundationError> {
    let requests:Vec<Uuid>=sqlx::query_scalar("SELECT r.request_ref FROM linggan_note_analysis_request r JOIN linggan_note_analysis_result result USING(request_ref) JOIN linggan_ai_batch_item_control c USING(item_ref) WHERE r.domain_ref=$1 AND r.work_ref=$2 AND r.input_manifest->>'materialSnapshotIdentity'=$3 AND result.result IS NOT NULL AND c.state='succeeded' ORDER BY r.created_at DESC,r.request_ref DESC").bind(domain).bind(work).bind(material).fetch_all(db.pool()).await?;
    for id in requests {
        match read(db, domain, id).await {
            Ok(v) => {
                let o: Output = serde_json::from_value(v["result"].clone())
                    .map_err(|_| FoundationError::Invalid)?;
                if let Some(v) = note_analysis::context(&o, id, material, purpose) {
                    return Ok(Some(v));
                }
            }
            Err(FoundationError::AdmissionDenied(_) | FoundationError::NotFound) => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(None)
}
