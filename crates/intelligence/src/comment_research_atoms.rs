//! Structured semantic-atom acceptance for COMMENT-RESEARCH-RESET-001.
//!
//! A model proposes spans over frozen research text. This module—not the model—maps them back to
//! immutable source offsets and decides whether the proposal satisfies the storage contract.

use crate::comment_research_kernel::{ResearchRunItemClaim, refresh_run_completion};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::Row;
use uuid::Uuid;

const MAX_ATOMS_PER_ITEM: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AtomKind {
    Problem,
    Need,
    Solution,
    Experience,
}

impl AtomKind {
    fn as_db(&self) -> &'static str {
        match self {
            Self::Problem => "problem",
            Self::Need => "need",
            Self::Solution => "solution",
            Self::Experience => "experience",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AtomBasis {
    Explicit,
    ContextResolved,
}

impl AtomBasis {
    fn as_db(&self) -> &'static str {
        match self {
            Self::Explicit => "explicit",
            Self::ContextResolved => "context_resolved",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticAtomProposal {
    pub kind: AtomKind,
    pub proposition: String,
    pub basis: AtomBasis,
    pub evidence_start: usize,
    pub evidence_end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum SemanticExtractionOutput {
    Atoms { atoms: Vec<SemanticAtomProposal> },
    NoSignal { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AtomAcceptanceReceipt {
    pub state: String,
    pub accepted_atoms: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum CommentResearchAtomError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("the run item is no longer claimed")]
    ClaimLost,
    #[error("the semantic extraction output violates the V1 contract")]
    InvalidOutput,
    #[error("the persisted derivation cannot map research spans to source spans")]
    DerivationCorrupt,
}

struct ClaimedInput {
    research_text: String,
    offsets: Vec<(usize, usize)>,
    extraction_rule_hash: String,
}

pub async fn accept_semantic_output(
    database: &Database,
    claim: &ResearchRunItemClaim,
    output: SemanticExtractionOutput,
    invocation_ref: Option<Uuid>,
) -> Result<AtomAcceptanceReceipt, CommentResearchAtomError> {
    let mut transaction = database.pool().begin().await?;
    let input = read_claimed_input(&mut transaction, claim).await?;
    let receipt = match output {
        SemanticExtractionOutput::Atoms { atoms } => {
            let accepted =
                accept_atoms(&mut transaction, claim, &input, &atoms, invocation_ref).await?;
            finish_item(&mut transaction, claim, "succeeded").await?;
            AtomAcceptanceReceipt {
                state: "succeeded".into(),
                accepted_atoms: accepted,
            }
        }
        SemanticExtractionOutput::NoSignal { reason } => {
            if !valid_text(&reason, 200) {
                return Err(CommentResearchAtomError::InvalidOutput);
            }
            finish_item(&mut transaction, claim, "no_signal").await?;
            AtomAcceptanceReceipt {
                state: "no_signal".into(),
                accepted_atoms: 0,
            }
        }
    };
    refresh_run_completion(&mut transaction, claim.run_ref)
        .await
        .map_err(|error| match error {
            crate::comment_research_kernel::CommentResearchKernelError::Database(error) => {
                CommentResearchAtomError::Database(error)
            }
            _ => CommentResearchAtomError::ClaimLost,
        })?;
    transaction.commit().await?;
    Ok(receipt)
}

async fn read_claimed_input(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    claim: &ResearchRunItemClaim,
) -> Result<ClaimedInput, CommentResearchAtomError> {
    let row = sqlx::query(
        "SELECT derivation.research_text,derivation.research_offsets,policy.extraction_rule_hash \
         FROM linggan_comment_research_run_item item \
         JOIN linggan_comment_research_derivation_readable derivation \
           ON derivation.derivation_ref=item.derivation_ref \
         JOIN linggan_comment_research_run run USING(run_ref) \
         JOIN linggan_comment_research_policy_revision policy \
           ON policy.policy_revision_ref=run.policy_revision_ref \
         WHERE item.run_ref=$1 AND item.derivation_ref=$2 AND item.state='running' \
         FOR UPDATE OF item",
    )
    .bind(claim.run_ref)
    .bind(claim.derivation_ref)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(CommentResearchAtomError::ClaimLost)?;
    let offsets = serde_json::from_value(row.get::<Value, _>("research_offsets"))
        .map_err(|_| CommentResearchAtomError::DerivationCorrupt)?;
    Ok(ClaimedInput {
        research_text: row.get("research_text"),
        offsets,
        extraction_rule_hash: row.get("extraction_rule_hash"),
    })
}

async fn accept_atoms(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    claim: &ResearchRunItemClaim,
    input: &ClaimedInput,
    atoms: &[SemanticAtomProposal],
    invocation_ref: Option<Uuid>,
) -> Result<usize, CommentResearchAtomError> {
    if atoms.is_empty() || atoms.len() > MAX_ATOMS_PER_ITEM {
        return Err(CommentResearchAtomError::InvalidOutput);
    }
    for (ordinal, atom) in atoms.iter().enumerate() {
        let (source_start, source_end) = validate_atom(input, atom)?;
        sqlx::query(
            "INSERT INTO linggan_comment_research_atom( \
                 atom_ref,run_ref,derivation_ref,ordinal,kind,proposition,basis,research_start, \
                 research_end,source_start,source_end,rule_hash,input_hash,invocation_ref \
             ) SELECT $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,item.input_hash,$13 \
               FROM linggan_comment_research_run_item item \
               WHERE item.run_ref=$2 AND item.derivation_ref=$3 AND item.state='running'",
        )
        .bind(Uuid::new_v4())
        .bind(claim.run_ref)
        .bind(claim.derivation_ref)
        .bind(i32::try_from(ordinal).map_err(|_| CommentResearchAtomError::InvalidOutput)?)
        .bind(atom.kind.as_db())
        .bind(atom.proposition.trim())
        .bind(atom.basis.as_db())
        .bind(
            i32::try_from(atom.evidence_start)
                .map_err(|_| CommentResearchAtomError::InvalidOutput)?,
        )
        .bind(
            i32::try_from(atom.evidence_end)
                .map_err(|_| CommentResearchAtomError::InvalidOutput)?,
        )
        .bind(i32::try_from(source_start).map_err(|_| CommentResearchAtomError::InvalidOutput)?)
        .bind(i32::try_from(source_end).map_err(|_| CommentResearchAtomError::InvalidOutput)?)
        .bind(&input.extraction_rule_hash)
        .bind(invocation_ref)
        .execute(&mut **transaction)
        .await?;
    }
    Ok(atoms.len())
}

fn validate_atom(
    input: &ClaimedInput,
    atom: &SemanticAtomProposal,
) -> Result<(usize, usize), CommentResearchAtomError> {
    if !valid_text(&atom.proposition, 1000)
        || atom.evidence_start >= atom.evidence_end
        || atom.evidence_end > input.research_text.chars().count()
    {
        return Err(CommentResearchAtomError::InvalidOutput);
    }
    if input.offsets.len() != input.research_text.chars().count() {
        return Err(CommentResearchAtomError::DerivationCorrupt);
    }
    let source_start = input
        .offsets
        .get(atom.evidence_start)
        .ok_or(CommentResearchAtomError::DerivationCorrupt)?
        .0;
    let source_end = input
        .offsets
        .get(atom.evidence_end - 1)
        .ok_or(CommentResearchAtomError::DerivationCorrupt)?
        .1;
    if source_end <= source_start {
        return Err(CommentResearchAtomError::DerivationCorrupt);
    }
    Ok((source_start, source_end))
}

async fn finish_item(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    claim: &ResearchRunItemClaim,
    state: &str,
) -> Result<(), CommentResearchAtomError> {
    let changed = sqlx::query(
        "UPDATE linggan_comment_research_run_item \
         SET state=$3,finished_at=scope_001_now(),lease_until=NULL,updated_at=scope_001_now() \
         WHERE run_ref=$1 AND derivation_ref=$2 AND state='running'",
    )
    .bind(claim.run_ref)
    .bind(claim.derivation_ref)
    .bind(state)
    .execute(&mut **transaction)
    .await?
    .rows_affected();
    if changed == 1 {
        Ok(())
    } else {
        Err(CommentResearchAtomError::ClaimLost)
    }
}

fn valid_text(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty() && value.chars().count() <= maximum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_validation_maps_unicode_scalars_to_original_source_offsets() {
        let input = ClaimedInput {
            research_text: "我😊不想催促".into(),
            offsets: vec![(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 6)],
            extraction_rule_hash: "unused".into(),
        };
        let atom = SemanticAtomProposal {
            kind: AtomKind::Problem,
            proposition: "家长不想以催促应对孩子".into(),
            basis: AtomBasis::Explicit,
            evidence_start: 1,
            evidence_end: 5,
        };
        assert_eq!(validate_atom(&input, &atom).unwrap(), (1, 5));
    }

    #[test]
    fn invalid_empty_or_out_of_range_evidence_is_rejected() {
        let input = ClaimedInput {
            research_text: "有效果吗".into(),
            offsets: vec![(0, 1), (1, 2), (2, 3), (3, 4)],
            extraction_rule_hash: "unused".into(),
        };
        let invalid = SemanticAtomProposal {
            kind: AtomKind::Need,
            proposition: " ".into(),
            basis: AtomBasis::Explicit,
            evidence_start: 4,
            evidence_end: 5,
        };
        assert!(matches!(
            validate_atom(&input, &invalid),
            Err(CommentResearchAtomError::InvalidOutput)
        ));
    }
}
