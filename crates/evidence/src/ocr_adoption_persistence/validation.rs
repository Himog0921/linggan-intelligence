use super::*;

pub(super) fn validate_raw(b: &mut Bridge) -> Result<(), InputError> {
    let anchor: Value =
        serde_json::from_str(&b.anchor_raw_json).map_err(|_| InputError::Encoding)?;
    let raw: Value = serde_json::from_str(&b.vl_raw_json).map_err(|_| InputError::Encoding)?;
    let blocks = raw["paddle"]["res"]["parsing_res_list"]
        .as_array()
        .ok_or(InputError::Source)?;
    if raw["paddle"]["res"]["width"] != b.observation.image.width
        || raw["paddle"]["res"]["height"] != b.observation.image.height
    {
        return Err(InputError::Geometry);
    }
    if blocks.len() != b.observation.layout.blocks.len() {
        return Err(InputError::Source);
    }
    for (upstream, block) in blocks.iter().zip(&b.observation.layout.blocks) {
        let id = upstream["block_id"].as_u64().map(|i| i.to_string());
        if id.as_deref() != Some(&block.block_ref)
            || upstream["block_label"] != block.label
            || upstream["block_content"] != block.text
            || !same_box(&upstream["block_bbox"], block.bbox)
        {
            return Err(InputError::Source);
        }
    }
    let texts = anchor["res"]["rec_texts"]
        .as_array()
        .ok_or(InputError::Source)?;
    let scores = anchor["res"]["rec_scores"]
        .as_array()
        .ok_or(InputError::Source)?;
    let polys = anchor["res"]["dt_polys"]
        .as_array()
        .ok_or(InputError::Source)?;
    if texts.len() != scores.len()
        || texts.len() != polys.len()
        || texts
            .iter()
            .filter(|v| v.as_str().is_some_and(|s| !s.trim().is_empty()))
            .count()
            != b.observation.anchors.lines.len()
    {
        return Err(InputError::Source);
    }
    for l in &b.observation.anchors.lines {
        let n: usize = l.line_ref.parse().map_err(|_| InputError::Reference)?;
        if texts.get(n) != Some(&json!(l.text))
            || scores.get(n).and_then(Value::as_f64) != Some(l.confidence)
        {
            return Err(InputError::Source);
        }
        let points = polys
            .get(n)
            .and_then(Value::as_array)
            .ok_or(InputError::Geometry)?;
        let mut box_ = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
        for p in points {
            let x = p[0].as_f64().ok_or(InputError::Geometry)?;
            let y = p[1].as_f64().ok_or(InputError::Geometry)?;
            box_ = [
                box_[0].min(x),
                box_[1].min(y),
                box_[2].max(x),
                box_[3].max(y),
            ];
        }
        if box_ != l.bbox {
            return Err(InputError::Geometry);
        }
    }
    let generations = raw["generations"].as_array().ok_or(InputError::Source)?;
    for t in &mut b.observation.layout.terminations {
        let block = b
            .observation
            .layout
            .blocks
            .iter()
            .find(|v| v.block_ref == t.block_ref)
            .ok_or(InputError::Reference)?;
        let found: Vec<_> = generations
            .iter()
            .filter(|g| g["label"] == block.label && same_box(&g["bbox"], block.bbox))
            .collect();
        if found.len() != 1 {
            return Err(InputError::Source);
        }
        let r = &found[0]["response"];
        validate_response(r)?;
        if r["finishReason"] != json!(t.finish_reason)
            || r["requestRef"] != json!(t.request_ref)
            || r["rawOutputSha256"] != json!(t.raw_output_sha256)
            || r["generatedTokens"] != json!(t.generated_tokens)
        {
            return Err(InputError::Source);
        }
        let raw_text = r["rawText"].as_str().ok_or(InputError::Source)?;
        if t.raw_text.as_deref().is_some_and(|v| v != raw_text) {
            return Err(InputError::Source);
        }
        t.raw_text = Some(raw_text.into());
    }
    let rechecks = raw["rechecks"].as_array().ok_or(InputError::Source)?;
    if rechecks.len() != b.observation.rechecks.len() {
        return Err(InputError::Source);
    }
    for (artifact, check) in rechecks.iter().zip(&b.observation.rechecks) {
        let saved: LocalRecheck =
            serde_json::from_value(artifact["check"].clone()).map_err(|_| InputError::Encoding)?;
        let raw_json = artifact["rawJson"].as_str().ok_or(InputError::Source)?;
        let raw_value: Value = serde_json::from_str(raw_json).map_err(|_| InputError::Encoding)?;
        if json!(saved) != json!(check)
            || raw_value != artifact["raw"]
            || rule::content_hash(raw_json.as_bytes()) != check.raw_sha256
        {
            return Err(InputError::Source);
        }
        if check.kind == RecheckKind::LineCrop {
            let raw = &artifact["raw"]["res"];
            let texts = raw["rec_texts"].as_array().ok_or(InputError::Source)?;
            let polys = raw["dt_polys"].as_array().ok_or(InputError::Source)?;
            let scores = raw["rec_scores"].as_array().ok_or(InputError::Source)?;
            if texts.len() != polys.len() || texts.len() != scores.len() {
                return Err(InputError::Source);
            }
            let mut lines = vec![];
            for (n, text) in texts.iter().enumerate() {
                let text = text.as_str().ok_or(InputError::Source)?;
                if text.trim().is_empty() {
                    continue;
                }
                let points = polys[n].as_array().ok_or(InputError::Geometry)?;
                let mut bbox = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
                for p in points {
                    let x = p[0].as_f64().ok_or(InputError::Geometry)?;
                    let y = p[1].as_f64().ok_or(InputError::Geometry)?;
                    bbox = [
                        bbox[0].min(x),
                        bbox[1].min(y),
                        bbox[2].max(x),
                        bbox[3].max(y),
                    ];
                }
                lines.push(Line {
                    line_ref: n.to_string(),
                    reading_order: n as u32,
                    text: text.into(),
                    confidence: scores[n].as_f64().ok_or(InputError::Source)?,
                    bbox,
                });
            }
            if rule::ordered_line_text(&lines) != check.text {
                return Err(InputError::Source);
            }
        }
        if check.kind == RecheckKind::VlCrop {
            validate_response(&artifact["raw"])?;
            if artifact["raw"]["rawText"] != check.text
                || artifact["raw"]["finishReason"] != json!(check.finish_reason)
            {
                return Err(InputError::Source);
            }
        }
    }
    Ok(())
}
fn validate_response(r: &Value) -> Result<(), InputError> {
    let text = r["rawText"].as_str().ok_or(InputError::Source)?;
    if r["rawOutputSha256"] != rule::content_hash(text.as_bytes())
        || r["timeoutSeconds"] != 45
        || r["maxTokens"].as_u64().is_none_or(|n| n > 4096)
    {
        return Err(InputError::Source);
    }
    if r["finishReason"] != "error" {
        let response = &r["rawResponse"];
        let content = &response["choices"][0]["message"]["content"];
        let observed = match content {
            Value::String(s) => s.as_str(),
            Value::Null => "",
            _ => return Err(InputError::Source),
        };
        if response["id"] != r["requestRef"]
            || response["choices"][0]["finish_reason"] != r["finishReason"]
            || observed != text
        {
            return Err(InputError::Source);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn empty() -> Value {
        json!({"rawText":"","rawOutputSha256":rule::content_hash(b""),"timeoutSeconds":45,"maxTokens":4096,
        "finishReason":"stop","requestRef":"synthetic-empty","rawResponse":{"id":"synthetic-empty","choices":[{"finish_reason":"stop","message":{"content":null}}]}})
    }
    #[test]
    fn actual_null_completion_is_empty_not_missing_source() {
        assert!(validate_response(&empty()).is_ok());
    }
    #[test]
    fn arbitrary_nontext_content_is_not_an_empty_completion() {
        let mut v = empty();
        v["rawResponse"]["choices"][0]["message"]["content"] = json!([]);
        assert!(validate_response(&v).is_err());
    }
    #[test]
    fn finish_reason_and_raw_text_cannot_be_relabelled() {
        let mut v = empty();
        v["finishReason"] = json!("length");
        assert!(validate_response(&v).is_err());
        let mut v = empty();
        v["rawText"] = json!("invented");
        assert!(validate_response(&v).is_err());
    }
}
fn same_box(raw: &Value, b: [f64; 4]) -> bool {
    raw.as_array()
        .is_some_and(|v| v.len() == 4 && v.iter().zip(b).all(|(a, b)| a.as_f64() == Some(b)))
}
