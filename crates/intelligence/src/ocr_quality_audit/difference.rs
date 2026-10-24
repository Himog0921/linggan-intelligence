//! Deterministic suggestions against the frozen canonical reading. A cloud
//! disagreement is a question for review, never a correction or a truth claim.
use super::{Legibility, TitlePresence, VisualResult, VisualRole, source::Source};
use linggan_evidence::research_dossier::content::block_text;
use serde::Serialize;
use serde_json::Value;
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Difference {
    pub kind: &'static str,
    pub block_ref: Option<String>,
    pub region_ref: Option<String>,
    pub local_text: Option<String>,
    pub suggested_text: Option<String>,
    pub bbox: Option<[f64; 4]>,
}

pub fn compare(
    local: &Source,
    v: &VisualResult,
    snapshot: &Value,
) -> Result<Vec<Difference>, crate::ai_foundation::store::FoundationError> {
    local.validate_snapshot(snapshot)?;
    let mut out = vec![];
    let title = local
        .blocks
        .iter()
        .find(|b| b["kind"] == "heading")
        .map(block_text);
    if matches!(
        local.image_role,
        linggan_evidence::ocr_adoption::ImageRole::FirstImage
            | linggan_evidence::ocr_adoption::ImageRole::ExplicitCover
    ) {
        if v.title_presence == TitlePresence::Absent && title.is_some() {
            out.push(Difference {
                kind: "title_false_adoption",
                block_ref: None,
                region_ref: None,
                local_text: title.clone(),
                suggested_text: None,
                bbox: None,
            });
        }
        if let Some(vision) = v
            .title_text
            .as_ref()
            .filter(|t| Some(t.as_str()) != title.as_deref())
        {
            out.push(Difference {
                kind: if title.is_some() {
                    "wrong_character"
                } else {
                    "title_missing"
                },
                block_ref: None,
                region_ref: None,
                local_text: title,
                suggested_text: Some(vision.clone()),
                bbox: None,
            });
        }
    }
    let rendered = local
        .blocks
        .iter()
        .map(block_text)
        .collect::<Vec<_>>()
        .join("\n\n");
    for region in &v.regions {
        if region.legibility != Legibility::Clear {
            out.push(Difference {
                kind: "uncertain",
                block_ref: None,
                region_ref: Some(region.region_ref.clone()),
                local_text: None,
                suggested_text: Some(region.text.clone()),
                bbox: region.bbox,
            });
        } else if !region.text.is_empty()
            && !rendered.contains(&region.text)
            && matches!(
                region.role,
                VisualRole::Body | VisualRole::ChapterHeading | VisualRole::MainTitle
            )
        {
            let same = region
                .bbox
                .and_then(|b| local.blocks.iter().find(|l| overlaps(&l["bbox"], b)));
            out.push(Difference {
                kind: if same.is_some() {
                    "wrong_character"
                } else {
                    "body_missing"
                },
                block_ref: same.and_then(|b| b["ref"].as_str()).map(str::to_owned),
                region_ref: Some(region.region_ref.clone()),
                local_text: same.map(block_text),
                suggested_text: Some(region.text.clone()),
                bbox: region.bbox,
            });
        }
    }
    for block in &local.blocks {
        let text = block_text(block);
        if block["kind"] != "gap"
            && text.chars().count() >= 4
            && !v.regions.iter().any(|r| r.text.contains(&text))
        {
            out.push(Difference {
                kind: "uncertain",
                block_ref: block["ref"].as_str().map(str::to_owned),
                region_ref: None,
                local_text: Some(text),
                suggested_text: None,
                bbox: None,
            });
        }
    }
    let positions: Vec<_> = local
        .blocks
        .iter()
        .filter_map(|b| {
            let text = block_text(b);
            let matches: Vec<_> = v
                .regions
                .iter()
                .enumerate()
                .filter(|(_, r)| r.text == text)
                .map(|(i, _)| i)
                .collect();
            (matches.len() == 1).then(|| matches[0])
        })
        .collect();
    if positions.windows(2).any(|p| p[0] > p[1]) {
        out.push(Difference {
            kind: "reading_order",
            block_ref: None,
            region_ref: None,
            local_text: None,
            suggested_text: None,
            bbox: None,
        });
    }
    Ok(out)
}

fn overlaps(value: &Value, b: [f64; 4]) -> bool {
    let Some(a) = value.as_array().filter(|a| a.len() == 4) else {
        return false;
    };
    let values: Option<Vec<f64>> = a.iter().map(Value::as_f64).collect();
    let Some(a) = values else { return false };
    let area = (b[2] - b[0]) * (b[3] - b[1]);
    if area <= 0. {
        return false;
    }
    ((a[0] + a[2]).min(b[2]) - a[0].max(b[0])).max(0.)
        * ((a[1] + a[3]).min(b[3]) - a[1].max(b[1])).max(0.)
        / area
        >= 0.6
}
