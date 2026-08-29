pub(crate) fn derivative_matches_processor(processor: &str, derivative: &str) -> bool {
    matches!(
        (processor, derivative),
        ("thumbnail", "thumbnail")
            | ("image_ocr", "ocr_text")
            | ("audio_extract", "audio")
            | ("asr", "asr_text")
            | ("video_frame_ocr", "frame_ocr_text")
    )
}

pub(crate) fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
