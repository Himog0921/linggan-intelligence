//! Storage-key grammar shared by media admission and derivative completion.

use std::path::Path;

const DEFAULT_MAX_LOCAL_ASSET_BYTES: i64 = 256 * 1024 * 1024;

pub(crate) fn maximum_local_asset_bytes() -> i64 {
    std::env::var("LINGGAN_LOCAL_MEDIA_MAX_BYTES")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_MAX_LOCAL_ASSET_BYTES)
}

pub(crate) fn is_safe_storage_key(value: &str) -> bool {
    let path = Path::new(value);
    !value.is_empty()
        && !path.is_absolute()
        && value
            .split('/')
            .all(|component| !component.is_empty() && !matches!(component, "." | ".."))
}

pub(crate) fn is_safe_media_contract(mime_type: &str, byte_size: i64) -> bool {
    (1..=maximum_local_asset_bytes()).contains(&byte_size)
        && !mime_type.trim().is_empty()
        && mime_type.len() <= 255
        && mime_type.is_ascii()
        && !mime_type.bytes().any(|byte| byte.is_ascii_control())
}

pub(crate) fn safe_inline_mime(mime_type: &str) -> Option<&str> {
    matches!(
        mime_type,
        "image/jpeg"
            | "image/png"
            | "image/webp"
            | "video/mp4"
            | "video/webm"
            | "video/quicktime"
            | "audio/mpeg"
            | "audio/mp4"
            | "audio/wav"
            | "audio/ogg"
            | "text/plain; charset=utf-8"
    )
    .then_some(mime_type)
}

pub(crate) fn derivative_mime(kind: &str) -> Option<&'static str> {
    match kind {
        "thumbnail" => Some("image/jpeg"),
        "audio" => Some("audio/mpeg"),
        "ocr_text" | "asr_text" | "frame_ocr_text" => Some("text/plain; charset=utf-8"),
        _ => None,
    }
}
