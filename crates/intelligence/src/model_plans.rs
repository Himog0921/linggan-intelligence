//! Explicit material grants. Automatic activation and historical selection have separate quotas.
use crate::{comment_analysis::COMMENT_RULE_VERSION, model_settings::*};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartModelPlan {
    pub plan_ref: Uuid,
    pub config_ref: Uuid,
    pub kind: String,
    pub source_refs: Vec<Uuid>,
    pub source_limit: i32,
    pub token_limit: i64,
    pub expected_auto_plan_ref: Option<Uuid>,
}
pub async fn start_model_plan(db: &Database, r: &StartModelPlan) -> Result<Value, ModelError> {
    if crate::comment_intelligence::schema_ready(db).await? {
        return Err(ModelError::ResearchPlanRetired);
    }
    if !matches!(r.kind.as_str(), "trial" | "automatic" | "backfill")
        || !(1..=1000).contains(&r.source_limit)
        || !(1024..=10_000_000).contains(&r.token_limit)
        || r.source_refs.len() > 100
        || (r.kind == "trial" && (r.source_refs.len() != 1 || r.source_limit != 1))
        || (r.kind == "backfill"
            && (r.source_refs.is_empty() || r.source_refs.len() > r.source_limit as usize))
        || (r.kind == "automatic" && !r.source_refs.is_empty())
    {
        return Err(ModelError::Invalid);
    }
    let request = serde_json::to_value(r).map_err(|_| ModelError::Invalid)?;
    let mut tx = db.pool().begin().await?;
    let workspace=sqlx::query("SELECT default_config_ref,active_auto_plan_ref FROM linggan_model_workspace WHERE singleton FOR UPDATE").fetch_one(&mut *tx).await?;
    if r.kind == "automatic" && crate::comment_daily::schema_ready(db).await? {
        let enabled: bool = sqlx::query_scalar(
            "SELECT enabled FROM linggan_comment_daily_schedule WHERE singleton",
        )
        .fetch_one(&mut *tx)
        .await?;
        if enabled {
            return Err(ModelError::Disabled);
        }
    }
    if let Some(row) =
        sqlx::query("SELECT request,enabled FROM linggan_model_plan WHERE plan_ref=$1")
            .bind(r.plan_ref)
            .fetch_optional(&mut *tx)
            .await?
    {
        if row.get::<Value, _>("request") != request {
            return Err(ModelError::Conflict);
        }
        return Ok(
            json!({"planRef":r.plan_ref,"enabled":row.get::<bool,_>("enabled"),"replayed":true,"existingPlans":existing_plan_owners(&mut tx,r).await?}),
        );
    }
    if workspace.get::<Option<Uuid>, _>("default_config_ref") != Some(r.config_ref) {
        return Err(ModelError::Conflict);
    }
    if r.kind == "automatic"
        && workspace.get::<Option<Uuid>, _>("active_auto_plan_ref") != r.expected_auto_plan_ref
    {
        return Err(ModelError::Conflict);
    }
    let eligible:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_model_config config JOIN linggan_model_entry model USING(model_ref) JOIN linggan_model_connection_version v ON v.version_ref=model.connection_version_ref JOIN linggan_model_connection c USING(connection_ref) WHERE config.config_ref=$1 AND c.enabled)")
        .bind(r.config_ref).fetch_one(&mut *tx).await?;
    if !eligible {
        return Err(ModelError::Disabled);
    }
    let minimum: i32 = sqlx::query_scalar(
        "SELECT input_token_limit+output_token_limit FROM linggan_model_config WHERE config_ref=$1",
    )
    .bind(r.config_ref)
    .fetch_one(&mut *tx)
    .await?;
    if r.token_limit < i64::from(minimum) {
        return Err(ModelError::Budget);
    }
    let distinct: std::collections::BTreeSet<_> = r.source_refs.iter().collect();
    if distinct.len() != r.source_refs.len() {
        return Err(ModelError::Invalid);
    }
    let readable:i64=sqlx::query_scalar("SELECT count(*) FROM linggan_comment_research_readable r JOIN linggan_material_comment_current c USING(material_ref) WHERE r.material_ref=ANY($1) AND r.body_state='KNOWN'")
        .bind(&r.source_refs).fetch_one(&mut *tx).await?;
    if readable != r.source_refs.len() as i64 {
        return Err(ModelError::Source);
    }
    sqlx::query("INSERT INTO linggan_model_plan(plan_ref,config_ref,kind,enabled,source_limit,token_limit,request) VALUES($1,$2,$3,true,$4,$5,$6)")
        .bind(r.plan_ref).bind(r.config_ref).bind(&r.kind).bind(r.source_limit).bind(r.token_limit).bind(request).execute(&mut *tx).await?;
    if r.kind == "automatic" {
        sqlx::query(
            "UPDATE linggan_model_plan SET enabled=false,revision=revision+1 WHERE kind='automatic' AND enabled AND plan_ref<>$1",
        ).bind(r.plan_ref)
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE linggan_model_plan SET enabled=true WHERE plan_ref=$1")
            .bind(r.plan_ref)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE linggan_model_workspace SET active_auto_plan_ref=$1 WHERE singleton")
            .bind(r.plan_ref)
            .execute(&mut *tx)
            .await?;
    }
    let mut added = 0;
    for source in &r.source_refs {
        added += insert_model_work(&mut tx, r.plan_ref, r.config_ref, *source).await?;
    }
    let existing = existing_plan_owners(&mut tx, r).await?;
    tx.commit().await?;
    Ok(
        json!({"planRef":r.plan_ref,"queued":added,"state":"GRANTED","kind":r.kind,"existingPlans":existing}),
    )
}
async fn insert_model_work(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    plan: Uuid,
    config: Uuid,
    source: Uuid,
) -> Result<u64, ModelError> {
    let alias = model_version(config);
    let reference:Option<Uuid>=sqlx::query_scalar("INSERT INTO linggan_comment_analysis_work(work_ref,source_ref,rule_version,model_version,state) VALUES(gen_random_uuid(),$1,$2,$3,'pending') ON CONFLICT DO NOTHING RETURNING work_ref")
        .bind(source).bind(COMMENT_RULE_VERSION).bind(alias).fetch_optional(&mut **tx).await?;
    let Some(reference) = reference else {
        return Ok(0);
    };
    sqlx::query(
        "INSERT INTO linggan_comment_model_work(work_ref,plan_ref,config_ref) VALUES($1,$2,$3)",
    )
    .bind(reference)
    .bind(plan)
    .bind(config)
    .execute(&mut **tx)
    .await?;
    Ok(1)
}
pub fn model_version(config: Uuid) -> String {
    format!("model-config.{config}")
}
pub async fn stop_model_plan(db: &Database, plan: Uuid) -> Result<Value, ModelError> {
    let result = sqlx::query("UPDATE linggan_model_plan SET enabled=false,revision=revision+CASE WHEN enabled THEN 1 ELSE 0 END WHERE plan_ref=$1")
        .bind(plan)
        .execute(db.pool())
        .await?;
    if result.rows_affected() != 1 {
        return Err(ModelError::NotFound);
    }
    Ok(json!({"planRef":plan,"enabled":false}))
}
pub async fn sync_automatic_model_work(db: &Database) -> Result<u64, ModelError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SELECT singleton FROM linggan_model_workspace WHERE singleton FOR UPDATE")
        .fetch_one(&mut *tx)
        .await?;
    let row=sqlx::query("SELECT p.plan_ref,w.default_config_ref AS config_ref,p.source_limit FROM linggan_model_workspace w JOIN linggan_model_plan p ON p.plan_ref=w.active_auto_plan_ref WHERE w.singleton AND p.enabled FOR UPDATE OF p")
        .fetch_optional(&mut *tx).await?;
    let Some(row) = row else {
        return Ok(0);
    };
    let plan: Uuid = row.get("plan_ref");
    let config: Uuid = row.get("config_ref");
    let used: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_comment_model_work WHERE plan_ref=$1")
            .bind(plan)
            .fetch_one(&mut *tx)
            .await?;
    let remaining = i64::from(row.get::<i32, _>("source_limit")) - used;
    if remaining <= 0 {
        return Ok(0);
    }
    let sources:Vec<Uuid>=sqlx::query_scalar("SELECT source.material_ref FROM linggan_material_comment_current source JOIN linggan_comment_research_readable readable USING(material_ref) JOIN linggan_model_plan p ON p.plan_ref=$1 WHERE source.created_at>=p.created_at AND source.body_state='KNOWN' AND NOT EXISTS(SELECT 1 FROM linggan_comment_model_work binding JOIN linggan_comment_analysis_work work USING(work_ref) WHERE binding.plan_ref=$1 AND work.source_ref=source.material_ref) AND NOT EXISTS(SELECT 1 FROM linggan_comment_analysis_work work WHERE work.source_ref=source.material_ref AND work.rule_version=$3 AND work.model_version=$4) ORDER BY source.created_at,source.material_ref LIMIT $2")
        .bind(plan).bind(remaining.min(100)).bind(COMMENT_RULE_VERSION).bind(model_version(config)).fetch_all(&mut *tx).await?;
    let mut added = 0;
    for source in sources {
        added += insert_model_work(&mut tx, plan, config, source).await?;
    }
    tx.commit().await?;
    Ok(added)
}

async fn existing_plan_owners(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    request: &StartModelPlan,
) -> Result<Vec<Value>, ModelError> {
    let rows=sqlx::query("SELECT DISTINCT p.plan_ref,p.kind,p.enabled FROM linggan_comment_analysis_work w JOIN linggan_comment_model_work b USING(work_ref) JOIN linggan_model_plan p ON p.plan_ref=b.plan_ref WHERE w.source_ref=ANY($1) AND w.rule_version=$2 AND w.model_version=$3 AND p.plan_ref<>$4 ORDER BY p.plan_ref")
        .bind(&request.source_refs).bind(COMMENT_RULE_VERSION).bind(model_version(request.config_ref)).bind(request.plan_ref).fetch_all(&mut **tx).await?;
    Ok(rows.iter().map(|r|json!({"planRef":r.get::<Uuid,_>("plan_ref"),"kind":r.get::<String,_>("kind"),"enabled":r.get::<bool,_>("enabled")})).collect())
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResumeModelPlan {
    pub expected_revision: i32,
}
/// An explicit current-revision command restores permission; it never changes work identity,
/// config, attempts, activation time or consumed quota. Old command replay cannot unpause again.
pub async fn resume_model_plan(
    db: &Database,
    plan: Uuid,
    request: &ResumeModelPlan,
) -> Result<Value, ModelError> {
    if crate::comment_intelligence::schema_ready(db).await? {
        return Err(ModelError::ResearchPlanRetired);
    }
    if request.expected_revision < 0 {
        return Err(ModelError::Invalid);
    }
    let mut tx = db.pool().begin().await?;
    sqlx::query("SELECT singleton FROM linggan_model_workspace WHERE singleton FOR UPDATE")
        .fetch_one(&mut *tx)
        .await?;
    let row=sqlx::query("UPDATE linggan_model_plan SET enabled=true,revision=revision+1 WHERE plan_ref=$1 AND revision=$2 AND NOT enabled RETURNING revision")
        .bind(plan).bind(request.expected_revision).fetch_optional(&mut *tx).await?.ok_or(ModelError::Conflict)?;
    tx.commit().await?;
    Ok(json!({"planRef":plan,"enabled":true,"revision":row.get::<i32,_>("revision")}))
}
