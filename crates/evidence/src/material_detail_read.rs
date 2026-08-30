//! Bounded single-work material detail projection.

use crate::material_projection::{
    MaterialLibraryItem, MaterialReadError, enrich_discovery_material, enrich_media_material,
    material_item,
};
use linggan_storage_postgres::Database;
use sqlx::Row;
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
    let engagement_rows = sqlx::query(
        "SELECT source_lane,observed_at,like_count,like_count_state,comment_count,comment_count_state, \
                collect_count,collect_count_state,share_count,share_count_state \
         FROM linggan_material_engagement_observation \
         WHERE content_public_ref=$1 AND created_at <= $2::timestamptz \
         ORDER BY observed_at::timestamptz,created_at LIMIT 100",
    )
    .bind(public_ref)
    .bind(&as_of)
    .fetch_all(&mut *tx)
    .await?;
    if let Some(inspector) = item.inspector.as_object_mut() {
        inspector.insert(
            "engagementTimeline".to_owned(),
            serde_json::Value::Array(
                engagement_rows
                    .into_iter()
                    .map(|row| {
                        serde_json::json!({
                            "sourceLane":row.get::<String,_>("source_lane"),
                            "observedAt":row.get::<String,_>("observed_at"),
                            "likeCount":row.get::<Option<i64>,_>("like_count"),
                            "likeCountState":row.get::<String,_>("like_count_state"),
                            "commentCount":row.get::<Option<i64>,_>("comment_count"),
                            "commentCountState":row.get::<String,_>("comment_count_state"),
                            "collectCount":row.get::<Option<i64>,_>("collect_count"),
                            "collectCountState":row.get::<String,_>("collect_count_state"),
                            "shareCount":row.get::<Option<i64>,_>("share_count"),
                            "shareCountState":row.get::<String,_>("share_count_state"),
                        })
                    })
                    .collect(),
            ),
        );
    }
    tx.commit().await?;
    Ok(Some(item))
}
