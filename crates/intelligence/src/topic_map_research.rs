//! Topic-map research commands and immutable canonical input identity.
use crate::topic_map_research_analysis::METHOD_VERSION;
use linggan_evidence::creator_discovery::{self, DiscoveryWork, Fragment};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

#[path = "topic_map_research/identity.rs"]
mod identity;
pub(crate) use identity::semantic_source_identity;
#[path = "topic_map_research/admission.rs"]
mod admission;
#[path = "topic_map_research/queue.rs"]
mod queue;
#[path = "topic_map_research/restoration.rs"]
mod restoration;
#[path = "topic_map_research/source.rs"]
mod source;
#[path = "topic_map_research/windows.rs"]
mod windows;
pub(crate) use queue::{
    attach_legacy_media_aliases, overlaps_unknown_dispatch, queue_automatic_run, queue_research_run,
};
#[cfg(test)]
use restoration::sources_current;
pub(crate) use restoration::{
    research_result_sources_current, restore_legacy_scoped, restore_scoped_window, restore_window,
};
pub(crate) use source::{load_inputs, load_inputs_for_works, manifest_source_work_refs};
pub(crate) use windows::{
    input_identity, is_research_evidence, reference_manifest, research_windows,
    selected_comment_study, with_comparison_context,
};
#[cfg(test)]
#[path = "topic_map_research/tests.rs"]
mod input_tests;

#[derive(Debug, thiserror::Error)]
pub enum ResearchError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("{0}")]
    Invalid(&'static str),
    #[error("research_not_found")]
    NotFound,
    #[error("request_identity_conflict")]
    Conflict,
}
impl ResearchError {
    pub fn code(&self) -> &str {
        match self {
            Self::Database(_) => "research_read_failed",
            Self::Invalid(c) => c,
            Self::NotFound => "research_not_found",
            Self::Conflict => "request_identity_conflict",
        }
    }
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(
    tag = "action",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum ResearchCommand {
    Configure {
        request_ref: Uuid,
        domain_ref: Uuid,
        model_config_ref: Uuid,
        daily_token_limit: i64,
        run_token_limit: i64,
        automatic_enabled: bool,
        collection_enabled: bool,
    },
    Start {
        request_ref: Uuid,
        domain_ref: Uuid,
        trigger: String,
        #[serde(default)]
        work_refs: Vec<Uuid>,
        topic_ref: Option<Uuid>,
    },
    Pause {
        request_ref: Uuid,
        domain_ref: Uuid,
        run_ref: Option<Uuid>,
    },
    Resume {
        request_ref: Uuid,
        domain_ref: Uuid,
        run_ref: Option<Uuid>,
    },
    Stop {
        request_ref: Uuid,
        domain_ref: Uuid,
        run_ref: Option<Uuid>,
    },
}
impl ResearchCommand {
    fn identity(&self) -> (Uuid, Uuid) {
        match self {
            Self::Configure {
                request_ref,
                domain_ref,
                ..
            }
            | Self::Start {
                request_ref,
                domain_ref,
                ..
            }
            | Self::Pause {
                request_ref,
                domain_ref,
                ..
            }
            | Self::Resume {
                request_ref,
                domain_ref,
                ..
            }
            | Self::Stop {
                request_ref,
                domain_ref,
                ..
            } => (*request_ref, *domain_ref),
        }
    }
}
#[derive(Debug, Clone)]
pub(crate) struct ResearchInput {
    pub work: DiscoveryWork,
    pub fragments: Vec<Fragment>,
    pub topics: Value,
    pub domain: Value,
    pub hash: String,
    pub context_work_refs: Vec<Uuid>,
    pub comment_study: Value,
    pub role_metadata: Value,
    pub coverage: Value,
}

const INPUT_CONTRACT: &str = "topic-map.source-windows.v2";
const PARENT_CONTEXT_FIELD: &str = "parent_comment_context";
const RESEARCH_WINDOW_CHARS: usize = 3000;
pub async fn apply_research_command(
    db: &Database,
    command: &ResearchCommand,
) -> Result<Value, ResearchError> {
    let (request, domain) = command.identity();
    let hash = creator_discovery::hash(
        &serde_json::to_string(command).map_err(|_| ResearchError::Invalid("invalid_command"))?,
    );
    let mut tx = db.pool().begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,41))")
        .bind(request.to_string())
        .execute(&mut *tx)
        .await?;
    if let Some(row) = sqlx::query(
        "SELECT command_hash,receipt FROM linggan_topic_map_research_command WHERE request_ref=$1",
    )
    .bind(request)
    .fetch_optional(&mut *tx)
    .await?
    {
        if row.get::<String, _>("command_hash") != hash {
            return Err(ResearchError::Conflict);
        }
        let receipt: Value = row.get("receipt");
        if receipt["state"] != "creating" {
            return Ok(receipt);
        }
        tx.rollback().await?;
        return finish_pending_start(db, command).await;
    }
    let valid: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM observation_domain WHERE domain_ref=$1)")
            .bind(domain)
            .fetch_one(&mut *tx)
            .await?;
    if !valid {
        return Err(ResearchError::NotFound);
    }
    let mut start = None;
    let receipt = match command {
        ResearchCommand::Configure {
            model_config_ref,
            daily_token_limit,
            run_token_limit,
            automatic_enabled,
            collection_enabled,
            ..
        } => {
            if !(1024..=10000000).contains(daily_token_limit)
                || !(1024..=10000000).contains(run_token_limit)
            {
                return Err(ResearchError::Invalid("invalid_research_budget"));
            }
            let config: Option<Uuid> = sqlx::query_scalar(
                "SELECT config_ref FROM linggan_model_config WHERE config_ref=$1",
            )
            .bind(model_config_ref)
            .fetch_optional(&mut *tx)
            .await?;
            if config.is_none() {
                return Err(ResearchError::Invalid("model_configuration_missing"));
            }
            let previous:Option<bool>=sqlx::query_scalar("SELECT automatic_enabled FROM linggan_topic_map_research_policy WHERE domain_ref=$1 FOR UPDATE").bind(domain).fetch_optional(&mut *tx).await?;
            sqlx::query("INSERT INTO linggan_topic_map_research_policy(domain_ref,model_config_ref,daily_token_limit,run_token_limit,automatic_enabled,collection_enabled) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(domain_ref) DO UPDATE SET model_config_ref=$2,daily_token_limit=$3,run_token_limit=$4,automatic_enabled=$5,collection_enabled=$6,status='active',revision=linggan_topic_map_research_policy.revision+1,updated_at=scope_001_now()").bind(domain).bind(model_config_ref).bind(daily_token_limit).bind(run_token_limit).bind(automatic_enabled).bind(collection_enabled).execute(&mut *tx).await?;
            sqlx::query("UPDATE linggan_topic_map_research_run SET state=CASE WHEN $2 THEN 'queued' ELSE 'paused' END,last_reason=CASE WHEN $2 THEN NULL ELSE 'automatic_disabled' END,updated_at=scope_001_now() WHERE domain_ref=$1 AND trigger<>'on_demand'AND (($2 AND state='paused'AND last_reason='automatic_disabled')OR(NOT $2 AND state IN('queued','running','daily_budget_paused')))").bind(domain).bind(automatic_enabled).execute(&mut *tx).await?;
            if *automatic_enabled && previous != Some(true) {
                start = Some(("historical".to_string(), vec![], None));
            }
            json!({"requestRef":request,"domainRef":domain,"state":"configured","automaticEnabled":automatic_enabled,"collectionEnabled":collection_enabled})
        }
        ResearchCommand::Start {
            trigger,
            work_refs,
            topic_ref,
            ..
        } => {
            if !["historical", "incremental", "on_demand"].contains(&trigger.as_str())
                || work_refs.len() > 100
            {
                return Err(ResearchError::Invalid("invalid_research_scope"));
            }
            start = Some((trigger.clone(), work_refs.clone(), *topic_ref));
            json!({"requestRef":request,"domainRef":domain,"state":"queued"})
        }
        ResearchCommand::Pause { run_ref, .. }
        | ResearchCommand::Resume { run_ref, .. }
        | ResearchCommand::Stop { run_ref, .. } => {
            let state = match command {
                ResearchCommand::Pause { .. } => "paused",
                ResearchCommand::Resume { .. } => "queued",
                _ => "stopped",
            };
            if let Some(run) = run_ref {
                let count=sqlx::query("UPDATE linggan_topic_map_research_run SET state=$3,last_reason=$3,updated_at=scope_001_now() WHERE run_ref=$1 AND domain_ref=$2 AND state IN ('queued','running','daily_budget_paused','paused')").bind(run).bind(domain).bind(state).execute(&mut *tx).await?.rows_affected();
                if count == 0 {
                    return Err(ResearchError::Invalid("run_terminal_or_unavailable"));
                }
                if state == "stopped" {
                    sqlx::query("UPDATE linggan_topic_map_research_task SET state='stopped',last_reason='user_stopped' WHERE run_ref=$1 AND state='queued'").bind(run).execute(&mut *tx).await?;
                }
            } else {
                let policy_state = if state == "queued" { "active" } else { state };
                sqlx::query("UPDATE linggan_topic_map_research_policy SET status=$2,updated_at=scope_001_now() WHERE domain_ref=$1").bind(domain).bind(policy_state).execute(&mut *tx).await?;
                if state == "stopped" {
                    sqlx::query("UPDATE linggan_topic_map_research_run SET state='stopped',last_reason='user_stopped' WHERE domain_ref=$1 AND state IN ('queued','running','paused','daily_budget_paused')").bind(domain).execute(&mut *tx).await?;
                }
            }
            json!({"requestRef":request,"domainRef":domain,"runRef":run_ref,"state":state})
        }
    };
    // Queue loading uses canonical readers outside this short policy transaction.
    sqlx::query("INSERT INTO linggan_topic_map_research_command(request_ref,domain_ref,command_hash,receipt) VALUES($1,$2,$3,$4)").bind(request).bind(domain).bind(hash).bind(if start.is_some(){json!({"requestRef":request,"domainRef":domain,"state":"creating"})}else{receipt.clone()}).execute(&mut *tx).await?;
    tx.commit().await?;
    if start.is_some() {
        return finish_pending_start(db, command).await;
    }
    Ok(receipt)
}
async fn finish_pending_start(
    db: &Database,
    command: &ResearchCommand,
) -> Result<Value, ResearchError> {
    let (request, domain) = command.identity();
    let (trigger, works, topic) = match command {
        ResearchCommand::Start {
            trigger,
            work_refs,
            topic_ref,
            ..
        } => (trigger.as_str(), work_refs.as_slice(), *topic_ref),
        ResearchCommand::Configure {
            automatic_enabled: true,
            ..
        } => ("historical", &[][..], None),
        _ => return Err(ResearchError::Invalid("invalid_pending_command")),
    };
    let run = queue_research_run(db, domain, request, trigger, works, topic).await?;
    let receipt = json!({"requestRef":request,"domainRef":domain,"state":if run.is_some(){"queued"}else{"no_new_input"},"runRef":run});
    sqlx::query("UPDATE linggan_topic_map_research_command SET receipt=$2 WHERE request_ref=$1 AND receipt->>'state'='creating'").bind(request).bind(&receipt).execute(db.pool()).await?;
    Ok(receipt)
}
pub async fn read_research_progress(db: &Database, domain: Uuid) -> Result<Value, ResearchError> {
    let p=sqlx::query("SELECT *,updated_at::text AS updated FROM linggan_topic_map_research_policy WHERE domain_ref=$1").bind(domain).fetch_optional(db.pool()).await?;
    let policy=p.map(|r|json!({"modelConfigRef":r.get::<Uuid,_>("model_config_ref"),"dailyTokenLimit":r.get::<i64,_>("daily_token_limit"),"runTokenLimit":r.get::<i64,_>("run_token_limit"),"automaticEnabled":r.get::<bool,_>("automatic_enabled"),"collectionEnabled":r.get::<bool,_>("collection_enabled"),"status":r.get::<String,_>("status"),"updatedAt":r.get::<String,_>("updated")}));
    let usage=sqlx::query("SELECT COALESCE(sum(i.charged_tokens),0)::bigint AS charged,COALESCE(sum(i.reserved_tokens)FILTER(WHERE i.state='running'),0)::bigint AS reserved FROM linggan_topic_map_research_request q JOIN linggan_model_invocation i USING(invocation_ref) JOIN linggan_topic_map_research_run r USING(run_ref) WHERE r.domain_ref=$1 AND q.budget_day=(scope_001_now() AT TIME ZONE 'Asia/Shanghai')::date").bind(domain).fetch_one(db.pool()).await?;
    let rows = read_research_runs(db, domain).await?;
    let summary = read_research_summary(db, domain).await?;
    let models=sqlx::query("SELECT c.*,m.model_id FROM linggan_model_config c JOIN linggan_model_entry m USING(model_ref) JOIN linggan_model_connection_version v ON v.version_ref=m.connection_version_ref JOIN linggan_model_connection conn USING(connection_ref) WHERE conn.enabled ORDER BY c.created_at DESC LIMIT 30").fetch_all(db.pool()).await?;
    let targets=sqlx::query("SELECT t.target_ref,t.target_kind,t.display_name,t.identity_key FROM collection_observation_target t JOIN observation_domain_target membership USING(target_ref)WHERE membership.domain_ref=$1 AND t.lifecycle_state<>'dismissed'ORDER BY t.display_name,t.target_ref LIMIT 100").bind(domain).fetch_all(db.pool()).await?;
    let rounds=sqlx::query("SELECT r.*,r.created_at::text AS created,(SELECT count(*)FROM linggan_topic_map_collection_slot s WHERE s.round_ref=r.round_ref)AS reserved_count FROM linggan_topic_map_collection_round r WHERE r.domain_ref=$1 ORDER BY created_at DESC LIMIT 20").bind(domain).fetch_all(db.pool()).await?;
    Ok(
        json!({"domainRef":domain,"methodVersion":METHOD_VERSION,"policy":policy,"targets":targets.iter().map(|r|json!({"targetRef":r.get::<Uuid,_>("target_ref"),"kind":r.get::<String,_>("target_kind"),"displayName":r.get::<Option<String>,_>("display_name").unwrap_or(r.get::<String,_>("identity_key")),"domainRef":domain})).collect::<Vec<_>>(),"collectionRounds":rounds.iter().map(|r|json!({"roundRef":r.get::<Uuid,_>("round_ref"),"kind":r.get::<String,_>("kind"),"state":r.get::<String,_>("state"),"reason":r.get::<Option<String>,_>("last_reason"),"detailSlotsReserved":r.get::<i64,_>("reserved_count"),"detailLimit":r.get::<i32,_>("detail_limit"),"createdAt":r.get::<String,_>("created")})).collect::<Vec<_>>(),"usage":{"chargedTokens":usage.get::<i64,_>("charged"),"reservedTokens":usage.get::<i64,_>("reserved"),"timezone":"Asia/Shanghai"},"summary":summary,"runListLimit":30,"runs":rows.iter().map(research_run_progress).collect::<Vec<_>>(),"models":models.iter().map(|r|json!({"configRef":r.get::<Uuid,_>("config_ref"),"modelId":r.get::<String,_>("model_id"),"inputTokenLimit":r.get::<i32,_>("input_token_limit"),"outputTokenLimit":r.get::<i32,_>("output_token_limit"),"maxAttempts":r.get::<i32,_>("max_attempts"),"timeoutSeconds":r.get::<i32,_>("timeout_seconds")})).collect::<Vec<_>>() }),
    )
}

macro_rules! task_progress_query {
    ($prefix:literal, $suffix:literal) => {
        concat!(
            $prefix,
            r#"
    'totalTaskCount',count(*),
    'queuedCount',count(*) FILTER(WHERE state='queued'),
    'runningCount',count(*) FILTER(WHERE state='running'),
    'succeededCount',count(*) FILTER(WHERE state='succeeded'),
    'noSignalCount',count(*) FILTER(WHERE state='no_signal'),
    'insufficientCount',count(*) FILTER(WHERE state='insufficient'),
    'failedCount',count(*) FILTER(WHERE state='failed'),
    'unknownDispatchCount',count(*) FILTER(WHERE state='unknown_dispatch'),
    'staleCount',count(*) FILTER(WHERE state='stale'),
    'stoppedCount',count(*) FILTER(WHERE state='stopped'),
    'extractedWindowCount',count(DISTINCT (work_public_ref,input_hash)) FILTER(WHERE distilled_json IS NOT NULL AND phase<>'compare' AND NOT (recall_manifest ? 'backfill')),
    'initialWindowCount',count(DISTINCT (work_public_ref,input_hash)) FILTER(WHERE phase<>'compare' AND NOT (recall_manifest ? 'backfill')),
    'completedInitialWindowCount',count(DISTINCT (work_public_ref,input_hash)) FILTER(WHERE state IN ('succeeded','no_signal','insufficient') AND phase<>'compare' AND NOT (recall_manifest ? 'backfill')),
    'coveredWorkCount',count(DISTINCT work_public_ref) FILTER(WHERE state IN ('succeeded','no_signal','insufficient') AND phase<>'compare' AND NOT (recall_manifest ? 'backfill')),
    'initialQueuedCount',count(*) FILTER(WHERE state='queued' AND phase<>'compare' AND NOT (recall_manifest ? 'backfill')),
    'reassessmentQueuedCount',count(*) FILTER(WHERE state='queued' AND recall_manifest ? 'backfill'),
    'reassessmentCompletedCount',count(*) FILTER(WHERE state='succeeded' AND recall_manifest ? 'backfill'),
    'phases',jsonb_build_object(
        'extractQueued',count(*) FILTER(WHERE phase='extract' AND state='queued'),
        'resolveQueued',count(*) FILTER(WHERE phase='resolve' AND state='queued'),
        'compareQueued',count(*) FILTER(WHERE phase='compare' AND state='queued'),
        'extracting',count(*) FILTER(WHERE phase='extract' AND state='running'),
        'resolving',count(*) FILTER(WHERE phase='resolve' AND state='running'),
        'comparing',count(*) FILTER(WHERE phase='compare' AND state='running'))
"#,
            $suffix
        )
    };
}

fn research_run_progress(row: &sqlx::postgres::PgRow) -> Value {
    let mut result = row.get::<Value, _>("counts");
    let object = result
        .as_object_mut()
        .expect("SQL builds task progress object");
    object.extend(
        json!({
            "runRef":row.get::<Uuid,_>("run_ref"),
            "trigger":row.get::<String,_>("trigger"),
            "state":row.get::<String,_>("state"),
            "methodVersion":row.get::<String,_>("method_version"),
            "lastReason":row.get::<Option<String>,_>("last_reason"),
            "taskIssues":row.get::<Value,_>("task_issues"),
            "inputScope":row.get::<Value,_>("input_scope"),
            "createdAt":row.get::<String,_>("created")
        })
        .as_object()
        .expect("fixed progress fields")
        .clone(),
    );
    result
}

async fn read_research_summary(db: &Database, domain: Uuid) -> Result<Value, ResearchError> {
    let query = task_progress_query!(
        "SELECT jsonb_build_object(",
        ") || jsonb_build_object(
            'totalRunCount',(SELECT count(*) FROM linggan_topic_map_research_run WHERE domain_ref=$1)) AS counts
         FROM linggan_topic_map_research_task WHERE domain_ref=$1"
    );
    Ok(sqlx::query(query)
        .bind(domain)
        .fetch_one(db.pool())
        .await?
        .get("counts"))
}

async fn read_research_runs(
    db: &Database,
    domain: Uuid,
) -> Result<Vec<sqlx::postgres::PgRow>, ResearchError> {
    let query = task_progress_query!(
        r#"SELECT r.*,r.created_at::text AS created,
            counts.statistics AS counts,COALESCE(issues.items,'[]'::jsonb) AS task_issues
        FROM (
            SELECT * FROM linggan_topic_map_research_run
            WHERE domain_ref=$1 ORDER BY created_at DESC,run_ref DESC LIMIT 30
        ) r
        CROSS JOIN LATERAL (
            SELECT jsonb_build_object("#,
        r#") AS statistics
            FROM linggan_topic_map_research_task t WHERE t.run_ref=r.run_ref
        ) counts
        CROSS JOIN LATERAL (
            SELECT jsonb_agg(jsonb_build_object(
                'taskRef',t.task_ref,'phase',t.phase,'state',t.state,'reason',t.last_reason)
                ORDER BY t.updated_at DESC,t.task_ref DESC) AS items
            FROM (
                SELECT task_ref,phase,state,last_reason,updated_at
                FROM linggan_topic_map_research_task
                WHERE run_ref=r.run_ref AND state IN ('failed','unknown_dispatch')
                ORDER BY updated_at DESC,task_ref DESC LIMIT 20
            ) t
        ) issues
        ORDER BY r.created_at DESC,r.run_ref DESC"#
    );
    Ok(sqlx::query(query).bind(domain).fetch_all(db.pool()).await?)
}
