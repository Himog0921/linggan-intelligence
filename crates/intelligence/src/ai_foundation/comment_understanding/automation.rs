//! Scheduled selections preserve source identity and use the ordinary operator confirmation path.
mod submission;
use super::{MAX_SELECTIONS, start::read_only_tx};
use crate::{
    ai_foundation::store::{Command, FoundationError},
    comment_cleaning::CLEANER_VERSION,
    comment_study_catalog,
};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::AssertSqlSafe;
use uuid::Uuid;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Change {
    pub command_ref: Uuid,
    pub domain_ref: Uuid,
    pub expected_revision: i64,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_submit: Option<bool>,
    pub minute_of_day: i32,
    pub time_zone: String,
    pub selection_limit: i32,
}

const SETTING: &str = "SELECT jsonb_build_object(
    'domainRef', d.domain_ref, 'enabled', coalesce(a.enabled,false), 'autoSubmit',coalesce(a.auto_submit,false),
    'minuteOfDay', coalesce(a.minute_of_day,540), 'timeZone', coalesce(a.time_zone,'Asia/Shanghai'),
    'selectionLimit', coalesce(a.selection_limit,300), 'revision', coalesce(a.revision,0),
    'enabledFrom', a.enabled_from, 'nextRunAt', a.next_run_at)
    FROM observation_domain d LEFT JOIN linggan_comment_research_automation a USING (domain_ref)
    WHERE d.domain_ref=$1";

pub async fn read(
    db: &Database,
    domain: Uuid,
    before: Option<Uuid>,
) -> Result<Value, FoundationError> {
    let mut tx = read_only_tx(db).await?;
    let setting: Value = sqlx::query_scalar(SETTING)
        .bind(domain)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(FoundationError::NotFound)?;
    let mut selections: Vec<Value> = sqlx::query_scalar(
        "SELECT jsonb_build_object('selectionRef', s.selection_ref,'scheduledAt',s.scheduled_at,
             'generatedAt',s.generated_at,
             'submissionCounts', (SELECT coalesce(jsonb_object_agg(state,n),'{}') FROM
              (SELECT submission_state state,count(*) n FROM linggan_comment_research_selection_target WHERE selection_ref=s.selection_ref GROUP BY submission_state) totals),
             'submissionReasons',(SELECT coalesce(jsonb_agg(reason ORDER BY reason),'[]') FROM
              (SELECT DISTINCT submission_reason reason FROM linggan_comment_research_selection_target WHERE selection_ref=s.selection_ref AND submission_reason IS NOT NULL ORDER BY reason LIMIT 8) reasons),
             'batchRefs',(SELECT coalesce(jsonb_agg(DISTINCT submitted_batch_ref),'[]') FROM linggan_comment_research_selection_target WHERE selection_ref=s.selection_ref AND submitted_batch_ref IS NOT NULL),
             'targetCount',(SELECT count(*) FROM linggan_comment_research_selection_target t WHERE t.selection_ref=s.selection_ref))
         FROM linggan_comment_research_selection s WHERE s.domain_ref=$1 AND
           ($2::uuid IS NULL OR s.scheduled_at < (SELECT scheduled_at FROM linggan_comment_research_selection WHERE domain_ref=$1 AND selection_ref=$2))
         ORDER BY s.scheduled_at DESC LIMIT 21",
    )
    .bind(domain)
    .bind(before)
    .fetch_all(&mut *tx)
    .await?;
    let more = selections.len() > 20;
    selections.truncate(20);
    let next = if more {
        selections.last().map(|item| &item["selectionRef"])
    } else {
        None
    };
    tx.commit().await?;
    Ok(json!({"setting":setting,"selections":selections,"nextBefore":next}))
}

pub async fn read_selection(
    db: &Database,
    domain: Uuid,
    selection: Uuid,
) -> Result<Value, FoundationError> {
    let mut tx = read_only_tx(db).await?;
    let result = sqlx::query_scalar(include_str!("automation_selection.sql"))
        .bind(selection)
        .bind(domain)
        .bind(Option::<Vec<i32>>::None)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(FoundationError::NotFound)?;
    tx.commit().await?;
    Ok(result)
}

pub async fn change(db: &Database, request: &Change) -> Result<Value, FoundationError> {
    if request.command_ref.is_nil()
        || request.domain_ref.is_nil()
        || (request.auto_submit == Some(true) && !request.enabled)
        || request.expected_revision < 0
        || !(0..1440).contains(&request.minute_of_day)
        || !(1..=MAX_SELECTIONS as i32).contains(&request.selection_limit)
        || request.time_zone.is_empty()
        || request.time_zone.len() > 128
    {
        return Err(FoundationError::Invalid);
    }
    let mut command = Command::begin(
        db,
        request.command_ref,
        "comment_research_automation",
        request,
    )
    .await?;
    if let Some(receipt) = command.replay()? {
        return Ok(receipt);
    }
    let valid_zone: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_timezone_names WHERE name=$1)")
            .bind(&request.time_zone)
            .fetch_one(&mut *command.tx)
            .await?;
    if !valid_zone {
        return Err(FoundationError::Invalid);
    }
    sqlx::query(
        "INSERT INTO linggan_comment_research_automation(domain_ref)
         SELECT domain_ref FROM observation_domain WHERE domain_ref=$1 ON CONFLICT DO NOTHING",
    )
    .bind(request.domain_ref)
    .execute(&mut *command.tx)
    .await?;
    let current: i64 = sqlx::query_scalar(
        "SELECT revision FROM linggan_comment_research_automation WHERE domain_ref=$1 FOR UPDATE",
    )
    .bind(request.domain_ref)
    .fetch_optional(&mut *command.tx)
    .await?
    .ok_or(FoundationError::NotFound)?;
    if current != request.expected_revision {
        return Err(FoundationError::Conflict);
    }
    sqlx::query(
        "UPDATE linggan_comment_research_automation
         SET enabled=$2,auto_submit=$2 AND coalesce($6,auto_submit),minute_of_day=$3,time_zone=$4,selection_limit=$5,revision=revision+1,
             enabled_from=CASE WHEN $2 THEN coalesce(enabled_from,scope_001_now()) ELSE enabled_from END,
             next_run_at=CASE WHEN $2 THEN linggan_comment_research_next_run(scope_001_now(),$3,$4) END
         WHERE domain_ref=$1",
    )
    .bind(request.domain_ref)
    .bind(request.enabled)
    .bind(request.minute_of_day)
    .bind(&request.time_zone)
    .bind(request.selection_limit)
    .bind(request.auto_submit)
    .execute(&mut *command.tx)
    .await?;
    let receipt: Value = sqlx::query_scalar(SETTING)
        .bind(request.domain_ref)
        .fetch_one(&mut *command.tx)
        .await?;
    command.finish(receipt).await
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ScanError {
    #[error(transparent)]
    Selection(#[from] comment_study_catalog::StudyCatalogError),
    #[error(transparent)]
    Submission(#[from] FoundationError),
}

pub(crate) async fn tick(db: &Database) -> Result<(), ScanError> {
    let generation = generate(db).await;
    let submission = submission::advance(db).await;
    if let Err(generation_error) = generation {
        if let Err(submission_error) = submission {
            eprintln!("scheduled comment submission deferred: {submission_error}");
        }
        return Err(generation_error.into());
    }
    submission.map_err(ScanError::from)
}

// Claiming the schedule, freezing the sources and advancing the next slot commit together.
pub(super) async fn generate(db: &Database) -> Result<(), comment_study_catalog::StudyCatalogError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET LOCAL statement_timeout='15s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL lock_timeout='3s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL jit=off").execute(&mut *tx).await?;
    let due: Option<(Uuid, String, String, i32, bool)> = sqlx::query_as(
        "SELECT a.domain_ref, a.enabled_from::text, a.next_run_at::text, a.selection_limit, a.auto_submit
         FROM linggan_comment_research_automation a JOIN observation_domain d USING(domain_ref)
         WHERE a.enabled AND a.next_run_at<=scope_001_now() AND d.status='active'
         ORDER BY a.next_run_at,a.domain_ref LIMIT 1 FOR UPDATE OF a SKIP LOCKED",
    )
    .fetch_optional(&mut *tx)
    .await?;
    let Some((domain, enabled_from, due_at, limit, auto_submit)) = due else {
        return Ok(());
    };
    let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(&mut *tx)
        .await?;
    let cache = comment_study_catalog::refresh_clean_cache_in(
        &mut tx,
        domain,
        comment_study_catalog::MAX_REFRESH_LIMIT,
        Some((&enabled_from, &as_of)),
    )
    .await?;
    if cache.examined_count == comment_study_catalog::MAX_REFRESH_LIMIT as usize {
        // Continue this slot on the next scan so pending earlier comments cannot be overtaken.
        tx.commit().await?;
        return Ok(());
    }
    let select_sql = format!(
        "{} {}",
        comment_study_catalog::facts_sql(),
        include_str!("automation.sql")
    );
    let targets: Vec<(Uuid, String, Uuid)> = sqlx::query_as(AssertSqlSafe(select_sql))
        .bind(domain)
        .bind(&as_of)
        .bind(CLEANER_VERSION)
        .bind(Option::<Uuid>::None)
        .bind(Option::<String>::None)
        .bind(enabled_from)
        .bind(i64::from(limit))
        .fetch_all(&mut *tx)
        .await?;
    let selection = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_comment_research_selection(selection_ref,domain_ref,scheduled_at,generated_at)
         VALUES($1,$2,$3::timestamptz,$4::timestamptz)",
    )
    .bind(selection)
    .bind(domain)
    .bind(due_at)
    .bind(&as_of)
    .execute(&mut *tx)
    .await?;
    let mut works = Vec::with_capacity(targets.len());
    let mut comments = Vec::with_capacity(targets.len());
    let mut sources = Vec::with_capacity(targets.len());
    for (work, external_id, source) in targets {
        works.push(work);
        comments.push(external_id);
        sources.push(source);
    }
    sqlx::query(
        "INSERT INTO linggan_comment_research_selection_target
            (domain_ref,selection_ref,position,work_ref,comment_external_id,source_ref,submission_state,next_check_at)
         SELECT $1,$2,t.position::integer,t.work_ref,t.comment_external_id,t.source_ref,CASE WHEN $6 THEN 'pending' ELSE 'manual' END, CASE WHEN $6 THEN scope_001_now() END
         FROM unnest($3::uuid[],$4::text[],$5::uuid[]) WITH ORDINALITY
            AS t(work_ref,comment_external_id,source_ref,position)",
    )
    .bind(domain)
    .bind(selection)
    .bind(works)
    .bind(comments)
    .bind(sources)
    .bind(auto_submit)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE linggan_comment_research_automation SET next_run_at=linggan_comment_research_next_run($2::timestamptz,minute_of_day,time_zone) WHERE domain_ref=$1")
        .bind(domain)
        .bind(as_of)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
