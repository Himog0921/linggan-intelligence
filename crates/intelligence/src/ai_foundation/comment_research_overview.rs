//! Current source catalog and independently qualified research, never retired read tables.
use super::{comment_problem, comment_research, store::FoundationError};
use crate::{comment_cleaning::CLEANER_VERSION, comment_study_catalog};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use std::collections::HashSet;
use uuid::Uuid;

pub async fn read(db: &Database, domain: Uuid) -> Result<Value, FoundationError> {
    if domain.is_nil() {
        return Err(FoundationError::Invalid);
    }
    let problems = comment_problem::read_problems(db, domain).await?;
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='15s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL jit=off").execute(&mut *tx).await?;
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM observation_domain WHERE domain_ref=$1)")
            .bind(domain)
            .fetch_one(&mut *tx)
            .await?;
    if !exists {
        return Err(FoundationError::NotFound);
    }
    let as_of: String = sqlx::query_scalar(
        "SELECT to_char(scope_001_now() AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')",
    )
    .fetch_one(&mut *tx)
    .await?;
    let query = format!(
        "{} {}",
        comment_study_catalog::facts_sql(),
        include_str!("comment_research_overview/catalog_counts.sql")
    );
    let counts: Value = sqlx::query_scalar(sqlx::AssertSqlSafe(query))
        .bind(domain)
        .bind(&as_of)
        .bind(CLEANER_VERSION)
        .bind(Option::<Uuid>::None)
        .bind(Option::<String>::None)
        .fetch_one(&mut *tx)
        .await?;
    let query = format!(
        "{} {}",
        comment_study_catalog::facts_sql(),
        include_str!("comment_research_overview/effective_results.sql")
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(query))
        .bind(domain)
        .bind(&as_of)
        .bind(CLEANER_VERSION)
        .bind(Option::<Uuid>::None)
        .bind(Option::<String>::None)
        .fetch_all(&mut *tx)
        .await?;
    let mut succeeded = 0;
    let mut no_signal = 0;
    let mut signal_count = 0;
    let mut solution_count = 0;
    let mut experience_count = 0;
    let mut solution_works = HashSet::new();
    let mut experience_works = HashSet::new();
    let mut solutions = Vec::new();
    let mut experiences = Vec::new();
    for row in rows {
        let request: Uuid = row.get("request_ref");
        let (source, _) = match comment_research::frozen_in(&mut tx, request).await {
            Ok(v) => v,
            Err(FoundationError::AdmissionDenied(_)) => continue,
            Err(e) => return Err(e),
        };
        let output: Value = row.get("result");
        let signals = output["signals"]
            .as_array()
            .ok_or(FoundationError::Invalid)?;
        succeeded += 1;
        if output["outcome"] == "no_signal" {
            no_signal += 1;
        }
        signal_count += signals.len();
        for signal in signals {
            let kind = signal["kind"].as_str().ok_or(FoundationError::Invalid)?;
            if !["solution", "experience"].contains(&kind) {
                continue;
            }
            let start = signal["evidenceSpan"]["start"]
                .as_u64()
                .ok_or(FoundationError::Invalid)? as usize;
            let end = signal["evidenceSpan"]["end"]
                .as_u64()
                .ok_or(FoundationError::Invalid)? as usize;
            if start >= end || end > source.raw_text.chars().count() {
                return Err(FoundationError::Invalid);
            }
            let quote: String = source
                .raw_text
                .chars()
                .skip(start)
                .take(end - start)
                .collect();
            let item = json!({"requestRef":request,"commentKey":{"workRef":source.source.content_public_ref,"commentExternalId":source.comment_external_id},"quote":quote,"proposition":signal["proposition"],"workTitle":source.source.work_title});
            if kind == "solution" {
                solution_count += 1;
                solution_works.insert(source.source.content_public_ref);
                if solutions.len() < 3 {
                    solutions.push(item)
                }
            } else {
                experience_count += 1;
                experience_works.insert(source.source.content_public_ref);
                if experiences.len() < 3 {
                    experiences.push(item)
                }
            }
        }
    }
    let series: Value = sqlx::query_scalar(include_str!(
        "comment_research_overview/observation_series.sql"
    ))
    .bind(domain)
    .bind(&as_of)
    .fetch_one(&mut *tx)
    .await?;
    let problem_rows = problems["problems"]
        .as_array()
        .ok_or(FoundationError::Invalid)?;
    let candidates = problems["candidates"]
        .as_array()
        .ok_or(FoundationError::Invalid)?;
    let refs: Vec<Uuid> = problem_rows
        .iter()
        .map(|v| {
            serde_json::from_value(v["definitionRef"].clone()).map_err(|_| FoundationError::Invalid)
        })
        .collect::<Result<_, _>>()?;
    let yesterday:i64=sqlx::query_scalar("SELECT count(*) FROM linggan_comment_problem_definition WHERE definition_ref=ANY($1) AND (created_at AT TIME ZONE 'Asia/Shanghai')::date=($2::timestamptz AT TIME ZONE 'Asia/Shanghai')::date-1").bind(&refs).bind(&as_of).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    Ok(
        json!({"contract":"comment-research.overview.v1","domainRef":domain,"asOf":as_of,"corpusSummary":counts,"indexCoverage":{"indexedCount":counts["indexedCount"],"pendingCount":counts["pendingCount"]},"researchSummary":{"studiedCommentCount":counts["studiedCommentCount"],"succeededCommentCount":succeeded,"noSignalCommentCount":no_signal,"currentSignalCount":signal_count},"knowledgeSummary":{"problemCount":problem_rows.len(),"pendingCandidateCount":candidates.len(),"solutionSignalCount":solution_count,"experienceSignalCount":experience_count,"solutionWorkCount":solution_works.len(),"experienceWorkCount":experience_works.len(),"solutionPreview":solutions,"experiencePreview":experiences,"problemPreview":problem_rows.iter().take(3).collect::<Vec<_>>()},"yesterdayNewProblemCount":yesterday,"observationSeries":series}),
    )
}
