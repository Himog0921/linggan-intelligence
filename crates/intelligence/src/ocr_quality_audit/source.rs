//! One admission predicate is reused at sample freeze, dispatch, publication and readback.
use crate::ai_foundation::store::FoundationError;
use linggan_evidence::ocr_adoption::{Assessment, ImageInput};
use serde_json::Value;
use sqlx::Row;
use uuid::Uuid;
pub struct Source {
    pub adoption_ref: Uuid,
    pub work_ref: Uuid,
    pub derivative_ref: Uuid,
    pub slot_key: String,
    pub storage_key: String,
    pub mime_type: String,
    pub byte_size: i64,
    pub input: ImageInput,
    pub assessment: Assessment,
}
pub async fn load_in(
    conn: &mut sqlx::PgConnection,
    id: Uuid,
) -> Result<Option<Source>, FoundationError> {
    let Some(r) =
        sqlx::query("SELECT * FROM linggan_ocr_audit_eligible_source WHERE adoption_ref=$1")
            .bind(id)
            .fetch_optional(conn)
            .await?
    else {
        return Ok(None);
    };
    let input: ImageInput = serde_json::from_value(r.get::<Value, _>("observation"))
        .map_err(|_| FoundationError::Invalid)?;
    let assessment: Assessment = serde_json::from_value(r.get::<Value, _>("assessment"))
        .map_err(|_| FoundationError::Invalid)?;
    let work_ref: Uuid = r.get("content_public_ref");
    let slot_key: String = r.get("slot_key");
    if input.image != assessment.image
        || input.image.work_ref != work_ref
        || input.image.image_ref != slot_key
    {
        return Err(FoundationError::Invalid);
    }
    Ok(Some(Source {
        adoption_ref: id,
        work_ref,
        derivative_ref: r.get("derivative_ref"),
        slot_key,
        storage_key: r.get("storage_key"),
        mime_type: r.get("mime_type"),
        byte_size: r.get("byte_size"),
        input,
        assessment,
    }))
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
        || linggan_evidence::ocr_adoption::content_hash(&bytes) != s.input.image.blob_sha256
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
    if w != s.input.image.width || h != s.input.image.height || (w as u64) * (h as u64) > 40_000_000
    {
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
