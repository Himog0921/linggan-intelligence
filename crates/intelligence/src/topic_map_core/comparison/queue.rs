use super::*;
use crate::{
    model_settings::ModelError, topic_map_research::ResearchError,
    topic_map_research_analysis::METHOD_VERSION,
};
use linggan_storage_postgres::Database;
use sqlx::{Executor, Postgres, Row, Transaction};

const CURRENT_COMPARISON_SQL: &str = "SELECT task_ref,state FROM linggan_topic_map_research_task WHERE domain_ref=$1 AND input_hash=$2 AND state IN ('queued','running','succeeded','no_signal','insufficient','unknown_dispatch') AND COALESCE(input_refs#>>'{source,coverage,kind}',input_refs#>>'{coverage,kind}')='comparison' ORDER BY created_at DESC,task_ref DESC LIMIT 1";

#[derive(PartialEq, Eq)]
struct QueueCandidate {
    run: Uuid,
    domain: Uuid,
    config: Uuid,
    trigger: String,
    dependency_fingerprint: String,
    source_dependencies: Value,
    definition_refs: Value,
    comparison_work: Option<Uuid>,
    requested: Option<Vec<Uuid>>,
}

impl QueueCandidate {
    fn from_row(row: &sqlx::postgres::PgRow) -> Self {
        let comparison_work = row.get::<Option<Uuid>, _>("comparison_work");
        Self {
            run: row.get("run_ref"),
            domain: row.get("domain_ref"),
            config: row.get("config_ref"),
            trigger: row.get("trigger"),
            dependency_fingerprint: row.get("dependency_fingerprint"),
            source_dependencies: row.get("source_dependencies"),
            definition_refs: row.get("definition_refs"),
            comparison_work,
            requested: comparison_work
                .map(|work| vec![work])
                .or_else(|| scope(&row.get::<Value, _>("input_scope"))),
        }
    }
}

struct ComparisonAdmission {
    input_scope: Value,
    authorization: Value,
}

struct ComparisonOutcome {
    summary: Value,
    reason: &'static str,
    queued: bool,
}

async fn load_comparison(
    db: &Database,
    inputs: &[ResearchInput],
    domain: Uuid,
    config: Uuid,
    requested: &[Uuid],
) -> Result<Result<ResearchInput, &'static str>, ModelError> {
    let rows = sqlx::query("SELECT * FROM (SELECT DISTINCT ON(t.work_public_ref,t.input_hash)t.task_ref,t.work_public_ref,t.input_refs,t.resolutions_json,t.updated_at FROM linggan_topic_map_research_task t JOIN linggan_topic_map_research_run r USING(run_ref) WHERE t.domain_ref=$1 AND t.work_public_ref=ANY($2) AND t.phase IN ('extract','resolve') AND t.state=ANY($3) AND r.config_ref=$4 AND r.method_version=$5 AND COALESCE(t.input_refs#>>'{source,coverage,kind}',t.input_refs#>>'{coverage,kind}','source')<>'comparison' AND jsonb_array_length(t.resolutions_json)>0 ORDER BY t.work_public_ref,t.input_hash,t.updated_at DESC,t.task_ref DESC) current_tasks ORDER BY updated_at DESC,task_ref DESC")
        .bind(domain).bind(requested).bind(TERMINAL.as_slice()).bind(config).bind(METHOD_VERSION).fetch_all(db.pool()).await?;
    let tasks: Vec<_> = rows
        .into_iter()
        .map(|r| TaskEvidence {
            task: r.get("task_ref"),
            work: r.get("work_public_ref"),
            manifest: r.get("input_refs"),
            resolutions: r.get("resolutions_json"),
        })
        .collect();
    Ok(build_comparison(
        inputs,
        &tasks,
        requested,
        &crate::topic_map_core::catalog(db, domain).await?,
    ))
}

/// Repeated explicit selections reuse the same current comparison, including an
/// ambiguous dispatch which must never be silently sent to the provider again.
pub(crate) async fn has_current_comparison(
    db: &Database,
    inputs: &[ResearchInput],
    domain: Uuid,
    config: Uuid,
    requested: &[Uuid],
) -> Result<bool, ModelError> {
    let input = match load_comparison(db, inputs, domain, config, requested).await? {
        Ok(input) => input,
        Err("comparison_requires_author_and_commenter_discussions") if requested.len() == 1 => {
            return Ok(true);
        }
        Err(_) => return Ok(false),
    };
    if sqlx::query(CURRENT_COMPARISON_SQL)
        .bind(domain)
        .bind(&input.hash)
        .fetch_optional(db.pool())
        .await?
        .is_some()
    {
        return Ok(true);
    }
    let mut tx = db.pool().begin().await?;
    let blocked = blocks_unknown_dispatch(&mut tx, domain, &input, inputs).await?;
    tx.commit().await?;
    Ok(blocked)
}

/// Adds at most one comparison to its original run and token budget. Automatic
/// work is limited to actual extraction participants of an enabled original run.
pub(crate) async fn queue_once(db: &Database) -> Result<bool, ModelError> {
    let row = sqlx::query(scheduling::CANDIDATE_SQL)
        .bind(METHOD_VERSION)
        .bind(Option::<Uuid>::None)
        .fetch_optional(db.pool())
        .await?;
    let Some(row) = row else {
        return Ok(false);
    };
    let candidate = QueueCandidate::from_row(&row);
    let mut inputs = Vec::new();
    let built = if let Some(requested) = &candidate.requested {
        match research::load_inputs_for_works(db, candidate.domain, candidate.config, requested)
            .await
        {
            Ok(loaded) => {
                let built =
                    load_comparison(db, &loaded, candidate.domain, candidate.config, requested)
                        .await?;
                inputs = loaded;
                built
            }
            Err(ResearchError::Database(error)) => return Err(ModelError::Database(error)),
            Err(_) => Err("comparison_source_unavailable"),
        }
    } else {
        Err("comparison_scope_invalid")
    };
    if built.as_ref().is_ok_and(|input| {
        input.coverage["scopeWorkRefs"] != json!(candidate.requested)
            || !captured_dependencies_match(
                input,
                &candidate.source_dependencies,
                &candidate.definition_refs,
            )
    }) || !snapshot_is_current(db.pool(), &candidate).await?
    {
        return Ok(false);
    }
    let mut tx = db.pool().begin().await?;
    let Some(admission) = lock_admission(&mut tx, &candidate).await? else {
        return Ok(false);
    };
    let Some(outcome) = enqueue_comparison(
        &mut tx,
        &candidate,
        built,
        &inputs,
        &admission.authorization,
    )
    .await?
    else {
        return Ok(false);
    };
    record_outcome(&mut tx, &candidate, admission.input_scope, outcome).await?;
    tx.commit().await?;
    Ok(true)
}

async fn lock_admission(
    tx: &mut Transaction<'_, Postgres>,
    candidate: &QueueCandidate,
) -> Result<Option<ComparisonAdmission>, ModelError> {
    let enabled = sqlx::query("SELECT p.status,p.automatic_enabled,d.status AS domain_status FROM linggan_topic_map_research_policy p JOIN observation_domain d USING(domain_ref) WHERE p.domain_ref=$1 FOR UPDATE OF p")
        .bind(candidate.domain).fetch_optional(&mut **tx).await?;
    let automatic = enabled
        .as_ref()
        .is_some_and(|row| row.get::<bool, _>("automatic_enabled"));
    if !enabled.is_some_and(|row| {
        row.get::<String, _>("status") == "active"
            && row.get::<String, _>("domain_status") == "active"
    }) {
        return Ok(None);
    }
    let current = sqlx::query("SELECT state,input_scope,trigger FROM linggan_topic_map_research_run WHERE run_ref=$1 FOR UPDATE")
        .bind(candidate.run).fetch_one(&mut **tx).await?;
    // The unlocked build must still have exactly the source and definition
    // snapshot which the captured grant and checkpoint describe.
    if !snapshot_is_current(&mut **tx, candidate).await? {
        return Ok(None);
    }
    let input_scope: Value = current.get("input_scope");
    if !input_scope.is_object()
        || !["queued", "running", "completed"].contains(&current.get::<String, _>("state").as_str())
        || current.get::<String, _>("trigger") != candidate.trigger
    {
        return Ok(None);
    }
    let previous = if let Some(work) = candidate.comparison_work {
        &input_scope["workComparisons"][work.to_string()]
    } else {
        &input_scope["comparison"]
    };
    let current_on_demand = scheduling::current_on_demand(
        &current.get::<String, _>("trigger"),
        &current.get::<String, _>("state"),
        &input_scope,
        previous,
    );
    if previous["dependencyFingerprint"].as_str() == Some(&candidate.dependency_fingerprint)
        || !scheduling::may_refresh(
            &input_scope,
            previous,
            &candidate.source_dependencies,
            candidate.requested.as_deref().unwrap_or(&[]),
            automatic,
            current_on_demand,
        )
        || (candidate.comparison_work.is_none() && scope(&input_scope) != candidate.requested)
    {
        return Ok(None);
    }
    let authorization = scheduling::renewal_authorization(
        &input_scope,
        previous,
        &candidate.source_dependencies,
        candidate.requested.as_deref().unwrap_or(&[]),
    )
    .unwrap_or_else(|| json!({}));
    if !sources_are_idle(tx, candidate).await? {
        return Ok(None);
    }
    Ok(Some(ComparisonAdmission {
        input_scope,
        authorization,
    }))
}

async fn snapshot_is_current<'e, E>(
    executor: E,
    candidate: &QueueCandidate,
) -> Result<bool, ModelError>
where
    E: Executor<'e, Database = Postgres>,
{
    Ok(sqlx::query(scheduling::CANDIDATE_SQL)
        .bind(METHOD_VERSION)
        .bind(Some(candidate.run))
        .fetch_optional(executor)
        .await?
        .is_some_and(|row| QueueCandidate::from_row(&row) == *candidate))
}

pub(super) fn captured_dependencies_match(
    input: &ResearchInput,
    dependencies: &Value,
    definitions: &Value,
) -> bool {
    let Some(discussions) = input.coverage["selectedDiscussions"].as_array() else {
        return false;
    };
    let Some(dependencies) = dependencies.as_array() else {
        return false;
    };
    let Some(definitions) = definitions.as_array() else {
        return false;
    };
    discussions.iter().all(|discussion| {
        dependencies.iter().any(|dependency| {
            discussion["taskRef"] == dependency["taskRef"]
                && discussion["workRef"] == dependency["workRef"]
        })
    }) && crate::topic_map_core::definition_dependencies(&input.coverage["selectedDiscussions"])
        .iter()
        .all(|definition| definitions.contains(&json!(definition)))
}

async fn sources_are_idle(
    tx: &mut Transaction<'_, Postgres>,
    candidate: &QueueCandidate,
) -> Result<bool, ModelError> {
    if let Some(work) = candidate.comparison_work {
        let source_member: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_topic_map_research_task WHERE run_ref=$1 AND work_public_ref=$2 AND phase IN ('extract','resolve') AND COALESCE(input_refs#>>'{source,coverage,kind}',input_refs#>>'{coverage,kind}','source')<>'comparison')")
            .bind(candidate.run).bind(work).fetch_one(&mut **tx).await?;
        if !source_member {
            return Ok(false);
        }
    }
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_topic_map_research_task WHERE run_ref=$1 AND ($2::uuid IS NULL OR work_public_ref=$2) AND state IN ('queued','running') AND (phase='compare' OR COALESCE(input_refs#>>'{source,coverage,kind}',input_refs#>>'{coverage,kind}')='comparison'))")
        .bind(candidate.run).bind(candidate.comparison_work).fetch_one(&mut **tx).await?;
    let active: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_topic_map_research_task WHERE domain_ref=$1 AND work_public_ref=ANY($2) AND phase IN ('extract','resolve') AND state IN ('queued','running') AND COALESCE(input_refs#>>'{source,coverage,kind}',input_refs#>>'{coverage,kind}','source')<>'comparison')")
        .bind(candidate.domain).bind(candidate.requested.as_deref().unwrap_or(&[])).fetch_one(&mut **tx).await?;
    Ok(!exists && !active)
}

async fn enqueue_comparison(
    tx: &mut Transaction<'_, Postgres>,
    candidate: &QueueCandidate,
    built: Result<ResearchInput, &'static str>,
    inputs: &[ResearchInput],
    authorization: &Value,
) -> Result<Option<ComparisonOutcome>, ModelError> {
    let (summary, reason, queued) = match built {
        Ok(input) => {
            if blocks_unknown_dispatch(tx, candidate.domain, &input, inputs).await? {
                (
                    json!({"state":"unavailable","reason":"unknown_dispatch","taskState":"unknown_dispatch",
                    "scopeWorkRefs":input.coverage["scopeWorkRefs"]}),
                    "unknown_dispatch",
                    false,
                )
            } else if let Some(existing) = sqlx::query(CURRENT_COMPARISON_SQL)
                .bind(candidate.domain)
                .bind(&input.hash)
                .fetch_optional(&mut **tx)
                .await?
            {
                (
                    json!({"state":"current","reason":"no_new_input","existingTaskRef":existing.get::<Uuid,_>("task_ref"),
                    "taskState":existing.get::<String,_>("state"),"scopeWorkRefs":input.coverage["scopeWorkRefs"],
                    "availableWorkRefs":input.coverage["availableWorkRefs"],"selectedWorkRefs":input.coverage["selectedWorkRefs"]}),
                    "no_new_input",
                    false,
                )
            } else {
                let task = Uuid::new_v4();
                let inserted = sqlx::query("INSERT INTO linggan_topic_map_research_task(task_ref,run_ref,domain_ref,work_public_ref,input_hash,input_refs,phase,recall_manifest)VALUES($1,$2,$3,$4,$5,$6,'compare',$7)ON CONFLICT DO NOTHING")
                    .bind(task).bind(candidate.run).bind(candidate.domain).bind(input.work.work_ref).bind(&input.hash)
                    .bind(research::reference_manifest(&input)).bind(authorization).execute(&mut **tx).await?;
                if inserted.rows_affected() == 0 {
                    return Ok(None);
                }
                (
                    json!({"state":"queued","taskRef":task,"scopeWorkRefs":input.coverage["scopeWorkRefs"],
                    "availableWorkRefs":input.coverage["availableWorkRefs"],"selectedWorkRefs":input.coverage["selectedWorkRefs"],
                    "availableDiscussionCount":input.coverage["availableDiscussionCount"],"selectedDiscussionCount":input.coverage["selectedDiscussionCount"],
                    "sourceChars":input.coverage["sourceChars"],"selectedChars":input.coverage["coveredChars"],"contextChars":input.coverage["contextChars"]}),
                    "comparison_queued",
                    true,
                )
            }
        }
        Err(reason) => (
            json!({"state":"unavailable","reason":reason,"scopeWorkRefs":candidate.requested}),
            reason,
            false,
        ),
    };
    Ok(Some(ComparisonOutcome {
        summary,
        reason,
        queued,
    }))
}

async fn record_outcome(
    tx: &mut Transaction<'_, Postgres>,
    candidate: &QueueCandidate,
    mut input_scope: Value,
    mut outcome: ComparisonOutcome,
) -> Result<(), ModelError> {
    outcome.summary["dependencyFingerprint"] = json!(candidate.dependency_fingerprint);
    outcome.summary["sourceTaskRefs"] = json!(
        candidate
            .source_dependencies
            .as_array()
            .into_iter()
            .flatten()
            .map(|dependency| dependency["taskRef"].clone())
            .collect::<Vec<_>>()
    );
    if let Some(work) = candidate.comparison_work {
        if !input_scope["workComparisons"].is_object() {
            input_scope["workComparisons"] = json!({});
        }
        input_scope["workComparisons"][work.to_string()] = outcome.summary;
    } else {
        input_scope["comparison"] = outcome.summary;
    }
    sqlx::query("UPDATE linggan_topic_map_research_run SET state=CASE WHEN $4 AND state='completed' THEN 'queued' ELSE state END,input_scope=$2,last_reason=$3,updated_at=scope_001_now()WHERE run_ref=$1")
        .bind(candidate.run).bind(input_scope).bind(outcome.reason).bind(outcome.queued).execute(&mut **tx).await?;
    Ok(())
}

/// Called while lifecycle holds this run's policy/run locks. A different run's
/// pending maintenance must not keep this run alive or make it finish too early.
pub(crate) async fn pending_in(
    tx: &mut Transaction<'_, Postgres>,
    run: Uuid,
) -> Result<bool, ModelError> {
    let row = sqlx::query(scheduling::CANDIDATE_SQL)
        .bind(METHOD_VERSION)
        .bind(Some(run))
        .fetch_optional(&mut **tx)
        .await?;
    let Some(row) = row else {
        return Ok(false);
    };
    let input_scope: Value = row.get("input_scope");
    let previous: Value = row.get("previous");
    let requested = row
        .get::<Option<Uuid>, _>("comparison_work")
        .map(|work| vec![work])
        .or_else(|| scope(&input_scope));
    let Some(requested) = requested else {
        return Ok(false);
    };
    Ok(scheduling::may_refresh(
        &input_scope,
        &previous,
        &row.get("source_dependencies"),
        &requested,
        row.get("automatic_enabled"),
        scheduling::current_on_demand(
            &row.get::<String, _>("trigger"),
            &row.get::<String, _>("state"),
            &input_scope,
            &previous,
        ),
    ))
}
