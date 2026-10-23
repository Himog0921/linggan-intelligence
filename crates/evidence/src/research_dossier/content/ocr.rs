//! Observed OCR layout becomes content with explicit local uncertainty.
use super::validate_location;
use serde_json::{Value, json};

/// OCR coordinates determine only reading order. They never prove author attribution.
pub fn ocr_blocks(lines: &[Value]) -> Vec<Value> {
    let ordered = reading_order(lines);
    let mut output = Vec::new();
    for (index, (line, layout_unconfirmed)) in ordered.into_iter().enumerate() {
        let text = line["text"].as_str().unwrap_or("");
        if text.trim().is_empty() {
            continue;
        }
        let uncertain = line["confidence"].as_f64().is_some_and(|c| c < 0.35);
        let kind = if uncertain {
            "gap"
        } else if line["kind"] == "heading" {
            "heading"
        } else {
            "paragraph"
        };
        let mut block = json!({"kind":kind,"text":if uncertain {"此处文字识别不可靠，待对照原图"} else {text},
            "speaker":line["speaker"].as_str().filter(|s| matches!(*s,"author"|"other"|"unconfirmed")).unwrap_or("unconfirmed")});
        if let Some(bbox) = line.get("bbox") {
            if validate_location(&json!({"bbox":bbox})).is_ok() {
                block["bbox"] = bbox.clone();
            }
        }
        let mut issues = vec![];
        if uncertain {
            issues.push(
                json!({"key":format!("ocr_low_confidence_{index}"),"code":"low_confidence",
            "message":"仅此处识别不可靠，未推测填字","resolution":"reading_edit"}),
            );
        }
        if layout_unconfirmed {
            issues.push(json!({"key":format!("ocr_layout_{index}"),"code":"structure_unconfirmed",
            "message":"此区域可能是多栏文字或表格；当前为待核对的阅读顺序，列间对应关系尚未确认，原始位置已保留","resolution":"reading_edit"}));
        }
        if !issues.is_empty() {
            block["issues"] = json!(issues);
        }
        output.push(block);
    }
    output
}

fn reading_order(lines: &[Value]) -> Vec<(&Value, bool)> {
    let mut lines: Vec<_> = lines.iter().collect();
    if lines
        .iter()
        .any(|l| validate_location(&json!({"bbox":l["bbox"]})).is_err())
    {
        return lines.into_iter().map(|line| (line, true)).collect();
    }
    lines.sort_by(|a, b| {
        coord(a, 1)
            .total_cmp(&coord(b, 1))
            .then(coord(a, 0).total_cmp(&coord(b, 0)))
    });
    let mut result = vec![];
    let mut segment = vec![];
    for line in lines {
        if coord(line, 2) > 0.70 {
            order_segment(&mut segment, &mut result);
            result.push((line, false));
        } else {
            segment.push(line);
        }
    }
    order_segment(&mut segment, &mut result);
    result
}

fn coord(line: &Value, index: usize) -> f64 {
    line["bbox"][index].as_f64().unwrap_or(0.0)
}

fn order_segment<'a>(segment: &mut Vec<&'a Value>, output: &mut Vec<(&'a Value, bool)>) {
    let left: Vec<_> = segment
        .iter()
        .copied()
        .filter(|l| coord(l, 0) + coord(l, 2) <= 0.52)
        .collect();
    let right: Vec<_> = segment
        .iter()
        .copied()
        .filter(|l| coord(l, 0) >= 0.52)
        .collect();
    if left.len() >= 2 && right.len() >= 2 && left.len() + right.len() == segment.len() {
        output.extend(left.into_iter().chain(right).map(|line| (line, true)));
    } else {
        output.extend(segment.iter().copied().map(|line| (line, false)));
    }
    segment.clear();
}
