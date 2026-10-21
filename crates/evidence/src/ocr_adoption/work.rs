//! Ordered image transcription, kept separate from the platform-native body and author claims.
use super::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkFragment {
    pub image_ref: String,
    pub blob_sha256: String,
    pub image_ordinal: u32,
    pub start: usize,
    pub end: usize,
    pub source: Fragment,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkImageText {
    pub image_ref: String,
    pub ordinal: Option<u32>,
    pub text: String,
    pub pending_blocks: Vec<String>,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkProjection {
    pub work_ref: uuid::Uuid,
    pub source_relation: &'static str,
    pub cover_title: Option<String>,
    pub image_text: String,
    pub images: Vec<WorkImageText>,
    pub fragments: Vec<WorkFragment>,
    pub order_incomplete: bool,
}

pub fn project_work(images: &[Assessment]) -> Result<WorkProjection, InputError> {
    let work = images.first().ok_or(InputError::Identity)?.image.work_ref;
    let mut seen = std::collections::HashSet::new();
    let mut ordinals = std::collections::HashSet::new();
    for a in images {
        if a.image.work_ref != work
            || a.rule_version != RULE_VERSION
            || !seen.insert(&a.image.image_ref)
            || a.sequence_ordinal.is_some_and(|o| !ordinals.insert(o))
        {
            return Err(InputError::Identity);
        }
    }
    let mut sorted: Vec<_> = images.iter().collect();
    sorted.sort_by_key(|a| {
        (
            a.sequence_ordinal.unwrap_or(u32::MAX),
            a.image.image_ref.as_str(),
        )
    });
    let mut result = WorkProjection {
        work_ref: work,
        source_relation: "image_transcription_not_author_claim",
        cover_title: None,
        image_text: String::new(),
        images: vec![],
        fragments: vec![],
        order_incomplete: false,
    };
    for a in sorted {
        if a.cover_title.state == TitleState::Usable && result.cover_title.is_none() {
            result.cover_title = a.cover_title.text.clone();
        }
        let (text, parts) = image_text(a);
        let pending_blocks = a
            .regions
            .iter()
            .filter(|r| r.adoption_state.has_unresolved_text())
            .map(|r| r.block_ref.clone())
            .collect();
        result.images.push(WorkImageText {
            image_ref: a.image.image_ref.clone(),
            ordinal: a.sequence_ordinal,
            text: text.clone(),
            pending_blocks,
        });
        let Some(ordinal) = a.sequence_ordinal else {
            result.order_incomplete = true;
            continue;
        };
        if text.is_empty() {
            continue;
        }
        if !result.image_text.is_empty() {
            result.image_text.push_str("\n\n");
        }
        let image_start = result.image_text.chars().count();
        result.image_text.push_str(&text);
        result
            .fragments
            .extend(parts.into_iter().map(|(start, end, source)| WorkFragment {
                image_ref: a.image.image_ref.clone(),
                blob_sha256: a.image.blob_sha256.clone(),
                image_ordinal: ordinal,
                start: image_start + start,
                end: image_start + end,
                source,
            }));
    }
    Ok(result)
}

pub(super) fn image_text(a: &Assessment) -> (String, Vec<(usize, usize, Fragment)>) {
    let mut text = String::new();
    let mut parts = vec![];
    let mut seen = std::collections::HashSet::new();
    let mut regions: Vec<_> = a.regions.iter().collect();
    regions.sort_by_key(|r| r.reading_order);
    for r in regions {
        // Cover title is projected separately; legitimate interior headings remain in body.
        if !matches!(
            r.role,
            Role::TextCandidate | Role::ImageHeading | Role::OtherTitleGroup | Role::StructuredText
        ) {
            continue;
        }
        for f in &r.usable_fragments {
            let key = (f.source_kind as u8, f.source_ref.as_str(), f.start, f.end);
            if !seen.insert(key) {
                continue;
            }
            if !text.is_empty() {
                text.push('\n');
            }
            let start = text.chars().count();
            text.push_str(&f.text);
            parts.push((start, text.chars().count(), f.clone()));
        }
    }
    (text, parts)
}
