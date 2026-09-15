//! Stable Problem identity and Atom membership for COMMENT-RESEARCH-RESET-001.
//!
//! Candidate recall never writes these relations by itself. A caller must supply an explicit
//! `new_problem` or `same_problem` decision with bounded evidence; this module persists the
//! durable identity and prevents one Atom from belonging to two current Problems.

use crate::{
    comment_research_kernel::{CommentResearchKernelError, refresh_run_completion},
    research_text::content_hash,
};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProblemMembershipBasis {
    Deterministic,
    ModelDecision,
    Manual,
}

impl ProblemMembershipBasis {
    fn as_db(self) -> &'static str {
        match self {
            Self::Deterministic => "deterministic",
            Self::ModelDecision => "model_decision",
            Self::Manual => "manual",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProblemDefinitionProposal {
    pub name: String,
    pub meaning: String,
}

/// A V2 stable identity is only admitted from an independently corroborated pair. `include` and
/// `exclude` are semantic boundaries, not optional display copy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StableProblemDefinitionProposal {
    pub name: String,
    pub meaning: String,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NewProblemPairAdmission {
    pub first_atom_ref: Uuid,
    pub second_atom_ref: Uuid,
    pub expected_catalog_revision: i64,
    pub definition: StableProblemDefinitionProposal,
    pub decision_evidence: Value,
    pub invocation_ref: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NewProblemAdmission {
    pub atom_ref: Uuid,
    pub definition: ProblemDefinitionProposal,
    pub basis: ProblemMembershipBasis,
    pub decision_evidence: Value,
    pub invocation_ref: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExistingProblemAdmission {
    pub atom_ref: Uuid,
    pub problem_ref: Uuid,
    pub definition_revision: i32,
    pub basis: ProblemMembershipBasis,
    pub decision_evidence: Value,
    pub invocation_ref: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProblemAdmissionReceipt {
    pub problem_ref: Uuid,
    pub definition_revision: i32,
    pub membership_ref: Uuid,
    pub basis: ProblemMembershipBasis,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProblemPairAdmissionReceipt {
    pub problem_ref: Uuid,
    pub definition_revision: i32,
    pub first_membership_ref: Uuid,
    pub second_membership_ref: Uuid,
    pub catalog_revision: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum CommentResearchProblemError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Kernel(#[from] CommentResearchKernelError),
    #[error("the Atom is unavailable because its source is no longer readable")]
    AtomUnavailable,
    #[error("the Atom already has a current Problem membership")]
    AtomAlreadyAssigned,
    #[error("the Atom kind is not problem-bearing in V1")]
    AtomNotProblemBearing,
    #[error("the requested Problem definition is unavailable")]
    ProblemUnavailable,
    #[error("the Problem decision violates the V1 contract")]
    InvalidDecision,
    #[error("the V2 pair does not consist of two deferred novel signals")]
    PairNotEligible,
    #[error("a V2 scoped Atom can create a Problem only through an independent pair")]
    PairRequiredForV2,
    #[error("the V2 pair belongs to different policy scopes")]
    PairScopeMismatch,
    #[error("the V2 pair does not prove independent comment sources")]
    PairSourcesNotIndependent,
    #[error("the Problem catalog changed after the pair was compared")]
    CatalogChanged,
}

/// Creates a V2 Stable Problem and both memberships atomically. Callers must provide a pair
/// comparison already accepted by the closed-world policy core; this function re-checks the
/// durable deferred state and fences the write with the scope catalog revision.
pub async fn admit_new_problem_pair(
    database: &Database,
    admission: NewProblemPairAdmission,
) -> Result<ProblemPairAdmissionReceipt, CommentResearchProblemError> {
    if admission.first_atom_ref == admission.second_atom_ref
        || admission.expected_catalog_revision < 0
        || !valid_stable_definition(&admission.definition)
        || !valid_pair_decision_evidence(&admission.decision_evidence)
        || admission.invocation_ref.is_none()
    {
        return Err(CommentResearchProblemError::InvalidDecision);
    }
    let mut transaction = database.pool().begin().await?;
    // Lock atoms in a stable order, so independently scheduled pair proposals cannot deadlock.
    let (first_ref, second_ref, reverse_receipts) =
        if admission.first_atom_ref < admission.second_atom_ref {
            (admission.first_atom_ref, admission.second_atom_ref, false)
        } else {
            (admission.second_atom_ref, admission.first_atom_ref, true)
        };
    let first = lock_unassigned_readable_atom(&mut transaction, first_ref).await?;
    let second = lock_unassigned_readable_atom(&mut transaction, second_ref).await?;
    ensure_problem_bearing_kind(&first.kind)?;
    ensure_problem_bearing_kind(&second.kind)?;
    if first.problem_resolution_contract.as_deref()
        != Some("comment-research.problem-resolution.v2")
        || second.problem_resolution_contract.as_deref()
            != Some("comment-research.problem-resolution.v2")
        || !valid_v2_in_scope_problem_frame(first.problem_frame.as_ref())
        || !valid_v2_in_scope_problem_frame(second.problem_frame.as_ref())
    {
        return Err(CommentResearchProblemError::PairNotEligible);
    }
    let scope_domain_ref = first
        .scope_domain_ref
        .ok_or(CommentResearchProblemError::PairScopeMismatch)?;
    if second.scope_domain_ref != Some(scope_domain_ref)
        || first.membership_policy_hash != second.membership_policy_hash
    {
        return Err(CommentResearchProblemError::PairScopeMismatch);
    }
    if first.source_ref == second.source_ref
        || first.author_external_id.is_none()
        || second.author_external_id.is_none()
        || first.author_external_id == second.author_external_id
        || first.body_hash == second.body_hash
    {
        return Err(CommentResearchProblemError::PairSourcesNotIndependent);
    }
    let current_revision: i64 = sqlx::query_scalar(
        "SELECT revision FROM linggan_comment_research_problem_catalog_guard \
         WHERE scope_domain_ref=$1 FOR UPDATE",
    )
    .bind(scope_domain_ref)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or(CommentResearchProblemError::PairScopeMismatch)?;
    if current_revision != admission.expected_catalog_revision {
        return Err(CommentResearchProblemError::CatalogChanged);
    }
    for atom_ref in [first_ref, second_ref] {
        let deferred: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM linggan_comment_research_problem_resolution \
             WHERE atom_ref=$1 AND state='succeeded' AND decision_kind='deferred_novel' \
               AND catalog_revision_at_recall=$2)",
        )
        .bind(atom_ref)
        .bind(current_revision)
        .fetch_one(&mut *transaction)
        .await?;
        if !deferred {
            return Err(CommentResearchProblemError::PairNotEligible);
        }
    }
    let problem_ref = Uuid::new_v4();
    let membership_a = Uuid::new_v4();
    let membership_b = Uuid::new_v4();
    let identity = stable_identity(&admission.definition);
    let definition_hash = content_hash(&identity.to_string());
    sqlx::query("INSERT INTO linggan_comment_research_problem(problem_ref) VALUES($1)")
        .bind(problem_ref)
        .execute(&mut *transaction)
        .await?;
    sqlx::query(
        "INSERT INTO linggan_comment_research_problem_definition( \
             problem_ref,revision,name,meaning,definition_hash,policy_hash,invocation_ref,stable_identity,scope_domain_ref \
         ) VALUES($1,1,$2,$3,$4,$5,$6,$7,$8)",
    )
    .bind(problem_ref)
    .bind(admission.definition.name.trim())
    .bind(admission.definition.meaning.trim())
    .bind(definition_hash)
    .bind(&first.membership_policy_hash)
    .bind(admission.invocation_ref)
    .bind(identity)
    .bind(scope_domain_ref)
    .execute(&mut *transaction)
    .await?;
    let membership_evidence = json!({
        "decision":"v2_pair_create",
        "pairDecision":admission.decision_evidence,
        "catalogRevision":current_revision,
    });
    insert_membership(
        &mut transaction,
        membership_a,
        first_ref,
        problem_ref,
        1,
        ProblemMembershipBasis::ModelDecision,
        &first.membership_policy_hash,
        &membership_evidence,
        admission.invocation_ref,
    )
    .await?;
    insert_membership(
        &mut transaction,
        membership_b,
        second_ref,
        problem_ref,
        1,
        ProblemMembershipBasis::ModelDecision,
        &second.membership_policy_hash,
        &membership_evidence,
        admission.invocation_ref,
    )
    .await?;
    let catalog_revision: i64 = sqlx::query_scalar(
        "UPDATE linggan_comment_research_problem_catalog_guard \
         SET revision=revision+1,updated_at=scope_001_now() \
         WHERE scope_domain_ref=$1 RETURNING revision",
    )
    .bind(scope_domain_ref)
    .fetch_one(&mut *transaction)
    .await?;
    for run_ref in [first.run_ref, second.run_ref] {
        refresh_run_completion(&mut transaction, run_ref).await?;
    }
    transaction.commit().await?;
    let (first_membership_ref, second_membership_ref) = if reverse_receipts {
        (membership_b, membership_a)
    } else {
        (membership_a, membership_b)
    };
    Ok(ProblemPairAdmissionReceipt {
        problem_ref,
        definition_revision: 1,
        first_membership_ref,
        second_membership_ref,
        catalog_revision,
    })
}

/// Historical V1-only admission path. A V2 scoped Atom must never use this single-Atom create
/// path: its only creation authority is `admit_new_problem_pair` after two independent signals.
/// The function remains to read and repair immutable V1 packet Runs without silently applying
/// V2 semantics to them.
pub async fn admit_new_problem(
    database: &Database,
    admission: NewProblemAdmission,
) -> Result<ProblemAdmissionReceipt, CommentResearchProblemError> {
    if !valid_text(&admission.definition.name, 120)
        || !valid_text(&admission.definition.meaning, 1000)
        || !valid_decision_evidence(&admission.decision_evidence, "new_problem")
        || !valid_basis(admission.basis, admission.invocation_ref)
        || (admission.basis == ProblemMembershipBasis::Deterministic
            && !has_empty_candidate_list(&admission.decision_evidence))
    {
        return Err(CommentResearchProblemError::InvalidDecision);
    }

    let mut transaction = database.pool().begin().await?;
    let atom = lock_unassigned_readable_atom(&mut transaction, admission.atom_ref).await?;
    ensure_problem_bearing_kind(&atom.kind)?;
    if atom.problem_resolution_contract.as_deref() == Some("comment-research.problem-resolution.v2") {
        return Err(CommentResearchProblemError::PairRequiredForV2);
    }
    let problem_ref = Uuid::new_v4();
    let membership_ref = Uuid::new_v4();
    let definition_hash = definition_hash(&admission.definition);
    sqlx::query("INSERT INTO linggan_comment_research_problem(problem_ref) VALUES($1)")
        .bind(problem_ref)
        .execute(&mut *transaction)
        .await?;
    sqlx::query(
        "INSERT INTO linggan_comment_research_problem_definition( \
             problem_ref,revision,name,meaning,definition_hash,policy_hash,invocation_ref \
         ) VALUES($1,1,$2,$3,$4,$5,$6)",
    )
    .bind(problem_ref)
    .bind(admission.definition.name.trim())
    .bind(admission.definition.meaning.trim())
    .bind(definition_hash)
    .bind(&atom.membership_policy_hash)
    .bind(admission.invocation_ref)
    .execute(&mut *transaction)
    .await?;
    insert_membership(
        &mut transaction,
        membership_ref,
        admission.atom_ref,
        problem_ref,
        1,
        admission.basis,
        &atom.membership_policy_hash,
        &admission.decision_evidence,
        admission.invocation_ref,
    )
    .await?;
    refresh_run_completion(&mut transaction, atom.run_ref).await?;
    transaction.commit().await?;
    Ok(ProblemAdmissionReceipt {
        problem_ref,
        definition_revision: 1,
        membership_ref,
        basis: admission.basis,
    })
}

/// Accepts an explicit `same_problem` decision for an existing definition revision. It cannot
/// revise the target definition, merge Problems, or replace a current membership.
pub async fn admit_existing_problem(
    database: &Database,
    admission: ExistingProblemAdmission,
) -> Result<ProblemAdmissionReceipt, CommentResearchProblemError> {
    if admission.definition_revision <= 0
        || !valid_decision_evidence(&admission.decision_evidence, "same_problem")
        || !valid_basis(admission.basis, admission.invocation_ref)
    {
        return Err(CommentResearchProblemError::InvalidDecision);
    }

    let mut transaction = database.pool().begin().await?;
    let atom = lock_unassigned_readable_atom(&mut transaction, admission.atom_ref).await?;
    ensure_problem_bearing_kind(&atom.kind)?;
    let definition_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS( \
             SELECT 1 \
             FROM linggan_comment_research_problem problem \
             JOIN linggan_comment_research_problem_definition definition USING(problem_ref) \
             WHERE problem.problem_ref=$1 AND problem.state='active' AND definition.revision=$2 \
             FOR KEY SHARE OF problem,definition \
         )",
    )
    .bind(admission.problem_ref)
    .bind(admission.definition_revision)
    .fetch_one(&mut *transaction)
    .await?;
    if !definition_exists {
        return Err(CommentResearchProblemError::ProblemUnavailable);
    }
    let membership_ref = Uuid::new_v4();
    insert_membership(
        &mut transaction,
        membership_ref,
        admission.atom_ref,
        admission.problem_ref,
        admission.definition_revision,
        admission.basis,
        &atom.membership_policy_hash,
        &admission.decision_evidence,
        admission.invocation_ref,
    )
    .await?;
    refresh_run_completion(&mut transaction, atom.run_ref).await?;
    transaction.commit().await?;
    Ok(ProblemAdmissionReceipt {
        problem_ref: admission.problem_ref,
        definition_revision: admission.definition_revision,
        membership_ref,
        basis: admission.basis,
    })
}

async fn lock_unassigned_readable_atom(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    atom_ref: Uuid,
) -> Result<ReadableUnassignedAtom, CommentResearchProblemError> {
    let row = sqlx::query(
        "SELECT atom.run_ref,atom.kind,atom.problem_frame,policy.membership_policy_hash,policy.problem_resolution_contract,policy.problem_scope_domain_ref, \
                source.material_ref,source.author_external_id,source.body_text \
         FROM linggan_comment_research_atom atom \
         JOIN linggan_comment_research_derivation_readable derivation \
           ON derivation.derivation_ref=atom.derivation_ref \
         JOIN linggan_comment_research_run run ON run.run_ref=atom.run_ref \
         JOIN linggan_comment_research_policy_revision policy \
           ON policy.policy_revision_ref=run.policy_revision_ref \
         JOIN linggan_comment_research_readable source ON source.material_ref=derivation.source_ref \
         WHERE atom.atom_ref=$1 \
         FOR UPDATE OF atom",
    )
    .bind(atom_ref)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(CommentResearchProblemError::AtomUnavailable)?;
    let assigned: bool = sqlx::query_scalar(
        "SELECT EXISTS( \
             SELECT 1 FROM linggan_comment_research_atom_problem_membership \
             WHERE atom_ref=$1 AND current \
             FOR KEY SHARE \
         )",
    )
    .bind(atom_ref)
    .fetch_one(&mut **transaction)
    .await?;
    if assigned {
        return Err(CommentResearchProblemError::AtomAlreadyAssigned);
    }
    Ok(ReadableUnassignedAtom {
        run_ref: row.get("run_ref"),
        kind: row.get("kind"),
        problem_frame: row.get("problem_frame"),
        membership_policy_hash: row.get("membership_policy_hash"),
        problem_resolution_contract: row.get("problem_resolution_contract"),
        scope_domain_ref: row.get("problem_scope_domain_ref"),
        source_ref: row.get("material_ref"),
        author_external_id: row
            .get::<Option<String>, _>("author_external_id")
            .filter(|value| !value.trim().is_empty()),
        body_hash: content_hash(
            &row.get::<Option<String>, _>("body_text")
                .unwrap_or_default(),
        ),
    })
}

struct ReadableUnassignedAtom {
    run_ref: Uuid,
    kind: String,
    problem_frame: Option<Value>,
    membership_policy_hash: String,
    problem_resolution_contract: Option<String>,
    scope_domain_ref: Option<Uuid>,
    source_ref: Uuid,
    author_external_id: Option<String>,
    body_hash: String,
}

fn ensure_problem_bearing_kind(kind: &str) -> Result<(), CommentResearchProblemError> {
    if matches!(kind, "problem" | "need") {
        Ok(())
    } else {
        Err(CommentResearchProblemError::AtomNotProblemBearing)
    }
}

fn valid_v2_problem_frame(frame: Option<&Value>) -> bool {
    let Some(frame) = frame.and_then(Value::as_object) else {
        return false;
    };
    matches!(frame.get("scopeRelation").and_then(Value::as_str), Some("in_scope" | "out_of_scope" | "uncertain"))
        && ["subject", "goal", "barrier", "context"]
            .iter()
            .all(|field| valid_v2_problem_frame_field(frame.get(*field)))
}

fn valid_v2_in_scope_problem_frame(frame: Option<&Value>) -> bool {
    valid_v2_problem_frame(frame)
        && frame
            .and_then(Value::as_object)
            .and_then(|value| value.get("scopeRelation"))
            .and_then(Value::as_str)
            == Some("in_scope")
}

fn valid_v2_problem_frame_field(field: Option<&Value>) -> bool {
    let Some(field) = field.and_then(Value::as_object) else {
        return false;
    };
    let value = field.get("value");
    let basis = field.get("basis").and_then(Value::as_str);
    let refs = field.get("evidenceRefs").and_then(Value::as_array);
    match (value, basis, refs) {
        (Some(Value::Null), Some("unknown"), Some(refs)) => refs.is_empty(),
        (Some(Value::String(value)), Some("explicit" | "context_resolved"), Some(refs)) => {
            valid_text(value, 300)
                && refs.len() == 1
                && refs.first().and_then(Value::as_str) == Some("atom_evidence")
        }
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
async fn insert_membership(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    membership_ref: Uuid,
    atom_ref: Uuid,
    problem_ref: Uuid,
    definition_revision: i32,
    basis: ProblemMembershipBasis,
    policy_hash: &str,
    decision_evidence: &Value,
    invocation_ref: Option<Uuid>,
) -> Result<(), CommentResearchProblemError> {
    sqlx::query(
        "INSERT INTO linggan_comment_research_atom_problem_membership( \
             membership_ref,atom_ref,problem_ref,definition_revision,relation,basis,policy_hash, \
             decision_evidence,invocation_ref,current \
         ) VALUES($1,$2,$3,$4,'same',$5,$6,$7,$8,true)",
    )
    .bind(membership_ref)
    .bind(atom_ref)
    .bind(problem_ref)
    .bind(definition_revision)
    .bind(basis.as_db())
    .bind(policy_hash)
    .bind(decision_evidence)
    .bind(invocation_ref)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

fn definition_hash(definition: &ProblemDefinitionProposal) -> String {
    // Normalization deliberately stops at surrounding whitespace and case. A qualifier or a
    // negation changes the stable definition identity and therefore cannot be silently merged.
    content_hash(&format!(
        "{}\u{0}{}",
        definition.name.trim().to_lowercase(),
        definition.meaning.trim().to_lowercase()
    ))
}

fn valid_stable_definition(definition: &StableProblemDefinitionProposal) -> bool {
    valid_text(&definition.name, 120)
        && valid_text(&definition.meaning, 1000)
        && !definition.include.is_empty()
        && !definition.exclude.is_empty()
        && definition
            .include
            .iter()
            .all(|value| valid_text(value, 300))
        && definition
            .exclude
            .iter()
            .all(|value| valid_text(value, 300))
}

fn stable_identity(definition: &StableProblemDefinitionProposal) -> Value {
    json!({
        "name": definition.name.trim(),
        "definition": definition.meaning.trim(),
        "include": definition.include.iter().map(|value| value.trim()).collect::<Vec<_>>(),
        "exclude": definition.exclude.iter().map(|value| value.trim()).collect::<Vec<_>>(),
    })
}

fn valid_pair_decision_evidence(evidence: &Value) -> bool {
    evidence.is_object()
        && evidence.get("decision").and_then(Value::as_str) == Some("v2_pair_equivalent")
        && serde_json::to_string(evidence)
            .is_ok_and(|serialized| serialized.chars().count() <= 4000)
}

fn valid_basis(basis: ProblemMembershipBasis, invocation_ref: Option<Uuid>) -> bool {
    basis != ProblemMembershipBasis::ModelDecision || invocation_ref.is_some()
}

fn valid_decision_evidence(evidence: &Value, expected_decision: &str) -> bool {
    evidence.is_object()
        && matches!(
            evidence.get("decision").and_then(Value::as_str),
            Some(decision) if decision == expected_decision
                || (expected_decision == "same_problem" && decision == "v2_existing_problem")
        )
        && serde_json::to_string(evidence)
            .is_ok_and(|serialized| serialized.chars().count() <= 4000)
}

fn has_empty_candidate_list(evidence: &Value) -> bool {
    evidence
        .get("candidateRefs")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty)
}

fn valid_text(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty() && value.chars().count() <= maximum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn definition_identity_normalizes_only_case_and_outer_whitespace() {
        let baseline = ProblemDefinitionProposal {
            name: " 写作业启动困难 ".into(),
            meaning: "孩子很难开始完成作业".into(),
        };
        let casing_only = ProblemDefinitionProposal {
            name: "写作业启动困难".into(),
            meaning: "孩子很难开始完成作业".into(),
        };
        let negated = ProblemDefinitionProposal {
            name: "写作业启动困难".into(),
            meaning: "孩子并非很难开始完成作业".into(),
        };
        assert_eq!(definition_hash(&baseline), definition_hash(&casing_only));
        assert_ne!(definition_hash(&baseline), definition_hash(&negated));
    }

    #[test]
    fn deterministic_new_problem_requires_an_explicit_empty_candidate_set() {
        assert!(has_empty_candidate_list(&serde_json::json!({
            "decision":"new_problem",
            "candidateRefs":[]
        })));
        assert!(!has_empty_candidate_list(&serde_json::json!({
            "decision":"new_problem"
        })));
    }

    #[test]
    fn model_decisions_require_an_invocation_receipt() {
        assert!(!valid_basis(ProblemMembershipBasis::ModelDecision, None));
        assert!(valid_basis(
            ProblemMembershipBasis::Manual,
            Some(Uuid::nil())
        ));
    }

    #[test]
    fn existing_admission_accepts_only_the_v2_program_owned_decision_name() {
        assert!(valid_decision_evidence(
            &serde_json::json!({"decision":"v2_existing_problem","candidateIndex":0}),
            "same_problem"
        ));
        assert!(!valid_decision_evidence(
            &serde_json::json!({"decision":"v2_pair_create"}),
            "same_problem"
        ));
    }

    #[test]
    fn only_problem_and_need_atoms_can_be_problem_members() {
        assert!(ensure_problem_bearing_kind("problem").is_ok());
        assert!(ensure_problem_bearing_kind("need").is_ok());
        assert!(matches!(
            ensure_problem_bearing_kind("solution"),
            Err(CommentResearchProblemError::AtomNotProblemBearing)
        ));
    }

    #[test]
    fn stable_problem_identity_carries_explicit_inclusion_and_exclusion_boundaries() {
        let definition = StableProblemDefinitionProposal {
            name: "作业启动困难".into(),
            meaning: "孩子在家庭作业情境难以自主开始。".into(),
            include: vec!["需外部催促才开始作业".into()],
            exclude: vec!["只是不喜欢某门课但未表达启动障碍".into()],
        };
        assert!(valid_stable_definition(&definition));
        let identity = stable_identity(&definition);
        assert_eq!(identity["name"], "作业启动困难");
        assert_eq!(identity["include"][0], "需外部催促才开始作业");
        assert_eq!(identity["exclude"][0], "只是不喜欢某门课但未表达启动障碍");
    }

    #[test]
    fn a_stable_problem_definition_cannot_omit_exclusion_boundary() {
        assert!(!valid_stable_definition(&StableProblemDefinitionProposal {
            name: "作业启动困难".into(),
            meaning: "孩子难以开始作业。".into(),
            include: vec!["需外部催促才开始作业".into()],
            exclude: vec![],
        }));
    }
}
