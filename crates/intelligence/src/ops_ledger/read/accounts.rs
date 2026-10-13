//! 账号管理 list (OPS-HTTP-001 §3): accounts in the ledger, ordered like the workbench.

use linggan_storage_postgres::Database;
use serde::Serialize;
use sqlx::Row;
use uuid::Uuid;

use super::{OpsAsOf, OpsBusiness, OpsReadError, ensure_business, names, read_ops_as_of};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct OpsHealth {
    /// Account health is filled in by operators (Mog 2026-10-13); nothing is filled before D4b.
    pub state: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsAccountItem {
    pub r#ref: Uuid,
    pub source_id: String,
    pub platform: String,
    pub name: String,
    pub profile_url: Option<String>,
    pub device_code: Option<String>,
    pub follower_count: Option<i64>,
    pub ip_location: Option<String>,
    pub operator: Option<String>,
    pub status: Option<String>,
    pub health: OpsHealth,
    pub is_retained: Option<bool>,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpsAccountPage {
    pub as_of: Option<OpsAsOf>,
    pub total: i64,
    pub active: i64,
    pub operators: Vec<String>,
    pub items: Vec<OpsAccountItem>,
}

pub async fn read_ops_accounts(
    database: &Database,
    business: OpsBusiness,
    operator: Option<&str>,
) -> Result<OpsAccountPage, OpsReadError> {
    ensure_business(database, business).await?;
    let operator = operator.map(str::trim).filter(|text| !text.is_empty());
    let scope = sqlx::query(
        "SELECT count(*) AS total, count(*) FILTER (WHERE status IN ('active', 'warming')) AS active \
           FROM linggan_ops_account WHERE ledger_state = 'in_ledger' AND ($1::uuid IS NULL OR domain_ref = $1)",
    )
    .bind(business.domain_ref())
    .fetch_one(database.pool())
    .await?;
    let rows = sqlx::query(
        "SELECT account_ref, source_record_id, platform, account_name, profile_url, device_code, follower_count, \
                ip_location, operator_name, status, is_retained, notes \
           FROM linggan_ops_account \
          WHERE ledger_state = 'in_ledger' AND ($1::uuid IS NULL OR domain_ref = $1) \
            AND ($2::text IS NULL OR operator_name = $2) \
          ORDER BY operator_name NULLS LAST, platform, account_name, account_ref",
    )
    .bind(business.domain_ref())
    .bind(operator)
    .fetch_all(database.pool())
    .await?;
    let items = rows
        .into_iter()
        .map(|row| {
            Ok(OpsAccountItem {
                r#ref: row.try_get("account_ref")?,
                source_id: row.try_get("source_record_id")?,
                platform: row.try_get("platform")?,
                name: row.try_get("account_name")?,
                profile_url: row.try_get("profile_url")?,
                device_code: row.try_get("device_code")?,
                follower_count: row.try_get("follower_count")?,
                ip_location: row.try_get("ip_location")?,
                operator: row.try_get("operator_name")?,
                status: row.try_get("status")?,
                health: OpsHealth {
                    state: "not_filled",
                },
                is_retained: row.try_get("is_retained")?,
                notes: row.try_get("notes")?,
            })
        })
        .collect::<Result<Vec<_>, OpsReadError>>()?;
    let operators = names(
        sqlx::query(
            "SELECT DISTINCT operator_name FROM linggan_ops_account \
              WHERE ledger_state = 'in_ledger' AND operator_name IS NOT NULL \
                AND ($1::uuid IS NULL OR domain_ref = $1) ORDER BY 1",
        )
        .bind(business.domain_ref())
        .fetch_all(database.pool())
        .await?,
    )?;
    Ok(OpsAccountPage {
        as_of: read_ops_as_of(database).await?,
        total: scope.try_get("total")?,
        active: scope.try_get("active")?,
        operators,
        items,
    })
}
