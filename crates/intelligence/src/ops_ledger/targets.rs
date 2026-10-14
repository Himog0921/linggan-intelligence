//! OPS-MIGRATION-001 D3: weekly and monthly 加微 targets (PAGE-OPS-002 §5, OPS-HTTP-001 §4).
//! The only write the operations pages make. Every save request leaves a receipt; a save only
//! lands when the page saw the current revision, so a retry or a stale page never overwrites
//! someone else's change. Until login (D4) a save can only say it came from the local machine.

use linggan_storage_postgres::Database;
use serde::Serialize;
use sqlx::Row;
use uuid::Uuid;

pub const OPS_TARGET_MAX: i32 = 1_000_000;
const METRIC: &str = "followers_gained";
const SET_BY: &str = "local_machine";

/// A target period: an ISO week (`2026-W42`) or a month (`2026-10`), on the Beijing calendar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetPeriod {
    period_type: &'static str,
    period_key: String,
}

impl TargetPeriod {
    pub fn parse(period_type: &str, period_key: &str) -> Result<Self, OpsTargetError> {
        let key = period_key.trim();
        let valid = match period_type {
            "week" => week_key(key),
            "month" => month_key(key),
            _ => false,
        };
        if !valid {
            return Err(OpsTargetError::InvalidParameter("period"));
        }
        Ok(Self {
            period_type: if period_type == "week" {
                "week"
            } else {
                "month"
            },
            period_key: key.to_owned(),
        })
    }

    pub fn period_type(&self) -> &'static str {
        self.period_type
    }

    pub fn period_key(&self) -> &str {
        &self.period_key
    }
}

fn week_key(key: &str) -> bool {
    let bytes = key.as_bytes();
    bytes.len() == 8
        && bytes[..4].iter().all(u8::is_ascii_digit)
        && &bytes[4..6] == b"-W"
        && bytes[6..].iter().all(u8::is_ascii_digit)
        && key[6..]
            .parse::<u8>()
            .is_ok_and(|week| (1..=53).contains(&week))
}

fn month_key(key: &str) -> bool {
    let bytes = key.as_bytes();
    bytes.len() == 7
        && bytes[..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'-'
        && bytes[5..].iter().all(u8::is_ascii_digit)
        && key[5..]
            .parse::<u8>()
            .is_ok_and(|month| (1..=12).contains(&month))
}

#[derive(Debug, thiserror::Error)]
pub enum OpsTargetError {
    #[error("ops_parameter_invalid")]
    InvalidParameter(&'static str),
    /// The same request id came back with a different body.
    #[error("ops_target_request_conflict")]
    RequestConflict,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsTargetRevision {
    pub revision_ref: Uuid,
    pub period_type: String,
    pub period_key: String,
    pub target_value: i32,
    pub previous_revision_ref: Option<Uuid>,
    pub set_by: String,
    pub set_at: String,
    /// `MM-DD HH:MM`, Beijing time.
    pub set_at_beijing: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsTargetHistory {
    pub period_type: &'static str,
    pub period_key: String,
    pub current: Option<OpsTargetRevision>,
    /// Newest first.
    pub revisions: Vec<OpsTargetRevision>,
}

pub struct SetTargetRequest {
    pub period: TargetPeriod,
    pub target_value: i32,
    pub expected_revision: Option<Uuid>,
    pub request_ref: Uuid,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsTargetReceipt {
    pub request_id: Uuid,
    /// `accepted`, `unchanged` or `stale`.
    pub outcome: String,
    pub replayed: bool,
    pub current: Option<OpsTargetRevision>,
}

const REVISION_COLUMNS: &str = "r.revision_ref, r.period_type, r.period_key, r.target_value, \
    r.previous_revision_ref, r.set_by, \
    to_char(r.set_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"') AS set_at, \
    to_char(r.set_at AT TIME ZONE 'Asia/Shanghai', 'MM-DD HH24:MI') AS set_at_beijing";

fn revision(row: &sqlx::postgres::PgRow) -> Result<OpsTargetRevision, sqlx::Error> {
    Ok(OpsTargetRevision {
        revision_ref: row.try_get("revision_ref")?,
        period_type: row.try_get("period_type")?,
        period_key: row.try_get("period_key")?,
        target_value: row.try_get("target_value")?,
        previous_revision_ref: row.try_get("previous_revision_ref")?,
        set_by: row.try_get("set_by")?,
        set_at: row.try_get("set_at")?,
        set_at_beijing: row.try_get("set_at_beijing")?,
    })
}

/// The tip of the period's chain: the revision no other revision replaces.
pub(crate) async fn current_target<'e, E>(
    executor: E,
    period: &TargetPeriod,
) -> Result<Option<OpsTargetRevision>, sqlx::Error>
where
    E: sqlx::PgExecutor<'e>,
{
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {REVISION_COLUMNS} FROM linggan_ops_target_revision r \
          WHERE r.period_type = $1 AND r.period_key = $2 AND r.metric = '{METRIC}' \
            AND NOT EXISTS (SELECT 1 FROM linggan_ops_target_revision n \
                             WHERE n.previous_revision_ref = r.revision_ref)"
    )))
    .bind(period.period_type)
    .bind(&period.period_key)
    .fetch_optional(executor)
    .await?
    .as_ref()
    .map(revision)
    .transpose()
}

pub async fn read_ops_target_history(
    database: &Database,
    period: &TargetPeriod,
) -> Result<OpsTargetHistory, OpsTargetError> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {REVISION_COLUMNS} FROM linggan_ops_target_revision r \
          WHERE r.period_type = $1 AND r.period_key = $2 AND r.metric = '{METRIC}' \
          ORDER BY r.set_at DESC, r.revision_ref"
    )))
    .bind(period.period_type)
    .bind(&period.period_key)
    .fetch_all(database.pool())
    .await?;
    Ok(OpsTargetHistory {
        period_type: period.period_type,
        period_key: period.period_key.clone(),
        current: current_target(database.pool(), period).await?,
        revisions: rows.iter().map(revision).collect::<Result<_, _>>()?,
    })
}

/// Saves a target. Serialised per period; the receipt is written in the same transaction as the
/// revision it reports, so a receipt never claims a revision that did not land.
pub async fn set_ops_target(
    database: &Database,
    request: &SetTargetRequest,
) -> Result<OpsTargetReceipt, OpsTargetError> {
    if !(1..=OPS_TARGET_MAX).contains(&request.target_value) {
        return Err(OpsTargetError::InvalidParameter("targetValue"));
    }
    let period = &request.period;
    let mut transaction = database.pool().begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(format!(
            "linggan_ops_target:{}:{}",
            period.period_type, period.period_key
        ))
        .execute(&mut *transaction)
        .await?;
    if let Some(receipt) = replay(&mut transaction, request).await? {
        transaction.commit().await?;
        return Ok(receipt);
    }
    let current = current_target(&mut *transaction, period).await?;
    let current_ref = current.as_ref().map(|revision| revision.revision_ref);
    let (outcome, after) = if current_ref != request.expected_revision {
        ("stale", current)
    } else if current
        .as_ref()
        .is_some_and(|revision| revision.target_value == request.target_value)
    {
        ("unchanged", current)
    } else {
        let revision_ref = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO linggan_ops_target_revision (revision_ref, period_type, period_key, metric, \
               target_value, previous_revision_ref, request_ref, set_by) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(revision_ref)
        .bind(period.period_type)
        .bind(&period.period_key)
        .bind(METRIC)
        .bind(request.target_value)
        .bind(current_ref)
        .bind(request.request_ref)
        .bind(SET_BY)
        .execute(&mut *transaction)
        .await?;
        ("accepted", current_target(&mut *transaction, period).await?)
    };
    sqlx::query(
        "INSERT INTO linggan_ops_target_request (request_ref, period_type, period_key, metric, \
           requested_value, expected_revision_ref, outcome, current_revision_ref, requested_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(request.request_ref)
    .bind(period.period_type)
    .bind(&period.period_key)
    .bind(METRIC)
    .bind(request.target_value)
    .bind(request.expected_revision)
    .bind(outcome)
    .bind(after.as_ref().map(|revision| revision.revision_ref))
    .bind(SET_BY)
    .execute(&mut *transaction)
    .await
    .map_err(|error| match &error {
        sqlx::Error::Database(db) if db.is_unique_violation() => OpsTargetError::RequestConflict,
        _ => OpsTargetError::Database(error),
    })?;
    transaction.commit().await?;
    Ok(OpsTargetReceipt {
        request_id: request.request_ref,
        outcome: outcome.to_owned(),
        replayed: false,
        current: after,
    })
}

/// A request id seen before returns its first receipt, with the revision that receipt named; the
/// same id with a different body is a conflict, never a second save.
async fn replay(
    transaction: &mut sqlx::PgConnection,
    request: &SetTargetRequest,
) -> Result<Option<OpsTargetReceipt>, OpsTargetError> {
    let Some(row) = sqlx::query(
        "SELECT period_type, period_key, requested_value, expected_revision_ref, outcome, \
                current_revision_ref \
           FROM linggan_ops_target_request WHERE request_ref = $1",
    )
    .bind(request.request_ref)
    .fetch_optional(&mut *transaction)
    .await?
    else {
        return Ok(None);
    };
    let same = row.try_get::<String, _>("period_type")? == request.period.period_type
        && row.try_get::<String, _>("period_key")? == request.period.period_key
        && row.try_get::<i32, _>("requested_value")? == request.target_value
        && row.try_get::<Option<Uuid>, _>("expected_revision_ref")? == request.expected_revision;
    if !same {
        return Err(OpsTargetError::RequestConflict);
    }
    let named: Option<Uuid> = row.try_get("current_revision_ref")?;
    let current = match named {
        Some(revision_ref) => sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT {REVISION_COLUMNS} FROM linggan_ops_target_revision r WHERE r.revision_ref = $1"
        )))
        .bind(revision_ref)
        .fetch_optional(&mut *transaction)
        .await?
        .as_ref()
        .map(revision)
        .transpose()?,
        None => None,
    };
    Ok(Some(OpsTargetReceipt {
        request_id: request.request_ref,
        outcome: row.try_get("outcome")?,
        replayed: true,
        current,
    }))
}

#[cfg(test)]
mod tests {
    use super::TargetPeriod;

    #[test]
    fn only_iso_weeks_and_months_are_target_periods() {
        for (period_type, key) in [
            ("week", "2026-W42"),
            ("week", "2026-W01"),
            ("month", "2026-10"),
        ] {
            assert!(TargetPeriod::parse(period_type, key).is_ok(), "{key}");
        }
        for (period_type, key) in [
            ("week", "2026-W00"),
            ("week", "2026-W54"),
            ("week", "2026-42"),
            ("week", "2026-W4"),
            ("month", "2026-13"),
            ("month", "2026-00"),
            ("month", "2026-W42"),
            ("day", "2026-10-13"),
            ("week", "２０２６-W42"),
        ] {
            assert!(
                TargetPeriod::parse(period_type, key).is_err(),
                "{period_type} {key}"
            );
        }
    }
}
