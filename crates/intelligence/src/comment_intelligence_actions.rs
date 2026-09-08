//! Versioned research decisions. Source eligibility is resolved in the transaction; commands
//! retain only decisions and references, never a second copy of source text.

use linggan_storage_postgres::{Database, StorageError};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{PgConnection, Row};
use std::collections::BTreeSet;
use uuid::Uuid;

#[path = "comment_intelligence_problem_commands.rs"]
mod problem_commands;
use problem_commands::{live_problem, problem_action};
pub(crate) use problem_commands::{problem_metadata, problem_snapshot, problem_version};

pub(crate) fn rejected(code: &str) -> StorageError {
    StorageError::Statement(sqlx::Error::Protocol(code.to_owned()))
}

fn statement(error: sqlx::Error) -> StorageError {
    StorageError::Statement(error)
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Action {
    command_ref: Uuid,
    domain: Uuid,
    kind: String,
    source_ref: Option<Uuid>,
    problem_ref: Option<Uuid>,
    expected_revision: i64,
    expected_source_sha256: Option<String>,
    payload: Value,
    #[serde(default)]
    reason: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Bookmark {
    enabled: bool,
    #[serde(default)]
    note: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Correction {
    labels: Vec<String>,
    problem_refs: Vec<Uuid>,
    needs_context: bool,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NewProblem {
    name: String,
    meaning: String,
    source_refs: Vec<Uuid>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Rename {
    name: String,
    meaning: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Merge {
    target_ref: Uuid,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Undo {
    revision: i64,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TermHide {
    term: String,
    hidden: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MembershipSnapshot {
    problem_ref: Uuid,
    origin: String,
    evidence: Value,
    analysis_ref: Option<Uuid>,
}

fn parse<T: serde::de::DeserializeOwned>(value: &Value) -> Result<T, StorageError> {
    serde_json::from_value(value.clone()).map_err(|_| rejected("CI_INVALID_COMMAND"))
}

fn text_ok(value: &str, max: usize) -> bool {
    !value.trim().is_empty()
        && value.chars().count() <= max
        && !value.chars().any(|c| c.is_control())
}

pub(crate) async fn lock_domain(
    connection: &mut PgConnection,
    domain: Uuid,
) -> Result<(), StorageError> {
    if !sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM observation_domain WHERE domain_ref=$1)",
    )
    .bind(domain)
    .fetch_one(&mut *connection)
    .await
    .map_err(statement)?
    {
        return Err(rejected("CI_DOMAIN_UNAVAILABLE"));
    }
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 93047))")
        .bind(domain.to_string())
        .execute(&mut *connection)
        .await
        .map_err(statement)?;
    Ok(())
}

pub(crate) async fn canonical_source(
    connection: &mut PgConnection,
    domain: Uuid,
    source: Uuid,
) -> Result<Uuid, StorageError> {
    sqlx::query_scalar("SELECT s.canonical_ref FROM linggan_ci_source s WHERE s.domain_ref=$1 AND (s.source_ref=$2 OR s.canonical_ref=$2 OR EXISTS(SELECT 1 FROM linggan_material_comment old WHERE old.material_ref=$2 AND old.content_public_ref=s.work_ref AND old.comment_external_id=s.comment_external_id AND EXISTS(SELECT 1 FROM observation_domain d WHERE d.domain_ref=s.domain_ref AND d.is_own_domain)))")
        .bind(domain).bind(source).fetch_optional(&mut *connection).await.map_err(statement)?
        .ok_or_else(|| rejected("CI_SOURCE_UNAVAILABLE"))
}

pub(crate) async fn source_snapshot(
    connection: &mut PgConnection,
    domain: Uuid,
    canonical: Uuid,
) -> Result<Value, StorageError> {
    let row = sqlx::query("SELECT revision,labels,needs_context,locked,labels_locked,bookmarked,bookmark_note,reason,source_sha256 FROM linggan_ci_source_research WHERE canonical_ref=$1 AND domain_ref=$2")
        .bind(canonical).bind(domain).fetch_optional(&mut *connection).await.map_err(statement)?;
    let problems = sqlx::query_scalar::<_, Uuid>("SELECT m.problem_ref FROM linggan_ci_problem_member m JOIN linggan_ci_problem p USING(problem_ref) WHERE m.canonical_ref=$1 AND p.domain_ref=$2 AND p.redirect_ref IS NULL ORDER BY m.problem_ref")
        .bind(canonical).bind(domain).fetch_all(&mut *connection).await.map_err(statement)?;
    let current_hash: String = sqlx::query_scalar(
        "SELECT source_sha256 FROM linggan_ci_source WHERE canonical_ref=$1 AND domain_ref=$2",
    )
    .bind(canonical)
    .bind(domain)
    .fetch_optional(&mut *connection)
    .await
    .map_err(statement)?
    .ok_or_else(|| rejected("CI_SOURCE_UNAVAILABLE"))?;
    let mut snapshot = match row {
        Some(row) => {
            json!({"canonicalRef":canonical,"revision":row.get::<i64,_>("revision"),"labels":row.get::<Value,_>("labels"),
            "needsContext":row.get::<bool,_>("needs_context"),"locked":row.get::<bool,_>("locked"),"labelsLocked":row.get::<bool,_>("labels_locked"),"bookmarked":row.get::<bool,_>("bookmarked"),
            "bookmarkNote":row.get::<String,_>("bookmark_note"),"decisionSourceSha256":row.get::<Option<String>,_>("source_sha256"),"reason":row.get::<String,_>("reason"),"problemRefs":problems})
        }
        None => {
            let legacy = sqlx::query("SELECT a.reason FROM linggan_comment_asset_current a JOIN linggan_material_comment m ON m.material_ref=a.source_ref JOIN linggan_ci_source s ON s.work_ref=m.content_public_ref AND s.comment_external_id=m.comment_external_id WHERE s.canonical_ref=$1 AND s.domain_ref=$2 AND EXISTS(SELECT 1 FROM observation_domain d WHERE d.domain_ref=s.domain_ref AND d.is_own_domain) AND NOT a.withdrawn ORDER BY a.created_at DESC,a.asset_ref LIMIT 1")
                .bind(canonical).bind(domain).fetch_optional(&mut *connection).await.map_err(statement)?;
            let bookmarked = legacy.is_some();
            let note = legacy
                .and_then(|r| r.get::<Option<String>, _>("reason"))
                .unwrap_or_default();
            json!({"canonicalRef":canonical,"revision":0,"labels":[],"needsContext":false,"locked":false,"labelsLocked":false,
            "bookmarked":bookmarked,"bookmarkNote":note,"decisionSourceSha256":current_hash,"reason":"","problemRefs":problems})
        }
    };
    snapshot["sourceSha256"] = json!(current_hash);
    snapshot["sourceChanged"] = json!(
        snapshot["locked"] == true && snapshot["decisionSourceSha256"] != snapshot["sourceSha256"]
    );
    snapshot["active"] = json!(snapshot["sourceChanged"] != true);
    snapshot["memberships"] = membership_snapshot(connection, domain, canonical).await?;
    Ok(snapshot)
}

async fn membership_snapshot(
    connection: &mut PgConnection,
    domain: Uuid,
    canonical: Uuid,
) -> Result<Value, StorageError> {
    let rows=sqlx::query("SELECT m.problem_ref,m.origin,m.evidence,m.analysis_ref FROM linggan_ci_problem_member m JOIN linggan_ci_problem p USING(problem_ref) WHERE m.canonical_ref=$1 AND p.domain_ref=$2 ORDER BY m.problem_ref")
        .bind(canonical).bind(domain).fetch_all(&mut *connection).await.map_err(statement)?;
    Ok(json!(rows.iter().map(|r|json!({"problemRef":r.get::<Uuid,_>("problem_ref"),"origin":r.get::<String,_>("origin"),"evidence":r.get::<Value,_>("evidence"),"analysisRef":r.get::<Option<Uuid>,_>("analysis_ref")})).collect::<Vec<_>>()))
}

async fn restore_memberships(
    connection: &mut PgConnection,
    domain: Uuid,
    canonical: Uuid,
    value: &Value,
) -> Result<Vec<Uuid>, StorageError> {
    let members: Vec<MembershipSnapshot> = parse(value)?;
    if members.len() > 30 {
        return Err(rejected("CI_INVALID_STATE"));
    }
    let mut resolved = Vec::new();
    for member in members {
        if !["manual", "exact_definition", "model_equivalence"].contains(&member.origin.as_str())
            || !member.evidence.is_array()
        {
            return Err(rejected("CI_INVALID_STATE"));
        }
        let problem = live_problem(connection, domain, member.problem_ref).await?;
        resolved.push((problem, member));
    }
    sqlx::query("DELETE FROM linggan_ci_problem_member WHERE canonical_ref=$1")
        .bind(canonical)
        .execute(&mut *connection)
        .await
        .map_err(statement)?;
    for (problem, member) in &resolved {
        sqlx::query("INSERT INTO linggan_ci_problem_member(problem_ref,canonical_ref,origin,evidence,analysis_ref) VALUES($1,$2,$3,$4,$5) ON CONFLICT(problem_ref,canonical_ref) DO NOTHING")
            .bind(problem).bind(canonical).bind(&member.origin).bind(&member.evidence).bind(member.analysis_ref).execute(&mut *connection).await.map_err(statement)?;
    }
    Ok(resolved
        .into_iter()
        .map(|(problem, _)| problem)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect())
}

/// Returns manual state and append-only revision metadata after checking current source access.
pub async fn source_research(
    database: &Database,
    domain: Uuid,
    source: Uuid,
) -> Result<Value, StorageError> {
    let mut tx = database.pool().begin().await.map_err(statement)?;
    let canonical = canonical_source(&mut tx, domain, source).await?;
    let mut result = source_snapshot(&mut tx, domain, canonical).await?;
    let history = sqlx::query("SELECT r.revision,r.kind,r.reason,r.created_at::text AS at FROM linggan_ci_source_revision r JOIN linggan_ci_command c USING(command_ref) WHERE r.canonical_ref=$1 AND c.domain_ref=$2 ORDER BY r.revision DESC LIMIT 100")
        .bind(canonical).bind(domain).fetch_all(&mut *tx).await.map_err(statement)?;
    result["history"] = json!(history.iter().map(|r| json!({"revision":r.get::<i64,_>("revision"),"kind":r.get::<String,_>("kind"),"reason":r.get::<String,_>("reason"),"at":r.get::<String,_>("at")})).collect::<Vec<_>>());
    tx.commit().await.map_err(statement)?;
    Ok(result)
}

pub(crate) async fn record_source(
    connection: &mut PgConnection,
    domain: Uuid,
    canonical: Uuid,
    command: Uuid,
    kind: &str,
    reason: &str,
    before: &Value,
    after: &mut Value,
) -> Result<(), StorageError> {
    // A canonical UUID cannot carry mutable state for two domains in this schema.
    let collision: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_ci_source_research WHERE canonical_ref=$1 AND domain_ref<>$2) OR EXISTS(SELECT 1 FROM linggan_ci_source WHERE canonical_ref=$1 AND domain_ref<>$2)")
        .bind(canonical).bind(domain).fetch_one(&mut *connection).await.map_err(statement)?;
    if collision {
        return Err(rejected("CI_SOURCE_DOMAIN_COLLISION"));
    }
    let revision = before["revision"]
        .as_i64()
        .ok_or_else(|| rejected("CI_INVALID_STATE"))?
        + 1;
    if kind == "correct" {
        after["decisionSourceSha256"] = before["sourceSha256"].clone();
    } else if kind.starts_with("problem_") && before["sourceChanged"] == true {
        return Err(rejected("CI_SOURCE_CHANGED"));
    }
    after["sourceSha256"] = before["sourceSha256"].clone();
    after["sourceChanged"] =
        json!(after["locked"] == true && after["decisionSourceSha256"] != after["sourceSha256"]);
    after["active"] = json!(after["sourceChanged"] != true);
    after["revision"] = json!(revision);
    after["reason"] = json!(reason);
    after["memberships"] = membership_snapshot(connection, domain, canonical).await?;
    let written = sqlx::query("INSERT INTO linggan_ci_source_research(canonical_ref,domain_ref,revision,labels,needs_context,locked,labels_locked,bookmarked,bookmark_note,reason,source_sha256) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) ON CONFLICT(canonical_ref) DO UPDATE SET revision=EXCLUDED.revision,labels=EXCLUDED.labels,needs_context=EXCLUDED.needs_context,locked=EXCLUDED.locked,labels_locked=EXCLUDED.labels_locked,bookmarked=EXCLUDED.bookmarked,bookmark_note=EXCLUDED.bookmark_note,reason=EXCLUDED.reason,source_sha256=EXCLUDED.source_sha256,updated_at=scope_001_now() WHERE linggan_ci_source_research.domain_ref=EXCLUDED.domain_ref")
        .bind(canonical).bind(domain).bind(revision).bind(&after["labels"])
        .bind(after["needsContext"].as_bool().unwrap_or(false)).bind(after["locked"].as_bool().unwrap_or(false))
        .bind(after["labelsLocked"].as_bool().unwrap_or(false))
        .bind(after["bookmarked"].as_bool().unwrap_or(false)).bind(after["bookmarkNote"].as_str().unwrap_or(""))
        .bind(reason).bind(after["decisionSourceSha256"].as_str()).execute(&mut *connection).await.map_err(statement)?;
    if written.rows_affected() != 1 {
        return Err(rejected("CI_SOURCE_DOMAIN_COLLISION"));
    }
    sqlx::query("INSERT INTO linggan_ci_source_revision(canonical_ref,revision,command_ref,kind,before_value,after_value,reason) VALUES($1,$2,$3,$4,$5,$6,$7)")
        .bind(canonical).bind(revision).bind(command).bind(kind).bind(before).bind(&*after).bind(reason)
        .execute(&mut *connection).await.map_err(statement)?;
    Ok(())
}

async fn replace_memberships(
    connection: &mut PgConnection,
    domain: Uuid,
    canonical: Uuid,
    refs: &[Uuid],
) -> Result<Vec<Uuid>, StorageError> {
    if refs.len() > 30 {
        return Err(rejected("CI_INVALID_COMMAND"));
    }
    let mut resolved = std::collections::BTreeSet::new();
    for problem in refs {
        resolved.insert(live_problem(connection, domain, *problem).await?);
    }
    sqlx::query("DELETE FROM linggan_ci_problem_member WHERE canonical_ref=$1")
        .bind(canonical)
        .execute(&mut *connection)
        .await
        .map_err(statement)?;
    for problem in &resolved {
        sqlx::query("INSERT INTO linggan_ci_problem_member(problem_ref,canonical_ref,origin) VALUES($1,$2,'manual')")
            .bind(problem).bind(canonical).execute(&mut *connection).await.map_err(statement)?;
    }
    Ok(resolved.into_iter().collect())
}

async fn source_action(
    connection: &mut PgConnection,
    action: &Action,
    canonical: Option<Uuid>,
) -> Result<Value, StorageError> {
    let source = canonical.ok_or_else(|| rejected("CI_INVALID_COMMAND"))?;
    let before = source_snapshot(connection, action.domain, source).await?;
    if action
        .expected_source_sha256
        .as_ref()
        .is_some_and(|hash| Some(hash.as_str()) != before["sourceSha256"].as_str())
    {
        return Err(rejected("CI_SOURCE_CHANGED"));
    }
    if before["revision"].as_i64() != Some(action.expected_revision) {
        return Err(rejected("CI_REVISION_CONFLICT"));
    }
    let mut after = before.clone();
    match action.kind.as_str() {
        "bookmark" => {
            let payload: Bookmark = parse(&action.payload)?;
            if payload.note.chars().count() > 1000 {
                return Err(rejected("CI_INVALID_COMMAND"));
            }
            after["bookmarked"] = json!(payload.enabled);
            after["bookmarkNote"] = json!(payload.note);
        }
        "correct" => {
            if action.reason.trim().is_empty() {
                return Err(rejected("CI_REASON_REQUIRED"));
            }
            let mut payload: Correction = parse(&action.payload)?;
            if payload.labels.len() > 4
                || payload
                    .labels
                    .iter()
                    .any(|s| !["need", "solution", "story", "quote"].contains(&s.as_str()))
            {
                return Err(rejected("CI_INVALID_COMMAND"));
            }
            payload.labels.sort();
            payload.labels.dedup();
            after["labels"] = json!(payload.labels);
            after["needsContext"] = json!(payload.needs_context);
            after["locked"] = json!(true);
            after["labelsLocked"] = json!(true);
            after["problemRefs"] = json!(
                replace_memberships(connection, action.domain, source, &payload.problem_refs)
                    .await?
            );
        }
        _ => {
            let payload: Undo = parse(&action.payload)?;
            if payload.revision <= 0 || payload.revision != action.expected_revision {
                return Err(rejected("CI_UNDO_NOT_LATEST"));
            }
            let previous: Value = sqlx::query_scalar("SELECT before_value FROM linggan_ci_source_revision WHERE canonical_ref=$1 AND revision=$2")
            .bind(source).bind(payload.revision).fetch_optional(&mut *connection).await.map_err(statement)?
            .ok_or_else(|| rejected("CI_REVISION_UNAVAILABLE"))?;
            after = previous;
            after["problemRefs"] = json!(
                restore_memberships(connection, action.domain, source, &after["memberships"])
                    .await?
            );
        }
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
    Ok(after)
}

/// Execute one domain-scoped command atomically. Reusing an ID with different content fails;
/// an unchanged replay returns its receipt without repeating membership or history writes.
pub async fn execute_action(database: &Database, value: Value) -> Result<Value, StorageError> {
    let action: Action = parse(&value)?;
    if action.expected_revision < 0
        || action.reason.chars().count() > 1000
        || action.reason.chars().any(|c| c.is_control())
    {
        return Err(rejected("CI_INVALID_COMMAND"));
    }
    let mut tx = database.pool().begin().await.map_err(statement)?;
    lock_domain(&mut tx, action.domain).await?;
    let canonical = if let Some(source) = action.source_ref {
        Some(canonical_source(&mut tx, action.domain, source).await?)
    } else {
        None
    };
    if let Some(row) =
        sqlx::query("SELECT request,result FROM linggan_ci_command WHERE command_ref=$1")
            .bind(action.command_ref)
            .fetch_optional(&mut *tx)
            .await
            .map_err(statement)?
    {
        if row.get::<Value, _>("request") != value {
            return Err(rejected("CI_IDEMPOTENCY_CONFLICT"));
        }
        return Ok(row.get::<Value, _>("result"));
    }
    let result = match action.kind.as_str() {
        "bookmark" | "correct" | "undo" => source_action(&mut tx, &action, canonical).await?,
        "term_hide" => {
            let payload: TermHide = parse(&action.payload)?;
            let term = payload.term.trim();
            if !text_ok(term, 48) || term.chars().count() < 2 {
                return Err(rejected("CI_INVALID_COMMAND"));
            }
            let revision: Option<i64> = sqlx::query_scalar(
                "SELECT revision FROM linggan_ci_term_setting WHERE domain_ref=$1 AND term=$2",
            )
            .bind(action.domain)
            .bind(term)
            .fetch_optional(&mut *tx)
            .await
            .map_err(statement)?;
            if revision.unwrap_or(0) != action.expected_revision {
                return Err(rejected("CI_REVISION_CONFLICT"));
            }
            let revision = action.expected_revision + 1;
            sqlx::query("INSERT INTO linggan_ci_term_setting(domain_ref,term,hidden,revision) VALUES($1,$2,$3,$4) ON CONFLICT(domain_ref,term) DO UPDATE SET hidden=EXCLUDED.hidden,revision=EXCLUDED.revision,updated_at=scope_001_now()")
                .bind(action.domain).bind(term).bind(payload.hidden).bind(revision).execute(&mut *tx).await.map_err(statement)?;
            json!({"term":term,"hidden":payload.hidden,"revision":revision})
        }
        "problem_create" | "problem_rename" | "problem_merge" | "problem_split"
        | "problem_bookmark" => problem_action(&mut tx, &action).await?,
        _ => return Err(rejected("CI_INVALID_COMMAND")),
    };
    sqlx::query(
        "INSERT INTO linggan_ci_command(command_ref,domain_ref,request,result) VALUES($1,$2,$3,$4)",
    )
    .bind(action.command_ref)
    .bind(action.domain)
    .bind(value)
    .bind(&result)
    .execute(&mut *tx)
    .await
    .map_err(statement)?;
    tx.commit().await.map_err(statement)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn commands_reject_unknown_fields_and_group_labels_as_manual_semantic_labels() {
        let input = json!({"labels":["need","conflict"],"problemRefs":[],"needsContext":false});
        let parsed: Correction = parse(&input).unwrap();
        assert!(
            parsed
                .labels
                .iter()
                .any(|l| !["need", "solution", "story", "quote"].contains(&l.as_str()))
        );
        assert!(
            parse::<Correction>(
                &json!({"labels":[],"problemRefs":[],"needsContext":false,"admin":true})
            )
            .is_err()
        );
        assert!(!text_ok("\n", 120));
        assert!(!text_ok("", 120));
        assert!(text_ok("多久才有变化", 120));
    }
}
