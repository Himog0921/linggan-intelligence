//! Atomic application and recovery receipts for trusted Task B relation responses.
use super::super::{problem_metadata, problem_version, rejected, statement};
use super::{
    PROBLEM_RELATION_VERSION, ProblemRelation, apply_problem_relation, validate_problem_relations,
};
use linggan_storage_postgres::StorageError;
use serde_json::{Value, json};
use sqlx::{PgConnection, Row};
use uuid::Uuid;

/// Apply a trusted server-stored snapshot and its validated response atomically. This is
/// deliberately separate from Task A persistence so a Task B failure cannot erase extraction.
pub async fn apply_problem_relation_output(
    database: &linggan_storage_postgres::Database,
    task: &Value,
    output: &Value,
) -> Result<Value, StorageError> {
    apply_relation_output(database, task, output, None).await
}

/// The durable worker reads its own snapshot; the acceptance receipt commits with every
/// relationship and optional emerging object, before the usage ledger is finalized.
pub async fn apply_problem_task_output(
    database: &linggan_storage_postgres::Database,
    task_ref: Uuid,
    output: &Value,
) -> Result<Value, StorageError> {
    let row = sqlx::query(
        "SELECT task_snapshot,applied_receipt FROM linggan_ci_problem_task WHERE task_ref=$1",
    )
    .bind(task_ref)
    .fetch_one(database.pool())
    .await
    .map_err(statement)?;
    if let Some(receipt) = row.get::<Option<Value>, _>("applied_receipt") {
        return Ok(receipt);
    }
    let task = row
        .get::<Option<Value>, _>("task_snapshot")
        .ok_or_else(|| rejected("CI_CONTEXT_UNAVAILABLE"))?;
    apply_relation_output(database, &task, output, Some(task_ref)).await
}

async fn apply_relation_output(
    database: &linggan_storage_postgres::Database,
    task: &Value,
    output: &Value,
    task_ref: Option<Uuid>,
) -> Result<Value, StorageError> {
    let decisions = validate_problem_relations(task, output).map_err(rejected)?;
    let domain = task["domain"]
        .as_str()
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| rejected("CI_INVALID_COMMAND"))?;
    let candidate = task["expression"]["candidateRef"]
        .as_str()
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| rejected("CI_INVALID_COMMAND"))?;
    let guard = json!({"contextRefs":task["sourceGuard"]});
    if !crate::comment_daily_read::context_readable(database, &guard)
        .await
        .map_err(|_| rejected("CI_CONTEXT_UNAVAILABLE"))?
    {
        return Err(rejected("CI_CONTEXT_UNAVAILABLE"));
    }
    // Re-read target evidence qualification as it may have been restricted during inference.
    for decision in &decisions {
        super::super::problem_details(database, domain, decision.target_ref).await?;
    }
    let mut tx = database.pool().begin().await.map_err(statement)?;
    super::super::lock_domain(&mut tx, domain).await?;
    if let Some(task_ref) = task_ref {
        if let Some(receipt) = lock_task_acceptance(&mut tx, task_ref, task).await? {
            return Ok(receipt);
        }
    }
    let frozen:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_ci_problem_candidate WHERE candidate_ref=$1 AND domain_ref=$2 AND analysis_ref::text=$3 AND definition_key=$4 AND state='unmerged')")
        .bind(candidate).bind(domain).bind(task["expression"]["analysisRef"].as_str()).bind(task["expression"]["definitionKey"].as_str())
        .fetch_one(&mut *tx).await.map_err(statement)?;
    if !frozen {
        return Err(rejected("CI_CANDIDATE_CHANGED"));
    }
    for decision in &decisions {
        let relation = match decision.relation {
            ProblemRelation::Same => "same",
            ProblemRelation::Related => "related",
            ProblemRelation::Different => "different",
            ProblemRelation::Uncertain => "uncertain",
        };
        apply_problem_relation(
            &mut tx,
            domain,
            candidate,
            decision.target_ref,
            decision.definition_revision,
            relation,
            &decision.reason,
            "task_b",
            None,
        )
        .await?;
    }
    // A single uncertain edge keeps this expression in the deduplicated decision queue.
    if decisions
        .iter()
        .all(|d| d.relation != ProblemRelation::Same)
        && decisions
            .iter()
            .any(|d| d.relation == ProblemRelation::Uncertain)
    {
        sqlx::query(
            "UPDATE linggan_ci_problem_candidate SET state='unmerged' WHERE candidate_ref=$1",
        )
        .bind(candidate)
        .execute(&mut *tx)
        .await
        .map_err(statement)?;
    }
    let mut receipt = json!({"candidateRef":candidate,"decisions":decisions,"contractVersion":PROBLEM_RELATION_VERSION});
    if let Some(task_ref) = task_ref {
        if decisions.iter().all(|d| {
            !matches!(
                d.relation,
                ProblemRelation::Same | ProblemRelation::Uncertain
            )
        }) {
            receipt["emergingProblemRef"] =
                json!(create_emerging_problem(&mut tx, domain, candidate).await?);
        }
        sqlx::query("UPDATE linggan_ci_problem_task SET applied_receipt=$2,applied_at=scope_001_now() WHERE task_ref=$1")
            .bind(task_ref).bind(&receipt).execute(&mut *tx).await.map_err(statement)?;
    }
    tx.commit().await.map_err(statement)?;
    Ok(receipt)
}

async fn lock_task_acceptance(
    connection: &mut PgConnection,
    task_ref: Uuid,
    task: &Value,
) -> Result<Option<Value>, StorageError> {
    let row = sqlx::query("SELECT task_snapshot,applied_receipt,state FROM linggan_ci_problem_task WHERE task_ref=$1 FOR UPDATE")
        .bind(task_ref).fetch_one(&mut *connection).await.map_err(statement)?;
    if let Some(receipt) = row.get::<Option<Value>, _>("applied_receipt") {
        return Ok(Some(receipt));
    }
    if row.get::<String, _>("state") != "running"
        || row.get::<Option<Value>, _>("task_snapshot").as_ref() != Some(task)
    {
        return Err(rejected("CI_CANDIDATE_CHANGED"));
    }
    Ok(None)
}

/// Called after a qualified retrieval returned no eligible target, or Task B found no
/// equivalent target. A new object starts emerging; it is not a claim of a new market need.
pub async fn promote_unmatched_expression(
    database: &linggan_storage_postgres::Database,
    domain: Uuid,
    candidate: Uuid,
) -> Result<Uuid, StorageError> {
    let row=sqlx::query("SELECT c.name,c.meaning,c.definition_key,c.canonical_ref,c.analysis_ref,c.evidence,a.result FROM linggan_ci_problem_candidate c JOIN linggan_ci_source s USING(canonical_ref,domain_ref) JOIN linggan_comment_analysis_work a ON a.work_ref=c.analysis_ref AND a.result->>'sourceSha256'=s.source_sha256 WHERE c.candidate_ref=$1 AND c.domain_ref=$2 AND c.state IN('unmerged','resolved') AND NOT COALESCE((SELECT locked FROM linggan_ci_source_research h WHERE h.canonical_ref=c.canonical_ref AND h.domain_ref=c.domain_ref),false)")
        .bind(candidate).bind(domain).fetch_optional(database.pool()).await.map_err(statement)?.ok_or_else(||rejected("CI_CANDIDATE_UNAVAILABLE"))?;
    if !crate::comment_daily_read::context_readable(database, &row.get::<Value, _>("result"))
        .await
        .map_err(|_| rejected("CI_CONTEXT_UNAVAILABLE"))?
    {
        return Err(rejected("CI_CONTEXT_UNAVAILABLE"));
    }
    let mut tx = database.pool().begin().await.map_err(statement)?;
    super::super::lock_domain(&mut tx, domain).await?;
    let problem = create_emerging_problem(&mut tx, domain, candidate).await?;
    tx.commit().await.map_err(statement)?;
    Ok(problem)
}

async fn create_emerging_problem(
    connection: &mut PgConnection,
    domain: Uuid,
    candidate: Uuid,
) -> Result<Uuid, StorageError> {
    let row=sqlx::query("SELECT c.name,c.meaning,c.definition_key,c.canonical_ref,c.analysis_ref,c.evidence,a.result FROM linggan_ci_problem_candidate c JOIN linggan_ci_source s USING(canonical_ref,domain_ref) JOIN linggan_comment_analysis_work a ON a.work_ref=c.analysis_ref AND a.result->>'sourceSha256'=s.source_sha256 WHERE c.candidate_ref=$1 AND c.domain_ref=$2 AND c.state IN('unmerged','resolved') AND NOT COALESCE((SELECT locked FROM linggan_ci_source_research h WHERE h.canonical_ref=c.canonical_ref AND h.domain_ref=c.domain_ref),false)")
        .bind(candidate).bind(domain).fetch_optional(&mut *connection).await.map_err(statement)?.ok_or_else(||rejected("CI_CANDIDATE_UNAVAILABLE"))?;
    let problem = Uuid::new_v4();
    let definition = json!({"lifecycle":"emerging","lifecycleBasis":"single_analyzed_expression","expressionDefinitionKey":row.get::<String,_>("definition_key"),"ruleVersion":PROBLEM_RELATION_VERSION,"boundary":"由一条已分析表达建立，待后续证据补充；不表示现实中首次出现"});
    sqlx::query("INSERT INTO linggan_ci_problem(problem_ref,domain_ref,name,meaning,definition,origin) VALUES($1,$2,$3,$4,$5,'model_expression')")
        .bind(problem).bind(domain).bind(row.get::<String,_>("name")).bind(row.get::<String,_>("meaning")).bind(definition).execute(&mut *connection).await.map_err(statement)?;
    sqlx::query("INSERT INTO linggan_ci_problem_member(problem_ref,canonical_ref,origin,evidence,analysis_ref) VALUES($1,$2,'model_expression',$3,$4)")
        .bind(problem).bind(row.get::<Uuid,_>("canonical_ref")).bind(row.get::<Value,_>("evidence")).bind(row.get::<Uuid,_>("analysis_ref")).execute(&mut *connection).await.map_err(statement)?;
    sqlx::query("UPDATE linggan_ci_problem_candidate SET state='assigned',problem_ref=$2 WHERE candidate_ref=$1").bind(candidate).bind(problem).execute(&mut *connection).await.map_err(statement)?;
    let after = problem_metadata(connection, domain, problem).await?;
    problem_version(
        connection,
        problem,
        None,
        "emerging_expression",
        "未找到可接纳的既有问题，以新兴对象保留表达",
        &json!({}),
        &after,
    )
    .await?;
    Ok(problem)
}
