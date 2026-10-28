//! Request ownership for the asset commands: one id belongs to one kind of command, a repeat of
//! the same command replays, and the same id with different content is refused.
use super::AssetsError;
use serde::Serialize;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

/// SQL assembled only from this module tree's static fragments, never from request data.
pub(super) fn sql(text: String) -> sqlx::AssertSqlSafe<String> {
    sqlx::AssertSqlSafe(text)
}

pub(super) enum Claim {
    Fresh,
    Replay,
}

pub(super) async fn lock(tx: &mut Transaction<'_, Postgres>, key: &str) -> Result<(), AssetsError> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("creation_assets:{key}"))
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Fingerprint of a command's meaning. The request id itself is not part of it.
pub(super) fn command_hash(kind: &str, command: &impl Serialize) -> Result<String, AssetsError> {
    Ok(crate::content_workbench::hash(serde_json::to_vec(
        &serde_json::json!({"kind": kind, "command": command}),
    )?))
}

pub(super) async fn claim(
    tx: &mut Transaction<'_, Postgres>,
    request: Uuid,
    kind: &str,
    sha: &str,
) -> Result<Claim, AssetsError> {
    lock(tx, &request.to_string()).await?;
    let inserted = sqlx::query(
        "INSERT INTO linggan_creation_assets_request(request_ref,kind,command_sha256) VALUES($1,$2,$3) ON CONFLICT (request_ref) DO NOTHING",
    )
    .bind(request)
    .bind(kind)
    .bind(sha)
    .execute(&mut **tx)
    .await?
    .rows_affected();
    if inserted == 1 {
        return Ok(Claim::Fresh);
    }
    let row = sqlx::query(
        "SELECT kind,command_sha256 FROM linggan_creation_assets_request WHERE request_ref=$1",
    )
    .bind(request)
    .fetch_one(&mut **tx)
    .await?;
    if row.try_get::<String, _>("kind")? == kind
        && row.try_get::<String, _>("command_sha256")? == sha
    {
        Ok(Claim::Replay)
    } else {
        Err(AssetsError::RequestConflict)
    }
}
