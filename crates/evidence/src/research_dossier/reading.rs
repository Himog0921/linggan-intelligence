//! One rebuildable title and reading order over the existing immutable blocks.
use super::{ResearchDossier, content};
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub(super) fn rebuild(dossier: &mut ResearchDossier) {
    title(&mut dossier.note, &dossier.materials);
    let has_body = dossier
        .materials
        .iter()
        .any(|m| m["kind"] == "body" && readable(m));
    let video_text = dossier
        .materials
        .iter()
        .any(|m| matches!(m["kind"].as_str(), Some("audio" | "video_frame")) && readable(m));
    let mut main = vec![];
    let mut supplement = vec![];
    for material in &dossier.materials {
        let primary = if has_body {
            material["kind"] == "body"
        } else {
            match material["kind"].as_str() {
                Some("image") => material["purpose"] != "cover" || !video_text,
                Some("audio") => true,
                Some("video_frame") => !dossier.materials.iter().any(|other| {
                    other["kind"] == "audio"
                        && other["slotKey"] == material["slotKey"]
                        && readable(other)
                }),
                _ => false,
            }
        };
        let refs = if primary { &mut main } else { &mut supplement };
        if material["sourceState"] != "restricted" {
            refs.extend(
                material["blocks"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|b| b["ref"].as_str().map(str::to_owned)),
            );
        }
    }
    dossier.reading_view = json!({"mainBlockRefs":main,"supplementBlockRefs":supplement});
    // Public materials retain diagnostics. Text consumers need at least one
    // actual source block, including after an explicit selection of only gaps.
    if !dossier
        .fragments
        .iter()
        .any(|f| content::has_source_text(&f.structure))
    {
        dossier.fragments.clear();
    }
    order_fragments(dossier);
}

pub(super) fn order_fragments(dossier: &mut ResearchDossier) {
    let references = ordered_refs(dossier);
    let order: BTreeMap<_, _> = references
        .iter()
        .enumerate()
        .map(|(i, r)| (r.to_string(), i))
        .collect();
    dossier
        .fragments
        .retain(|f| f.field == "title" || order.contains_key(f.fragment_id.as_str()));
    dossier.fragments.sort_by_key(|f| {
        if f.field == "title" {
            0
        } else {
            order
                .get(f.fragment_id.as_str())
                .copied()
                .unwrap_or(usize::MAX - 1)
                + 1
        }
    });
}

fn readable(material: &Value) -> bool {
    material["sourceState"] != "restricted"
        && material["blocks"]
            .as_array()
            .into_iter()
            .flatten()
            .any(content::has_source_text)
}

fn title(note: &mut Value, materials: &[Value]) {
    if let Some(title) = note["platformTitle"]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .map(str::to_owned)
    {
        note["displayTitle"] = json!(title);
        note["displayTitleBasis"] = json!("platform");
        note["titleBlockRefs"] = json!([]);
        return;
    }
    // An explicit cover takes precedence even if it has no reliable heading.
    let cover = materials
        .iter()
        .find(|m| m["kind"] == "image" && m["purpose"] == "cover")
        .or_else(|| {
            materials
                .iter()
                .find(|m| m["kind"] == "image" && m["page"] == 1)
        });
    let blocks: Vec<_> = cover
        .filter(|m| m["sourceState"] != "restricted")
        .and_then(|m| m["blocks"].as_array())
        .into_iter()
        .flatten()
        .collect();
    let start = blocks
        .iter()
        .position(|b| b["kind"] == "heading" && !content::block_text(b).trim().is_empty());
    let headings: Vec<_> = start
        .map(|i| {
            blocks[i..]
                .iter()
                .take_while(|b| b["kind"] == "heading")
                .copied()
                .collect()
        })
        .unwrap_or_default();
    let mut text = String::new();
    for block in &headings {
        for line in content::block_text(block)
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            if text
                .chars()
                .last()
                .is_some_and(|c| c.is_ascii_alphanumeric())
                && line.starts_with(|c: char| c.is_ascii_alphanumeric())
            {
                text.push(' ');
            }
            text.push_str(line);
        }
    }
    note["displayTitle"] = if text.is_empty() {
        Value::Null
    } else {
        json!(text)
    };
    note["displayTitleBasis"] = json!(if text.is_empty() { "missing" } else { "cover" });
    note["titleBlockRefs"] = json!(
        headings
            .iter()
            .filter_map(|b| b["ref"].as_str())
            .collect::<Vec<_>>()
    );
}

pub fn ordered_refs(dossier: &ResearchDossier) -> Vec<&str> {
    ["mainBlockRefs", "supplementBlockRefs"]
        .into_iter()
        .flat_map(|key| dossier.reading_view[key].as_array().into_iter().flatten())
        .filter_map(Value::as_str)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::research_dossier::VERSION;
    use uuid::Uuid;

    fn fixture(title: bool, body: Option<&str>, video: bool) -> ResearchDossier {
        let mut materials = vec![];
        if let Some(text) = body {
            materials.push(json!({"ref":"body","kind":"body","blocks":[{"ref":"b","kind":"paragraph","text":text,"speaker":"author"}]}));
        }
        materials.push(
            json!({"ref":"cover","kind":"image","purpose":"cover","page":1,"blocks":[
            {"ref":"h1","kind":"heading","text":"封面标题\n分行","speaker":"unconfirmed"},
            {"ref":"p1","kind":"paragraph","text":"第1页不能丢失","speaker":"unconfirmed"}]}),
        );
        if video {
            materials.extend([
            json!({"ref":"audio","kind":"audio","slotKey":"video","blocks":[{"ref":"a","kind":"paragraph","text":"口播","time":{"startMs":0,"endMs":2000}}]}),
            json!({"ref":"frame","kind":"video_frame","slotKey":"video","blocks":[{"ref":"v","kind":"paragraph","text":"画面","time":{"startMs":1000,"endMs":1000}}]})]);
        }
        ResearchDossier {
            contract: VERSION,
            work_ref: Uuid::nil(),
            revision: "r".into(),
            material_revision: "m".into(),
            as_of: "t".into(),
            note: json!({"platformTitle":title.then_some("平台标题")}),
            materials,
            reading_view: Value::Null,
            coverage: json!({}),
            attachments: json!({}),
            selection: json!({"mode":"full"}),
            fragments: vec![],
            media: vec![],
            media_inventory: vec![],
            candidate_statuses: vec![],
            metadata: json!({}),
        }
    }

    #[test]
    fn t01_t03_eight_combinations_keep_every_block_and_short_body_wins() {
        for title in [false, true] {
            for body in [None, Some("🙂")] {
                for video in [false, true] {
                    let mut d = fixture(title, body, video);
                    rebuild(&mut d);
                    assert_eq!(
                        d.note["displayTitle"],
                        if title {
                            "平台标题"
                        } else {
                            "封面标题分行"
                        }
                    );
                    let refs = ordered_refs(&d);
                    assert_eq!(
                        refs.len(),
                        d.materials
                            .iter()
                            .map(|m| m["blocks"].as_array().unwrap().len())
                            .sum::<usize>()
                    );
                    let main = &d.reading_view["mainBlockRefs"];
                    if body.is_some() {
                        assert_eq!(*main, json!(["b"]));
                    } else if video {
                        assert_eq!(*main, json!(["a"]));
                    } else {
                        assert_eq!(*main, json!(["h1", "p1"]));
                    }
                }
            }
        }
        for text in ["一句话", "🙂", "#标签"] {
            let mut d = fixture(false, Some(text), false);
            rebuild(&mut d);
            assert_eq!(d.reading_view["mainBlockRefs"], json!(["b"]));
        }
    }

    #[test]
    fn t02_t07_unknown_order_no_heading_and_failed_audio_do_not_invent_content() {
        let mut d = fixture(false, None, true);
        d.materials[0]["purpose"] = json!("body_image");
        d.materials[0]["page"] = Value::Null;
        d.materials[1]["blocks"] = json!([]);
        d.materials[1]["processing"] = json!({"state":"terminal","reason":"asr_failed"});
        rebuild(&mut d);
        assert!(d.note["displayTitle"].is_null());
        assert_eq!(d.reading_view["mainBlockRefs"], json!(["h1", "p1", "v"]));
        d.materials[0]["purpose"] = json!("cover");
        d.materials[0]["blocks"][0]["kind"] = json!("paragraph");
        rebuild(&mut d);
        assert_eq!(d.note["displayTitleBasis"], "missing");
        assert_eq!(d.reading_view["mainBlockRefs"], json!(["v"]));
        d.materials[2]["blocks"] = json!([]);
        rebuild(&mut d);
        assert_eq!(d.reading_view["mainBlockRefs"], json!(["h1", "p1"]));
    }

    #[test]
    fn t05_t11_rearrangement_and_explicit_selection_remove_unselected_title() {
        let mut d = fixture(false, Some("短正文"), true);
        rebuild(&mut d);
        let original = d.materials.clone();
        super::super::select(
            &mut d,
            &super::super::Selection {
                kinds: vec!["body".into()],
                ..Default::default()
            },
        )
        .unwrap();
        assert!(d.note["displayTitle"].is_null());
        assert_eq!(ordered_refs(&d), ["b"]);
        assert_eq!(d.materials[0], original[0]);
        let mut empty = fixture(false, None, false);
        empty.materials.clear();
        rebuild(&mut empty);
        assert!(ordered_refs(&empty).is_empty());
    }
}
