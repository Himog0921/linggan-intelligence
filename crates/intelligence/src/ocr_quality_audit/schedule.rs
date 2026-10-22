//! A lightweight tick inside the existing AI worker; no external cron or service.
use super::{
    Settings,
    store::{StartRun, start_run_kind},
};
use crate::ai_foundation::store::FoundationError;
use linggan_storage_postgres::Database;
use serde_json::Value;
pub async fn tick(db: &Database) -> Result<Option<Value>, FoundationError> {
    let present: bool =
        sqlx::query_scalar("SELECT to_regclass('linggan_ocr_audit_settings') IS NOT NULL")
            .fetch_one(db.pool())
            .await?;
    if !present {
        return Ok(None);
    }
    let enabled:bool=sqlx::query_scalar("SELECT count(*)=3 AND bool_and(enabled) FROM linggan_ai_control WHERE scope_key IN ('global','executor','task:ocr_quality_audit')").fetch_one(db.pool()).await?;
    if !enabled {
        return Ok(None);
    }
    let value: Value =
        sqlx::query_scalar("SELECT settings FROM linggan_ocr_audit_settings WHERE singleton")
            .fetch_one(db.pool())
            .await?;
    let settings: Settings = serde_json::from_value(value).map_err(|_| FoundationError::Invalid)?;
    if !settings.automatic {
        return Ok(None);
    }
    let week:Option<String>=sqlx::query_scalar("SELECT CASE WHEN scope_001_now() AT TIME ZONE 'Asia/Shanghai'>=date_trunc('week',scope_001_now() AT TIME ZONE 'Asia/Shanghai')+interval '9 hours' THEN to_char(date_trunc('week',scope_001_now() AT TIME ZONE 'Asia/Shanghai'),'YYYY-MM-DD') END").fetch_one(db.pool()).await?;
    let Some(week) = week else { return Ok(None) };
    let key = format!("weekly:{week}");
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_ocr_audit_run WHERE run_key=$1)")
            .bind(&key)
            .fetch_one(db.pool())
            .await?;
    if exists {
        return Ok(None);
    }
    let hash = crate::research_text::content_hash(&format!("ocr-audit-schedule:{key}"));
    let id = uuid::Uuid::from_u128(
        u128::from_str_radix(&hash[..32], 16).map_err(|_| FoundationError::Invalid)?,
    );
    start_run_kind(
        db,
        &StartRun {
            command_ref: id,
            run_key: key,
            seed: format!("week:{week}"),
            visual_probe: false,
        },
        "weekly",
    )
    .await
    .map(Some)
}
