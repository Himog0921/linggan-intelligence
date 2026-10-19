//! A bounded selector of current qualified writing previews, never a second copy of material.
use super::store::FoundationError;
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;

pub async fn writing_samples(db: &Database) -> Result<Value, FoundationError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let connected: bool =
        sqlx::query_scalar("SELECT to_regclass('linggan_creation_ai_preview') IS NOT NULL")
            .fetch_one(&mut *tx)
            .await?;
    if !connected {
        return Ok(json!({"state":"not_connected","samples":[],"examined":null,"limit":50}));
    }
    let rows=sqlx::query("SELECT p.preview_ref,p.work_ref,p.input_revision,p.created_at::text,
        p.input_manifest->>'action' AS action,left(r.document->>'title',120) AS title
        FROM linggan_creation_ai_preview p JOIN linggan_creation_work w ON w.work_ref=p.work_ref AND w.current_revision_ref=p.input_revision
        JOIN linggan_creation_revision r ON r.revision_ref=p.input_revision
        WHERE p.task_config_ref IS NOT NULL ORDER BY p.created_at DESC,p.preview_ref DESC LIMIT 50")
        .fetch_all(&mut *tx).await?;
    let examined = rows.len();
    let mut samples = Vec::new();
    for row in rows {
        let reference = row.get("preview_ref");
        let qualified =
            match crate::content_workbench_ai::execution::load_in(&mut tx, reference).await {
                Ok(input) => input.readable_in(&mut tx).await,
                Err(error) => Err(error),
            };
        match qualified {
            Ok(true)=>samples.push(json!({"previewRef":reference,"workRef":row.get::<uuid::Uuid,_>("work_ref"),
                "revisionRef":row.get::<uuid::Uuid,_>("input_revision"),"title":row.get::<Option<String>,_>("title"),
                "action":row.get::<String,_>("action"),"createdAt":row.get::<String,_>("created_at")})),
            Err(crate::content_workbench_ai::AiError::Database(error))=>return Err(FoundationError::Database(error)),
            _=>{},
        }
    }
    tx.commit().await?;
    Ok(json!({"state":"connected","samples":samples,"examined":examined,"limit":50}))
}
