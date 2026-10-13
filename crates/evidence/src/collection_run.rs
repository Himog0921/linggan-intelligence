//! One durable user request; execution remains Request -> Admission -> WorkOrder -> Lease.
//! No browser calls, material copies, or inferred completion live here.
use crate::acquisition_chain::{MaterialDeepeningTarget, request_and_admit_in_transaction_scoped};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error("{0}")]
    Invalid(&'static str),
    #[error("collection run not found in this domain")]
    NotFound,
    #[error("request identity or revision conflict")]
    Conflict,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Admission(#[from] crate::AcquisitionChainError),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommentMode {
    #[serde(rename = "sample_30")]
    Sample30,
    AllPublic,
}
impl CommentMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sample30 => "sample_30",
            Self::AllPublic => "all_public",
        }
    }
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum RunSource {
    TemporaryKeyword { query: String },
    ObservationTarget { target_ref: Uuid },
    SelectedWorks { work_refs: Vec<Uuid> },
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateRun {
    pub idempotency_key: Uuid,
    pub domain_ref: Uuid,
    pub source: RunSource,
    pub ranking: Option<String>,
    pub work_limit: i32,
    pub comment_mode: CommentMode,
}
fn hash<T: Serialize>(v: &T) -> Result<String, RunError> {
    let bytes = serde_json::to_vec(v).map_err(|_| RunError::Invalid("invalid_request"))?;
    Ok(Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}
impl CreateRun {
    fn validate(&self) -> Result<(), RunError> {
        if !(1..=200).contains(&self.work_limit) {
            return Err(RunError::Invalid("invalid_work_limit"));
        }
        if self.ranking.as_deref().is_some_and(|r| {
            !matches!(
                r,
                "comprehensive" | "latest" | "most_liked" | "most_collected" | "most_commented"
            )
        }) {
            return Err(RunError::Invalid("unsupported_ranking"));
        }
        match &self.source {
            RunSource::TemporaryKeyword { query }
                if query.trim().is_empty() || query.chars().count() > 80 =>
            {
                Err(RunError::Invalid("invalid_keyword"))
            }
            RunSource::SelectedWorks { work_refs }
                if work_refs.is_empty()
                    || work_refs.len() > 200
                    || work_refs
                        .iter()
                        .collect::<std::collections::HashSet<_>>()
                        .len()
                        != work_refs.len()
                    || work_refs.len() != self.work_limit as usize =>
            {
                Err(RunError::Invalid("invalid_work_set"))
            }
            RunSource::SelectedWorks { .. } if self.ranking.is_some() => {
                Err(RunError::Invalid("ranking_not_applicable"))
            }
            _ => Ok(()),
        }
    }
}

pub async fn create_run(db: &Database, input: &CreateRun) -> Result<Value, RunError> {
    input.validate()?;
    let fingerprint = hash(input)?;
    let mut tx = db.pool().begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,398))")
        .bind(input.idempotency_key.to_string())
        .execute(&mut *tx)
        .await?;
    if let Some(row) =
        sqlx::query("SELECT run_ref,request_hash FROM collection_run WHERE idempotency_key=$1")
            .bind(input.idempotency_key)
            .fetch_optional(&mut *tx)
            .await?
    {
        if row.get::<String, _>("request_hash") != fingerprint {
            return Err(RunError::Conflict);
        }
        let run = row.get::<Uuid, _>("run_ref");
        tx.commit().await?;
        return Ok(json!({"runRef":run,"replayed":true}));
    }
    let active:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM observation_domain WHERE domain_ref=$1 AND status='active' FOR SHARE)").bind(input.domain_ref).fetch_one(&mut *tx).await?;
    if !active {
        return Err(RunError::Invalid("domain_not_active"));
    }
    let (kind, source_target, target_kind, query, name) = match &input.source {
        RunSource::TemporaryKeyword { query } => (
            "temporary_keyword",
            None,
            "keyword".to_owned(),
            Some(query.trim().to_owned()),
            query.trim().to_owned(),
        ),
        RunSource::ObservationTarget { target_ref } => {
            let row=sqlx::query("SELECT t.target_kind,t.identity_key,COALESCE(t.display_name,t.identity_key) name FROM collection_observation_target t JOIN observation_domain_target relation USING(target_ref) WHERE t.target_ref=$1 AND relation.domain_ref=$2 AND t.platform='xhs' AND t.purpose_kind='long_term' AND t.lifecycle_state<>'dismissed' FOR SHARE OF t").bind(target_ref).bind(input.domain_ref).fetch_optional(&mut *tx).await?.ok_or(RunError::Invalid("target_outside_domain_or_stopped"))?;
            let tk: String = row.get("target_kind");
            if tk == "creator" && input.ranking.is_some() {
                return Err(RunError::Invalid("ranking_not_applicable"));
            }
            (
                "observation_target",
                Some(*target_ref),
                tk.clone(),
                Some(if tk == "keyword" {
                    crate::work_order_lease::search_term(
                        &row.get::<String, _>("identity_key"),
                        input.ranking.as_deref(),
                    )
                } else {
                    row.get("identity_key")
                }),
                row.get::<String, _>("name"),
            )
        }
        RunSource::SelectedWorks { work_refs } => {
            let count:i64=sqlx::query_scalar("SELECT count(*) FROM linggan_material_content c WHERE c.public_ref=ANY($1) AND c.platform='xhs' AND EXISTS(SELECT 1 FROM linggan_material_domain_usage u WHERE u.content_public_ref=c.public_ref AND u.domain_ref=$2)").bind(work_refs).bind(input.domain_ref).fetch_one(&mut *tx).await?;
            if count != work_refs.len() as i64 {
                return Err(RunError::Invalid("works_outside_domain"));
            }
            (
                "selected_works",
                None,
                "keyword".to_owned(),
                None,
                format!("已选 {} 篇作品", work_refs.len()),
            )
        }
    };
    let run = Uuid::new_v4();
    let target = Uuid::new_v4();
    let auth = Uuid::new_v4();
    sqlx::query("INSERT INTO collection_run(run_ref,idempotency_key,request_hash,domain_ref,source_kind,source_target_ref,name,search_query,ranking,work_limit,comment_mode) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)").bind(run).bind(input.idempotency_key).bind(fingerprint).bind(input.domain_ref).bind(kind).bind(source_target).bind(&name).bind(&query).bind(&input.ranking).bind(input.work_limit).bind(input.comment_mode.as_str()).execute(&mut *tx).await?;
    // An execution-only identity never mutates or joins the person's long-term target.
    // The true source query/creator id is frozen separately and supplied to TaskSpec.
    sqlx::query("INSERT INTO collection_observation_target(target_ref,platform,target_kind,identity_key,display_name,source,purpose_kind,purpose_run_ref) VALUES($1,'xhs',$2,$3,$4,'manual','single_run',$5)").bind(target).bind(&target_kind).bind(format!("single-run:{run}")).bind(&name).bind(run).execute(&mut *tx).await?;
    sqlx::query(
        "INSERT INTO observation_domain_target(domain_ref,target_ref,role) VALUES($1,$2,'primary')",
    )
    .bind(input.domain_ref)
    .bind(target)
    .execute(&mut *tx)
    .await?;
    let templates = if target_kind == "creator" {
        vec!["creator_archive", "material_deepening"]
    } else {
        vec!["keyword_archive", "material_deepening"]
    };
    sqlx::query("INSERT INTO collection_acquisition_authorization(authorization_ref,platform,target_kind,lane,purpose,max_works_per_target,allowed_task_templates,allowed_dispatch_lanes,max_work_units,granted_by,expires_at) VALUES($1,'xhs',$2,'deep_archive',$3,$4,$5,ARRAY['immediate','batch'],$4,'person',scope_001_now()+interval '7 days')").bind(auth).bind(target_kind).bind(format!("collection_run:{run}")).bind(input.work_limit).bind(templates).execute(&mut *tx).await?;
    sqlx::query(
        "UPDATE collection_run SET execution_target_ref=$2,authorization_ref=$3 WHERE run_ref=$1",
    )
    .bind(run)
    .bind(target)
    .bind(auth)
    .execute(&mut *tx)
    .await?;
    if let RunSource::SelectedWorks { work_refs } = &input.source {
        for (n, work) in work_refs.iter().enumerate() {
            sqlx::query("INSERT INTO collection_run_work(run_ref,content_public_ref,ordinal) VALUES($1,$2,$3)").bind(run).bind(work).bind(n as i32+1).execute(&mut *tx).await?;
        }
        sqlx::query("UPDATE collection_run SET scope_frozen_at=scope_001_now(),phase='materials',discovery_reason='person_selected_exact_set' WHERE run_ref=$1").bind(run).execute(&mut *tx).await?;
    } else {
        admit_stage(&mut tx, run, None, "discovery", 1).await?;
    }
    tx.commit().await?;
    Ok(json!({"runRef":run,"replayed":false}))
}

pub(crate) async fn admit_stage(
    tx: &mut Transaction<'_, Postgres>,
    run: Uuid,
    work: Option<Uuid>,
    stage: &str,
    segment: i32,
) -> Result<(), RunError> {
    let r = sqlx::query("SELECT * FROM collection_run WHERE run_ref=$1 FOR UPDATE")
        .bind(run)
        .fetch_one(&mut **tx)
        .await?;
    if r.get::<String, _>("control_intent") != "running" {
        return Ok(());
    }
    // Reconciliation and retries may observe the same completed segment again.
    // An admitted stage remains authoritative, including while its newer lease is pending.
    let admitted: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM collection_run_order WHERE run_ref=$1 AND content_public_ref IS NOT DISTINCT FROM $2 AND stage=$3 AND segment=$4 AND work_order_ref IS NOT NULL)")
        .bind(run).bind(work).bind(stage).bind(segment).fetch_one(&mut **tx).await?;
    if admitted {
        return Ok(());
    }
    let target: Uuid = r.get("execution_target_ref");
    let domain: Uuid = r.get("domain_ref");
    let auth: Uuid = r.get("authorization_ref");
    let known: Vec<String> = if stage == "comments" {
        sqlx::query_scalar("SELECT DISTINCT COALESCE(record.value->'payload'->>'commentId',record.value->'payload'->>'id') FROM collection_run_package p JOIN linggan_runtime_capture_package raw USING(package_ref) CROSS JOIN LATERAL jsonb_array_elements(raw.payload->'records') record(value) JOIN linggan_material_content c ON c.platform=raw.platform AND c.content_external_id=record.value->'sourceObject'->>'externalId' WHERE p.run_ref=$1 AND c.public_ref=$2 AND raw.package_kind IN('comments','replies') AND COALESCE(record.value->'payload'->>'commentId',record.value->'payload'->>'id','')<>'' ORDER BY 1").bind(run).bind(work).fetch_all(&mut **tx).await?
    } else {
        Vec::new()
    };
    let sample = r.get::<String, _>("comment_mode") == "sample_30";
    let comment_limit = if stage == "detail" {
        0
    } else if sample {
        30 - known.len().min(30) as i32
    } else {
        30
    };
    if stage == "comments" && comment_limit == 0 {
        return Ok(());
    }
    let materials: Vec<MaterialDeepeningTarget> = work
        .into_iter()
        .map(|content_public_ref| MaterialDeepeningTarget {
            content_public_ref,
            comment_limit,
            reply_expand_limit: if stage == "detail" { 0 } else { 2 },
            acquire_media: stage == "detail" || (sample && segment == 1),
            allow_ocr: stage == "detail" || (sample && segment == 1),
            allow_asr: stage == "detail" || (sample && segment == 1),
        })
        .collect();
    let result = request_and_admit_in_transaction_scoped(
        tx,
        target,
        "deep_archive",
        &format!(
            "collection_run:{run}:{}:{stage}:{segment}",
            work.map(|v| v.to_string())
                .unwrap_or_else(|| "discovery".into())
        ),
        "person",
        Some(domain),
        &materials,
        Some(auth),
        None,
        false,
    )
    .await?;
    if let Some(order) = result.work_order_ref {
        let mut directive = json!({"runRef":run,"stage":stage,"commentMode":r.get::<String,_>("comment_mode"),"searchQuery":r.get::<Option<String>,_>("search_query"),"ranking":r.get::<Option<String>,_>("ranking").unwrap_or_else(||"comprehensive".into()),"workLimit":r.get::<i32,_>("work_limit"),"segment":segment});
        if stage == "comments" && (!sample || !known.is_empty()) {
            directive["commentBudget"] = json!({"knownCommentIds":known,"newUniqueLimit":comment_limit,"maxScrollRounds":50,"maxDurationSeconds":180});
            if sample {
                let counts:Vec<(String,i64)>=sqlx::query_as("SELECT COALESCE(record.value->'payload'->>'rootCommentId',record.value->'payload'->>'parentCommentId'),count(DISTINCT COALESCE(record.value->'payload'->>'commentId',record.value->'payload'->>'id')) FROM collection_run_package p JOIN linggan_runtime_capture_package raw USING(package_ref) CROSS JOIN LATERAL jsonb_array_elements(raw.payload->'records') record(value) JOIN linggan_material_content c ON c.platform=raw.platform AND c.content_external_id=record.value->'sourceObject'->>'externalId' WHERE p.run_ref=$1 AND c.public_ref=$2 AND raw.package_kind='replies' AND COALESCE(record.value->'payload'->>'rootCommentId',record.value->'payload'->>'parentCommentId','')<>'' GROUP BY 1").bind(run).bind(work).fetch_all(&mut **tx).await?;
                directive["commentBudget"]["knownReplyCounts"] = json!(
                    counts
                        .into_iter()
                        .map(|(id, n)| (id, n.min(2)))
                        .collect::<std::collections::BTreeMap<_, _>>()
                );
            }
        }
        sqlx::query("UPDATE collection_work_order SET max_works=CASE WHEN $3='discovery' THEN $4 ELSE max_works END,stop_conditions=stop_conditions||jsonb_build_object('collectionRun',$2::jsonb) WHERE work_order_ref=$1 AND queue_state='queued' AND NOT EXISTS(SELECT 1 FROM collection_work_order_lease WHERE work_order_ref=$1)").bind(order).bind(directive).bind(stage).bind(r.get::<i32,_>("work_limit")).execute(&mut **tx).await?;
    }
    sqlx::query("INSERT INTO collection_run_order(run_ref,request_ref,work_order_ref,content_public_ref,stage,segment) VALUES($1,$2,$3,$4,$5,$6)").bind(run).bind(result.request_ref).bind(result.work_order_ref).bind(work).bind(stage).bind(segment).execute(&mut **tx).await?;
    sqlx::query("UPDATE collection_run SET updated_at=scope_001_now() WHERE run_ref=$1")
        .bind(run)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunCommand {
    pub command_ref: Uuid,
    pub domain_ref: Uuid,
    pub expected_revision: i32,
    pub action: String,
}
pub async fn command_run(db: &Database, run: Uuid, input: &RunCommand) -> Result<Value, RunError> {
    if !matches!(input.action.as_str(), "pause" | "continue" | "stop") {
        return Err(RunError::Invalid("invalid_control"));
    }
    let fingerprint = hash(&(run, input))?;
    let mut tx = db.pool().begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,399))")
        .bind(input.command_ref.to_string())
        .execute(&mut *tx)
        .await?;
    if let Some(row) =
        sqlx::query("SELECT command_hash,receipt FROM collection_run_command WHERE command_ref=$1")
            .bind(input.command_ref)
            .fetch_optional(&mut *tx)
            .await?
    {
        if row.get::<String, _>("command_hash") != fingerprint {
            return Err(RunError::Conflict);
        }
        return Ok(row.get("receipt"));
    }
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,400))")
        .bind(run.to_string())
        .execute(&mut *tx)
        .await?;
    let row=sqlx::query("SELECT revision,control_intent,phase FROM collection_run WHERE run_ref=$1 AND domain_ref=$2 FOR UPDATE").bind(run).bind(input.domain_ref).fetch_optional(&mut *tx).await?.ok_or(RunError::NotFound)?;
    if row.get::<String, _>("phase") == "ended" {
        return Err(RunError::Invalid("run_ended"));
    }
    if row.get::<i32, _>("revision") != input.expected_revision {
        return Err(RunError::Conflict);
    }
    if row.get::<String, _>("control_intent") == "stopped" && input.action != "stop" {
        return Err(RunError::Invalid("run_stopped"));
    }
    let intent = match input.action.as_str() {
        "pause" => "paused",
        "stop" => "stopped",
        _ => "running",
    };
    if intent == "running" {
        let active:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM observation_domain WHERE domain_ref=$1 AND status='active' FOR SHARE)").bind(input.domain_ref).fetch_one(&mut *tx).await?;
        if !active {
            return Err(RunError::Invalid("domain_not_active"));
        }
    }
    sqlx::query("UPDATE collection_run SET control_intent=$2,revision=revision+1,updated_at=scope_001_now(),ended_at=CASE WHEN $2='stopped' THEN scope_001_now() ELSE ended_at END WHERE run_ref=$1").bind(run).bind(intent).execute(&mut *tx).await?;
    // Lock order matches dispatch: run first, then order. Obtained packages are never deleted.
    if intent == "stopped" {
        sqlx::query("UPDATE collection_work_order SET queue_state='cancelled' WHERE work_order_ref IN(SELECT work_order_ref FROM collection_run_order WHERE run_ref=$1) AND queue_state='queued'").bind(run).execute(&mut *tx).await?;
    }
    let receipt = json!({"runRef":run,"commandRef":input.command_ref,"controlIntent":intent,"revision":input.expected_revision+1});
    sqlx::query("INSERT INTO collection_run_command(command_ref,run_ref,command_hash,receipt) VALUES($1,$2,$3,$4)").bind(input.command_ref).bind(run).bind(fingerprint).bind(&receipt).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(receipt)
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunQuery {
    pub domain_ref: Option<Uuid>,
    pub filter: Option<String>,
    pub q: Option<String>,
    pub page: Option<i64>,
}
pub async fn list_runs(db: &Database, q: &RunQuery) -> Result<Value, RunError> {
    let filter = q.filter.as_deref().unwrap_or("all");
    if !matches!(filter, "all" | "active" | "attention" | "results" | "ended") {
        return Err(RunError::Invalid("invalid_filter"));
    }
    let page = q.page.unwrap_or(1).clamp(1, 1000000);
    let term = q.q.as_deref().unwrap_or("");
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let rows: Vec<Value> = sqlx::query_scalar(RUN_LIST_SQL)
        .bind(q.domain_ref)
        .bind(term)
        .bind(filter)
        .bind((page - 1) * 20)
        .fetch_all(&mut *tx)
        .await?;
    let count: i64 = sqlx::query_scalar(RUN_COUNT_SQL)
        .bind(q.domain_ref)
        .bind(term)
        .bind(filter)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(json!({"items":rows,"total":count,"page":page,"pageSize":20}))
}
const RUN_LIST_SQL: &str = r#"SELECT body FROM collection_user_summary v WHERE ($1::uuid IS NULL OR domain_ref=$1) AND ($2='' OR strpos(lower(name||' '||run_ref::text),lower($2))>0) AND ($3='all' OR ($3='active' AND NOT ended) OR ($3='attention' AND needs_attention) OR ($3='results' AND has_results) OR ($3='ended' AND ended)) ORDER BY created_at DESC,run_ref LIMIT 20 OFFSET $4"#;
const RUN_COUNT_SQL: &str = r#"SELECT count(*) FROM collection_user_summary WHERE ($1::uuid IS NULL OR domain_ref=$1) AND ($2='' OR strpos(lower(name||' '||run_ref::text),lower($2))>0) AND ($3='all' OR ($3='active' AND NOT ended) OR ($3='attention' AND needs_attention) OR ($3='results' AND has_results) OR ($3='ended' AND ended))"#;

pub async fn read_run(db: &Database, run: Uuid, domain: Uuid) -> Result<Value, RunError> {
    let row: Option<Value> = sqlx::query_scalar(
        "SELECT body FROM collection_user_summary v WHERE run_ref=$1 AND domain_ref=$2",
    )
    .bind(run)
    .bind(domain)
    .fetch_optional(db.pool())
    .await?;
    row.ok_or(RunError::NotFound)
}
pub async fn read_run_works(
    db: &Database,
    run: Uuid,
    domain: Uuid,
    page: i64,
) -> Result<Value, RunError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let found: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM collection_user_summary WHERE run_ref=$1 AND domain_ref=$2)",
    )
    .bind(run)
    .bind(domain)
    .fetch_one(&mut *tx)
    .await?;
    if !found {
        return Err(RunError::NotFound);
    }
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_user_member WHERE run_ref=$1 AND domain_ref=$2",
    )
    .bind(run)
    .bind(domain)
    .fetch_one(&mut *tx)
    .await?;
    let page = page.clamp(1, 1000000);
    let items:Vec<Value>=sqlx::query_scalar(r#"SELECT jsonb_build_object('publicRef',m.content_public_ref,'ordinal',w.ordinal,'title',COALESCE(detail.title,finding.title),'creator',detail.creator_display_name,'reusedDetailPackageRef',w.reused_detail_package_ref,'checkpoint',w.checkpoint,'gapReason',w.gap_reason)
     FROM collection_user_member m LEFT JOIN collection_run_work w ON w.run_ref=m.run_ref AND w.content_public_ref=m.content_public_ref
     LEFT JOIN LATERAL(SELECT d.title,d.creator_display_name FROM linggan_material_content_detail d JOIN linggan_runtime_capture_package p USING(package_ref) WHERE d.content_public_ref=m.content_public_ref AND (d.package_ref=w.reused_detail_package_ref OR EXISTS(SELECT 1 FROM collection_user_package u WHERE u.run_ref=m.run_ref AND u.domain_ref=m.domain_ref AND u.package_ref=d.package_ref)) ORDER BY p.accepted_at DESC,d.material_ref LIMIT 1) detail ON true
     LEFT JOIN LATERAL(SELECT f.title FROM linggan_material_discovery_finding f JOIN collection_user_package p USING(package_ref) WHERE f.content_public_ref=m.content_public_ref AND p.run_ref=m.run_ref AND p.domain_ref=m.domain_ref ORDER BY p.accepted_at DESC,f.result_position LIMIT 1) finding ON true
     WHERE m.run_ref=$1 AND m.domain_ref=$2 ORDER BY w.ordinal NULLS LAST,m.content_public_ref LIMIT 20 OFFSET $3"#).bind(run).bind(domain).bind((page-1)*20).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    Ok(
        json!({"runRef":run,"domainRef":domain,"items":items,"total":total,"page":page,"pageSize":20}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_selection_and_modes_are_not_numeric_sentinels() {
        let mut input = CreateRun {
            idempotency_key: Uuid::new_v4(),
            domain_ref: Uuid::new_v4(),
            source: RunSource::SelectedWorks {
                work_refs: vec![Uuid::new_v4()],
            },
            ranking: None,
            work_limit: 1,
            comment_mode: CommentMode::AllPublic,
        };
        assert!(input.validate().is_ok());
        input.work_limit = 0;
        assert!(input.validate().is_err());
        assert_eq!(
            serde_json::to_value(CommentMode::AllPublic).unwrap(),
            "all_public"
        );
    }
}

/// Full execution history is pageable independently of the user-run grouping. Parameters
/// constrain the server query; records outside a selected domain/run never enter a page.
pub async fn executions(
    db: &Database,
    domain: Option<Uuid>,
    run: Option<Uuid>,
    page: i64,
) -> Result<Value, RunError> {
    if run.is_some() && domain.is_none() {
        return Err(RunError::Invalid("execution_domain_required"));
    }
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let scope = "FROM linggan_runtime_task t LEFT JOIN collection_work_order_lease_task lt USING(task_id) LEFT JOIN collection_work_order_lease l USING(lease_ref) LEFT JOIN collection_work_order w USING(work_order_ref) WHERE ($1::uuid IS NULL OR EXISTS(SELECT 1 FROM collection_work_order_domain_usage d WHERE d.work_order_ref=w.work_order_ref AND d.domain_ref=$1)) AND ($2::uuid IS NULL OR EXISTS(SELECT 1 FROM collection_user_order u WHERE u.run_ref=$2 AND u.domain_ref=$1 AND u.work_order_ref=w.work_order_ref))";
    let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) {scope}")))
        .bind(domain)
        .bind(run)
        .fetch_one(&mut *tx)
        .await?;
    let items:Vec<Value>=sqlx::query_scalar(sqlx::AssertSqlSafe(format!(r#"SELECT jsonb_build_object('taskId',t.task_id,'source',t.source,'createdAt',t.created_at,'capabilities',t.task_spec->'capabilitiesRequested','target',t.task_spec->'target',
      'workOrderRef',w.work_order_ref,'queueState',w.queue_state,'stageState',lt.execution_state,
      'frozenScope',jsonb_build_object('lane',w.lane,'workLimit',w.max_works,'stopConditions',w.stop_conditions),
      'lease',CASE WHEN l.lease_ref IS NULL THEN NULL ELSE jsonb_build_object('leaseRef',l.lease_ref,'stationRef',l.station_ref,'issuedAt',l.issued_at,'expiresAt',l.expires_at,'releasedAt',l.released_at,'releaseReason',l.release_reason) END,
      'attempts',COALESCE((SELECT jsonb_agg(jsonb_build_object('attemptId',a.attempt_id,'startedAt',a.started_at,'packageRef',p.package_ref,'packageKind',p.package_kind,'coverage',p.coverage,'receiptRef',r.receipt_ref,'executionEffect',r.execution_effect,'materialAdmission',r.material_admission,'receivedAt',r.received_at) ORDER BY a.started_at,a.attempt_id) FROM linggan_runtime_attempt a LEFT JOIN linggan_runtime_capture_package p USING(attempt_id) LEFT JOIN linggan_runtime_submission_receipt r USING(attempt_id) WHERE a.task_id=t.task_id),'[]'::jsonb),
      'delivery',COALESCE((SELECT jsonb_agg(jsonb_build_object('sessionRef',s.session_ref,'state',s.state,'stopReason',s.stop_reason,'lastProgressAt',s.last_progress_at,'preparedLanes',(SELECT count(*) FROM collection_detail_page_session_lane_preparation lp WHERE lp.session_ref=s.session_ref),'receivedLanes',(SELECT count(*) FROM collection_detail_page_session_lane_preparation lp JOIN linggan_runtime_submission_receipt r USING(attempt_id) WHERE lp.session_ref=s.session_ref))) FROM collection_detail_page_session s WHERE s.work_order_ref=w.work_order_ref),'[]'::jsonb))
      {scope} ORDER BY t.created_at DESC,t.task_id LIMIT 25 OFFSET $3"#))).bind(domain).bind(run).bind((page.max(1).min(1000000)-1)*25).fetch_all(&mut *tx).await?;
    tx.commit().await?;
    Ok(json!({"items":items,"total":count,"page":page.max(1),"pageSize":25}))
}
