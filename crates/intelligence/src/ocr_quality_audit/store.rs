//! Explicit commands freeze a reproducible audit; no provider is called here.
use super::{
    Settings,
    sampling::{self, Candidate},
    source,
};
use crate::ai_foundation::{
    TaskKey,
    batch_store::{self, BatchInput, BatchKind, InputRef},
    store::{Command, FoundationError},
    transport_store,
};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveSettings {
    pub command_ref: Uuid,
    pub expected_revision: i64,
    pub settings: Settings,
}
pub async fn save_settings(db: &Database, r: &SaveSettings) -> Result<Value, FoundationError> {
    r.settings.validate()?;
    let mut c = Command::begin(db, r.command_ref, "ocr_audit_settings", r).await?;
    if let Some(v) = c.replay()? {
        return Ok(v);
    }
    let changed=sqlx::query("UPDATE linggan_ocr_audit_settings SET settings=$1,revision=revision+1,updated_at=scope_001_now() WHERE singleton AND revision=$2 RETURNING revision").bind(json!(r.settings)).bind(r.expected_revision).fetch_optional(&mut *c.tx).await?.ok_or(FoundationError::Conflict)?;
    c.finish(json!({"revision":changed.get::<i64,_>("revision"),"saved":true,"runStarted":false}))
        .await
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartRun {
    pub command_ref: Uuid,
    pub run_key: String,
    pub seed: String,
    pub visual_probe: bool,
}
pub async fn start_run(db: &Database, r: &StartRun) -> Result<Value, FoundationError> {
    if r.run_key.starts_with("weekly:") {
        return Err(FoundationError::Invalid);
    }
    start_run_kind(
        db,
        r,
        if r.visual_probe {
            "visual_probe"
        } else {
            "manual"
        },
    )
    .await
}
pub(crate) async fn start_run_kind(
    db: &Database,
    r: &StartRun,
    kind: &str,
) -> Result<Value, FoundationError> {
    if r.run_key.is_empty() || r.run_key.len() > 180 || r.seed.is_empty() || r.seed.len() > 120 {
        return Err(FoundationError::Invalid);
    }
    let mut c = Command::begin(db, r.command_ref, "ocr_audit_run", r).await?;
    if let Some(v) = c.replay()? {
        return Ok(v);
    }
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,4540143))")
        .bind(&r.run_key)
        .execute(&mut *c.tx)
        .await?;
    if let Some(id) =
        sqlx::query_scalar::<_, Uuid>("SELECT run_ref FROM linggan_ocr_audit_run WHERE run_key=$1")
            .bind(&r.run_key)
            .fetch_optional(&mut *c.tx)
            .await?
    {
        return c.finish(json!({"runRef":id,"replayed":true})).await;
    }
    let value: Value = sqlx::query_scalar(
        "SELECT settings FROM linggan_ocr_audit_settings WHERE singleton FOR SHARE",
    )
    .fetch_one(&mut *c.tx)
    .await?;
    let s: Settings =
        serde_json::from_value(value.clone()).map_err(|_| FoundationError::Invalid)?;
    s.validate()?;
    if !r.visual_probe && s.domain_ref.is_none() {
        return Err(FoundationError::ConfigurationIncomplete);
    }
    let cfg=sqlx::query("SELECT s.current_config_ref,s.revision,c.model_config_ref FROM linggan_ai_task_setting s JOIN linggan_ai_task_config c ON c.task_config_ref=s.current_config_ref WHERE s.task_key='ocr_quality_audit' FOR SHARE OF s").fetch_one(&mut *c.tx).await?;
    let model = transport_store::load_frozen_in(
        &mut c.tx,
        cfg.get::<Option<Uuid>, _>("model_config_ref")
            .ok_or(FoundationError::ConfigurationIncomplete)?,
    )
    .await?;
    if model.limits.attempts != 1 || model.limits.timeout_seconds > u64::from(s.timeout_seconds) {
        return Err(FoundationError::Invalid);
    }
    let price:Option<Uuid>=sqlx::query_scalar("SELECT price_ref FROM linggan_model_price WHERE identity_ref=$1 AND endpoint_ref=$2 ORDER BY revision_sequence DESC LIMIT 1").bind(model.identity_ref).bind(model.endpoint_ref).fetch_optional(&mut *c.tx).await?;
    if price.is_none() {
        return Err(FoundationError::AdmissionDenied(vec![
            "audit_price_unconfigured".into(),
        ]));
    }
    let run = r.command_ref;
    let mut selections = vec![];
    let mut inventory = json!({"eligible":0,"selected":1,"skippedRestrictedOrRetired":0,"skippedPrivacyUnreviewed":0,"seed":r.seed,"filterVersion":"canonical-reading-v2"});
    if !r.visual_probe {
        let rows=sqlx::query("SELECT reading_ref,content_public_ref,blob_sha256,(jsonb_array_length(issues)>0 OR EXISTS(SELECT 1 FROM jsonb_array_elements(blocks) b WHERE jsonb_array_length(COALESCE(b->'issues','[]'))>0)) AS risk FROM linggan_ocr_audit_eligible_source a WHERE EXISTS(SELECT 1 FROM linggan_material_domain_usage u WHERE u.content_public_ref=a.content_public_ref AND u.domain_ref=$1) ORDER BY reading_ref").bind(s.domain_ref).fetch_all(&mut *c.tx).await?;
        let candidates: Vec<_> = rows
            .into_iter()
            .map(|row| Candidate {
                reading_ref: row.get("reading_ref"),
                work_ref: row.get("content_public_ref"),
                blob_sha256: row.get("blob_sha256"),
                risk: row.get("risk"),
            })
            .collect();
        let admitted_count = candidates.len();
        let candidates: Vec<_> = candidates
            .into_iter()
            .filter(|c| s.privacy_cleared_refs.contains(&c.reading_ref))
            .collect();
        selections = sampling::select(&r.seed, &candidates, &s);
        let total:i64=sqlx::query_scalar("SELECT count(*) FROM linggan_material_unit material WHERE material.kind='image' AND material.current_reading_ref IS NOT NULL AND EXISTS(SELECT 1 FROM linggan_material_domain_usage u WHERE u.content_public_ref=material.content_public_ref AND u.domain_ref=$1)").bind(s.domain_ref).fetch_one(&mut *c.tx).await?;
        inventory = json!({"eligible":candidates.len(),"selected":selections.len(),"skippedRestrictedOrRetired":total-admitted_count as i64,"skippedPrivacyUnreviewed":admitted_count-candidates.len(),"candidates":candidates,"seed":r.seed,"filterVersion":"canonical-reading-v2","randomPopulation":"eligible_except_fixed_regression","requested":{"random":s.random_count,"risk":s.risk_count,"regression":s.regression_count}});
    }
    sqlx::query("INSERT INTO linggan_ocr_audit_run(run_ref,run_key,seed,kind,settings,task_config_ref,endpoint_ref,model_id,connection_version_ref,price_ref,prompt_version,inventory) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)")
 .bind(run).bind(&r.run_key).bind(&r.seed).bind(kind).bind(value).bind(cfg.get::<Uuid,_>("current_config_ref")).bind(model.endpoint_ref).bind(&model.model_id).bind(model.connection_version_ref).bind(price).bind(super::PROMPT_VERSION).bind(&inventory).execute(&mut *c.tx).await?;
    let mut items = vec![];
    if r.visual_probe {
        use base64::Engine;
        let image = source::probe_image();
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(image.data)
            .map_err(|_| FoundationError::Invalid)?;
        let sample = Uuid::new_v4();
        sqlx::query("INSERT INTO linggan_ocr_audit_sample(sample_ref,run_ref,pool,blob_sha256) VALUES($1,$2,'probe',$3)").bind(sample).bind(run).bind(linggan_evidence::ocr_adoption::content_hash(&bytes)).execute(&mut *c.tx).await?;
        items.push(BatchInput {
            item_key: sample.to_string(),
            input: InputRef::OcrAuditSample { sample_ref: sample },
        });
    } else {
        for selected in &selections {
            let source = source::load_in(&mut c.tx, selected.candidate.reading_ref)
                .await?
                .ok_or(FoundationError::Conflict)?;
            let sample = Uuid::new_v4();
            let local_snapshot = source.frozen_snapshot();
            sqlx::query("INSERT INTO linggan_ocr_audit_sample(sample_ref,run_ref,reading_ref,pool,blob_sha256,local_rule_version,local_snapshot) VALUES($1,$2,$3,$4,$5,$6,$7)")
                .bind(sample).bind(run).bind(source.reading_ref).bind(&selected.pool).bind(&source.image.blob_sha256).bind(&source.rule_version).bind(local_snapshot).execute(&mut *c.tx).await?;
            items.push(BatchInput {
                item_key: sample.to_string(),
                input: InputRef::OcrAuditSample { sample_ref: sample },
            });
        }
    }
    // Freeze and enqueue are one transaction; the shared batch command uses the same connection.
    if !items.is_empty() {
        let hash = batch_store::input_hash(TaskKey::OcrQualityAudit, &items, false)?;
        batch_store::insert_batch(
            &mut c.tx,
            run,
            TaskKey::OcrQualityAudit,
            cfg.get("current_config_ref"),
            BatchKind::Interactive,
            &items,
            &hash,
        )
        .await?;
    }
    c.finish(json!({"runRef":run,"selected":items.len(),"inventory":inventory,"queued":!items.is_empty()})).await
}
pub async fn read_settings(db: &Database) -> Result<Value, FoundationError> {
    let row =
        sqlx::query("SELECT revision,settings FROM linggan_ocr_audit_settings WHERE singleton")
            .fetch_one(db.pool())
            .await?;
    Ok(
        json!({"revision":row.get::<i64,_>("revision"),"settings":row.get::<Value,_>("settings"),"schedule":{"weekday":"Monday","time":"09:00","timezone":"Asia/Shanghai"},"automaticCorrection":false}),
    )
}
