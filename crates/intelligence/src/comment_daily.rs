//! Explicit daily material grants. SQL time windows use Asia/Shanghai and server corpus intake time.
use crate::{model_plans::model_version, model_settings::ModelError};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;
pub const DAILY_RULE: &str = "comment-research.v2";
pub async fn schema_ready(db: &Database) -> Result<bool, ModelError> {
    Ok(
        sqlx::query_scalar("SELECT to_regclass('linggan_comment_daily_batch') IS NOT NULL")
            .fetch_one(db.pool())
            .await?,
    )
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DailySchedule {
    pub expected_revision: i32,
    pub enabled: bool,
    pub config_ref: Uuid,
    pub source_limit: i32,
    pub token_limit: i64,
}
async fn validate_config(
    db: &Database,
    config: Uuid,
    sources: i32,
    tokens: i64,
) -> Result<(), ModelError> {
    if !(1..=1000).contains(&sources) || !(1024..=10000000).contains(&tokens) {
        return Err(ModelError::Invalid);
    }
    let allowed:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_model_config cfg JOIN linggan_model_entry m USING(model_ref) JOIN linggan_model_connection_version v ON v.version_ref=m.connection_version_ref JOIN linggan_model_connection c USING(connection_ref) WHERE cfg.config_ref=$1 AND c.enabled AND cfg.input_token_limit+cfg.output_token_limit<=$2)")
        .bind(config).bind(tokens).fetch_one(db.pool()).await?;
    if !allowed {
        return Err(ModelError::NotQualified);
    }
    Ok(())
}
pub async fn save_schedule(db: &Database, r: &DailySchedule) -> Result<Value, ModelError> {
    if r.enabled {
        validate_config(db, r.config_ref, r.source_limit, r.token_limit).await?;
    } else if !(1..=1000).contains(&r.source_limit) || !(1024..=10_000_000).contains(&r.token_limit)
    {
        return Err(ModelError::Invalid);
    }
    let mut tx = db.pool().begin().await?;
    sqlx::query("SELECT singleton FROM linggan_model_workspace WHERE singleton FOR UPDATE")
        .fetch_one(&mut *tx)
        .await?;
    let row=sqlx::query("UPDATE linggan_comment_daily_schedule SET revision=revision+1,enabled=$2,config_ref=$3,source_limit=$4,token_limit=$5,next_start=CASE WHEN $2 THEN COALESCE(next_start,scope_001_now()) ELSE next_start END,next_end=CASE WHEN $2 THEN COALESCE(next_end,(date_trunc('day',scope_001_now() AT TIME ZONE 'Asia/Shanghai')+interval '23 hours'+CASE WHEN (scope_001_now() AT TIME ZONE 'Asia/Shanghai')::time>=time '23:00' THEN interval '1 day' ELSE interval '0' END) AT TIME ZONE 'Asia/Shanghai') ELSE next_end END,updated_at=scope_001_now() WHERE singleton AND revision=$1 RETURNING revision")
        .bind(r.expected_revision).bind(r.enabled).bind(r.config_ref).bind(r.source_limit).bind(r.token_limit).fetch_optional(&mut *tx).await?.ok_or(ModelError::Conflict)?;
    // Daily is the new scheduling authority; do not leave the previous continuous auto loop active.
    sqlx::query("UPDATE linggan_model_plan SET enabled=false,revision=revision+1 WHERE kind='automatic' AND enabled").execute(&mut *tx).await?;
    sqlx::query("UPDATE linggan_model_workspace SET active_auto_plan_ref=NULL WHERE singleton")
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(json!({"revision":row.get::<i32,_>("revision"),"enabled":r.enabled}))
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelectedBatch {
    pub batch_ref: Uuid,
    pub config_ref: Uuid,
    pub source_refs: Vec<Uuid>,
    pub token_limit: i64,
}
pub async fn create_selected(db: &Database, r: &SelectedBatch) -> Result<Value, ModelError> {
    if r.source_refs.is_empty()
        || r.source_refs.len() > 100
        || r.source_refs
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != r.source_refs.len()
    {
        return Err(ModelError::Invalid);
    }
    let request = serde_json::to_value(r).map_err(|_| ModelError::Invalid)?;
    validate_config(db, r.config_ref, r.source_refs.len() as i32, r.token_limit).await?;
    let mut tx = db.pool().begin().await?;
    sqlx::query("SELECT singleton FROM linggan_model_workspace WHERE singleton FOR UPDATE")
        .fetch_one(&mut *tx)
        .await?;
    if let Some(old) = sqlx::query_scalar::<_, Value>(
        "SELECT request FROM linggan_comment_daily_batch WHERE batch_ref=$1",
    )
    .bind(r.batch_ref)
    .fetch_optional(&mut *tx)
    .await?
    {
        if old != request {
            return Err(ModelError::Conflict);
        }
        return Ok(json!({"batchRef":r.batch_ref,"replayed":true}));
    }
    let count:i64=sqlx::query_scalar("SELECT count(*) FROM linggan_comment_research_readable s JOIN linggan_material_comment_current c USING(material_ref) WHERE s.material_ref=ANY($1)").bind(&r.source_refs).fetch_one(&mut *tx).await?;
    if count != r.source_refs.len() as i64 {
        return Err(ModelError::Source);
    }
    sqlx::query("INSERT INTO linggan_comment_daily_batch(batch_ref,kind,config_ref,window_start,window_end,source_limit,token_limit,request) VALUES($1,'selected',$2,scope_001_now(),scope_001_now(),$3,$4,$5)")
        .bind(r.batch_ref).bind(r.config_ref).bind(r.source_refs.len()as i32).bind(r.token_limit).bind(request).execute(&mut *tx).await?;
    sqlx::query(
        "INSERT INTO linggan_comment_daily_item(batch_ref,source_ref) SELECT $1,unnest($2::uuid[])",
    )
    .bind(r.batch_ref)
    .bind(&r.source_refs)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(json!({"batchRef":r.batch_ref,"replayed":false}))
}
pub async fn seal_due(db: &Database) -> Result<bool, ModelError> {
    let mut tx = db.pool().begin().await?;
    let Some(r)=sqlx::query("SELECT *,next_start::text AS start_text,next_end::text AS end_text FROM linggan_comment_daily_schedule WHERE singleton AND enabled AND next_end<=scope_001_now() FOR UPDATE SKIP LOCKED").fetch_optional(&mut *tx).await? else{return Ok(false)};
    let batch = Uuid::new_v4();
    let start: String = r.get("start_text");
    let end: String = r.get("end_text");
    sqlx::query("INSERT INTO linggan_comment_daily_batch(batch_ref,kind,config_ref,window_start,window_end,source_limit,token_limit,request) VALUES($1,'daily',$2,$3::timestamptz,$4::timestamptz,$5,$6,$7)")
        .bind(batch).bind(r.get::<Uuid,_>("config_ref")).bind(&start).bind(&end).bind(r.get::<i32,_>("source_limit")).bind(r.get::<i64,_>("token_limit"))
        .bind(json!({"scheduleRevision":r.get::<i32,_>("revision"),"timezone":"Asia/Shanghai","cleanerVersion":crate::comment_cleaning::CLEANER_VERSION,"ruleVersion":DAILY_RULE})).execute(&mut *tx).await?;
    // Identity first acceptance is independent of current observation/interaction updates.
    sqlx::query("INSERT INTO linggan_comment_daily_item(batch_ref,source_ref) SELECT $1,c.material_ref FROM linggan_comment_research_current($3::timestamptz-interval '1 microsecond') c JOIN linggan_comment_research_readable s USING(material_ref) JOIN LATERAL (SELECT min(created_at) AS first_at FROM linggan_material_comment a WHERE a.content_public_ref=c.content_public_ref AND a.comment_external_id=c.comment_external_id) first_seen ON true WHERE first_seen.first_at>=$2::timestamptz AND first_seen.first_at<$3::timestamptz")
        .bind(batch).bind(&start).bind(&end).execute(&mut *tx).await?;
    sqlx::query("UPDATE linggan_comment_daily_schedule SET next_start=next_end,next_end=next_end+interval '1 day' WHERE singleton").execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(true)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BatchAction {
    pub enabled: bool,
}
pub async fn set_batch_enabled(
    db: &Database,
    batch: Uuid,
    enabled: bool,
) -> Result<Value, ModelError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SELECT singleton FROM linggan_model_workspace WHERE singleton FOR UPDATE")
        .fetch_one(&mut *tx)
        .await?;
    if sqlx::query("UPDATE linggan_comment_daily_batch SET enabled=$2 WHERE batch_ref=$1")
        .bind(batch)
        .bind(enabled)
        .execute(&mut *tx)
        .await?
        .rows_affected()
        != 1
    {
        return Err(ModelError::NotFound);
    }
    tx.commit().await?;
    Ok(json!({"batchRef":batch,"enabled":enabled}))
}
pub fn version(config: Uuid) -> String {
    model_version(config)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RetryDaily {
    pub command_ref: Uuid,
}
pub async fn retry_failed(db: &Database, batch: Uuid, command: Uuid) -> Result<Value, ModelError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SELECT singleton FROM linggan_model_workspace WHERE singleton FOR UPDATE")
        .fetch_one(&mut *tx)
        .await?;
    if let Some(row) = sqlx::query(
        "SELECT batch_ref,result FROM linggan_comment_daily_command WHERE command_ref=$1",
    )
    .bind(command)
    .fetch_optional(&mut *tx)
    .await?
    {
        if row.get::<Uuid, _>("batch_ref") != batch {
            return Err(ModelError::Conflict);
        }
        return Ok(row.get("result"));
    }
    let allowed: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_comment_daily_batch WHERE batch_ref=$1 AND enabled)",
    )
    .bind(batch)
    .fetch_one(&mut *tx)
    .await?;
    if !allowed {
        return Err(ModelError::Disabled);
    }
    let queued=sqlx::query("UPDATE linggan_comment_daily_item i SET state='pending',failure_code=NULL FROM linggan_comment_daily_batch b JOIN linggan_model_config c USING(config_ref) WHERE i.batch_ref=b.batch_ref AND b.batch_ref=$1 AND i.state='failed' AND i.attempts<c.max_attempts AND EXISTS(SELECT 1 FROM linggan_comment_research_readable s WHERE s.material_ref=i.source_ref)").bind(batch).execute(&mut *tx).await?.rows_affected();
    let result = json!({"batchRef":batch,"queued":queued});
    sqlx::query(
        "INSERT INTO linggan_comment_daily_command(command_ref,batch_ref,result) VALUES($1,$2,$3)",
    )
    .bind(command)
    .bind(batch)
    .bind(&result)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(result)
}

pub use crate::comment_cleaning::{CLEANER_VERSION, clean_pending};
pub use crate::comment_daily_read::{batch_detail, cleaning_members, overview, source_states};
pub use crate::comment_daily_runner::run_daily_once;
