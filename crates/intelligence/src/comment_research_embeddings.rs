//! Versioned embedding inputs and exact candidate recall for COMMENT-RESEARCH-RESET-001.
//!
//! This module treats embeddings as a rebuildable derived capability. It only queues and accepts
//! vectors for readable V1 atoms and Problem definitions, then returns at most ten candidates.
//! Cosine similarity never writes a Problem membership and never authorizes a model request.

use crate::research_text::content_hash;
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

const CANDIDATE_RECALL_VERSION: &str = "comment-research.exact-cosine.v1";
const MAX_VECTOR_DIMENSIONS: usize = 8192;
const MAX_CANDIDATES: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingSpaceReceipt {
    pub space_ref: Uuid,
    pub config_ref: Uuid,
    pub model_ref: Uuid,
    pub dimensions: usize,
    pub policy_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AtomEmbeddingInput {
    pub atom_ref: Uuid,
    pub space_ref: Uuid,
    pub input_hash: String,
    pub dimensions: usize,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProblemDefinitionEmbeddingInput {
    pub problem_ref: Uuid,
    pub definition_revision: i32,
    pub space_ref: Uuid,
    pub input_hash: String,
    pub dimensions: usize,
    pub text: String,
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProblemDefinitionEmbeddingResult {
    pub problem_ref: Uuid,
    pub definition_revision: i32,
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
    pub cosine: f64,
}

/// A single provider-facing embedding claim.  The worker receives the already frozen text and
/// identity, never a query it has to rebuild from a separate research implementation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EmbeddingWorkClaim {
    Atom {
        input: AtomEmbeddingInput,
        run_ref: Uuid,
    },
    ProblemDefinition {
        input: ProblemDefinitionEmbeddingInput,
        run_ref: Uuid,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum CommentResearchEmbeddingError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("no enabled, qualified embedding configuration is available")]
    EmbeddingUnavailable,
    #[error("the requested embedding space is unavailable")]
    SpaceUnavailable,
    #[error("the Atom is unavailable because its source is no longer readable")]
    AtomUnavailable,
    #[error("the Atom kind is not eligible for Problem candidate recall")]
    AtomNotProblemBearing,
    #[error("the requested Problem definition is unavailable")]
    DefinitionUnavailable,
    #[error("the embedding work is no longer pending")]
    WorkUnavailable,
    #[error("the embedding output violates the V1 contract")]
    InvalidEmbedding,
}

/// Creates or resolves the immutable embedding space for the saved, enabled, qualified embedding
/// configuration. Calling it never invokes the provider; the actual embedding worker comes later.
pub async fn activate_configured_embedding_space(
    database: &Database,
) -> Result<EmbeddingSpaceReceipt, CommentResearchEmbeddingError> {
    let row = sqlx::query(
        "SELECT config.config_ref,config.model_ref,config.dimensions \
         FROM linggan_embedding_settings settings \
         JOIN linggan_embedding_config config USING(config_ref) \
         JOIN linggan_model_entry model USING(model_ref) \
         JOIN linggan_model_connection_version version \
           ON version.version_ref=model.connection_version_ref \
         JOIN linggan_model_connection connection USING(connection_ref) \
         WHERE settings.singleton AND config.enabled AND config.qualified AND connection.enabled",
    )
    .fetch_optional(database.pool())
    .await?
    .ok_or(CommentResearchEmbeddingError::EmbeddingUnavailable)?;
    let config_ref: Uuid = row.get("config_ref");
    let model_ref: Uuid = row.get("model_ref");
    let dimensions = positive_dimensions(row.get::<i32, _>("dimensions"))?;
    let policy_hash = content_hash(
        &json!({
            "version":CANDIDATE_RECALL_VERSION,
            "configRef":config_ref,
            "modelRef":model_ref,
            "dimensions":dimensions,
        })
        .to_string(),
    );
    let space_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_comment_research_embedding_space( \
             space_ref,config_ref,model_ref,dimensions,policy_hash \
         ) VALUES($1,$2,$3,$4,$5) ON CONFLICT(policy_hash) DO NOTHING",
    )
    .bind(space_ref)
    .bind(config_ref)
    .bind(model_ref)
    .bind(i32::try_from(dimensions).map_err(|_| CommentResearchEmbeddingError::InvalidEmbedding)?)
    .bind(&policy_hash)
    .execute(database.pool())
    .await?;
    let existing = sqlx::query(
        "SELECT space_ref,config_ref,model_ref,dimensions,policy_hash \
         FROM linggan_comment_research_embedding_space WHERE policy_hash=$1",
    )
    .bind(&policy_hash)
    .fetch_one(database.pool())
    .await?;
    Ok(EmbeddingSpaceReceipt {
        space_ref: existing.get("space_ref"),
        config_ref: existing.get("config_ref"),
        model_ref: existing.get("model_ref"),
        dimensions: positive_dimensions(existing.get::<i32, _>("dimensions"))?,
        policy_hash: existing.get("policy_hash"),
    })
}

/// Queues one readable, problem-bearing Atom for a single embedding space. The returned text is
/// the exact semantic proposition to embed, not the raw comment or ambient page context.
pub async fn queue_atom_embedding(
    database: &Database,
    atom_ref: Uuid,
    space_ref: Uuid,
) -> Result<AtomEmbeddingInput, CommentResearchEmbeddingError> {
    let input = read_atom_embedding_input(database, atom_ref, space_ref).await?;
    sqlx::query(
        "INSERT INTO linggan_comment_research_atom_embedding( \
             atom_ref,space_ref,input_hash,state \
         ) VALUES($1,$2,$3,'pending') ON CONFLICT DO NOTHING",
    )
    .bind(input.atom_ref)
    .bind(input.space_ref)
    .bind(&input.input_hash)
    .execute(database.pool())
    .await?;
    Ok(input)
}

/// Queues the current definition revision only while at least one current member still has a
/// readable source. A withdrawn corpus cannot remain a candidate merely because its vector cache
/// survived.
pub async fn queue_problem_definition_embedding(
    database: &Database,
    problem_ref: Uuid,
    definition_revision: i32,
    space_ref: Uuid,
) -> Result<ProblemDefinitionEmbeddingInput, CommentResearchEmbeddingError> {
    let input =
        read_definition_embedding_input(database, problem_ref, definition_revision, space_ref)
            .await?;
    sqlx::query(
        "INSERT INTO linggan_comment_research_problem_definition_embedding( \
             problem_ref,definition_revision,space_ref,input_hash,state \
         ) VALUES($1,$2,$3,$4,'pending') ON CONFLICT DO NOTHING",
    )
    .bind(input.problem_ref)
    .bind(input.definition_revision)
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
    let vector = validate_vector(&result.values, expected.dimensions)?;
    let changed = sqlx::query(
        "UPDATE linggan_comment_research_atom_embedding \
         SET state='succeeded',dimensions=$4,vector=$5,invocation_ref=$6,failure_code=NULL, \
             updated_at=scope_001_now() \
         WHERE atom_ref=$1 AND space_ref=$2 AND input_hash=$3 AND state IN ('pending','running')",
    )
    .bind(result.atom_ref)
    .bind(result.space_ref)
    .bind(&result.input_hash)
    .bind(
        i32::try_from(expected.dimensions)
            .map_err(|_| CommentResearchEmbeddingError::InvalidEmbedding)?,
    )
    .bind(vector)
    .bind(result.invocation_ref)
    .execute(database.pool())
    .await?
    .rows_affected();
    if changed == 1 {
        Ok(())
    } else {
        Err(CommentResearchEmbeddingError::WorkUnavailable)
    }
}

pub async fn accept_problem_definition_embedding(
    database: &Database,
    result: ProblemDefinitionEmbeddingResult,
) -> Result<(), CommentResearchEmbeddingError> {
    let expected = read_definition_embedding_input(
        database,
        result.problem_ref,
        result.definition_revision,
        result.space_ref,
    )
    .await?;
    if expected.input_hash != result.input_hash {
        return Err(CommentResearchEmbeddingError::InvalidEmbedding);
    }
    let vector = validate_vector(&result.values, expected.dimensions)?;
    let changed = sqlx::query(
        "UPDATE linggan_comment_research_problem_definition_embedding \
         SET state='succeeded',dimensions=$5,vector=$6,invocation_ref=$7,failure_code=NULL, \
             updated_at=scope_001_now() \
         WHERE problem_ref=$1 AND definition_revision=$2 AND space_ref=$3 AND input_hash=$4 \
           AND state IN ('pending','running')",
    )
    .bind(result.problem_ref)
    .bind(result.definition_revision)
    .bind(result.space_ref)
    .bind(&result.input_hash)
    .bind(
        i32::try_from(expected.dimensions)
            .map_err(|_| CommentResearchEmbeddingError::InvalidEmbedding)?,
    )
    .bind(vector)
    .bind(result.invocation_ref)
    .execute(database.pool())
    .await?
    .rows_affected();
    if changed == 1 {
        Ok(())
    } else {
        Err(CommentResearchEmbeddingError::WorkUnavailable)
    }
}

/// Returns exact cosine candidates in a single configured space. The result is bounded and has no
/// authority to mutate membership; callers must make a separate deterministic/manual/model decision.
pub async fn recall_problem_candidates(
    database: &Database,
    atom_ref: Uuid,
    space_ref: Uuid,
) -> Result<Vec<ProblemCandidate>, CommentResearchEmbeddingError> {
    let query = sqlx::query(
        "SELECT embedding.dimensions,embedding.vector \
         FROM linggan_comment_research_atom_embedding embedding \
         JOIN linggan_comment_research_atom atom USING(atom_ref) \
         JOIN linggan_comment_research_derivation_readable derivation \
           ON derivation.derivation_ref=atom.derivation_ref \
         WHERE embedding.atom_ref=$1 AND embedding.space_ref=$2 AND embedding.state='succeeded' \
           AND atom.kind IN ('problem','need')",
    )
    .bind(atom_ref)
    .bind(space_ref)
    .fetch_optional(database.pool())
    .await?
    .ok_or(CommentResearchEmbeddingError::AtomUnavailable)?;
    let dimensions = positive_dimensions(query.get::<i32, _>("dimensions"))?;
    let query_values = value_to_vector(query.get("vector"), dimensions)?;
    let rows = sqlx::query(
        "SELECT embedding.problem_ref,embedding.definition_revision,embedding.dimensions,embedding.vector \
         FROM linggan_comment_research_problem_definition_embedding embedding \
         JOIN linggan_comment_research_problem problem USING(problem_ref) \
         WHERE embedding.space_ref=$1 AND embedding.state='succeeded' AND problem.state='active' \
           AND EXISTS( \
               SELECT 1 \
               FROM linggan_comment_research_atom_problem_membership membership \
               JOIN linggan_comment_research_atom member_atom USING(atom_ref) \
               JOIN linggan_comment_research_derivation_readable member_derivation \
                 ON member_derivation.derivation_ref=member_atom.derivation_ref \
               WHERE membership.problem_ref=embedding.problem_ref \
                 AND membership.definition_revision=embedding.definition_revision \
                 AND membership.current \
           ) \
           AND NOT EXISTS( \
               SELECT 1 FROM linggan_comment_research_atom_problem_membership own_membership \
               WHERE own_membership.atom_ref=$2 AND own_membership.problem_ref=embedding.problem_ref \
                 AND own_membership.current \
           )",
    )
    .bind(space_ref)
    .bind(atom_ref)
    .fetch_all(database.pool())
    .await?;
    let candidates = rows
        .into_iter()
        .filter_map(|row| {
            let candidate_dimensions = positive_dimensions(row.get::<i32, _>("dimensions")).ok()?;
            let values = value_to_vector(row.get("vector"), candidate_dimensions).ok()?;
            Some(ProblemCandidateVector {
                problem_ref: row.get("problem_ref"),
                definition_revision: row.get("definition_revision"),
                dimensions: candidate_dimensions,
                values,
            })
        })
        .collect::<Vec<_>>();
    Ok(exact_cosine_candidates(
        &query_values,
        dimensions,
        &candidates,
    ))
}

/// Advances exactly one V1 embedding queue item.  Definition vectors are prioritised so the
/// first newly admitted Problem becomes recallable before another similar Atom is resolved.
/// Failed work is terminal for this immutable input and is deliberately not silently retried as
/// a different vector.
pub async fn claim_next_embedding_work(
    database: &Database,
) -> Result<Option<EmbeddingWorkClaim>, CommentResearchEmbeddingError> {
    let space = activate_configured_embedding_space(database).await?;
    if let Some((input, run_ref)) =
        claim_pending_definition_embedding(database, space.space_ref).await?
    {
        return Ok(Some(EmbeddingWorkClaim::ProblemDefinition {
            input,
            run_ref,
        }));
    }
    if let Some((input, run_ref)) = claim_pending_atom_embedding(database, space.space_ref).await? {
        return Ok(Some(EmbeddingWorkClaim::Atom { input, run_ref }));
    }
    Ok(None)
}

/// A provider failure belongs to this immutable input, not to every pending embedding in the
/// system.  Leaving it explicit lets the run screen distinguish an incomplete result from an
/// empty one and prevents a poisoned input from head-of-line blocking a healthy later item.
pub async fn record_embedding_failure(
    database: &Database,
    claim: &EmbeddingWorkClaim,
    invocation_ref: Option<Uuid>,
    failure_code: &str,
) -> Result<(), CommentResearchEmbeddingError> {
    let (is_atom, first, second, space_ref, input_hash) = match claim {
        EmbeddingWorkClaim::Atom { input, .. } => (
            true,
            input.atom_ref,
            None,
            input.space_ref,
            input.input_hash.as_str(),
        ),
        EmbeddingWorkClaim::ProblemDefinition { input, .. } => (
            false,
            input.problem_ref,
            Some(input.definition_revision),
            input.space_ref,
            input.input_hash.as_str(),
        ),
    };
    let updated = if is_atom {
        sqlx::query(
            "UPDATE linggan_comment_research_atom_embedding \
             SET state='failed',invocation_ref=$4,failure_code=$5,updated_at=scope_001_now() \
             WHERE atom_ref=$1 AND space_ref=$2 AND input_hash=$3 AND state='running'",
        )
        .bind(first)
        .bind(space_ref)
        .bind(input_hash)
        .bind(invocation_ref)
        .bind(failure_code)
        .execute(database.pool())
        .await?
        .rows_affected()
    } else {
        sqlx::query(
            "UPDATE linggan_comment_research_problem_definition_embedding \
             SET state='failed',invocation_ref=$5,failure_code=$6,updated_at=scope_001_now() \
             WHERE problem_ref=$1 AND definition_revision=$2 AND space_ref=$3 AND input_hash=$4 \
               AND state='running'",
        )
        .bind(first)
        .bind(second)
        .bind(space_ref)
        .bind(input_hash)
        .bind(invocation_ref)
        .bind(failure_code)
        .execute(database.pool())
        .await?
        .rows_affected()
    };
    if updated == 1 {
        Ok(())
    } else {
        Err(CommentResearchEmbeddingError::WorkUnavailable)
    }
}

async fn claim_pending_definition_embedding(
    database: &Database,
    space_ref: Uuid,
) -> Result<Option<(ProblemDefinitionEmbeddingInput, Uuid)>, CommentResearchEmbeddingError> {
    let Some(run_ref) = candidate_recall_run(database).await? else {
        return Ok(None);
    };
    let existing: Option<(Uuid, i32)> = sqlx::query_as(
        "SELECT embedding.problem_ref,embedding.definition_revision \
         FROM linggan_comment_research_problem_definition_embedding embedding \
         WHERE embedding.space_ref=$1 AND embedding.state='pending' \
         ORDER BY embedding.created_at,embedding.problem_ref LIMIT 1",
    )
    .bind(space_ref)
    .fetch_optional(database.pool())
    .await?;
    let target = if let Some(target) = existing {
        target
    } else {
        let candidate: Option<(Uuid, i32)> = sqlx::query_as(
            "SELECT definition.problem_ref,definition.revision \
             FROM linggan_comment_research_problem_definition definition \
             JOIN linggan_comment_research_problem problem USING(problem_ref) \
             WHERE problem.state='active' \
               AND EXISTS( \
                 SELECT 1 FROM linggan_comment_research_atom_problem_membership membership \
                 JOIN linggan_comment_research_atom atom USING(atom_ref) \
                 JOIN linggan_comment_research_derivation_readable derivation \
                   ON derivation.derivation_ref=atom.derivation_ref \
                 WHERE membership.problem_ref=definition.problem_ref \
                   AND membership.definition_revision=definition.revision AND membership.current \
               ) \
               AND NOT EXISTS( \
                 SELECT 1 FROM linggan_comment_research_problem_definition_embedding prior \
                 WHERE prior.problem_ref=definition.problem_ref \
                   AND prior.definition_revision=definition.revision AND prior.space_ref=$1 \
               ) \
             ORDER BY definition.created_at,definition.problem_ref LIMIT 1",
        )
        .bind(space_ref)
        .fetch_optional(database.pool())
        .await?;
        let Some(target) = candidate else {
            return Ok(None);
        };
        queue_problem_definition_embedding(database, target.0, target.1, space_ref).await?;
        target
    };
    let input = read_definition_embedding_input(database, target.0, target.1, space_ref).await?;
    let claimed = sqlx::query(
        "UPDATE linggan_comment_research_problem_definition_embedding \
         SET state='running',updated_at=scope_001_now() \
         WHERE problem_ref=$1 AND definition_revision=$2 AND space_ref=$3 AND input_hash=$4 \
           AND state='pending'",
    )
    .bind(input.problem_ref)
    .bind(input.definition_revision)
    .bind(input.space_ref)
    .bind(&input.input_hash)
    .execute(database.pool())
    .await?
    .rows_affected();
    Ok((claimed == 1).then_some((input, run_ref)))
}

/// Definition vectors are only built while a current Run has an Atom ready for candidate recall.
/// This prevents a global cache warmer from sending unbounded provider calls after a Run ended.
async fn candidate_recall_run(
    database: &Database,
) -> Result<Option<Uuid>, CommentResearchEmbeddingError> {
    sqlx::query_scalar(
        "SELECT atom.run_ref \
         FROM linggan_comment_research_atom atom \
         JOIN linggan_comment_research_run run ON run.run_ref=atom.run_ref \
         LEFT JOIN linggan_comment_research_atom_problem_membership membership \
           ON membership.atom_ref=atom.atom_ref AND membership.current \
         WHERE atom.kind IN ('problem','need') AND membership.atom_ref IS NULL \
           AND run.state IN ('queued','running') \
           AND EXISTS(SELECT 1 FROM linggan_comment_research_atom_embedding embedding \
                      WHERE embedding.atom_ref=atom.atom_ref AND embedding.state='succeeded') \
         ORDER BY run.created_at,atom.created_at,atom.atom_ref LIMIT 1",
    )
    .fetch_optional(database.pool())
    .await
    .map_err(CommentResearchEmbeddingError::Database)
}

async fn claim_pending_atom_embedding(
    database: &Database,
    space_ref: Uuid,
) -> Result<Option<(AtomEmbeddingInput, Uuid)>, CommentResearchEmbeddingError> {
    let existing: Option<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT embedding.atom_ref,atom.run_ref \
         FROM linggan_comment_research_atom_embedding embedding \
         JOIN linggan_comment_research_atom atom USING(atom_ref) \
         JOIN linggan_comment_research_run run ON run.run_ref=atom.run_ref \
         WHERE embedding.space_ref=$1 AND embedding.state='pending' \
           AND run.state IN ('queued','running') \
         ORDER BY embedding.created_at,embedding.atom_ref LIMIT 1",
    )
    .bind(space_ref)
    .fetch_optional(database.pool())
    .await?;
    let (atom_ref, run_ref) = if let Some(existing) = existing {
        existing
    } else {
        let candidate: Option<(Uuid, Uuid)> = sqlx::query_as(
            "SELECT atom.atom_ref,atom.run_ref \
             FROM linggan_comment_research_atom atom \
             JOIN linggan_comment_research_derivation_readable derivation \
               ON derivation.derivation_ref=atom.derivation_ref \
             LEFT JOIN linggan_comment_research_atom_problem_membership membership \
               ON membership.atom_ref=atom.atom_ref AND membership.current \
             JOIN linggan_comment_research_run run ON run.run_ref=atom.run_ref \
             WHERE atom.kind IN ('problem','need') AND membership.atom_ref IS NULL \
               AND run.state IN ('queued','running') \
               AND NOT EXISTS( \
                 SELECT 1 FROM linggan_comment_research_atom_embedding prior \
                 WHERE prior.atom_ref=atom.atom_ref AND prior.space_ref=$1 \
               ) \
             ORDER BY atom.created_at,atom.atom_ref LIMIT 1",
        )
        .bind(space_ref)
        .fetch_optional(database.pool())
        .await?;
        let Some(candidate) = candidate else {
            return Ok(None);
        };
        queue_atom_embedding(database, candidate.0, space_ref).await?;
        candidate
    };
    let input = read_atom_embedding_input(database, atom_ref, space_ref).await?;
    let claimed = sqlx::query(
        "UPDATE linggan_comment_research_atom_embedding \
         SET state='running',updated_at=scope_001_now() \
         WHERE atom_ref=$1 AND space_ref=$2 AND input_hash=$3 AND state='pending'",
    )
    .bind(input.atom_ref)
    .bind(input.space_ref)
    .bind(&input.input_hash)
    .execute(database.pool())
    .await?
    .rows_affected();
    Ok((claimed == 1).then_some((input, run_ref)))
}

struct ProblemCandidateVector {
    problem_ref: Uuid,
    definition_revision: i32,
    dimensions: usize,
    values: Vec<f64>,
}

async fn read_atom_embedding_input(
    database: &Database,
    atom_ref: Uuid,
    space_ref: Uuid,
) -> Result<AtomEmbeddingInput, CommentResearchEmbeddingError> {
    let row = sqlx::query(
        "SELECT atom.kind,atom.proposition,atom.rule_hash \
         FROM linggan_comment_research_atom atom \
         JOIN linggan_comment_research_derivation_readable derivation \
           ON derivation.derivation_ref=atom.derivation_ref \
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
    let text: String = row.get("proposition");
    let rule_hash: String = row.get("rule_hash");
    let dimensions = space_dimensions(database, space_ref).await?;
    Ok(AtomEmbeddingInput {
        atom_ref,
        space_ref,
        input_hash: content_hash(
            &json!({"kind":"atom","atomKind":kind,"proposition":text,"ruleHash":rule_hash})
                .to_string(),
        ),
        dimensions,
        text,
    })
}

async fn read_definition_embedding_input(
    database: &Database,
    problem_ref: Uuid,
    definition_revision: i32,
    space_ref: Uuid,
) -> Result<ProblemDefinitionEmbeddingInput, CommentResearchEmbeddingError> {
    if definition_revision <= 0 {
        return Err(CommentResearchEmbeddingError::DefinitionUnavailable);
    }
    let row = sqlx::query(
        "SELECT definition.name,definition.meaning,definition.definition_hash \
         FROM linggan_comment_research_problem problem \
         JOIN linggan_comment_research_problem_definition definition USING(problem_ref) \
         WHERE problem.problem_ref=$1 AND definition.revision=$2 AND problem.state='active' \
           AND EXISTS( \
               SELECT 1 \
               FROM linggan_comment_research_atom_problem_membership membership \
               JOIN linggan_comment_research_atom atom USING(atom_ref) \
               JOIN linggan_comment_research_derivation_readable derivation \
                 ON derivation.derivation_ref=atom.derivation_ref \
               WHERE membership.problem_ref=problem.problem_ref \
                 AND membership.definition_revision=definition.revision AND membership.current \
           )",
    )
    .bind(problem_ref)
    .bind(definition_revision)
    .fetch_optional(database.pool())
    .await?
    .ok_or(CommentResearchEmbeddingError::DefinitionUnavailable)?;
    let name: String = row.get("name");
    let meaning: String = row.get("meaning");
    let definition_hash: String = row.get("definition_hash");
    let dimensions = space_dimensions(database, space_ref).await?;
    let text = format!("问题：{name}\n定义：{meaning}");
    Ok(ProblemDefinitionEmbeddingInput {
        problem_ref,
        definition_revision,
        space_ref,
        input_hash: content_hash(
            &json!({
                "kind":"problem_definition",
                "definitionHash":definition_hash,
                "revision":definition_revision,
                "text":text,
            })
            .to_string(),
        ),
        dimensions,
        text,
    })
}

async fn space_dimensions(
    database: &Database,
    space_ref: Uuid,
) -> Result<usize, CommentResearchEmbeddingError> {
    let dimensions: Option<i32> = sqlx::query_scalar(
        "SELECT dimensions FROM linggan_comment_research_embedding_space WHERE space_ref=$1",
    )
    .bind(space_ref)
    .fetch_optional(database.pool())
    .await?;
    dimensions
        .ok_or(CommentResearchEmbeddingError::SpaceUnavailable)
        .and_then(positive_dimensions)
}

fn positive_dimensions(dimensions: i32) -> Result<usize, CommentResearchEmbeddingError> {
    let dimensions =
        usize::try_from(dimensions).map_err(|_| CommentResearchEmbeddingError::InvalidEmbedding)?;
    if (1..=MAX_VECTOR_DIMENSIONS).contains(&dimensions) {
        Ok(dimensions)
    } else {
        Err(CommentResearchEmbeddingError::InvalidEmbedding)
    }
}

fn validate_vector(
    values: &[f64],
    dimensions: usize,
) -> Result<Value, CommentResearchEmbeddingError> {
    if values.len() != dimensions
        || values.iter().any(|value| !value.is_finite())
        || !values
            .iter()
            .map(|value| value * value)
            .sum::<f64>()
            .is_finite()
        || values.iter().map(|value| value * value).sum::<f64>() <= 0.0
    {
        return Err(CommentResearchEmbeddingError::InvalidEmbedding);
    }
    serde_json::to_value(values).map_err(|_| CommentResearchEmbeddingError::InvalidEmbedding)
}

fn value_to_vector(
    value: Value,
    dimensions: usize,
) -> Result<Vec<f64>, CommentResearchEmbeddingError> {
    let values: Vec<f64> = serde_json::from_value(value)
        .map_err(|_| CommentResearchEmbeddingError::InvalidEmbedding)?;
    validate_vector(&values, dimensions)?;
    Ok(values)
}

fn exact_cosine_candidates(
    query: &[f64],
    dimensions: usize,
    candidates: &[ProblemCandidateVector],
) -> Vec<ProblemCandidate> {
    let query_norm = query.iter().map(|value| value * value).sum::<f64>().sqrt();
    let mut matches = candidates
        .iter()
        .filter(|candidate| {
            candidate.dimensions == dimensions
                && candidate.values.len() == dimensions
                && candidate.values.iter().all(|value| value.is_finite())
        })
        .filter_map(|candidate| {
            let candidate_norm = candidate
                .values
                .iter()
                .map(|value| value * value)
                .sum::<f64>()
                .sqrt();
            if !query_norm.is_finite()
                || query_norm <= 0.0
                || !candidate_norm.is_finite()
                || candidate_norm <= 0.0
            {
                return None;
            }
            let cosine = query
                .iter()
                .zip(&candidate.values)
                .map(|(left, right)| left * right)
                .sum::<f64>()
                / (query_norm * candidate_norm);
            cosine.is_finite().then_some(ProblemCandidate {
                problem_ref: candidate.problem_ref,
                definition_revision: candidate.definition_revision,
                cosine: cosine.clamp(-1.0, 1.0),
            })
        })
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| {
        right
            .cosine
            .total_cmp(&left.cosine)
            .then(left.problem_ref.cmp(&right.problem_ref))
            .then(left.definition_revision.cmp(&right.definition_revision))
    });
    matches.truncate(MAX_CANDIDATES);
    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_recall_is_bounded_stable_and_skips_incomparable_vectors() {
        let primary = Uuid::from_u128(1);
        let secondary = Uuid::from_u128(2);
        let candidates = vec![
            ProblemCandidateVector {
                problem_ref: secondary,
                definition_revision: 1,
                dimensions: 2,
                values: vec![0.8, 0.2],
            },
            ProblemCandidateVector {
                problem_ref: primary,
                definition_revision: 1,
                dimensions: 2,
                values: vec![1.0, 0.0],
            },
            ProblemCandidateVector {
                problem_ref: Uuid::from_u128(3),
                definition_revision: 1,
                dimensions: 3,
                values: vec![1.0, 0.0, 0.0],
            },
        ];
        let recalled = exact_cosine_candidates(&[1.0, 0.0], 2, &candidates);
        assert_eq!(recalled.len(), 2);
        assert_eq!(recalled[0].problem_ref, primary);
        assert_eq!(recalled[1].problem_ref, secondary);
    }

    #[test]
    fn vector_validation_rejects_wrong_dimensions_nan_and_zero_norm() {
        assert!(matches!(
            validate_vector(&[1.0], 2),
            Err(CommentResearchEmbeddingError::InvalidEmbedding)
        ));
        assert!(matches!(
            validate_vector(&[f64::NAN, 0.0], 2),
            Err(CommentResearchEmbeddingError::InvalidEmbedding)
        ));
        assert!(matches!(
            validate_vector(&[0.0, 0.0], 2),
            Err(CommentResearchEmbeddingError::InvalidEmbedding)
        ));
    }
}
