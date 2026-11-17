//! Serialize billing pause recording with connection admission in the executor.
use super::store::FoundationError;
use sqlx::PgConnection;
use uuid::Uuid;

pub(super) const REASON: &str = "model_connection_billing_paused";

async fn lock(conn: &mut PgConnection, connection: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1::uuid::text,4020226))")
        .bind(connection)
        .execute(conn)
        .await?;
    Ok(())
}

pub(super) async fn check(
    conn: &mut PgConnection,
    connection: Uuid,
) -> Result<(), FoundationError> {
    lock(conn, connection).await?;
    let paused: bool = sqlx::query_scalar("SELECT linggan_ai_billing_paused($1)")
        .bind(connection)
        .fetch_one(conn)
        .await?;
    if paused {
        return Err(FoundationError::AdmissionDenied(vec![REASON.into()]));
    }
    Ok(())
}

pub(super) async fn record(conn: &mut PgConnection, invocation: Uuid) -> Result<(), sqlx::Error> {
    // Take the same connection lock as dispatch preparation first. If a key
    // rotation won the race, an old in-flight 402 must not pause the new key.
    let (connection, version): (Uuid, Uuid) = sqlx::query_as(
        "SELECT v.connection_ref, v.version_ref FROM linggan_model_invocation i
         JOIN linggan_model_connection_version v ON v.version_ref=i.connection_version_ref
         JOIN linggan_model_connection c USING(connection_ref)
         WHERE i.invocation_ref=$1 FOR SHARE OF c",
    )
    .bind(invocation)
    .fetch_one(&mut *conn)
    .await?;
    lock(conn, connection).await?;
    sqlx::query(
        "INSERT INTO linggan_ai_connection_billing_pause(connection_ref,invocation_ref,opened_at)
         SELECT $1,$2,clock_timestamp()
         WHERE $3=(SELECT v.version_ref FROM linggan_model_connection_version v
                   WHERE v.connection_ref=$1 ORDER BY v.revision DESC LIMIT 1)
         ON CONFLICT(connection_ref) DO UPDATE
         SET invocation_ref=EXCLUDED.invocation_ref,opened_at=EXCLUDED.opened_at",
    )
    .bind(connection)
    .bind(invocation)
    .bind(version)
    .execute(conn)
    .await?;
    Ok(())
}
