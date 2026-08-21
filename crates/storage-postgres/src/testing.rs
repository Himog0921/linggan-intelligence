//! Proof-database helpers. These only ever touch a schema inside the disposable proof database
//! created by `scripts/test-scope-001-postgres.sh`; they never reset the development database.

use sqlx::{AssertSqlSafe, Connection, PgConnection};

use crate::error::StorageError;
use crate::pool::{Database, validated_schema_name};

/// Creates one isolated schema in the proof database, applies the given migration inside it and
/// returns a database whose every connection is pinned to that schema.
pub async fn isolated_proof_schema(
    proof_database_url: &str,
    schema: &str,
    migration_sql: &str,
) -> Result<Database, StorageError> {
    let schema = validated_schema_name(schema)?;
    let mut admin = PgConnection::connect(proof_database_url)
        .await
        .map_err(StorageError::Connect)?;
    sqlx::raw_sql(AssertSqlSafe(format!(
        "DROP SCHEMA IF EXISTS {schema} CASCADE; CREATE SCHEMA {schema};"
    )))
    .execute(&mut admin)
    .await
    .map_err(StorageError::Statement)?;
    admin.close().await.map_err(StorageError::Statement)?;

    let database = Database::connect_within_schema(proof_database_url, schema).await?;
    sqlx::raw_sql(AssertSqlSafe(migration_sql.to_owned()))
        .execute(database.pool())
        .await
        .map_err(StorageError::Statement)?;
    Ok(database)
}
