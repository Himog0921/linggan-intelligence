//! Lossless content shapes shared by admission, recognition, human edits and exports.
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub const RULE_VERSION: &str = "dossier-content.v2";

pub fn validate_blocks(blocks: &[Value]) -> Result<(), String> {
    let mut refs = BTreeSet::new();
    for block in blocks {
        validate_block(block)?;
        if let Some(reference) = block.get("ref") {
            let reference = reference.as_str().filter(|s| !s.is_empty())
                .ok_or("block_ref_invalid")?;
            if !refs.insert(reference) { return Err("duplicate_block_ref".into()); }
        }
    }
    Ok(())
}

fn validate_block(block: &Value) -> Result<(), String> {
    let object = block.as_object().ok_or("block_object_required")?;
    if object.keys().any(|key| !matches!(key.as_str(), "ref"|"kind"|"text"|"items"|
        "ordered"|"start"|"columns"|"rows"|"turns"|"speaker"|"bbox"|"time"|"issues"|"level")) {
        return Err("unknown_block_field".into());
    }
    let kind = block["kind"].as_str().ok_or("block_kind_required")?;
    let shapes = ["text", "items", "columns", "turns"].iter()
        .filter(|field| block.get(**field).is_some()).count();
    if shapes != 1 { return Err("exactly_one_block_content_shape_required".into()); }
    match kind {
        "heading"|"paragraph"|"quote"|"gap" => {
            if !block["text"].is_string() { return Err("block_text_required".into()); }
        }
        "list" => {
            string_array(&block["items"])?;
            if !block["ordered"].is_boolean() { return Err("list_order_required".into()); }
            if block.get("start").is_some_and(|v| v.as_u64().is_none_or(|n| n == 0)) {
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
    if kind != "table" && block.get("rows").is_some() { return Err("rows_on_non_table".into()); }
    if let Some(level) = block.get("level") {
        if kind != "heading" || level.as_u64().is_none_or(|n| !(1..=6).contains(&n)) {
            return Err("heading_level_invalid".into());
        }
    }
    if let Some(speaker) = block.get("speaker") { validate_speaker(speaker)?; }
    validate_location(block)?;
    if let Some(issues) = block.get("issues") { validate_issues(issues)?; }
    Ok(())
}

fn string_array(value: &Value) -> Result<(), String> {
    if value.as_array().is_none_or(|a| a.is_empty() || a.iter().any(|v| !v.is_string())) {
        return Err("nonempty_string_array_required".into());
    }
    Ok(())
}

fn validate_table(block: &Value) -> Result<(), String> {
    string_array(&block["columns"])?;
    let columns = block["columns"].as_array().expect("validated").len();
    let rows = block["rows"].as_array().ok_or("table_rows_required")?;
    if rows.iter().any(|row| row.as_array().is_none_or(|cells|
        cells.len() != columns || cells.iter().any(|c| !c.is_string()))) {
        return Err("table_row_width_mismatch".into());
    }
    Ok(())
}

fn validate_speaker(value: &Value) -> Result<(), String> {
    if !matches!(value.as_str(), Some("author"|"other"|"unconfirmed")) {
        return Err("speaker_invalid".into());
    }
    Ok(())
}

fn validate_turns(value: &Value) -> Result<(), String> {
    let turns = value.as_array().filter(|a| !a.is_empty()).ok_or("dialogue_turns_required")?;
    for turn in turns {
        let object = turn.as_object().ok_or("dialogue_turn_invalid")?;
        if object.keys().any(|k| !matches!(k.as_str(), "speaker"|"text")) || !turn["text"].is_string() {
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
        if object.keys().any(|k| !matches!(k.as_str(), "key"|"code"|"message"|"resolution"|"blockRef")) {
            return Err("unknown_issue_field".into());
        }
        for key in ["key", "code", "message"] {
            if issue[key].as_str().is_none_or(|s| s.is_empty()) { return Err("issue_text_required".into()); }
        }
        if !keys.insert(issue["key"].as_str().expect("validated")) { return Err("duplicate_issue_key".into()); }
        if !matches!(issue["resolution"].as_str(), Some("reading_edit"|"source_refresh")) {
            return Err("issue_resolution_invalid".into());
        }
        if issue.get("blockRef").is_some_and(|v| v.as_str().is_none_or(str::is_empty)) {
            return Err("issue_block_ref_invalid".into());
        }
    }
    Ok(())
}

fn validate_location(block: &Value) -> Result<(), String> {
    if let Some(bbox) = block.get("bbox") {
        let coords = bbox.as_array().filter(|a| a.len() == 4).ok_or("bbox_invalid")?;
        let mut numbers = [0.0; 4];
        for (index, value) in coords.iter().enumerate() {
            numbers[index] = value.as_f64().filter(|n| n.is_finite() && (0.0..=1.0).contains(n))
                .ok_or("bbox_invalid")?;
        }
        if numbers[2] <= 0.0 || numbers[3] <= 0.0 || numbers[0]+numbers[2] > 1.000001
            || numbers[1]+numbers[3] > 1.000001 { return Err("bbox_invalid".into()); }
    }
    if let Some(time) = block.get("time") {
        let object = time.as_object().ok_or("time_invalid")?;
        if object.len() != 2 || object.keys().any(|k| !matches!(k.as_str(), "startMs"|"endMs")) {
            return Err("time_invalid".into());
        }
        let start = time["startMs"].as_u64().ok_or("time_invalid")?;
        let end = time["endMs"].as_u64().ok_or("time_invalid")?;
        if start > end { return Err("time_invalid".into()); }
    }
    Ok(())
}

pub fn blocks_from_text(text: &str, speaker: &str) -> Vec<Value> {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized.lines().collect();
    let mut blocks = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        if lines[index].trim().is_empty() { index += 1; continue; }
        if let Some((table, next)) = parse_table(&lines, index, speaker) {
            blocks.push(table); index = next; continue;
        }
        if let Some((level, heading)) = heading(lines[index]) {
            blocks.push(json!({"kind":"heading","level":level,"text":heading,"speaker":speaker}));
            index += 1; continue;
        }
        if let Some((ordered, start, _)) = list_item(lines[index]) {
            let mut items = Vec::new();
            while let Some(line) = lines.get(index) {
                let Some((next_ordered, number, text)) = list_item(line) else { break; };
                if next_ordered != ordered || (ordered && number != start + items.len()) { break; }
                items.push(text); index += 1;
            }
            let mut block=json!({"kind":"list","ordered":ordered,"items":items,"speaker":speaker});
            if ordered { block["start"]=json!(start); }
            blocks.push(block); continue;
        }
        let quote = lines[index].starts_with("> ");
        let mut paragraph = vec![];
        while index < lines.len() && !lines[index].trim().is_empty() {
            if !paragraph.is_empty() && (heading(lines[index]).is_some() || list_item(lines[index]).is_some()
                || parse_table(&lines,index,speaker).is_some() || lines[index].starts_with("> ") != quote) { break; }
            paragraph.push(if quote { &lines[index][2..] } else { lines[index] }); index += 1;
        }
        blocks.push(json!({"kind":if quote {"quote"} else {"paragraph"},"text":paragraph.join("\n"),
            "speaker":if quote {"unconfirmed"} else {speaker}}));
    }
    blocks
}

fn heading(line: &str) -> Option<(usize, &str)> {
    let level = line.chars().take_while(|c| *c == '#').count();
    (level > 0 && level <= 6 && line.as_bytes().get(level) == Some(&b' '))
        .then(|| (level, &line[level+1..]))
}

fn list_item(line: &str) -> Option<(bool, usize, &str)> {
    if let Some(text) = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")).or_else(|| line.strip_prefix("+ ")) {
        return Some((false, 1, text));
    }
    let digits = line.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 { return None; }
    let rest = line.get(digits..)?;
    let text = rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") "))?;
    let number = line[..digits].parse::<usize>().ok().filter(|n| *n > 0)?;
    Some((true, number, text))
}

fn table_cells(line: &str) -> Option<Vec<String>> {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') || !trimmed.ends_with('|') || trimmed.len() < 2 { return None; }
    let mut cells = vec![String::new()];
    let mut escaped = false;
    for c in trimmed[1..trimmed.len()-1].chars() {
        if escaped {
            if !matches!(c, '|' | '\\') { cells.last_mut().expect("cell").push('\\'); }
            cells.last_mut().expect("cell").push(c); escaped=false;
        }
        else if c == '\\' { escaped=true; }
        else if c == '|' { cells.push(String::new()); }
        else { cells.last_mut().expect("cell").push(c); }
    }
    if escaped { cells.last_mut().expect("cell").push('\\'); }
    Some(cells.into_iter().map(|c| c.trim().to_owned()).collect())
}

fn parse_table(lines: &[&str], index: usize, speaker: &str) -> Option<(Value, usize)> {
    let columns = table_cells(lines[index])?;
    let separators = table_cells(lines.get(index+1)?)?;
    if separators.len() != columns.len() || separators.iter().any(|c| {
        let inner=c.trim_matches(':'); inner.len()<3 || !inner.chars().all(|ch| ch=='-')
    }) { return None; }
    let mut rows = vec![];
    let mut next = index+2;
    while let Some(line) = lines.get(next) {
        let Some(cells) = table_cells(line) else { break; };
        if cells.len()!=columns.len() { break; }
        rows.push(cells); next+=1;
    }
    Some((json!({"kind":"table","columns":columns,"rows":rows,"speaker":speaker}),next))
}

pub fn block_text(block: &Value) -> String {
    match block["kind"].as_str() {
        Some("list") => block["items"].as_array().into_iter().flatten().enumerate()
            .map(|(i,v)| format!("{} {}",if block["ordered"]==true {
                format!("{}.",block["start"].as_u64().unwrap_or(1)+i as u64)
            } else { "-".into() },v.as_str().unwrap_or(""))).collect::<Vec<_>>().join("\n"),
        Some("table") => table_text(block),
        Some("dialogue") => block["turns"].as_array().into_iter().flatten()
            .map(|v| format!("{}：{}",speaker_label(v["speaker"].as_str().unwrap_or("unconfirmed")),v["text"].as_str().unwrap_or("")))
            .collect::<Vec<_>>().join("\n"),
        _ => block["text"].as_str().unwrap_or("").to_owned(),
    }
}

pub fn speaker_label(speaker: &str) -> &'static str {
    match speaker { "author"=>"作者", "other"=>"他人", _=>"角色未确认" }
}

fn table_text(block: &Value) -> String {
    let row = |values: &[Value]| format!("| {} |",values.iter().map(|v| v.as_str().unwrap_or("")
        .replace('\\',"\\\\").replace('|',"\\|").replace('\n',"<br>"))
        .collect::<Vec<_>>().join(" | "));
    let columns = block["columns"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    let mut lines = vec![row(columns),format!("| {} |",vec!["---";columns.len()].join(" | "))];
    for cells in block["rows"].as_array().into_iter().flatten() {
        lines.push(row(cells.as_array().map(Vec::as_slice).unwrap_or(&[])));
    }
    lines.join("\n")
}

/// OCR coordinates determine only reading order. They never prove author attribution.
pub fn ocr_blocks(lines: &[Value]) -> Vec<Value> {
    let ordered = reading_order(lines);
    let mut output = Vec::new();
    for (index, (line, layout_unconfirmed)) in ordered.into_iter().enumerate() {
        let text = line["text"].as_str().unwrap_or("");
        if text.trim().is_empty() { continue; }
        let uncertain = line["confidence"].as_f64().is_some_and(|c| c < 0.35);
        let kind = if uncertain { "gap" } else if line["kind"] == "heading" { "heading" } else { "paragraph" };
        let mut block=json!({"kind":kind,"text":if uncertain {"此处文字识别不可靠，待对照原图"} else {text},
            "speaker":line["speaker"].as_str().filter(|s| matches!(*s,"author"|"other"|"unconfirmed")).unwrap_or("unconfirmed")});
        if let Some(bbox) = line.get("bbox") {
            if validate_location(&json!({"bbox":bbox})).is_ok() { block["bbox"]=bbox.clone(); }
        }
        let mut issues=vec![];
        if uncertain {issues.push(json!({"key":format!("ocr_low_confidence_{index}"),"code":"low_confidence",
            "message":"仅此处识别不可靠，未推测填字","resolution":"reading_edit"}));}
        if layout_unconfirmed {issues.push(json!({"key":format!("ocr_layout_{index}"),"code":"structure_unconfirmed",
            "message":"此区域可能是多栏文字或表格；当前为待核对的阅读顺序，列间对应关系尚未确认，原始位置已保留","resolution":"reading_edit"}));}
        if !issues.is_empty(){block["issues"]=json!(issues);}
        output.push(block);
    }
    output
}

fn reading_order(lines: &[Value]) -> Vec<(&Value,bool)> {
    let mut lines: Vec<_> = lines.iter().collect();
    if lines.iter().any(|l| validate_location(&json!({"bbox":l["bbox"]})).is_err()) { return lines.into_iter().map(|line|(line,true)).collect(); }
    lines.sort_by(|a,b| coord(a,1).total_cmp(&coord(b,1)).then(coord(a,0).total_cmp(&coord(b,0))));
    let mut result=vec![];
    let mut segment=vec![];
    for line in lines {
        if coord(line,2) > 0.70 {
            order_segment(&mut segment,&mut result); result.push((line,false));
        } else { segment.push(line); }
    }
    order_segment(&mut segment,&mut result);
    result
}

fn coord(line: &Value, index: usize) -> f64 { line["bbox"][index].as_f64().unwrap_or(0.0) }

fn order_segment<'a>(segment: &mut Vec<&'a Value>, output: &mut Vec<(&'a Value,bool)>) {
    let left:Vec<_>=segment.iter().copied().filter(|l| coord(l,0)+coord(l,2)<=0.52).collect();
    let right:Vec<_>=segment.iter().copied().filter(|l| coord(l,0)>=0.52).collect();
    if left.len()>=2 && right.len()>=2 && left.len()+right.len()==segment.len() {
        output.extend(left.into_iter().chain(right).map(|line|(line,true)));
    } else { output.extend(segment.iter().copied().map(|line|(line,false))); }
    segment.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_text_structure_and_repeated_content_are_preserved() {
        let long=format!("{}\n最后否定：并不是 30 mg。", "完整原文🙂".repeat(20000));
        let blocks=blocks_from_text(&long,"author");
        assert_eq!(block_text(&blocks[0]),long);
        let structured=blocks_from_text("# 标题\n\n1. 不要删除否定\n2. 30 mg\n\n| 名称 | 数量 |\n| --- | --- |\n| A | 0 |\n\n> 他人观点\n\n重复\n\n重复","author");
        validate_blocks(&structured).unwrap();
        assert_eq!(structured[1]["kind"],"list");
        assert_eq!(structured[2]["rows"][0][1],"0");
        assert_eq!(structured[3]["speaker"],"unconfirmed");
        assert_eq!(structured.iter().filter(|b| b["text"]=="重复").count(),2);
    }
    #[test]
    fn rejects_ambiguous_shapes_and_invented_locations() {
        for block in [json!({"kind":"paragraph","text":"a","items":["b"]}),
            json!({"kind":"table","columns":["a","b"],"rows":[["a"]]}),
            json!({"kind":"paragraph","text":"a","bbox":[0.8,0.1,0.5,0.2]}),
            json!({"kind":"paragraph","text":"a","time":{"startMs":5,"endMs":2}})] {
            assert!(validate_blocks(&[block]).is_err());
        }
    }
    #[test]
    fn ocr_preserves_two_columns_ui_words_and_local_uncertainty() {
        let lines=vec![json!({"text":"左1不删除","confidence":0.99,"bbox":[0.1,0.1,0.35,0.04]}),
            json!({"text":"右1点赞","confidence":0.99,"bbox":[0.55,0.1,0.35,0.04]}),
            json!({"text":"左2收藏","confidence":0.99,"bbox":[0.1,0.2,0.35,0.04]}),
            json!({"text":"右2不可信","confidence":0.1,"bbox":[0.55,0.2,0.35,0.04]})];
        let blocks=ocr_blocks(&lines);
        assert_eq!(blocks[0]["text"],"左1不删除"); assert_eq!(blocks[1]["text"],"左2收藏");
        assert_eq!(blocks[2]["text"],"右1点赞"); assert_eq!(blocks[3]["kind"],"gap");
        validate_blocks(&blocks).unwrap();
        assert!(blocks.iter().all(|b|b["issues"].as_array().unwrap().iter().any(|i|i["code"]=="structure_unconfirmed")));
    }
    #[test]
    fn two_column_table_never_becomes_an_unqualified_column_major_fact(){
        let lines=vec![json!({"text":"A","confidence":0.99,"bbox":[0.1,0.1,0.1,0.04]}),
            json!({"text":"1 mg","confidence":0.99,"bbox":[0.6,0.1,0.2,0.04]}),
            json!({"text":"B","confidence":0.99,"bbox":[0.1,0.2,0.1,0.04]}),
            json!({"text":"2 mg","confidence":0.99,"bbox":[0.6,0.2,0.2,0.04]})];
        let blocks=ocr_blocks(&lines);validate_blocks(&blocks).unwrap();
        for line in &lines{
            let block=blocks.iter().find(|b|b["text"]==line["text"]).unwrap();
            assert_eq!(block["bbox"],line["bbox"]);
            assert!(block["issues"].as_array().unwrap().iter().any(|i|i["code"]=="structure_unconfirmed"));
        }
    }
}
