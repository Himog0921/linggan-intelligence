//! Local, reproducible OCR selection. No model call and no rewritten source text.
use crate::{OcrExcludedLineInput, OcrLayeringInput, OcrLineInput};

pub const VERSION: &str = "rules-v2";

fn height(line: &OcrLineInput) -> f64 {
    line.bbox_norm[3] - line.bbox_norm[1]
}
fn width(line: &OcrLineInput) -> f64 {
    line.bbox_norm[2] - line.bbox_norm[0]
}

fn exclusion(line: &OcrLineInput) -> Option<(&'static str, &'static str)> {
    let text: String = line.text.chars().filter(|c| !c.is_whitespace()).collect();
    let b = line.bbox_norm;
    let edge = b[1] <= 0.14 || b[3] >= 0.86 || b[0] <= 0.08 || b[2] >= 0.92;
    if (text == "小红书" || text.eq_ignore_ascii_case("xiaohongshu")) && edge {
        Some(("platform_watermark", "edge_platform_watermark"))
    } else if matches!(
        text.as_str(),
        "作者赞过" | "回复" | "展开更多回复" | "发布于" | "编辑于"
    ) || ["分钟前", "小时前", "天前"].iter().any(|suffix| {
        text.strip_suffix(suffix)
            .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
    }) {
        Some(("platform_ui", "platform_ui_metadata"))
    } else if line.confidence < 0.35 {
        Some(("low_confidence", "below_local_confidence_floor"))
    } else {
        None
    }
}

fn ordered(lines: &[OcrLineInput], mut selected: Vec<usize>) -> Vec<usize> {
    selected.sort_by(|a, b| {
        lines[*a].bbox_norm[1]
            .total_cmp(&lines[*b].bbox_norm[1])
            .then_with(|| lines[*a].bbox_norm[0].total_cmp(&lines[*b].bbox_norm[0]))
    });
    selected
}

fn join(lines: &[OcrLineInput], ordinals: &[usize], separator: &str) -> Option<String> {
    (!ordinals.is_empty()).then(|| {
        ordinals
            .iter()
            .map(|i| lines[*i].text.trim())
            .collect::<Vec<_>>()
            .join(separator)
    })
}

/// Grow a connected block instead of clipping every cover to four lines around one anchor.
fn headline(lines: &[OcrLineInput], retained: &[usize]) -> Vec<usize> {
    let Some(anchor) = retained
        .iter()
        .copied()
        .filter(|i| {
            let l = &lines[*i];
            l.confidence >= 0.75 && width(l) >= 0.24 && height(l) >= 0.045
        })
        .max_by(|a, b| height(&lines[*a]).total_cmp(&height(&lines[*b])))
    else {
        return vec![];
    };
    let scale = height(&lines[anchor]);
    let candidates = ordered(
        lines,
        retained
            .iter()
            .copied()
            .filter(|i| {
                let l = &lines[*i];
                l.confidence >= 0.75
                    && width(l) >= 0.20
                    && height(l) >= scale * 0.60
                    && height(l) <= scale * 1.55
            })
            .collect(),
    );
    let mut block = vec![anchor];
    loop {
        let additions: Vec<_> = candidates
            .iter()
            .copied()
            .filter(|i| {
                !block.contains(i)
                    && block.iter().any(|j| {
                        let a = lines[*i].bbox_norm;
                        let b = lines[*j].bbox_norm;
                        let gap = (a[1] - b[3]).max(b[1] - a[3]);
                        gap >= -scale * 0.3 && gap <= scale * 1.2 && a[0].max(b[0]) < a[2].min(b[2])
                    })
            })
            .collect();
        if additions.is_empty() {
            break;
        }
        block.extend(additions);
    }
    let selected = ordered(lines, block);
    if join(lines, &selected, " ").is_some_and(|s| s.len() <= 500) {
        selected
    } else {
        vec![]
    }
}

fn body(
    lines: &[OcrLineInput],
    retained: &[usize],
    headings: &[usize],
    screenshot: bool,
) -> Vec<usize> {
    if screenshot {
        return vec![];
    }
    let candidates = ordered(
        lines,
        retained
            .iter()
            .copied()
            .filter(|i| {
                let l = &lines[*i];
                !headings.contains(i)
                    && l.confidence >= 0.75
                    && width(l) >= 0.25
                    && l.text.trim().chars().count() >= 6
                    && l.bbox_norm[1] >= 0.08
                    && l.bbox_norm[3] <= 0.92
            })
            .collect(),
    );
    if headings.is_empty() && retained.len() == 1 {
        return candidates;
    }
    // A continuous text-card block must be below its heading. Detached photographed text
    // above a large caption remains uncertain; OCR geometry cannot establish its purpose.
    let heading_bottom = headings
        .iter()
        .map(|i| lines[*i].bbox_norm[3])
        .fold(0.0, f64::max);
    let mut groups: Vec<Vec<usize>> = vec![];
    for i in candidates
        .into_iter()
        .filter(|i| lines[*i].bbox_norm[1] >= heading_bottom)
    {
        let joins = groups.last().and_then(|g| g.last()).is_some_and(|j| {
            let a = &lines[*j];
            let b = &lines[i];
            let gap = b.bbox_norm[1] - a.bbox_norm[3];
            (b.bbox_norm[0] - a.bbox_norm[0]).abs() <= 0.08
                && gap >= -0.005
                && gap <= height(a) * 2.0
                && height(b) >= height(a) * 0.65
                && height(b) <= height(a) * 1.5
        });
        if joins {
            groups.last_mut().expect("existing group").push(i);
        } else {
            groups.push(vec![i]);
        }
    }
    let selected: Vec<_> = groups
        .into_iter()
        .filter(|g| g.len() >= 3)
        .flatten()
        .collect();
    if join(lines, &selected, "\n").is_some_and(|s| s.len() <= 20_000) {
        selected
    } else {
        vec![]
    }
}

pub fn layer_lines(lines: &[OcrLineInput]) -> OcrLayeringInput {
    let mut retained = vec![];
    let mut excluded = vec![];
    for (ordinal, line) in lines.iter().enumerate() {
        if let Some((classification, reason)) = exclusion(line) {
            excluded.push(OcrExcludedLineInput {
                ordinal,
                classification: classification.into(),
                reason: reason.into(),
            });
        } else {
            retained.push(ordinal);
        }
    }
    let headings = headline(lines, &retained);
    let substantive = body(
        lines,
        &retained,
        &headings,
        excluded.iter().any(|l| l.classification == "platform_ui"),
    );
    let state = if retained.is_empty() {
        "NEEDS_REVIEW"
    } else if headings.len() + substantive.len() == retained.len() {
        "ACCEPTED"
    } else {
        "PARTIAL"
    };
    OcrLayeringInput {
        layer_version: VERSION.into(),
        state: state.into(),
        cover_headline: join(lines, &headings, " "),
        image_substantive_text: join(lines, &substantive, "\n"),
        retained_ordinals: retained,
        headline_ordinals: headings,
        substantive_ordinals: substantive,
        excluded_lines: excluded,
    }
}

pub(crate) fn has_exact_provenance(lines: &[OcrLineInput], input: &OcrLayeringInput) -> bool {
    let selected: std::collections::BTreeSet<_> = input
        .headline_ordinals
        .iter()
        .chain(&input.substantive_ordinals)
        .copied()
        .collect();
    let all: std::collections::BTreeSet<_> = input
        .retained_ordinals
        .iter()
        .copied()
        .chain(input.excluded_lines.iter().map(|l| l.ordinal))
        .collect();
    selected.len() == input.headline_ordinals.len() + input.substantive_ordinals.len()
        && selected
            .iter()
            .all(|i| *i < lines.len() && input.retained_ordinals.contains(i))
        && all.len() == lines.len()
        && all.iter().copied().eq(0..lines.len())
        && (input.state != "ACCEPTED" || selected.len() == input.retained_ordinals.len())
        && join(lines, &input.headline_ordinals, " ") == input.cover_headline
        && join(lines, &input.substantive_ordinals, "\n") == input.image_substantive_text
}

#[cfg(test)]
mod tests {
    use super::*;
    fn line(text: &str, y: f64, h: f64) -> OcrLineInput {
        OcrLineInput {
            text: text.into(),
            confidence: 0.98,
            bbox_norm: [0.12, y, 0.88, y + h],
        }
    }
    #[test]
    fn v2_rejects_invented_or_duplicate_provenance() {
        let lines = vec![line("这是一行正文内容", 0.3, 0.025)];
        let mut result = layer_lines(&lines);
        assert!(has_exact_provenance(&lines, &result));
        result.image_substantive_text = Some("机器编造的文字".into());
        assert!(!has_exact_provenance(&lines, &result));
        result.substantive_ordinals = vec![0, 0];
        result.image_substantive_text = Some("这是一行正文内容\n这是一行正文内容".into());
        assert!(!has_exact_provenance(&lines, &result));
    }

    #[test]
    fn six_line_cover_is_complete_and_reconstructable() {
        let lines: Vec<_> = (0..6)
            .map(|i| {
                line(
                    &format!("第{i}行封面文字"),
                    0.18 + f64::from(i) * 0.09,
                    0.065,
                )
            })
            .collect();
        let result = layer_lines(&lines);
        assert_eq!(result.headline_ordinals.len(), 6);
        assert_eq!(result.state, "ACCEPTED");
        assert!(has_exact_provenance(&lines, &result));
    }
    #[test]
    fn partial_card_keeps_known_body_without_promoting_uncertain_line() {
        let mut lines: Vec<_> = (0..4)
            .map(|i| {
                line(
                    &format!("这是正文第{i}行，不应丢失。"),
                    0.28 + f64::from(i) * 0.055,
                    0.025,
                )
            })
            .collect();
        lines.push(OcrLineInput {
            text: "无法判断的背景".into(),
            confidence: 0.6,
            bbox_norm: [0.01, 0.9, 0.2, 0.92],
        });
        let result = layer_lines(&lines);
        assert_eq!(result.state, "PARTIAL");
        assert_eq!(result.substantive_ordinals, vec![0, 1, 2, 3]);
        assert!(
            !result
                .image_substantive_text
                .as_ref()
                .unwrap()
                .contains("背景")
        );
        assert!(has_exact_provenance(&lines, &result));
    }
    #[test]
    fn ordinary_card_is_body_and_prose_is_not_platform_chrome() {
        let lines: Vec<_> = (0..3)
            .map(|i| {
                line(
                    "回复孩子之前先停下来想一想",
                    0.2 + f64::from(i) * 0.05,
                    0.025,
                )
            })
            .collect();
        let result = layer_lines(&lines);
        assert_eq!(result.state, "ACCEPTED");
        assert_eq!(result.substantive_ordinals.len(), 3);
        assert!(result.cover_headline.is_none());
    }
}
