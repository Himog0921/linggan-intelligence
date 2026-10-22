//! Deterministic suggestions only. Cloud disagreement is not an observed error.
use super::{Legibility, TitlePresence, VisualResult, VisualRole};
use linggan_evidence::ocr_adoption::{Assessment, Role};
use serde::Serialize;
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
    local: &Assessment,
    v: &VisualResult,
    snapshot: &serde_json::Value,
) -> Vec<Difference> {
    let mut out = vec![];
    let human = &snapshot["humanAdoptions"];
    let title = if human["cover_headline"].is_object() {
        human["cover_headline"]["after_text"]
            .as_str()
            .filter(|s| !s.is_empty())
    } else {
        local.cover_title.text.as_deref()
    };
    let mut add = |kind, block_ref, region_ref, local_text, suggested_text, bbox| {
        out.push(Difference {
            kind,
            block_ref,
            region_ref,
            local_text,
            suggested_text,
            bbox,
        })
    };
    if v.title_presence == TitlePresence::Absent && title.is_some() {
        add(
            "title_false_adoption",
            None,
            None,
            title.map(str::to_owned),
            None,
            None,
        );
    }
    if let Some(vision) = v.title_text.as_ref().filter(|t| Some(t.as_str()) != title) {
        add(
            if title.is_some() {
                "wrong_character"
            } else {
                "title_missing"
            },
            None,
            None,
            title.map(str::to_owned),
            Some(vision.clone()),
            None,
        );
    }
    if v.unreadable || v.title_presence == TitlePresence::Uncertain {
        add("uncertain", None, None, None, None, None);
    }
    let machine_text = local
        .regions
        .iter()
        .flat_map(|r| &r.usable_fragments)
        .map(|f| f.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let local_text = human["adopted_body"]["after_text"]
        .as_str()
        .unwrap_or(&machine_text);
    for r in &v.regions {
        if r.legibility != Legibility::Clear {
            add(
                "uncertain",
                None,
                Some(r.region_ref.clone()),
                None,
                Some(r.text.clone()),
                r.bbox,
            );
            continue;
        }
        if matches!(r.role, VisualRole::Body | VisualRole::ChapterHeading)
            && !r.text.trim().is_empty()
            && !local_text.contains(&r.text)
        {
            let same_region = r.bbox.and_then(|b| {
                local.regions.iter().find(|l| {
                    let a = [
                        l.bbox[0] / f64::from(local.image.width),
                        l.bbox[1] / f64::from(local.image.height),
                        l.bbox[2] / f64::from(local.image.width),
                        l.bbox[3] / f64::from(local.image.height),
                    ];
                    let overlap = (a[2].min(b[2]) - a[0].max(b[0])).max(0.)
                        * (a[3].min(b[3]) - a[1].max(b[1])).max(0.);
                    !l.usable_fragments.is_empty()
                        && overlap / ((b[2] - b[0]) * (b[3] - b[1])) >= 0.6
                })
            });
            add(
                if same_region.is_some() {
                    "wrong_character"
                } else {
                    "body_missing"
                },
                same_region.map(|l| l.block_ref.clone()),
                Some(r.region_ref.clone()),
                same_region.map(|l| {
                    l.usable_fragments
                        .iter()
                        .map(|f| f.text.as_str())
                        .collect::<Vec<_>>()
                        .join("\n")
                }),
                Some(r.text.clone()),
                r.bbox,
            );
        }
        if matches!(
            r.role,
            VisualRole::Ui | VisualRole::Background | VisualRole::Subtitle
        ) && r.text.chars().count() >= 2
            && local_text.contains(&r.text)
        {
            add(
                "source_role_mismatch",
                None,
                Some(r.region_ref.clone()),
                Some(r.text.clone()),
                None,
                r.bbox,
            );
        }
    }
    for r in local
        .regions
        .iter()
        .filter(|_| !human["adopted_body"].is_object())
    {
        if !matches!(
            r.role,
            Role::TextCandidate | Role::ImageHeading | Role::OtherTitleGroup | Role::CoverTitle
        ) {
            continue;
        }
        for f in &r.usable_fragments {
            if f.text.chars().count() >= 4 && !v.regions.iter().any(|v| v.text.contains(&f.text)) {
                // Lack of matching cloud text alone cannot certify hallucination.
                add(
                    "uncertain",
                    Some(r.block_ref.clone()),
                    None,
                    Some(f.text.clone()),
                    None,
                    None,
                );
            }
        }
    }
    let positions: Vec<_> = local
        .regions
        .iter()
        .flat_map(|r| &r.usable_fragments)
        .filter_map(|f| {
            let matches: Vec<_> = v
                .regions
                .iter()
                .enumerate()
                .filter(|(_, r)| r.text == f.text)
                .collect();
            (matches.len() == 1).then(|| matches[0].0)
        })
        .collect();
    if positions.windows(2).any(|p| p[0] > p[1]) {
        add("reading_order", None, None, None, None, None);
    }
    out.truncate(200);
    out
}
