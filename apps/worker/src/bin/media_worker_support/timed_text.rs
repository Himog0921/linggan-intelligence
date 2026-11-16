//! Preserve producer-reported times; a model transcript without timings is rejected.
use serde_json::{Value, json};

pub fn asr_blocks(raw: &str) -> Result<Vec<Value>, String> {
    let value: Value = serde_json::from_str(raw).map_err(|_| "asr_json_invalid".to_owned())?;
    let segments = value["segments"].as_array().ok_or("asr_segments_missing")?;
    let mut blocks = vec![];
    for (index, segment) in segments.iter().enumerate() {
        let start = segment["start"]
            .as_f64()
            .filter(|s| s.is_finite() && *s >= 0.)
            .ok_or("asr_time_invalid")?;
        let end = segment["end"]
            .as_f64()
            .filter(|s| s.is_finite() && *s > start)
            .ok_or("asr_time_invalid")?;
        let text = segment["text"].as_str().ok_or("asr_text_invalid")?;
        let start_ms = (start * 1000.).round();
        let end_ms = (end * 1000.).round();
        if end_ms > u64::MAX as f64 || start_ms >= end_ms {
            return Err("asr_time_invalid".into());
        }
        if text.trim().is_empty() {
            continue;
        }
        blocks.push(json!({"ref":format!("segment.{index}"),"kind":"paragraph","speaker":"unconfirmed","text":text,"time":{"startMs":start_ms as u64,"endMs":end_ms as u64}}));
    }
    if blocks.is_empty() && value["text"].as_str().is_some_and(|t| !t.trim().is_empty()) {
        return Err("asr_text_without_timing".into());
    }
    Ok(blocks)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn asr_preserves_full_negated_statement_and_actual_times() {
        let raw = r#"{"text":"不可以。仍需检查。","segments":[{"start":1.25,"end":3.82,"text":"不可以。"},{"start":8.3,"end":9.7,"text":"仍需检查。"}]}"#;
        let blocks = asr_blocks(raw).unwrap();
        assert_eq!(blocks[0]["time"], json!({"startMs":1250,"endMs":3820}));
        assert_eq!(blocks[1]["text"], "仍需检查。");
        assert!(asr_blocks(r#"{"text":"无时序正文","segments":[]}"#).is_err());
    }
}
