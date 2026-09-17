//! The one structured semantic contract for a Study target.
//!
//! The model may summarize a comment, but it cannot mint evidence: every proposed evidence and
//! every non-null problem-frame basis must resolve to one unambiguous continuous span of the
//! frozen target comment. Context is deliberately absent from this admission boundary.

use crate::comment_cleaning::clean;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

pub const SEMANTIC_CONTRACT: &str = "comment-study.semantic.v1";
const MAX_SIGNALS_PER_TARGET: usize = 12;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct SemanticOutput {
    contract: String,
    signals: Vec<ProposedSignal>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SignalKind {
    Problem,
    Need,
    Belief,
    Emotion,
    Experience,
    Solution,
    Quote,
    Context,
    Question,
}

impl SignalKind {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Problem => "problem",
            Self::Need => "need",
            Self::Belief => "belief",
            Self::Emotion => "emotion",
            Self::Experience => "experience",
            Self::Solution => "solution",
            Self::Quote => "quote",
            Self::Context => "context",
            Self::Question => "question",
        }
    }

    fn has_problem_frame(&self) -> bool {
        matches!(self, Self::Problem | Self::Need)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProposedSignal {
    kind: SignalKind,
    proposition: String,
    evidence: String,
    problem_frame: Option<ProposedProblemFrame>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProposedProblemFrame {
    actor: FramedValue,
    goal_or_expected_state: FramedValue,
    barrier_or_unmet_need: FramedValue,
    context: FramedValue,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FramedValue {
    value: Option<String>,
    basis: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AcceptedSignal {
    pub kind: String,
    pub proposition: String,
    pub evidence: String,
    pub evidence_start: i32,
    pub evidence_end: i32,
    pub problem_frame: Option<Value>,
    pub eligibility_state: String,
    pub eligibility_reason: Option<String>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SemanticContractError {
    #[error("semantic output does not satisfy the JSON contract")]
    JsonSchema,
    #[error("semantic output declares another contract")]
    Contract,
    #[error("semantic output exceeds the bounded signal count")]
    SignalLimit,
    #[error("semantic signal has invalid text or an invalid frame shape")]
    SignalShape,
    #[error("semantic evidence is not one unambiguous continuous target-comment span")]
    EvidenceNotContiguousOrAmbiguous,
    #[error("a non-problem signal cannot carry a problem frame")]
    UnsupportedProblemFrame,
}

impl SemanticContractError {
    pub fn rejection_code(&self) -> &'static str {
        match self {
            Self::JsonSchema => "semantic_json_schema",
            Self::Contract | Self::SignalLimit | Self::SignalShape => "semantic_contract",
            Self::EvidenceNotContiguousOrAmbiguous => "evidence_not_contiguous",
            Self::UnsupportedProblemFrame => "unsupported_problem_frame",
        }
    }
}

/// Validates one model response atomically. Any violation rejects its complete output: callers
/// must record a rejected semantic attempt and must not save a partial signal list.
pub fn accept_semantic_output(
    raw_output: Value,
    frozen_target_comment: &str,
) -> Result<Vec<AcceptedSignal>, SemanticContractError> {
    let output: SemanticOutput =
        serde_json::from_value(raw_output).map_err(|_| SemanticContractError::JsonSchema)?;
    if output.contract != SEMANTIC_CONTRACT {
        return Err(SemanticContractError::Contract);
    }
    if output.signals.len() > MAX_SIGNALS_PER_TARGET {
        return Err(SemanticContractError::SignalLimit);
    }
    let cleaned = clean(frozen_target_comment);
    if !matches!(cleaned.state.as_str(), "direct" | "context") {
        return Err(SemanticContractError::SignalShape);
    }
    let mut seen = BTreeSet::new();
    output
        .signals
        .into_iter()
        .map(|signal| accept_one_signal(signal, frozen_target_comment, &cleaned, &mut seen))
        .collect()
}

fn accept_one_signal(
    signal: ProposedSignal,
    frozen_target_comment: &str,
    cleaned: &crate::comment_cleaning::CleanComment,
    seen: &mut BTreeSet<(String, i32, i32)>,
) -> Result<AcceptedSignal, SemanticContractError> {
    let proposition = bounded_text(signal.proposition, 1000)?;
    let evidence = bounded_text(signal.evidence, 1000)?;
    let (evidence_start, evidence_end, original_evidence) = cleaned
        .resolve(&evidence, frozen_target_comment)
        .map_err(|_| SemanticContractError::EvidenceNotContiguousOrAmbiguous)?;
    let kind = signal.kind.as_str().to_owned();
    if !seen.insert((kind.clone(), evidence_start, evidence_end)) {
        return Err(SemanticContractError::SignalShape);
    }
    let (problem_frame, eligibility_state, eligibility_reason) = accept_problem_frame(
        &signal.kind,
        signal.problem_frame,
        frozen_target_comment,
        cleaned,
    )?;
    Ok(AcceptedSignal {
        kind,
        proposition,
        evidence: original_evidence,
        evidence_start,
        evidence_end,
        problem_frame,
        eligibility_state,
        eligibility_reason,
    })
}

fn accept_problem_frame(
    kind: &SignalKind,
    proposed: Option<ProposedProblemFrame>,
    raw: &str,
    cleaned: &crate::comment_cleaning::CleanComment,
) -> Result<(Option<Value>, String, Option<String>), SemanticContractError> {
    if !kind.has_problem_frame() {
        return if proposed.is_none() {
            Ok((None, "not_applicable".to_owned(), None))
        } else {
            Err(SemanticContractError::UnsupportedProblemFrame)
        };
    }
    let frame = proposed.ok_or(SemanticContractError::SignalShape)?;
    validate_frame_basis(&frame.actor, raw, cleaned)?;
    validate_frame_basis(&frame.goal_or_expected_state, raw, cleaned)?;
    validate_frame_basis(&frame.barrier_or_unmet_need, raw, cleaned)?;
    validate_frame_basis(&frame.context, raw, cleaned)?;
    let missing = missing_required_frame_fields(&frame);
    let state = if missing.is_empty() {
        "eligible"
    } else {
        "deferred_context"
    };
    let reason = (!missing.is_empty()).then(|| format!("missing_{}", missing.join("_")));
    Ok((
        Some(serde_json::to_value(frame).map_err(|_| SemanticContractError::SignalShape)?),
        state.to_owned(),
        reason,
    ))
}

fn bounded_text(value: String, maximum: usize) -> Result<String, SemanticContractError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().count() > maximum {
        return Err(SemanticContractError::SignalShape);
    }
    Ok(trimmed.to_owned())
}

fn validate_frame_basis(
    field: &FramedValue,
    raw: &str,
    cleaned: &crate::comment_cleaning::CleanComment,
) -> Result<(), SemanticContractError> {
    match (&field.value, &field.basis) {
        (None, None) => Ok(()),
        (Some(value), Some(basis)) => {
            bounded_text(value.clone(), 500)?;
            let basis = bounded_text(basis.clone(), 1000)?;
            cleaned
                .resolve(&basis, raw)
                .map(|_| ())
                .map_err(|_| SemanticContractError::EvidenceNotContiguousOrAmbiguous)
        }
        _ => Err(SemanticContractError::SignalShape),
    }
}

fn missing_required_frame_fields(frame: &ProposedProblemFrame) -> Vec<&'static str> {
    [
        ("actor", &frame.actor),
        ("goal", &frame.goal_or_expected_state),
        ("barrier", &frame.barrier_or_unmet_need),
    ]
    .into_iter()
    .filter_map(|(name, field)| field.value.is_none().then_some(name))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn complete_frame() -> Value {
        json!({
            "actor":{"value":"评论者","basis":"我很着急"},
            "goalOrExpectedState":{"value":"孩子自主开始作业","basis":"不催就不开始"},
            "barrierOrUnmetNeed":{"value":"需要外部催促","basis":"都要催"},
            "context":{"value":"家庭作业","basis":"写作业"}
        })
    }

    #[test]
    fn accepts_complete_problem_only_when_all_claim_bases_are_target_spans() {
        let raw = "孩子每天写作业都要催，不催就不开始，我很着急。";
        let output = json!({"contract":SEMANTIC_CONTRACT,"signals":[{
            "kind":"problem","proposition":"孩子在家庭作业中存在自主启动困难。",
            "evidence":"每天写作业都要催,不催就不开始","problemFrame":complete_frame()
        }]});
        let signals = accept_semantic_output(output, raw).unwrap();
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].eligibility_state, "eligible");
        assert_eq!(signals[0].evidence, "每天写作业都要催，不催就不开始");
        assert!(signals[0].problem_frame.is_some());
    }

    #[test]
    fn incomplete_problem_frame_stays_deferred_instead_of_becoming_a_problem() {
        let raw = "换一种题型就不会。";
        let output = json!({"contract":SEMANTIC_CONTRACT,"signals":[{
            "kind":"problem","proposition":"新题型应用困难。","evidence":"换一种题型就不会",
            "problemFrame":{
                "actor":{"value":null,"basis":null},
                "goalOrExpectedState":{"value":null,"basis":null},
                "barrierOrUnmetNeed":{"value":"新题型无法应用","basis":"换一种题型就不会"},
                "context":{"value":null,"basis":null}
            }
        }]});
        let signals = accept_semantic_output(output, raw).unwrap();
        assert_eq!(signals[0].eligibility_state, "deferred_context");
        assert_eq!(
            signals[0].eligibility_reason.as_deref(),
            Some("missing_actor_goal")
        );
    }

    #[test]
    fn rejects_ambiguous_evidence_and_never_returns_partial_signals() {
        let output = json!({"contract":SEMANTIC_CONTRACT,"signals":[
            {"kind":"emotion","proposition":"焦虑。","evidence":"孩子拖延","problemFrame":null},
            {"kind":"belief","proposition":"需要努力。","evidence":"努力","problemFrame":null}
        ]});
        assert_eq!(
            accept_semantic_output(output, "孩子拖延，孩子拖延，要努力。"),
            Err(SemanticContractError::EvidenceNotContiguousOrAmbiguous)
        );
    }

    #[test]
    fn rejects_problem_frame_on_non_problem_signal() {
        let output = json!({"contract":SEMANTIC_CONTRACT,"signals":[{
            "kind":"belief","proposition":"向往朴素生活。","evidence":"向往朴素生活",
            "problemFrame":complete_frame()
        }]});
        assert_eq!(
            accept_semantic_output(output, "我向往朴素生活。"),
            Err(SemanticContractError::UnsupportedProblemFrame)
        );
    }
}
