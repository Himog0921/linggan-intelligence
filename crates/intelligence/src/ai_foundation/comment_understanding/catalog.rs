//! The domain's problem catalog as the model is shown it.
//!
//! G2 shows the whole catalog. A domain past the ceiling is refused rather than truncated: a
//! truncated catalog would make the model create duplicates of what it could not see.
use super::contract::{CatalogEntry, catalog_alias};
use crate::ai_foundation::store::FoundationError;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

pub const CATALOG_FULL_MAX: usize = 200;

#[derive(Debug, Clone)]
pub struct CatalogProblem {
    pub problem_ref: Uuid,
    pub label: String,
    pub actor: Option<String>,
    pub goal: String,
    pub barrier: String,
    pub context: Option<String>,
}

/// Live Problems of the domain, candidate and formal alike, oldest first, so a new Problem only
/// ever appends and the prompt prefix stays stable between units.
pub async fn read_in(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
) -> Result<Vec<CatalogProblem>, FoundationError> {
    let rows = sqlx::query("SELECT d.problem_ref,d.label,d.actor,d.goal,d.barrier,d.context FROM linggan_comment_problem_definition d JOIN linggan_comment_problem_resolution r ON r.problem_ref=d.problem_ref AND r.current_ref=d.problem_ref WHERE d.domain_ref=$1 ORDER BY d.created_at,d.problem_ref")
        .bind(domain)
        .fetch_all(&mut **tx)
        .await?;
    rows.into_iter()
        .map(|row| {
            Ok(CatalogProblem {
                problem_ref: row.try_get("problem_ref")?,
                label: row.try_get("label")?,
                actor: row.try_get("actor")?,
                goal: row.try_get("goal")?,
                barrier: row.try_get("barrier")?,
                context: row.try_get("context")?,
            })
        })
        .collect()
}

pub fn entries(problems: &[CatalogProblem]) -> Vec<CatalogEntry> {
    problems
        .iter()
        .enumerate()
        .map(|(index, problem)| CatalogEntry {
            alias: catalog_alias(index),
            label: problem.label.clone(),
            actor: problem.actor.clone(),
            goal: problem.goal.clone(),
            barrier: problem.barrier.clone(),
            context: problem.context.clone(),
        })
        .collect()
}
