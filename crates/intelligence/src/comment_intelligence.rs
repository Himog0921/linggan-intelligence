//! One server-owned scope for every comment research view. No request starts an LLM.
use crate::model_settings::ModelError;
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{AssertSqlSafe, Row};
use uuid::Uuid;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResearchScope {
    pub view: Option<String>,
    pub domain: Option<Uuid>,
    pub days: Option<i32>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub time_basis: Option<String>,
    pub text: Option<String>,
    pub work_ref: Option<Uuid>,
    pub lenses: Option<String>,
    pub problem_ref: Option<Uuid>,
    pub term: Option<String>,
    pub bookmarked_only: Option<bool>,
    pub sort: Option<String>,
    pub offset: Option<i64>,
    pub limit: Option<i64>,
    pub result_revision: Option<String>,
    pub batch_ref: Option<Uuid>,
    pub source_refs: Option<String>,
    pub processing_state: Option<String>,
}
impl ResearchScope {
    pub fn validate(&self) -> Result<(), ModelError> {
        if self.days.is_some_and(|v| v != 7 && v != 30)
            || self.limit.is_some_and(|v| v != 20 && v != 50)
            || self.offset.is_some_and(|v| !(0..=100_000).contains(&v))
            || self.text.as_ref().is_some_and(|v| v.chars().count() > 200)
            || self.term.as_ref().is_some_and(|v| v.chars().count() > 48)
            || self
                .time_basis
                .as_deref()
                .is_some_and(|v| !matches!(v, "observed" | "published"))
            || self
                .sort
                .as_deref()
                .is_some_and(|v| !matches!(v, "observed" | "likes"))
            || self.view.as_deref().is_some_and(|v| {
                !matches!(
                    v,
                    "overview" | "voices" | "problems" | "daily" | "changes" | "runs"
                )
            })
            || self.processing_state.as_deref().is_some_and(|v| {
                !matches!(
                    v,
                    "pending"
                        | "analyzed"
                        | "succeeded"
                        | "no_signal"
                        | "failed"
                        | "direct"
                        | "context"
                        | "low_information"
                        | "anomaly"
                )
            })
            || self.lenses.as_ref().is_some_and(|v| {
                v.split(',').any(|s| {
                    !matches!(
                        s,
                        "need" | "solution" | "story" | "quote" | "resonance" | "conflict"
                    )
                })
            })
        {
            return Err(ModelError::Invalid);
        }
        self.refs()?;
        Ok(())
    }
    fn refs(&self) -> Result<Vec<Uuid>, ModelError> {
        let refs = self
            .source_refs
            .as_deref()
            .unwrap_or("")
            .split(',')
            .filter(|v| !v.is_empty())
            .map(|s| Uuid::parse_str(s).map_err(|_| ModelError::Invalid))
            .collect::<Result<Vec<_>, _>>()?;
        if refs.len() > crate::comment_preflight::MAX_RESEARCH_SOURCES {
            return Err(ModelError::SelectionLimit);
        }
        Ok(refs)
    }
}
pub async fn schema_ready(db: &Database) -> Result<bool, ModelError> {
    Ok(sqlx::query_scalar("SELECT to_regclass('linggan_ci_source') IS NOT NULL AND to_regclass('linggan_ci_term_index') IS NOT NULL").fetch_one(db.pool()).await?)
}

pub async fn automation_schema_ready(db: &Database) -> Result<bool, ModelError> {
    Ok(sqlx::query_scalar("SELECT to_regclass('linggan_ci_problem_task') IS NOT NULL AND to_regclass('linggan_ci_definition_vector') IS NOT NULL").fetch_one(db.pool()).await?)
}

#[path = "comment_intelligence_observations.rs"]
mod observations;
#[path = "comment_intelligence_read.rs"]
mod queries;
#[path = "comment_intelligence_query_sql.rs"]
mod query_sql;
pub use queries::{explain_scope, read, source, source_with_scope};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Prepare {
    pub scope: ResearchScope,
    pub source_refs: Option<Vec<Uuid>>,
    #[serde(default)]
    pub reanalyze: bool,
}
pub async fn prepare(db: &Database, r: &Prepare) -> Result<Value, ModelError> {
    let mut scope = r.scope.clone();
    scope.offset = Some(0);
    scope.limit = Some(50);
    if let Some(refs) = &r.source_refs {
        if refs.is_empty() {
            return Err(ModelError::Invalid);
        }
        if refs.len() > crate::comment_preflight::MAX_RESEARCH_SOURCES {
            return Err(ModelError::SelectionLimit);
        }
        scope.source_refs = Some(
            refs.iter()
                .map(Uuid::to_string)
                .collect::<Vec<_>>()
                .join(","),
        );
    }
    let mut result = read(db, &scope).await?;
    let count = result["page"]["total"]
        .as_i64()
        .ok_or(ModelError::Invalid)?;
    // One confirmation has a bounded explicit manifest. Larger history uses successive batches.
    if count == 0 && r.source_refs.is_none() {
        return Err(ModelError::Invalid);
    }
    if count > crate::comment_preflight::MAX_RESEARCH_SOURCES as i64 {
        return Err(ModelError::SelectionLimit);
    }
    let mut refs = read_manifest(db, &mut scope, &result, count).await?;
    if r.source_refs.is_some() {
        refs = r.source_refs.clone().unwrap_or_default();
        refs.sort();
        refs.dedup();
        let allowed: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM linggan_ci_source WHERE domain_ref=$1 AND source_ref=ANY($2)",
        )
        .bind(
            result["scope"]["domain"]
                .as_str()
                .and_then(|s| s.parse::<Uuid>().ok()),
        )
        .bind(&refs)
        .fetch_one(db.pool())
        .await?;
        if allowed != refs.len() as i64 {
            return Err(ModelError::Source);
        }
    }
    if r.source_refs.is_none() && refs.len() != count as usize {
        return Err(ModelError::Conflict);
    }
    let domain = Uuid::parse_str(
        result["scope"]["domain"]
            .as_str()
            .ok_or(ModelError::Invalid)?,
    )
    .map_err(|_| ModelError::Invalid)?;
    let own: bool =
        sqlx::query_scalar("SELECT is_own_domain FROM observation_domain WHERE domain_ref=$1")
            .bind(domain)
            .fetch_one(db.pool())
            .await?;
    // Cross-industry raw material retains its pre-existing no-external-agent contract.
    if !own {
        return Err(ModelError::Source);
    }
    let hashes:Value=sqlx::query_scalar("SELECT jsonb_object_agg(source_ref,source_sha256) FROM linggan_ci_source WHERE domain_ref=$1 AND source_ref=ANY($2)").bind(domain).bind(&refs).fetch_one(db.pool()).await?;
    let id = Uuid::new_v4();
    let policy = crate::comment_runtime::settings(db).await?["policy"].clone();
    let parsed_policy = crate::comment_runtime::ContextPolicy::parse(policy.clone())?;
    let preflight =
        crate::comment_preflight::inspect(db, &refs, &parsed_policy, r.reanalyze).await?;
    let config: Option<Uuid> = preflight["configRef"].as_str().and_then(|s| s.parse().ok());
    let expiry:String=sqlx::query_scalar("INSERT INTO linggan_ci_prepare(prepare_ref,domain_ref,scope,source_refs,source_hashes,reanalyze,context_policy,preflight,config_ref) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9) RETURNING expires_at::text").bind(id).bind(domain).bind(&result["scope"]).bind(&refs).bind(hashes).bind(r.reanalyze).bind(&policy).bind(&preflight).bind(config).fetch_one(db.pool()).await?;
    result["preflight"] = preflight;
    result["limits"] = json!({"maxResearchSources":crate::comment_preflight::MAX_RESEARCH_SOURCES});
    result["contextPolicy"] = policy;
    result["prepareRef"] = json!(id);
    result["sourceRefs"] = json!(refs);
    result["count"] = json!(refs.len());
    result["works"] = result["summary"]["works"].clone();
    result["states"] = result["summary"].clone();
    result["expiresAt"] = json!(expiry);
    Ok(result)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Run {
    pub prepare_ref: Uuid,
    pub config_ref: Uuid,
    pub token_limit: i64,
    #[serde(default)]
    pub reanalyze: bool,
}
pub async fn run(db: &Database, r: &Run) -> Result<Value, ModelError> {
    let row=sqlx::query("SELECT source_refs,source_hashes,domain_ref,reanalyze,preflight,context_policy,config_ref FROM linggan_ci_prepare WHERE prepare_ref=$1 AND expires_at>scope_001_now()").bind(r.prepare_ref).fetch_optional(db.pool()).await?.ok_or(ModelError::NotFound)?;
    if row.get::<bool, _>("reanalyze") != r.reanalyze {
        return Err(ModelError::Conflict);
    }
    if row.get::<Option<Uuid>, _>("config_ref") != Some(r.config_ref) {
        return Err(ModelError::Conflict);
    }
    let refs: Vec<Uuid> = row.get("source_refs");
    let policy = crate::comment_runtime::ContextPolicy::parse(row.get("context_policy"))?;
    let fresh = crate::comment_preflight::inspect(db, &refs, &policy, r.reanalyze).await?;
    if fresh["fingerprints"] != row.get::<Value, _>("preflight")["fingerprints"]
        || fresh["configRef"] != json!(r.config_ref)
    {
        return Err(ModelError::Conflict);
    }
    let hashes:Value=sqlx::query_scalar("SELECT COALESCE(jsonb_object_agg(source_ref,source_sha256),'{}') FROM linggan_ci_source WHERE domain_ref=$1 AND source_ref=ANY($2)").bind(row.get::<Uuid,_>("domain_ref")).bind(&refs).fetch_one(db.pool()).await?;
    if hashes != row.get::<Value, _>("source_hashes") {
        return Err(ModelError::Conflict);
    }
    crate::comment_daily::create_selected(
        db,
        &crate::comment_daily::SelectedBatch {
            batch_ref: r.prepare_ref,
            config_ref: r.config_ref,
            source_refs: refs,
            token_limit: r.token_limit,
            reanalyze: r.reanalyze,
        },
    )
    .await
}

async fn read_manifest(
    db: &Database,
    scope: &mut ResearchScope,
    result: &Value,
    count: i64,
) -> Result<Vec<Uuid>, ModelError> {
    let mut items = result["page"]["items"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    for offset in (50..count).step_by(50) {
        scope.offset = Some(offset);
        let page = read(db, scope).await?;
        if page["scope"]["resultRevision"] != result["scope"]["resultRevision"] {
            return Err(ModelError::Conflict);
        }
        items.extend(
            page["page"]["items"]
                .as_array()
                .cloned()
                .unwrap_or_default(),
        );
    }
    let refs: Vec<Uuid> = items
        .iter()
        .filter_map(|v| {
            v["sourceRef"]
                .as_str()
                .and_then(|s| Uuid::parse_str(s).ok())
        })
        .collect();
    Ok(refs)
}
