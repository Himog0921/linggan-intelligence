//! Exact definition changes reuse accepted source distillation inside its original
//! run and budget. Recall only proposes bounded reassignment work; it admits none.
use crate::model_settings::ModelError;
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;
#[path = "backfill/authorization.rs"]
mod authorization;
pub(crate) use authorization::{authorize_scope, task_is_explicitly_authorized};

const TASK_LIMIT: usize = 12;
const DISCUSSION_LIMIT: usize = 240;
const DEFINITION_BATCH_LIMIT: usize = 6;
const ACTIVE_TASK_LIMIT: i64 = 2;
const RECALL_DEBOUNCE_SECONDS: i32 = 15 * 60;

struct UnitCandidate {
    task: Uuid,
    work: Uuid,
    input_hash: String,
    key: String,
    statement: String,
    dependent: bool,
    created: f64,
}

struct TaskCandidate {
    task: Uuid,
    keys: BTreeSet<String>,
    dependent: bool,
    score: f64,
    created: f64,
}

fn select_candidates(query: &str, units: Vec<UnitCandidate>) -> (Vec<TaskCandidate>, usize) {
    let scores = super::recall::lexical_scores(
        query,
        &units
            .iter()
            .map(|u| u.statement.clone())
            .collect::<Vec<_>>(),
    );
    let mut windows = BTreeMap::<(Uuid, String), TaskCandidate>::new();
    for (unit, score) in units.into_iter().zip(scores) {
        if !unit.dependent && score <= 0.0 {
            continue;
        }
        let candidate = windows
            .entry((unit.work, unit.input_hash))
            .or_insert_with(|| TaskCandidate {
                task: unit.task,
                keys: BTreeSet::new(),
                dependent: false,
                score: 0.0,
                created: unit.created,
            });
        if unit.created > candidate.created {
            candidate.task = unit.task;
            candidate.created = unit.created;
        }
        candidate.keys.insert(unit.key);
        candidate.dependent |= unit.dependent;
        candidate.score = candidate.score.max(score);
    }
    let mut candidates: Vec<_> = windows.into_values().collect();
    candidates.sort_by(|a, b| {
        b.dependent
            .cmp(&a.dependent)
            .then(b.score.total_cmp(&a.score))
            .then(b.created.total_cmp(&a.created))
            .then(a.task.cmp(&b.task))
    });
    let total = candidates.len();
    candidates.truncate(TASK_LIMIT);
    (candidates, total)
}

fn keep_other_resolutions(previous: &Value, keys: &[String]) -> Value {
    let keys: BTreeSet<_> = keys.iter().map(String::as_str).collect();
    json!(
        previous
            .as_array()
            .into_iter()
            .flatten()
            .filter(|unit| unit["unitId"].as_str().is_none_or(|id| !keys.contains(id)))
            .cloned()
            .collect::<Vec<_>>()
    )
}

/// A bounded batch of definition events and at most one source activation per tick. This
/// function never invokes a model, acquires material, or creates a research run.
pub(crate) async fn advance_once(db: &Database) -> Result<bool, ModelError> {
    let mut tx = db.pool().begin().await?;
    let owner: bool = sqlx::query_scalar(
        "SELECT pg_try_advisory_xact_lock(hashtextextended('topic-map-backfill',120))",
    )
    .fetch_one(&mut *tx)
    .await?;
    if !owner {
        return Ok(false);
    }
    let settled = sqlx::query("UPDATE linggan_topic_map_backfill_queue q SET state=CASE WHEN t.state='succeeded' THEN 'completed' ELSE 'skipped' END,last_reason=CASE WHEN t.state='succeeded' THEN NULL ELSE COALESCE(t.last_reason,t.state) END,updated_at=scope_001_now() FROM linggan_topic_map_research_task t WHERE q.queued_task_ref=t.task_ref AND q.state='active' AND t.state NOT IN ('queued','running')")
        .execute(&mut *tx).await?.rows_affected()>0;
    let blocked = sqlx::query("UPDATE linggan_topic_map_backfill_queue q SET state='skipped',last_reason=COALESCE(t.last_reason,t.state),updated_at=scope_001_now() FROM linggan_topic_map_research_task t WHERE q.source_task_ref=t.task_ref AND q.state='pending' AND t.state IN ('failed','stopped','stale','unknown_dispatch')")
        .execute(&mut *tx).await?.rows_affected()>0;
    let mut scanned = false;
    for _ in 0..DEFINITION_BATCH_LIMIT {
        if !scan_one(&mut tx).await? {
            break;
        }
        scanned = true;
    }
    let activated = activate_one(&mut tx).await?;
    tx.commit().await?;
    Ok(settled || blocked || scanned || activated)
}

async fn scan_one(tx: &mut Transaction<'_, Postgres>) -> Result<bool, ModelError> {
    let definition = sqlx::query(r#"
        SELECT d.definition_ref,d.topic_ref,b.domain_ref,d.version,d.display_name,d.definition_text,
               rule.inclusion_criteria,rule.exclusion_criteria
        FROM linggan_topic_definition d
        JOIN LATERAL(SELECT domain_ref FROM linggan_topic_map_binding WHERE topic_ref=d.topic_ref ORDER BY version DESC LIMIT 1)b ON true
        JOIN observation_domain domain ON domain.domain_ref=b.domain_ref AND domain.status='active'
        JOIN linggan_topic_map_research_policy policy ON policy.domain_ref=b.domain_ref AND policy.status='active'
        LEFT JOIN linggan_topic_map_concept_rule rule USING(definition_ref)
        WHERE d.version=(SELECT max(version) FROM linggan_topic_definition WHERE topic_ref=d.topic_ref)
          AND NOT EXISTS(SELECT 1 FROM linggan_topic_map_structure_source WHERE topic_ref=d.topic_ref)
          AND NOT EXISTS(SELECT 1 FROM linggan_topic_map_definition_scan WHERE definition_ref=d.definition_ref)
          AND COALESCE(rule.method_version,'')<>'topic-map.core.parent.v1'
        ORDER BY d.created_at,d.definition_ref LIMIT 1
    "#).fetch_optional(&mut **tx).await?;
    let Some(definition) = definition else {
        return Ok(false);
    };
    let definition_ref: Uuid = definition.get("definition_ref");
    let topic: Uuid = definition.get("topic_ref");
    let domain: Uuid = definition.get("domain_ref");
    let changed = definition.get::<i32, _>("version") > 1;
    let reason = if changed {
        "definition_changed"
    } else {
        "new_topic"
    };
    let concept = json!({"label":definition.get::<String,_>("display_name"),
        "definition":definition.get::<String,_>("definition_text"),
        "inclusionCriteria":definition.get::<Option<Vec<String>>,_>("inclusion_criteria").unwrap_or_default(),
        "exclusionCriteria":definition.get::<Option<Vec<String>>,_>("exclusion_criteria").unwrap_or_default()});
    let query = super::recall::concept_text(&concept);
    let terms: Vec<_> = super::recall::terms(&query)
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .take(48)
        .map(|term| format!("%{term}%"))
        .collect();
    // A single definition searches discussion statements, never the Cartesian
    // product of every work and every topic. The persisted manifest reports caps.
    let rows = sqlx::query(r#"
        WITH candidates AS (
          SELECT unit.unit_key,unit.work_public_ref,unit.statement,source.task_ref,source.input_hash,
                 extract(epoch FROM source.created_at)::float8 AS source_created,
                 ($4 AND (recent.candidate_manifest @> jsonb_build_object('topics',jsonb_build_array(jsonb_build_object('topicRef',$3::uuid)))
                   OR EXISTS(SELECT 1 FROM linggan_topic_map_unit_assignment a WHERE a.resolution_ref=recent.resolution_ref AND a.topic_ref=$3))) AS dependent
          FROM linggan_topic_map_discussion_unit unit
          JOIN LATERAL(
            SELECT resolution.* FROM linggan_topic_map_unit_resolution resolution
            JOIN linggan_topic_map_research_task task ON task.task_ref=resolution.task_ref
            WHERE resolution.unit_ref=unit.unit_ref AND task.state IN ('queued','running','succeeded') AND task.distilled_json IS NOT NULL
            ORDER BY resolution.created_at DESC,resolution.resolution_ref DESC LIMIT 1
          )recent ON true
          JOIN linggan_topic_map_research_task latest ON latest.task_ref=recent.task_ref
          JOIN LATERAL(
            SELECT initial.* FROM linggan_topic_map_research_task initial
            JOIN linggan_topic_map_research_run initial_run USING(run_ref)
            WHERE initial.domain_ref=unit.domain_ref AND initial.work_public_ref=unit.work_public_ref
              AND initial.input_hash=latest.input_hash AND initial.state='succeeded'
              AND initial.distilled_json IS NOT NULL AND initial.phase<>'compare'
              AND NOT (initial.recall_manifest ? 'backfill') AND initial_run.method_version=$7
            ORDER BY initial.created_at,initial.task_ref LIMIT 1
          )source ON true
          WHERE unit.domain_ref=$1
            AND NOT EXISTS(SELECT 1 FROM linggan_topic_map_unit_resolution compared
              WHERE compared.unit_ref=unit.unit_ref AND (
                compared.candidate_manifest @> jsonb_build_object('topics',jsonb_build_array(jsonb_build_object('definitionRef',$2::uuid)))
                OR EXISTS(SELECT 1 FROM linggan_topic_map_unit_assignment a WHERE a.resolution_ref=compared.resolution_ref AND a.definition_ref=$2)))
        )
        SELECT * FROM candidates WHERE dependent OR lower(statement) LIKE ANY($5)
        ORDER BY dependent DESC,source_created DESC,unit_key LIMIT $6
    "#).bind(domain).bind(definition_ref).bind(topic).bind(changed).bind(&terms)
        .bind((DISCUSSION_LIMIT+1) as i64).bind(crate::topic_map_research_analysis::METHOD_VERSION).fetch_all(&mut **tx).await?;
    let limited = rows.len() > DISCUSSION_LIMIT;
    let units: Vec<_> = rows
        .into_iter()
        .take(DISCUSSION_LIMIT)
        .map(|row| UnitCandidate {
            task: row.get("task_ref"),
            work: row.get("work_public_ref"),
            input_hash: row.get("input_hash"),
            key: row.get("unit_key"),
            statement: row.get("statement"),
            dependent: row.get("dependent"),
            created: row.get("source_created"),
        })
        .collect();
    let discussion_count = units.len();
    let (candidates, window_count) = select_candidates(&query, units);
    let manifest = json!({"method":"definition-dependency+bounded-cjk-bm25","definitionRef":definition_ref,
        "discussionLimit":DISCUSSION_LIMIT,"taskLimit":TASK_LIMIT,"candidateDiscussions":discussion_count,
        "candidateWindows":window_count,"queuedWindows":candidates.len(),
        "state":if limited || window_count>TASK_LIMIT { "bounded_partial" } else { "scanned" },
        "similarityIsMembership":false,"sourceValidation":"worker_restore_window","newRun":false});
    sqlx::query("INSERT INTO linggan_topic_map_definition_scan(definition_ref,domain_ref,topic_ref,reason,recall_manifest)VALUES($1,$2,$3,$4,$5)ON CONFLICT DO NOTHING")
        .bind(definition_ref).bind(domain).bind(topic).bind(reason).bind(manifest).execute(&mut **tx).await?;
    for candidate in candidates {
        sqlx::query("INSERT INTO linggan_topic_map_backfill_queue(definition_ref,source_task_ref,unit_keys,reason)VALUES($1,$2,$3,$4)ON CONFLICT DO NOTHING")
            .bind(definition_ref).bind(candidate.task).bind(candidate.keys.into_iter().collect::<Vec<_>>())
            .bind(if candidate.dependent { "definition_changed" } else { "new_topic_recall" }).execute(&mut **tx).await?;
    }
    Ok(true)
}

async fn activate_one(tx: &mut Transaction<'_, Postgres>) -> Result<bool, ModelError> {
    let row = sqlx::query(r#"
        SELECT queue.definition_ref,queue.source_task_ref,queue.unit_keys,queue.reason,
               source.run_ref,source.domain_ref,source.work_public_ref,source.input_hash,
               source.input_refs,source.distilled_json,latest.resolutions_json
        FROM linggan_topic_map_backfill_queue queue
        JOIN linggan_topic_map_research_task source ON source.task_ref=queue.source_task_ref
        JOIN linggan_topic_map_research_run run ON run.run_ref=source.run_ref
        JOIN LATERAL(SELECT completed.resolutions_json FROM linggan_topic_map_research_task completed
          WHERE completed.domain_ref=source.domain_ref AND completed.work_public_ref=source.work_public_ref
            AND completed.input_hash=source.input_hash AND completed.state='succeeded'
          ORDER BY completed.updated_at DESC,completed.task_ref DESC LIMIT 1)latest ON true
        JOIN linggan_topic_map_research_policy policy ON policy.domain_ref=run.domain_ref
        JOIN observation_domain domain ON domain.domain_ref=run.domain_ref
        JOIN linggan_topic_definition definition ON definition.definition_ref=queue.definition_ref
        WHERE queue.state='pending' AND source.state='succeeded' AND source.distilled_json IS NOT NULL
          AND NOT (source.recall_manifest ? 'backfill') AND run.method_version=$1
          AND (SELECT count(*) FROM linggan_topic_map_research_task maintenance JOIN linggan_topic_map_research_run owner ON owner.run_ref=maintenance.run_ref WHERE maintenance.domain_ref=source.domain_ref AND maintenance.recall_manifest ? 'backfill' AND maintenance.state IN ('queued','running') AND owner.state IN ('queued','running','daily_budget_paused'))<$2
          AND run.state IN ('queued','running','completed') AND policy.status='active' AND domain.status='active'
          AND (policy.automatic_enabled
            OR (run.trigger='on_demand' AND run.state IN ('queued','running')
                AND COALESCE(run.input_scope->>'backfillReopened','false')<>'true' AND definition.created_at>=run.created_at)
            OR EXISTS(SELECT 1 FROM jsonb_array_elements(COALESCE(run.input_scope->'backfillAuthorizations','[]'::jsonb)) permission
                WHERE permission->'workRefs' ? source.work_public_ref::text AND permission->'definitionRefs' ? queue.definition_ref::text))
          AND NOT EXISTS(SELECT 1 FROM linggan_topic_map_research_task active
            WHERE active.domain_ref=source.domain_ref AND active.work_public_ref=source.work_public_ref
              AND active.input_hash=source.input_hash AND active.state IN ('queued','running'))
          AND (queue.reason<>'new_topic_recall'
            OR (SELECT count(*) FROM linggan_topic_map_backfill_queue pending WHERE pending.source_task_ref=source.task_ref AND pending.state='pending')>=$4
            OR (SELECT min(pending.created_at) FROM linggan_topic_map_backfill_queue pending WHERE pending.source_task_ref=source.task_ref AND pending.state='pending')<=scope_001_now()-($3::integer*interval '1 second')
            OR EXISTS(SELECT 1 FROM jsonb_array_elements(COALESCE(run.input_scope->'backfillAuthorizations','[]'::jsonb)) permission
                WHERE permission->'workRefs' ? source.work_public_ref::text AND permission->'definitionRefs' ? queue.definition_ref::text)
            OR NOT EXISTS(SELECT 1 FROM linggan_topic_map_research_task initial
                JOIN linggan_topic_map_research_run initial_run ON initial_run.run_ref=initial.run_ref
                WHERE initial.domain_ref=source.domain_ref AND initial.state IN ('queued','running')
                  AND initial.phase<>'compare' AND NOT (initial.recall_manifest ? 'backfill')
                  AND initial_run.method_version=$1 AND initial_run.state IN ('queued','running')))

        ORDER BY queue.created_at,queue.definition_ref,queue.source_task_ref LIMIT 1 FOR UPDATE OF queue SKIP LOCKED
    "#).bind(crate::topic_map_research_analysis::METHOD_VERSION).bind(ACTIVE_TASK_LIMIT).bind(RECALL_DEBOUNCE_SECONDS).bind(DEFINITION_BATCH_LIMIT as i64).fetch_optional(&mut **tx).await?;
    let Some(row) = row else {
        return Ok(false);
    };
    let definition: Uuid = row.get("definition_ref");
    let source: Uuid = row.get("source_task_ref");
    let domain: Uuid = row.get("domain_ref");
    let work: Uuid = row.get("work_public_ref");
    let run: Uuid = row.get("run_ref");
    let input_hash: String = row.get("input_hash");
    // The same lock order used by normal dispatch serializes pause/stop, budget
    // ownership and the partial active-input uniqueness check.
    sqlx::query(
        "SELECT domain_ref FROM linggan_topic_map_research_policy WHERE domain_ref=$1 FOR UPDATE",
    )
    .bind(domain)
    .fetch_one(&mut **tx)
    .await?;
    let permission = sqlx::query("SELECT r.input_scope,r.state IN ('queued','running','completed') AND p.status='active' AND d.status='active' AS enabled,p.automatic_enabled,(r.trigger='on_demand' AND r.state IN ('queued','running') AND COALESCE(r.input_scope->>'backfillReopened','false')<>'true' AND definition.created_at>=r.created_at) AS live_on_demand FROM linggan_topic_map_research_run r JOIN linggan_topic_map_research_policy p USING(domain_ref) JOIN observation_domain d USING(domain_ref) JOIN linggan_topic_definition definition ON definition.definition_ref=$2 WHERE r.run_ref=$1 FOR UPDATE OF r")
        .bind(run).bind(definition).fetch_one(&mut **tx).await?;
    let authorization = authorization::matching_authorization(
        &permission.get::<Value, _>("input_scope"),
        work,
        &[definition],
    );
    let enabled = permission.get::<bool, _>("enabled")
        && (permission.get::<bool, _>("automatic_enabled")
            || permission.get::<bool, _>("live_on_demand")
            || authorization.is_some());
    if !enabled {
        return Ok(false);
    }
    let batch = coalesce_pending(tx, &permission, &row, authorization).await?;
    if batch.definitions.is_empty() {
        return Ok(true);
    }
    let definitions = batch.definitions;
    let keys: Vec<_> = batch.keys.into_iter().collect();
    let busy: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_topic_map_research_task WHERE domain_ref=$1 AND work_public_ref=$2 AND input_hash=$3 AND state IN ('queued','running'))")
        .bind(domain).bind(work).bind(&input_hash).fetch_one(&mut **tx).await?;
    if busy {
        return Ok(false);
    }
    let task = Uuid::new_v4();
    let previous = keep_other_resolutions(&row.get::<Value, _>("resolutions_json"), &keys);
    let scope = json!({"backfill":{"unitKeys":keys,"forcedDefinitionRefs":definitions,
        "definitionLimit":DEFINITION_BATCH_LIMIT,"coalesced":true,
        "sourceTaskRef":source,"reason":row.get::<String,_>("reason"),"authorizationRequestRef":authorization}});
    let inserted = sqlx::query("INSERT INTO linggan_topic_map_research_task(task_ref,run_ref,domain_ref,work_public_ref,input_hash,input_refs,phase,distilled_json,resolutions_json,recall_manifest)VALUES($1,$2,$3,$4,$5,$6,'resolve',$7,$8,$9)ON CONFLICT DO NOTHING")
        .bind(task).bind(run).bind(domain).bind(work).bind(&input_hash).bind(row.get::<Value,_>("input_refs"))
        .bind(row.get::<Option<Value>,_>("distilled_json")).bind(previous).bind(scope).execute(&mut **tx).await?.rows_affected()>0;
    if !inserted {
        return Ok(false);
    }
    sqlx::query("UPDATE linggan_topic_map_backfill_queue SET state='active',queued_task_ref=$3,last_reason=NULL,updated_at=scope_001_now() WHERE definition_ref=ANY($1) AND source_task_ref=$2 AND state='pending'")
        .bind(&definitions).bind(source).bind(task).execute(&mut **tx).await?;
    sqlx::query("UPDATE linggan_topic_map_research_run SET state='queued',input_scope=jsonb_set(input_scope,'{backfillReopened}','true'::jsonb),last_reason='definition_reassignment',updated_at=scope_001_now() WHERE run_ref=$1 AND state='completed'")
        .bind(run).execute(&mut **tx).await?;
    Ok(true)
}

struct ReassessmentBatch {
    definitions: Vec<Uuid>,
    keys: BTreeSet<String>,
}

async fn coalesce_pending(
    tx: &mut Transaction<'_, Postgres>,
    permission: &sqlx::postgres::PgRow,
    source: &sqlx::postgres::PgRow,
    authorization: Option<Uuid>,
) -> Result<ReassessmentBatch, ModelError> {
    let anchor: Uuid = source.get("source_task_ref");
    let domain: Uuid = source.get("domain_ref");
    let work: Uuid = source.get("work_public_ref");
    let rows = sqlx::query("SELECT q.definition_ref,q.unit_keys,definition.created_at>=run.created_at AS within_live_run FROM linggan_topic_map_backfill_queue q JOIN linggan_topic_definition definition USING(definition_ref) JOIN linggan_topic_map_research_task task ON task.task_ref=q.source_task_ref JOIN linggan_topic_map_research_run run USING(run_ref) WHERE q.source_task_ref=$1 AND q.state='pending' ORDER BY (q.definition_ref=$3) DESC,q.created_at,q.definition_ref LIMIT $2 FOR UPDATE OF q")
        .bind(anchor).bind(DEFINITION_BATCH_LIMIT as i64).bind(source.get::<Uuid,_>("definition_ref")).fetch_all(&mut **tx).await?;
    let mut batch = ReassessmentBatch {
        definitions: Vec::new(),
        keys: BTreeSet::new(),
    };
    for pending in rows {
        let definition: Uuid = pending.get("definition_ref");
        let mut definitions = batch.definitions.clone();
        definitions.push(definition);
        let automatic = permission.get::<bool, _>("automatic_enabled");
        let live = permission.get::<bool, _>("live_on_demand")
            && pending.get::<bool, _>("within_live_run");
        let explicit = authorization.is_some()
            && authorization::matching_authorization(
                &permission.get::<Value, _>("input_scope"),
                work,
                &definitions,
            ) == authorization;
        if !automatic && !live && !explicit {
            continue;
        }
        if let Some(reason) = reassignment_blocker(
            tx,
            domain,
            work,
            definition,
            &source.get::<String, _>("input_hash"),
        )
        .await?
        {
            skip(tx, definition, anchor, reason).await?;
            continue;
        }
        let keys = uncompared_keys(tx, domain, work, definition, pending.get("unit_keys")).await?;
        if keys.is_empty() {
            skip(tx, definition, anchor, "definition_already_compared").await?;
            continue;
        }
        batch.keys.extend(keys);
        batch.definitions = definitions;
    }
    Ok(batch)
}

async fn uncompared_keys(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    work: Uuid,
    definition: Uuid,
    pending_keys: Vec<String>,
) -> Result<Vec<String>, ModelError> {
    Ok(sqlx::query_scalar(r#"
            SELECT unit.unit_key FROM linggan_topic_map_discussion_unit unit
            WHERE unit.domain_ref=$1 AND unit.work_public_ref=$2 AND unit.unit_key=ANY($3)
              AND NOT EXISTS(SELECT 1 FROM linggan_topic_map_unit_resolution compared WHERE compared.unit_ref=unit.unit_ref AND (
                compared.candidate_manifest @> jsonb_build_object('topics',jsonb_build_array(jsonb_build_object('definitionRef',$4::uuid)))
                OR EXISTS(SELECT 1 FROM linggan_topic_map_unit_assignment a WHERE a.resolution_ref=compared.resolution_ref AND a.definition_ref=$4)))
            ORDER BY unit.unit_key
        "#).bind(domain).bind(work).bind(&pending_keys).bind(definition).fetch_all(&mut **tx).await?)
}

async fn reassignment_blocker(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    work: Uuid,
    definition: Uuid,
    input_hash: &str,
) -> Result<Option<&'static str>, ModelError> {
    let current: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_topic_definition d JOIN LATERAL(SELECT domain_ref FROM linggan_topic_map_binding WHERE topic_ref=d.topic_ref ORDER BY version DESC LIMIT 1)b ON true WHERE d.definition_ref=$1 AND d.version=(SELECT max(version)FROM linggan_topic_definition WHERE topic_ref=d.topic_ref) AND b.domain_ref=$2 AND NOT EXISTS(SELECT 1 FROM linggan_topic_map_structure_source WHERE topic_ref=d.topic_ref) AND NOT EXISTS(SELECT 1 FROM linggan_topic_map_concept_rule rule WHERE rule.definition_ref=d.definition_ref AND rule.method_version='topic-map.core.parent.v1'))")
        .bind(definition).bind(domain).fetch_one(&mut **tx).await?;
    let unknown: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_topic_map_research_task WHERE domain_ref=$1 AND work_public_ref=$2 AND input_hash=$3 AND state='unknown_dispatch')")
        .bind(domain).bind(work).bind(input_hash).fetch_one(&mut **tx).await?;
    Ok(if unknown {
        Some("unknown_dispatch")
    } else if !current {
        Some("definition_superseded")
    } else {
        None
    })
}

async fn skip(
    tx: &mut Transaction<'_, Postgres>,
    definition: Uuid,
    source: Uuid,
    reason: &str,
) -> Result<(), ModelError> {
    sqlx::query("UPDATE linggan_topic_map_backfill_queue SET state='skipped',last_reason=$3,updated_at=scope_001_now() WHERE definition_ref=$1 AND source_task_ref=$2 AND state='pending'")
        .bind(definition).bind(source).bind(reason).execute(&mut **tx).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_recall_merges_same_window_and_prioritizes_actual_dependencies() {
        let mut units: Vec<_> = (1..=30)
            .map(|index| UnitCandidate {
                task: Uuid::from_u128(index),
                work: Uuid::from_u128(index),
                input_hash: format!("source-{index}"),
                key: format!("unit-{index}"),
                statement: "开始任务之前无法行动的启动困难".into(),
                dependent: false,
                created: index as f64,
            })
            .collect();
        units.push(UnitCandidate {
            task: Uuid::from_u128(1),
            work: Uuid::from_u128(1),
            input_hash: "source-1".into(),
            key: "another-unit".into(),
            statement: "旧定义实际比较过这一条".into(),
            dependent: true,
            created: 1.0,
        });
        let (selected, total) = select_candidates("启动困难，开始任务无法行动", units);
        assert_eq!(total, 30);
        assert_eq!(selected.len(), TASK_LIMIT);
        assert_eq!(selected[0].task, Uuid::from_u128(1));
        assert_eq!(selected[0].keys.len(), 2);
    }

    #[test]
    fn new_topic_recall_does_not_select_unrelated_statements() {
        let (selected, _) = select_candidates(
            "启动困难",
            vec![UnitCandidate {
                task: Uuid::from_u128(1),
                work: Uuid::from_u128(1),
                input_hash: "one".into(),
                key: "one".into(),
                statement: "午饭去什么餐厅".into(),
                dependent: false,
                created: 0.0,
            }],
        );
        assert!(selected.is_empty());
    }

    #[test]
    fn reassignment_keeps_other_units_in_the_next_complete_projection() {
        let previous = json!([{"unitId":"revisit","statement":"旧归属"},{"unitId":"keep","statement":"不相关讨论"}]);
        assert_eq!(
            keep_other_resolutions(&previous, &["revisit".into()]),
            json!([{"unitId":"keep","statement":"不相关讨论"}])
        );
    }
}
