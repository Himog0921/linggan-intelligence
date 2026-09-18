//! Deterministic admission rules for durable comment-study Problems.
//!
//! Retrieval may choose a bounded server-side candidate set, and a model may compare the fixed
//! semantic dimensions. Neither may create a Problem directly: the closed-set result below either
//! assigns one existing Problem or defers the Signal. A new Problem is separately admissible only
//! from two independent Signals with a shared, explicit definition.

use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeSet;
use thiserror::Error;
use uuid::Uuid;

pub const PROBLEM_RESOLUTION_CONTRACT: &str = "comment-study.problem-resolution.v1";
pub const PROBLEM_PAIR_CONTRACT: &str = "comment-study.problem-pair.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExistingResolutionDecision {
    Assign(Uuid),
    DeferAmbiguous,
    DeferNovel,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PairCreationDecision {
    Create(NewProblemDefinition),
    DeferInsufficientIndependentEvidence,
    DeferAmbiguous,
    DeferNotSameProblem,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewProblemDefinition {
    /// A short neutral name. The revision stores it separately from the definition because the
    /// title is what a reader scans, while the definition is what membership is judged against.
    pub title: String,
    pub definition: String,
    pub stable_identity: Value,
    pub include_criteria: Vec<String>,
    pub exclude_criteria: Vec<String>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProblemResolutionContractError {
    #[error("problem-resolution output does not satisfy the JSON contract")]
    JsonSchema,
    #[error("problem-resolution output declares another contract")]
    Contract,
    #[error("candidate comparison set is not exactly the server-provided closed set")]
    CandidateSetMismatch,
    #[error("a comparison contains an invalid dimension verdict")]
    InvalidVerdict,
    #[error("a proposed durable Problem definition is incomplete")]
    InvalidProblemDefinition,
    #[error("pair proposal does not name exactly the two supplied Signals")]
    PairSignalMismatch,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExistingResolutionOutput {
    contract: String,
    candidates: Vec<CandidateComparison>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CandidateComparison {
    problem_ref: Uuid,
    dimensions: DimensionVerdicts,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DimensionVerdicts {
    actor: Verdict,
    goal_or_expected_state: Verdict,
    barrier_or_unmet_need: Verdict,
    context: Verdict,
}

impl DimensionVerdicts {
    fn relationship(&self) -> Relationship {
        let all = [
            &self.actor,
            &self.goal_or_expected_state,
            &self.barrier_or_unmet_need,
            &self.context,
        ];
        if all.iter().all(|verdict| matches!(verdict, Verdict::Same)) {
            Relationship::Match
        } else if all
            .iter()
            .any(|verdict| matches!(verdict, Verdict::Different))
        {
            Relationship::Different
        } else {
            Relationship::Unknown
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Verdict {
    Same,
    Different,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Relationship {
    Match,
    Different,
    Unknown,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PairProposal {
    contract: String,
    first_signal_ref: Uuid,
    second_signal_ref: Uuid,
    dimensions: DimensionVerdicts,
    proposed_problem: Option<ProposedProblemDefinition>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ProposedProblemDefinition {
    title: String,
    definition: String,
    stable_identity: Value,
    include_criteria: Vec<String>,
    exclude_criteria: Vec<String>,
}

/// Interprets a fully closed candidate comparison. Missing, duplicated or invented candidates are
/// protocol failures; they must not be treated as a negative match result.
pub fn decide_existing_resolution(
    raw_output: Value,
    server_candidate_refs: &[Uuid],
) -> Result<ExistingResolutionDecision, ProblemResolutionContractError> {
    let output: ExistingResolutionOutput = serde_json::from_value(raw_output)
        .map_err(|_| ProblemResolutionContractError::JsonSchema)?;
    if output.contract != PROBLEM_RESOLUTION_CONTRACT {
        return Err(ProblemResolutionContractError::Contract);
    }
    let expected: BTreeSet<Uuid> = server_candidate_refs.iter().copied().collect();
    let actual: BTreeSet<Uuid> = output
        .candidates
        .iter()
        .map(|item| item.problem_ref)
        .collect();
    if expected.len() != server_candidate_refs.len()
        || actual.len() != output.candidates.len()
        || actual != expected
    {
        return Err(ProblemResolutionContractError::CandidateSetMismatch);
    }

    let relationships: Vec<(Uuid, Relationship)> = output
        .candidates
        .iter()
        .map(|candidate| (candidate.problem_ref, candidate.dimensions.relationship()))
        .collect();
    let matches: Vec<Uuid> = relationships
        .iter()
        .filter_map(|(problem_ref, relationship)| {
            (*relationship == Relationship::Match).then_some(*problem_ref)
        })
        .collect();
    if matches.len() == 1
        && relationships
            .iter()
            .all(|(_, relationship)| *relationship != Relationship::Unknown)
    {
        return Ok(ExistingResolutionDecision::Assign(matches[0]));
    }
    if matches.len() >= 2
        || relationships
            .iter()
            .any(|(_, value)| *value == Relationship::Unknown)
    {
        return Ok(ExistingResolutionDecision::DeferAmbiguous);
    }
    Ok(ExistingResolutionDecision::DeferNovel)
}

/// Admits a proposed durable Problem only after the two source Signals were independently
/// authored and every identity dimension agrees. One isolated Signal never reaches this function
/// as a creation decision.
pub fn decide_pair_creation(
    raw_output: Value,
    first_signal_ref: Uuid,
    second_signal_ref: Uuid,
    independent_sources: bool,
    independent_authors: bool,
) -> Result<PairCreationDecision, ProblemResolutionContractError> {
    let proposal: PairProposal = serde_json::from_value(raw_output)
        .map_err(|_| ProblemResolutionContractError::JsonSchema)?;
    if proposal.contract != PROBLEM_PAIR_CONTRACT {
        return Err(ProblemResolutionContractError::Contract);
    }
    let expected = BTreeSet::from([first_signal_ref, second_signal_ref]);
    let actual = BTreeSet::from([proposal.first_signal_ref, proposal.second_signal_ref]);
    if expected.len() != 2 || actual != expected {
        return Err(ProblemResolutionContractError::PairSignalMismatch);
    }
    if !independent_sources || !independent_authors {
        return Ok(PairCreationDecision::DeferInsufficientIndependentEvidence);
    }
    match proposal.dimensions.relationship() {
        Relationship::Different => Ok(PairCreationDecision::DeferNotSameProblem),
        Relationship::Unknown => Ok(PairCreationDecision::DeferAmbiguous),
        Relationship::Match => proposal
            .proposed_problem
            .ok_or(ProblemResolutionContractError::InvalidProblemDefinition)
            .and_then(validate_problem_definition)
            .map(PairCreationDecision::Create),
    }
}

fn validate_problem_definition(
    proposed: ProposedProblemDefinition,
) -> Result<NewProblemDefinition, ProblemResolutionContractError> {
    let title = bounded_text(proposed.title, 200)?;
    let definition = bounded_text(proposed.definition, 1000)?;
    if !proposed.stable_identity.is_object()
        || proposed
            .stable_identity
            .as_object()
            .is_none_or(|v| v.is_empty())
    {
        return Err(ProblemResolutionContractError::InvalidProblemDefinition);
    }
    let include_criteria = proposed
        .include_criteria
        .into_iter()
        .map(|criterion| bounded_text(criterion, 500))
        .collect::<Result<Vec<_>, _>>()?;
    let exclude_criteria = proposed
        .exclude_criteria
        .into_iter()
        .map(|criterion| bounded_text(criterion, 500))
        .collect::<Result<Vec<_>, _>>()?;
    if include_criteria.is_empty() || exclude_criteria.is_empty() {
        return Err(ProblemResolutionContractError::InvalidProblemDefinition);
    }
    Ok(NewProblemDefinition {
        title,
        definition,
        stable_identity: proposed.stable_identity,
        include_criteria,
        exclude_criteria,
    })
}

fn bounded_text(value: String, maximum: usize) -> Result<String, ProblemResolutionContractError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().count() > maximum {
        return Err(ProblemResolutionContractError::InvalidProblemDefinition);
    }
    Ok(trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn dimensions(actor: &str, goal: &str, barrier: &str, context: &str) -> Value {
        json!({
            "actor":actor,
            "goalOrExpectedState":goal,
            "barrierOrUnmetNeed":barrier,
            "context":context
        })
    }

    #[test]
    fn assigns_only_one_explicit_match_after_every_candidate_is_closed() {
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let output = json!({
            "contract":PROBLEM_RESOLUTION_CONTRACT,
            "candidates":[
                {"problemRef":first,"dimensions":dimensions("same","same","same","same")},
                {"problemRef":second,"dimensions":dimensions("different","different","different","different")}
            ]
        });
        assert_eq!(
            decide_existing_resolution(output, &[first, second]),
            Ok(ExistingResolutionDecision::Assign(first))
        );
    }

    #[test]
    fn unknown_or_multiple_matches_are_never_resolved_by_vector_order() {
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let output = json!({
            "contract":PROBLEM_RESOLUTION_CONTRACT,
            "candidates":[
                {"problemRef":first,"dimensions":dimensions("same","same","same","same")},
                {"problemRef":second,"dimensions":dimensions("unknown","same","same","same")}
            ]
        });
        assert_eq!(
            decide_existing_resolution(output, &[first, second]),
            Ok(ExistingResolutionDecision::DeferAmbiguous)
        );
    }

    #[test]
    fn all_explicit_differences_defer_novel_instead_of_creating() {
        let first = Uuid::new_v4();
        let output = json!({
            "contract":PROBLEM_RESOLUTION_CONTRACT,
            "candidates":[{"problemRef":first,"dimensions":dimensions("different","different","different","different")}]
        });
        assert_eq!(
            decide_existing_resolution(output, &[first]),
            Ok(ExistingResolutionDecision::DeferNovel)
        );
    }

    #[test]
    fn omission_or_duplicate_candidate_is_a_protocol_failure_not_no_match() {
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let output = json!({
            "contract":PROBLEM_RESOLUTION_CONTRACT,
            "candidates":[
                {"problemRef":first,"dimensions":dimensions("different","different","different","different")},
                {"problemRef":first,"dimensions":dimensions("different","different","different","different")}
            ]
        });
        assert_eq!(
            decide_existing_resolution(output, &[first, second]),
            Err(ProblemResolutionContractError::CandidateSetMismatch)
        );
    }

    #[test]
    fn two_independent_matching_signals_can_propose_a_bounded_problem_definition() {
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let output = json!({
            "contract":PROBLEM_PAIR_CONTRACT,
            "firstSignalRef":first,
            "secondSignalRef":second,
            "dimensions":dimensions("same","same","same","same"),
            "proposedProblem":{
                "definition":"孩子在家庭作业中存在自主启动困难",
                "stableIdentity":{"actor":"孩子","barrier":"需要催促"},
                "includeCriteria":["需要持续外部催促才能开始作业"],
                "excludeCriteria":["仅偶发忘记作业"]
            }
        });
        assert!(matches!(
            decide_pair_creation(output, first, second, true, true),
            Ok(PairCreationDecision::Create(_))
        ));
    }

    #[test]
    fn one_author_or_one_source_cannot_create_a_problem() {
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let output = json!({
            "contract":PROBLEM_PAIR_CONTRACT,
            "firstSignalRef":first,
            "secondSignalRef":second,
            "dimensions":dimensions("same","same","same","same"),
            "proposedProblem":{
                "definition":"孩子在家庭作业中存在自主启动困难",
                "stableIdentity":{"actor":"孩子"},
                "includeCriteria":["需要催促"],
                "excludeCriteria":["偶发"]
            }
        });
        assert_eq!(
            decide_pair_creation(output, first, second, true, false),
            Ok(PairCreationDecision::DeferInsufficientIndependentEvidence)
        );
    }
}
