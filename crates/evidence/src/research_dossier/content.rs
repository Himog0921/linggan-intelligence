//! Lossless content shapes shared by admission, recognition, human edits and exports.
use serde_json::Value;
use std::collections::BTreeSet;

mod ocr;
mod text;
pub use ocr::ocr_blocks;
pub use text::{block_text, blocks_from_text, speaker_label};

/// Gap labels describe missing recognition, not words supplied by the source.
pub fn has_source_text(block: &Value) -> bool {
    block["kind"] != "gap" && !block_text(block).trim().is_empty()
}

pub const RULE_VERSION: &str = "dossier-content.v2";

pub fn validate_blocks(blocks: &[Value]) -> Result<(), String> {
    let mut refs = BTreeSet::new();
    for block in blocks {
        validate_block(block)?;
        if let Some(reference) = block.get("ref") {
            let reference = reference
                .as_str()
                .filter(|s| !s.is_empty())
                .ok_or("block_ref_invalid")?;
            if !refs.insert(reference) {
                return Err("duplicate_block_ref".into());
            }
        }
    }
    Ok(())
}

fn validate_block(block: &Value) -> Result<(), String> {
    let object = block.as_object().ok_or("block_object_required")?;
    if object.keys().any(|key| {
        !matches!(
            key.as_str(),
            "ref"
                | "kind"
                | "text"
                | "items"
                | "ordered"
                | "start"
                | "columns"
                | "rows"
                | "turns"
                | "speaker"
                | "bbox"
                | "time"
                | "issues"
                | "level"
        )
    }) {
        return Err("unknown_block_field".into());
    }
    let kind = block["kind"].as_str().ok_or("block_kind_required")?;
    let shapes = ["text", "items", "columns", "turns"]
        .iter()
        .filter(|field| block.get(**field).is_some())
        .count();
    if shapes != 1 {
        return Err("exactly_one_block_content_shape_required".into());
    }
    match kind {
        "heading" | "paragraph" | "quote" | "gap" => {
            if !block["text"].is_string() {
                return Err("block_text_required".into());
            }
        }
        "list" => {
            string_array(&block["items"])?;
            if !block["ordered"].is_boolean() {
                return Err("list_order_required".into());
            }
            if block
                .get("start")
                .is_some_and(|v| v.as_u64().is_none_or(|n| n == 0))
            {
                return Err("list_start_invalid".into());
            }
        }
        "table" => validate_table(block)?,
        "dialogue" => validate_turns(&block["turns"])?,
        _ => return Err("block_kind_invalid".into()),
    }
    if kind != "list" && (block.get("ordered").is_some() || block.get("start").is_some()) {
        return Err("list_fields_on_other_block".into());
    }
    if kind != "table" && block.get("rows").is_some() {
        return Err("rows_on_non_table".into());
    }
    if let Some(level) = block.get("level") {
        if kind != "heading" || level.as_u64().is_none_or(|n| !(1..=6).contains(&n)) {
            return Err("heading_level_invalid".into());
        }
    }
    if let Some(speaker) = block.get("speaker") {
        validate_speaker(speaker)?;
    }
    validate_location(block)?;
    if let Some(issues) = block.get("issues") {
        validate_issues(issues)?;
    }
    Ok(())
}

fn string_array(value: &Value) -> Result<(), String> {
    if value
        .as_array()
        .is_none_or(|a| a.is_empty() || a.iter().any(|v| !v.is_string()))
    {
        return Err("nonempty_string_array_required".into());
    }
    Ok(())
}

fn validate_table(block: &Value) -> Result<(), String> {
    string_array(&block["columns"])?;
    let columns = block["columns"].as_array().expect("validated").len();
    let rows = block["rows"].as_array().ok_or("table_rows_required")?;
    if rows.iter().any(|row| {
        row.as_array()
            .is_none_or(|cells| cells.len() != columns || cells.iter().any(|c| !c.is_string()))
    }) {
        return Err("table_row_width_mismatch".into());
    }
    Ok(())
}

fn validate_speaker(value: &Value) -> Result<(), String> {
    if !matches!(value.as_str(), Some("author" | "other" | "unconfirmed")) {
        return Err("speaker_invalid".into());
    }
    Ok(())
}

fn validate_turns(value: &Value) -> Result<(), String> {
    let turns = value
        .as_array()
        .filter(|a| !a.is_empty())
        .ok_or("dialogue_turns_required")?;
    for turn in turns {
        let object = turn.as_object().ok_or("dialogue_turn_invalid")?;
        if object
            .keys()
            .any(|k| !matches!(k.as_str(), "speaker" | "text"))
            || !turn["text"].is_string()
        {
            return Err("dialogue_turn_invalid".into());
        }
        validate_speaker(&turn["speaker"])?;
    }
    Ok(())
}

pub fn validate_issues(value: &Value) -> Result<(), String> {
    let issues = value.as_array().ok_or("issues_array_required")?;
    let mut keys = BTreeSet::new();
    for issue in issues {
        let object = issue.as_object().ok_or("issue_object_required")?;
        if object.keys().any(|k| {
            !matches!(
                k.as_str(),
                "key" | "code" | "message" | "resolution" | "blockRef"
            )
        }) {
            return Err("unknown_issue_field".into());
        }
        for key in ["key", "code", "message"] {
            if issue[key].as_str().is_none_or(|s| s.is_empty()) {
                return Err("issue_text_required".into());
            }
        }
        if !keys.insert(issue["key"].as_str().expect("validated")) {
            return Err("duplicate_issue_key".into());
        }
        if !matches!(
            issue["resolution"].as_str(),
            Some("reading_edit" | "source_refresh")
        ) {
            return Err("issue_resolution_invalid".into());
        }
        if issue
            .get("blockRef")
            .is_some_and(|v| v.as_str().is_none_or(str::is_empty))
        {
            return Err("issue_block_ref_invalid".into());
        }
    }
    Ok(())
}

fn validate_location(block: &Value) -> Result<(), String> {
    if let Some(bbox) = block.get("bbox") {
        let coords = bbox
            .as_array()
            .filter(|a| a.len() == 4)
            .ok_or("bbox_invalid")?;
        let mut numbers = [0.0; 4];
        for (index, value) in coords.iter().enumerate() {
            numbers[index] = value
                .as_f64()
                .filter(|n| n.is_finite() && (0.0..=1.0).contains(n))
                .ok_or("bbox_invalid")?;
        }
        if numbers[2] <= 0.0
            || numbers[3] <= 0.0
            || numbers[0] + numbers[2] > 1.000001
            || numbers[1] + numbers[3] > 1.000001
        {
            return Err("bbox_invalid".into());
        }
    }
    if let Some(time) = block.get("time") {
        let object = time.as_object().ok_or("time_invalid")?;
        if object.len() != 2
            || object
                .keys()
                .any(|k| !matches!(k.as_str(), "startMs" | "endMs"))
        {
            return Err("time_invalid".into());
        }
        let start = time["startMs"].as_u64().ok_or("time_invalid")?;
        let end = time["endMs"].as_u64().ok_or("time_invalid")?;
        if start > end {
            return Err("time_invalid".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn full_text_structure_and_repeated_content_are_preserved() {
        let long = format!("{}\n最后否定：并不是 30 mg。", "完整原文🙂".repeat(20000));
        let blocks = blocks_from_text(&long, "author");
        assert_eq!(block_text(&blocks[0]), long);
        let structured = blocks_from_text(
            "# 标题\n\n1. 不要删除否定\n2. 30 mg\n\n| 名称 | 数量 |\n| --- | --- |\n| A | 0 |\n\n> 他人观点\n\n重复\n\n重复",
            "author",
        );
        validate_blocks(&structured).unwrap();
        assert_eq!(structured[1]["kind"], "list");
        assert_eq!(structured[2]["rows"][0][1], "0");
        assert_eq!(structured[3]["speaker"], "unconfirmed");
        assert_eq!(structured.iter().filter(|b| b["text"] == "重复").count(), 2);
    }
    #[test]
    fn rejects_ambiguous_shapes_and_invented_locations() {
        for block in [
            json!({"kind":"paragraph","text":"a","items":["b"]}),
            json!({"kind":"table","columns":["a","b"],"rows":[["a"]]}),
            json!({"kind":"paragraph","text":"a","bbox":[0.8,0.1,0.5,0.2]}),
            json!({"kind":"paragraph","text":"a","time":{"startMs":5,"endMs":2}}),
        ] {
            assert!(validate_blocks(&[block]).is_err());
        }
    }
    #[test]
    fn ocr_preserves_two_columns_ui_words_and_local_uncertainty() {
        let lines = vec![
            json!({"text":"左1不删除","confidence":0.99,"bbox":[0.1,0.1,0.35,0.04]}),
            json!({"text":"右1点赞","confidence":0.99,"bbox":[0.55,0.1,0.35,0.04]}),
            json!({"text":"左2收藏","confidence":0.99,"bbox":[0.1,0.2,0.35,0.04]}),
            json!({"text":"右2不可信","confidence":0.1,"bbox":[0.55,0.2,0.35,0.04]}),
        ];
        let blocks = ocr_blocks(&lines);
        assert_eq!(blocks[0]["text"], "左1不删除");
        assert_eq!(blocks[1]["text"], "左2收藏");
        assert_eq!(blocks[2]["text"], "右1点赞");
        assert_eq!(blocks[3]["kind"], "gap");
        validate_blocks(&blocks).unwrap();
        assert!(blocks.iter().all(|b| {
            b["issues"]
                .as_array()
                .unwrap()
                .iter()
                .any(|i| i["code"] == "structure_unconfirmed")
        }));
    }
    #[test]
    fn two_column_table_never_becomes_an_unqualified_column_major_fact() {
        let lines = vec![
            json!({"text":"A","confidence":0.99,"bbox":[0.1,0.1,0.1,0.04]}),
            json!({"text":"1 mg","confidence":0.99,"bbox":[0.6,0.1,0.2,0.04]}),
            json!({"text":"B","confidence":0.99,"bbox":[0.1,0.2,0.1,0.04]}),
            json!({"text":"2 mg","confidence":0.99,"bbox":[0.6,0.2,0.2,0.04]}),
        ];
        let blocks = ocr_blocks(&lines);
        validate_blocks(&blocks).unwrap();
        for line in &lines {
            let block = blocks.iter().find(|b| b["text"] == line["text"]).unwrap();
            assert_eq!(block["bbox"], line["bbox"]);
            assert!(
                block["issues"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|i| i["code"] == "structure_unconfirmed")
            );
        }
    }
}
