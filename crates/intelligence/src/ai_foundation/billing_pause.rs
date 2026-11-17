//! Serialize billing pause recording with connection admission in the executor.
use super::store::FoundationError;
use sqlx::PgConnection;
use uuid::Uuid;

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
        return Err(FoundationError::AdmissionDenied(vec![
            "model_connection_billing_paused".into(),
        ]));
    }
    Ok(())
}

pub(super) async fn record(conn: &mut PgConnection, invocation: Uuid) -> Result<(), sqlx::Error> {
    let connection: Uuid = sqlx::query_scalar(
        "SELECT v.connection_ref FROM linggan_model_invocation i
         JOIN linggan_model_connection_version v ON v.version_ref=i.connection_version_ref
         WHERE i.invocation_ref=$1",
    )
    .bind(invocation)
    .fetch_one(&mut *conn)
    .await?;
    lock(conn, connection).await?;
    sqlx::query(
        "INSERT INTO linggan_ai_connection_billing_pause(connection_ref,invocation_ref,opened_at)
         VALUES($1,$2,clock_timestamp()) ON CONFLICT(connection_ref) DO UPDATE
         SET invocation_ref=EXCLUDED.invocation_ref,opened_at=EXCLUDED.opened_at",
    )
    .bind(connection)
    .bind(invocation)
    .execute(conn)
    .await?;
    Ok(())
}
