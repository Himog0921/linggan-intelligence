//! LOCAL-EMBEDDING-001 Atom document vectors and exact candidate recall.
//!
//! This module intentionally contains no second queue: pending/running/failed rows are the
//! existing Comment Research stage checkpoint. Only canonical Atom propositions are embedded;
//! a nearest Atom may nominate its already-admitted Problem, but similarity never mutates one.

use crate::{local_embedding_profile, research_text::content_hash};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

const CANDIDATE_RECALL_VERSION: &str = "comment-research.exact-cosine.pgvector.v1";
const MAX_CANDIDATES: i64 = 10;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingSpaceReceipt {
    pub space_ref: Uuid,
    pub profile_ref: Uuid,
    pub model_id: String,
    pub model_revision: String,
    pub encoding_mode: String,
    pub dimensions: usize,
    pub preprocessing_version: String,
    pub policy_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AtomEmbeddingInput {
    pub atom_ref: Uuid,
    pub space_ref: Uuid,
    pub input_hash: String,
    pub dimensions: usize,
    /// The existing semantic proposition is the V1 Atom canonical_text contract. Raw comments,
    /// prompts, Problem definitions and multimodal material never reach the local runtime.
    pub canonical_text: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AtomEmbeddingResult {
    pub atom_ref: Uuid,
    pub space_ref: Uuid,
    pub input_hash: String,
    pub values: Vec<f64>,
    pub invocation_ref: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProblemCandidate {
    pub problem_ref: Uuid,
    pub definition_revision: i32,
    pub neighbor_atom_ref: Uuid,
    pub cosine: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EmbeddingWorkClaim {
    pub input: AtomEmbeddingInput,
    pub run_ref: Uuid,
}

#[derive(Debug, thiserror::Error)]
pub enum CommentResearchEmbeddingError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("the fixed local embedding profile is unavailable")]
    EmbeddingUnavailable,
    #[error("the requested embedding space is unavailable")]
    SpaceUnavailable,
    #[error("the Atom is unavailable because its source is no longer readable")]
    AtomUnavailable,
    #[error("the Atom kind is not eligible for Problem candidate recall")]
    AtomNotProblemBearing,
    #[error("the embedding work is no longer pending")]
    WorkUnavailable,
    #[error("the embedding output violates the LOCAL-EMBEDDING-001 contract")]
    InvalidEmbedding,
}

/// Resolves one immutable space from the one enabled local profile. Profile fields belong in the
/// policy hash, so mixed model commits, modes, dimensions or preprocessing cannot share recall.
pub async fn activate_configured_embedding_space(
    database: &Database,
) -> Result<EmbeddingSpaceReceipt, CommentResearchEmbeddingError> {
    let profile = local_embedding_profile::active(database)
        .await
        .map_err(|_| CommentResearchEmbeddingError::EmbeddingUnavailable)?
        .filter(|profile| {
            profile.model_id == local_embedding_profile::MODEL_ID
                && profile.model_revision == local_embedding_profile::MODEL_REVISION
                && profile.encoding_mode == local_embedding_profile::ENCODING_MODE
                && profile.dimension == local_embedding_profile::DIMENSION
                && profile.preprocessing_version == local_embedding_profile::PREPROCESSING_VERSION
        })
        .ok_or(CommentResearchEmbeddingError::EmbeddingUnavailable)?;
    let policy_hash = content_hash(
        &json!({
            "version": CANDIDATE_RECALL_VERSION,
            "profileRef": profile.profile_ref,
            "modelId": profile.model_id,
            "modelRevision": profile.model_revision,
            "encodingMode": profile.encoding_mode,
            "dimension": profile.dimension,
            "preprocessingVersion": profile.preprocessing_version,
        })
        .to_string(),
    );
    sqlx::query(
        "INSERT INTO linggan_comment_research_embedding_space(\
             space_ref,profile_ref,dimensions,policy_hash\
         ) VALUES($1,$2,$3,$4) ON CONFLICT(policy_hash) DO NOTHING",
    )
    .bind(Uuid::new_v4())
    .bind(profile.profile_ref)
    .bind(i32::try_from(profile.dimension).map_err(|_| CommentResearchEmbeddingError::InvalidEmbedding)?)
    .bind(&policy_hash)
    .execute(database.pool())
    .await?;
    let row = sqlx::query(
        "SELECT space_ref,profile_ref,dimensions,policy_hash \
         FROM linggan_comment_research_embedding_space WHERE policy_hash=$1",
    )
    .bind(&policy_hash)
    .fetch_one(database.pool())
    .await?;
    Ok(EmbeddingSpaceReceipt {
        space_ref: row.get("space_ref"),
        profile_ref: row.get("profile_ref"),
        model_id: profile.model_id,
        model_revision: profile.model_revision,
        encoding_mode: profile.encoding_mode,
        dimensions: positive_dimensions(row.get("dimensions"))?,
        preprocessing_version: profile.preprocessing_version,
        policy_hash: row.get("policy_hash"),
    })
}

pub async fn queue_atom_embedding(
    database: &Database,
    atom_ref: Uuid,
    space_ref: Uuid,
) -> Result<AtomEmbeddingInput, CommentResearchEmbeddingError> {
    let input = read_atom_embedding_input(database, atom_ref, space_ref).await?;
    sqlx::query(
        "INSERT INTO linggan_comment_research_atom_embedding(\
             atom_ref,space_ref,input_hash,state\
         ) VALUES($1,$2,$3,'pending') ON CONFLICT DO NOTHING",
    )
    .bind(input.atom_ref)
    .bind(input.space_ref)
    .bind(&input.input_hash)
    .execute(database.pool())
    .await?;
    Ok(input)
}

pub async fn accept_atom_embedding(
    database: &Database,
    result: AtomEmbeddingResult,
) -> Result<(), CommentResearchEmbeddingError> {
    let expected = read_atom_embedding_input(database, result.atom_ref, result.space_ref).await?;
    if expected.input_hash != result.input_hash {
        return Err(CommentResearchEmbeddingError::InvalidEmbedding);
    }
    let vector = normalized_vector_literal(&result.values, expected.dimensions)?;
    let changed = sqlx::query(
        "UPDATE linggan_comment_research_atom_embedding \
         SET state='succeeded',dimensions=$4,vector=$5::vector,invocation_ref=$6,failure_code=NULL, \
             updated_at=scope_001_now() \
         WHERE atom_ref=$1 AND space_ref=$2 AND input_hash=$3 AND state IN ('pending','running')",
    )
    .bind(result.atom_ref)
    .bind(result.space_ref)
    .bind(&result.input_hash)
    .bind(i32::try_from(expected.dimensions).map_err(|_| CommentResearchEmbeddingError::InvalidEmbedding)?)
    .bind(vector)
    .bind(result.invocation_ref)
    .execute(database.pool())
    .await?
    .rows_affected();
    (changed == 1)
        .then_some(())
        .ok_or(CommentResearchEmbeddingError::WorkUnavailable)
}

/// Exact pgvector cosine distance, scoped to one profile-derived space. Candidate vectors are
/// canonical Atoms already admitted to active Problems; no Problem-definition embedding exists.
pub async fn recall_problem_candidates(
    database: &Database,
    atom_ref: Uuid,
    space_ref: Uuid,
) -> Result<Vec<ProblemCandidate>, CommentResearchEmbeddingError> {
    let rows = sqlx::query(
        "SELECT candidate.problem_ref,candidate.definition_revision,candidate.neighbor_atom_ref,candidate.cosine \
         FROM ( \
           SELECT DISTINCT ON (membership.problem_ref,membership.definition_revision) \
                  membership.problem_ref,membership.definition_revision,neighbor.atom_ref AS neighbor_atom_ref, \
                  1-(neighbor_embedding.vector <=> query_embedding.vector) AS cosine \
           FROM linggan_comment_research_atom_embedding query_embedding \
           JOIN linggan_comment_research_atom query_atom ON query_atom.atom_ref=query_embedding.atom_ref \
           JOIN linggan_comment_research_derivation_readable query_derivation \
             ON query_derivation.derivation_ref=query_atom.derivation_ref \
           JOIN linggan_comment_research_atom_embedding neighbor_embedding \
             ON neighbor_embedding.space_ref=query_embedding.space_ref AND neighbor_embedding.state='succeeded' \
           JOIN linggan_comment_research_atom neighbor ON neighbor.atom_ref=neighbor_embedding.atom_ref \
           JOIN linggan_comment_research_derivation_readable neighbor_derivation \
             ON neighbor_derivation.derivation_ref=neighbor.derivation_ref \
           JOIN linggan_comment_research_atom_problem_membership membership \
             ON membership.atom_ref=neighbor.atom_ref AND membership.current \
           JOIN linggan_comment_research_problem problem ON problem.problem_ref=membership.problem_ref \
           WHERE query_embedding.atom_ref=$1 AND query_embedding.space_ref=$2 \
             AND query_embedding.state='succeeded' AND query_atom.kind IN ('problem','need') \
             AND neighbor.atom_ref<>query_atom.atom_ref AND neighbor.kind IN ('problem','need') \
             AND problem.state='active' \
             AND NOT EXISTS(SELECT 1 FROM linggan_comment_research_atom_problem_membership own \
                            WHERE own.atom_ref=query_atom.atom_ref AND own.problem_ref=membership.problem_ref \
                              AND own.current) \
           ORDER BY membership.problem_ref,membership.definition_revision,\
                    neighbor_embedding.vector <=> query_embedding.vector,neighbor.atom_ref \
         ) candidate \
         ORDER BY candidate.cosine DESC,candidate.problem_ref,candidate.neighbor_atom_ref LIMIT $3",
    )
    .bind(atom_ref)
    .bind(space_ref)
    .bind(MAX_CANDIDATES)
    .fetch_all(database.pool())
    .await?;
    if rows.is_empty() {
        let present: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM linggan_comment_research_atom_embedding embedding \
             JOIN linggan_comment_research_atom atom USING(atom_ref) \
             JOIN linggan_comment_research_derivation_readable derivation ON derivation.derivation_ref=atom.derivation_ref \
             WHERE embedding.atom_ref=$1 AND embedding.space_ref=$2 AND embedding.state='succeeded' \
               AND atom.kind IN ('problem','need'))",
        )
        .bind(atom_ref)
        .bind(space_ref)
        .fetch_one(database.pool())
        .await?;
        if !present {
            return Err(CommentResearchEmbeddingError::AtomUnavailable);
        }
    }
    Ok(rows
        .into_iter()
        .map(|row| ProblemCandidate {
            problem_ref: row.get("problem_ref"),
            definition_revision: row.get("definition_revision"),
            neighbor_atom_ref: row.get("neighbor_atom_ref"),
            cosine: row.get::<f64, _>("cosine").clamp(-1.0, 1.0),
        })
        .collect())
}

pub async fn claim_next_embedding_work(
    database: &Database,
) -> Result<Option<EmbeddingWorkClaim>, CommentResearchEmbeddingError> {
    let space = activate_configured_embedding_space(database).await?;
    let existing: Option<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT embedding.atom_ref,atom.run_ref \
         FROM linggan_comment_research_atom_embedding embedding \
         JOIN linggan_comment_research_atom atom USING(atom_ref) \
         JOIN linggan_comment_research_run run ON run.run_ref=atom.run_ref \
         WHERE embedding.space_ref=$1 AND embedding.state='pending' AND run.state IN ('queued','running') \
         ORDER BY embedding.created_at,embedding.atom_ref LIMIT 1",
    )
    .bind(space.space_ref)
    .fetch_optional(database.pool())
    .await?;
    let (atom_ref, run_ref) = if let Some(existing) = existing {
        existing
    } else {
        let candidate: Option<(Uuid, Uuid)> = sqlx::query_as(
            "SELECT atom.atom_ref,atom.run_ref \
             FROM linggan_comment_research_atom atom \
             JOIN linggan_comment_research_derivation_readable derivation ON derivation.derivation_ref=atom.derivation_ref \
             JOIN linggan_comment_research_run run ON run.run_ref=atom.run_ref \
             LEFT JOIN linggan_comment_research_atom_problem_membership membership \
               ON membership.atom_ref=atom.atom_ref AND membership.current \
             WHERE atom.kind IN ('problem','need') AND membership.atom_ref IS NULL \
               AND run.state IN ('queued','running') \
               AND NOT EXISTS(SELECT 1 FROM linggan_comment_research_atom_embedding prior \
                              WHERE prior.atom_ref=atom.atom_ref AND prior.space_ref=$1) \
             ORDER BY atom.created_at,atom.atom_ref LIMIT 1",
        )
        .bind(space.space_ref)
        .fetch_optional(database.pool())
        .await?;
        let Some(candidate) = candidate else { return Ok(None); };
        queue_atom_embedding(database, candidate.0, space.space_ref).await?;
        candidate
    };
    let input = read_atom_embedding_input(database, atom_ref, space.space_ref).await?;
    let claimed = sqlx::query(
        "UPDATE linggan_comment_research_atom_embedding SET state='running',updated_at=scope_001_now() \
         WHERE atom_ref=$1 AND space_ref=$2 AND input_hash=$3 AND state='pending'",
    )
    .bind(input.atom_ref)
    .bind(input.space_ref)
    .bind(&input.input_hash)
    .execute(database.pool())
    .await?
    .rows_affected();
    Ok((claimed == 1).then_some(EmbeddingWorkClaim { input, run_ref }))
}

pub async fn record_embedding_failure(
    database: &Database,
    claim: &EmbeddingWorkClaim,
    invocation_ref: Option<Uuid>,
    failure_code: &str,
) -> Result<(), CommentResearchEmbeddingError> {
    let changed = sqlx::query(
        "UPDATE linggan_comment_research_atom_embedding \
         SET state='failed',invocation_ref=$4,failure_code=$5,updated_at=scope_001_now() \
         WHERE atom_ref=$1 AND space_ref=$2 AND input_hash=$3 AND state='running'",
    )
    .bind(claim.input.atom_ref)
    .bind(claim.input.space_ref)
    .bind(&claim.input.input_hash)
    .bind(invocation_ref)
    .bind(failure_code)
    .execute(database.pool())
    .await?
    .rows_affected();
    (changed == 1)
        .then_some(())
        .ok_or(CommentResearchEmbeddingError::WorkUnavailable)
}

async fn read_atom_embedding_input(
    database: &Database,
    atom_ref: Uuid,
    space_ref: Uuid,
) -> Result<AtomEmbeddingInput, CommentResearchEmbeddingError> {
    let row = sqlx::query(
        "SELECT atom.kind,atom.proposition,atom.rule_hash \
         FROM linggan_comment_research_atom atom \
         JOIN linggan_comment_research_derivation_readable derivation ON derivation.derivation_ref=atom.derivation_ref \
         WHERE atom.atom_ref=$1",
    )
    .bind(atom_ref)
    .fetch_optional(database.pool())
    .await?
    .ok_or(CommentResearchEmbeddingError::AtomUnavailable)?;
    let kind: String = row.get("kind");
    if !matches!(kind.as_str(), "problem" | "need") {
        return Err(CommentResearchEmbeddingError::AtomNotProblemBearing);
    }
    let canonical_text: String = row.get("proposition");
    let dimensions = space_dimensions(database, space_ref).await?;
    Ok(AtomEmbeddingInput {
        atom_ref,
        space_ref,
        input_hash: content_hash(&json!({
            "kind": "atom_canonical_text",
            "atomKind": kind,
            "canonicalText": canonical_text,
            "ruleHash": row.get::<String, _>("rule_hash"),
            "preprocessingVersion": local_embedding_profile::PREPROCESSING_VERSION,
        }).to_string()),
        dimensions,
        canonical_text,
    })
}

async fn space_dimensions(database: &Database, space_ref: Uuid) -> Result<usize, CommentResearchEmbeddingError> {
    let dimensions: Option<i32> = sqlx::query_scalar(
        "SELECT space.dimensions FROM linggan_comment_research_embedding_space space \
         JOIN linggan_comment_research_embedding_profile profile USING(profile_ref) \
         WHERE space.space_ref=$1 AND profile.enabled \
           AND profile.model_id='Tencent/WeMM-Embedding-2B' \
           AND profile.model_revision='bbd6cd4bf52cfc6716f752a2df80b2706720bd95' \
           AND profile.encoding_mode='document' AND profile.dimension=512 \
           AND profile.preprocessing_version='comment-research.atom-canonical-text.v1'",
    )
    .bind(space_ref)
    .fetch_optional(database.pool())
    .await?;
    dimensions.ok_or(CommentResearchEmbeddingError::SpaceUnavailable).and_then(positive_dimensions)
}

fn positive_dimensions(dimensions: i32) -> Result<usize, CommentResearchEmbeddingError> {
    let dimensions = usize::try_from(dimensions).map_err(|_| CommentResearchEmbeddingError::InvalidEmbedding)?;
    (dimensions == local_embedding_profile::DIMENSION)
        .then_some(dimensions)
        .ok_or(CommentResearchEmbeddingError::InvalidEmbedding)
}

fn normalized_vector_literal(values: &[f64], dimensions: usize) -> Result<String, CommentResearchEmbeddingError> {
    if values.len() != dimensions || values.iter().any(|value| !value.is_finite()) {
        return Err(CommentResearchEmbeddingError::InvalidEmbedding);
    }
    let norm_squared: f64 = values.iter().map(|value| value * value).sum();
    if !norm_squared.is_finite() || norm_squared <= 0.0 {
        return Err(CommentResearchEmbeddingError::InvalidEmbedding);
    }
    let norm = norm_squared.sqrt();
    serde_json::to_string(&values.iter().map(|value| value / norm).collect::<Vec<_>>())
        .map_err(|_| CommentResearchEmbeddingError::InvalidEmbedding)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalized_vector_is_pgvector_literal_with_unit_norm() {
        assert_eq!(normalized_vector_literal(&[3.0, 4.0], 2).unwrap(), "[0.6,0.8]");
        assert!(normalized_vector_literal(&[0.0, 0.0], 2).is_err());
        assert!(normalized_vector_literal(&[1.0], 2).is_err());
    }
}
