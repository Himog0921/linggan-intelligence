//! Platform body normalization occurs at admission or an explicit development rebuild.
use super::content;
use crate::material_reading::{NewReading, Publication};
use serde_json::{Value, json};
use sqlx::PgConnection;
use uuid::Uuid;

pub async fn publish_current_body_in(
    conn: &mut PgConnection,
    work: Uuid,
) -> Result<Value, sqlx::Error> {
    sqlx::query("SELECT pg_advisory_xact_lock(420,hashtext($1::uuid::text))")
        .bind(work)
        .execute(&mut *conn)
        .await?;
    let cutoff: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(&mut *conn)
        .await?;
    let current =
        crate::work_resource_current::read_work_resource_currents_in(conn, &[work], &cutoff)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| sqlx::Error::Protocol("material_not_found".into()))?;
    let Some(source) = current.body_source.material_ref else {
        return Ok(json!({"workRef":work,"state":"not_acquired"}));
    };
    let Some(text) = current
        .body_text
        .as_deref()
        .filter(|_| current.body_state == "KNOWN")
    else {
        return Ok(json!({"workRef":work,"state":"not_acquired"}));
    };
    let existing:Option<Uuid>=sqlx::query_scalar("SELECT current_reading_ref FROM linggan_material_unit WHERE content_public_ref=$1 AND unit_key='body'")
        .bind(work).fetch_optional(&mut *conn).await?.flatten();
    let published=crate::material_reading::publish_in(conn,NewReading{
        content_public_ref:work,unit_key:"body".into(),kind:"body".into(),slot_key:None,page:None,
        source_version:source.to_string(),source_state:if text.trim().is_empty(){"known_empty"}else{"present"}.into(),
        derivative_ref:None,producer_ref:Some(source),generated_by:"platform".into(),rule_version:content::RULE_VERSION.into(),
        blocks:content::blocks_from_text(text,"author"),issues:vec![],
        source_location:json!({"materialRef":source,"packageRef":current.body_source.package_ref,"recordOrdinal":current.body_source.record_ordinal,"field":"bodyText"}),
        publication:Publication::Automatic{expected_current:existing},
    }).await?;
    Ok(
        json!({"workRef":work,"unitRef":published.unit_ref,"readingRef":published.reading_ref,"current":published.current,
        "state":if existing==Some(published.reading_ref){"already_current"}else{"updated"},"sourceVersion":source}),
    )
}
