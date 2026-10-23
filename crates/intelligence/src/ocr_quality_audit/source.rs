//! One admission predicate is reused at sample freeze, dispatch, publication and readback.
use crate::ai_foundation::store::FoundationError;
use linggan_evidence::ocr_adoption::{ImageIdentity, ImageRole};
use serde_json::Value;
use sqlx::Row;
use uuid::Uuid;
pub struct Source {
    pub reading_ref: Uuid,
    pub unit_ref: Uuid,
    pub work_ref: Uuid,
    pub derivative_ref: Uuid,
    pub slot_key: String,
    pub storage_key: String,
    pub mime_type: String,
    pub byte_size: i64,
    pub image: ImageIdentity,
    pub image_role: ImageRole,
    pub sequence_ordinal: Option<u32>,
    pub blocks: Vec<Value>,
    pub issues: Vec<Value>,
    pub rule_version: String,
    pub original_url: String,
}
impl Source {
    pub fn frozen_snapshot(&self) -> Value {
        serde_json::json!({"readingRef":self.reading_ref,"unitRef":self.unit_ref,"ruleVersion":self.rule_version})
    }

    pub fn validate_snapshot(&self, snapshot: &Value) -> Result<(), FoundationError> {
        if snapshot != &self.frozen_snapshot() {
            return Err(FoundationError::Invalid);
        }
        Ok(())
    }
}
/// Frozen samples retain their exact reading even after another reading is selected.
/// Current withdrawal, bytes disposition and job retirement still revoke access.
pub async fn load_in(
    conn: &mut sqlx::PgConnection,
    id: Uuid,
) -> Result<Option<Source>, FoundationError> {
    let Some(r)=sqlx::query("SELECT r.*,u.slot_key,o.purpose,o.display_ordinal,o.display_order_state,m.blob_sha256,m.local_asset_path,b.storage_key,b.mime_type,b.byte_size,l.image_width,l.image_height FROM linggan_material_reading r JOIN linggan_material_unit u USING(unit_ref) JOIN linggan_media_materialization m ON m.materialization_ref::text=r.source_version JOIN linggan_media_download_attempt a USING(download_attempt_ref) JOIN linggan_material_media_origin o ON o.observation_ref=a.media_observation_ref AND o.slot_key=u.slot_key JOIN linggan_media_blob b ON b.sha256=m.blob_sha256 JOIN linggan_media_ocr_layout l ON l.ocr_derivative_ref=r.derivative_ref WHERE r.reading_ref=$1 AND u.kind='image' AND NOT EXISTS(SELECT 1 FROM linggan_current_material_media_disposition dis WHERE dis.slot_key=u.slot_key OR dis.blob_sha256=m.blob_sha256 OR dis.materialization_ref=m.materialization_ref OR dis.derivative_ref=r.derivative_ref) AND NOT EXISTS(SELECT 1 FROM linggan_media_derivative d JOIN linggan_media_ocr_retirement retired ON retired.retired_job_ref=d.job_ref WHERE d.derivative_ref=r.derivative_ref)")
        .bind(id).fetch_optional(conn).await? else {return Ok(None)};
    let work_ref: Uuid = r.get("content_public_ref");
    let slot_key: String = r.get("slot_key");
    let ordinal: Option<i32> = if r.get::<String, _>("display_order_state") == "KNOWN" {
        r.get("display_ordinal")
    } else {
        None
    };
    let image_role = if r.get::<String, _>("purpose") == "cover" {
        ImageRole::ExplicitCover
    } else if ordinal == Some(1) {
        ImageRole::FirstImage
    } else if ordinal.is_some() {
        ImageRole::Interior
    } else {
        ImageRole::Unknown
    };
    let blocks: Vec<Value> =
        serde_json::from_value(r.get("blocks")).map_err(|_| FoundationError::Invalid)?;
    let issues: Vec<Value> =
        serde_json::from_value(r.get("issues")).map_err(|_| FoundationError::Invalid)?;
    linggan_evidence::research_dossier::content::validate_blocks(&blocks)
        .map_err(|_| FoundationError::Invalid)?;
    linggan_evidence::research_dossier::content::validate_issues(&serde_json::json!(issues))
        .map_err(|_| FoundationError::Invalid)?;
    Ok(Some(Source {
        reading_ref: id,
        unit_ref: r.get("unit_ref"),
        work_ref,
        derivative_ref: r.get("derivative_ref"),
        slot_key: slot_key.clone(),
        storage_key: r.get("storage_key"),
        mime_type: r.get("mime_type"),
        byte_size: r.get("byte_size"),
        image: ImageIdentity {
            work_ref,
            image_ref: slot_key,
            blob_sha256: r.get("blob_sha256"),
            width: r.get::<i32, _>("image_width") as u32,
            height: r.get::<i32, _>("image_height") as u32,
        },
        image_role,
        sequence_ordinal: ordinal.map(|n| n as u32),
        blocks,
        issues,
        rule_version: r.get("rule_version"),
        original_url: format!(
            "/api/local/media/{}/{}",
            r.get::<String, _>("source_version"),
            r.get::<String, _>("blob_sha256")
        ),
    }))
}
/// Local use eligibility does not grant permission to send an image externally.
/// An operator must explicitly clear the exact immutable reading, with an audit reason.
pub async fn external_clearance_in(
    conn: &mut sqlx::PgConnection,
    id: Uuid,
) -> Result<bool, FoundationError> {
    let value: Value =
        sqlx::query_scalar("SELECT settings FROM linggan_ocr_audit_settings WHERE singleton")
            .fetch_one(&mut *conn)
            .await?;
    let settings: super::Settings =
        serde_json::from_value(value).map_err(|_| FoundationError::Invalid)?;
    settings.validate()?;
    Ok(settings.privacy_cleared_refs.contains(&id))
}
pub fn read_image(s: &Source) -> Result<crate::pi_adapter::PiImage, FoundationError> {
    use base64::Engine;
    use std::path::{Path, PathBuf};
    let key = &s.storage_key;
    if Path::new(key).is_absolute()
        || key
            .split('/')
            .any(|p| p.is_empty() || matches!(p, "." | ".."))
        || !(1..=8_000_000).contains(&s.byte_size)
    {
        return Err(FoundationError::Invalid);
    }
    let root = std::env::var_os("LINGGAN_LOCAL_MEDIA_ROOT")
        .map(PathBuf::from)
        .ok_or(FoundationError::ConfigurationIncomplete)?;
    let root = std::fs::canonicalize(root).map_err(|_| FoundationError::ConfigurationIncomplete)?;
    let file = std::fs::canonicalize(root.join(key)).map_err(|_| FoundationError::NotFound)?;
    if !file.starts_with(root) {
        return Err(FoundationError::Invalid);
    }
    let bytes = std::fs::read(file).map_err(|_| FoundationError::NotFound)?;
    if bytes.len() as i64 != s.byte_size
        || linggan_evidence::ocr_adoption::content_hash(&bytes) != s.image.blob_sha256
    {
        return Err(FoundationError::Conflict);
    }
    let format = image::guess_format(&bytes).map_err(|_| FoundationError::Invalid)?;
    let mime = match format {
        image::ImageFormat::Png => "image/png",
        image::ImageFormat::Jpeg => "image/jpeg",
        image::ImageFormat::WebP => "image/webp",
        _ => return Err(FoundationError::Invalid),
    };
    if mime != s.mime_type {
        return Err(FoundationError::Invalid);
    }
    let decoder = image::ImageReader::with_format(std::io::Cursor::new(&bytes), format);
    let (w, h) = decoder
        .into_dimensions()
        .map_err(|_| FoundationError::Invalid)?;
    if w != s.image.width || h != s.image.height || (w as u64) * (h as u64) > 40_000_000 {
        return Err(FoundationError::Invalid);
    }
    Ok(crate::pi_adapter::PiImage {
        mime_type: mime.into(),
        data: base64::engine::general_purpose::STANDARD.encode(bytes),
    })
}
/// Server-owned, procedurally drawn 42; never accepts a client image labelled synthetic.
pub fn probe_image_bytes() -> Vec<u8> {
    let mut image = image::RgbImage::from_pixel(144, 96, image::Rgb([255, 255, 255]));
    let segments = [
        (8, 0, 40, 8),
        (40, 8, 48, 32),
        (40, 40, 48, 64),
        (8, 64, 40, 72),
        (0, 40, 8, 64),
        (0, 8, 8, 32),
        (8, 32, 40, 40),
    ];
    for (offset, on) in [
        (12, [false, true, true, false, false, true, true]),
        (84, [true, true, false, true, true, false, true]),
    ] {
        for (n, &(x1, y1, x2, y2)) in segments.iter().enumerate() {
            if on[n] {
                for y in y1 + 12..y2 + 12 {
                    for x in x1 + offset..x2 + offset {
                        image.put_pixel(x, y, image::Rgb([0, 0, 0]));
                    }
                }
            }
        }
    }
    let mut bytes = std::io::Cursor::new(vec![]);
    image::DynamicImage::ImageRgb8(image)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .expect("bounded in-memory PNG encoding");
    bytes.into_inner()
}

pub fn probe_image() -> crate::pi_adapter::PiImage {
    use base64::Engine;
    crate::pi_adapter::PiImage {
        mime_type: "image/png".into(),
        data: base64::engine::general_purpose::STANDARD.encode(probe_image_bytes()),
    }
}
