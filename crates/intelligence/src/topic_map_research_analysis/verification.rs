//! An independent, bounded review of a frozen proposal. A verdict never edits it.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

mod evidence;
mod packet;
pub use packet::{extraction_review, resolution_review};

pub const VERIFY_CONTRACT: &str = "topic-map.verify.v1";
pub const MAX_VERIFY_CHECKS: usize = 96;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VerificationCheck {
    pub check_id: String,
    pub verdict: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationOutput {
    pub contract: String,
    pub checks: Vec<VerificationCheck>,
}

pub fn verify_schema() -> Value {
    json!({"type":"object","additionalProperties":false,"required":["contract","checks"],
        "properties":{"contract":{"type":"string","enum":[VERIFY_CONTRACT]},
          "checks":{"type":"array","minItems":1,"maxItems":MAX_VERIFY_CHECKS,
            "items":{"type":"object","additionalProperties":false,
              "required":["checkId","verdict","reason"],"properties":{
                "checkId":{"type":"string","minLength":1,"maxLength":32,"pattern":"^[a-z][a-z0-9.]*$"},
                "verdict":{"type":"string","enum":["pass","fail","uncertain"]},
                "reason":{"type":"string","minLength":1,"maxLength":60}}}}}})
}

const RULES: [&str; 12] = [
    "semantic_outcome_unsupported",
    "semantic_discussion_unsupported",
    "semantic_discussion_non_atomic",
    "semantic_scene_unsupported",
    "semantic_journey_unsupported",
    "semantic_response_unsupported",
    "semantic_angle_unsupported",
    "semantic_opportunity_unsupported",
    "semantic_assignment_boundary",
    "semantic_identity_relation",
    "semantic_concept_boundary",
    "semantic_parent_containment",
];

fn safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value.as_bytes()[0].is_ascii_lowercase()
        && value
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'.')
}

/// `expected` is the program-built packet["checks"], never provider output.
/// Check the entire closed set before inspecting any verdict; missing checks cannot pass.
pub fn validate_verification(
    output: &VerificationOutput,
    expected: &Value,
) -> Result<(), &'static str> {
    if output.contract != VERIFY_CONTRACT {
        return Err("semantic_review_contract_invalid");
    }
    let checks = expected
        .as_array()
        .ok_or("semantic_review_expected_invalid")?;
    if checks.is_empty() || checks.len() > MAX_VERIFY_CHECKS {
        return Err("semantic_review_expected_invalid");
    }
    let mut rules = HashMap::new();
    for check in checks {
        let id = check["checkId"]
            .as_str()
            .ok_or("semantic_review_expected_invalid")?;
        let rule = check["rule"]
            .as_str()
            .ok_or("semantic_review_expected_invalid")?;
        if !safe_id(id) || !RULES.contains(&rule) || rules.insert(id, rule).is_some() {
            return Err("semantic_review_expected_invalid");
        }
    }
    if output.checks.len() != checks.len() {
        return Err("semantic_review_coverage_invalid");
    }
    let mut seen = HashSet::new();
    for check in &output.checks {
        if !safe_id(&check.check_id)
            || !rules.contains_key(check.check_id.as_str())
            || !seen.insert(check.check_id.as_str())
        {
            return Err("semantic_review_coverage_invalid");
        }
        if !["pass", "fail", "uncertain"].contains(&check.verdict.as_str())
            || check.reason.trim().is_empty()
            || check.reason.chars().count() > 60
        {
            return Err("semantic_review_check_invalid");
        }
    }
    for check in checks {
        let id = check["checkId"]
            .as_str()
            .ok_or("semantic_review_expected_invalid")?;
        let verdict = output
            .checks
            .iter()
            .find(|c| c.check_id == id)
            .ok_or("semantic_review_coverage_invalid")?;
        if verdict.verdict == "fail" {
            return Err(RULES
                .iter()
                .copied()
                .find(|r| *r == rules[id])
                .ok_or("semantic_review_expected_invalid")?);
        }
        if verdict.verdict == "uncertain" {
            return Err("semantic_review_uncertain");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
