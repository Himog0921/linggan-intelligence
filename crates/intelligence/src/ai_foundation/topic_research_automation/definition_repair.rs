//! Reuse approved extraction; append only a new assignment and its review to the existing ledger.
use super::FoundationError;
use crate::ai_foundation::{
    batch_store::{self, BatchInput, BatchKind, InputRef},
    store::Command,
    task_config::lock_setting,
    topic_research::{self as owner, Receipt, TASKS},
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

pub(super) async fn tick(
    db: &Database,
    cursor: &mut Option<(Uuid, Uuid)>,
) -> Result<usize, FoundationError> {
    let rows = sqlx::query(include_str!("definition_candidates.sql"))
        .bind(crate::topic_map_research_analysis::METHOD_VERSION)
        .bind(cursor.map(|v| v.0))
        .bind(cursor.map(|v| v.1))
        .fetch_all(db.pool())
        .await?;
    if rows.is_empty() {
        *cursor = None;
    }
    let mut queued = 0;
    for row in rows {
        *cursor = Some((row.get("domain_ref"), row.get("window_ref")));
        match enqueue(
            db,
            row.get("domain_ref"),
            row.get("window_ref"),
            row.get("command_ref"),
            row.get("versions"),
        )
        .await
        {
            Ok(_) => queued += 1,
            Err(FoundationError::Database(e)) => return Err(e.into()),
            Err(_) => {}
        }
    }
    Ok(queued)
}
async fn enqueue(
    db: &Database,
    domain: Uuid,
    source: Uuid,
    command_ref: Uuid,
    versions: Value,
) -> Result<Receipt, FoundationError> {
    // Restore exact original Core under current material restrictions, never a latest-Core substitution.
    let frozen = owner::frozen(db, source).await?;
    let mut command = Command::begin(
        db,
        command_ref,
        "topic_definition_repair",
        &json!({"domainRef":domain,"sourceWindow":source,"versions":versions}),
    )
    .await?;
    if let Some(receipt) = command.replay()? {
        return Ok(receipt);
    }
    let enabled: Option<bool> = sqlx::query_scalar(
        "SELECT enabled FROM linggan_topic_research_automation WHERE domain_ref=$1 FOR SHARE",
    )
    .bind(domain)
    .fetch_optional(&mut *command.tx)
    .await?;
    if enabled != Some(true) {
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
    let controls=sqlx::query("SELECT enabled FROM linggan_ai_control WHERE scope_key IN('global','executor','task:topic_research_resolve','task:topic_research_resolve_review') ORDER BY scope_key FOR SHARE")
        .fetch_all(&mut *command.tx).await?;
    if controls.len() != 4 || controls.iter().any(|row| !row.get::<bool, _>("enabled")) {
        return Err(FoundationError::AdmissionDenied(vec![
            "executor_off".into(),
        ]));
    }
    let pending:i64=sqlx::query_scalar("SELECT count(DISTINCT w.window_ref) FROM linggan_topic_research_run r JOIN linggan_topic_research_window w USING(run_ref) JOIN linggan_topic_research_stage s USING(window_ref) JOIN linggan_ai_batch_item i USING(item_ref) JOIN linggan_ai_batch b USING(batch_ref) JOIN linggan_ai_batch_item_control c USING(item_ref) WHERE r.domain_ref=$1 AND b.kind='automatic' AND c.state IN('pending','running')").bind(domain).fetch_one(&mut *command.tx).await?;
    if pending >= 8 {
        return Err(FoundationError::AdmissionDenied(vec![
            "topic_automatic_queue_full".into(),
        ]));
    }
    let row=sqlx::query("SELECT w.input_refs,w.input_hash,w.work_ref,r.configurations,r.knowledge_cutoff::text AS cutoff FROM linggan_topic_research_window w JOIN linggan_topic_research_run r USING(run_ref) WHERE window_ref=$1 AND r.domain_ref=$2").bind(source).bind(domain).fetch_one(&mut *command.tx).await?;
    let conflict: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_topic_research_window w JOIN linggan_topic_research_run r USING(run_ref) JOIN linggan_topic_research_stage s USING(window_ref) JOIN linggan_ai_batch_item_control c USING(item_ref) WHERE r.domain_ref=$1 AND w.input_hash=$2 AND (c.state IN('pending','running') OR c.failure_code='ai_dispatch_unknown'))")
        .bind(domain).bind(row.get::<String,_>("input_hash")).fetch_one(&mut *command.tx).await?;
    if conflict {
        return Err(FoundationError::AdmissionDenied(vec![
            "ai_prior_dispatch_unresolved".into(),
        ]));
    }
    let mut manifest: Value = row.get("input_refs");
    let root = manifest
        .get("reusedExtractionWindow")
        .and_then(Value::as_str)
        .and_then(|id| id.parse::<Uuid>().ok())
        .unwrap_or(source);
    // Both outputs must have actually succeeded; no synthetic skipped/succeeded extraction stages.
    for stage in [0, 1] {
        let succeeded:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_topic_research_stage s JOIN linggan_ai_batch_item_control c USING(item_ref) JOIN linggan_topic_research_stage_result output USING(item_ref) WHERE s.window_ref=$1 AND s.stage_number=$2 AND c.state='succeeded' AND output.output IS NOT NULL AND output.failure_code IS NULL)").bind(root).bind(stage).fetch_one(&mut *command.tx).await?;
        if !succeeded {
            return Err(FoundationError::AdmissionDenied(vec![
                "topic_dependency_failed".into(),
            ]));
        }
    }
    let draft =
        crate::ai_foundation::topic_research_packet::draft_in(&mut command.tx, root).await?;
    crate::topic_map_research_analysis::validate_output(&draft, &frozen.fragments, &[])
        .map_err(|_| FoundationError::AdmissionDenied(vec!["ai_source_unavailable".into()]))?;
    let mut configs: Vec<Uuid> =
        serde_json::from_value(row.get("configurations")).map_err(|_| FoundationError::Invalid)?;
    configs[2] = active[2];
    configs[3] = active[3];
    manifest["reusedExtractionWindow"] = json!(root);
    manifest["definitionReassessment"] = json!({"sourceWindow":source,"versions":versions});
    let run = Uuid::new_v4();
    let window = Uuid::new_v4();
    let work: Uuid = row.get("work_ref");
    sqlx::query("INSERT INTO linggan_topic_research_run(run_ref,command_ref,domain_ref,work_refs,knowledge_cutoff,configurations) VALUES($1,$2,$3,$4,$5::timestamptz,$6)").bind(run).bind(command_ref).bind(domain).bind(vec![work]).bind(row.get::<String,_>("cutoff")).bind(json!(configs)).execute(&mut *command.tx).await?;
    sqlx::query("INSERT INTO linggan_topic_research_window(window_ref,run_ref,work_ref,input_hash,input_refs) VALUES($1,$2,$3,$4,$5)").bind(window).bind(run).bind(work).bind(row.get::<String,_>("input_hash")).bind(manifest).execute(&mut *command.tx).await?;
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
            BatchKind::Automatic,
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
}
