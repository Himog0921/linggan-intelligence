//! Deterministic adoption of saved OCR observations. No I/O, inference, or persistence.
//!
//! Layout labels describe the model's guess, not the author's cover title. An accepted
//! projection remains a machine decision; it never certifies truth or author attribution.
mod partial;
mod quality;
mod title;
mod types;
mod ui;
mod validation;
mod work;

pub use types::*;
pub use work::{WorkFragment, WorkImageText, WorkProjection, WorkSegment, project_work};

pub const RULE_VERSION: &str = "ocr-adoption-v6.1";

pub fn assess(input: &ImageInput) -> Result<Assessment, InputError> {
    validation::validate(input)?;
    let mut regions = quality::assess_regions(input);
    let cover_title = title::select(input, &mut regions);
    Ok(Assessment {
        rule_version: RULE_VERSION.into(),
        input_fingerprint: content_hash(
            &[
                RULE_VERSION.as_bytes(),
                b"\n",
                &serde_json::to_vec(input).map_err(|_| InputError::Encoding)?,
            ]
            .concat(),
        ),
        image: input.image.clone(),
        image_role: input.image_role,
        sequence_ordinal: input.sequence_ordinal,
        pipeline: input.layout.pipeline.clone(),
        anchor_pipeline: input.anchors.pipeline.clone(),
        execution_completed: input.layout.completed,
        cover_title,
        regions,
    })
}

pub fn content_hash(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A display projection only. This function never writes to a platform-title field.
pub fn display_title<'a>(
    platform: Option<&'a str>,
    assessment: &'a Assessment,
) -> DisplayTitle<'a> {
    if let Some(text) = platform.filter(|s| !s.trim().is_empty()) {
        DisplayTitle {
            source: DisplaySource::PlatformTitle,
            text: Some(text),
        }
    } else if assessment.cover_title.state == TitleState::Usable {
        DisplayTitle {
            source: DisplaySource::CoverOcr,
            text: assessment.cover_title.text.as_deref(),
        }
    } else {
        DisplayTitle {
            source: DisplaySource::Unknown,
            text: None,
        }
    }
}

/// Derives visual order for a single crop without changing its raw line ordinals.
pub(crate) fn ordered_line_text(lines: &[Line]) -> String {
    quality::visual_order(lines.iter().collect())
        .iter()
        .map(|l| l.text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}
