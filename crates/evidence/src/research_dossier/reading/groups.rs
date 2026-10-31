//! Reference-only presentation groups over one frozen canonical block order.
use super::super::{ResearchDossier, content};
use crate::creator_discovery::hash;
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy)]
struct Located<'a> {
    material: &'a Value,
    block: &'a Value,
    index: usize,
}

#[derive(Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Mode {
    Blocks,
    Paragraph,
    TranscriptParagraph,
    List,
    Table,
    Tags,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Group {
    #[serde(rename = "ref")]
    reference: String,
    mode: Mode,
    block_refs: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    joiners: Option<Vec<&'static str>>,
    continuation: Continuation,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Continuation {
    from_previous: bool,
    to_next: bool,
}

#[derive(Serialize)]
struct Section {
    #[serde(rename = "ref")]
    reference: String,
    kind: &'static str,
    groups: Vec<Group>,
    gaps: Vec<SectionGap>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SectionGap {
    before_group_ref: Option<String>,
    missing_pages: Vec<u64>,
}

fn section_kind(material: &Value) -> &'static str {
    match material["kind"].as_str() {
        Some("image") => "images",
        Some("audio") => "audio",
        Some("video_frame") => "frames",
        _ => "publication",
    }
}

fn role(block: &Value) -> &str {
    block["speaker"].as_str().unwrap_or("unconfirmed")
}

fn clean(value: &Value) -> bool {
    value["issues"].as_array().is_none_or(Vec::is_empty)
}

fn text(block: &Value) -> &str {
    block["text"].as_str().unwrap_or("")
}

fn identity(dossier: &ResearchDossier, mode: Mode, refs: &[String]) -> String {
    format!(
        "{}.group.{}",
        dossier.work_ref,
        hash(&json!(["reading-groups.v1", mode, refs]).to_string())
    )
}

pub(super) fn sections(dossier: &ResearchDossier) -> Value {
    let located: BTreeMap<_, _> = dossier
        .materials
        .iter()
        .flat_map(|material| {
            material["blocks"]
                .as_array()
                .into_iter()
                .flatten()
                .enumerate()
                .filter_map(move |(index, block)| {
                    block["ref"].as_str().map(|reference| {
                        (
                            reference,
                            Located {
                                material,
                                block,
                                index,
                            },
                        )
                    })
                })
        })
        .collect();
    let included: BTreeSet<_> = ["mainBlockRefs", "supplementBlockRefs"]
        .into_iter()
        .flat_map(|key| dossier.reading_view[key].as_array().into_iter().flatten())
        .filter_map(Value::as_str)
        .collect();
    let ordered: Vec<_> = ["publication", "images", "audio", "frames"]
        .into_iter()
        .flat_map(|kind| {
            dossier
                .materials
                .iter()
                .filter(move |material| section_kind(material) == kind)
                .flat_map(ordered_blocks)
                .filter_map(|block| block["ref"].as_str())
        })
        .filter(|reference| included.contains(reference))
        .collect();
    let mut sections: Vec<Section> = vec![];
    for reference in ordered {
        let Some(current) = located.get(reference).copied() else {
            continue;
        };
        let kind = section_kind(current.material);
        if sections.last().is_none_or(|section| section.kind != kind) {
            sections.push(Section {
                reference: format!(
                    "{}.section.{}",
                    dossier.work_ref,
                    hash(&json!(["reading-groups.v1", kind, reference]).to_string())
                ),
                kind,
                groups: vec![],
                gaps: vec![],
            });
        }
        let section = sections.last_mut().expect("section was created");
        if let Some(group) = section.groups.last_mut()
            && let Some(previous) = group
                .block_refs
                .last()
                .and_then(|r| located.get(r.as_str()))
            && let Some((mode, joiner)) = continuation(group, *previous, current, dossier)
        {
            group.mode = mode;
            if let Some(joiner) = joiner {
                group.joiners.get_or_insert_with(Vec::new).push(joiner);
            } else {
                group.joiners = None;
            }
            group.block_refs.push(reference.to_owned());
            group.reference = identity(dossier, mode, &group.block_refs);
            continue;
        }
        let mode = if kind == "audio" && current.block["kind"] == "paragraph" {
            Mode::TranscriptParagraph
        } else {
            Mode::Blocks
        };
        let refs = vec![reference.to_owned()];
        section.groups.push(Group {
            reference: identity(dossier, mode, &refs),
            mode,
            block_refs: refs,
            joiners: (mode == Mode::TranscriptParagraph).then(Vec::new),
            continuation: Continuation::default(),
        });
    }
    for section in &mut sections {
        section.gaps = positioned_gaps(section, dossier, &located);
    }
    serde_json::to_value(sections).expect("reference groups are serializable")
}

/// Text coverage differs from source acquisition: a present image can still lack a reading.
pub(in crate::research_dossier) fn image_text_gap_pages(
    dossier: &ResearchDossier,
) -> BTreeSet<u64> {
    let whole_images = dossier.selection["mode"] != "explicit"
        || dossier.selection["kinds"]
            .as_array()
            .is_some_and(|kinds| kinds.contains(&json!("image")));
    let selected_pages: BTreeSet<_> = dossier.selection["imagePages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_u64)
        .chain(
            dossier
                .materials
                .iter()
                .filter(|unit| {
                    unit["kind"] == "image"
                        && dossier.selection["unitRefs"]
                            .as_array()
                            .is_some_and(|refs| refs.contains(&unit["ref"]))
                })
                .filter_map(|unit| unit["page"].as_u64()),
        )
        .collect();
    dossier.coverage["imageInventory"]["missingPages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_u64)
        .chain(
            dossier
                .materials
                .iter()
                .filter(|unit| {
                    unit["kind"] == "image"
                        && !matches!(
                            unit["sourceState"].as_str(),
                            Some("known_empty" | "not_applicable")
                        )
                        && !unit["blocks"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .any(content::has_source_text)
                })
                .filter_map(|unit| unit["page"].as_u64().filter(|page| *page > 0)),
        )
        .filter(|page| whole_images || selected_pages.contains(page))
        .collect()
}

fn positioned_gaps(
    section: &Section,
    dossier: &ResearchDossier,
    located: &BTreeMap<&str, Located<'_>>,
) -> Vec<SectionGap> {
    if section.kind != "images" || dossier.coverage["imageInventory"]["unknownOrder"] == true {
        return vec![];
    }
    let mut gaps: Vec<SectionGap> = vec![];
    for page in image_text_gap_pages(dossier) {
        let next = section
            .groups
            .iter()
            .find(|group| {
                group.block_refs.iter().any(|reference| {
                    located.get(reference.as_str()).is_some_and(|entry| {
                        entry.material["page"].as_u64().is_some_and(|p| p > page)
                    })
                })
            })
            .map(|group| group.reference.clone());
        if next.is_none()
            && section
                .groups
                .last()
                .and_then(|group| group.block_refs.last())
                .and_then(|r| located.get(r.as_str()))
                .is_none_or(|entry| entry.material["page"].as_u64().is_none())
        {
            continue;
        }
        if let Some(gap) = gaps.iter_mut().find(|gap| gap.before_group_ref == next) {
            gap.missing_pages.push(page);
        } else {
            gaps.push(SectionGap {
                before_group_ref: next,
                missing_pages: vec![page],
            });
        }
    }
    gaps
}

fn continuation(
    group: &Group,
    previous: Located<'_>,
    current: Located<'_>,
    dossier: &ResearchDossier,
) -> Option<(Mode, Option<&'static str>)> {
    let page_order_only = |material: &Value| {
        material["issues"].as_array().is_none_or(|issues| {
            issues
                .iter()
                .all(|issue| issue["code"] == "page_order_unknown")
        })
    };
    if previous.material["kind"] == "image"
        && previous.material["ref"] == current.material["ref"]
        && role(previous.block) == role(current.block)
        && clean(previous.block)
        && clean(current.block)
        && page_order_only(current.material)
        && ((previous.block["kind"] == "paragraph" && current.block["kind"] == "paragraph")
            || wrapped_heading_prose(previous, current, group.mode))
        && dossier.selection["blockRefs"]
            .as_array()
            .is_none_or(Vec::is_empty)
        && matches!(group.mode, Mode::Blocks | Mode::Paragraph)
        && (adjacent_lines(previous.block, current.block)
            || same_line(previous.block, current.block))
    {
        return Some((Mode::Paragraph, Some(joiner(previous.block, current.block))));
    }
    if role(previous.block) != role(current.block)
        || !clean(previous.block)
        || !clean(current.block)
        || !clean(previous.material)
        || !clean(current.material)
    {
        return None;
    }
    if group.mode == Mode::Blocks
        && group.block_refs.len() == 1
        && equivalent_tags(previous, current)
    {
        return Some((Mode::Tags, None));
    }
    if group.mode == Mode::TranscriptParagraph
        && previous.material["ref"] == current.material["ref"]
        && current.block["kind"] == "paragraph"
    {
        let (previous_end, start) = (
            previous.block["time"]["endMs"].as_i64()?,
            current.block["time"]["startMs"].as_i64()?,
        );
        if start < previous_end {
            return None;
        }
        let first_ref = group.block_refs.first()?;
        let first_start = previous.material["blocks"]
            .as_array()?
            .iter()
            .find(|block| block["ref"] == *first_ref)?["time"]["startMs"]
            .as_i64()?;
        // A sentence ending plus a real pause or elapsed reading span is a soft
        // layout boundary. An unfinished sentence is never cut to meet a budget.
        if closes_sentence(text(previous.block))
            && (start - previous_end >= 600 || previous_end - first_start >= 20_000)
        {
            return None;
        }
        return Some((
            Mode::TranscriptParagraph,
            Some(joiner(previous.block, current.block)),
        ));
    }
    if !adjacent_image_pages(previous, current, dossier) {
        return None;
    }
    match (
        previous.block["kind"].as_str(),
        current.block["kind"].as_str(),
    ) {
        (Some("paragraph"), Some("paragraph"))
            if matches!(group.mode, Mode::Blocks | Mode::Paragraph)
                && page_edges(previous.block, current.block)
                && unfinished_clause(text(previous.block)) =>
        {
            Some((Mode::Paragraph, Some(joiner(previous.block, current.block))))
        }
        (Some("list"), Some("list"))
            if matches!(group.mode, Mode::Blocks | Mode::List)
                && previous.block["ordered"] == true
                && current.block["ordered"] == true
                && current.block["start"].as_u64().is_some_and(|start| {
                    start
                        == previous.block["start"].as_u64().unwrap_or(1)
                            + previous.block["items"].as_array().map_or(0, Vec::len) as u64
                }) =>
        {
            Some((Mode::List, None))
        }
        (Some("table"), Some("table"))
            if matches!(group.mode, Mode::Blocks | Mode::Table)
                && previous.block["columns"] == current.block["columns"]
                && page_edges(previous.block, current.block)
                && table_rows_continue(previous.block, current.block) =>
        {
            Some((Mode::Table, None))
        }
        _ => None,
    }
}

// A tilted printed line may arrive as right/middle/left fragments. Correct only
// touching fragments on that one baseline, keeping all source blocks unchanged.
fn ordered_blocks(material: &Value) -> Vec<&Value> {
    let blocks: Vec<_> = material["blocks"]
        .as_array()
        .into_iter()
        .flatten()
        .collect();
    if material["kind"] != "image" {
        return blocks;
    }
    let mut output = vec![];
    let mut start = 0;
    while start < blocks.len() {
        let mut end = start + 1;
        while end < blocks.len() && blocks[start..end].iter().any(|b| same_line(b, blocks[end])) {
            end += 1;
        }
        let mut row = blocks[start..end].to_vec();
        row.sort_by(|a, b| {
            a["bbox"][0]
                .as_f64()
                .unwrap_or(0.0)
                .total_cmp(&b["bbox"][0].as_f64().unwrap_or(0.0))
        });
        output.extend(row);
        start = end;
    }
    output
}

fn same_line(a: &Value, b: &Value) -> bool {
    if a["kind"] != "paragraph"
        || b["kind"] != "paragraph"
        || !clean(a)
        || !clean(b)
        || role(a) != role(b)
    {
        return false;
    }
    let values = |v: &Value| -> Option<[f64; 4]> {
        Some([
            v["bbox"][0].as_f64()?,
            v["bbox"][1].as_f64()?,
            v["bbox"][2].as_f64()?,
            v["bbox"][3].as_f64()?,
        ])
    };
    let (Some(a), Some(b)) = (values(a), values(b)) else {
        return false;
    };
    let overlap = (a[1] + a[3]).min(b[1] + b[3]) - a[1].max(b[1]);
    let gap = if a[0] < b[0] {
        b[0] - a[0] - a[2]
    } else {
        a[0] - b[0] - b[2]
    };
    a[3] > 0.0 && b[3] > 0.0 && overlap >= a[3].min(b[3]) * 0.5 && (-0.005..=0.04).contains(&gap)
}

// Local OCR can label every full-width line in a large-font paragraph as a heading.
// A run of at least three wrapped heading lines ending in a same-font paragraph
// is prose; keep the original blocks and only change their presentation group.
fn wrapped_heading_prose(previous: Located<'_>, current: Located<'_>, mode: Mode) -> bool {
    if previous.block["kind"] != "heading"
        || !matches!(
            current.block["kind"].as_str(),
            Some("heading" | "paragraph")
        )
    {
        return false;
    }
    if mode == Mode::Paragraph {
        return true;
    }
    let Some(blocks) = previous.material["blocks"].as_array() else {
        return false;
    };
    let mut last = previous.block;
    let mut headings = 1;
    for next in &blocks[current.index..] {
        if !adjacent_lines(last, next) || !clean(next) || role(last) != role(next) {
            return false;
        }
        match next["kind"].as_str() {
            Some("heading") => headings += 1,
            Some("paragraph") => return headings >= 3,
            _ => return false,
        }
        last = next;
    }
    false
}

/// Join wrapped lines on one image, stopping at paragraph space or a column boundary.
pub(super) fn adjacent_lines(left: &Value, right: &Value) -> bool {
    let bbox = |value: &Value| -> Option<[f64; 4]> {
        Some([
            value["bbox"][0].as_f64()?,
            value["bbox"][1].as_f64()?,
            value["bbox"][2].as_f64()?,
            value["bbox"][3].as_f64()?,
        ])
    };
    let (Some(a), Some(b)) = (bbox(left), bbox(right)) else {
        return false;
    };
    let gap = b[1] - a[1] - a[3];
    a[2] >= 0.45
        && a[3] > 0.0
        && b[3] > 0.0
        && (a[0] - b[0]).abs() <= 0.04
        && b[3] >= a[3] * 0.6
        && b[3] <= a[3] * 1.6
        && gap >= -0.005
        && gap <= a[3].max(b[3]) * 1.2
}

fn adjacent_image_pages(left: Located<'_>, right: Located<'_>, dossier: &ResearchDossier) -> bool {
    if left.material["kind"] != "image"
        || right.material["kind"] != "image"
        || left.material["ref"] == right.material["ref"]
        || left.material["sourceState"] != "present"
        || right.material["sourceState"] != "present"
        || dossier.coverage["imageInventory"]["unknownOrder"] == true
        || dossier.selection["blockRefs"]
            .as_array()
            .is_some_and(|refs| !refs.is_empty())
        || right.index != 0
        || left.material["blocks"].as_array().map(Vec::len) != Some(left.index + 1)
    {
        return false;
    }
    matches!((left.material["page"].as_u64(), right.material["page"].as_u64()),
        (Some(a), Some(b)) if a.checked_add(1) == Some(b))
}

fn page_edges(left: &Value, right: &Value) -> bool {
    let bbox = |block: &Value| -> Option<[f64; 4]> {
        let values = block["bbox"].as_array()?;
        if values.len() != 4 {
            return None;
        }
        let result = [
            values[0].as_f64()?,
            values[1].as_f64()?,
            values[2].as_f64()?,
            values[3].as_f64()?,
        ];
        result
            .iter()
            .all(|n| n.is_finite() && (0.0..=1.0).contains(n))
            .then_some(result)
    };
    matches!((bbox(left), bbox(right)), (Some(a), Some(b)) if a[1] + a[3] >= 0.88 && b[1] <= 0.18)
}

fn table_rows_continue(left: &Value, right: &Value) -> bool {
    // Equal headers alone do not prove that two independent tables are one.
    let row_number = |row: &Value| {
        row.as_array()?
            .first()?
            .as_str()?
            .trim()
            .parse::<u64>()
            .ok()
    };
    let previous = left["rows"]
        .as_array()
        .and_then(|rows| rows.last())
        .and_then(row_number);
    let next = right["rows"]
        .as_array()
        .and_then(|rows| rows.first())
        .and_then(row_number);
    matches!((previous, next), (Some(a), Some(b)) if a.checked_add(1) == Some(b))
}

fn closes_sentence(value: &str) -> bool {
    value
        .trim_end()
        .trim_end_matches(['”', '’', '"', '\'', '」', '』', '）', ')', ']'])
        .ends_with(['。', '！', '？', '.', '!', '?', '…'])
}

fn unfinished_clause(value: &str) -> bool {
    let value = value.trim_end();
    if closes_sentence(value) {
        return false;
    }
    // Geometry proves a page boundary, not a sentence. Require an explicit
    // unfinished clause or unmatched opening delimiter as additional evidence.
    value.ends_with(['，', ',', '、', '：', ':', '；', ';'])
        || [
            ('“', '”'),
            ('「', '」'),
            ('『', '』'),
            ('（', '）'),
            ('(', ')'),
            ('[', ']'),
        ]
        .iter()
        .any(|(open, close)| value.matches(*open).count() > value.matches(*close).count())
}

fn joiner(left: &Value, right: &Value) -> &'static str {
    let a = text(left).chars().last();
    let b = text(right).chars().next();
    if a.is_some_and(char::is_whitespace) || b.is_some_and(char::is_whitespace) {
        ""
    } else if a.is_some_and(|c| c.is_ascii_alphanumeric())
        && b.is_some_and(|c| c.is_ascii_alphanumeric())
    {
        " "
    } else {
        ""
    }
}

fn tag_label(value: &str) -> &str {
    value
        .trim()
        .strip_suffix("[话题]")
        .unwrap_or(value.trim())
        .trim()
}

fn pure_tags(mut value: &str) -> Option<Vec<String>> {
    let mut tags = vec![];
    value = value.trim();
    while !value.is_empty() {
        value = value.strip_prefix('#')?;
        let end = value.find('#')?;
        let label = tag_label(&value[..end]);
        if label.is_empty() {
            return None;
        }
        tags.push(label.to_owned());
        value = value[end + 1..].trim_start();
    }
    (!tags.is_empty()).then_some(tags)
}

fn equivalent_tags(body: Located<'_>, tags: Located<'_>) -> bool {
    if body.material["kind"] != "body"
        || tags.material["kind"] != "tags"
        || body.block["kind"] != "paragraph"
        || tags.block["kind"] != "list"
        || tags.block["ordered"] != false
        || body.material["blocks"].as_array().map(Vec::len) != Some(1)
        || tags.material["blocks"].as_array().map(Vec::len) != Some(1)
        || body.material["sourceRef"]
            .as_str()
            .is_none_or(str::is_empty)
        || body.material["sourceRef"] != tags.material["sourceRef"]
    {
        return false;
    }
    let Some(items) = tags.block["items"].as_array() else {
        return false;
    };
    let Some(items) = items
        .iter()
        .map(|v| v.as_str().map(|s| tag_label(s).to_owned()))
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };
    pure_tags(text(body.block)).is_some_and(|body_tags| body_tags == items)
}

pub(in crate::research_dossier) fn tags_for_group(
    dossier: &ResearchDossier,
    group: &Value,
) -> Vec<String> {
    let refs: BTreeSet<_> = group["blockRefs"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    for material in &dossier.materials {
        if material["kind"] == "tags" {
            for block in material["blocks"].as_array().into_iter().flatten() {
                if block["ref"].as_str().is_some_and(|r| refs.contains(r)) {
                    return block["items"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .map(|s| tag_label(s).to_owned())
                        .collect();
                }
            }
        }
    }
    dossier
        .materials
        .iter()
        .flat_map(|m| m["blocks"].as_array().into_iter().flatten())
        .find(|block| block["ref"].as_str().is_some_and(|r| refs.contains(r)))
        .and_then(|block| pure_tags(text(block)))
        .unwrap_or_default()
}

/// Clip a frozen transport plan. Never rebuild priorities or generate new group identities.
pub(in crate::research_dossier) fn clip_sections(frozen: &Value, selected: &[String]) -> Value {
    let selected: BTreeSet<_> = selected.iter().map(String::as_str).collect();
    let sections: Vec<_> = frozen["sections"].as_array().into_iter().flatten().filter_map(|section| {
        let groups: Vec<_> = section["groups"].as_array().into_iter().flatten().filter_map(|group| {
            let refs = group["blockRefs"].as_array()?;
            let kept: Vec<_> = refs.iter().enumerate().filter(|(_, r)| r.as_str().is_some_and(|r| selected.contains(r))).collect();
            let first = kept.first()?.0;
            let last = kept.last()?.0;
            let mut clipped = group.clone();
            clipped["blockRefs"] = json!(kept.iter().map(|(_, r)| *r).collect::<Vec<_>>());
            clipped["continuation"] = json!({"fromPrevious":group["continuation"]["fromPrevious"] == true || first > 0,
                "toNext":group["continuation"]["toNext"] == true || last + 1 < refs.len()});
            if let Some(joiners) = group["joiners"].as_array() {
                // A segment is a contiguous slice of the frozen reference sequence.
                clipped["joiners"] = json!(joiners[first..last].to_vec());
            }
            Some(clipped)
        }).collect();
        if groups.is_empty() { return None; }
        let mut clipped = section.clone();
        clipped["gaps"] = clip_gaps(section, &groups);
        clipped["groups"] = json!(groups);
        Some(clipped)
    }).collect();
    json!(sections)
}

fn clip_gaps(section: &Value, groups: &[Value]) -> Value {
    let original = section["groups"].as_array().expect("frozen section groups");
    let gaps: Vec<_> = section["gaps"].as_array().into_iter().flatten().filter(|gap| {
        if gap["beforeGroupRef"].is_null() {
            return matches!((original.last(), groups.last()), (Some(before), Some(after))
                if before["ref"] == after["ref"] && before["blockRefs"].as_array().and_then(|r|r.last()) == after["blockRefs"].as_array().and_then(|r|r.last()));
        }
        let before = original.iter().find(|g| g["ref"] == gap["beforeGroupRef"]);
        let after = groups.iter().find(|g| g["ref"] == gap["beforeGroupRef"]);
        matches!((before, after), (Some(before), Some(after))
            if before["blockRefs"].as_array().and_then(|r|r.first()) == after["blockRefs"].as_array().and_then(|r|r.first()))
    }).cloned().collect();
    json!(gaps)
}

#[cfg(test)]
mod tests;
