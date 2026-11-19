//! T2: write what was accepted, in the transaction that settles the unit's invocation.
//!
//! Results, new Problems and support land together or not at all. A target's failure is a result
//! row of its own and never rolls back another target's result.
use super::{
    accept::{AcceptedTarget, AcceptedUnit, Assignment, NewProblem, TargetOutcome},
    contract::catalog_alias,
    unit::Unit,
};
use crate::ai_foundation::store::FoundationError;
use sqlx::{Postgres, Row, Transaction};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Published {
    pub accepted: usize,
    pub rejected: usize,
    pub new_problems: usize,
    pub ignored_entries: usize,
}

/// Serializes the writers of one domain's catalog. Units already run one at a time per domain;
/// this keeps that true for a write that is retried or recovered.
pub async fn lock_domain_in(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
) -> Result<(), FoundationError> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("comment-understanding-domain:{domain}"))
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub async fn write_failure_in(
    tx: &mut Transaction<'_, Postgres>,
    request_ref: Uuid,
    invocation: Uuid,
    code: &str,
) -> Result<(), FoundationError> {
    sqlx::query("INSERT INTO linggan_comment_research_result(request_ref,invocation_ref,result,failure_code) VALUES($1,$2,NULL,$3)")
        .bind(request_ref)
        .bind(invocation)
        .bind(code)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

fn position(alias: &str) -> Option<i32> {
    alias.strip_prefix('t')?.parse().ok()
}

/// `catalog` is the ordered list frozen at dispatch: alias `p<n>` is element `n`.
pub async fn publish_in(
    tx: &mut Transaction<'_, Postgres>,
    unit: &Unit,
    invocation: Uuid,
    catalog: &[Uuid],
    accepted: &AcceptedUnit,
) -> Result<Published, FoundationError> {
    lock_domain_in(tx, unit.domain_ref).await?;
    let frozen = |alias: &str| {
        position(alias).and_then(|p| unit.targets.iter().find(|target| target.position == p))
    };
    let existing: HashMap<String, Uuid> = catalog
        .iter()
        .enumerate()
        .map(|(index, problem)| (catalog_alias(index), *problem))
        .collect();
    let definitions: HashMap<Uuid, Uuid> = sqlx::query("SELECT problem_ref,definition_ref FROM linggan_comment_problem_definition WHERE problem_ref=ANY($1)")
        .bind(catalog)
        .fetch_all(&mut **tx)
        .await?
        .into_iter()
        .map(|row| Ok((row.try_get("problem_ref")?, row.try_get("definition_ref")?)))
        .collect::<Result<_, sqlx::Error>>()?;

    // A new Problem takes its source from the first signal that names it.
    let declared: HashMap<&str, &NewProblem> = accepted
        .new_problems
        .iter()
        .map(|p| (p.key.as_str(), p))
        .collect();
    let mut created: HashMap<String, (Uuid, Uuid)> = HashMap::new();
    for outcome in &accepted.targets {
        let TargetOutcome::Accepted(target) = outcome else {
            continue;
        };
        let Some(frozen) = frozen(&target.alias) else {
            return Err(FoundationError::Invalid);
        };
        for (ordinal, signal) in target.signals.iter().enumerate() {
            let Some(Assignment::New(key)) = &signal.assignment else {
                continue;
            };
            if created.contains_key(key) {
                continue;
            }
            let problem = declared.get(key.as_str()).ok_or(FoundationError::Invalid)?;
            let (problem_ref, definition_ref) = (Uuid::new_v4(), Uuid::new_v4());
            sqlx::query("INSERT INTO linggan_comment_problem_definition(definition_ref,problem_ref,domain_ref,label,actor,goal,barrier,context,source_request_ref,source_signal_ordinal,origin_invocation_ref) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)")
                .bind(definition_ref).bind(problem_ref).bind(unit.domain_ref).bind(&problem.label)
                .bind(&problem.actor).bind(&problem.goal).bind(&problem.barrier).bind(&problem.context)
                .bind(frozen.request_ref).bind(ordinal as i32).bind(invocation)
                .execute(&mut **tx).await?;
            created.insert(key.clone(), (problem_ref, definition_ref));
        }
    }

    let mut published = Published {
        new_problems: created.len(),
        ignored_entries: accepted.ignored_entries,
        ..Published::default()
    };
    for outcome in &accepted.targets {
        match outcome {
            TargetOutcome::Rejected { alias, code } => {
                let frozen = frozen(alias).ok_or(FoundationError::Invalid)?;
                write_failure_in(tx, frozen.request_ref, invocation, code).await?;
                published.rejected += 1;
            }
            TargetOutcome::Accepted(target) => {
                write_target_in(
                    tx,
                    unit,
                    invocation,
                    target,
                    &existing,
                    &definitions,
                    &created,
                )
                .await?;
                published.accepted += 1;
            }
        }
    }
    Ok(published)
}

async fn write_target_in(
    tx: &mut Transaction<'_, Postgres>,
    unit: &Unit,
    invocation: Uuid,
    target: &AcceptedTarget,
    existing: &HashMap<String, Uuid>,
    definitions: &HashMap<Uuid, Uuid>,
    created: &HashMap<String, (Uuid, Uuid)>,
) -> Result<(), FoundationError> {
    let frozen = position(&target.alias)
        .and_then(|p| unit.targets.iter().find(|t| t.position == p))
        .ok_or(FoundationError::Invalid)?;
    let problem_of = |assignment: &Assignment| match assignment {
        Assignment::Existing(alias) => existing.get(alias).copied(),
        Assignment::New(key) => created.get(key).map(|(problem, _)| *problem),
    };
    let result = target.result_json(frozen.source_ref, &problem_of);
    sqlx::query("INSERT INTO linggan_comment_research_result(request_ref,invocation_ref,result,failure_code) VALUES($1,$2,$3,NULL)")
        .bind(frozen.request_ref)
        .bind(invocation)
        .bind(&result)
        .execute(&mut **tx)
        .await?;
    for (ordinal, signal) in target.signals.iter().enumerate() {
        let Some(assignment) = &signal.assignment else {
            continue;
        };
        let definition = match assignment {
            Assignment::Existing(alias) => existing
                .get(alias)
                .and_then(|problem| definitions.get(problem))
                .copied(),
            Assignment::New(key) => created.get(key).map(|(_, definition)| *definition),
        }
        .ok_or(FoundationError::Invalid)?;
        sqlx::query("INSERT INTO linggan_comment_problem_membership(request_ref,signal_ordinal,definition_ref,origin_invocation_ref) VALUES($1,$2,$3,$4)")
            .bind(frozen.request_ref)
            .bind(ordinal as i32)
            .bind(definition)
            .bind(invocation)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}
