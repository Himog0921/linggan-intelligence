//! Bounded single-work material detail projection.

use crate::material_projection::{
    MaterialLibraryItem, MaterialReadError, enrich_discovery_material, enrich_media_material,
    material_item,
};
use linggan_storage_postgres::Database;
use uuid::Uuid;

pub async fn read_material_detail(
    database: &Database,
    public_ref: Uuid,
) -> Result<Option<MaterialLibraryItem>, MaterialReadError> {
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(&mut *tx)
        .await?;
    let row = sqlx::query(crate::material_query_sql::MATERIAL_PAGE_SQL)
        .bind(None::<String>)
        .bind(&as_of)
        .bind(None::<String>)
        .bind(None::<String>)
        .bind(None::<String>)
        .bind(None::<String>)
        .bind(None::<String>)
        .bind(Some(public_ref))
        .fetch_optional(&mut *tx)
        .await?;
    let Some(row) = row else {
        tx.commit().await?;
        return Ok(None);
    };
    let mut item = material_item(row, None);
    enrich_discovery_material(&mut tx, &mut item, &as_of).await?;
    crate::material_social_read::enrich(&mut tx, &mut item, None, &as_of).await?;
    enrich_media_material(&mut tx, &mut item, &as_of).await?;
    tx.commit().await?;
    Ok(Some(item))
}
