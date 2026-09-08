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
            || self
                .view
                .as_deref()
                .is_some_and(|v| !matches!(v, "overview" | "voices" | "problems" | "daily"))
            || self.processing_state.as_deref().is_some_and(|v| {
                !matches!(
                    v,
                    "pending"
                        | "analyzed"
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
        if refs.len() > 1000 {
            return Err(ModelError::Invalid);
        }
        Ok(refs)
    }
}
pub async fn schema_ready(db: &Database) -> Result<bool, ModelError> {
    Ok(sqlx::query_scalar("SELECT to_regclass('linggan_ci_source') IS NOT NULL AND to_regclass('linggan_ci_term_index') IS NOT NULL").fetch_one(db.pool()).await?)
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
        if refs.is_empty() || refs.len() > 100 {
            return Err(ModelError::Invalid);
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
    if count == 0 || count > 100 {
        return Err(ModelError::Invalid);
    }
    let mut items = result["page"]["items"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    if count > 50 {
        scope.offset = Some(50);
        let page = read(db, &scope).await?;
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
    if refs.len() != count as usize {
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
    let expiry:String=sqlx::query_scalar("INSERT INTO linggan_ci_prepare(prepare_ref,domain_ref,scope,source_refs,source_hashes,reanalyze) VALUES($1,$2,$3,$4,$5,$6) RETURNING expires_at::text").bind(id).bind(domain).bind(&result["scope"]).bind(&refs).bind(hashes).bind(r.reanalyze).fetch_one(db.pool()).await?;
    result["prepareRef"] = json!(id);
    result["sourceRefs"] = json!(refs);
    result["count"] = json!(count);
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
    let row=sqlx::query("SELECT source_refs,source_hashes,domain_ref,reanalyze FROM linggan_ci_prepare WHERE prepare_ref=$1 AND expires_at>scope_001_now()").bind(r.prepare_ref).fetch_optional(db.pool()).await?.ok_or(ModelError::NotFound)?;
    if row.get::<bool, _>("reanalyze") != r.reanalyze {
        return Err(ModelError::Conflict);
    }
    let refs: Vec<Uuid> = row.get("source_refs");
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
