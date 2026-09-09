//! Stable Problem identity and Atom membership for COMMENT-RESEARCH-RESET-001.
//!
//! Candidate recall never writes these relations by itself. A caller must supply an explicit
//! `new_problem` or `same_problem` decision with bounded evidence; this module persists the
//! durable identity and prevents one Atom from belonging to two current Problems.

use crate::comment_research::comment_source_hash;
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::Value;
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

#[derive(Debug, thiserror::Error)]
pub enum CommentResearchProblemError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("the Atom is unavailable because its source is no longer readable")]
    AtomUnavailable,
    #[error("the Atom already has a current Problem membership")]
    AtomAlreadyAssigned,
    #[error("the requested Problem definition is unavailable")]
    ProblemUnavailable,
    #[error("the Problem decision violates the V1 contract")]
    InvalidDecision,
}

/// Accepts an explicit decision that an unassigned Atom is the first member of a new Problem.
/// `deterministic` bootstrap is allowed only when the supplied candidate list is explicitly empty;
/// a model decision must reference its invocation ledger row.
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
    let membership_policy_hash =
        lock_unassigned_readable_atom(&mut transaction, admission.atom_ref).await?;
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
    .bind(&membership_policy_hash)
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
        &membership_policy_hash,
        &admission.decision_evidence,
        admission.invocation_ref,
    )
    .await?;
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
    let membership_policy_hash =
        lock_unassigned_readable_atom(&mut transaction, admission.atom_ref).await?;
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
        &membership_policy_hash,
        &admission.decision_evidence,
        admission.invocation_ref,
    )
    .await?;
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
) -> Result<String, CommentResearchProblemError> {
    let row = sqlx::query(
        "SELECT policy.membership_policy_hash \
         FROM linggan_comment_research_atom atom \
         JOIN linggan_comment_research_derivation_readable derivation \
           ON derivation.derivation_ref=atom.derivation_ref \
         JOIN linggan_comment_research_run run ON run.run_ref=atom.run_ref \
         JOIN linggan_comment_research_policy_revision policy \
           ON policy.policy_revision_ref=run.policy_revision_ref \
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
    Ok(row.get("membership_policy_hash"))
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
    comment_source_hash(&format!(
        "{}\u{0}{}",
        definition.name.trim().to_lowercase(),
        definition.meaning.trim().to_lowercase()
    ))
}

fn valid_basis(basis: ProblemMembershipBasis, invocation_ref: Option<Uuid>) -> bool {
    basis != ProblemMembershipBasis::ModelDecision || invocation_ref.is_some()
}

fn valid_decision_evidence(evidence: &Value, expected_decision: &str) -> bool {
    evidence.is_object()
        && evidence.get("decision").and_then(Value::as_str) == Some(expected_decision)
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
}
