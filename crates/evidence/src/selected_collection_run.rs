//! The frozen work list of one selected collection run can restrict this process's dispatch.
use linggan_storage_postgres::Database;
use sqlx::PgConnection;
use std::{
    env,
    time::{SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

const ENV: &str = "LINGGAN_SELECTED_COLLECTION_RUN";
const MAX_WINDOW_SECONDS: u64 = 24 * 60 * 60;

#[derive(Debug)]
pub enum ExecutionScope {
    Unrestricted,
    Blocked,
    Selected(SelectedCollectionRun),
}

#[derive(Debug)]
pub struct SelectedCollectionRun {
    pub run_ref: Uuid,
    pub domain_ref: Uuid,
    pub work_refs: Vec<Uuid>,
}

/// A configured but invalid, expired, paused or revoked scope must still block default work.
pub fn configured() -> bool {
    env::var_os(ENV).is_some()
}

pub async fn load(db: &Database) -> Result<ExecutionScope, sqlx::Error> {
    if !configured() {
        return Ok(ExecutionScope::Unrestricted);
    }
    load_in(&mut *db.pool().acquire().await?).await
}

pub async fn load_in(conn: &mut PgConnection) -> Result<ExecutionScope, sqlx::Error> {
    resolve_in(conn, false).await
}

/// Revalidate before admitting an external effect, holding its authority until commit.
/// Call this on the same open transaction that records the admission.
pub async fn lock_in(conn: &mut PgConnection) -> Result<ExecutionScope, sqlx::Error> {
    resolve_in(conn, true).await
}

async fn resolve_in(conn: &mut PgConnection, lock: bool) -> Result<ExecutionScope, sqlx::Error> {
    let Some(setting) = env::var_os(ENV) else {
        return Ok(ExecutionScope::Unrestricted);
    };
    let Some((run_ref, expiry)) = setting.to_str().and_then(|s| {
        parse(
            s,
            SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs(),
        )
    }) else {
        return Ok(ExecutionScope::Blocked);
    };
    const SELECTED: &str =
        "SELECT r.domain_ref, ARRAY(SELECT w.content_public_ref FROM collection_run_work w
             WHERE w.run_ref=r.run_ref ORDER BY w.ordinal)
         FROM collection_run r
         JOIN observation_domain d USING(domain_ref)
         JOIN collection_acquisition_authorization a USING(authorization_ref)
         WHERE r.run_ref=$1 AND d.status='active' AND r.source_kind='selected_works'
           AND r.comment_mode='none' AND r.scope_frozen_at IS NOT NULL
           AND r.control_intent='running' AND r.phase IN ('materials','ended')
           AND r.work_limit<=10 AND a.revoked_at IS NULL AND a.expires_at>clock_timestamp()
           AND clock_timestamp()<to_timestamp($2::double precision)";
    let selected: Option<(Uuid, Vec<Uuid>)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "{SELECTED}{}",
        if lock { " FOR SHARE OF r,d,a" } else { "" },
    )))
    .bind(run_ref)
    .bind(expiry as f64)
    .fetch_optional(conn)
    .await?;
    Ok(match selected {
        Some((domain_ref, work_refs)) if (1..=10).contains(&work_refs.len()) => {
            ExecutionScope::Selected(SelectedCollectionRun {
                run_ref,
                domain_ref,
                work_refs,
            })
        }
        _ => ExecutionScope::Blocked,
    })
}

fn parse(value: &str, now: u64) -> Option<(Uuid, u64)> {
    let (run, expiry) = value.split_once('@')?;
    let run = run.parse::<Uuid>().ok()?;
    let expiry = expiry.parse::<u64>().ok()?;
    if run.is_nil() || !(1..=MAX_WINDOW_SECONDS).contains(&expiry.checked_sub(now)?) {
        return None;
    }
    Some((run, expiry))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_window_requires_a_non_nil_run_and_a_bounded_live_expiry() {
        let run = Uuid::from_u128(1);
        let now = 1_000_000;
        assert_eq!(
            parse(&format!("{run}@{}", now + 1), now),
            Some((run, now + 1))
        );
        for invalid in [
            String::new(),
            run.to_string(),
            format!("{}@{}", Uuid::nil(), now + 1),
            format!("{run}@{now}"),
            format!("{run}@{}", now - 1),
            format!("{run}@{}", now + MAX_WINDOW_SECONDS + 1),
        ] {
            assert_eq!(parse(&invalid, now), None, "{invalid}");
        }
    }
}
