//! Deterministic policy core for COMMENT-RESEARCH-PROBLEM-RESOLUTION-V2.
//!
//! The model may describe an evidence-grounded frame and compare fixed dimensions, but it never
//! receives database identifiers as actions and it never decides whether a Problem is created.
//! This module validates the closed-world response and converts it into one durable outcome.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeRelation {
    InScope,
    OutOfScope,
    Uncertain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalKind {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Truth {
    Yes,
    No,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonDimension {
    Subject,
    Goal,
    Barrier,
    Context,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EligibilityInput {
    pub source_readable: bool,
    pub evidence_refs_valid: bool,
    pub scope_relation: ScopeRelation,
    pub kind: SignalKind,
    /// The person who experiences the difficulty, not an inferred author identity.
    pub subject_resolved: Truth,
    pub goal_or_expected_state: Truth,
    pub barrier_or_unmet_need: Truth,
    /// A missing fact that could change the scope or identity conclusion.
    pub material_context_missing: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EligibilityDecision {
    Eligible,
    SourceUnavailable,
    ProtocolFailure,
    OutOfScope,
    NotAUserProblem,
    DeferredContext,
}

pub fn decide_eligibility(input: &EligibilityInput) -> EligibilityDecision {
    if !input.source_readable {
        return EligibilityDecision::SourceUnavailable;
    }
    if !input.evidence_refs_valid {
        return EligibilityDecision::ProtocolFailure;
    }
    if input.scope_relation == ScopeRelation::OutOfScope {
        return EligibilityDecision::OutOfScope;
    }
    if !matches!(input.kind, SignalKind::Problem | SignalKind::Need) {
        return EligibilityDecision::NotAUserProblem;
    }
    if input.scope_relation == ScopeRelation::Uncertain
        || input.material_context_missing
        || input.subject_resolved == Truth::Unknown
        || input.goal_or_expected_state == Truth::Unknown
        || input.barrier_or_unmet_need == Truth::Unknown
    {
        return EligibilityDecision::DeferredContext;
    }
    if input.subject_resolved == Truth::No
        || input.goal_or_expected_state == Truth::No
        || input.barrier_or_unmet_need == Truth::No
    {
        return EligibilityDecision::NotAUserProblem;
    }
    EligibilityDecision::Eligible
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateComparison {
    /// Server-issued ordinal only. It is deliberately not a Problem UUID.
    pub candidate_index: usize,
    pub subject: Truth,
    pub goal: Truth,
    pub barrier: Truth,
    pub context: Truth,
    pub material_contradiction: Truth,
    pub evidence_refs_valid: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExistingResolutionInput {
    /// Exact set displayed to the model. The response is invalid if it omits or invents one.
    pub expected_candidate_indices: Vec<usize>,
    pub comparisons: Vec<CandidateComparison>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExistingResolutionDecision {
    MatchExisting { candidate_index: usize },
    DeferredNovel,
    DeferredAmbiguous,
    ProtocolFailure,
}

/// Resolves only a complete, closed-world comparison set.
///
/// `unknown` is never coerced to `no`. A candidate may be assigned only when it is the single
/// explicit match and every other candidate is an explicit non-match.
pub fn resolve_existing(input: &ExistingResolutionInput) -> ExistingResolutionDecision {
    if !valid_candidate_set(input) {
        return ExistingResolutionDecision::ProtocolFailure;
    }
    let mut matches = Vec::new();
    let mut unknown = false;
    for comparison in &input.comparisons {
        if !comparison.evidence_refs_valid {
            return ExistingResolutionDecision::ProtocolFailure;
        }
        if has_unknown(comparison) {
            unknown = true;
            continue;
        }
        if is_explicit_match(comparison) {
            matches.push(comparison.candidate_index);
        }
    }
    if unknown || matches.len() > 1 {
        return ExistingResolutionDecision::DeferredAmbiguous;
    }
    if let Some(candidate_index) = matches.first() {
        return ExistingResolutionDecision::MatchExisting {
            candidate_index: *candidate_index,
        };
    }
    ExistingResolutionDecision::DeferredNovel
}

fn valid_candidate_set(input: &ExistingResolutionInput) -> bool {
    let mut expected = input.expected_candidate_indices.clone();
    let mut actual = input
        .comparisons
        .iter()
        .map(|comparison| comparison.candidate_index)
        .collect::<Vec<_>>();
    expected.sort_unstable();
    actual.sort_unstable();
    expected == actual && expected.windows(2).all(|pair| pair[0] != pair[1])
}

fn has_unknown(comparison: &CandidateComparison) -> bool {
    matches!(comparison.subject, Truth::Unknown)
        || matches!(comparison.goal, Truth::Unknown)
        || matches!(comparison.barrier, Truth::Unknown)
        || matches!(comparison.context, Truth::Unknown)
        || matches!(comparison.material_contradiction, Truth::Unknown)
}

fn is_explicit_match(comparison: &CandidateComparison) -> bool {
    comparison.subject == Truth::Yes
        && comparison.goal == Truth::Yes
        && comparison.barrier == Truth::Yes
        && comparison.context == Truth::Yes
        && comparison.material_contradiction == Truth::No
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreationSignal {
    pub atom_ref: String,
    pub source_ref: String,
    /// Absence is insufficient to prove two signals independent.
    pub author_external_id: Option<String>,
    /// A known duplicate/repost group prevents independent corroboration.
    pub duplicate_group: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairComparison {
    pub subject: Truth,
    pub goal: Truth,
    pub barrier: Truth,
    pub context: Truth,
    pub material_contradiction: Truth,
    pub evidence_refs_valid: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedProblemDefinition {
    pub name: String,
    pub meaning: String,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub evidence_refs_valid: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairCreationInput {
    pub first: CreationSignal,
    pub second: CreationSignal,
    pub comparison: PairComparison,
    /// Only required when the explicit pair comparison would otherwise create a Problem. A
    /// non-match must be able to remain deferred without the model inventing a definition.
    pub definition: Option<SharedProblemDefinition>,
    /// The caller must still re-check this guard inside the write transaction.
    pub catalog_revision_current: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PairCreationDecision {
    CreateNewProblem,
    DeferredNovel,
    DeferredAmbiguous,
    ProtocolFailure,
    ReevaluateCatalog,
}

/// New Problem creation requires two independently sourced, explicitly equivalent signals.
pub fn decide_pair_creation(input: &PairCreationInput) -> PairCreationDecision {
    if !valid_signal(&input.first) || !valid_signal(&input.second) {
        return PairCreationDecision::ProtocolFailure;
    }
    if !input.comparison.evidence_refs_valid {
        return PairCreationDecision::ProtocolFailure;
    }
    if !independent_sources(&input.first, &input.second) {
        return PairCreationDecision::DeferredNovel;
    }
    if pair_has_unknown(&input.comparison) {
        return PairCreationDecision::DeferredAmbiguous;
    }
    if !pair_is_explicit_match(&input.comparison) {
        return PairCreationDecision::DeferredNovel;
    }
    if !input.definition.as_ref().is_some_and(valid_definition) {
        return PairCreationDecision::ProtocolFailure;
    }
    if !input.catalog_revision_current {
        return PairCreationDecision::ReevaluateCatalog;
    }
    PairCreationDecision::CreateNewProblem
}

fn valid_signal(signal: &CreationSignal) -> bool {
    non_empty(&signal.atom_ref, 100) && non_empty(&signal.source_ref, 100)
}

fn independent_sources(first: &CreationSignal, second: &CreationSignal) -> bool {
    first.atom_ref != second.atom_ref
        && first.source_ref != second.source_ref
        && first.author_external_id.is_some()
        && second.author_external_id.is_some()
        && first.author_external_id != second.author_external_id
        && !same_known_duplicate_group(first, second)
}

fn same_known_duplicate_group(first: &CreationSignal, second: &CreationSignal) -> bool {
    first.duplicate_group.is_some() && first.duplicate_group == second.duplicate_group
}

fn pair_has_unknown(comparison: &PairComparison) -> bool {
    [
        comparison.subject,
        comparison.goal,
        comparison.barrier,
        comparison.context,
        comparison.material_contradiction,
    ]
    .contains(&Truth::Unknown)
}

fn pair_is_explicit_match(comparison: &PairComparison) -> bool {
    comparison.subject == Truth::Yes
        && comparison.goal == Truth::Yes
        && comparison.barrier == Truth::Yes
        && comparison.context == Truth::Yes
        && comparison.material_contradiction == Truth::No
}

fn valid_definition(definition: &SharedProblemDefinition) -> bool {
    definition.evidence_refs_valid
        && non_empty(&definition.name, 120)
        && non_empty(&definition.meaning, 1000)
        && !definition.include.is_empty()
        && definition.include.iter().all(|entry| non_empty(entry, 300))
        && definition.exclude.iter().all(|entry| non_empty(entry, 300))
}

fn non_empty(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty() && value.chars().count() <= maximum
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eligible_input() -> EligibilityInput {
        EligibilityInput {
            source_readable: true,
            evidence_refs_valid: true,
            scope_relation: ScopeRelation::InScope,
            kind: SignalKind::Problem,
            subject_resolved: Truth::Yes,
            goal_or_expected_state: Truth::Yes,
            barrier_or_unmet_need: Truth::Yes,
            material_context_missing: false,
        }
    }

    fn no_contradiction() -> CandidateComparison {
        CandidateComparison {
            candidate_index: 0,
            subject: Truth::Yes,
            goal: Truth::Yes,
            barrier: Truth::Yes,
            context: Truth::Yes,
            material_contradiction: Truth::No,
            evidence_refs_valid: true,
        }
    }

    #[test]
    fn eligibility_does_not_turn_unknown_into_negative() {
        let mut input = eligible_input();
        input.subject_resolved = Truth::Unknown;
        assert_eq!(
            decide_eligibility(&input),
            EligibilityDecision::DeferredContext
        );
    }

    #[test]
    fn belief_is_preserved_but_not_admitted_to_problem_resolution() {
        let mut input = eligible_input();
        input.kind = SignalKind::Belief;
        assert_eq!(
            decide_eligibility(&input),
            EligibilityDecision::NotAUserProblem
        );
    }

    #[test]
    fn scope_uncertainty_is_not_out_of_scope() {
        let mut input = eligible_input();
        input.scope_relation = ScopeRelation::Uncertain;
        assert_eq!(
            decide_eligibility(&input),
            EligibilityDecision::DeferredContext
        );
    }

    #[test]
    fn exactly_one_complete_match_is_assigned() {
        let mut non_match = no_contradiction();
        non_match.candidate_index = 1;
        non_match.barrier = Truth::No;
        assert_eq!(
            resolve_existing(&ExistingResolutionInput {
                expected_candidate_indices: vec![0, 1],
                comparisons: vec![no_contradiction(), non_match],
            }),
            ExistingResolutionDecision::MatchExisting { candidate_index: 0 }
        );
    }

    #[test]
    fn model_must_answer_every_server_candidate() {
        assert_eq!(
            resolve_existing(&ExistingResolutionInput {
                expected_candidate_indices: vec![0, 1],
                comparisons: vec![no_contradiction()],
            }),
            ExistingResolutionDecision::ProtocolFailure
        );
    }

    #[test]
    fn unknown_other_candidate_blocks_a_seemingly_good_match() {
        let mut unknown = no_contradiction();
        unknown.candidate_index = 1;
        unknown.context = Truth::Unknown;
        assert_eq!(
            resolve_existing(&ExistingResolutionInput {
                expected_candidate_indices: vec![0, 1],
                comparisons: vec![no_contradiction(), unknown],
            }),
            ExistingResolutionDecision::DeferredAmbiguous
        );
    }

    #[test]
    fn explicit_non_matches_wait_for_independent_corroboration() {
        let mut first = no_contradiction();
        first.subject = Truth::No;
        assert_eq!(
            resolve_existing(&ExistingResolutionInput {
                expected_candidate_indices: vec![0],
                comparisons: vec![first],
            }),
            ExistingResolutionDecision::DeferredNovel
        );
    }

    fn creation_input() -> PairCreationInput {
        PairCreationInput {
            first: CreationSignal {
                atom_ref: "atom-1".into(),
                source_ref: "comment-1".into(),
                author_external_id: Some("author-1".into()),
                duplicate_group: None,
            },
            second: CreationSignal {
                atom_ref: "atom-2".into(),
                source_ref: "comment-2".into(),
                author_external_id: Some("author-2".into()),
                duplicate_group: None,
            },
            comparison: PairComparison {
                subject: Truth::Yes,
                goal: Truth::Yes,
                barrier: Truth::Yes,
                context: Truth::Yes,
                material_contradiction: Truth::No,
                evidence_refs_valid: true,
            },
            definition: Some(SharedProblemDefinition {
                name: "作业启动困难".into(),
                meaning: "ADHD 相关家庭中，孩子在家庭作业场景难以自主开始。".into(),
                include: vec!["需要外部催促才能开始作业".into()],
                exclude: vec!["仅表示不喜欢某门课但未表达启动障碍".into()],
                evidence_refs_valid: true,
            }),
            catalog_revision_current: true,
        }
    }

    #[test]
    fn one_signal_never_creates_a_problem() {
        let mut input = creation_input();
        input.second.source_ref = input.first.source_ref.clone();
        assert_eq!(
            decide_pair_creation(&input),
            PairCreationDecision::DeferredNovel
        );
    }

    #[test]
    fn same_author_is_not_independent_corroboration() {
        let mut input = creation_input();
        input.second.author_external_id = input.first.author_external_id.clone();
        assert_eq!(
            decide_pair_creation(&input),
            PairCreationDecision::DeferredNovel
        );
    }

    #[test]
    fn a_non_matching_pair_never_needs_an_invented_definition() {
        let mut input = creation_input();
        input.definition = None;
        input.comparison.barrier = Truth::No;
        assert_eq!(
            decide_pair_creation(&input),
            PairCreationDecision::DeferredNovel
        );
    }

    #[test]
    fn a_stale_catalog_requires_recall_before_create() {
        let mut input = creation_input();
        input.catalog_revision_current = false;
        assert_eq!(
            decide_pair_creation(&input),
            PairCreationDecision::ReevaluateCatalog
        );
    }

    #[test]
    fn independently_equivalent_pair_can_enter_creation_transaction() {
        assert_eq!(
            decide_pair_creation(&creation_input()),
            PairCreationDecision::CreateNewProblem
        );
    }
}
