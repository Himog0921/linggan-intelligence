//! Transactional stable-problem commands and append-only definition history.

use super::{
    Action, Bookmark, Merge, NewProblem, Rename, canonical_source, parse, record_source, rejected,
    source_snapshot, statement, text_ok,
};
use linggan_storage_postgres::StorageError;
use serde_json::{Value, json};
use sqlx::{PgConnection, Row};
use uuid::Uuid;

pub(crate) async fn problem_snapshot(
    connection: &mut PgConnection,
    domain: Uuid,
    problem: Uuid,
) -> Result<Value, StorageError> {
    let mut value = problem_metadata(connection, domain, problem).await?;
    let members = sqlx::query_scalar::<_, Uuid>("SELECT canonical_ref FROM linggan_ci_problem_member WHERE problem_ref=$1 ORDER BY canonical_ref")
        .bind(problem).fetch_all(&mut *connection).await.map_err(statement)?;
    value["sourceRefs"] = json!(members);
    Ok(value)
}

pub(crate) async fn problem_metadata(
    connection: &mut PgConnection,
    domain: Uuid,
    problem: Uuid,
) -> Result<Value, StorageError> {
    let row = sqlx::query("SELECT name,meaning,definition,revision,definition_revision,redirect_ref,bookmarked,origin FROM linggan_ci_problem WHERE problem_ref=$1 AND domain_ref=$2")
        .bind(problem).bind(domain).fetch_optional(&mut *connection).await.map_err(statement)?
        .ok_or_else(|| rejected("CI_PROBLEM_UNAVAILABLE"))?;
    Ok(
        json!({"problemRef":problem,"name":row.get::<String,_>("name"),"meaning":row.get::<String,_>("meaning"),"definition":row.get::<Value,_>("definition"),
        "revision":row.get::<i64,_>("revision"),"definitionRevision":row.get::<i64,_>("definition_revision"),"redirectRef":row.get::<Option<Uuid>,_>("redirect_ref"),"bookmarked":row.get::<bool,_>("bookmarked"),"origin":row.get::<String,_>("origin")}),
    )
}

pub(crate) async fn problem_version(
    connection: &mut PgConnection,
    problem: Uuid,
    command: Option<Uuid>,
    kind: &str,
    reason: &str,
    before: &Value,
    after: &Value,
) -> Result<(), StorageError> {
    sqlx::query("INSERT INTO linggan_ci_problem_version(problem_ref,revision,command_ref,kind,before_value,after_value,reason) VALUES($1,$2,$3,$4,$5,$6,$7)")
        .bind(problem).bind(after["revision"].as_i64().ok_or_else(|| rejected("CI_INVALID_STATE"))?).bind(command).bind(kind)
        .bind(before).bind(after).bind(reason).execute(&mut *connection).await.map_err(statement)?;
    Ok(())
}

pub(super) async fn live_problem(
    connection: &mut PgConnection,
    domain: Uuid,
    problem: Uuid,
) -> Result<Uuid, StorageError> {
    let mut current = problem;
    let mut visited = std::collections::BTreeSet::new();
    for _ in 0..64 {
        if !visited.insert(current) {
            return Err(rejected("CI_PROBLEM_ALIAS_CYCLE"));
        }
        let snapshot = problem_snapshot(connection, domain, current).await?;
        match snapshot["redirectRef"].as_str() {
            Some(value) => {
                current = Uuid::parse_str(value).map_err(|_| rejected("CI_INVALID_STATE"))?
            }
            None => return Ok(current),
        }
    }
    Err(rejected("CI_PROBLEM_ALIAS_CYCLE"))
}

pub(super) async fn problem_action(
    connection: &mut PgConnection,
    action: &Action,
) -> Result<Value, StorageError> {
    if action.kind == "candidate_resolve" {
        return resolve_candidate(connection, action).await;
    }
    if action.kind == "problem_create" {
        if action.expected_revision != 0 {
            return Err(rejected("CI_REVISION_CONFLICT"));
        }
        let payload: NewProblem = parse(&action.payload)?;
        let problem = action.problem_ref.unwrap_or(action.command_ref);
        return create_problem(connection, action, problem, payload, None).await;
    }
    let problem = action
        .problem_ref
        .ok_or_else(|| rejected("CI_INVALID_COMMAND"))?;
    let before = problem_snapshot(connection, action.domain, problem).await?;
    if before["revision"].as_i64() != Some(action.expected_revision) {
        return Err(rejected("CI_REVISION_CONFLICT"));
    }
    if !before["redirectRef"].is_null() {
        return Err(rejected("CI_PROBLEM_REDIRECTED"));
    }
    let mut other_result = None;
    match action.kind.as_str() {
        "problem_rename" => {
            let payload: Rename = parse(&action.payload)?;
            if !text_ok(&payload.name, 120) || !text_ok(&payload.meaning, 1000) {
                return Err(rejected("CI_INVALID_COMMAND"));
            }
            sqlx::query("UPDATE linggan_ci_problem SET name=$2,meaning=$3,definition=definition || jsonb_build_object('humanEdited',true),revision=revision+1,definition_revision=definition_revision+1,updated_at=scope_001_now() WHERE problem_ref=$1")
                .bind(problem).bind(payload.name.trim()).bind(payload.meaning.trim()).execute(&mut *connection).await.map_err(statement)?;
        }
        "problem_bookmark" => {
            let payload: Bookmark = parse(&action.payload)?;
            sqlx::query("UPDATE linggan_ci_problem SET bookmarked=$2,revision=revision+1,updated_at=scope_001_now() WHERE problem_ref=$1")
                .bind(problem).bind(payload.enabled).execute(&mut *connection).await.map_err(statement)?;
        }
        "problem_split" => {
            if action.reason.trim().is_empty() {
                return Err(rejected("CI_REASON_REQUIRED"));
            }
            let payload: NewProblem = parse(&action.payload)?;
            let new = create_problem(
                connection,
                action,
                action.command_ref,
                payload,
                Some(problem),
            )
            .await?;
            sqlx::query("UPDATE linggan_ci_problem SET revision=revision+1,definition_revision=definition_revision+1,updated_at=scope_001_now() WHERE problem_ref=$1")
                .bind(problem).execute(&mut *connection).await.map_err(statement)?;
            other_result = Some(new);
        }
        "problem_merge" => {
            other_result = Some(merge_problem(connection, action, problem, &before).await?);
        }
        _ => return Err(rejected("CI_INVALID_COMMAND")),
    }
    let mut after = problem_snapshot(connection, action.domain, problem).await?;
    problem_version(
        connection,
        problem,
        Some(action.command_ref),
        &action.kind,
        &action.reason,
        &before,
        &after,
    )
    .await?;
    if let Some(other) = other_result {
        after["relatedProblem"] = other;
    }
    Ok(after)
}

async fn merge_problem(
    connection: &mut PgConnection,
    action: &Action,
    problem: Uuid,
    before: &Value,
) -> Result<Value, StorageError> {
    if action.reason.trim().is_empty() {
        return Err(rejected("CI_REASON_REQUIRED"));
    }
    let payload: Merge = parse(&action.payload)?;
    let target = live_problem(connection, action.domain, payload.target_ref).await?;
    if target == problem {
        return Err(rejected("CI_PROBLEM_ALIAS_CYCLE"));
    }
    let target_before = problem_snapshot(connection, action.domain, target).await?;
    let sources: Vec<Uuid> = parse(&before["sourceRefs"])?;
    for source in sources {
        canonical_source(connection, action.domain, source).await?;
        let source_before = source_snapshot(connection, action.domain, source).await?;
        sqlx::query("INSERT INTO linggan_ci_problem_member(problem_ref,canonical_ref,origin,evidence,analysis_ref) SELECT $1,canonical_ref,'manual',evidence,analysis_ref FROM linggan_ci_problem_member WHERE problem_ref=$2 AND canonical_ref=$3 ON CONFLICT(problem_ref,canonical_ref) DO NOTHING")
        .bind(target).bind(problem).bind(source).execute(&mut *connection).await.map_err(statement)?;
        sqlx::query(
            "DELETE FROM linggan_ci_problem_member WHERE problem_ref=$1 AND canonical_ref=$2",
        )
        .bind(problem)
        .bind(source)
        .execute(&mut *connection)
        .await
        .map_err(statement)?;
        let mut source_after = source_snapshot(connection, action.domain, source).await?;
        source_after["locked"] = json!(true);
        record_source(
            connection,
            action.domain,
            source,
            action.command_ref,
            "problem_merge",
            &action.reason,
            &source_before,
            &mut source_after,
        )
        .await?;
    }
    sqlx::query("UPDATE linggan_ci_problem SET redirect_ref=$2,revision=revision+1,definition_revision=definition_revision+1,updated_at=scope_001_now() WHERE problem_ref=$1")
    .bind(problem).bind(target).execute(&mut *connection).await.map_err(statement)?;
    sqlx::query("UPDATE linggan_ci_problem SET revision=revision+1,updated_at=scope_001_now() WHERE problem_ref=$1")
    .bind(target).execute(&mut *connection).await.map_err(statement)?;
    let target_after = problem_snapshot(connection, action.domain, target).await?;
    problem_version(
        connection,
        target,
        Some(action.command_ref),
        "merge_target",
        &action.reason,
        &target_before,
        &target_after,
    )
    .await?;
    Ok(target_after)
}

async fn create_problem(
    connection: &mut PgConnection,
    action: &Action,
    problem: Uuid,
    payload: NewProblem,
    split_from: Option<Uuid>,
) -> Result<Value, StorageError> {
    if !text_ok(&payload.name, 120)
        || !text_ok(&payload.meaning, 1000)
        || payload.source_refs.is_empty()
        || payload.source_refs.len() > 500
    {
        return Err(rejected("CI_INVALID_COMMAND"));
    }
    let mut sources = std::collections::BTreeSet::new();
    for source in payload.source_refs {
        sources.insert(canonical_source(connection, action.domain, source).await?);
    }
    if let Some(parent) = split_from {
        let members: Vec<Uuid> =
            parse(&problem_snapshot(connection, action.domain, parent).await?["sourceRefs"])?;
        if sources.len() >= members.len() || sources.iter().any(|s| !members.contains(s)) {
            return Err(rejected("CI_SPLIT_MEMBERS_INVALID"));
        }
    }
    if sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM linggan_ci_problem WHERE problem_ref=$1)",
    )
    .bind(problem)
    .fetch_one(&mut *connection)
    .await
    .map_err(statement)?
    {
        return Err(rejected("CI_PROBLEM_ALREADY_EXISTS"));
    }
    let definition = json!({"lifecycle":"stable","lifecycleBasis":"human_definition","scope":"user_confirmed","boundaries":[],"examples":sources,"counterexamples":[],"splitFrom":split_from});
    sqlx::query("INSERT INTO linggan_ci_problem(problem_ref,domain_ref,name,meaning,definition,origin) VALUES($1,$2,$3,$4,$5,'manual')")
        .bind(problem).bind(action.domain).bind(payload.name.trim()).bind(payload.meaning.trim()).bind(definition).execute(&mut *connection).await.map_err(statement)?;
    let candidate_refs = payload.candidate_refs;
    let expression_scoped = !candidate_refs.is_empty();
    if candidate_refs.len() > 500 {
        return Err(rejected("CI_INVALID_COMMAND"));
    }
    for source in &sources {
        let source = *source;
        let before = source_snapshot(connection, action.domain, source).await?;
        if let Some(parent) = split_from {
            sqlx::query(
                "DELETE FROM linggan_ci_problem_member WHERE problem_ref=$1 AND canonical_ref=$2",
            )
            .bind(parent)
            .bind(source)
            .execute(&mut *connection)
            .await
            .map_err(statement)?;
        }
        sqlx::query("INSERT INTO linggan_ci_problem_member(problem_ref,canonical_ref,origin) VALUES($1,$2,'manual')")
            .bind(problem).bind(source).execute(&mut *connection).await.map_err(statement)?;
        let mut after = source_snapshot(connection, action.domain, source).await?;
        if !expression_scoped {
            after["locked"] = json!(true);
        }
        record_source(
            connection,
            action.domain,
            source,
            action.command_ref,
            &action.kind,
            &action.reason,
            &before,
            &mut after,
        )
        .await?;
    }
    confirm_problem_expressions(connection, action, problem, &sources, candidate_refs).await?;
    let after = problem_snapshot(connection, action.domain, problem).await?;
    problem_version(
        connection,
        problem,
        Some(action.command_ref),
        &action.kind,
        &action.reason,
        &json!({}),
        &after,
    )
    .await?;
    Ok(after)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CandidateResolution {
    candidate_ref: Uuid,
    target_ref: Uuid,
    target_definition_revision: i64,
    relation: String,
}

async fn resolve_candidate(
    connection: &mut PgConnection,
    action: &Action,
) -> Result<Value, StorageError> {
    let payload: CandidateResolution = parse(&action.payload)?;
    if !text_ok(&action.reason, 1000)
        || !["same", "related", "different"].contains(&payload.relation.as_str())
    {
        return Err(rejected("CI_INVALID_COMMAND"));
    }
    let revision: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_ci_problem_boundary_decision WHERE candidate_ref=$1 AND origin='manual'")
        .bind(payload.candidate_ref).fetch_one(&mut *connection).await.map_err(statement)?;
    if action.expected_revision != revision {
        return Err(rejected("CI_REVISION_CONFLICT"));
    }
    crate::comment_intelligence_problems::apply_problem_relation(
        connection,
        action.domain,
        payload.candidate_ref,
        payload.target_ref,
        payload.target_definition_revision,
        &payload.relation,
        &action.reason,
        "manual",
        Some(action.command_ref),
    )
    .await?;
    Ok(
        json!({"candidateRef":payload.candidate_ref,"targetRef":payload.target_ref,"relation":payload.relation,"revision":revision+1}),
    )
}

async fn confirm_problem_expressions(
    connection: &mut PgConnection,
    action: &Action,
    problem: Uuid,
    sources: &std::collections::BTreeSet<Uuid>,
    candidate_refs: Vec<Uuid>,
) -> Result<(), StorageError> {
    // Explicit expression identities prevent resolving unrelated problems in the same comment.
    for candidate in candidate_refs {
        let row = sqlx::query("SELECT canonical_ref,definition_key,state FROM linggan_ci_problem_candidate WHERE candidate_ref=$1 AND domain_ref=$2")
            .bind(candidate).bind(action.domain).fetch_optional(&mut *connection).await.map_err(statement)?
            .ok_or_else(|| rejected("CI_CANDIDATE_UNAVAILABLE"))?;
        if !sources.contains(&row.get::<Uuid, _>("canonical_ref"))
            || row.get::<String, _>("state") == "superseded"
        {
            return Err(rejected("CI_CANDIDATE_UNAVAILABLE"));
        }
        let reason = if action.reason.trim().is_empty() {
            "人工确认此问题表达"
        } else {
            action.reason.as_str()
        };
        crate::comment_intelligence_problems::record_boundary_decision(
            connection,
            action.domain,
            candidate,
            problem,
            1,
            "same",
            reason,
            "manual",
            Some(action.command_ref),
        )
        .await?;
        sqlx::query("UPDATE linggan_ci_problem_candidate SET state='assigned',problem_ref=$2 WHERE candidate_ref=$1")
            .bind(candidate).bind(problem).execute(&mut *connection).await.map_err(statement)?;
    }
    Ok(())
}
