//! Creative model spending, in integer cents at the API boundary and pico-USD in the ledger.
use super::AssetsError;
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreativeBudget {
    pub cap_cents: i64,
    pub revision: i32,
    pub month: String,
    pub known_pico_usd: String,
    pub reserved_pico_usd: String,
    pub unknown_calls: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChangeCreativeBudget {
    pub request_id: Uuid,
    pub expected_revision: i32,
    pub cap_cents: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreativeBudgetReceipt {
    pub cap_cents: i64,
    pub revision: i32,
    pub replayed: bool,
}

pub async fn read_creative_budget(db: &Database) -> Result<CreativeBudget, AssetsError> {
    let row = sqlx::query("SELECT (b.cap_pico_usd / 10000000000)::bigint AS cap_cents,b.revision, to_char(scope_001_now() AT TIME ZONE 'Asia/Shanghai','YYYY-MM') AS month, COALESCE(sum(i.cost_pico_usd),0)::text AS known, COALESCE(sum(r.reserved_pico_usd) FILTER(WHERE i.cost_pico_usd IS NULL),0)::text AS reserved, count(*) FILTER(WHERE i.invocation_ref IS NOT NULL AND i.cost_pico_usd IS NULL AND r.invocation_ref IS NULL) AS unknown FROM linggan_creation_ai_budget b LEFT JOIN linggan_model_invocation i ON i.task_key IN ('writing_legacy','writing_assistant','creation_title','creation_template') AND date_trunc('month',i.created_at AT TIME ZONE 'Asia/Shanghai')=date_trunc('month',scope_001_now() AT TIME ZONE 'Asia/Shanghai') LEFT JOIN linggan_creation_ai_budget_reservation r ON r.invocation_ref=i.invocation_ref WHERE b.singleton GROUP BY b.cap_pico_usd,b.revision")
        .fetch_one(db.pool()).await?;
    Ok(CreativeBudget {
        cap_cents: row.try_get("cap_cents")?,
        revision: row.try_get("revision")?,
        month: row.try_get("month")?,
        known_pico_usd: row.try_get("known")?,
        reserved_pico_usd: row.try_get("reserved")?,
        unknown_calls: row.try_get("unknown")?,
    })
}

pub async fn change_creative_budget(
    db: &Database,
    cmd: &ChangeCreativeBudget,
) -> Result<CreativeBudgetReceipt, AssetsError> {
    if cmd.expected_revision < 1 || !(0..=1_000_000_000).contains(&cmd.cap_cents) {
        return Err(AssetsError::Invalid("budget"));
    }
    let mut tx = db.pool().begin().await?;
    // The same lock also serializes cap changes with model admission.
    let revision: i32 = sqlx::query_scalar(
        "SELECT revision FROM linggan_creation_ai_budget WHERE singleton FOR UPDATE",
    )
    .fetch_one(&mut *tx)
    .await?;
    if let Some(old) = sqlx::query("SELECT expected_revision,cap_cents,result_revision FROM linggan_creation_ai_budget_change WHERE request_ref=$1")
        .bind(cmd.request_id).fetch_optional(&mut *tx).await? {
        if old.get::<i32,_>("expected_revision") != cmd.expected_revision || old.get::<i64,_>("cap_cents") != cmd.cap_cents { return Err(AssetsError::RequestConflict); }
        let receipt = CreativeBudgetReceipt { cap_cents: cmd.cap_cents, revision: old.get("result_revision"), replayed: true };
        tx.commit().await?;
        return Ok(receipt);
    }
    if revision != cmd.expected_revision {
        return Err(AssetsError::Stale);
    }
    let next = revision
        .checked_add(1)
        .ok_or(AssetsError::Invalid("budget_revision"))?;
    sqlx::query("UPDATE linggan_creation_ai_budget SET cap_pico_usd=$1::bigint::numeric*10000000000,revision=$2,updated_at=scope_001_now() WHERE singleton")
        .bind(cmd.cap_cents).bind(next).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO linggan_creation_ai_budget_change(request_ref,expected_revision,cap_cents,result_revision) VALUES($1,$2,$3,$4)")
        .bind(cmd.request_id).bind(cmd.expected_revision).bind(cmd.cap_cents).bind(next).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(CreativeBudgetReceipt {
        cap_cents: cmd.cap_cents,
        revision: next,
        replayed: false,
    })
}
