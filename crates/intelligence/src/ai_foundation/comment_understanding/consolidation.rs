//! `comment-problem-consolidation.v1`: the model judges pairs of Problems that the program
//! proposed; it never chooses which Problem survives and never merges anything itself. Only a
//! `same` verdict can become a redirect.
use super::contract::{REASON_MAX_CHARS, trimmed};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

pub const CONSOLIDATION_CONTRACT: &str = "comment-problem-consolidation.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Same subject, goal and barrier mechanism: the two are one Problem.
    Same,
    Different,
    /// One is a narrower instance of the other. Never merged: the broader Problem would lose
    /// the distinction that the narrower one carries.
    Narrower,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsolidationDecision {
    pub pair_key: String,
    pub verdict: Verdict,
    pub reason: String,
}

/// The response is unusable as a whole; no pair is decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConsolidationRejected;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawResponse {
    contract: String,
    pairs: Vec<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawPair {
    #[serde(rename = "pairKey")]
    _pair_key: String,
    verdict: Verdict,
    reason: String,
}

pub fn consolidation_schema() -> Value {
    let pair = json!({"type":"object","additionalProperties":false,
        "required":["pairKey","verdict","reason"],
        "properties":{
            "pairKey":{"type":"string"},
            "verdict":{"type":"string","enum":["same","different","narrower"]},
            "reason":{"type":"string"}}});
    json!({"type":"object","additionalProperties":false,"required":["contract","pairs"],
        "properties":{
            "contract":{"type":"string","enum":[CONSOLIDATION_CONTRACT]},
            "pairs":{"type":"array","items":pair}}})
}

/// Decisions for the pairs that were asked. A pair the model skipped is simply undecided and is
/// never merged. An unknown, repeated or malformed entry decides nothing.
pub fn accept_consolidation(
    text: &str,
    asked: &HashSet<String>,
) -> Result<Vec<ConsolidationDecision>, ConsolidationRejected> {
    let response: RawResponse = serde_json::from_str(text).map_err(|_| ConsolidationRejected)?;
    if response.contract != CONSOLIDATION_CONTRACT {
        return Err(ConsolidationRejected);
    }
    let mut count: HashMap<String, usize> = HashMap::new();
    let parsed: Vec<ConsolidationDecision> = response
        .pairs
        .into_iter()
        .filter_map(|value| {
            let pair_key = trimmed(value.get("pairKey")?.as_str()?, 40)?;
            *count.entry(pair_key.clone()).or_default() += 1;
            let raw: RawPair = serde_json::from_value(value).ok()?;
            let reason = trimmed(&raw.reason, REASON_MAX_CHARS)?;
            Some(ConsolidationDecision {
                pair_key,
                verdict: raw.verdict,
                reason,
            })
        })
        .collect();
    Ok(parsed
        .into_iter()
        .filter(|d| asked.contains(&d.pair_key) && count.get(&d.pair_key) == Some(&1))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asked(keys: &[&str]) -> HashSet<String> {
        keys.iter().map(|k| (*k).to_owned()).collect()
    }

    fn response(pairs: Value) -> String {
        json!({"contract":CONSOLIDATION_CONTRACT,"pairs":pairs}).to_string()
    }

    #[test]
    fn decides_only_asked_pairs_and_skips_repeated_or_malformed_entries() {
        let text = response(json!([
            {"pairKey":"q1","verdict":"same","reason":"主体、目标、障碍机制一致"},
            {"pairKey":"q2","verdict":"narrower","reason":"后者是前者的子集"},
            {"pairKey":"q3","verdict":"same","reason":"重复"},
            {"pairKey":"q3","verdict":"different","reason":"重复"},
            {"pairKey":"q9","verdict":"same","reason":"没有被问到"},
            {"pairKey":"q4","verdict":"maybe","reason":"未知结论"},
            {"pairKey":"q5","verdict":"same","reason":"   "},
            {"pairKey":"q6","verdict":"same","reason":"重复"},
            {"pairKey":"q6","verdict":"invalid"},
        ]));
        let decisions =
            accept_consolidation(&text, &asked(&["q1", "q2", "q3", "q4", "q5", "q6"])).unwrap();
        assert_eq!(
            decisions
                .iter()
                .map(|d| (d.pair_key.as_str(), d.verdict))
                .collect::<Vec<_>>(),
            [("q1", Verdict::Same), ("q2", Verdict::Narrower)]
        );
    }

    #[test]
    fn a_broken_response_decides_nothing() {
        for text in [
            "not json".to_owned(),
            json!({"contract":"other","pairs":[]}).to_string(),
            json!({"contract":CONSOLIDATION_CONTRACT}).to_string(),
        ] {
            assert_eq!(
                accept_consolidation(&text, &asked(&["q1"])),
                Err(ConsolidationRejected)
            );
        }
    }

    #[test]
    fn schema_requires_every_pair_field() {
        let schema = consolidation_schema();
        assert_eq!(
            schema["properties"]["pairs"]["items"]["required"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
    }
}
