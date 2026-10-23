//! Preserve producer-reported times. Sampling is a coverage limitation, not an
//! invented frame interval, and a model transcript without timings is rejected.
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

pub fn sampled_pts(stderr: &[u8]) -> Result<Vec<u64>, String> {
    let mut times = vec![];
    for line in String::from_utf8_lossy(stderr)
        .lines()
        .filter(|l| l.contains("showinfo") && l.contains("pts_time:"))
    {
        let seconds = line
            .split("pts_time:")
            .nth(1)
            .and_then(|v| v.split_whitespace().next())
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| v.is_finite() && *v >= 0.)
            .ok_or("frame_pts_invalid")?;
        if seconds * 1000. > u64::MAX as f64 {
            return Err("frame_pts_invalid".into());
        }
        times.push((seconds * 1000.).round() as u64);
    }
    if times.windows(2).any(|w| w[1] < w[0]) {
        return Err("frame_pts_out_of_order".into());
    }
    Ok(times)
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
    #[test]
    fn reads_actual_pts_without_first_twelve_cutoff() {
        let raw = (0..31)
            .map(|i| {
                format!(
                    "[Parsed_showinfo] n:{i} pts:{} pts_time:{} pos:1",
                    i * 1200,
                    i as f64 * 11.6
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let points = sampled_pts(raw.as_bytes()).unwrap();
        assert_eq!(points.len(), 31);
        assert_eq!(points[30], 348000);
    }
}
