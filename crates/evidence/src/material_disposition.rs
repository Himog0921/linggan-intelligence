//! Append-only media disposition facts owned by the Evidence boundary.

use linggan_storage_postgres::Database;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaterialMediaDisposition {
    BytesCleaned,
    WithdrawnOrRestricted,
}

impl MaterialMediaDisposition {
    fn as_str(self) -> &'static str {
        match self {
            Self::BytesCleaned => "BYTES_CLEANED",
            Self::WithdrawnOrRestricted => "WITHDRAWN_OR_RESTRICTED",
        }
    }
}

pub async fn record_materialization_disposition(
    database: &Database,
    materialization_ref: Uuid,
    disposition: MaterialMediaDisposition,
    authority_ref: &str,
    reason: &str,
) -> Result<Uuid, sqlx::Error> {
    let event_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_material_media_disposition_event \
         (event_ref,materialization_ref,state,authority_ref,reason,effective_at) \
         VALUES ($1,$2,$3,$4,$5,scope_001_now())",
    )
    .bind(event_ref)
    .bind(materialization_ref)
    .bind(disposition.as_str())
    .bind(authority_ref)
    .bind(reason)
    .execute(database.pool())
    .await?;
    Ok(event_ref)
}

pub async fn record_derivative_disposition(
    database: &Database,
    derivative_ref: Uuid,
    disposition: MaterialMediaDisposition,
    authority_ref: &str,
    reason: &str,
) -> Result<Uuid, sqlx::Error> {
    if disposition == MaterialMediaDisposition::BytesCleaned {
        return Err(sqlx::Error::Protocol(
            "BYTES_CLEANED requires a materialization target".to_owned(),
        ));
    }
    let event_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_material_media_disposition_event \
         (event_ref,derivative_ref,state,authority_ref,reason,effective_at) \
         VALUES ($1,$2,$3,$4,$5,scope_001_now())",
    )
    .bind(event_ref)
    .bind(derivative_ref)
    .bind(disposition.as_str())
    .bind(authority_ref)
    .bind(reason)
    .execute(database.pool())
    .await?;
    Ok(event_ref)
}

pub async fn record_blob_disposition(
    database: &Database,
    blob_sha256: &str,
    disposition: MaterialMediaDisposition,
    authority_ref: &str,
    reason: &str,
) -> Result<Uuid, sqlx::Error> {
    if disposition == MaterialMediaDisposition::BytesCleaned {
        return Err(sqlx::Error::Protocol(
            "BYTES_CLEANED requires a materialization target".to_owned(),
        ));
    }
    let event_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_material_media_disposition_event \
         (event_ref,blob_sha256,state,authority_ref,reason,effective_at) \
         VALUES ($1,$2,$3,$4,$5,scope_001_now())",
    )
    .bind(event_ref)
    .bind(blob_sha256)
    .bind(disposition.as_str())
    .bind(authority_ref)
    .bind(reason)
    .execute(database.pool())
    .await?;
    Ok(event_ref)
}

pub async fn record_slot_disposition(
    database: &Database,
    slot_key: &str,
    disposition: MaterialMediaDisposition,
    authority_ref: &str,
    reason: &str,
) -> Result<Uuid, sqlx::Error> {
    if disposition == MaterialMediaDisposition::BytesCleaned {
        return Err(sqlx::Error::Protocol(
            "BYTES_CLEANED requires a materialization target".to_owned(),
        ));
    }
    let event_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_material_media_disposition_event \
         (event_ref,slot_key,state,authority_ref,reason,effective_at) \
         VALUES ($1,$2,$3,$4,$5,scope_001_now())",
    )
    .bind(event_ref)
    .bind(slot_key)
    .bind(disposition.as_str())
    .bind(authority_ref)
    .bind(reason)
    .execute(database.pool())
    .await?;
    Ok(event_ref)
}
