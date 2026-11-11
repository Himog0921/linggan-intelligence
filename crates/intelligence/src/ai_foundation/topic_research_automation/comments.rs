//! Saved comment semantics enter the same assignment queue; no semantic stage is fabricated.
use super::FoundationError;
use crate::ai_foundation::{
    batch_store::{self, BatchInput, BatchKind, InputRef},
    store::Command,
    task_config::lock_setting,
    topic_research::{Receipt, TASKS},
};
use linggan_storage_postgres::Database;
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

pub(super) async fn tick(
    db: &Database,
    cursor: &mut Option<(Uuid, Uuid)>,
) -> Result<usize, FoundationError> {
    let rows = sqlx::query(include_str!("comment_candidates.sql"))
        .bind(cursor.map(|v| v.0))
        .bind(cursor.map(|v| v.1))
        .fetch_all(db.pool())
        .await?;
    if rows.is_empty() {
        *cursor = None;
    }
    let mut queued = 0;
    for row in rows {
        *cursor = Some((row.get("domain_ref"), row.get("request_ref")));
        match enqueue(db, row.get("domain_ref"), row.get("request_ref")).await {
            Ok(Some(_)) => queued += 1,
            Ok(None) => {}
            Err(FoundationError::Database(e)) => return Err(e.into()),
            Err(_) => {}
        }
    }
    Ok(queued)
}
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Retry {
    pub command_ref: Uuid,
    pub domain_ref: Uuid,
    pub window_ref: Uuid,
}

pub async fn retry(db: &Database, request: &Retry) -> Result<Receipt, FoundationError> {
    let source: Option<String> = sqlx::query_scalar("SELECT w.input_refs->>'semanticRequestRef' FROM linggan_topic_research_window w JOIN linggan_topic_research_run r USING(run_ref) WHERE w.window_ref=$1 AND r.domain_ref=$2 AND w.input_refs->>'inputContract'=$3")
        .bind(request.window_ref).bind(request.domain_ref).bind(super::super::topic_comment_source::CONTRACT).fetch_optional(db.pool()).await?.flatten();
    let source = source
        .and_then(|s| s.parse().ok())
        .ok_or(FoundationError::NotFound)?;
    enqueue_with_retry(db, request.domain_ref, source, Some(request))
        .await?
        .ok_or(FoundationError::Invalid)
}

pub(crate) async fn enqueue(
    db: &Database,
    domain: Uuid,
    source: Uuid,
) -> Result<Option<Receipt>, FoundationError> {
    enqueue_with_retry(db, domain, source, None).await
}
async fn enqueue_with_retry(
    db: &Database,
    domain: Uuid,
    source: Uuid,
    retry: Option<&Retry>,
) -> Result<Option<Receipt>, FoundationError> {
    let catalog = crate::topic_map_core::catalog(db, domain)
        .await
        .map_err(|e| match e {
            crate::model_settings::ModelError::Database(e) => FoundationError::Database(e),
            _ => FoundationError::Invalid,
        })?;
    let versions = super::super::topic_comment_source::catalog_versions(&catalog);
    let previous=sqlx::query("SELECT w.input_refs,result.core_json,dispatch.context FROM linggan_topic_research_window w JOIN linggan_topic_research_run r USING(run_ref) LEFT JOIN linggan_topic_research_result result USING(window_ref) LEFT JOIN linggan_topic_research_stage s ON s.window_ref=w.window_ref AND s.stage_number=2 LEFT JOIN linggan_topic_research_dispatch dispatch USING(item_ref) WHERE r.domain_ref=$1 AND w.input_refs->>'semanticRequestRef'=$2 ORDER BY r.created_at DESC,r.run_ref DESC LIMIT 1").bind(domain).bind(source.to_string()).fetch_optional(db.pool()).await?;
    if let Some(previous) = previous.filter(|_| retry.is_none()) {
        let core: Option<serde_json::Value> = previous.get("core_json");
        let context: Option<serde_json::Value> = previous.get("context");
        let manifest: serde_json::Value = previous.get("input_refs");
        let frozen = context
            .as_ref()
            .map(|c| super::super::topic_comment_source::catalog_versions(&c["candidates"]))
            .unwrap_or_else(|| manifest["catalogVersions"].clone());
        let current = core
            .as_ref()
            .and_then(|c| c["units"].as_array())
            .is_some_and(|units| {
                units.iter().all(|u| {
                    super::super::topic_comment_source::assignment_current(u, &frozen, &catalog)
                })
            });
        if (current || (core.is_none() && frozen == versions))
            && manifest["methodVersion"] == crate::topic_map_research_analysis::METHOD_VERSION
        {
            return Ok(None);
        }
    }
    let key=linggan_evidence::creator_discovery::hash(&json!({"contract":super::super::topic_comment_source::CONTRACT,"domain":domain,"semanticRequestRef":source,"versions":versions,"method":crate::topic_map_research_analysis::METHOD_VERSION}).to_string());
    let command_ref = retry
        .map(|r| r.command_ref)
        .unwrap_or(Uuid::parse_str(&key[..32]).map_err(|_| FoundationError::Invalid)?);
    let mut command = Command::begin(
        db,
        command_ref,
        "topic_comment_assignment",
        &match retry { Some(r) => json!({"domainRef":domain,"semanticRequestRef":source,"retryWindowRef":r.window_ref}), None => json!({"domainRef":domain,"semanticRequestRef":source,"catalogVersions":versions}) },
    )
    .await?;
    if let Some(receipt) = command.replay::<Receipt>()? {
        return Ok(retry.map(|_| receipt));
    }
    let enabled: Option<bool> = sqlx::query_scalar(
        "SELECT enabled FROM linggan_topic_research_automation WHERE domain_ref=$1 FOR SHARE",
    )
    .bind(domain)
    .fetch_optional(&mut *command.tx)
    .await?;
    if retry.is_none() && enabled != Some(true) {
        return Err(FoundationError::AdmissionDenied(vec![
            "topic_automation_disabled".into(),
        ]));
    }
    // Same setting lock order as ordinary starts also serializes queue admission/cap checks.
    let mut active = Vec::new();
    for task in TASKS {
        let revision: i64 =
            sqlx::query_scalar("SELECT revision FROM linggan_ai_task_setting WHERE task_key=$1")
                .bind(task.as_str())
                .fetch_one(&mut *command.tx)
                .await?;
        active.push(lock_setting(&mut command.tx, task, revision).await?);
    }
    if let Some(retry) = retry {
        let allowed: bool = sqlx::query_scalar(include_str!("comment_retry_eligible.sql"))
            .bind(retry.window_ref)
            .bind(domain)
            .bind(source.to_string())
            .fetch_one(&mut *command.tx)
            .await?;
        if !allowed {
            return Err(FoundationError::AdmissionDenied(vec![
                "topic_comment_retry_not_available".into(),
            ]));
        }
    }
    let controls=sqlx::query("SELECT enabled FROM linggan_ai_control WHERE scope_key IN('global','executor','task:topic_research_resolve','task:topic_research_resolve_review') ORDER BY scope_key FOR SHARE")
        .fetch_all(&mut *command.tx).await?;
    if controls.len() != 4 || controls.iter().any(|row| !row.get::<bool, _>("enabled")) {
        return Err(FoundationError::AdmissionDenied(vec![
            "executor_off".into(),
        ]));
    }
    let pending:i64=sqlx::query_scalar("SELECT count(DISTINCT w.window_ref) FROM linggan_topic_research_run r JOIN linggan_topic_research_window w USING(run_ref) JOIN linggan_topic_research_stage s USING(window_ref) JOIN linggan_ai_batch_item i USING(item_ref) JOIN linggan_ai_batch b USING(batch_ref) JOIN linggan_ai_batch_item_control c USING(item_ref) WHERE r.domain_ref=$1 AND b.kind='automatic' AND c.state IN('pending','running')").bind(domain).fetch_one(&mut *command.tx).await?;
    if retry.is_none() && pending >= 8 {
        return Err(FoundationError::AdmissionDenied(vec![
            "topic_automatic_queue_full".into(),
        ]));
    }
    let input = super::super::topic_comment_source::load_in(&mut command.tx, source).await?;
    if input.domain["domainRef"] != json!(domain) || input.fragments.is_empty() {
        return Err(FoundationError::Invalid);
    }
    let conflict:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_topic_research_window w JOIN linggan_topic_research_run r USING(run_ref) JOIN linggan_topic_research_stage s USING(window_ref) JOIN linggan_ai_batch_item_control c USING(item_ref) WHERE r.domain_ref=$1 AND w.input_refs->>'semanticRequestRef'=$2 AND (c.state IN('pending','running') OR c.failure_code='ai_dispatch_unknown'))").bind(domain).bind(source.to_string()).fetch_one(&mut *command.tx).await?;
    if conflict {
        return Err(FoundationError::AdmissionDenied(vec![
            "ai_prior_dispatch_unresolved".into(),
        ]));
    }
    let configs = active;
    let manifest = json!({"inputContract":super::super::topic_comment_source::CONTRACT,"semanticRequestRef":source,"inputHash":input.hash,"catalogVersions":versions,"methodVersion":crate::topic_map_research_analysis::METHOD_VERSION});
    let run = Uuid::new_v4();
    let window = Uuid::new_v4();
    let work = input.work.work_ref;
    let cutoff = input.coverage["asOf"]
        .as_str()
        .ok_or(FoundationError::Invalid)?;
    sqlx::query("INSERT INTO linggan_topic_research_run(run_ref,command_ref,domain_ref,work_refs,knowledge_cutoff,configurations) VALUES($1,$2,$3,$4,$5::timestamptz,$6)").bind(run).bind(command_ref).bind(domain).bind(vec![work]).bind(cutoff).bind(json!(configs)).execute(&mut *command.tx).await?;
    sqlx::query("INSERT INTO linggan_topic_research_window(window_ref,run_ref,work_ref,input_hash,input_refs) VALUES($1,$2,$3,$4,$5)").bind(window).bind(run).bind(work).bind(&input.hash).bind(manifest).execute(&mut *command.tx).await?;
    for index in [2, 3] {
        let task = TASKS[index];
        let inputs = vec![BatchInput {
            item_key: format!("{window}:{index}"),
            input: InputRef::TopicResearch {
                window_ref: window,
                stage_number: index as u8,
            },
        }];
        let hash = batch_store::input_hash(task, &inputs, false)?;
        let batch = batch_store::insert_batch(
            &mut command.tx,
            command_ref,
            task,
            configs[index],
            if retry.is_some() {
                BatchKind::Interactive
            } else {
                BatchKind::Automatic
            },
            &inputs,
            &hash,
        )
        .await?;
        sqlx::query("INSERT INTO linggan_topic_research_stage(item_ref,window_ref,stage_number) SELECT item_ref,$2,$3 FROM linggan_ai_batch_item WHERE batch_ref=$1").bind(batch).bind(window).bind(index as i32).execute(&mut *command.tx).await?;
    }
    command
        .finish(Receipt {
            run_ref: run,
            window_count: 1,
            stage_count: 2,
        })
        .await
        .map(Some)
}
