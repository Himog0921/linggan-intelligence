//! Typed problem proposals and strict stage output validation.
use super::*;
use serde::{Deserialize, Serialize};
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Problem {
    pub(super) label: String,
    pub(super) actor: String,
    pub(super) goal: String,
    pub(super) barrier: String,
    pub(super) context: Option<String>,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Decision {
    pub(super) signal_ordinal: usize,
    pub(super) status: String,
    pub(super) definition_ref: Option<Uuid>,
    pub(super) problem: Option<Problem>,
    pub(super) reason: String,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Resolution {
    pub(super) contract: String,
    pub(super) decisions: Vec<Decision>,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Check {
    pub(super) signal_ordinal: usize,
    pub(super) verdict: String,
    pub(super) reason: String,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Review {
    pub(super) contract: String,
    pub(super) checks: Vec<Check>,
}
pub(super) fn schema(stage: u8) -> Value {
    let object = |properties: Value| {
        let keys = properties
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        json!({"type":"object","additionalProperties":false,"required":keys,"properties":properties})
    };
    if stage == 2 {
        return object(
            json!({"contract":{"type":"string","enum":["comment-problem-review.v1"]},"checks":{"type":"array","maxItems":12,"items":object(json!({"signalOrdinal":{"type":"integer","minimum":0,"maximum":11},"verdict":{"type":"string","enum":["pass","fail","uncertain"]},"reason":{"type":"string","maxLength":500}}))}}),
        );
    }
    let text = json!({"type":"string","minLength":1,"maxLength":500});
    let mut problem = object(
        json!({"label":{"type":"string","minLength":1,"maxLength":120},"actor":text,"goal":text,"barrier":text,"context":{"type":["string","null"]}}),
    );
    problem["type"] = json!(["object", "null"]);
    object(
        json!({"contract":{"type":"string","enum":["comment-problem-resolution.v1"]},"decisions":{"type":"array","maxItems":12,"items":object(json!({"signalOrdinal":{"type":"integer","minimum":0,"maximum":11},"status":{"type":"string","enum":["matched","new","needs_context","out_of_scope"]},"definitionRef":{"type":["string","null"]},"problem":problem,"reason":text}))}}),
    )
}
pub(crate) fn validate(text: &str, stage: u8, context: &Value) -> Option<Value> {
    let ordinals: Vec<usize> = serde_json::from_value(context["signalOrdinals"].clone()).ok()?;
    if stage == 2 {
        let review: Review = serde_json::from_str(text).ok()?;
        if review.contract != "comment-problem-review.v1" || review.checks.len() != ordinals.len() {
            return None;
        }
        let mut seen = std::collections::HashSet::new();
        for check in &review.checks {
            if !ordinals.contains(&check.signal_ordinal)
                || !seen.insert(check.signal_ordinal)
                || check.verdict != "pass"
                || !bounded(&check.reason, 500)
            {
                return None;
            }
        }
        return serde_json::to_value(review).ok();
    }
    let resolution: Resolution = serde_json::from_str(text).ok()?;
    if resolution.contract != "comment-problem-resolution.v1"
        || resolution.decisions.len() != ordinals.len()
    {
        return None;
    }
    let mut seen = std::collections::HashSet::new();
    for d in &resolution.decisions {
        if !ordinals.contains(&d.signal_ordinal)
            || !seen.insert(d.signal_ordinal)
            || !bounded(&d.reason, 500)
        {
            return None;
        }
        match d.status.as_str() {
            "matched"
                if d.problem.is_none()
                    && d.definition_ref.is_some_and(|id| {
                        context["candidates"]
                            .as_array()
                            .is_some_and(|cs| cs.iter().any(|c| c["definitionRef"] == json!(id)))
                    }) => {}
            "new" if d.definition_ref.is_none() => {
                let p = d.problem.as_ref()?;
                if !bounded(&p.label, 120)
                    || [&p.actor, &p.goal, &p.barrier]
                        .iter()
                        .any(|s| !bounded(s, 500))
                    || p.context.as_ref().is_some_and(|s| !bounded(s, 500))
                    || context["candidates"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .any(|c| c["identityHash"] == problem_identity(p))
                {
                    return None;
                }
            }
            "needs_context" | "out_of_scope"
                if d.problem.is_none() && d.definition_ref.is_none() => {}
            _ => return None,
        }
    }
    serde_json::to_value(resolution).ok()
}
